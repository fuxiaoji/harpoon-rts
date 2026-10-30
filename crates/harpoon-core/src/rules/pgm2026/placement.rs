//! Source-qualified RF input and the existing once-only placement executor.
//! The SATNAV coordinate allocation is deliberately not an FCS producer.
use super::*;
use crate::db::SensorKind;

fn section(w:&World,p:&Prepared)->Option<(String,Section)> {
    let name=&w.entity(p.actor)?.name;
    w.rules.pgm.as_ref()?.config.sections.iter().find(|(_,s)|s.briefing==p.request.briefing && s.members.contains(name)).map(|(key,s)|(key.clone(),s.clone()))
}
/// Section completeness is checked before budget reservation, which itself consumes a turn slot.
pub(super) fn section_declared(w:&World,p:&Prepared)->Result<(),String> {
    let Some((_,group))=section(w,p) else {return Ok(())};
    let state=w.rules.pgm.as_ref().ok_or("9.6.8 section state missing")?;
    for name in &group.members {
        let member=w.entities.values().find(|e|e.name==*name && e.alive && e.side==p.side).ok_or("9.6.8 section member unavailable")?;
        state.pending.get(&member.id).filter(|q|q.target==p.target && q.request.briefing==p.request.briefing && q.accepted_at==p.accepted_at).ok_or("9.6.8 section simultaneous declarations required")?;
    }
    Ok(())
}
pub(super) fn qualify(w: &World, p: &Prepared) -> Result<PlacementEvidence, String> {
    let actor = w.entity(p.actor).ok_or("9.6.8 RF FCS actor missing")?;
    let leader_id=if let Some((_,group))=section(w,p) {
        w.entities.values().find(|e|e.name==group.members[0] && e.side==p.side && e.alive).map(|e|e.id).ok_or("9.6.8 section RF leader unavailable")?
    } else {p.actor};
    let rf_actor=w.entity(leader_id).ok_or("9.6.8 RF leader absent")?;
    let mut best: Option<PlacementEvidence> = None;
    for fit in &rf_actor.sensors {
        if !w
            .db
            .sensors
            .get(&fit.sensor)
            .is_some_and(|d| matches!(d.kind, SensorKind::Radar | SensorKind::Esm))
        {
            continue;
        }
        let Ok(solution) = fc::own_air_surface_rf_solution(w, leader_id, p.target, &fit.sensor)
        else {
            continue;
        };
        if solution.quality == fc::Quality::NA {
            continue;
        }
        let evidence = PlacementEvidence {
            quality: solution.quality,
            provenance: json!({"kind":"native_rf","rule":"6.3/9.6.8","observer":leader_id,"sensor":fit.sensor,"at":w.tick,"solution":solution,"weapon_neutral":true}),
            placement: None,
            exemption: None,
        };
        if best
            .as_ref()
            .is_none_or(|old| evidence.quality > old.quality)
        {
            best = Some(evidence);
        }
    }
    if let Ok(Some(received)) =
        super::super::communications2026::received_solution(w, leader_id, p.target)
    {
        // A frozen ASCM/torpedo-tailored solution cannot silently become a
        // SATNAV RF solution. Accept only an explicitly weapon-neutral RF
        // producer, with real receiver/link evidence retained by the service.
        let neutral = received.solution.as_ref().is_some_and(|q| {
            !q.modifiers.keys().any(|key| {
                key.starts_with("ascm_") || key == "long_range_scout" || key == "torpedo_seeker"
            })
        });
        let rf =
            w.db.sensors
                .get(&received.source_sensor)
                .is_some_and(|d| matches!(d.kind, SensorKind::Radar | SensorKind::Esm));
        if neutral
            && rf
            && !received.refs.is_empty()
            && received.domain == super::super::communications2026::FcsDomain::Surface
            && received.quality != fc::Quality::NA
            && best
                .as_ref()
                .is_none_or(|old| received.quality > old.quality)
        {
            best = Some(PlacementEvidence {
                quality: received.quality,
                provenance: json!({"kind":"received_rf","evidence":received,"weapon_neutral":true}),
                placement: None,
                exemption: None,
            });
        }
    }
    if best.is_none() {
        if let Some(authored) = w.rules.pgm.as_ref().unwrap().config.briefings[&p.request.briefing]
            .fcs
            .as_ref()
        {
            if authored.source.starts_with("scenario_assumption:")
                && authored.quality != fc::Quality::NA
            {
                best = Some(PlacementEvidence {
                    quality: authored.quality,
                    provenance: json!({"kind":"scenario_rf_control","source":authored.source,"quality":authored.quality,"native":false,"rule":"6.3/9.6.8"}),
                    placement: None,
                    exemption: None,
                });
            }
        }
    }
    let mut evidence = best.ok_or("9.6.8 RF FCS/placement source pending: fixed briefing coordinates do not supply an observer-qualified or explicitly authored RF quality")?;
    // English2026 PDF108/9-7 explicitly exempts a ship (or another moving
    // target) within10nm from Weapons Placement. A stationary Base has no
    // equivalent exemption; target category here is the allocated knowledge.
    let b = &w.rules.pgm.as_ref().unwrap().config.briefings[&p.request.briefing];
    let source_point = if b.known_motion == Motion::Moving {
        let m = b.moving_lead.as_ref().ok_or("9.6.8 moving placement requires own estimate")?;
        let point = w.context.aviation_2026.as_ref().and_then(|c|c.surface_aimpoints.get(&b.aimpoint)).ok_or("9.6.8 moving estimate point missing")?.point;
        point.add(Vec2::from_bearing(m.course_deg).scaled(m.speed_kts * (w.tick-m.observed_at) as f64 / 3600.))
    } else { p.aim };
    if p.known_kind == PlatformKind::Ship && actor.pos.range_to(source_point) <= 10. {
        evidence.exemption = Some("English2026 PDF108/9-7: PGM attack against a ship within10nm may skip Weapons Placement; RF provenance is still required".into());
    }
    Ok(evidence)
}

/// Reject an incomplete section before any member offers the shared budget.
pub(super) fn section_ready(w:&World,p:&Prepared,epoch:u64)->Result<(),String> {
    let Some((_,group))=section(w,p) else{return Ok(())};
    let state=w.rules.pgm.as_ref().ok_or("9.6.8 section state missing")?;
    let mut exemption=None;
    for name in &group.members {
        let member=w.entities.values().find(|e|e.name==*name && e.alive && e.side==p.side).ok_or("9.6.8 section member unavailable")?;
        let old=state.pending.get(&member.id).filter(|q|q.target==p.target && q.request.briefing==p.request.briefing && q.accepted_at==p.accepted_at).ok_or("9.6.8 section simultaneous declarations required")?;
        if budget::decision(w,&old.identity)!=budget::Decision::Reserved || !matches!(ac::surface::defense_gate(w,member.id),ac::surface::DefenseGate::Ready{epoch:e} if e==epoch) {return Err("9.6.8 section member is not ready in common defense epoch".into());}
        let member_ready=super::ready(w,member.id,&old.request)?;
        let is_exempt=qualify(w,&member_ready)?.exemption.is_some();
        if exemption.is_some_and(|old|old!=is_exempt) {return Err("9.6.8 mixed section placement exemption pending".into());}
        exemption=Some(is_exempt);
    }
    Ok(())
}
pub(super) fn commit(w: &mut World, p: &Prepared) -> Result<PlacementEvidence, String> {
    let mut evidence = p.fcs.clone().ok_or("9.6.8 RF FCS/placement was not prepared")?;
    let group=section(w,p);
    let identity = if let Some((key,group))=&group {
        let identity=format!("9.6.8.section:{}:{}:rf-placement",key,p.accepted_at);
        if let Some(cached)=w.rules.pgm.as_ref().unwrap().section_placements.get(&identity) {return Ok(cached.clone());}
        let state=w.rules.pgm.as_ref().unwrap();
        for name in &group.members {
            let member=w.entities.values().find(|e|e.name==*name && e.alive && e.side==p.side).ok_or("9.6.8 section member unavailable")?;
            let pending=state.pending.get(&member.id).filter(|q|q.target==p.target && q.request.briefing==p.request.briefing && q.accepted_at==p.accepted_at && q.fcs.is_some()).ok_or("9.6.8 section simultaneous ready claims required")?;
            let record=w.rules.aircombat.as_ref().and_then(|a|a.attack_budget.records.get(&pending.identity)).ok_or("9.6.8 section ticket missing")?;
            if record.ticket.as_ref().is_none_or(|t|t.at!=w.tick) && member.id!=p.actor {return Err("9.6.8 section same-phase ready tickets required".into());}
            if pending.fcs.as_ref().unwrap().exemption.is_some()!=evidence.exemption.is_some() {return Err("9.6.8 mixed section placement exemption pending".into());}
        }
        identity
    } else {format!("{}:rf-placement",p.identity)};
    if evidence.exemption.is_none() {
        let mut state=std::mem::take(&mut w.rules.aaw.as_mut().ok_or("9.6.8 RF placement state missing")?.firecontrol);
        let result=fc::placement(w,&mut state,&identity,evidence.quality,0);
        w.rules.aaw.as_mut().unwrap().firecontrol=state;
        evidence.placement=Some(result?);
    }
    if group.is_some() {w.rules.pgm.as_mut().unwrap().section_placements.insert(identity,evidence.clone());}
    Ok(evidence)
}

pub(super) fn terminal(evidence: &Option<PlacementEvidence>) -> Result<(), String> {
    let evidence = evidence.as_ref().ok_or("9.6.8 saved physical flight lacks RF FCS/placement evidence; no retroactive quality or dice may be invented")?;
    if evidence.exemption.is_some() {
        return Ok(());
    }
    match evidence.placement.as_ref() {
        Some(p) if p.placed => Ok(()),
        Some(_) => Err("9.6.8 RF placement failed; original D10/deviation retained, SATNAV ground-impact/placement interaction source pending".into()),
        None => Err("9.6.8 RF FCS exists but Weapons Placement evidence is missing".into()),
    }
}

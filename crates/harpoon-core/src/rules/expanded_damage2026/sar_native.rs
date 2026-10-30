//! 14.8 physical survivor points consumed by the real chapter-5 lookout.
//! Supported subset: unambiguous source people classes, daylight open water,
//! ship Visual, including a physically drifted point with declared geography.
//! Other domains remain source-blocked, never truth contacts.
use super::{SurvivorReport, Survivors};
use crate::db::SensorKind;
use crate::rules::detection2026 as d;
use crate::types::{DayNight, PlatformKind};
use crate::{EntityId, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Private evidence from a real scan of the original living platform. It is
/// not a shared Contact and never publishes friendly or enemy true positions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LossWitness {
    observer: EntityId,
    target: EntityId,
    sensor: String,
    at: i64,
}

// A scan result cannot be deserialized or supplied by a command/observation.
struct SuccessfulScan {
    observer: EntityId,
    group: String,
    sensor: String,
    at: i64,
    geometry: d::ReportGeometry,
}
pub(crate) struct ScanBatch {
    reports: Vec<SuccessfulScan>,
    witnesses: Vec<LossWitness>,
    traces: Vec<(EntityId, Value)>,
}

fn evaluate(
    w: &World,
    state: &mut d::State,
    observer: EntityId,
    sensor: &str,
    key: &str,
    group: &Survivors,
) -> Result<(d::ScanDecision, Option<d::ReportGeometry>), String> {
    let o = w.entity(observer).ok_or("survivor observer absent")?;
    if !o.alive || o.kind != PlatformKind::Ship {
        return Err("14.8 native survivor search currently requires an actual surface ship".into());
    }
    if w.day_night != DayNight::Day {
        return Err("14.8 night survivor search source combination pending".into());
    }
    let def = w.db.sensors.get(sensor).ok_or("survivor lookout absent")?;
    if def.kind != SensorKind::Visual || !o.sensors.iter().any(|s| s.sensor == sensor) {
        return Err("14.8 requires an actually installed Visual lookout".into());
    }
    if crate::sensors::sensor_damaged(o, sensor) || !super::sensor_operational(w, observer, sensor)?
    {
        return Err("14.8 physical lookout unavailable".into());
    }
    if let Some(receiver) = state.config.bindings.get(&format!("{}:{sensor}", o.class)) {
        if !matches!(receiver,d::Receiver::Visual{source} if !source.trim().is_empty()) {
            return Err("14.8 lookout receiver source binding is not Visual".into());
        }
    }
    super::survivor_position_source(w,group)?;
    let env = state
        .config
        .environment
        .as_ref()
        .filter(|e| !e.source.trim().is_empty())
        .ok_or("14.8 Visual requires explicit source/scenario environment")?;
    if env.cloud.is_some() || env.ice != 0 {
        return Err("14.8 cloud/ice survivor search source combination pending".into());
    }
    if w.context
        .terrain
        .as_ref()
        .is_some_and(|chart| chart.blocked(o.pos, group.position))
    {
        return Err("14.8 survivor lookout line is blocked by land".into());
    }
    let observer_size = if let Some(p) = state
        .config
        .targets
        .get(&o.class)
        .filter(|p| !p.source.trim().is_empty())
    {
        p.size
    } else {
        let sizes = &crate::rules::operations::tables()["radar"]["physical_sizes"];
        let letter = sizes[&o.class]
            .as_str()
            .and_then(|s| s.as_bytes().first())
            .copied()
            .filter(|b| (b'A'..=b'H').contains(b))
            .ok_or("14.8 actual observer physical size is not source-verified")?;
        usize::from(letter - b'A')
    };
    let profile = super::survivor_sighting_profile(w, key, false)?;
    // Survivors have no wake. They are not stationary ships: do not additionally
    // apply the ship <=5kt visibility-row modifier to their special14.8 profile.
    let mods = d::VisualModifiers::default();
    let base = d::visual_base(
        env.visibility_percent,
        observer_size,
        profile.physical_size,
        false,
        false,
        false,
        &mods,
    )? * profile.no_wake_multiplier;
    // The ordinary native Visual path uses exactly this receiver/Tac die for
    // every target. Additional hidden groups consume no global RNG or new die.
    let visual_key = format!("5.8.visual:{}:{sensor}", observer.0);
    let (roll, reused) = d::trial(state, w.seed, w.tick, &visual_key, super::sar_tactical_seconds());
    let visible_range = d::visual_detection_range(
        base,
        roll,
        d::visual_roll_modifier(&mods, profile.physical_size),
    )?;
    let track_key = format!("14.8.sar:{}:{sensor}:{key}", observer.0);
    let held = state.tracks.get(&track_key).is_some_and(|t| t.held);
    let distance_kyd = o.pos.range_to(group.position) * 2025.3718285 / 1000.;
    // The table note is based on actual range, not the modified visibility row.
    let automatic_close = distance_kyd < 0.3;
    let detected = automatic_close || distance_kyd <= if held { base } else { visible_range };
    state.tracks.entry(track_key).or_default().held = detected;
    let scan = d::ScanDecision {
                    source_point:None,
        sonar_attempt: None,
        detected,
        known_category: None,
        group: None,
        bearing_only: false,
        identify_class: false,
        class_label: None,
        range_known: detected,
        bearing_error_deg: None,
        fix_quality: None,
        trace: json!({"rule":"14.8/5.8","profile":profile.category,"physical_size":profile.physical_size,"signature":profile.signature,"no_wake_multiplier":profile.no_wake_multiplier,"base_kyd":base,"detection_kyd":visible_range,"distance_kyd":distance_kyd,"automatic_close_under_0_3_kyd":automatic_close,"held":held,"d100":roll,"reused":reused,"detected":detected}),
    };
    let geometry = detected.then(|| {
        d::project_point_report(
            w,
            state,
            o,
            group.position,
            json!({"survivor_group":key}),
            &format!("14.8.bearing:{}:{sensor}:{key}", observer.0),
            sensor,
            &scan,
        )
        .0
    });
    Ok((scan, geometry))
}

pub(crate) fn scan_world(w: &World, state: &mut d::State) -> ScanBatch {
    let mut batch = ScanBatch {
        reports: vec![],
        witnesses: vec![],
        traces: vec![],
    };
    if !super::full(w) {
        return batch;
    }
    let Some(s) = w
        .rules
        .expanded_damage
        .as_ref()
        .filter(|s| s.config.optional.search_and_rescue)
    else {
        return batch;
    };
    for o in w.entities.values().filter(|o| {
        o.alive && matches!(o.kind, PlatformKind::Ship | PlatformKind::Aircraft)
            && !crate::rules::ew2026::is_object(w, o.id)
    }) {
        if !crate::rules::unmanned2026::detection_allowed(w, o.id)
            || !crate::rules::unmanned2026::allow_side_projection(w, o.id)
        {
            continue;
        }
        for inst in &o.sensors {
            if !w
                .db
                .sensors
                .get(&inst.sensor)
                .is_some_and(|d| d.kind == SensorKind::Visual)
            {
                continue;
            }
            // Native combat scans omit friendly targets. This independent
            // lookout evidence admits either side without minting a Contact.
            for t in w.entities.values().filter(|t| t.alive && t.id != o.id
                && matches!(t.kind, PlatformKind::Ship | PlatformKind::Sub | PlatformKind::Aircraft)
                && super::fit(w,t.id).is_some_and(|f|f.crew.is_some())) {
                if visible_platform(w,state,o.id,&inst.sensor,t.id).is_some() {
                    batch.witnesses.push(LossWitness { observer:o.id,target:t.id,
                        sensor:inst.sensor.clone(),at:w.tick });
                }
            }
            if o.kind != PlatformKind::Ship { continue; }
            for (key, group) in &s.survivors {
                if group.count <= group.rescued {
                    continue;
                }
                let result = evaluate(w, state, o.id, &inst.sensor, key, group);
                let detail = match &result {
                    Ok((scan, _)) => scan.trace.clone(),
                    Err(reason) => json!({"rule":"14.8/5.8","blocked":reason}),
                };
                let trace_key = format!("14.8.scan:{}:{}:{key}", o.id.0, inst.sensor);
                let window = w.tick.div_euclid(super::sar_tactical_seconds());
                // Retain dice once per Tactical Turn and report actual changes;
                // a successful scan's `reused` flag must not flood the trace.
                let marker=json!({"blocked":detail.get("blocked"),"d100":detail.get("d100"),"detected":detail.get("detected"),"held":detail.get("held")}).to_string();
                if !state
                    .reported
                    .get(&trace_key)
                    .is_some_and(|(t, m)| *t == window && *m == marker)
                {
                    state.reported.insert(trace_key, (window, marker));
                    batch.traces.push((
                        o.id,
                        json!({"sensor":inst.sensor,"private_group":key,"result":detail}),
                    ));
                }
                if let Ok((scan, Some(geometry))) = result {
                    if scan.detected {
                        batch.reports.push(SuccessfulScan {
                            observer: o.id,
                            group: key.clone(),
                            sensor: inst.sensor.clone(),
                            at: w.tick,
                            geometry,
                        });
                    }
                }
            }
        }
    }
    batch
}

pub(crate) fn commit(w: &mut World, batch: ScanBatch) {
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        let max_age = witness_max_age();
        s.loss_witnesses.retain(|_,r| (0..=max_age).contains(&(w.tick-r.at)));
        for r in batch.witnesses {
            let key=format!("{}:{}:{}",r.observer.0,r.sensor,r.target.0);
            s.loss_witnesses.insert(key,r);
        }
    }
    for report in batch.reports {
        if report.at != w.tick {
            continue;
        }
        let Some(o) = w.entity(report.observer).filter(|o| o.alive) else {
            continue;
        };
        if !o.sensors.iter().any(|s| s.sensor == report.sensor)
            || !super::sensor_operational(w, report.observer, &report.sensor).unwrap_or(false)
        {
            continue;
        }
        let report_side = o.side;
        let side = report_side.name().to_string();
        let Some(position) = report.geometry.measured_position else {
            continue;
        };
        let Ok(profile)=super::survivor_sighting_profile(w,&report.group,false) else {continue;};
        if w.rules.expanded_damage.as_ref().and_then(|s|s.survivors.get(&report.group))
            .is_none_or(|g|super::survivor_position_source(w,g).is_err()) {continue;}
        let Some(s) = w.rules.expanded_damage.as_mut() else {
            continue;
        };
        let Some(g) = s.survivors.get_mut(&report.group) else {
            continue;
        };
        // Snapshot only the geometry produced by this actual successful scan.
        // There is deliberately no fallback to the current private group point.
        g.observed_by.insert(report.observer);
        g.sightings.insert(
            report.observer,
            SurvivorReport {
                observed_at: report.at,
                position,
                category: profile.category,
                side: Some(report_side),
            },
        );
        let labels = s.survivor_labels.entry(side).or_default();
        if !labels.contains_key(&report.group) {
            let label = format!("survivors-{}", labels.len() + 1);
            labels.insert(report.group, label);
        }
    }
    for (observer, detail) in batch.traces {
        super::private_trace(w, observer, "14.8.native_visual_scan", detail);
    }
}

fn witness_max_age() -> i64 {
    super::tables()["sar"]["loss_witness_scan_age_seconds"].as_i64()
        .filter(|v|*v>=0).expect("declared SAR loss witness phase mapping")
}

fn visible_platform(w:&World,state:&mut d::State,observer:EntityId,sensor:&str,target:EntityId)
    -> Option<d::ReportGeometry> {
    let o=w.entity(observer).filter(|o|o.alive)?;
    let t=w.entity(target).filter(|t|t.alive)?;
    if !o.sensors.iter().any(|i|i.sensor==sensor)
        || w.db.sensors.get(sensor)?.kind!=SensorKind::Visual
        || !crate::rules::unmanned2026::detection_allowed(w,observer)
        || !crate::rules::unmanned2026::allow_side_projection(w,observer)
        || w.context.terrain.as_ref().is_some_and(|chart|chart.blocked(o.pos,t.pos)) {
        return None;
    }
    let scan=d::evaluate_world(w,state,observer,sensor,target).ok()?;
    if !scan.detected || !scan.range_known {return None;}
    let (report,_)=d::project_report(w,state,o,t,sensor,&scan);
    report.measured_position?;
    Some(d::ReportGeometry {observer_position:report.observer_position,
        bearing_deg:report.bearing_deg,measured_position:report.measured_position,
        bearing_error_deg:report.bearing_error_deg})
}

/// Called before the original entity dies. A fresh actual lookout scan is
/// rechecked at the loss; a stale Contact alone is never a loss witness.
pub(crate) fn witness_loss(w:&mut World,target:EntityId,group:&str) {
    let Some(s)=w.rules.expanded_damage.as_ref() else{return;};
    if s.survivors.get(group).is_none_or(|g|g.count<=g.rescued) {return;}
    let prior:Vec<_>=s.loss_witnesses.values().filter(|r|r.target==target
        && (0..=witness_max_age()).contains(&(w.tick-r.at))).cloned().collect();
    if prior.is_empty(){return;}
    let Some(mut detection)=w.rules.detection.take() else{return;};
    let mut reports=vec![];
    for r in prior {
        if let Some(geometry)=visible_platform(w,&mut detection,r.observer,&r.sensor,target) {
            if let (Some(position),Some(o))=(geometry.measured_position,w.entity(r.observer)) {
                reports.push((r.observer,r.sensor,o.side,position));
            }
        }
    }
    w.rules.detection=Some(detection);
    for (observer,sensor,side,position) in reports {
        let s=w.rules.expanded_damage.as_mut().unwrap();
        let g=s.survivors.get_mut(group).unwrap();
        g.observed_by.insert(observer);
        g.sightings.insert(observer,SurvivorReport { observed_at:w.tick,position,
            category:"witnessed_loss".into(),side:Some(side) });
        let labels=s.survivor_labels.entry(side.name().into()).or_default();
        if !labels.contains_key(group) {
            labels.insert(group.into(),format!("survivors-{}",labels.len()+1));
        }
        super::private_trace(w,observer,"14.8.witnessed_loss",
            json!({"private_group":group,"lost_unit":target,"sensor":sensor,
                "measured_position":position,"observed_at":w.tick}));
    }
}

use super::*;
use crate::db::{SensorKind, WeaponKind};
use crate::rules::firecontrol2026::{Quality, Solution};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageRef {
    pub sender: EntityId,
    pub id: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FcsDomain {
    Surface,
    Asw,
    Air,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CuePurpose {
    Rf,
    Acoustic,
    AirDefenseReaction,
    AswPlacement,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CueEvidence {
    pub modifier: u8,
    pub refs: Vec<MessageRef>,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FcsEvidence {
    pub quality: Quality,
    pub solution: Option<Solution>,
    pub aim: Option<crate::Vec2>,
    pub formed_at: i64,
    pub source_observer: EntityId,
    pub source_sensor: String,
    pub domain: FcsDomain,
    pub refs: Vec<MessageRef>,
    pub source: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct ExchangeState {
    compositions: BTreeMap<String, Composition>,
}
impl ExchangeState {
    pub(super) fn is_empty(&self) -> bool {
        self.compositions.is_empty()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Composition {
    actor: EntityId,
    target: EntityId,
    side: Side,
    contact: ContactId,
    evidence: FcsEvidence,
}

pub(super) fn validate_control(w: &World, side: Side, actor: EntityId) -> Result<(), String> {
    if w.over || crate::exercise::locked(w, side) {
        return Err("6.3 communications unavailable after game end/exercise lock".into());
    }
    owned(w, side, actor)?;
    check_state(w, w.rules.communications.as_ref().ok_or_else(source_error)?)
}
pub(super) fn report_live(w: &World, s: &State, r: &NativeReport) -> bool {
    let Some(o) = w.entity(r.report.observer).filter(|e| e.alive) else {
        return false;
    };
    if s.disabled_causes
        .get(&o.id)
        .is_some_and(|causes| !causes.is_empty())
        || crate::rules::unmanned2026::is_configured(w, o.id)
            && !crate::rules::unmanned2026::allow_side_projection(w, o.id)
    {
        return false;
    }
    if report_source_fingerprint(w, &r.report).ok().as_ref() != Some(&r.source_fingerprint) {
        return false;
    }
    if crate::sensors::sensor_damaged(o, &r.report.sensor) {
        return false;
    }
    let Some(inst) = o.sensors.iter().find(|i| i.sensor == r.report.sensor) else {
        return false;
    };
    if !inst.is_active_kind(r.report.sensor_kind) {
        return false;
    }
    if r.report.sensor_kind == SensorKind::Radar
        && (!crate::sensors::radar_emitting(o) || o.systems.radar <= 0.05)
    {
        return false;
    }
    if !crate::rules::expanded_damage2026::sensor_operational(w, o.id, &r.report.sensor)
        .unwrap_or(false)
    {
        return false;
    }
    s.scans
        .get(&report_key(
            r.report.observer,
            &r.report.sensor,
            r.report.target,
        ))
        .is_some_and(|scan| scan.detected && scan.at >= r.report.at && scan.at <= w.tick)
}
fn usable<'a>(
    w: &World,
    s: &'a State,
    receiver: EntityId,
    target: EntityId,
) -> impl Iterator<Item = &'a Message> + 'a {
    let now = w.tick;
    // This closure captures only references with the same call lifetime.
    s.messages.values().filter(move |m| {
        m.status == Status::Delivered
            && m.receivers.contains(&receiver)
            && m.snapshot.report.target == target
            && m.terminal_at
                .is_some_and(|at| at.div_euclid(180) == now.div_euclid(180))
    })
}
/// Actual delivered located geometry, for consumers explicitly accepting a
/// friendly report. This is the sender's immutable measurement, not a cue,
/// fused side icon, targeting quality or extrapolated target position.
pub fn received_located_report(
    w: &World,
    receiver: EntityId,
    target: EntityId,
) -> Result<Option<SensorReport>, String> {
    if !active(w) {
        return Ok(None);
    }
    let Some(state) = w.rules.communications.as_ref() else {
        return Ok(None);
    };
    check_state(w, state)?;
    Ok(usable(w, state, receiver, target)
        .filter(|m| {
            let report = &m.snapshot.report;
            report.bearing_alternatives.is_empty()
                && report
                    .measured_position
                    .is_some_and(|p| p.x.is_finite() && p.y.is_finite())
                && report_live(w, state, &m.snapshot)
                && edges(w, &state.config, m.side, m.sender, &m.fit, &[receiver]).is_ok()
        })
        .max_by_key(|m| (m.snapshot.report.at, m.sender, m.id))
        .map(|m| m.snapshot.report.clone()))
}
pub fn cue_evidence(
    w: &World,
    receiver: EntityId,
    target: EntityId,
    purpose: CuePurpose,
) -> CueEvidence {
    let mut result = CueEvidence {
        source: "English2026 6.3.9; RF/Acoustic TDL Cue; 8.1.1/8.5.2; current shared epoch".into(),
        ..Default::default()
    };
    if !active(w) {
        return result;
    }
    let Some(s) = w.rules.communications.as_ref() else {
        return result;
    };
    if check_state(w, s).is_err() {
        return result;
    }
    for m in usable(w, s, receiver, target) {
        let level = match m.receiver_kinds.get(&receiver) {
            Some(LinkKind::NearRealTime) => 1,
            Some(LinkKind::RealTime) => 2,
            _ => continue,
        };
        if !report_live(w, s, &m.snapshot) {
            continue;
        }
        let kind = m.snapshot.report.sensor_kind;
        let valid = match purpose {
            CuePurpose::Rf => matches!(kind, SensorKind::Radar | SensorKind::Esm),
            CuePurpose::Acoustic | CuePurpose::AswPlacement => {
                matches!(kind, SensorKind::SonarActive | SensorKind::SonarPassive)
            }
            CuePurpose::AirDefenseReaction => kind == SensorKind::Radar,
        };
        if valid {
            result.modifier = result.modifier.max(level);
            result.refs.push(MessageRef {
                sender: m.sender,
                id: m.id,
            });
        }
    }
    result.refs.sort();
    result.refs.dedup();
    result
}
pub(super) fn prepare_solution(
    w: &World,
    actor: EntityId,
    report: &NativeReport,
    tx: &VoiceFit,
    receivers: &BTreeMap<EntityId, LinkKind>,
    weapon: &str,
    speed_index: Option<usize>,
) -> Result<FcsEvidence, String> {
    if !matches!(tx.warning_kind, LinkKind::NearRealTime | LinkKind::RealTime)
        || !has_real_time_targeting(w, actor)
        || receivers
            .values()
            .any(|k| !matches!(k, LinkKind::NearRealTime | LinkKind::RealTime))
    {
        return Err("6.3.9 existing FCS transfer requires RT targeting transmitter and compatible RT/NRT receivers".into());
    }
    let s = w.rules.communications.as_ref().ok_or_else(source_error)?;
    if !report_live(w, s, report) {
        return Err("6.3.9 transmitting platform is not currently in native contact".into());
    }
    let e = w.entity(actor).ok_or_else(source_error)?;
    if !e.weapons.iter().any(|m| m.weapon == weapon && m.ammo > 0) {
        return Err("6.3.9 source FCS weapon is not actually fitted/available".into());
    }
    let def =
        w.db.weapons
            .get(weapon)
            .ok_or("6.3.9 unknown source weapon")?;
    let (domain, generation, speed) = match def.kind {
        WeaponKind::Torpedo | WeaponKind::Lwt => {
            let p = w
                .rules
                .torpedoes
                .as_ref()
                .and_then(|s| s.config.weapons.get(weapon))
                .ok_or("6.3.9 torpedo FCS source setting unavailable")?;
            let setting = if let Some(index) = speed_index {
                p.settings.get(index)
            } else {
                p.settings
                    .iter()
                    .find(|s| (s.speed_kts - def.speed_kts).abs() < 1e-9)
            }
            .ok_or("6.3.9 exact source torpedo speed setting required")?;
            (FcsDomain::Asw, p.generation.max(1), setting.speed_kts)
        }
        WeaponKind::Ascm => {
            let row = &crate::rules::surface2026::tables()["weapons"][weapon];
            let generation = row["seeker_generation"]
                .as_u64()
                .ok_or("6.3.9 source ASCM seeker generation unavailable")?
                as usize;
            let speed = row["speed_kts"]
                .as_f64()
                .ok_or("6.3.9 source ASCM speed unavailable")?;
            (FcsDomain::Surface, generation, speed)
        }
        _ => return Err(
            "6.3.9 existing FCS producer for this weapon domain awaits a verified source executor"
                .into(),
        ),
    };
    let q = crate::rules::firecontrol2026::own_sensor_solution(
        w,
        actor,
        report.report.target,
        &report.report.sensor,
        generation,
        speed,
    )?;
    if !report.report.bearing_alternatives.is_empty() {
        return Err(
            "6.3.9 two-ray bearing ambiguity cannot be treated as a unique targeting fix".into(),
        );
    }
    let aim = report
        .report
        .measured_position
        .ok_or("6.3.9 selected native source report has no measured position")?;
    if q.quality == Quality::NA {
        return Err("6.3.9 No Attack is not an existing targeting-quality solution".into());
    }
    Ok(FcsEvidence {
        quality: q.quality,
        solution: Some(q),
        aim: Some(aim),
        formed_at: w.tick,
        source_observer: actor,
        source_sensor: report.report.sensor.clone(),
        domain,
        refs: Vec::new(),
        source: "English2026 6.3.9 existing own targeting-system solution; no remote recursion"
            .into(),
    })
}
fn has_real_time_targeting(w: &World, actor: EntityId) -> bool {
    let Some(e) = w.entity(actor) else {
        return false;
    };
    w.rules
        .communications
        .as_ref()
        .and_then(|s| s.config.units.get(&e.name))
        .is_some_and(|u| {
            authored(&u.source)
                && u.fits.values().any(|f| {
                    authored(&f.source)
                        && f.warning_kind == LinkKind::RealTime
                        && f.targeting_system
                        && f.operational
                })
        })
}
pub fn received_solution(
    w: &World,
    receiver: EntityId,
    target: EntityId,
) -> Result<Option<FcsEvidence>, String> {
    if !active(w) {
        return Ok(None);
    }
    let Some(s) = w.rules.communications.as_ref() else {
        return Ok(None);
    };
    check_state(w, s)?;
    let mut best = None::<FcsEvidence>;
    for m in usable(w, s, receiver, target) {
        let Some(original) = m.solution.as_ref() else {
            continue;
        };
        if !report_live(w, s, &m.snapshot) {
            continue;
        }
        let mut evidence = original.clone();
        match m.receiver_kinds.get(&receiver) {
            Some(LinkKind::RealTime) => {}
            Some(LinkKind::NearRealTime) if evidence.domain != FcsDomain::Air => {
                let key = format!("{:?}", evidence.quality);
                evidence.quality =
                    serde_json::from_value(tables()["tdl_source"]["nrt_transfer"][key].clone())
                        .map_err(|_| "6.3.1/6.3.9 sourced NRT quality mapping unavailable")?;
                if let Some(q) = evidence.solution.as_mut() {
                    q.quality = evidence.quality;
                    q.modifiers.insert("received_nrt_quality_levels".into(), -1);
                }
                evidence
                    .source
                    .push_str("; NRT receiver one printed quality level lower (6.3.1/6.3.9)");
            }
            _ => continue,
        }
        evidence.refs = vec![MessageRef {
            sender: m.sender,
            id: m.id,
        }];
        if best.as_ref().is_none_or(|b| evidence.quality > b.quality) {
            best = Some(evidence);
        }
    }
    // A declared multi-source computation is independent from an existing FCS
    // transfer; it cannot silently downgrade a valid native own solution.
    if let Some(c) = s
        .exchange
        .compositions
        .get(&format!("{}:{}", receiver.0, target.0))
    {
        if c.evidence.formed_at.div_euclid(180) == w.tick.div_euclid(180)
            && best.as_ref().is_none_or(|b| c.evidence.quality > b.quality)
        {
            best = Some(c.evidence.clone());
        }
    }
    Ok(best)
}
fn passive(kind: SensorKind) -> Option<bool> {
    match kind {
        SensorKind::Esm | SensorKind::SonarPassive => Some(true),
        SensorKind::Radar | SensorKind::SonarActive => Some(false),
        _ => None,
    }
}
fn acoustic(kind: SensorKind) -> bool {
    matches!(kind, SensorKind::SonarActive | SensorKind::SonarPassive)
}
pub(super) fn prepare_composition(
    w: &World,
    side: Side,
    actor: EntityId,
    action: &Action,
) -> Result<Composition, String> {
    validate_control(w, side, actor)?;
    let Action::CombineReports {
        contact,
        own_sensor,
        messages,
        aim_source,
    } = action
    else {
        return Err(source_error());
    };
    let s = w.rules.communications.as_ref().unwrap();
    let target = w.sides[side.index()]
        .contacts
        .get(contact)
        .filter(|c| !c.stale)
        .ok_or("6.3.10 current own source label required")?
        .about;
    let mut reports = Vec::<(EntityId, Option<MessageRef>, &NativeReport)>::new();
    if let Some(sensor) = own_sensor {
        let r = s
            .reports
            .get(&report_key(actor, sensor, target))
            .ok_or("6.3.10 own contributor has no native report")?;
        if !report_live(w, s, r) {
            return Err(source_error());
        }
        reports.push((actor, None, r));
    }
    let mut unique = BTreeSet::new();
    for reference in messages {
        if !unique.insert(*reference) {
            return Err("6.3.10 repeated message is not a second source".into());
        }
        let m = s
            .messages
            .get(&message_key(reference.sender, reference.id))
            .filter(|m| {
                m.side == side
                    && m.status == Status::Delivered
                    && m.receivers.contains(&actor)
                    && m.snapshot.report.target == target
                    && m.kind == LinkKind::Voice
                    && m.solution.is_none()
            })
            .ok_or("6.3.10 contributor must be a received voice report of this contact")?;
        if m.terminal_at
            .is_none_or(|at| at.div_euclid(180) != w.tick.div_euclid(180))
            || report_source_fingerprint(w, &m.snapshot.report)? != m.snapshot.source_fingerprint
        {
            return Err(source_error());
        }
        reports.push((m.sender, Some(*reference), &m.snapshot));
    }
    let observers: BTreeSet<_> = reports.iter().map(|r| r.0).collect();
    if observers.len() < 2 {
        return Err("6.3.10 at least two distinct native observers required".into());
    }
    let first = reports[0].2;
    let mode =
        passive(first.report.sensor_kind).ok_or("6.3.10 unsupported source sensor family")?;
    if reports
        .iter()
        .any(|(_, _, r)| passive(r.report.sensor_kind) != Some(mode))
    {
        return Err(
            "6.3.10 mixed active/passive composition has no sourced quality procedure".into(),
        );
    }
    let mut evidence_report = first;
    let aim = if mode {
        None
    } else {
        let chosen = if let Some(reference) = aim_source {
            reports
                .iter()
                .find(|(_, r, _)| r.as_ref() == Some(reference))
                .ok_or("6.3.10 aim source must be one selected received report")?
        } else {
            reports
                .iter()
                .find(|(observer, _, _)| *observer == actor)
                .ok_or(
                    "6.3.10 select an explicit received active aim source; no invented averaging",
                )?
        };
        if !chosen.2.report.bearing_alternatives.is_empty() {
            return Err(
                "6.3.10 selected two-ray report is not a unique measured aim source".into(),
            );
        }
        evidence_report = chosen.2;
        Some(
            chosen
                .2
                .report
                .measured_position
                .ok_or("6.3.10 active source has no measured rough position")?,
        )
    };
    let q = if mode { Quality::NA } else { Quality::Poor };
    Ok(Composition{actor,target,side,contact:*contact,evidence:FcsEvidence{quality:q,solution:None,aim,formed_at:w.tick,source_observer:evidence_report.report.observer,source_sensor:evidence_report.report.sensor.clone(),domain:if acoustic(evidence_report.report.sensor_kind){FcsDomain::Asw}else{FcsDomain::Surface},refs:unique.into_iter().collect(),source:"English2026 6.3.10 explicit multi-observer non-TDL computation; passive NA / active Poor; same-mode RF/acoustic allowed; selected source aim, no new triangulation error model".into()}})
}
pub(super) fn apply_composition(
    w: &mut World,
    side: Side,
    actor: EntityId,
    action: &Action,
) -> Result<(), String> {
    let c = prepare_composition(w, side, actor, action)?;
    let detail = json!({"contact":c.contact,"quality":c.evidence.quality,"refs":c.evidence.refs,"source":c.evidence.source,"bearing_only_launch_retained":c.evidence.quality==Quality::NA});
    w.rules
        .communications
        .as_mut()
        .unwrap()
        .exchange
        .compositions
        .insert(format!("{}:{}", actor.0, c.target.0), c);
    emit(w, side, actor, "6.3.10.multi_source_composition", detail);
    Ok(())
}
pub(super) fn enqueue_automatic(w: &mut World, include_current_batch: bool) {
    let Some(s) = w.rules.communications.as_ref() else {
        return;
    };
    if check_state(w, s).is_err() || w.over {
        return;
    }
    let mut candidates = Vec::new();
    let mut scheduled = BTreeSet::new();
    for observer in w.entities.values().filter(|e| e.alive) {
        let Some(unit) = s.config.units.get(&observer.name) else {
            continue;
        };
        for (fit_key, tx) in &unit.fits {
            if tx.warning_kind.is_voice()
                || !tx.send
                || !s
                    .automatic
                    .get(&format!("{}:{fit_key}", observer.id.0))
                    .copied()
                    .unwrap_or(tx.automatic_warning)
            {
                continue;
            }
            let mut reports: Vec<_> = s
                .reports
                .values()
                .filter(|r| {
                    r.report.observer == observer.id
                        && if include_current_batch {
                            r.report.at <= w.tick
                        } else {
                            r.report.at < w.tick
                        }
                        && r.report.at.div_euclid(180) == w.tick.div_euclid(180)
                        && report_live(w, s, r)
                })
                .collect();
            reports.sort_by_key(|r| {
                (
                    r.report.target,
                    std::cmp::Reverse(r.apparent_category == Some(PlatformKind::Sub)),
                    std::cmp::Reverse(r.report.at),
                    r.report.sensor.clone(),
                )
            });
            for report in reports {
                let Some(contact) = w.sides[observer.side.index()]
                    .contacts
                    .values()
                    .find(|c| !c.stale && c.about == report.report.target)
                else {
                    continue;
                };
                for receiver in w
                    .entities
                    .values()
                    .filter(|e| e.side == observer.side && e.alive && e.id != observer.id)
                {
                    if edges(
                        w,
                        &s.config,
                        observer.side,
                        observer.id,
                        fit_key,
                        &[receiver.id],
                    )
                    .is_err()
                    {
                        continue;
                    }
                    let window = if tx.warning_kind == LinkKind::TimeLate {
                        0
                    } else {
                        w.tick.div_euclid(180)
                    };
                    let key = format!(
                        "{}:{fit_key}:{}:{}:{}:{window}",
                        observer.id.0,
                        receiver.id.0,
                        report.report.target.0,
                        report.apparent_category == Some(PlatformKind::Sub)
                    );
                    if !s.auto_sent.contains(&key) && scheduled.insert(key.clone()) {
                        let action = Action::SendWarning {
                            contact: contact.id,
                            sensor: report.report.sensor.clone(),
                            fit: fit_key.clone(),
                            receivers: vec![receiver.id],
                        };
                        candidates.push((observer.side, observer.id, action, key.clone()));
                    }
                    if tx.warning_kind == LinkKind::RealTime
                        && tx.targeting_system
                        && matches!(
                            report.apparent_category,
                            Some(PlatformKind::Aircraft | PlatformKind::Missile)
                        )
                    {
                        let air_key =
                            format!("{key}:air:{}", w.tick.div_euclid(air_increment_seconds()));
                        if !s.auto_sent.contains(&air_key) && !scheduled.contains(&air_key) {
                            let action = Action::SendAirSolution {
                                contact: contact.id,
                                sensor: report.report.sensor.clone(),
                                fit: fit_key.clone(),
                                receivers: vec![receiver.id],
                            };
                            if super::prepare(w, observer.side, observer.id, &action).is_ok() {
                                scheduled.insert(air_key.clone());
                                candidates.push((observer.side, observer.id, action, air_key));
                            }
                        }
                        continue;
                    }
                    if matches!(tx.warning_kind, LinkKind::NearRealTime | LinkKind::RealTime)
                        && has_real_time_targeting(w, observer.id)
                    {
                        if let Some(weapon) = &tx.automatic_fcs_weapon {
                            let fcs_key = format!("{key}:fcs");
                            if !s.auto_sent.contains(&fcs_key) && !scheduled.contains(&fcs_key) {
                                // A warning normally precedes native motion/FCS
                                // formation. Only commit a terminal send after
                                // the actual selected sensor can form its own
                                // solution; a pending attempt cannot consume
                                // this epoch's eventual FCS opportunity.
                                let receivers = BTreeMap::from([(
                                    receiver.id,
                                    match super::receiver_fit_key(w, &s.config, observer.side, observer.id, fit_key, receiver.id)
                                        .and_then(|key| fit(w, &s.config, observer.side, receiver.id, &key))
                                        .and_then(|rx| links::effective(tx, rx)) {
                                        Ok(kind) => kind,
                                        Err(_) => continue,
                                    },
                                )]);
                                if prepare_solution(
                                    w,
                                    observer.id,
                                    report,
                                    tx,
                                    &receivers,
                                    weapon,
                                    None,
                                )
                                .is_err()
                                {
                                    let pending_key = format!("{fcs_key}:pending");
                                    if !s.auto_sent.contains(&pending_key)
                                        && scheduled.insert(pending_key.clone())
                                    {
                                        candidates.push((
                                            observer.side,
                                            observer.id,
                                            Action::SendSolution {
                                                contact: contact.id,
                                                sensor: report.report.sensor.clone(),
                                                fit: fit_key.clone(),
                                                receivers: vec![receiver.id],
                                                weapon: weapon.clone(),
                                                speed_index: None,
                                            },
                                            pending_key,
                                        ));
                                    }
                                    continue;
                                }
                                scheduled.insert(fcs_key.clone());
                                candidates.push((
                                    observer.side,
                                    observer.id,
                                    Action::SendSolution {
                                        contact: contact.id,
                                        sensor: report.report.sensor.clone(),
                                        fit: fit_key.clone(),
                                        receivers: vec![receiver.id],
                                        weapon: weapon.clone(),
                                        speed_index: None,
                                    },
                                    fcs_key,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    for (side, actor, action, key) in candidates {
        match apply(w, side, actor, &action) {
            Ok(()) => {
                if let Some(s) = w.rules.communications.as_mut() {
                    s.auto_sent.insert(key);
                }
            }
            Err(reason) if matches!(action, Action::SendSolution { .. }) => {
                // One explicit pending trace per attempted automatic formation
                // window. Never replace missing real fit with a guessed FCS.
                if let Some(s) = w.rules.communications.as_mut() {
                    s.auto_sent.insert(key);
                }
                w.emit(
                    EventKind::RulesEvent {
                        domain: "communications2026".into(),
                        rule: "6.3.9.auto_fcs_pending".into(),
                        side: None,
                        unit: Some(actor),
                        contact: None,
                        detail:
                            json!({"reason":reason,"source":"native own targeting FCS required"})
                                .to_string(),
                    },
                    Visibility::Referee,
                );
            }
            Err(_) => {}
        }
    }
}
pub(super) fn own_solutions(w: &World, unit: EntityId) -> Value {
    let Some(s) = w.rules.communications.as_ref() else {
        return json!([]);
    };
    let mut out = Vec::new();
    for m in s
        .messages
        .values()
        .filter(|m| m.status == Status::Delivered && m.receivers.contains(&unit))
    {
        if let Some(q) = &m.solution {
            out.push(json!({"contact":m.contact,"contact_is_source_label":true,"formed_at":q.formed_at,"source_observer":q.source_observer,"source_sensor":q.source_sensor,"quality_at_source":q.quality,"link_kind":m.receiver_kinds.get(&unit),"message_ref":MessageRef{sender:m.sender,id:m.id},"report_is_live_track":false}));
        }
    }
    for c in s.exchange.compositions.values().filter(|c| c.actor == unit) {
        out.push(json!({"contact":c.contact,"quality":c.evidence.quality,"formed_at":c.evidence.formed_at,"source":"nonTDL_multi_source","refs":c.evidence.refs}));
    }
    for m in s
        .messages
        .values()
        .filter(|m| m.status == Status::Delivered && m.receivers.contains(&unit))
    {
        if let Some(evidence) = &m.air_solution {
            out.push(json!({"contact":m.contact,"contact_is_source_label":true,"domain":"air_native_geometry","formed_at":evidence.formed_at,"source_observer":evidence.source_observer,"source_sensor":evidence.source_sensor,"observed_at":evidence.observed_at,"height_known":evidence.altitude_m.is_some(),"message_ref":MessageRef{sender:m.sender,id:m.id},"report_is_live_track":false}));
        }
    }
    json!(out)
}

//! RT air fire control is native targeting geometry/readiness. It is not the
//! Good/Fair/Poor/NA placement quality used for ASCMs and torpedoes.
use super::*;
use crate::db::SensorKind;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AirFcsEvidence {
    pub formed_at: i64,
    pub observed_at: i64,
    pub source_observer: EntityId,
    pub source_sensor: String,
    pub position: crate::Vec2,
    pub altitude_m: Option<f64>,
    pub category: PlatformKind,
    pub refs: Vec<MessageRef>,
}

pub(super) fn native_altitude(
    w: &World,
    r: &SensorReport,
    category: Option<PlatformKind>,
) -> Option<f64> {
    if r.sensor_kind != SensorKind::Radar
        || r.measured_position.is_none()
        || !matches!(
            category,
            Some(PlatformKind::Aircraft | PlatformKind::Missile)
        )
    {
        return None;
    }
    let observer = w.entity(r.observer)?;
    let sensor = w.db.sensors.get(&r.sensor)?;
    let role = crate::rules::aviation2026::operations_radar_profile(w, observer, sensor)?["role"]
        .as_str()?;
    if !matches!(role, "3D" | "HF" | "AI" | "LD/SD") {
        return None;
    }
    w.entity(r.target)
        .filter(|t| t.altitude_m.is_finite())
        .map(|t| t.altitude_m)
}

pub(super) fn prepare(
    w: &World,
    actor: EntityId,
    r: &NativeReport,
    tx: &VoiceFit,
    receivers: &BTreeMap<EntityId, LinkKind>,
) -> Result<AirFcsEvidence, String> {
    if tx.warning_kind != LinkKind::RealTime
        || !tx.targeting_system
        || receivers.is_empty()
        || receivers.values().any(|kind| *kind != LinkKind::RealTime)
    {
        return Err("6.3.9 air targeting geometry requires a compatible RT targeting sender and RT receivers; NRT is surface/ASW only".into());
    }
    let s = w.rules.communications.as_ref().ok_or_else(source_error)?;
    if r.report.sensor_kind != SensorKind::Radar
        || !exchange::report_live(w, s, r)
        || !r.report.bearing_alternatives.is_empty()
        || !crate::rules::operations::native_air_target_ready(
            w,
            actor,
            r.report.target,
            &r.report.sensor,
        )
    {
        return Err("6.3.9 selected actual native radar has not formed air targeting data".into());
    }
    let category = r
        .apparent_category
        .filter(|kind| matches!(kind, PlatformKind::Aircraft | PlatformKind::Missile))
        .ok_or("6.3.9 air category is not established by the selected native source")?;
    Ok(AirFcsEvidence {
        formed_at: w.tick,
        observed_at: r.report.at,
        source_observer: actor,
        source_sensor: r.report.sensor.clone(),
        position: r
            .report
            .measured_position
            .ok_or("6.3.9 measured air position required")?,
        altitude_m: r.air_altitude_m,
        category,
        refs: Vec::new(),
    })
}

fn eligible(
    w: &World,
    s: &State,
    m: &Message,
    receiver: EntityId,
    target: EntityId,
    require_increment: bool,
) -> bool {
    m.status == Status::Delivered
        && m.receivers.contains(&receiver)
        && m.snapshot.report.target == target
        && m.air_solution.is_some()
        && m.receiver_kinds.get(&receiver) == Some(&LinkKind::RealTime)
        && (!require_increment
            || m.terminal_at.is_some_and(|at| {
                at.div_euclid(air_increment_seconds()) == w.tick.div_euclid(air_increment_seconds())
            }))
        && exchange::report_live(w, s, &m.snapshot)
        && edges(w, &s.config, m.side, m.sender, &m.fit, &[receiver]).is_ok()
}
pub fn received_air_solution(
    w: &World,
    receiver: EntityId,
    target: EntityId,
) -> Result<Option<AirFcsEvidence>, String> {
    if !active(w) {
        return Ok(None);
    }
    let Some(s) = w.rules.communications.as_ref() else {
        return Ok(None);
    };
    check_state(w, s)?;
    let selected = s
        .messages
        .values()
        .filter(|m| eligible(w, s, m, receiver, target, true))
        .max_by_key(|m| (m.air_solution.as_ref().unwrap().formed_at, m.sender, m.id));
    Ok(selected.map(|m| {
        let mut evidence = m.air_solution.clone().unwrap();
        evidence.refs = vec![MessageRef {
            sender: m.sender,
            id: m.id,
        }];
        evidence
    }))
}
/// Continuity of an actually fitted source for the receiving reaction clock.
/// Packet age alone never rerolls an already running clock at an Increment
/// boundary; a real failed native scan/route interruption does break it.
pub(crate) fn source_still_live(w: &World, receiver: EntityId, target: EntityId) -> bool {
    if !active(w) {
        return false;
    }
    let Some(s) = w.rules.communications.as_ref() else {
        return false;
    };
    check_state(w, s).is_ok()
        && s.messages
            .values()
            .any(|m| eligible(w, s, m, receiver, target, false))
}

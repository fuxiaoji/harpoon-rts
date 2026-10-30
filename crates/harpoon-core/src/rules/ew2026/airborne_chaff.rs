//! §12.2 finite aircraft-laid chaff geometry. Not §12.3 false echoes, burst
//! ammunition, noise jamming, a combat Entity or a second native sensor scan.
use crate::types::PlatformKind;
use crate::{EntityId, EventKind, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/airborne-chaff-2026.json"
        ))
        .expect("verified12.2 physical corridor source data")
    })
}
fn number(key: &str) -> f64 {
    tables()[key].as_f64().expect("12.2 source number")
}
fn seconds(key: &str) -> i64 {
    tables()[key].as_i64().expect("12.2 source seconds")
}
const EPS: f64 = 1e-9;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    #[serde(default)]
    pub units: BTreeMap<String, Fit>,
    pub wind_update_policy: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    pub source: String,
    pub fit: String,
    pub installed: bool,
    pub capacity_nm: f64,
    pub capacity_source: String,
    pub lifetime_s: i64,
    pub lifetime_source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wind {
    pub source: String,
    pub from_deg: f64,
    pub speed_kts: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    pub unit: EntityId,
    pub class: String,
    pub name: String,
    pub side: Side,
    pub fit: Fit,
    pub remaining_nm: f64,
    pub laid_nm: f64,
    pub laying: bool,
    pub sequence: u64,
    pub last_position: Vec2,
    pub last_altitude_m: f64,
    pub last_sample_at: i64,
    pub start_at: Option<i64>,
    pub pending: Option<String>,
    pub unplaced_path: bool,
    pub unplaced_until: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Segment {
    pub id: String,
    pub owner: EntityId,
    pub side: Side,
    pub source: String,
    pub original_from: Vec2,
    pub original_to: Vec2,
    pub from: Vec2,
    pub to: Vec2,
    pub laid_at: i64,
    pub initial_band: usize,
    pub band: usize,
    pub expires_at: i64,
    pub expired: bool,
    pub drift_quanta: i64,
    pub pending: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub config: Configuration,
    pub units: BTreeMap<EntityId, Unit>,
    pub segments: BTreeMap<String, Segment>,
    pub wind: Option<Wind>,
    pub pending_weather: Option<String>,
    pub last_step: Option<i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub exposure: BTreeMap<EntityId, Exposure>,
}
/// Referee-only lower bounds from actual normal movement; never a private
/// cloud proximity hint in a side view or a fabricated engine casualty.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Exposure {
    pub class: String,
    pub name: String,
    pub side: Side,
    pub last_at: i64,
    pub last_position: Vec2,
    pub last_altitude_m: f64,
    pub confirmed_total_s: i64,
    pub confirmed_continuous_s: i64,
    pub partial_boundary_seen: bool,
    pub pending: Option<String>,
}
fn runtime(w: &World) -> Option<&State> {
    w.rules.ew.as_ref()?.airborne_chaff.as_ref()
}
fn take(w: &mut World) -> Option<State> {
    w.rules.ew.as_mut()?.airborne_chaff.take()
}
fn put(w: &mut World, state: State) {
    if let Some(parent) = w.rules.ew.as_mut() {
        parent.airborne_chaff = Some(state);
    }
}
fn finite_point(p: Vec2) -> bool {
    p.x.is_finite() && p.y.is_finite()
}
fn wind(w: &World) -> Result<Wind, String> {
    let e = w
        .rules
        .ew
        .as_ref()
        .and_then(|s| s.config.environment.as_ref())
        .ok_or("12.2 source constant wind absent")?;
    let supported = tables()["execution_policy"]["maximum_supported_constant_wind_kts"]
        .as_f64()
        .unwrap();
    if e.source.trim().is_empty()
        || !e.wind_from_deg.is_finite()
        || !e.wind_speed_kts.is_finite()
        || !(0.0..=supported).contains(&e.wind_speed_kts)
    {
        return Err("12.2 constant-wind first scope requires explicit finite0..6kt; high/unknown wind movement cadence source pending (6kt is a capability limit, not a printed threshold)".into());
    }
    Ok(Wind {
        source: e.source.clone(),
        from_deg: crate::geom::normalize_deg(e.wind_from_deg),
        speed_kts: e.wind_speed_kts,
    })
}
pub fn initialize(w: &World, config: Option<&Configuration>) -> Result<Option<State>, String> {
    let Some(c) = config else { return Ok(None) };
    if !super::active(w)
        || c.source.trim().is_empty()
        || c.wind_update_policy != "source_600s_movement"
    {
        return Err("12.2 Full/source/source_600s_movement qualification required".into());
    }
    let mut s = State {
        config: c.clone(),
        wind: Some(wind(w)?),
        ..Default::default()
    };
    for (name, f) in &c.units {
        let bodies: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
        if bodies.len() != 1 {
            return Err("12.2 exact named aircraft dispenser must bind uniquely".into());
        }
        let e = bodies[0];
        if e.kind != PlatformKind::Aircraft
            || !e.alive
            || !finite_point(e.pos)
            || !e.altitude_m.is_finite()
            || e.altitude_m < 0.
            || !f.source.starts_with("scenario_assumption:")
            || f.fit.trim().is_empty()
            || !f.capacity_source.starts_with("scenario_assumption:")
            || !f.lifetime_source.starts_with("scenario_assumption:")
            || !f.capacity_nm.is_finite()
            || f.capacity_nm <= 0.
            || f.lifetime_s <= 0
            || f.lifetime_s > seconds("maximum_lifetime_s")
        {
            return Err("12.2 actual aircraft and explicit author installed fit/finite nm/exact material life<=1800s required; no real Annex or burst-capacity inference".into());
        }
        s.units.insert(
            e.id,
            Unit {
                unit: e.id,
                class: e.class.clone(),
                name: e.name.clone(),
                side: e.side,
                fit: f.clone(),
                remaining_nm: f.capacity_nm,
                laid_nm: 0.,
                laying: false,
                sequence: 0,
                last_position: e.pos,
                last_altitude_m: e.altitude_m,
                last_sample_at: w.tick,
                start_at: None,
                pending: None,
                unplaced_path: false,
                unplaced_until: None,
            },
        );
    }
    Ok(Some(s))
}
fn available(w: &World, u: &Unit) -> Result<(), String> {
    let e = w
        .entity(u.unit)
        .ok_or("12.2 original dispenser aircraft absent")?;
    if !e.alive
        || e.kind != PlatformKind::Aircraft
        || e.class != u.class
        || e.name != u.name
        || e.side != u.side
        || !u.fit.installed
        || !finite_point(e.pos)
        || !e.altitude_m.is_finite()
        || e.altitude_m < 0.
    {
        return Err("12.2 original live installed aircraft binding changed/unavailable".into());
    }
    crate::rules::unmanned2026::can_control(w, u.side, u.unit)?;
    if !crate::rules::expanded_damage2026::equipment_operational(
        w,
        u.unit,
        &format!("native.ew:airborne_chaff:{}", u.fit.fit),
    )? {
        return Err("12.2 actual authored dispenser component unavailable".into());
    }
    Ok(())
}
pub fn validate_start_stop(w: &World, unit: EntityId, fit: Option<&str>) -> Result<(), String> {
    if !super::active(w) {
        return Err("12.2 explicit Full physical corridor profile required".into());
    }
    let s = runtime(w).ok_or("12.2 actual aircraft corridor configuration absent")?;
    let u = s
        .units
        .get(&unit)
        .ok_or("12.2 actual fitted own aircraft dispenser absent")?;
    available(w, u)?;
    if let Some(fit) = fit {
        if fit != u.fit.fit {
            return Err("12.2 exact installed corridor fit required".into());
        }
        if s.pending_weather.is_some() || u.pending.is_some() || u.unplaced_path {
            return Err(
                "12.2 previous unresolved physical corridor work requires source adjudication"
                    .into(),
            );
        }
        if u.remaining_nm <= EPS {
            return Err("12.2 finite aircraft corridor nm capacity exhausted".into());
        }
        wind(w)?;
        let band = crate::rules::operations::altitude_band(w.entity(unit).unwrap().altitude_m);
        if band < tables()["low_band"].as_u64().unwrap() as usize {
            return Err("12.2 initial below-Low deposition band source policy pending".into());
        }
    }
    Ok(())
}
fn event(w: &mut World, unit: EntityId, side: Side, rule: &str, value: Value, referee: bool) {
    w.emit(
        EventKind::RulesEvent {
            domain: "ew2026".into(),
            rule: rule.into(),
            side: Some(side),
            unit: Some(unit),
            contact: None,
            detail: value.to_string(),
        },
        if referee {
            Visibility::Referee
        } else {
            Visibility::Side(side)
        },
    );
}
pub fn apply(w: &mut World, unit: EntityId, fit: Option<&str>) -> Result<(), String> {
    validate_start_stop(w, unit, fit)?;
    let e = w.entity(unit).unwrap();
    let (position, altitude, now) = (e.pos, e.altitude_m, w.tick);
    let u = w
        .rules
        .ew
        .as_mut()
        .and_then(|s| s.airborne_chaff.as_mut())
        .unwrap()
        .units
        .get_mut(&unit)
        .unwrap();
    let start = fit.is_some();
    if u.laying == start {
        return Ok(());
    }
    u.laying = start;
    u.last_position = position;
    u.last_altitude_m = altitude;
    u.last_sample_at = now;
    u.start_at = start.then_some(now);
    let (side, value) = (
        u.side,
        json!({"fit":u.fit.fit,"laying":u.laying,"remaining_nm":u.remaining_nm,"capacity_unit":"nautical_miles_of_actual_flight"}),
    );
    event(
        w,
        unit,
        side,
        if start {
            "12.2.airborne_chaff_started"
        } else {
            "12.2.airborne_chaff_stopped"
        },
        value,
        false,
    );
    Ok(())
}
/// Parent EW State stays in World. Only this child is taken; wind/health and
/// other source consumers always see their authoritative normal parent state.
pub fn step(w: &mut World) {
    if !super::active(w) {
        return;
    }
    let Some(mut s) = take(w) else { return };
    if s.last_step == Some(w.tick) {
        put(w, s);
        return;
    }
    let now = w.tick;
    let current_wind = wind(w);
    if s.pending_weather.is_none() {
        let changed = match (&s.wind, &current_wind) {
            (Some(a), Ok(b)) => a.from_deg != b.from_deg || a.speed_kts != b.speed_kts,
            _ => true,
        };
        if changed {
            s.pending_weather=Some("12.2 changed/unknown past wind interval source pending; no retrospective current-wind resampling".into());
        }
    }
    let mut notices = Vec::new();
    for u in s.units.values_mut() {
        if !u.laying {
            continue;
        }
        let health = available(w, u);
        let reason = health.err().or_else(|| s.pending_weather.clone());
        if let Some(reason) = reason {
            u.laying = false;
            u.pending = Some(reason.clone());
            if s.pending_weather.is_some() {
                u.unplaced_path = true;
                u.unplaced_until = Some(now.saturating_add(u.fit.lifetime_s));
            }
            notices.push((
                u.unit,
                u.side,
                "12.2.airborne_chaff_pending",
                json!({"reason":reason,"remaining_nm":u.remaining_nm}),
                false,
            ));
            continue;
        }
        let e = w.entity(u.unit).unwrap();
        let expected = u.start_at == Some(now) || u.last_sample_at.checked_add(1) == Some(now);
        if !expected {
            u.laying = false;
            u.unplaced_path = true;
            u.unplaced_until = Some(now.saturating_add(u.fit.lifetime_s));
            u.pending=Some("12.2 unobserved movement interval source pending; no straight-line replay of an unknown flight path".into());
            notices.push((
                u.unit,
                u.side,
                "12.2.airborne_chaff_pending",
                json!({"reason":u.pending,"remaining_nm":u.remaining_nm}),
                false,
            ));
            continue;
        }
        let distance = u.last_position.range_to(e.pos);
        if !distance.is_finite() {
            u.laying = false;
            u.unplaced_path = true;
            u.unplaced_until = Some(now.saturating_add(u.fit.lifetime_s));
            u.pending = Some("12.2 nonfinite physical flight displacement pending".into());
            continue;
        }
        if distance > 0. && u.remaining_nm > EPS {
            let paid = distance.min(u.remaining_nm);
            let ratio = paid / distance;
            let endpoint = Vec2::new(
                u.last_position.x + (e.pos.x - u.last_position.x) * ratio,
                u.last_position.y + (e.pos.y - u.last_position.y) * ratio,
            );
            let initial_band = crate::rules::operations::altitude_band(u.last_altitude_m);
            let end_band = crate::rules::operations::altitude_band(e.altitude_m);
            let pending = (initial_band != end_band).then(|| {
                "12.2 mixed-altitude actual deposition vertical trajectory source pending"
                    .to_string()
            });
            u.sequence += 1;
            let id = format!("air_chaff:{}:{}", u.unit.0, u.sequence);
            let seg = Segment {
                id: id.clone(),
                owner: u.unit,
                side: u.side,
                source: u.fit.source.clone(),
                original_from: u.last_position,
                original_to: endpoint,
                from: u.last_position,
                to: endpoint,
                laid_at: now,
                initial_band,
                band: initial_band,
                expires_at: now.saturating_add(u.fit.lifetime_s),
                expired: false,
                drift_quanta: 0,
                pending: pending.clone(),
            };
            u.remaining_nm = (u.remaining_nm - paid).max(0.);
            if u.remaining_nm < EPS {
                u.remaining_nm = 0.;
            }
            u.laid_nm = u.fit.capacity_nm - u.remaining_nm;
            if u.remaining_nm == 0. {
                u.laying = false;
            }
            if let Some(reason) = pending {
                u.laying = false;
                u.pending = Some(reason);
            }
            notices.push((u.unit,u.side,"12.2.airborne_chaff_laid",json!({"segment":id,"paid_nm":paid,"remaining_nm":u.remaining_nm,"laid_nm":u.laid_nm,"laying":u.laying}),false));
            notices.push((
                u.unit,
                u.side,
                "12.2.airborne_chaff_geometry",
                json!({"segment":seg}),
                true,
            ));
            s.segments.insert(id, seg);
        }
        u.last_position = e.pos;
        u.last_altitude_m = e.altitude_m;
        u.last_sample_at = now;
        u.start_at = None;
    }
    for seg in s.segments.values_mut() {
        if seg.expired {
            continue;
        }
        let before = (seg.band, seg.drift_quanta, seg.pending.clone());
        if now >= seg.expires_at {
            seg.expired = true;
        } else {
            let age = now.saturating_sub(seg.laid_at).max(0);
            let falls = (age / seconds("fall_interval_s")) as usize;
            let low = tables()["low_band"].as_u64().unwrap() as usize;
            if falls > 0 {
                match seg.initial_band.checked_sub(falls) {
                    Some(band) if band >= low => seg.band = band,
                    _ => {
                        if seg.pending.is_none() {
                            seg.pending = Some(
                                "12.2 surviving material descent below Low source policy pending"
                                    .into(),
                            );
                        }
                    }
                }
            }
            if let Some(wind) = &s.wind {
                seg.drift_quanta = age / seconds("wind_update_s");
                let distance =
                    seg.drift_quanta as f64 * seconds("wind_update_s") as f64 * wind.speed_kts
                        / 3600.
                        * number("wind_speed_multiplier");
                seg.from = seg.original_from.advanced(wind.from_deg + 180., distance);
                seg.to = seg.original_to.advanced(wind.from_deg + 180., distance);
            }
        }
        if seg.expired || before != (seg.band, seg.drift_quanta, seg.pending.clone()) {
            notices.push((
                seg.owner,
                seg.side,
                "12.2.airborne_chaff_lifecycle",
                json!({"segment":seg}),
                true,
            ));
        }
    }
    sample_exposure(w, &mut s, &mut notices);
    s.last_step = Some(now);
    put(w, s);
    for (unit, side, rule, value, referee) in notices {
        event(w, unit, side, rule, value, referee);
    }
}
fn occupies(band: usize, altitude: f64) -> bool {
    let actual = crate::rules::operations::altitude_band(altitude);
    if band == tables()["low_band"].as_u64().unwrap() as usize {
        tables()["low_covers_bands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_u64() == Some(actual as u64))
    } else {
        band == actual
    }
}
fn sample_exposure(
    w: &World,
    s: &mut State,
    notices: &mut Vec<(EntityId, Side, &'static str, Value, bool)>,
) {
    let now = w.tick;
    for e in w
        .entities
        .values()
        .filter(|e| e.alive && e.kind == PlatformKind::Aircraft)
    {
        let previous = s
            .exposure
            .get(&e.id)
            .filter(|p| p.class == e.class && p.name == e.name && p.side == e.side)
            .cloned();
        let mut confirmed = false;
        let mut inside = false;
        let mut unknown = false;
        for seg in s
            .segments
            .values()
            .filter(|v| !v.expired && v.laid_at <= now && now < v.expires_at)
        {
            let horizontal =
                point_distance(e.pos, seg.from, seg.to) <= number("width_nm") / 2. + 1e-12;
            if !horizontal {
                continue;
            }
            if seg.pending.is_some() || s.pending_weather.is_some() {
                unknown = true;
                continue;
            }
            if !occupies(seg.band, e.altitude_m) {
                continue;
            }
            inside = true;
            if let Some(p) = &previous {
                if p.last_at.checked_add(1) == Some(now)
                    && seg.laid_at <= p.last_at
                    && p.last_altitude_m.is_finite()
                    && occupies(seg.band, p.last_altitude_m)
                    && (p.last_at - seg.laid_at) / seconds("fall_interval_s")
                        == (now - seg.laid_at) / seconds("fall_interval_s")
                    && (p.last_at - seg.laid_at) / seconds("wind_update_s")
                        == (now - seg.laid_at) / seconds("wind_update_s")
                    && point_distance(p.last_position, seg.from, seg.to)
                        <= number("width_nm") / 2. + 1e-12
                {
                    // Normal movement is a straight one-second segment and the
                    // unchanged capsule is convex; both endpoints prove this
                    // complete interval inside the same actual cloud volume.
                    confirmed = true;
                }
            }
        }
        let mut p = previous.unwrap_or(Exposure {
            class: e.class.clone(),
            name: e.name.clone(),
            side: e.side,
            last_at: now,
            last_position: e.pos,
            last_altitude_m: e.altitude_m,
            confirmed_total_s: 0,
            confirmed_continuous_s: 0,
            partial_boundary_seen: false,
            pending: None,
        });
        if confirmed {
            p.confirmed_total_s += 1;
            p.confirmed_continuous_s += 1;
        } else {
            p.confirmed_continuous_s = 0;
        }
        p.partial_boundary_seen |= (inside && !confirmed) || unknown;
        let threshold = tables()["engine_risk"]["safe_tactical_turns"]
            .as_i64()
            .unwrap()
            * tables()["engine_risk"]["existing_rts_tactical_turn_s"]
                .as_i64()
                .unwrap();
        let reason = if unknown {
            Some("12.2 actual aircraft cloud exposure has unqualified band/wind geometry; engine-risk duration source pending")
        } else if p.confirmed_total_s > threshold {
            Some("12.2 actual confirmed cloud exposure exceeds two TacticalTurns; engine10percent repeat cadence and separated-visit reset policy source pending; no laying-aircraft exemption")
        } else {
            None
        };
        if p.pending.is_none() {
            if let Some(reason) = reason {
                p.pending = Some(reason.into());
                notices.push((e.id,e.side,"12.2.airborne_chaff_engine_risk_pending",json!({"reason":reason,"confirmed_total_s":p.confirmed_total_s,"confirmed_continuous_s":p.confirmed_continuous_s,"partial_boundary_seen":p.partial_boundary_seen,"engine_damage_resolved":false,"d100":Value::Null}),true));
            }
        }
        p.last_at = now;
        p.last_position = e.pos;
        p.last_altitude_m = e.altitude_m;
        s.exposure.insert(e.id, p);
    }
}
fn cross(a: Vec2, b: Vec2) -> f64 {
    a.x * b.y - a.y * b.x
}
fn point_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let denom = dx * dx + dy * dy;
    if denom <= 0. {
        return p.range_to(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / denom).clamp(0., 1.);
    p.range_to(Vec2::new(a.x + t * dx, a.y + t * dy))
}
fn ray_distance(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> f64 {
    let r = Vec2::new(b.x - a.x, b.y - a.y);
    let s = Vec2::new(d.x - c.x, d.y - c.y);
    let gap = Vec2::new(c.x - a.x, c.y - a.y);
    let denom = cross(r, s);
    if denom.abs() > 1e-14 {
        let t = cross(gap, s) / denom;
        let u = cross(gap, r) / denom;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            return 0.;
        }
    }
    point_distance(a, c, d)
        .min(point_distance(b, c, d))
        .min(point_distance(c, a, b))
        .min(point_distance(d, a, b))
}
/// true means physically occluded. A specific pending ray never becomes a
/// guessed clear line. No ES/Visual/IR/false-echo path calls this radar helper.
pub fn radar_los(w: &World, o: EntityId, t: EntityId, oh: f64, th: f64) -> Result<bool, String> {
    let Some(s) = runtime(w) else {
        return Ok(false);
    };
    let active: Vec<_> = s
        .segments
        .values()
        .filter(|v| !v.expired && v.laid_at <= w.tick && w.tick < v.expires_at)
        .collect();
    let unplaced = s
        .units
        .values()
        .any(|u| u.unplaced_path && u.unplaced_until.is_none_or(|at| w.tick < at));
    if active.is_empty() && !unplaced {
        return Ok(false);
    }
    if s.pending_weather.is_some() || unplaced {
        return Err("12.2 physical corridor past wind/path source pending; cannot prove this radar ray clear".into());
    }
    let a = w.entity(o).ok_or("12.2 radar observer absent")?;
    let b = w.entity(t).ok_or("12.2 radar target absent")?;
    if !finite_point(a.pos)
        || !finite_point(b.pos)
        || !oh.is_finite()
        || !th.is_finite()
        || oh < 0.
        || th < 0.
    {
        return Err("12.2 source radar endpoint height/position unavailable".into());
    }
    let ob = crate::rules::operations::altitude_band(oh);
    let tb = crate::rules::operations::altitude_band(th);
    for seg in active {
        let distance = ray_distance(a.pos, b.pos, seg.from, seg.to);
        if !distance.is_finite() {
            return Err("12.2 nonfinite radar/corridor geometry source pending".into());
        }
        if distance > number("width_nm") / 2. + 1e-12 {
            continue;
        }
        if seg.pending.is_some() {
            return Err("12.2 intersecting corridor vertical/source policy pending".into());
        }
        if ob != tb {
            return Err(
                "12.2 mixed-altitude intersecting radar ray source geometry pending".into(),
            );
        }
        let occupied = if seg.band == tables()["low_band"].as_u64().unwrap() as usize {
            tables()["low_covers_bands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_u64() == Some(ob as u64))
        } else {
            seg.band == ob
        };
        if occupied {
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn own_view(w: &World, side: Side, unit: EntityId) -> Value {
    let Some(s) = runtime(w) else {
        return Value::Null;
    };
    let Some(u) = s.units.get(&unit) else {
        return Value::Null;
    };
    let Some(e) = w.entity(unit) else {
        return Value::Null;
    };
    if e.side != side || u.side != side || e.class != u.class || e.name != u.name {
        return Value::Null;
    }
    json!({"fit":u.fit.fit,"remaining_nm":u.remaining_nm,"laid_nm":u.laid_nm,"laying":u.laying,"capacity_unit":"nautical_miles_of_actual_flight","pending":u.pending,"source_scope":"explicit author corridor installation; no real Annex capacity or complete12.2 risk claim"})
}

//! Explicit HV2026 v3 operations. Missing annex facts are errors, never defaults.
//! Legacy/v2 worlds never enter these paths. Tables: data/rules/operations-v3.json.
use crate::{
    air,
    commands::{AirMission, LaunchProfile},
    db::{SensorDef, SensorKind, WeaponDef, WeaponKind},
    geom::{angle_diff, horizon_nm},
    types::PlatformKind,
    world::Entity,
    EntityId, EventKind, Rng, Visibility, World,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/operations-v3.json"))
            .expect("operations tables")
    })
}
fn number(v: &Value) -> f64 {
    v.as_f64().expect("verified numeric cell")
}
fn seconds(key: &str) -> i64 {
    tables()["aviation"][key].as_i64().unwrap()
}
pub fn fingerprint(damage: &str) -> String {
    let ew: Value = serde_json::from_str(include_str!(
        "../../../../data/rules/electronic-warfare.json"
    ))
    .unwrap();
    let flight: Value =
        serde_json::from_str(include_str!("../../../../data/rules/flight-profiles.json")).unwrap();
    let caps: Value = serde_json::from_str(include_str!(
        "../../../../data/rules/deck-capabilities.json"
    ))
    .unwrap();
    let bytes = serde_json::to_vec(&json!([damage, tables(), ew, flight, caps])).unwrap();
    let h = bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x100000001b3)
    });
    format!("fnv64:{h:016x}")
}
pub fn active(w: &World) -> bool {
    matches!(
        w.rules.profile,
        super::damage_runtime::RuleProfile::Rts2026Operations
            | super::damage_runtime::RuleProfile::Rts2026Full
    )
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlightEnvironment {
    pub wind_from_deg: f64,
    pub wind_kts: f64,
}
impl FlightEnvironment {
    pub fn valid(&self) -> bool {
        self.wind_from_deg.is_finite()
            && (0.0..360.0).contains(&self.wind_from_deg)
            && self.wind_kts.is_finite()
            && self.wind_kts >= 0.0
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LandClutterRegion {
    pub center: crate::Vec2,
    pub radius_nm: f64,
    pub terrain: usize,
}
impl LandClutterRegion {
    pub fn valid(&self) -> bool {
        self.center.x.is_finite()
            && self.center.y.is_finite()
            && self.radius_nm.is_finite()
            && self.radius_nm >= 0.0
            && self.terrain < 5
    }
    pub fn affects(&self, observer: crate::Vec2, bearing: f64, range: f64) -> bool {
        let distance = observer.range_to(self.center);
        let delta = (angle_diff(bearing, observer.bearing_to(self.center)).abs() - 5.0)
            .max(0.0)
            .to_radians();
        let along = (distance * delta.cos()).clamp(0.0, range);
        distance * distance + along * along - 2.0 * distance * along * delta.cos()
            <= self.radius_nm * self.radius_nm + 1e-9
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub launches: Vec<LaunchJob>,
    pub lanes: BTreeMap<String, i64>,
    pub service_places: BTreeMap<EntityId, Vec<i64>>,
    pub recoveries: BTreeMap<EntityId, RecoveryJob>,
    pub tracks: BTreeMap<String, RadarTrack>,
    pub diagnostics: BTreeSet<String>,
    pub motion: BTreeMap<String, MotionFix>,
    pub pad_last: BTreeMap<String, i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rt_air_tracks: BTreeMap<String, RtAirTrack>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RtAirTrack {
    pub receiver: EntityId,
    pub target: EntityId,
    pub clock: RadarTrack,
    pub continuous: bool,
    pub last_received: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchJob {
    pub carrier: EntityId,
    pub class: String,
    pub mission: AirMission,
    pub profile: Option<LaunchProfile>,
    pub mode: String,
    pub lane: String,
    pub due: i64,
    pub duration: i64,
    pub started: i64,
    #[serde(default)]
    pub manual: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryJob {
    pub carrier: EntityId,
    pub due: i64,
    pub lane: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadarTrack {
    pub seen: i64,
    pub recognized: i64,
    pub engage: i64,
    pub last_eligible: i64,
    pub roll: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MotionFix {
    pub start: i64,
    pub last: i64,
    pub first: crate::Vec2,
    pub known: bool,
}

pub fn trace(w: &mut World, id: EntityId, rule: &str, detail: Value) {
    let side = w.entity(id).map(|e| e.side);
    let visibility = if super::aaw2026::active(w) && rule == "8.1.5.fragment_roll" {
        // Referee evidence includes the incoming missile's actual speed and
        // the raw probability/roll; v3 trace visibility remains unchanged.
        Visibility::Referee
    } else {
        side.map(Visibility::Side).unwrap_or(Visibility::Referee)
    };
    w.emit(
        EventKind::RulesEvent {
            domain: "operations".into(),
            rule: rule.into(),
            side,
            unit: Some(id),
            contact: None,
            detail: detail.to_string(),
        },
        visibility,
    );
}
pub fn blocked_once(w: &mut World, id: EntityId, rule: &str, reason: &str) {
    let key = format!("{}:{rule}:{reason}", id.0);
    if w.rules
        .operations
        .as_mut()
        .is_some_and(|s| s.diagnostics.insert(key))
    {
        trace(w, id, rule, json!({"reason":reason,"status":"blocked"}));
    }
}
fn aircraft(w: &World, class: &str) -> Result<&'static Value, String> {
    super::aviation2026::operations_aircraft(w, class)
        .ok_or_else(|| format!("4.6: missing verified aircraft size/launch data: {class}"))
}
/// Unknown training types do not authorize invented strict-profile cycles.
pub fn has_aircraft_data(class: &str) -> bool {
    tables()["aviation"]["aircraft"].get(class).is_some()
}
fn deck(e: &Entity) -> Result<&'static Value, String> {
    tables()["aviation"]["decks"]
        .get(&e.class)
        .ok_or_else(|| format!("4.6: missing verified deck/runway/spot data: {}", e.class))
}
pub fn reserved(w: &World, id: EntityId, class: &str) -> usize {
    w.rules.operations.as_ref().map_or(0, |s| {
        s.launches
            .iter()
            .filter(|j| j.carrier == id && j.class == class)
            .count()
    })
}

pub fn ready_seconds(size: usize, load: &str, rotary: bool) -> Result<i64, String> {
    if size > 3 {
        return Err("unknown aircraft size".into());
    }
    let a = &tables()["aviation"];
    let arm = a["arm_minutes"]
        .get(load)
        .and_then(|v| v[size].as_i64())
        .ok_or("unavailable size/load readiness cell")?;
    let fuel = a["fuel_minutes"][size].as_i64().unwrap();
    let alert = a["alert_minutes"][size].as_i64().unwrap() / if rotary { 2 } else { 1 };
    Ok((arm.max(fuel) + alert) * 60)
}
fn load_type(w: &World, class: &str) -> Result<&'static str, String> {
    if super::aviation2026::active(w) && aircraft(w, class)?["service_load"] == "unguided" {
        return Ok("unguided");
    }
    if aircraft(w, class)?["service_load"] == "guided" {
        return Ok("guided");
    }
    let kinds: Vec<_> = w.db.platforms[class]
        .weapons
        .iter()
        .filter_map(|m| w.db.weapons.get(&m.weapon).map(|d| d.kind))
        .collect();
    if kinds
        .iter()
        .any(|k| matches!(k, WeaponKind::Ascm | WeaponKind::Slcm))
    {
        Ok("guided")
    } else if kinds.iter().any(|k| {
        matches!(
            k,
            WeaponKind::Lwt | WeaponKind::Torpedo | WeaponKind::AswRocket
        )
    }) {
        Ok("asw")
    } else if kinds.iter().any(|k| matches!(k, WeaponKind::Bomb)) {
        Err("4.6.7: bomb guidance/load readiness category not verified".into())
    } else if kinds.iter().any(|k| matches!(k, WeaponKind::Rocket)) {
        Ok("unguided")
    } else if kinds.is_empty() {
        Ok("none")
    } else {
        Ok("air")
    }
}
fn weather_check(
    w: &World,
    c: &Entity,
    a: &Value,
    catapult: bool,
    landing: bool,
) -> Result<f64, String> {
    let d = deck(c)?;
    let env = w
        .context
        .flight_environment
        .as_ref()
        .filter(|e| e.valid())
        .ok_or("4.6: explicit wind conditions required")?;
    let sea = w
        .context
        .radar_environment
        .as_ref()
        .filter(|e| e.valid())
        .ok_or("4.6: explicit sea state required")?
        .sea_state;
    if catapult {
        let headwind = if angle_diff(c.course_deg, env.wind_from_deg).abs() <= 1.0 {
            env.wind_kts
        } else {
            0.0
        };
        if c.speed_kts + headwind < 30.0 {
            return Err("4.6.4: catapult requires30kt wind over deck; turn into wind or increase ship speed".into());
        }
    }
    let bonus = tables()["aviation"]["ship_size_bonus"][d["size"].as_str().unwrap()]
        .as_u64()
        .unwrap_or(0) as u8;
    // Stabilizers/recovery systems are not inferred from displacement or class.
    let stabilizers = if c.speed_kts >= number(&tables()["aviation"]["stabilizer_min_kts"]) {
        d["stabilizers"].as_u64().unwrap_or(0) as u8
    } else {
        0
    };
    let recovery = if landing && a["rotary"] == true && d["recovery_system"] == true {
        1
    } else {
        0
    };
    let safe = seconds("ship_safe_sea_state") as u8
        + bonus
        + stabilizers
        + recovery
        + if a["size"].as_u64().unwrap() >= 2 {
            1
        } else {
            0
        };
    Ok((sea.saturating_sub(safe) as f64
        * number(&tables()["aviation"]["crash_chance_per_excess_state"]))
    .min(1.0))
}
pub fn flight_risk(
    w: &World,
    carrier: EntityId,
    class: &str,
    catapult: bool,
    landing: bool,
) -> Result<f64, String> {
    weather_check(
        w,
        w.entity(carrier).ok_or("carrier absent")?,
        aircraft(w, class)?,
        catapult,
        landing,
    )
}

/// Called after the shared command/profile/inventory validation in air.rs.
pub fn queue_launch(
    w: &mut World,
    id: EntityId,
    class: &str,
    count: u8,
    mission: &AirMission,
    profile: Option<&LaunchProfile>,
) -> Result<(), String> {
    if count == 0 {
        return Err("4.6: count must be positive".into());
    }
    let c = w.entity(id).ok_or("missing carrier")?;
    let a = aircraft(w, class)?;
    let d = deck(c)?;
    let available = c
        .deck
        .as_ref()
        .unwrap()
        .ready
        .iter()
        .filter(|(s, _)| s == class)
        .map(|(_, n)| *n as usize)
        .sum::<usize>();
    if available < reserved(w, id, class) + count as usize {
        return Err("4.6: aircraft already reserved in launch queue".into());
    }
    let rotary = a["rotary"].as_bool().unwrap();
    let cat = d["catapults"].as_u64().unwrap() as usize;
    let vtol = super::expanded_damage2026::vtol_launch_fit(w,id,class);
    let (mode, lanes, duration) = if let Some(v) = vtol {
        ("pad", usize::from(v.pads), v.duration_s)
    } else if rotary {
        // A flight deck does not establish how many independent vertical spots exist.
        let spots = d["pads"]
            .as_u64()
            .ok_or("4.6.4.3: launch/landing spot count not verified for this deck")?
            as usize;
        ("pad", spots, seconds("pad_s"))
    } else if cat > 0 {
        if a["catapult"] != true {
            return Err("4.6: this aircraft variant is not verified for catapult launch".into());
        }
        ("catapult", cat, seconds("catapult_s"))
    } else if d["flyoff"] == true && a["flyoff"] == true {
        ("flyoff", 1, seconds("flyoff_s"))
    } else {
        return Err("4.6.4.2: fixed-wing fly-off deck/aircraft capability not verified".into());
    };
    if lanes == 0 {
        return Err("4.6: no verified launch lane".into());
    }
    weather_check(w, c, a, mode == "catapult", false)?;
    load_type(w, class)?;
    let manual = c.ext.get("control.manual") == Some(&1.0);
    let now = w.tick;
    let s = w.rules.operations.as_mut().unwrap();
    let earliest = now + seconds("alert5_s");
    for _ in 0..count {
        let lane = (0..lanes)
            .map(|i| format!("{}:{mode}:{i}", id.0))
            .min_by_key(|k| (s.lanes.get(k).copied().unwrap_or(0), k.clone()))
            .unwrap();
        let mut start = s.lanes.get(&lane).copied().unwrap_or(now).max(now).max(
            s.launches
                .iter()
                .filter(|j| j.lane == lane)
                .map(|j| j.due)
                .max()
                .unwrap_or(0),
        );
        let bow = mode == "catapult"
            && lane
                .rsplit(':')
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .is_some_and(|i| i < d["bow_catapults"].as_u64().unwrap_or(0));
        if mode == "flyoff" || d["dual"] != true || !bow {
            start = start.max(
                s.lanes
                    .get(&format!("{}:recover:0", id.0))
                    .copied()
                    .unwrap_or(0),
            );
        }
        let due = (start + duration).max(earliest);
        s.lanes.insert(lane.clone(), due);
        s.launches.push(LaunchJob {
            carrier: id,
            class: class.into(),
            mission: mission.clone(),
            profile: profile.cloned(),
            mode: mode.into(),
            lane,
            due,
            duration,
            started: due - duration,
            manual,
        });
    }
    trace(
        w,
        id,
        "4.6.launch_queued",
        json!({"class":class,"count":count,"mode":mode,"earliest":earliest,"source":"English2026 PDF35-36/4-3-4-4"}),
    );
    Ok(())
}

/// RTS order cancellation releases reservations, never destroys parked aircraft.
pub fn cancel_launches(w: &mut World, carrier: EntityId) {
    if !active(w) {
        return;
    }
    let s = w.rules.operations.as_mut().unwrap();
    let before = s.launches.len();
    s.launches.retain(|j| j.carrier != carrier);
    let count = before - s.launches.len();
    if count == 0 {
        return;
    }
    for mode in ["catapult", "flyoff", "pad"] {
        let prefix = format!("{}:{mode}:", carrier.0);
        s.lanes.retain(|k, _| !k.starts_with(&prefix));
    }
    trace(
        w,
        carrier,
        "4.6.launch_cancelled",
        json!({"count":count,"reason":"recall","inventory_lost":0}),
    );
}

/// Continuous scheduling; no simulated seconds are skipped by accelerated play.
pub fn update(w: &mut World) {
    if !active(w) {
        return;
    }
    let now = w.tick;
    let gaps: Vec<_> = w
        .entities
        .values()
        .filter(|e| e.alive && !e.kind.is_projectile())
        .flat_map(|e| {
            e.sensors
                .iter()
                .filter_map(|s| w.db.sensors.get(&s.sensor))
                .filter(|s| {
                    s.kind == SensorKind::Radar
                        && super::aviation2026::operations_radar_profile(w, e, s).is_none()
                })
                .map(|s| (e.id, e.class.clone(), s.id.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    for (id, class, sensor) in gaps {
        if w.rules
            .operations
            .as_mut()
            .unwrap()
            .diagnostics
            .insert(format!("{id:?}:{sensor}"))
        {
            trace(
                w,
                id,
                "5.2.sensor_data_blocked",
                json!({"class":class,"sensor":sensor,"reason":"未核验雷达附件参数：此传感器不使用旧近似值自动裁决 / verified annex fit required"}),
            );
        }
    }
    w.rules
        .operations
        .as_mut()
        .unwrap()
        .recoveries
        .retain(|_, r| r.due > now);
    let jobs = std::mem::take(&mut w.rules.operations.as_mut().unwrap().launches);
    let mut remaining = Vec::new();
    let mut lane_ends: BTreeMap<String, i64> = BTreeMap::new();
    for mut j in jobs {
        let Some(c) = w.entity(j.carrier).filter(|c| c.alive).cloned() else {
            continue;
        };
        if let Some(previous) = lane_ends.get(&j.lane) {
            if j.started < *previous {
                j.started = *previous;
                j.due = j.started + j.duration;
            }
        }
        let d = deck(&c).unwrap();
        for recovery in w
            .rules
            .operations
            .as_ref()
            .unwrap()
            .recoveries
            .values()
            .filter(|r| r.carrier == j.carrier && r.due > now)
        {
            if j.mode == "pad" {
                continue;
            }
            let bow = j.mode == "catapult"
                && j.lane
                    .rsplit(':')
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .is_some_and(|i| i < d["bow_catapults"].as_u64().unwrap_or(0));
            if j.mode == "flyoff" || d["dual"] != true || !bow {
                j.started = j.started.max(recovery.due);
                j.due = j.started + j.duration;
            }
        }
        let usable = super::expanded_damage2026::flight_launch_operational(w, j.carrier, &j.class) && (j.class != "us_f35b" || j.mode != "pad" || super::expanded_damage2026::vtol_launch_fit(w,j.carrier,&j.class).is_some())
            && weather_check(
                w,
                &c,
                aircraft(w, &j.class).unwrap(),
                j.mode == "catapult",
                false,
            )
            .is_ok();
        if !usable {
            if j.started <= now {
                j.started = now + 1;
                j.due = j.started + j.duration;
            }
            lane_ends.insert(j.lane.clone(), j.due);
            remaining.push(j);
            continue;
        }
        lane_ends.insert(j.lane.clone(), j.due);
        if j.due > now {
            remaining.push(j);
            continue;
        }
        let before = w.next_entity;
        match air::complete_queued_launch(w, j.carrier, &j.class, &j.mission, j.profile.as_ref()) {
            Ok(()) => {
                if j.mode == "pad" {
                    w.rules
                        .operations
                        .as_mut()
                        .unwrap()
                        .pad_last
                        .insert(j.lane.clone(), now);
                }
                let a = aircraft(w, &j.class).unwrap();
                let risk = weather_check(w, &c, a, j.mode == "catapult", false).unwrap();
                let identity = super::damage_runtime::event_identity(w, j.carrier, "4.6.launch");
                let mut rng = w.rng.derive(Rng::hash_tag(now, j.carrier.0, &identity));
                let roll = 1 + (rng.next_f64() * 100.0) as u8;
                let crash = (roll as f64) <= risk * 100.0;
                let new_ids: Vec<_> = w
                    .entities
                    .values()
                    .filter(|e| e.id.0 >= before && e.kind == PlatformKind::Aircraft)
                    .map(|e| e.id)
                    .collect();
                for id in new_ids {
                    let heading = if j.mode == "catapult" {
                        c.course_deg
                    } else {
                        w.context.flight_environment.as_ref().unwrap().wind_from_deg
                    };
                    let e = w.entity_mut(id).unwrap();
                    e.altitude_m = 61.0;
                    e.speed_kts = number(&a["fmp_low_kts"]) * 0.25;
                    e.course_deg = heading;
                    if j.manual {
                        e.ext.insert("control.manual".into(), 1.0);
                    }
                    if j.profile.is_none() {
                        e.ordered_altitude_m = 1000.0;
                    }
                    if crash {
                        crash_aircraft(w, id);
                    }
                }
                trace(
                    w,
                    j.carrier,
                    "4.6.launch_complete",
                    json!({"identity":identity,"class":j.class,"lane":j.lane,"risk":risk,"d100":roll,"crash":crash,"source":"English2026 PDF36"}),
                );
            }
            Err(reason) => trace(
                w,
                j.carrier,
                "4.6.launch_cancelled",
                json!({"class":j.class,"reason":reason}),
            ),
        }
    }
    w.rules
        .operations
        .as_mut()
        .unwrap()
        .launches
        .extend(remaining);
}

pub fn crash_aircraft(w: &mut World, id: EntityId) {
    let Some(a) = w.entity(id).filter(|a| a.alive).cloned() else {
        return;
    };
    super::expanded_damage2026::record_loss(w, id, false);
    let tick = w.tick;
    let e = w.entity_mut(id).unwrap();
    e.alive = false;
    e.hp = 0.0;
    e.last_emit_tick = tick - 1000;
    w.losses[a.side.index()] += 1;
    w.losses_by_kind.entry("aircraft".into()).or_default()[a.side.index()] += 1;
    w.emit(
        EventKind::Ditched {
            side: a.side,
            unit: id,
            unit_name: a.name,
        },
        Visibility::Side(a.side),
    );
}

/// Valid touchdown reserves the lane until its next permitted recovery. The
/// aircraft need not circle for an invented minute-long touchdown animation.
pub fn recovery_ready(w: &mut World, id: EntityId, carrier: EntityId) -> Option<i64> {
    if !super::expanded_damage2026::bridge_flight_operational(w,carrier) || super::aviation2026::recovery_gate(w, carrier).is_err() {
        return None;
    }
    let now = w.tick;
    let a = w.entity(id)?.clone();
    let c = w.entity(carrier)?.clone();
    if !a.alive || !c.alive {
        return None;
    }
    let p = aircraft(w, &a.class).ok()?;
    let d = deck(&c).ok()?;
    if weather_check(w, &c, p, false, true).is_err() {
        return None;
    }
    let rotary = p["rotary"].as_bool()?;
    // Verify servicing data before reserving any landing resources.
    let load = load_type(w, &a.class).ok()?;
    let size = p["size"].as_u64()? as usize;
    let total = ready_seconds(size, load, rotary).ok()?;
    let alert =
        tables()["aviation"]["alert_minutes"][size].as_i64()? * 60 / if rotary { 2 } else { 1 };
    let mode = if rotary {
        if d["pads"].as_u64().unwrap_or(0) == 0 {
            return None;
        }
        "pad"
    } else {
        "recover"
    };
    let duration = if rotary {
        seconds("pad_s")
    } else {
        seconds("recovery_s")
    };
    let speed = number(&p["fmp_low_kts"]) * 0.25;
    let in_envelope = a.altitude_m >= 61.0
        && a.altitude_m <= 2000.0
        && (a.speed_kts - speed).abs() <= 0.5
        && angle_diff(a.course_deg, c.course_deg).abs() <= 1.0;
    if !in_envelope {
        w.rules.operations.as_mut().unwrap().recoveries.remove(&id);
        let e = w.entity_mut(id)?;
        e.ordered_altitude_m = 1000.0;
        e.ordered_speed_kts = speed;
        e.ordered_course_deg = c.course_deg;
        return None;
    }
    let lanes = if rotary {
        d["pads"].as_u64()? as usize
    } else {
        1
    };
    let lane = (0..lanes)
        .map(|i| format!("{}:{mode}:{i}", carrier.0))
        .min_by_key(|k| {
            w.rules
                .operations
                .as_ref()
                .unwrap()
                .lanes
                .get(k)
                .copied()
                .unwrap_or(0)
        })?;
    let s = w.rules.operations.as_mut().unwrap();
    if s.lanes.get(&lane).copied().unwrap_or(0) > now
        || rotary
            && s.pad_last
                .get(&lane)
                .is_some_and(|last| now - *last < duration)
    {
        return None;
    }
    let conflict = s.launches.iter().any(|j| {
        j.carrier == carrier
            && (!rotary || j.lane == lane)
            && j.started < now + duration
            && j.due > now
            && (j.mode == "flyoff"
                || d["dual"] != true
                || j.lane
                    .rsplit(':')
                    .next()
                    .and_then(|n| n.parse::<u64>().ok())
                    .is_none_or(|i| i >= d["bow_catapults"].as_u64().unwrap_or(0)))
    });
    if conflict {
        return None;
    }
    s.lanes
        .insert(lane.clone(), if rotary { now } else { now + duration });
    if rotary {
        s.pad_last.insert(lane.clone(), now);
    }
    s.recoveries.insert(
        id,
        RecoveryJob {
            carrier,
            due: now + duration,
            lane,
        },
    );
    let risk = weather_check(w, &c, p, false, true).ok()?;
    let identity = super::damage_runtime::event_identity(w, carrier, "4.6.recovery");
    let mut rng = w.rng.derive(Rng::hash_tag(now, id.0, &identity));
    let roll = 1 + (rng.next_f64() * 100.0) as u8;
    let crash = (roll as f64) <= risk * 100.0;
    if crash {
        trace(
            w,
            carrier,
            "4.6.recovery_complete",
            json!({"identity":identity,"class":a.class,"aircraft":id,"d100":roll,"risk":risk,"crash":true,"ready_at":null,"service_start":null}),
        );
        crash_aircraft(w, id);
        return None;
    }
    let places = w
        .rules
        .operations
        .as_mut()
        .unwrap()
        .service_places
        .entry(carrier)
        .or_insert_with(|| vec![0; seconds("service_slots") as usize]);
    let i = places
        .iter()
        .enumerate()
        .min_by_key(|(_, t)| **t)
        .unwrap()
        .0;
    let start = now.max(places[i]);
    places[i] = start + total - alert;
    let ready = start + total;
    trace(
        w,
        carrier,
        "4.6.recovery_complete",
        json!({"identity":identity,"class":a.class,"aircraft":id,"d100":roll,"risk":risk,"crash":crash,"ready_at":ready,"load":load,"service_start":start}),
    );
    Some(ready)
}

pub fn radar_profile(o: &Entity, def: &SensorDef) -> Option<&'static Value> {
    tables()["radar"]["bindings"]
        .get(format!("{}:{}", o.class, def.id))
        .or_else(|| tables()["radar"]["sensors"].get(&def.id))
}
pub fn altitude_band(m: f64) -> usize {
    if m <= 20.0 {
        0
    } else if m <= 60.0 {
        1
    } else if m <= 2000.0 {
        2
    } else if m <= 7500.0 {
        3
    } else if m <= 15500.0 {
        4
    } else if m <= 30000.0 {
        5
    } else {
        6
    }
}
pub fn altitude_modifier(role: &str, airborne: bool, observer_m: f64, target_m: f64) -> f64 {
    let a = altitude_band(observer_m) as i32;
    let b = altitude_band(target_m) as i32;
    match role {
        "HF" | "RO" | "GFC" | "MFC" | "TF" | "TA" | "SLAR" | "Nav" => 0.0,
        "SS" | "SS-T" => {
            if b <= 1 {
                0.1
            } else {
                0.0
            }
        }
        "LAS" => {
            if b <= 3 {
                1.0
            } else {
                0.0
            }
        }
        "AS" | "3D" if airborne => {
            if b == a || b == a + 1 {
                1.0
            } else if b == a - 1 {
                0.5
            } else {
                0.0
            }
        }
        "AI" => {
            if b == a {
                1.0
            } else if (b - a).abs() == 1 {
                0.5
            } else {
                0.0
            }
        }
        "LD/SD" => {
            if (b - a).abs() <= 2 {
                1.0
            } else {
                0.0
            }
        }
        "AS" | "3D" => 1.0,
        _ => 0.0,
    }
}
/// Largest sea/rain/land source, then resistance. Land None is explicit open sea.
pub fn clutter_modifier(
    generation: usize,
    surface_radar: bool,
    ship_as_low: bool,
    sea_state: u8,
    precip: &str,
    land: Option<usize>,
    low: bool,
) -> Result<f64, String> {
    if !(1..=6).contains(&generation) || sea_state > 9 || land.is_some_and(|x| x > 4) {
        return Err("invalid clutter input".into());
    }
    let d = &tables()["radar"];
    let mut sea = if low {
        d[if surface_radar {
            "sea_surface"
        } else {
            "sea_air"
        }][sea_state as usize]
            .as_i64()
            .unwrap()
    } else {
        0
    };
    let mut ground = land.map_or(0, |i| {
        d[if surface_radar {
            "land_surface"
        } else {
            "land_air"
        }][i]
            .as_i64()
            .unwrap()
    });
    if !low && !surface_radar {
        ground = 0;
    }
    let mut rain = d["precipitation"]
        .get(precip)
        .and_then(|x| x.as_i64())
        .ok_or("unknown precipitation class")?;
    if ship_as_low {
        sea *= 2;
        ground *= 2;
        rain *= 2;
    }
    let net =
        (sea.max(ground).max(rain) - d["resistance"][generation].as_i64().unwrap()).max(0) as usize;
    if net > 20 {
        return Err("5.2.9: net clutter exceeds the last printed range-modifier row20; no extrapolation authorized".into());
    }
    Ok(number(&d["range_modifiers"][net]))
}

/// Return Err for missing required data; a strict radar never falls back to a guessed range.
pub fn radar_range(w: &World, o: &Entity, def: &SensorDef, t: &Entity) -> Result<f64, String> {
    let p = super::aviation2026::operations_radar_profile(w, o, def)
        .ok_or("5.2: radar fit not source-verified")?;
    let env = w
        .context
        .radar_environment
        .as_ref()
        .filter(|e| e.valid())
        .ok_or("5.2.9: missing environment")?;
    if w.weather.jam.iter().any(|x| *x > 0.0) {
        return Err("5.2.9: legacy jamming scalar cannot be combined with strict clutter".into());
    }
    let air = matches!(t.kind, PlatformKind::Aircraft | PlatformKind::Missile);
    let signature = if t.kind == PlatformKind::Missile {
        let weapon = t
            .projectile
            .as_ref()
            .map(|p| p.weapon.as_str())
            .unwrap_or(&t.class);
        tables()["radar"]["missile_signatures"]
            .get(weapon)
            .and_then(|v| v.as_u64())
            .or_else(|| {
                super::surface2026::antiship::incoming_radar_signature(w, weapon).map(|n| n as u64)
            })
            .or_else(|| super::aaw2026::incoming_radar_signature(w, weapon).map(|n| n as u64))
            .ok_or("5.2.2: missile signature not verified")? as usize
    } else if t.kind == PlatformKind::Aircraft {
        let a = aircraft(w, &t.class)?;
        let mut signature = a["signature"].as_u64().unwrap() as usize;
        if a["front_half_deg"]
            .as_f64()
            .is_some_and(|half| angle_diff(t.course_deg, t.pos.bearing_to(o.pos)).abs() <= half)
        {
            signature = a["front_signature"].as_u64().unwrap() as usize;
        }
        if a["external_ordnance"] == true && t.weapons.iter().any(|m| m.ammo > 0) {
            signature = signature.min(2);
        }
        signature
    } else {
        tables()["radar"]["ship_signatures"]
            .get(&t.class)
            .and_then(|v| v.as_u64())
            .ok_or("5.2.2: missing verified radar signature; physical size is not signature")?
            as usize
    };
    let signature = if w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full {
        let fitted = if t.kind == PlatformKind::Ship {
            w.rules
                .detection
                .as_ref()
                .and_then(|s| s.config.targets.get(&t.class))
                .filter(|p| !p.source.trim().is_empty())
                .and_then(|p| p.radar_signature)
                .unwrap_or(signature)
        } else {
            signature
        };
        super::signature_damage2026::project_radar(w, t.id, fitted)?
    } else {
        signature
    };
    if !air && t.kind != PlatformKind::Ship && t.combat_kind() != PlatformKind::Ship {
        return Ok(0.0);
    }
    let role = p["role"].as_str().unwrap();
    let altitude = if air {
        altitude_modifier(
            role,
            o.kind == PlatformKind::Aircraft,
            o.altitude_m,
            t.altitude_m,
        )
    } else {
        1.0
    };
    let arc = number(&p["arc"]);
    if angle_diff(o.course_deg, o.pos.bearing_to(t.pos)).abs() > arc / 2.0 {
        return Ok(0.0);
    }
    let low = !air || t.altitude_m <= 60.0;
    // The chart contains coastline geometry, not elevations or terrain classes.
    // Do not turn an unclassified coast into an invented jungle/mountain cell.
    if w.context
        .terrain
        .as_ref()
        .is_some_and(|c| c.blocked(o.pos, t.pos))
        && o.kind == PlatformKind::Ship
        && !air
    {
        return Ok(0.0);
    }
    if w.context.terrain.is_some() && w.context.radar_land_clutter.is_none() {
        return Err(
            "5.2.9.3: coastline has no verified or explicitly authored land-clutter classification"
                .into(),
        );
    }
    let max_range = number(&p[if air { "air" } else { "surface" }][signature]);
    let land = w.context.radar_land_clutter.as_ref().and_then(|regions| {
        regions
            .iter()
            .filter(|r| r.affects(o.pos, o.pos.bearing_to(t.pos), max_range))
            .map(|r| r.terrain)
            .max()
    });
    let generation = p["generation"].as_u64().unwrap() as usize;
    let clutter = clutter_modifier(
        generation,
        o.kind != PlatformKind::Aircraft && !air,
        o.kind != PlatformKind::Aircraft && air && matches!(role, "AS" | "3D") && low,
        env.sea_state,
        &env.precipitation,
        land,
        low,
    )?;
    let ew = super::electronic_warfare::range_modifier_for(w, o, def, t, generation);
    let range = max_range * altitude * clutter.min(ew);
    Ok(range.min(horizon_nm(radar_height(o)?, radar_height(t)?)))
}
pub fn radar_height(e: &Entity) -> Result<f64, String> {
    if matches!(e.kind, PlatformKind::Aircraft | PlatformKind::Missile) {
        return Ok(e.altitude_m.max(0.0));
    }
    if e.kind == PlatformKind::Base {
        return Ok(30.0);
    }
    let size = tables()["radar"]["physical_sizes"]
        .get(&e.class)
        .and_then(|s| s.as_str())
        .ok_or("5.2.8: physical radar-height class not verified")?;
    Ok(number(&tables()["radar"]["ship_radar_heights_m"][size]))
}

pub fn reaction(generation: usize, automatic: bool, roll: u8) -> Result<(i64, i64), String> {
    if !(1..=6).contains(&generation) || !(2..=12).contains(&roll) {
        return Err("invalid reaction input".into());
    }
    let col = match generation {
        1 => 0,
        2 | 3 => 1,
        4 => 2,
        5 | 6 if !automatic => 2,
        5 => 3,
        _ => 4,
    };
    let index = if generation <= 4 {
        generation - 1
    } else if !automatic {
        4
    } else if generation == 5 {
        5
    } else {
        6
    };
    Ok((
        tables()["radar"]["reaction_2d6_columns"][col][(roll - 2) as usize]
            .as_i64()
            .unwrap(),
        tables()["radar"]["reaction_normal"][index]
            .as_i64()
            .unwrap(),
    ))
}
/// PDF84: one original 2D6, own ES +1 and the best qualified TDL cue.
/// Results above the final printed row use row12 (explicit RTS table ceiling).
pub fn reaction_with_cues(
    generation: usize,
    automatic: bool,
    roll: u8,
    own_es: bool,
    tdl: u8,
) -> Result<(i64, i64), String> {
    if !(2..=12).contains(&roll) || tdl > 2 {
        return Err("8.1.1 invalid reaction cue input".into());
    }
    reaction(
        generation,
        automatic,
        (roll + u8::from(own_es) + tdl).min(12),
    )
}
pub fn reaction_cues(w: &World, observer: EntityId, target: EntityId) -> (bool, u8) {
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return (false, 0);
    }
    let own_es = w.rules.aaw.as_ref().is_some_and(|s| {
        s.firecontrol.tracks.values().any(|t| {
            t.observer == observer
                && t.target == target
                && t.report.as_ref().is_some_and(|r| {
                    r.sensor_kind == crate::db::SensorKind::Esm
                        && r.at <= w.tick
                        && w.tick - r.at < 180
                })
        })
    });
    (
        own_es,
        super::communications2026::cue_evidence(
            w,
            observer,
            target,
            super::communications2026::CuePurpose::AirDefenseReaction,
        )
        .modifier,
    )
}

pub fn fragment_probability(caliber: Option<f64>, sam: bool, speed: f64) -> Result<f64, String> {
    if !speed.is_finite() || speed < 0.0 {
        return Err("unknown missile speed".into());
    }
    let row = if sam {
        3
    } else {
        let c = caliber
            .filter(|c| c.is_finite() && *c > 0.0)
            .ok_or("8.1.5: unknown interceptor caliber")?;
        if c <= 30.0 {
            0
        } else if c <= 60.0 {
            1
        } else {
            2
        }
    };
    let col = if speed <= 750.0 {
        0
    } else if speed <= 1525.0 {
        1
    } else {
        2
    };
    Ok(number(&tables()["aaw"]["fragment_probabilities"][row][col]))
}
pub fn gun_caliber(class: &str, weapon: &str) -> Option<f64> {
    tables()["aaw"]["gun_caliber_bindings"]
        .get(format!("{class}:{weapon}"))
        .and_then(|v| v.as_f64())
}
pub fn sam_pd_legal(def: &WeaponDef, distance: f64) -> bool {
    !(def.kind == WeaponKind::Sam
        && distance <= number(&tables()["aaw"]["point_defense_nm"])
        && def.min_range_nm > number(&tables()["aaw"]["sam_pd_min_exclusion_nm"]))
}

pub fn intercept(w: &mut World, incoming: EntityId, shooter: EntityId, weapon: &WeaponDef) {
    if w.rules.pending_adjudications.contains_key(&incoming) {
        return;
    }
    let Some(m) = w
        .entity(incoming)
        .filter(|m| m.kind == PlatformKind::Missile)
        .cloned()
    else {
        return;
    };
    let Some(target) = m
        .projectile
        .as_ref()
        .and_then(|p| p.target)
        .and_then(|id| w.entity(id))
        .filter(|e| e.alive && e.kind == PlatformKind::Ship)
        .cloned()
    else {
        return;
    };
    let distance = m.pos.range_to(target.pos);
    if distance > number(&tables()["aaw"]["point_defense_nm"]) {
        return;
    }
    if !matches!(
        weapon.kind,
        WeaponKind::Sam | WeaponKind::Gun | WeaponKind::Ciws
    ) {
        return;
    }
    let caliber = w
        .entity(shooter)
        .and_then(|s| gun_caliber(&s.class, &weapon.id));
    let before_seq = w.event_seq;
    let identity = super::damage_runtime::event_identity(w, incoming, "8.1.5.fragment");
    let result = fragment_probability(caliber, weapon.kind == WeaponKind::Sam, m.speed_kts);
    let result=result.and_then(|p| {
        let mut rng=w.rng.derive(Rng::hash_tag(w.tick,incoming.0,&identity));let roll=1+(rng.next_f64()*100.0) as u8;
        let struck=roll as f64<=p*100.0;
        trace(w,target.id,"8.1.5.fragment_roll",json!({"identity":identity,"probability":p,"d100":roll,"struck":struck,"caliber_mm":caliber,"interceptor":weapon.id,"missile_speed_kts":m.speed_kts,"distance_nm":distance,"time_to_go_s":distance/m.speed_kts.max(1.0)*3600.0,"source":"English2026 PDF84/8-4"}));
        if struck {
            let def=m.projectile.as_ref().and_then(|d|w.db.weapons.get(&d.weapon)).cloned().ok_or("incoming missile data missing".to_string())?;
            super::damage_runtime::intercept_fragments(w,target.id,&def,identity)
        }else{Ok(())}
    });
    if let Err(reason) = result {
        let evidence = w
            .events
            .iter()
            .filter(|e| e.seq > before_seq)
            .cloned()
            .collect();
        w.rules.pending_adjudications.insert(
            incoming,
            super::damage_runtime::PendingAdjudication {
                evidence,
                tick: w.tick,
                rule: "8.1.5.fragments".into(),
                target: target.id,
                projectile: m,
            },
        );
        trace(
            w,
            target.id,
            "8.1.5.adjudication_blocked",
            json!({"reason":reason,"intercepted":true,"fragment_outcome":"pending"}),
        );
    }
}

fn track_key(observer: EntityId, sensor: &str, target: EntityId) -> String {
    format!("{}:{sensor}:{}", observer.0, target.0)
}
/// Storage ends this observer's continuous targeting work. Historical side
/// observations are not rewritten and redeployment earns a fresh native clock.
pub(crate) fn stow_platform(w:&mut World,id:EntityId) {
    if let Some(s)=w.rules.operations.as_mut() {
        s.tracks.retain(|key,_|!key.starts_with(&format!("{}:",id.0)));
        s.rt_air_tracks.retain(|_,track|track.receiver!=id);
    }
    if let Some(aaw)=w.rules.aaw.as_mut() {aaw.firecontrol.tracks.retain(|_,track|track.observer!=id);}
}
/// Native selected radar targeting readiness. No artificial ASCM/torpedo
/// placement-quality scale is applied to an air target.
pub fn native_air_target_ready(w: &World, actor: EntityId, target: EntityId, sensor: &str) -> bool {
    let Some(e) = w
        .entity(actor)
        .filter(|e| e.alive && crate::sensors::radar_emitting(e) && e.systems.radar > 0.05)
    else {
        return false;
    };
    let Some(def) = w.db.sensors.get(sensor) else {
        return false;
    };
    if def.kind != SensorKind::Radar
        || !e
            .sensors
            .iter()
            .any(|s| s.sensor == sensor && s.is_active_kind(SensorKind::Radar))
        || crate::sensors::sensor_damaged(e, sensor)
    {
        return false;
    }
    if e.kind == PlatformKind::Aircraft {
        return super::aviation2026::operations_radar_profile(w, e, def)
            .is_some_and(|p| matches!(p["role"].as_str(), Some("AI" | "LD/SD")));
    }
    w.rules
        .operations
        .as_ref()
        .and_then(|s| s.tracks.get(&track_key(actor, sensor, target)))
        .is_some_and(|clock| w.tick >= clock.engage && clock.last_eligible <= w.tick)
}
/// Actual RT delivery starts the receiver's own reaction process once. It
/// neither borrows the sender's combat generation nor rerolls repeated packets.
pub fn receive_rt_air_target(
    w: &mut World,
    receiver: EntityId,
    target: EntityId,
    evidence: &super::communications2026::AirFcsEvidence,
) {
    if !super::aaw2026::active(w) {
        return;
    }
    let Some(entity) = w.entity(receiver).filter(|e| e.alive) else {
        return;
    };
    if entity.kind == PlatformKind::Aircraft {
        return;
    } // AAM uses its original60s packet clock.
    let key = format!("{}:{}", receiver.0, target.0);
    let old = w
        .rules
        .operations
        .as_ref()
        .and_then(|s| s.rt_air_tracks.get(&key))
        .cloned();
    if let Some(mut existing) = old.filter(|r| r.continuous) {
        existing.last_received = w.tick;
        w.rules
            .operations
            .as_mut()
            .unwrap()
            .rt_air_tracks
            .insert(key, existing);
        return;
    }
    let native = w.rules.operations.as_ref().and_then(|s| {
        s.tracks
            .iter()
            .filter(|(key, _)| {
                key.starts_with(&format!("{}:", receiver.0))
                    && key.ends_with(&format!(":{}", target.0))
            })
            .map(|(_, clock)| clock.clone())
            .min_by_key(|clock| clock.engage)
    });
    let Some(clock) = native.or_else(|| initial_clock(w, receiver, "RT-air", target)) else {
        return;
    };
    let detail = json!({"source_observer":evidence.source_observer,"source_sensor":evidence.source_sensor,
        "receiver":receiver,"truth_target":target,"2d6":clock.roll,"seen":clock.seen,
        "recognize_at":clock.recognized,"engage_at":clock.engage,"source":"English2026 6.3.9/8.1.1; receiver reaction, no placement quality"});
    w.rules.operations.as_mut().unwrap().rt_air_tracks.insert(
        key,
        RtAirTrack {
            receiver,
            target,
            clock,
            continuous: true,
            last_received: w.tick,
        },
    );
    w.emit(
        EventKind::RulesEvent {
            domain: "operations".into(),
            rule: "6.3.9.rt_air_reaction".into(),
            side: None,
            unit: Some(receiver),
            contact: None,
            detail: detail.to_string(),
        },
        Visibility::Referee,
    );
}
pub fn radar_recognized(w: &World, o: &Entity, def: &SensorDef, t: &Entity) -> bool {
    if !matches!(t.kind, PlatformKind::Aircraft | PlatformKind::Missile) {
        return true;
    }
    if super::aviation2026::operations_radar_profile(w, o, def)
        .is_some_and(|p| matches!(p["role"].as_str(), Some("AI" | "LD/SD")))
    {
        return true;
    }
    w.rules
        .operations
        .as_ref()
        .and_then(|s| s.tracks.get(&track_key(o.id, &def.id, t.id)))
        .cloned()
        .or_else(|| initial_clock(w, o.id, &def.id, t.id))
        .is_some_and(|r| w.tick >= r.recognized)
}
fn initial_clock(w: &World, o: EntityId, sensor: &str, t: EntityId) -> Option<RadarTrack> {
    let c = tables()["aaw"]["combat_systems"].get(&w.entity(o)?.class)?;
    let gen = c[0].as_u64()? as usize;
    let auto = c[1] == 1;
    let mut rng = w.rng.derive(Rng::hash_tag(
        w.tick,
        o.0,
        &format!("5.2.reaction:{sensor}:{}", t.0),
    ));
    let roll = 2 + (rng.next_f64() * 6.0) as u8 + (rng.next_f64() * 6.0) as u8;
    let (own_es, tdl) = reaction_cues(w, o, t);
    let (extra, normal) = reaction_with_cues(gen, auto, roll, own_es, tdl).ok()?;
    Some(RadarTrack {
        seen: w.tick,
        recognized: w.tick + extra.max(0) * 30,
        engage: w.tick + (normal + extra).max(0) * 30,
        last_eligible: w.tick,
        roll,
    })
}
/// Apply timers only after both the immutable sensor scan and eligibility check.
pub fn radar_clocks(w: &mut World, candidates: &[(EntityId, String, EntityId)]) {
    if !active(w) {
        return;
    }
    let now = w.tick;
    let continuity: Vec<_> = w
        .rules
        .operations
        .as_ref()
        .unwrap()
        .rt_air_tracks
        .iter()
        .map(|(key, track)| {
            (
                key.clone(),
                super::communications2026::air_source_still_live(w, track.receiver, track.target),
            )
        })
        .collect();
    for (key, live) in continuity {
        if !live {
            w.rules
                .operations
                .as_mut()
                .unwrap()
                .rt_air_tracks
                .get_mut(&key)
                .unwrap()
                .continuous = false;
        }
    }
    let eligible: BTreeSet<_> = candidates
        .iter()
        .map(|(o, s, t)| track_key(*o, s, *t))
        .collect();
    w.rules
        .operations
        .as_mut()
        .unwrap()
        .tracks
        .retain(|k, _| eligible.contains(k));
    for (o, sensor, t) in candidates {
        let key = track_key(*o, sensor, *t);
        if let Some(r) = w.rules.operations.as_mut().unwrap().tracks.get_mut(&key) {
            r.last_eligible = now;
            continue;
        }
        let Some(r) = initial_clock(w, *o, sensor, *t) else {
            continue;
        };
        // Before recognition, even the existence of a clock is referee-only.
        let (own_es, tdl) = reaction_cues(w, *o, *t);
        let mut detail = json!({"sensor":sensor,"truth_target":t,"2d6":r.roll,"recognize_at":r.recognized,"engage_at":r.engage,"source":"English2026 PDF41/84"});
        if w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full {
            detail["own_es_modifier"] = json!(u8::from(own_es));
            detail["tdl_modifier"] = json!(tdl);
            detail["modified_2d6"] = json!((r.roll + u8::from(own_es) + tdl).min(12));
            detail["tdl_refs"] = json!(
                super::communications2026::cue_evidence(
                    w,
                    *o,
                    *t,
                    super::communications2026::CuePurpose::AirDefenseReaction
                )
                .refs
            );
        }
        w.emit(
            EventKind::RulesEvent {
                domain: "operations".into(),
                rule: "5.2.1.reaction_clock".into(),
                side: None,
                unit: Some(*o),
                contact: None,
                detail: detail.to_string(),
            },
            Visibility::Referee,
        );
        w.rules.operations.as_mut().unwrap().tracks.insert(key, r);
    }
}

pub fn sam_reaction_ready(w: &World, shooter: EntityId, target: EntityId, def: &WeaponDef) -> bool {
    if def.kind != WeaponKind::Sam {
        return true;
    }
    let Some(o) = w.entity(shooter) else {
        return false;
    };
    let Some(t) = w.entity(target) else {
        return false;
    };
    let height = o
        .sensors
        .iter()
        .filter_map(|s| w.db.sensors.get(&s.sensor))
        .filter(|s| !crate::sensors::sensor_damaged(o, &s.id))
        .any(|s| {
            super::aviation2026::operations_radar_profile(w, o, s)
                .is_some_and(|p| matches!(p["role"].as_str(), Some("3D" | "HF")))
                && radar_range(w, o, s, t).is_ok_and(|r| r >= o.pos.range_to(t.pos))
        });
    let penalty = if o.pos.range_to(t.pos) > 15.0 && !height {
        60
    } else {
        0
    };
    let own_ready = w.rules.operations.as_ref().is_some_and(|s| {
        o.sensors.iter().any(|sensor| {
            s.tracks
                .get(&track_key(shooter, &sensor.sensor, target))
                .is_some_and(|r| w.tick >= r.engage + penalty)
        })
    });
    if own_ready {
        return true;
    }
    let Some(remote) = super::communications2026::received_air_solution(w, shooter, target)
        .ok()
        .flatten()
    else {
        return false;
    };
    let remote_penalty =
        if o.pos.range_to(remote.position) > 15.0 && !height && remote.altitude_m.is_none() {
            60
        } else {
            0
        };
    w.rules
        .operations
        .as_ref()
        .and_then(|s| s.rt_air_tracks.get(&format!("{}:{}", shooter.0, target.0)))
        .is_some_and(|clock| clock.continuous && w.tick >= clock.clock.engage + remote_penalty)
}

pub fn motion_known(w: &World, side: crate::Side, target: EntityId) -> Option<bool> {
    w.rules
        .operations
        .as_ref()?
        .motion
        .get(&format!("{}:{}", side.index(), target.0))
        .map(|m| m.known)
}
pub fn update_radar_motion(w: &mut World, side: crate::Side, targets: &[EntityId]) {
    if !active(w) {
        return;
    }
    let mut changes = Vec::new();
    let now = w.tick;
    for target in targets {
        let Some(c) = w.sides[side.index()]
            .contacts
            .values()
            .find(|c| c.about == *target && !c.stale)
        else {
            continue;
        };
        let m = w
            .rules
            .operations
            .as_mut()
            .unwrap()
            .motion
            .entry(format!("{}:{}", side.index(), target.0))
            .or_insert(MotionFix {
                start: now,
                last: now,
                first: c.estimate,
                known: false,
            });
        if now - m.last > 1 {
            *m = MotionFix {
                start: now,
                last: now,
                first: c.estimate,
                known: false,
            };
        }
        m.last = now;
        if now - m.start >= 180 {
            let distance = m.first.range_to(c.estimate);
            changes.push((
                c.id,
                if distance > 0.0 {
                    m.first.bearing_to(c.estimate)
                } else {
                    c.est_course_deg
                },
                distance / (now - m.start) as f64 * 3600.0,
            ));
            m.known = true;
            m.start = now;
            m.first = c.estimate;
        } else if !m.known {
            changes.push((c.id, 0.0, 0.0));
        }
    }
    for (id, course, speed) in changes {
        if let Some(c) = w.sides[side.index()].contacts.get_mut(&id) {
            c.est_course_deg = course;
            c.est_speed_kts = speed;
        }
    }
}

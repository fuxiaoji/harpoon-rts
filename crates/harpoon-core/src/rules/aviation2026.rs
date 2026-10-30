//! Harpoon V 2026 chapter 4, explicitly versioned source mechanisms.
//! State and input data never infer aircraft fuel, payload or equipment.
use crate::{EntityId, Rng, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
pub mod waterborne;
pub mod micro_uav;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub waterborne:Option<waterborne::State>,
    pub airframes: BTreeMap<String, Airframe>,
    pub deck_parking: BTreeMap<EntityId, u32>,
    pub respots: BTreeMap<EntityId, Respot>,
    pub flights: BTreeMap<EntityId, Flight>,
    pub refuels: Vec<RefuelJob>,
    pub diagnostics: BTreeSet<String>,
    pub hifr: BTreeMap<EntityId, (EntityId, i64)>,
    pub hifr_used: BTreeSet<EntityId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hifr_under_fire_tick: BTreeMap<EntityId, i64>,
    pub low_exposure: BTreeMap<EntityId, LowExposure>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LowExposure {
    pub next_roll: i64,
    pub turns: u32,
    pub turn_degrees: f64,
    pub last_course: f64,
    pub attacked: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Airframe {
    pub carrier: EntityId,
    pub class: String,
    pub size: usize,
    pub rotary: bool,
    pub load: String,
    pub ready_since: i64,
    pub sorties: Vec<(i64, Option<i64>)>,
    pub aircraft: Option<EntityId>,
    pub ready_at: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Respot {
    pub due: i64,
    pub dice: Vec<u8>,
    pub parked: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Flight {
    pub spec: FlightSpec,
    pub fuel_nm: f64,
    pub capacity_nm: f64,
    pub throttle: Throttle,
    pub payload_fraction: f64,
    pub last_tick: i64,
    pub last_position: crate::Vec2,
    #[serde(default)]
    pub external_weapon_rounds: BTreeMap<String, u16>,
    #[serde(default)]
    pub internal_weapon_rounds: BTreeMap<String, u16>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RefuelJob {
    pub tanker: EntityId,
    pub receiver: EntityId,
    pub due: i64,
    pub receiver_range_nm: f64,
    pub tanker_range_nm: f64,
    pub method: RefuelMethod,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlightSpec {
    pub engine: Engine,
    pub size: usize,
    pub cruise_kts: f64,
    pub cruise_range_nm: f64,
    pub fuel_kg: f64,
    pub max_payload_kg: f64,
    pub source: String,
    pub receive: Option<RefuelMethod>,
    pub tanker_slots: u8,
    pub centerline: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Engine {
    TJ,
    TF,
    TP,
    TS,
    IP,
    RP,
    IPS,
    RPS,
    RT,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Throttle {
    Cruise,
    Fmp,
    Reheat,
    Loiter,
    Hover,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RefuelMethod {
    Boom,
    WesternProbe,
    SovietProbe,
    ZA,
    BuddyWestern,
    BuddySoviet,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Configuration {
    /// Explicit scenario-author qualification for the narrow §4.5.3 exemption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub micro_uav: Option<micro_uav::Configuration>,
    pub loads: BTreeMap<String, Loadout>,
    /// Keys are scenario unit names, not hidden opposing observations.
    pub parking: BTreeMap<String, u32>,
    pub source: String,
    #[serde(default)]
    pub initial_fuel_nm: BTreeMap<String, f64>,
    #[serde(default)]
    pub visibility_percent: Option<f64>,
    /// Scenario-author declaration for the narrow §9.6 NoAA executor. It is
    /// never inferred from hidden opposing units, inventories or event tails.
    #[serde(default)]
    pub no_aa_surface_attack_source: Option<String>,
    /// Only explicitly known fixed range aimpoints are supported here.
    #[serde(default)]
    pub surface_aimpoints: BTreeMap<String, SurfaceAimpoint>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SurfaceAimpoint {
    pub source: String,
    pub point: crate::Vec2,
    pub controlled_range: bool,
    pub fixed_target: bool,
    /// Scenario-author knowledge allocation, never derived from target truth.
    /// Missing allocation grants no side permission to use or discover a key.
    #[serde(default)]
    pub allowed_sides: Vec<crate::Side>,
}

pub fn surface_aimpoint<'a>(
    w: &'a World,
    side: crate::Side,
    key: &str,
) -> Option<&'a SurfaceAimpoint> {
    w.context
        .aviation_2026
        .as_ref()?
        .surface_aimpoints
        .get(key)
        .filter(|p| {
            p.allowed_sides.contains(&side)
                && !p.source.trim().is_empty()
                && p.controlled_range
                && p.fixed_target
                && p.point.x.is_finite()
                && p.point.y.is_finite()
        })
}

fn surface_attack_authorizations(w: &World, side: crate::Side) -> Vec<Value> {
    let Some(config) = w.context.aviation_2026.as_ref() else {
        return vec![];
    };
    let no_aa = config
        .no_aa_surface_attack_source
        .as_deref()
        .filter(|s| !s.trim().is_empty());
    config.surface_aimpoints.iter().filter_map(|(key, _)| {
        let p = surface_aimpoint(w, side, key)?;
        Some(json!({"aimpoint":key,"point":p.point,"source":p.source,"controlled_range":p.controlled_range,
            "fixed_target":p.fixed_target,"no_aa_source":no_aa,"no_aa_declared":no_aa.is_some()}))
    }).collect()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Loadout {
    pub payload_kg: f64,
    pub tanks: Vec<Tank>,
    pub source: String,
    #[serde(default)]
    pub buddy: bool,
    /// Explicit scenario fit: every carried round of a listed weapon is
    /// external. Mixed internal/external stores of the same variant need
    /// mount metadata and are deliberately not inferred by this schema.
    #[serde(default)]
    pub external_weapon_rounds: BTreeMap<String, u16>,
    #[serde(default)]
    pub internal_weapon_rounds: BTreeMap<String, u16>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tank {
    pub range_add_nm: f64,
    pub fuel_weight_kg: f64,
    pub source: String,
}
/// Additional fits belong only to v4; the archived v3 table stays unchanged.
pub fn operations_aircraft(w: &World, class: &str) -> Option<&'static Value> {
    (active(w)
        .then(|| tables()["operation_aircraft_variants"].get(class))
        .flatten())
    .or_else(|| super::operations::tables()["aviation"]["aircraft"].get(class))
}
pub fn operations_radar_profile(
    w: &World,
    o: &crate::world::Entity,
    def: &crate::db::SensorDef,
) -> Option<&'static Value> {
    (active(w)
        .then(|| tables()["operation_radar_variants"].get(format!("{}:{}", o.class, def.id)))
        .flatten())
    .or_else(|| super::operations::radar_profile(o, def))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    WaterTakeoff,
    WaterLand,
    Respot,
    Alert {
        class: String,
        count: u8,
    },
    Throttle {
        setting: Throttle,
    },
    Refuel {
        receiver: EntityId,
        method: RefuelMethod,
    },
    Hifr {
        ship: EntityId,
    },
    Jettison {
        weapon: String,
        rounds: u16,
    },
}
pub fn tables() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/aviation-2026.json")).unwrap()
    })
}
fn n(section: &str, key: &str) -> f64 {
    tables()[section][key].as_f64().unwrap()
}
fn finite_positive(v: f64) -> bool {
    v.is_finite() && v > 0.0
}
pub fn spec(class: &str) -> Result<FlightSpec, String> {
    serde_json::from_value(
        tables()["aircraft"]
            .get(class)
            .cloned()
            .ok_or_else(|| format!("4.7 Annex B flight specification not verified: {class}"))?,
    )
    .map_err(|e| e.to_string())
}
pub fn range_cost(
    engine: Engine,
    throttle: Throttle,
    distance_nm: f64,
    elapsed_s: f64,
    cruise_kts: f64,
    altitude_m: f64,
    payload_fraction: f64,
) -> Result<f64, String> {
    if ![distance_nm, elapsed_s, altitude_m, payload_fraction]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.0)
        || !finite_positive(cruise_kts)
        || payload_fraction > 1.0
    {
        return Err("4.7 invalid distance/time/altitude/payload/cruise speed".into());
    }
    let jet = matches!(engine, Engine::TJ | Engine::TF);
    if engine == Engine::RT {
        return Err("4.7 rocket propulsion range expenditure not specified".into());
    }
    if matches!(engine, Engine::IP | Engine::RP)
        && altitude_m > 7500.0
        && matches!(throttle, Throttle::Cruise | Throttle::Loiter)
    {
        return Err("4.7.3 unsupercharged piston aircraft must use FMP at High altitude".into());
    }
    let base = match throttle {
        Throttle::Cruise => distance_nm,
        Throttle::Fmp => distance_nm * n("fuel", if jet { "jet_fmp" } else { "prop_fmp" }),
        Throttle::Reheat if jet => distance_nm * n("fuel", "reheat"),
        Throttle::Loiter => {
            if distance_nm > 1e-9 {
                return Err("4.7.4 loiter may not change location".into());
            }
            cruise_kts * elapsed_s / 3600.0 * n("fuel", "loiter")
        }
        Throttle::Hover if engine == Engine::TS => {
            if distance_nm > 1e-9 {
                return Err("4.3 hovering helicopter is stationary".into());
            }
            cruise_kts * elapsed_s / 3600.0 * n("fuel", "prop_fmp")
        }
        _ => return Err("4.2 unsupported propulsion/throttle combination".into()),
    };
    let altitude = if jet && altitude_m <= 2000.0 {
        n("fuel", "jet_low")
    } else {
        1.0
    };
    let heavy = if payload_fraction >= n("fuel", "heavy_fraction") {
        n("fuel", "heavy_remaining_range")
    } else {
        1.0
    };
    Ok(base * altitude / heavy)
}
pub fn refuel_seconds(size: usize, method: RefuelMethod) -> Result<i64, String> {
    if !(1..=3).contains(&size) {
        return Err("4.8.2 VSmall/unlisted size not printed in refuel table".into());
    }
    let key = match method {
        RefuelMethod::Boom => "boom_turns",
        RefuelMethod::WesternProbe | RefuelMethod::SovietProbe => "probe_turns",
        RefuelMethod::BuddyWestern | RefuelMethod::BuddySoviet => "buddy_turns",
        RefuelMethod::ZA => return Err("4.8 early Badger timing not specified".into()),
    };
    let hookup = tables()["refuel"]["hookup_turns"][size].as_i64().unwrap();
    let pumping = tables()["refuel"][key][size]
        .as_i64()
        .ok_or("4.8.2 Large aircraft cannot use buddy stores")?;
    Ok((hookup + pumping) * n("refuel", "tactical_turn_s") as i64)
}
pub fn respot_seconds(parked: u32, dice: &[u8]) -> Result<i64, String> {
    let batch = n("respot", "aircraft_per_batch") as u32;
    if parked % batch != 0 {
        return Err("4.6 partial twelve-aircraft respot batch has no printed timing; author adaptation required".into());
    }
    if dice.len() != parked as usize / batch as usize || dice.iter().any(|v| !(1..=6).contains(v)) {
        return Err("4.6 requires one explicit D6 for every twelve parked aircraft".into());
    }
    Ok(dice
        .iter()
        .map(|d| n("respot", "base_minutes") as i64 + *d as i64)
        .sum::<i64>()
        * 60)
}
pub fn cap_time_hours(range: f64, station: f64, cruise: f64) -> Result<f64, String> {
    if ![range, station].iter().all(|v| v.is_finite() && *v >= 0.) || !finite_positive(cruise) {
        return Err("4.7.7 invalid CAP input".into());
    }
    if range < 2. * station {
        return Err("4.7.6 pilot refuses mission without enough return fuel".into());
    }
    Ok((range - 2. * station) / n("fuel", "loiter") / cruise)
}
pub fn compatible(tanker: RefuelMethod, receiver: RefuelMethod) -> bool {
    tanker == receiver
        || matches!(
            (tanker, receiver),
            (RefuelMethod::BuddyWestern, RefuelMethod::WesternProbe)
                | (RefuelMethod::BuddySoviet, RefuelMethod::SovietProbe)
        )
}
pub fn source_speed(class: &str, altitude: f64, setting: Throttle) -> Result<f64, String> {
    if !altitude.is_finite() || altitude < 0. {
        return Err("4.5 invalid altitude".into());
    }
    let ceiling = tables()["ceilings_m"][class]
        .as_f64()
        .ok_or("4.5 verified ceiling missing")?;
    if altitude > ceiling {
        return Err("4.5 exceeds verified aircraft ceiling".into());
    }
    if matches!(setting, Throttle::Loiter | Throttle::Hover) {
        return Ok(0.);
    }
    let row = if altitude <= 2000. {
        0
    } else if altitude <= 7500. {
        1
    } else if altitude > 15500. {
        3
    } else {
        2
    };
    let col = match setting {
        Throttle::Cruise => 0,
        Throttle::Fmp => 1,
        Throttle::Reheat => 2,
        _ => unreachable!(),
    };
    let speed = tables()["speed_rows"][class][row][col]
        .as_f64()
        .ok_or("4.2 throttle/altitude speed is not printed")?;
    if altitude <= 60. {
        if setting == Throttle::Reheat {
            return Err("4.5.2/4.5.4 SSkim/VLow cannot use reheat".into());
        }
        let fmp = tables()["speed_rows"][class][row][1].as_f64().unwrap();
        return Ok(speed.min(fmp).min(n(
            "low_flight",
            if altitude <= 20. {
                "sskim_max_kts"
            } else {
                "vlow_max_kts"
            },
        )));
    }
    Ok(speed)
}
/// Source speed with the current aircraft's optional §14.9 restriction. The
/// exact source altitude row is retained; no aggregate DB maximum is used.
pub fn entity_source_speed(
    w: &World,
    id: EntityId,
    altitude: f64,
    setting: Throttle,
) -> Result<f64, String> {
    let e = w.entity(id).ok_or("missing aircraft")?;
    let speed = source_speed_world(w,&e.class,altitude,setting)?;
    match super::expanded_damage2026::aircraft_speed_cap(w, id)? {
        Some(cap) => Ok(speed
            .min(cap)
            .min(source_speed_world(w,&e.class, altitude, Throttle::Cruise)?)),
        None => Ok(speed),
    }
}
fn source_speed_world(w:&World,class:&str,altitude:f64,setting:Throttle)->Result<f64,String>{
    waterborne::declared_speed(w,class,altitude,setting).unwrap_or_else(||source_speed(class,altitude,setting))
}
#[derive(Clone, Debug)]
pub struct MovementLimits {
    pub climb_mps: f64,
    pub descend_mps: f64,
    pub minimum_kts: f64,
    pub maximum_kts: f64,
    pub ceiling_m: f64,
    pub no_acceleration: bool,
}
/// Called by the real movement integration, keeping source rates out of UI.
/// This deliberately rejects the unsupported terrain/NOE branch rather than
/// treating an ocean coastline polygon as a digital elevation model.
pub fn movement_limits(w: &World, id: EntityId) -> Result<MovementLimits, String> {
    if let Some(limits)=waterborne::movement_limits(w,id){return limits;}
    let e = w.entity(id).ok_or("missing aircraft")?;
    let f = w
        .rules
        .aviation
        .as_ref()
        .and_then(|s| s.flights.get(&id))
        .ok_or("4.5 source flight/load data missing")?;
    let rotary = f.spec.engine == Engine::TS;
    if !rotary
        && e.ordered_altitude_m <= 20.
        && (w.day_night != crate::types::DayNight::Day
            || !w
                .context
                .aviation_2026
                .as_ref()
                .and_then(|c| c.visibility_percent)
                .is_some_and(|v| v.is_finite() && (40.0..=100.0).contains(&v)))
    {
        return Err(
            "4.5.2 manned SSkim requires daylight and explicitly declared visibility >=40%".into(),
        );
    }
    if e.ordered_altitude_m <= 60.
        && w.context
            .terrain
            .as_ref()
            .is_some_and(|t| t.contains(e.pos))
    {
        return Err("4.5.3/4.5.5 low flight over land requires verified elevation and navigation/weather equipment".into());
    }
    let heavy = f.payload_fraction >= 0.5;
    let fitted_mr = tables()["maneuver"][&e.class][usize::from(heavy)]
        .as_f64()
        .ok_or("4.5 source maneuver rating missing")?;
    let mr = super::expanded_damage2026::aircraft_profile(w, id, fitted_mr, heavy)?.maneuver_rating;
    // The source rate is proportional to MR. Critical/full-load MR zero
    // therefore yields zero rate without passing zero as a positive source MR.
    let (climb, descend, dive) = if mr == 0. {
        (0., 0., 1.)
    } else if e.ordered_altitude_m == e.altitude_m
        && !rotary
        && matches!(f.spec.engine, Engine::TJ | Engine::TF)
        && mr > 2.
        && mr < 2.5
    {
        // An unprinted vertical category does not prevent source-supported
        // level flight. A later altitude request must resolve that category.
        (0., 0., 1.)
    } else {
        let climb = altitude_rate(f.spec.engine, rotary, mr, e.altitude_m, heavy, true)?.0;
        let (descend, dive) = altitude_rate(f.spec.engine, rotary, mr, e.altitude_m, heavy, false)?;
        (climb, descend, dive)
    };
    let from_fmp = source_speed(&e.class, e.altitude_m, Throttle::Fmp)?;
    let level = entity_source_speed(w, id, e.altitude_m, f.throttle)?;
    let to = entity_source_speed(w, id, e.ordered_altitude_m, f.throttle)?;
    let climbing = e.ordered_altitude_m > e.altitude_m;
    let diving = e.ordered_altitude_m < e.altitude_m;
    let no_acceleration = climbing && (e.ordered_altitude_m - e.altitude_m) > climb * 0.5;
    let mut max = level.min(to);
    if diving && (e.altitude_m - e.ordered_altitude_m) > descend * 0.5 {
        let fastest = tables()["speed_rows"][&e.class]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|a| a.as_array().unwrap())
            .filter_map(|v| v.as_f64())
            .fold(0., f64::max);
        max = (level * dive).min(fastest);
    }
    if super::expanded_damage2026::aircraft_speed_cap(w, id)?.is_some() {
        max = max
            .min(entity_source_speed(w, id, e.altitude_m, Throttle::Cruise)?)
            .min(entity_source_speed(
                w,
                id,
                e.ordered_altitude_m,
                Throttle::Cruise,
            )?);
    }
    let minimum = if matches!(f.throttle, Throttle::Loiter | Throttle::Hover) {
        0.
    } else {
        from_fmp * n("low_flight", "minimum_fmp_fraction")
    };
    Ok(MovementLimits {
        climb_mps: climb,
        descend_mps: descend,
        minimum_kts: minimum,
        maximum_kts: max,
        ceiling_m: tables()["ceilings_m"][&e.class].as_f64().unwrap(),
        no_acceleration,
    })
}
/// Printed factors only. Do not recreate Annex Ba rounding from kg/nmi ratios.
pub fn offload_factor(tanker: &str, receiver: &str) -> Result<f64, String> {
    if matches!(tanker, "us_f18e" | "us_f18e_asm") {
        let receiver_rate = tables()["offload_fuel_rates"][receiver]
            .as_f64()
            .ok_or("4.8.1 verified receiving fuel rate required")?;
        return Ok(tables()["offload_fuel_rates"]["us_f18e"].as_f64().unwrap() / receiver_rate);
    }
    tables()["offload"][tanker][receiver]
        .as_f64()
        .ok_or("4.8.1 Annex Ba tanker/receiver offload factor is not verified".into())
}
pub fn transfer_limits(
    tanker_remaining: f64,
    tanker_return: f64,
    receiver_missing: f64,
    factor: f64,
) -> Result<(f64, f64), String> {
    if ![tanker_remaining, tanker_return, receiver_missing]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.)
        || !finite_positive(factor)
    {
        return Err("4.8 invalid transfer budget".into());
    }
    let given = receiver_missing.min((tanker_remaining - tanker_return).max(0.) * factor);
    Ok((given / factor, given))
}
/// Printed altitude-change table, per-second RTS integration. Rate is selected
/// at start altitude, not recomputed midway through a source Tactical Turn.
pub fn altitude_rate(
    engine: Engine,
    rotary: bool,
    maneuver: f64,
    start_m: f64,
    loaded: bool,
    climb: bool,
) -> Result<(f64, f64), String> {
    if !finite_positive(maneuver) || !start_m.is_finite() || start_m < 0. {
        return Err("4.5 invalid altitude/maneuver".into());
    }
    let key = if rotary {
        "helicopter"
    } else {
        match engine {
            Engine::TJ | Engine::TF => {
                if maneuver <= 2. {
                    "jet_low_maneuver"
                } else if maneuver >= 2.5 {
                    "jet_high_maneuver"
                } else {
                    return Err("4.5 Maneuver Rating between printed 2.0 and 2.5 rows".into());
                }
            }
            Engine::RT => "rocket",
            _ => "prop",
        }
    };
    let a = &tables()["climb"][key];
    let band = if start_m <= 2000. {
        2.
    } else if start_m > 7500. {
        0.5
    } else {
        1.
    };
    Ok((
        a[if climb { 0 } else { 1 }].as_f64().unwrap() * maneuver * band / 180.
            * if loaded && climb { 0.5 } else { 1. },
        a[2].as_f64().unwrap(),
    ))
}
pub fn low_crash_chance(
    speed: f64,
    consecutive_turns: u32,
    turn_deg: f64,
    attacked: bool,
) -> Result<f64, String> {
    if !speed.is_finite() || speed < 0. || !turn_deg.is_finite() {
        return Err("4.5.2 invalid speed/turn".into());
    }
    let d = &tables()["low_flight"];
    let col = d["crash_speed_maxima_kts"]
        .as_array()
        .unwrap()
        .iter()
        .position(|n| speed <= n.as_f64().unwrap())
        .ok_or("4.5.2 speed above last printed crash band")?;
    Ok((d["crash_probability"][col].as_f64().unwrap()
        + if speed > 150. {
            (consecutive_turns as f64 * n("low_flight", "crash_per_turn"))
                .min(n("low_flight", "crash_turn_cap"))
        } else {
            0.
        }
        + if turn_deg.abs() > 30. {
            n("low_flight", "crash_turning")
        } else {
            0.
        }
        + if attacked {
            n("low_flight", "crash_attacked")
        } else {
            0.
        })
    .min(1.))
}
pub fn swamp_chance(sea_state: u8, moored: bool, shelter_states: u8) -> Result<f64, String> {
    if sea_state > 9 || shelter_states > 2 {
        return Err("4.6.6 invalid sea state or undeclared shelter".into());
    }
    Ok(if moored {
        0.
    } else {
        sea_state.saturating_sub(shelter_states).saturating_sub(3) as f64
            * n("swamp", "chance_per_excess")
    })
}
pub fn active(w: &World) -> bool {
    serde_json::to_value(w.rules.profile)
        .ok()
        .as_ref()
        .is_some_and(|v| v == "hv2026-rts-v4")
}
pub fn trace(w: &mut World, id: EntityId, rule: &str, detail: Value) {
    super::operations::trace(w, id, rule, detail);
}
/// Caller projects this only while iterating its own units. Contents are
/// strictly unit-local plus friendly refueling/airframe identities.
pub fn own_view(w: &World, id: EntityId) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let e = w.entity(id)?;
    let s = w.rules.aviation.as_ref()?;
    let frames:Vec<_>=s.airframes.iter().filter(|(_,a)|a.carrier==id).map(|(key,a)|json!({
        "id":key,"class":a.class,"aircraft":a.aircraft,"ready_at":a.ready_at,"alert5_since":a.ready_since,
        "alert5_expires":a.ready_since+3600,"alert5_valid":a.aircraft.is_none()&&a.ready_at.is_none()&&w.tick-a.ready_since<=3600,
        "sorties":a.sorties,"daily_sortie_eligible":sortie_allowed(a,w.tick)
    })).collect();
    let flight=s.flights.get(&id).map(|f|json!({
        "source":f.spec.source,"fuel_cruise_equivalent_nm":f.fuel_nm,"capacity_cruise_equivalent_nm":f.capacity_nm,
        "fraction":f.fuel_nm/f.capacity_nm,"throttle":f.throttle,"payload_fraction":f.payload_fraction,"external_weapon_rounds":f.external_weapon_rounds,"internal_weapon_rounds":f.internal_weapon_rounds,
        "can_receive":f.spec.receive,"tanker_slots":f.spec.tanker_slots,"tanker_methods":tables()["tanker_methods"][&e.class],"hifr_already_used":s.hifr_used.contains(&id)
    }));
    let refuels:Vec<_>=s.refuels.iter().filter(|j|j.tanker==id||j.receiver==id).map(|j|json!({"tanker":j.tanker,"receiver":j.receiver,"method":j.method,"remaining_s":(j.due-w.tick).max(0)})).collect();
    let hifr = s
        .hifr
        .get(&id)
        .map(|(ship, due)| json!({"ship":ship,"remaining_s":(*due-w.tick).max(0)}));
    if e.deck.is_none() && e.kind != crate::types::PlatformKind::Aircraft {
        return None;
    }
    Some(
        json!({"profile":"hv2026-rts-v4","deck_parking":s.deck_parking.get(&id),"respot":s.respots.get(&id),"airframes":frames,
        "flight":flight,"flight_data_blocked":e.kind==crate::types::PlatformKind::Aircraft&&flight.is_none(),"refuels":refuels,"hifr":hifr,"aircombat":super::aircombat2026::own_view(w,id),
        "surface_attack_authorizations":surface_attack_authorizations(w,e.side),"waterborne":waterborne::own_view(w,id)}),
    )
}
pub fn mark_under_fire(w: &mut World, id: EntityId) {
    if !active(w) {
        return;
    }
    if let Some(s) = w
        .rules
        .aviation
        .as_mut()
        .and_then(|s| s.low_exposure.get_mut(&id))
    {
        s.attacked = true;
    }
    // The actual AAW/AAM consumer calls this only after physical fire commits.
    // Preserve the current tick even if the shot is an instantaneous gun miss,
    // so a later same-phase HIFR command cannot recreate a noncombat job.
    let now = w.tick;
    let interrupted = if let Some(s) = w.rules.aviation.as_mut() {
        s.hifr_under_fire_tick.insert(id, now);
        s.hifr.remove(&id)
    } else {
        None
    };
    if let Some((ship, _)) = interrupted {
        trace(w, id, "4.9.hifr_cancelled", json!({"ship":ship,"reason":"actual_defensive_fire","transferred":0}));
    }
}
/// Reads entity/combat state only: the caller may own the temporarily taken
/// aviation State while checking each continuing HIFR job.
fn hifr_noncombat(w: &World, aircraft: EntityId) -> bool {
    w.entity(aircraft).is_some_and(|e| e.alive && e.assigned.is_none())
        && !super::aircombat2026::in_dogfight(w, aircraft)
        && !w.entities.values().any(|e| {
            e.alive && e.projectile.as_ref().is_some_and(|p| {
                p.launched_by == aircraft || p.target == Some(aircraft)
            })
        })
}
/// The first roll precedes the first low-altitude attack. Subsequent rolls use
/// each completed 180-second exposure, before the next attack opportunity.
pub fn safety_step(w: &mut World) {
    if !active(w) {
        return;
    }
    let ids: Vec<_> = w
        .entities
        .values()
        .filter(|e| e.alive && e.kind == crate::types::PlatformKind::Aircraft)
        .map(|e| e.id)
        .collect();
    for id in ids {
        if waterborne::controls_water_motion(w,id){continue;}
        let e = w.entity(id).unwrap().clone();
        let Some(f) = w.rules.aviation.as_ref().and_then(|s| s.flights.get(&id)) else {
            continue;
        };
        if e.altitude_m > 20. || f.spec.engine == Engine::TS {
            w.rules.aviation.as_mut().unwrap().low_exposure.remove(&id);
            continue;
        }
        let micro = w.context.aviation_2026.as_ref().and_then(|c| c.micro_uav.as_ref());
        if micro_uav::exempt_with(w, id, micro) {
            let aviation = w.rules.aviation.as_mut().unwrap();
            aviation.low_exposure.remove(&id);
            if aviation.diagnostics.insert(format!("4.5.3.micro_uav_exempt:{}", id.0)) {
                trace(w, id, "4.5.3.micro_uav_exempt", json!({
                    "source_pdf":34,"qualification":"author_micro+actual_unmanned+live_hitl+daylight+clear_weather"
                }));
            }
            continue;
        }
        let s = w
            .rules
            .aviation
            .as_mut()
            .unwrap()
            .low_exposure
            .entry(id)
            .or_insert(LowExposure {
                next_roll: w.tick,
                turns: 0,
                turn_degrees: 0.,
                last_course: e.course_deg,
                attacked: false,
            });
        s.turn_degrees += crate::geom::angle_diff(s.last_course, e.course_deg).abs();
        s.last_course = e.course_deg;
        if w.tick < s.next_roll {
            continue;
        }
        s.turns += 1;
        let chance = match low_crash_chance(e.speed_kts, s.turns, s.turn_degrees, s.attacked) {
            Ok(p) => p,
            Err(reason) => {
                super::operations::blocked_once(w, id, "4.5.2.low_risk_blocked", &reason);
                continue;
            }
        };
        let mut rng = w.rng.derive(Rng::hash_tag(w.tick, id.0, "4.5.2.low_crash"));
        let roll = rng.next_f64();
        let crashed = roll < chance;
        s.next_roll = w.tick + 180;
        let turns = s.turns;
        let turn = s.turn_degrees;
        let attacked = s.attacked;
        s.turn_degrees = 0.;
        s.attacked = false;
        trace(
            w,
            id,
            "4.5.2.low_crash",
            json!({"chance":chance,"roll_uniform":roll,"crashed":crashed,"consecutive_turns":turns,"turn_degrees":turn,"attacked":attacked,"source_pdf":34}),
        );
        if crashed {
            super::operations::crash_aircraft(w, id);
        }
    }
}
fn loadout(w: &World, class: &str) -> Result<Loadout, String> {
    let l = w
        .context
        .aviation_2026
        .as_ref()
        .and_then(|c| c.loads.get(class))
        .cloned()
        .ok_or_else(|| format!("4.7 explicit cited load mass and tanks required for {class}"))?;
    let s = spec(class)?;
    let mut external_kg = 0.;
    for (weapon, rounds) in &l.external_weapon_rounds {
        let fit =
            w.db.platforms
                .get(class)
                .ok_or("4.7 aircraft fit missing")?;
        let carried: u32 = fit
            .weapons
            .iter()
            .filter(|m| &m.weapon == weapon)
            .map(|m| m.ammo as u32 * m.mounts as u32)
            .sum();
        if *rounds == 0 || *rounds as u32 != carried {
            return Err("9.5.1 external-store declaration must cover every fitted round of that exact weapon; mixed/internal mounts not inferred".into());
        }
        external_kg += store_weight(class, weapon)? * *rounds as f64;
    }
    let mut internal_kg = 0.;
    let internal_fit = &tables()["internal_store_fits"][class];
    if !internal_fit.is_null() {
        let expected: BTreeMap<String, u16> = serde_json::from_value(internal_fit["rounds"].clone())
            .map_err(|_| "4.7 exact internal source inventory missing")?;
        if l.internal_weapon_rounds != expected || !l.external_weapon_rounds.is_empty()
            || l.payload_kg != internal_fit["payload_kg"].as_f64().unwrap()
        {
            return Err("4.7 declared internal load differs from the exact source fit".into());
        }
    } else if !l.internal_weapon_rounds.is_empty() {
        return Err("4.7 internal source fit not verified for this aircraft".into());
    }
    for (weapon, rounds) in &l.internal_weapon_rounds {
        let fit = w.db.platforms.get(class).ok_or("4.7 internal aircraft fit missing")?;
        let carried: u32 = fit.weapons.iter().filter(|m| &m.weapon == weapon)
            .map(|m| m.ammo as u32 * m.mounts as u32).sum();
        if *rounds == 0 || *rounds as u32 != carried {
            return Err("4.7 internal declaration must match physical source inventory".into());
        }
        internal_kg += internal_store_weight(class, weapon)? * *rounds as f64;
    }
    if external_kg + internal_kg + l.tanks.iter().map(|t| t.fuel_weight_kg).sum::<f64>() > l.payload_kg {
        return Err("4.7 declared payload omits external weapon/tank mass".into());
    }
    if l.source.trim().is_empty()
        || !l.payload_kg.is_finite()
        || l.payload_kg < 0.
        || l.payload_kg > s.max_payload_kg
        || l.tanks.iter().any(|t| {
            !finite_positive(t.range_add_nm)
                || !finite_positive(t.fuel_weight_kg)
                || t.source.trim().is_empty()
        })
    {
        return Err("4.7 invalid/uncited loadout or tank capacity".into());
    }
    let allowed = tables()["tank_data"][class]
        .as_array()
        .ok_or("4.7 additional tank data not verified")?;
    if l.tanks.iter().any(|t| {
        !allowed.iter().any(|v| {
            v[0].as_f64() == Some(t.fuel_weight_kg) && v[1].as_f64() == Some(t.range_add_nm)
        })
    }) {
        return Err("4.7 tank weight/range does not match verified Annex B row".into());
    }
    if l.tanks.iter().map(|t| t.fuel_weight_kg).sum::<f64>() > l.payload_kg {
        return Err("4.7 declared payload omits carried tank fuel weight".into());
    }
    let max_tanks = if matches!(class, "us_f18e" | "us_f18e_asm" | "us_f18e_mk82") {
        5
    } else if class == "us_mh60r" {
        4
    } else {
        0
    };
    if l.tanks.len() > max_tanks
        || l.tanks.iter().filter(|t| t.fuel_weight_kg == 920.).count() > 1
        || l.tanks.iter().filter(|t| t.fuel_weight_kg == 1000.).count() > 1
    {
        return Err("4.7 tank count exceeds printed source loadout envelope".into());
    }
    if l.buddy
        && (!matches!(class, "us_f18e" | "us_f18e_asm")
            || !l
                .tanks
                .iter()
                .any(|t| t.fuel_weight_kg == 920. && t.range_add_nm == 180.))
    {
        return Err("4.8 buddy configuration requires verified F/A-18E 300 USG store".into());
    }
    Ok(l)
}
fn internal_store_weight(class: &str, weapon: &str) -> Result<f64, String> {
    tables()["internal_store_fits"][class]["round_weight_kg"][weapon]
        .as_f64().filter(|v| finite_positive(*v))
        .ok_or_else(|| "4.7 exact internal-store hang weight not verified".into())
}
fn store_weight(class: &str, weapon: &str) -> Result<f64, String> {
    let source_fit = &tables()["external_store_fits"][class];
    source_fit
        .get("round_weight_kg")
        .filter(|_| source_fit["weapon"] == weapon)
        .and_then(Value::as_f64)
        .or_else(|| {
            super::aam2026::weapon_profile(weapon)
                .ok()
                .and_then(|p| p["hang_weight_kg"].as_f64())
        })
        .or_else(|| {
            super::aircombat2026::tables()["surface_bombs"][weapon]["hang_weight_kg"].as_f64()
        })
        .or_else(|| super::pgm2026::external_store_weight(class,weapon))
        .filter(|v| finite_positive(*v))
        .ok_or_else(|| "9.5.1 exact external-store hang weight not verified".into())
}
/// Existing declared stores and tanks already occupy the physical Flight's
/// load. Other carried equipment may use only the remaining declared mass.
pub fn remaining_declared_store_mass_kg(w:&World,aircraft:EntityId)->Result<f64,String>{
    let e=w.entity(aircraft).filter(|e|e.alive&&e.kind==crate::types::PlatformKind::Aircraft)
        .ok_or("4.7 actual live aircraft required for carried-mass accounting")?;
    let f=w.rules.aviation.as_ref().and_then(|s|s.flights.get(&aircraft))
        .ok_or("4.7 actual source Flight required for carried-mass accounting")?;
    let l=loadout(w,&e.class)?;
    let mut mass=l.tanks.iter().map(|t|t.fuel_weight_kg).sum::<f64>();
    for (weapon,rounds) in &f.external_weapon_rounds {
        if *rounds>0 {mass+=f64::from(*rounds)*store_weight(&e.class,weapon)?;}
    }
    for (weapon,rounds) in &f.internal_weapon_rounds {
        if *rounds>0 {mass+=f64::from(*rounds)*internal_store_weight(&e.class,weapon)?;}
    }
    let mut actual:BTreeMap<&str,u32>=BTreeMap::new();
    for m in &e.weapons {*actual.entry(&m.weapon).or_default()+=u32::from(m.ammo);}
    for (weapon,rounds) in actual {
        let declared=u32::from(f.external_weapon_rounds.get(weapon).copied().unwrap_or(0))
            +u32::from(f.internal_weapon_rounds.get(weapon).copied().unwrap_or(0));
        if rounds>declared {return Err(format!("4.7 remaining carried mass unresolved for actual mounted {weapon}; source Flight must declare every carried store"));}
    }
    if !mass.is_finite()||mass<0. {return Err("4.7 remaining declared store mass invalid".into());}
    Ok(mass)
}
/// Call once for each successful physical weapon release. Only declared
/// internal and external stores remain distinct; ammo alone never proves carriage.
pub fn ordnance_released(w: &mut World, aircraft: EntityId, weapon: &str, rounds: u16) {
    if !active(w) {
        return;
    }
    let Some(class) = w.entity(aircraft).map(|e| e.class.clone()) else {
        return;
    };
    let internal = w.rules.aviation.as_ref().and_then(|s| s.flights.get(&aircraft))
        .is_some_and(|f| f.internal_weapon_rounds.contains_key(weapon));
    let Ok(weight) = (if internal { internal_store_weight(&class, weapon) } else { store_weight(&class, weapon) }) else {
        return;
    };
    let Some(flight) = w
        .rules
        .aviation
        .as_mut()
        .and_then(|s| s.flights.get_mut(&aircraft))
    else {
        return;
    };
    let Some(remaining) = (if internal { &mut flight.internal_weapon_rounds } else { &mut flight.external_weapon_rounds }).get_mut(weapon) else {
        return;
    };
    let removed = rounds.min(*remaining);
    if removed == 0 {
        return;
    }
    *remaining -= removed;
    flight.payload_fraction =
        (flight.payload_fraction - removed as f64 * weight / flight.spec.max_payload_kg).max(0.);
    let payload_fraction = flight.payload_fraction;
    trace(
        w,
        aircraft,
        if internal {"4.7.5.internal_store_removed"} else {"4.7.5.external_store_removed"},
        json!({"weapon":weapon,"rounds":removed,"weight_each_kg":weight,"payload_fraction":payload_fraction,"internal":internal}),
    );
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let mut s = State::default();
    for e in w.entities.values() {
        if let Some(d) = &e.deck {
            for (class, count) in &d.ready {
                let Some(a) = operations_aircraft(w, class) else {
                    continue;
                };
                for serial in 0..*count {
                    let key = format!("{}:{class}:{serial}", e.id.0);
                    s.airframes.insert(
                        key,
                        Airframe {
                            carrier: e.id,
                            class: class.clone(),
                            size: a["size"].as_u64().unwrap() as usize,
                            rotary: a["rotary"] == true,
                            load: a["service_load"].as_str().unwrap_or("air").into(),
                            ready_since: w.tick,
                            sorties: vec![],
                            aircraft: None,
                            ready_at: None,
                        },
                    );
                }
            }
            if let Some(count) = w
                .context
                .aviation_2026
                .as_ref()
                .and_then(|c| c.parking.get(&e.name))
            {
                s.deck_parking.insert(e.id, *count);
            }
        }
    }
    // Airborne scenario starts need an explicit loadout too. They are not
    // silently initialized from the database's old approximate endurance.
    for e in w
        .entities
        .values()
        .filter(|e| e.alive && e.kind == crate::types::PlatformKind::Aircraft)
    {
        if let Ok(l) = loadout(w, &e.class) {
            let Some(fuel) = w
                .context
                .aviation_2026
                .as_ref()
                .and_then(|c| c.initial_fuel_nm.get(&e.name))
                .copied()
            else {
                continue;
            };
            let mut spec = spec(&e.class)?;
            let capacity =
                spec.cruise_range_nm + l.tanks.iter().map(|t| t.range_add_nm).sum::<f64>();
            if !fuel.is_finite() || fuel < 0. || fuel > capacity {
                return Err(format!(
                    "4.7 invalid initial remaining range for {}",
                    e.name
                ));
            }
            if l.buddy && matches!(e.class.as_str(), "us_f18e" | "us_f18e_asm") {
                spec.tanker_slots = 1;
            }
            s.flights.insert(
                e.id,
                Flight {
                    payload_fraction: l.payload_kg / spec.max_payload_kg,
                    external_weapon_rounds: l.external_weapon_rounds.clone(),
                    internal_weapon_rounds: l.internal_weapon_rounds.clone(),
                    spec,
                    fuel_nm: fuel,
                    capacity_nm: capacity,
                    throttle: Throttle::Cruise,
                    last_tick: w.tick,
                    last_position: e.pos,
                },
            );
        }
    }
    w.rules.aviation = Some(s);
    Ok(())
}
fn sortie_allowed(a: &Airframe, now: i64) -> bool {
    let day = n("ready", "day_s") as i64;
    let sorties: Vec<_> = a
        .sorties
        .iter()
        .filter(|(t, _)| t.div_euclid(day) == now.div_euclid(day))
        .collect();
    let limit = if a.size == 3
        || sorties.iter().any(|(t, end)| {
            end.is_some_and(|end| end - *t > n("ready", "two_sortie_max_duration_s") as i64)
        }) {
        1
    } else {
        2
    };
    sorties.len() < limit
}
pub fn launch_gate(w: &World, carrier: EntityId, class: &str, count: u8) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let s = w
        .rules
        .aviation
        .as_ref()
        .ok_or("4.6 aviation state missing")?;
    loadout(w, class)?;
    if s.deck_parking.get(&carrier).copied().unwrap_or(0) > 0
        || s.respots.get(&carrier).is_some_and(|r| r.due > w.tick)
    {
        return Err("4.6 deck parking obstructs flight operations; respot first".into());
    }
    let available = s
        .airframes
        .values()
        .filter(|a| {
            a.carrier == carrier
                && a.class == class
                && a.aircraft.is_none()
                && a.ready_at.is_none()
                && w.tick - a.ready_since <= n("ready", "max_alert5_s") as i64
                && sortie_allowed(a, w.tick)
        })
        .count();
    if (count == 0 && available == 0)
        || (count > 0
            && available
                < super::operations::reserved(w, carrier, class)
                    + super::ground2026::reserved(w, carrier, class)
                    + count as usize)
    {
        return Err("4.6.7/4.7.7 no unreserved Alert+5 airframes within crew-hour and daily sortie limits; re-alert or wait".into());
    }
    Ok(())
}
pub fn mission_gate(
    w: &World,
    carrier: EntityId,
    class: &str,
    mission: &crate::commands::AirMission,
    profile: Option<&crate::commands::LaunchProfile>,
) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let c = w.entity(carrier).ok_or("missing launch platform")?;
    let l = loadout(w, class)?;
    let s = spec(class)?;
    let end = match mission {
        crate::commands::AirMission::Cap { station, .. } => *station,
        crate::commands::AirMission::Asw { area, .. } => *area,
        crate::commands::AirMission::Recon { point } => *point,
        crate::commands::AirMission::Rtb => c.pos,
        crate::commands::AirMission::Strike { contact } => {
            let target = w.sides[c.side.index()]
                .contacts
                .get(contact)
                .ok_or("4.7.6 mission fuel planning needs a friendly target contact")?;
            if target.bearing_only {
                return Err(
                    "4.7.6 cannot establish a return fuel budget from bearing-only target range"
                        .into(),
                );
            }
            target.estimate_at(w.tick)
        }
    };
    let altitude = profile.map(|p| p.altitude_m).unwrap_or(1000.);
    source_speed(class, altitude, Throttle::Cruise)?;
    let mut previous = c.pos;
    let mut distance = 0.;
    if let Some(p) = profile {
        for point in &p.waypoints {
            distance += previous.range_to(*point);
            previous = *point;
        }
    }
    distance += previous.range_to(end) + end.range_to(c.pos);
    let cost = range_cost(
        s.engine,
        Throttle::Cruise,
        distance,
        0.,
        s.cruise_kts,
        altitude,
        l.payload_kg / s.max_payload_kg,
    )?;
    let capacity = s.cruise_range_nm + l.tanks.iter().map(|t| t.range_add_nm).sum::<f64>();
    if cost >= capacity {
        return Err(format!("4.7.6 pilot refuses mission: route+return requires {cost:.3} cruise-equivalent nmi, available {capacity:.3}; a verified alternate load/tank/refuel plan is required"));
    }
    Ok(())
}
pub fn recovery_gate(w: &World, carrier: EntityId) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let s = w
        .rules
        .aviation
        .as_ref()
        .ok_or("4.6 aviation state missing")?;
    if s.deck_parking.get(&carrier).copied().unwrap_or(0) > 0
        || s.respots.get(&carrier).is_some_and(|r| r.due > w.tick)
    {
        return Err("4.6 deck obstructed by parking/respot".into());
    }
    Ok(())
}
pub fn launch_completed(
    w: &mut World,
    carrier: EntityId,
    class: &str,
    aircraft: EntityId,
) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let e = w
        .entity(aircraft)
        .ok_or("missing launched airframe")?
        .clone();
    let l = loadout(w, class)?;
    let mut spec = spec(class)?;
    if l.buddy && matches!(class, "us_f18e" | "us_f18e_asm") {
        spec.tanker_slots = 1;
    }
    let capacity = spec.cruise_range_nm + l.tanks.iter().map(|t| t.range_add_nm).sum::<f64>();
    let s = w
        .rules
        .aviation
        .as_mut()
        .ok_or("4.6 aviation state missing")?;
    let (key, a) = s
        .airframes
        .iter_mut()
        .find(|(_, a)| {
            a.carrier == carrier
                && a.class == class
                && a.aircraft.is_none()
                && a.ready_at.is_none()
                && w.tick - a.ready_since <= 3600
                && sortie_allowed(a, w.tick)
        })
        .ok_or("4.7.7 no eligible individual airframe for completed launch")?;
    let key = key.clone();
    a.aircraft = Some(aircraft);
    a.sorties.push((w.tick, None));
    s.flights.insert(
        aircraft,
        Flight {
            payload_fraction: l.payload_kg / spec.max_payload_kg,
            external_weapon_rounds: l.external_weapon_rounds.clone(),
            internal_weapon_rounds: l.internal_weapon_rounds.clone(),
            spec,
            fuel_nm: capacity,
            capacity_nm: capacity,
            throttle: Throttle::Cruise,
            last_tick: w.tick,
            last_position: e.pos,
        },
    );
    trace(
        w,
        carrier,
        "4.7.sortie_started",
        json!({"airframe":key,"aircraft":aircraft,"class":class,"fuel_cruise_equivalent_nm":capacity,"load_source":l.source}),
    );
    Ok(())
}
pub fn recovery_completed(w: &mut World, aircraft: EntityId, carrier: EntityId) {
    if !active(w) {
        return;
    }
    let now = w.tick;
    let Some(s) = w.rules.aviation.as_mut() else {
        return;
    };
    s.flights.remove(&aircraft);
    s.hifr.remove(&aircraft);
    s.hifr_used.remove(&aircraft);
    if let Some((key, a)) = s
        .airframes
        .iter_mut()
        .find(|(_, a)| a.aircraft == Some(aircraft))
    {
        let key = key.clone();
        a.aircraft = None;
        a.carrier = carrier;
        if let Some(last) = a.sorties.last_mut() {
            last.1 = Some(now);
        }
        // The production operations scheduler owns the actual fuel/arm/alert
        // due tick; consume its individual cycling entry, not a second timer.
        let due = w
            .entities
            .get(&carrier)
            .and_then(|e| e.deck.as_ref())
            .and_then(|d| {
                d.cycling
                    .iter()
                    .filter(|(class, _, due)| class == &a.class && *due >= now)
                    .map(|(_, _, due)| *due)
                    .max()
            });
        a.ready_at = Some(due.unwrap_or(i64::MAX));
        trace(
            w,
            carrier,
            "4.7.sortie_recovered",
            json!({"airframe":key,"aircraft":aircraft,"ready_at":due,"missing_cycle_blocked":due.is_none()}),
        );
    }
}
/// Consume actual movement distance. Loiter/hover consume time at the source
/// cruise rate. Return an observation budget in seconds for existing UI only.
pub fn fuel_step(w: &mut World, aircraft: EntityId) -> Result<f64, String> {
    let e = w.entity(aircraft).ok_or("missing aircraft")?.clone();
    if waterborne::controls_water_motion(w,aircraft){
        let f=w.rules.aviation.as_mut().and_then(|s|s.flights.get_mut(&aircraft)).ok_or("4.7 declared waterborne aircraft flight data absent")?;
        if w.tick<f.last_tick{return Err("4.7 fuel clock moved backwards".into());}
        f.last_tick=w.tick;f.last_position=e.pos;
        return Ok(f.fuel_nm/f.spec.cruise_kts*3600.);
    }
    let dogfight = super::aircombat2026::in_dogfight(w, aircraft);
    let maneuvering = super::aircombat2026::maneuvering(w, aircraft);
    let cruise_cap = super::expanded_damage2026::aircraft_speed_cap(w, aircraft)?;
    let cruise_limited = cruise_cap.is_some();
    let declared_nominal=w.rules.aviation.as_ref().and_then(|s|s.flights.get(&aircraft)).and_then(|f|{
        let effective=if cruise_limited&&matches!(f.throttle,Throttle::Fmp|Throttle::Reheat){Throttle::Cruise}else{f.throttle};
        waterborne::declared_speed(w,&e.class,e.altitude_m,if maneuvering&&!cruise_limited{Throttle::Fmp}else{effective})
    }).transpose()?;
    let f = w
        .rules
        .aviation
        .as_mut()
        .and_then(|s| s.flights.get_mut(&aircraft))
        .ok_or("4.7 flight fuel/load data not initialized")?;
    if w.tick < f.last_tick {
        return Err("4.7 fuel clock moved backwards".into());
    }
    if w.tick > f.last_tick {
        let elapsed = (w.tick - f.last_tick) as f64;
        let effective_throttle =
            if cruise_limited && matches!(f.throttle, Throttle::Fmp | Throttle::Reheat) {
                Throttle::Cruise
            } else {
                f.throttle
            };
        let nominal = declared_nominal.unwrap_or_else(||source_speed(
            &e.class,
            e.altitude_m,
            if maneuvering && !cruise_limited {
                Throttle::Fmp
            } else {
                effective_throttle
            },
        )
        .unwrap_or(f.spec.cruise_kts))
        .min(cruise_cap.unwrap_or(f64::INFINITY))
            * elapsed
            / 3600.;
        let cost = range_cost(
            f.spec.engine,
            if dogfight && maneuvering && !cruise_limited {
                Throttle::Fmp
            } else {
                effective_throttle
            },
            if dogfight
                && (maneuvering || !matches!(f.throttle, Throttle::Loiter | Throttle::Hover))
            {
                nominal
            } else {
                e.pos.range_to(f.last_position)
            },
            elapsed,
            f.spec.cruise_kts,
            e.altitude_m,
            f.payload_fraction,
        )?;
        f.fuel_nm = (f.fuel_nm - cost).max(0.);
        f.last_tick = w.tick;
        f.last_position = e.pos;
    }
    Ok(f.fuel_nm / f.spec.cruise_kts * 3600.)
}
fn rendezvous(a: &crate::world::Entity, b: &crate::world::Entity) -> bool {
    a.side == b.side
        && a.alive
        && b.alive
        && a.kind == crate::types::PlatformKind::Aircraft
        && b.kind == crate::types::PlatformKind::Aircraft
        && a.altitude_m > 0.
        && b.altitude_m > 0.
        && a.pos.range_to(b.pos) <= 1e-9
        && (a.altitude_m - b.altitude_m).abs() <= 1e-9
        && (a.speed_kts - b.speed_kts).abs() <= 1e-9
        && crate::geom::angle_diff(a.course_deg, b.course_deg).abs() <= 1e-9
}
fn return_cost(w: &World, id: EntityId, f: &Flight) -> Result<f64, String> {
    let a = w.entity(id).ok_or("missing tanker")?;
    let distance = w
        .entities
        .values()
        .filter(|e| {
            e.alive
                && e.side == a.side
                && e.deck.is_some()
                && crate::air::compatible_recovery(w, id, e.id)
        })
        .map(|e| a.pos.range_to(e.pos))
        .fold(f64::INFINITY, f64::min);
    if !distance.is_finite() {
        return Err("4.8.1 no compatible friendly recovery base for tanker return reserve".into());
    }
    range_cost(
        f.spec.engine,
        Throttle::Cruise,
        distance,
        0.,
        f.spec.cruise_kts,
        a.altitude_m,
        f.payload_fraction,
    )
}
pub fn apply(w: &mut World, unit: EntityId, action: &Action) -> Result<(), String> {
    if !active(w) {
        return Err("4.6–4.9 requires explicit hv2026-rts-v4".into());
    }
    match action {
        Action::WaterTakeoff=>return waterborne::declare(w,unit,"takeoff"),
        Action::WaterLand=>return waterborne::declare(w,unit,"land"),
        Action::Jettison { weapon, rounds } => {
            if !super::expanded_damage2026::aircraft_profile(w, unit, 0., false)?.jettison {
                return Err(
                    "14.9 recorded offensive-weapons damage prevents ordnance jettison".into(),
                );
            }
            let e = w
                .entity(unit)
                .filter(|e| e.alive && e.kind == crate::types::PlatformKind::Aircraft)
                .ok_or("9.5.1 own airborne aircraft required")?;
            let flight = w
                .rules
                .aviation
                .as_ref()
                .and_then(|s| s.flights.get(&unit))
                .ok_or("9.5.1 source flight data missing")?;
            let declared = flight
                .external_weapon_rounds
                .get(weapon)
                .copied()
                .unwrap_or(0);
            let carried: u32 = e
                .weapons
                .iter()
                .filter(|m| &m.weapon == weapon)
                .map(|m| m.ammo as u32)
                .sum();
            store_weight(&e.class, weapon)?;
            if *rounds == 0 || *rounds > declared || *rounds as u32 > carried {
                return Err(
                    "9.5.1 only explicitly declared remaining external rounds may be jettisoned"
                        .into(),
                );
            }
            let mut remaining = *rounds;
            for mount in w
                .entity_mut(unit)
                .unwrap()
                .weapons
                .iter_mut()
                .filter(|m| &m.weapon == weapon)
            {
                let removed = remaining.min(mount.ammo);
                mount.ammo -= removed;
                remaining -= removed;
            }
            ordnance_released(w, unit, weapon, *rounds);
            trace(
                w,
                unit,
                "9.5.1.jettison",
                json!({"weapon":weapon,"rounds":rounds,"ordnance_misses":true,"projectiles_spawned":0}),
            );
        }
        Action::Respot => {
            let s = w.rules.aviation.as_ref().ok_or("missing aviation state")?;
            if s.respots.contains_key(&unit) {
                return Err("4.6 respot already in progress".into());
            }
            let parked = s
                .deck_parking
                .get(&unit)
                .copied()
                .ok_or("4.6 explicit initial deck parking declaration required")?;
            if parked % 12 != 0 {
                return Err("4.6 partial-batch respot timing unresolved".into());
            }
            let mut r = w.rng.derive(Rng::hash_tag(w.tick, unit.0, "4.6.respot"));
            let dice: Vec<_> = (0..parked / 12)
                .map(|_| (r.next_f64() * 6.).floor() as u8 + 1)
                .collect();
            let due = w.tick + respot_seconds(parked, &dice)?;
            w.rules.aviation.as_mut().unwrap().respots.insert(
                unit,
                Respot {
                    due,
                    dice: dice.clone(),
                    parked,
                },
            );
            trace(
                w,
                unit,
                "4.6.respot_started",
                json!({"parked":parked,"dice":dice,"due":due,"source_pdf":35}),
            );
        }
        Action::Alert { class, count } => {
            let def =
                operations_aircraft(w, class).ok_or("4.6.7 verified aircraft fit required")?;
            let size = def["size"].as_u64().ok_or("4.6.7 verified size required")? as usize;
            let alert = super::operations::tables()["aviation"]["alert_minutes"][size]
                .as_i64()
                .unwrap()
                * 60
                / if def["rotary"] == true { 2 } else { 1 };
            let s = w.rules.aviation.as_mut().ok_or("missing aviation state")?;
            let keys: Vec<_> = s
                .airframes
                .iter()
                .filter(|(_, a)| {
                    a.carrier == unit
                        && &a.class == class
                        && a.aircraft.is_none()
                        && a.ready_at.is_none()
                        && w.tick - a.ready_since > 3600
                        && sortie_allowed(a, w.tick)
                })
                .map(|(k, _)| k.clone())
                .take(*count as usize)
                .collect();
            if *count == 0 || keys.len() != *count as usize {
                return Err("4.6.7 insufficient unalerted airframes eligible today".into());
            }
            for key in &keys {
                s.airframes.get_mut(key).unwrap().ready_at = Some(w.tick + alert);
            }
            trace(
                w,
                unit,
                "4.6.7.alert_started",
                json!({"airframes":keys,"class":class,"due":w.tick+alert}),
            );
        }
        Action::Throttle { setting } => {
            if waterborne::controls_water_motion(w,unit){return Err("4.6.5 resting aircraft requires a declared water takeoff before flight power".into());}
            let f = w
                .rules
                .aviation
                .as_ref()
                .and_then(|s| s.flights.get(&unit))
                .ok_or("4.7 explicit flight specification required")?;
            let e = w.entity(unit).ok_or("missing aircraft")?;
            range_cost(
                f.spec.engine,
                *setting,
                0.,
                1.,
                f.spec.cruise_kts,
                e.altitude_m,
                f.payload_fraction,
            )?;
            if *setting == Throttle::Hover && (f.spec.engine != Engine::TS || e.altitude_m > 2000.)
            {
                return Err("4.3 hover restricted to eligible helicopters at Low or below".into());
            }
            let speed = source_speed_world(w,&e.class, e.altitude_m, *setting)?;
            if speed > entity_source_speed(w, unit, e.altitude_m, *setting)? {
                return Err("14.9 damaged aircraft cannot use speed above source cruise".into());
            }
            w.entity_mut(unit).unwrap().ordered_speed_kts = speed;
            w.entity_mut(unit).unwrap().ext.insert(
                "flight.throttle".into(),
                match setting {
                    Throttle::Cruise => 0.,
                    Throttle::Fmp => 1.,
                    Throttle::Reheat => 2.,
                    Throttle::Loiter => 3.,
                    Throttle::Hover => 4.,
                },
            );
            w.rules
                .aviation
                .as_mut()
                .unwrap()
                .flights
                .get_mut(&unit)
                .unwrap()
                .throttle = *setting;
            trace(w, unit, "4.7.throttle", json!({"setting":setting}));
        }
        Action::Refuel { receiver, method } => {
            if unit == *receiver {
                return Err("4.8 tanker cannot refuel itself".into());
            }
            let a = w.entity(unit).ok_or("missing tanker")?;
            let b = w.entity(*receiver).ok_or("missing receiver")?;
            if matches!(a.class.as_str(), "us_f18e" | "us_f18e_asm")
                && *method != RefuelMethod::BuddyWestern
            {
                return Err("4.8 F/A-18 buddy store must use the buddy pumping-time column".into());
            }
            if !rendezvous(a, b) {
                return Err("4.8 declared rendezvous requires same friendly location, altitude, course and speed".into());
            }
            let s = w.rules.aviation.as_ref().ok_or("missing aviation state")?;
            let t = s.flights.get(&unit).ok_or("4.8 tanker data missing")?;
            let r = s.flights.get(receiver).ok_or("4.8 receiver data missing")?;
            let encoded_method = serde_json::to_value(method).unwrap();
            if !tables()["tanker_methods"][&a.class]
                .as_array()
                .is_some_and(|methods| methods.contains(&encoded_method))
            {
                return Err("4.8 outgoing refuel equipment/method not verified for this tanker fit".into());
            }
            if !r.spec.receive.is_some_and(|m| compatible(*method, m)) {
                return Err("4.8 incompatible boom/probe/ZA equipment".into());
            }
            let count = s.refuels.iter().filter(|j| j.tanker == unit).count();
            if t.spec.tanker_slots == 0
                || count >= t.spec.tanker_slots as usize
                || s.refuels
                    .iter()
                    .any(|j| j.receiver == *receiver || j.receiver == unit || j.tanker == *receiver)
                || r.spec.size == 3 && (!t.spec.centerline || count > 0)
                || s.refuels
                    .iter()
                    .any(|j| j.tanker == unit && s.flights[&j.receiver].spec.size == 3)
            {
                return Err("4.8 verified refuel slot/centerline capacity unavailable".into());
            }
            let factor = offload_factor(&a.class, &b.class)?;
            let return_nm = return_cost(w, unit, t)?;
            let (take, give) =
                transfer_limits(t.fuel_nm, return_nm, r.capacity_nm - r.fuel_nm, factor)?;
            let due = w.tick + refuel_seconds(r.spec.size, *method)?;
            w.rules.aviation.as_mut().unwrap().refuels.push(RefuelJob {
                tanker: unit,
                receiver: *receiver,
                due,
                receiver_range_nm: give,
                tanker_range_nm: take,
                method: *method,
            });
            trace(
                w,
                unit,
                "4.8.refuel_started",
                json!({"receiver":receiver,"due":due,"factor":factor,"planned_receiver_nm":give}),
            );
        }
        Action::Hifr { ship } => {
            let a = w.entity(unit).ok_or("missing helicopter")?;
            let b = w.entity(*ship).ok_or("missing friendly aviation ship")?;
            let s = w.rules.aviation.as_ref().ok_or("missing aviation state")?;
            let f = s
                .flights
                .get(&unit)
                .ok_or("4.9 helicopter flight data missing")?;
            if f.spec.engine != Engine::TS || !tables()["hifr_capable_airframes"][&a.class]
                .as_str().is_some_and(|source| !source.trim().is_empty())
            {
                return Err("4.9 HIFR capability not verified for this helicopter".into());
            }
            if s.hifr_used.contains(&unit) || s.hifr.contains_key(&unit) {
                return Err("4.9 repeated HIFR fatigue limit not specified; repeat blocked".into());
            }
            if a.side != b.side
                || !b.alive
                || b.kind != crate::types::PlatformKind::Ship
                || b.deck.is_none()
                || a.altitude_m > 20.
                || crate::geom::angle_diff(a.course_deg, b.course_deg).abs() > 1e-9
                || (a.speed_kts - b.speed_kts).abs() > 1e-9
                || a.pos.range_to(b.pos) > 1e-9
                || !hifr_noncombat(w, unit)
                || s.hifr_under_fire_tick.get(&unit) == Some(&w.tick)
            {
                return Err(
                    "4.9 requires noncombat SSkim rendezvous with matching ship course/speed"
                        .into(),
                );
            }
            w.rules
                .aviation
                .as_mut()
                .unwrap()
                .hifr
                .insert(unit, (*ship, w.tick + n("refuel", "hifr_s") as i64));
            trace(
                w,
                unit,
                "4.9.hifr_started",
                json!({"ship":ship,"due":w.tick+900}),
            );
        }
    }
    Ok(())
}
pub fn step(w: &mut World) {
    step_inner(w, false)
}
pub fn post_flight_step(w: &mut World) {
    step_inner(w, true)
}
fn step_inner(w: &mut World, settle_fuel: bool) {
    if !active(w) {
        return;
    }
    let Some(mut s) = w.rules.aviation.take() else {
        return;
    };
    s.hifr_under_fire_tick.retain(|_, tick| *tick == w.tick);
    let completed: Vec<_> = s
        .respots
        .iter()
        .filter(|(_, r)| r.due <= w.tick)
        .map(|(k, _)| *k)
        .collect();
    for id in completed {
        s.respots.remove(&id);
        s.deck_parking.insert(id, 0);
        trace(w, id, "4.6.respot_completed", json!({"parked":0}));
    }
    for (key, a) in &mut s.airframes {
        if a.ready_at.is_some_and(|t| t <= w.tick) {
            a.ready_at = None;
            a.ready_since = w.tick;
            trace(
                w,
                a.carrier,
                "4.6.7.alert_ready",
                json!({"airframe":key,"expires_at":w.tick+3600}),
            );
        }
    }
    let mut jobs = std::mem::take(&mut s.refuels);
    // Connection order is not an allocation priority. Canonical state order
    // also survives saves made before the common completion tick.
    jobs.sort_by_key(|j| (j.tanker, j.due, j.receiver));
    let mut ready: BTreeMap<(EntityId, i64), Vec<RefuelJob>> = BTreeMap::new();
    for j in jobs {
        if !w
            .entity(j.tanker)
            .zip(w.entity(j.receiver))
            .is_some_and(|(a, b)| rendezvous(a, b))
        {
            trace(
                w,
                j.tanker,
                "4.8.refuel_cancelled",
                json!({"receiver":j.receiver,"reason":"rendezvous_interrupted","transferred":0}),
            );
            continue;
        }
        if w.tick < j.due || !settle_fuel {
            s.refuels.push(j);
            continue;
        }
        if !s.flights.contains_key(&j.tanker) || !s.flights.contains_key(&j.receiver) {
            continue;
        }
        ready.entry((j.tanker, j.due)).or_default().push(j);
    }
    for ((tanker, due), batch) in ready {
        let f = &s.flights[&tanker];
        let reserve = match return_cost(w, tanker, f) {
            Ok(value) => value,
            Err(reason) => {
                for j in batch {
                    trace(w, tanker, "4.8.refuel_cancelled", json!({"receiver":j.receiver,"reason":reason}));
                }
                continue;
            }
        };
        let available = (f.fuel_nm - reserve).max(0.);
        let requested: Vec<_> = batch.iter().map(|j| {
            let factor = if j.tanker_range_nm > 0. { j.receiver_range_nm / j.tanker_range_nm } else { 0. };
            let r = &s.flights[&j.receiver];
            let need = (r.capacity_nm - r.fuel_nm).max(0.);
            let take = if factor > 0. { j.tanker_range_nm.min(need / factor).max(0.) } else { 0. };
            (take, factor)
        }).collect();
        let total: f64 = requested.iter().map(|(take, _)| *take).sum();
        // §4.8 supplies the printed offload factors and return reserve. The
        // proportional sharing of deliberately insufficient simultaneous
        // requests is an explicit RTS policy, not an unprinted book constant.
        let allocation = if total > 0. { (available / total).min(1.) } else { 0. };
        let debit: f64 = requested.iter().map(|(take, _)| *take * allocation).sum();
        s.flights.get_mut(&tanker).unwrap().fuel_nm -= debit;
        for (j, (requested_take, factor)) in batch.into_iter().zip(requested) {
            let take = requested_take * allocation;
            let give = take * factor;
            let r = s.flights.get_mut(&j.receiver).unwrap();
            r.fuel_nm = (r.fuel_nm + give).min(r.capacity_nm);
            trace(w, tanker, "4.8.refuel_completed", json!({
                "receiver":j.receiver,"receiver_nm":give,"tanker_nm":take,"due":due,
                "printed_offload_factor":factor,"requested_tanker_nm":requested_take,
                "shared_return_reserve_nm":reserve,"shared_available_nm":available,
                "shared_requested_nm":total,"allocation_fraction":allocation,
                "allocation_policy":tables()["refuel_allocation_policy"]
            }));
        }
    }
    let jobs = std::mem::take(&mut s.hifr);
    for (aircraft, (ship, due)) in jobs {
        let noncombat = hifr_noncombat(w, aircraft)
            && s.hifr_under_fire_tick.get(&aircraft) != Some(&w.tick);
        let valid = noncombat && w
            .entity(aircraft)
            .zip(w.entity(ship))
            .is_some_and(|(a, b)| {
                a.alive
                    && b.alive
                    && b.kind == crate::types::PlatformKind::Ship
                    && b.deck.is_some()
                    && a.side == b.side
                    && a.altitude_m <= 20.
                    && a.pos.range_to(b.pos) <= 1e-9
                    && (a.speed_kts - b.speed_kts).abs() <= 1e-9
                    && crate::geom::angle_diff(a.course_deg, b.course_deg).abs() <= 1e-9
            });
        if !valid {
            trace(w, aircraft, "4.9.hifr_cancelled", json!({"ship":ship,"reason":if noncombat {"rendezvous_interrupted"} else {"combat_interrupted"},"transferred":0}));
            continue;
        }
        if w.tick < due || !settle_fuel {
            s.hifr.insert(aircraft, (ship, due));
            continue;
        }
        if let Some(f) = s.flights.get_mut(&aircraft) {
            f.fuel_nm = f.capacity_nm;
            s.hifr_used.insert(aircraft);
            trace(
                w,
                aircraft,
                "4.9.hifr_completed",
                json!({"ship":ship,"fuel_fraction":1.0}),
            );
        }
    }
    w.rules.aviation = Some(s);
}

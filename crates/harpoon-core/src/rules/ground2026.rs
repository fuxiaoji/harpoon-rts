//! Harpoon V2026 chapter 11: explicitly configured facilities and source tables.
pub mod berth;
pub mod gci;
pub mod drydock;
pub mod ngfs;
pub mod parking;
pub mod aircraft_maintenance;
use crate::{EntityId, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Configuration {
    pub source: String,
    #[serde(default,skip_serializing_if="BTreeMap::is_empty")]
    pub aircraft_maintenance:BTreeMap<String,aircraft_maintenance::Assignment>,
    #[serde(default)]
    pub facilities: BTreeMap<String, FacilityConfig>,
    #[serde(default)]
    pub complexes: BTreeMap<String, ComplexConfig>,
    #[serde(default)]
    pub missiles: BTreeMap<String, MissileSpec>,
    #[serde(default)]
    pub air_services: BTreeMap<String, AirService>,
    #[serde(default)]
    pub observers: BTreeMap<String, ObserverConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub berth_depots: BTreeMap<String, berth::DepotConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub berth_services: BTreeMap<String, berth::ServiceConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gci_services: BTreeMap<String, gci::ServiceConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub drydocks: BTreeMap<String, drydock::Configuration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ngfs: Option<ngfs::Configuration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parking: Option<parking::Configuration>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FacilityConfig {
    pub template: Option<String>,
    pub custom: Option<Target>,
    pub source: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub runway: Option<RunwayConfig>,
    #[serde(default)]
    pub taxiway: Option<TaxiwayConfig>,
    #[serde(default)]
    pub stored_aircraft: Vec<String>,
    #[serde(default)]
    pub stored_airframes: Vec<String>,
    #[serde(default)]
    pub repair_rate_factor: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Target {
    pub size: char,
    pub damage_points: Option<f64>,
    pub vertical_armor: f64,
    pub horizontal_armor: f64,
    pub effect: String,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunwayConfig {
    pub length_m: u32,
    pub baseline: RunwayClass,
    #[serde(default)]
    pub repair: Option<RepairSpec>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaxiwayConfig {
    pub isolated_parking: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepairSpec {
    pub seconds_per_cut: i64,
    pub teams: u32,
    pub source: String,
    /// Explicit scenario service assignment, never inferred from a shared base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance_provider: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComplexConfig {
    pub kind: ComplexKind,
    pub components: BTreeMap<String, Vec<String>>,
    pub source: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ComplexKind {
    Airbase,
    Port,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RunwayClass {
    STOL,
    Small,
    Medium,
    Large,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MissileFamily {
    Ballistic,
    Lacm,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MissileSpec {
    pub family: MissileFamily,
    pub guidance: String,
    pub satellite: bool,
    pub nuclear: bool,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AirService {
    pub source: String,
    pub launch_lanes: u8,
    pub landing_lanes: u8,
    pub launch_interval_s: i64,
    pub landing_interval_s: i64,
    pub heading_deg: f64,
    pub initial_speed_kts: BTreeMap<String, f64>,
    pub landing_speed_kts: BTreeMap<String, f64>,
    pub turnaround_s: BTreeMap<String, i64>,
    #[serde(default)]
    pub stol: BTreeSet<String>,
    #[serde(default)]
    pub lightly_loaded: BTreeSet<String>,
    pub landing_min_altitude_m: f64,
    pub landing_max_altitude_m: f64,
    pub landing_speed_tolerance_kts: f64,
    pub landing_heading_tolerance_deg: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObserverConfig {
    pub radius_nm: f64,
    pub beacon: bool,
    pub source: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub aircraft_maintenance:Option<aircraft_maintenance::State>,
    pub facilities: BTreeMap<EntityId, Facility>,
    pub complexes: BTreeMap<EntityId, Complex>,
    pub missiles: BTreeMap<String, MissileSpec>,
    pub resolved: BTreeSet<String>,
    pub diagnostics: BTreeSet<String>,
    pub repair_jobs: Vec<RepairJob>,
    pub spotting: BTreeMap<EntityId, SpottingState>,
    pub air_services: BTreeMap<EntityId, AirService>,
    pub launches: Vec<LaunchJob>,
    pub lanes: BTreeMap<String, i64>,
    /// Physical release clocks are distinct from future queue reservations.
    /// A closure may collapse scheduled due times but cannot collapse spacing.
    pub launch_not_before: BTreeMap<String, i64>,
    pub observers: BTreeMap<EntityId, ObserverConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub berth: Option<berth::State>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gci: Option<gci::State>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drydocks: Option<drydock::State>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ngfs: Option<ngfs::State>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchJob {
    pub base: EntityId,
    pub class: String,
    pub mission: crate::commands::AirMission,
    pub profile: Option<crate::commands::LaunchProfile>,
    pub due: i64,
    pub lane: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Facility {
    pub target: Target,
    pub config: FacilityConfig,
    pub parent: Option<EntityId>,
    pub damage: f64,
    pub disabled: bool,
    pub destroyed: bool,
    pub critical_hits: u32,
    pub cuts: u32,
    pub gun_only_until: Option<i64>,
    pub pending_aircraft_loss: Vec<(u8, u8)>,
    pub loss_count: u32,
}
/// A live individual airframe explicitly stored at a facility. Altitude zero
/// alone cannot distinguish a parked aircraft from an unresolved flight state.
pub fn is_parked_aircraft(w: &World, id: EntityId) -> bool {
    let Some(e) = w.entity(id).filter(|e| {
        e.alive
            && e.kind == crate::types::PlatformKind::Aircraft
            && e.altitude_m == 0.
            && e.speed_kts == 0.
    }) else {
        return false;
    };
    w.rules.ground.as_ref().is_some_and(|s| {
        s.facilities.iter().any(|(facility, f)| {
            !f.destroyed
                && w.entity(*facility)
                    .is_some_and(|p| p.alive && p.side == e.side)
                && f.config.stored_aircraft.iter().any(|n| n == &e.name)
        })
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Complex {
    pub kind: ComplexKind,
    pub components: BTreeMap<String, Vec<EntityId>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepairJob {
    pub runway: EntityId,
    pub due: i64,
    pub owner: crate::Side,
    /// Absent for historical/unbound jobs: retain their original due/JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work: Option<RepairWork>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepairWork {
    pub provider: EntityId,
    pub required_seconds: i64,
    pub completed_seconds: f64,
    pub last_tick: i64,
    pub last_rate: Option<f64>,
    pub pending: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpottingState {
    pub target: EntityId,
    pub method: Spotting,
    pub started: i64,
    pub last_fire: Option<i64>,
    pub beacon_ready: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Spotting {
    Ship,
    Aerial { aircraft: EntityId },
    Ground { observer: EntityId },
    Beacon { observer: EntityId },
    Map,
    Blind,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    GciVector { service: String, aircraft: EntityId, course_deg: f64 },
    PumpDock { service: String, flooded: bool },
    RepairRunway {
        runway: EntityId,
    },
    Spot {
        contact: crate::ContactId,
        method: Spotting,
    },
    AlongsideResupply {
        service: String,
        rounds: u16,
    },
    CancelAlongsideResupply {
        service: String,
    },
}
pub fn tables() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/ground-2026.json")).unwrap()
    })
}
pub fn active(w: &World) -> bool {
    super::aviation2026::active(w)
}
pub fn template(name: &str) -> Result<Target, String> {
    let t = tables()["templates"]
        .get(name)
        .ok_or_else(|| format!("11.1 source generic template missing: {name}"))?;
    Ok(Target {
        size: t["size"].as_str().unwrap().chars().next().unwrap(),
        damage_points: t["dp"].as_f64(),
        vertical_armor: t["armor"][0].as_f64().unwrap(),
        horizontal_armor: t["armor"][1].as_f64().unwrap(),
        effect: t["effect"].as_str().unwrap().into(),
        source: "English2026 PDF118 / 11-3 Generic Land Targets".into(),
    })
}
pub fn missile_probability(spec: &MissileSpec, size: char) -> Result<f64, String> {
    if spec.source.trim().is_empty() {
        return Err("11.2 missile guidance binding requires a source".into());
    }
    let col = tables()["size_columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|v| v.as_str().unwrap() == size.to_string())
        .ok_or("11.2 size class must be A–H")?;
    let rows = tables()["guidance_rows"].as_array().unwrap();
    let mut row = rows
        .iter()
        .position(|v| v.as_str() == Some(spec.guidance.as_str()))
        .ok_or("11.2 guidance row not verified")?;
    if spec.satellite
        && !tables()["satnav_already_assumed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some(spec.guidance.as_str()))
    {
        row += 1;
    }
    if spec.nuclear {
        return Ok(1.);
    }
    Ok(tables()[match spec.family {
        MissileFamily::Ballistic => "ballistic",
        MissileFamily::Lacm => "lacm",
    }][row][col]
        .as_f64()
        .unwrap())
}
pub fn runway_capacity(cuts: u32, length_m: u32) -> Result<Option<RunwayClass>, String> {
    if !(1..=10).contains(&cuts) {
        return Err(
            "11.5.1 use original declared capacity at zero cuts; >10 cuts is outside printed table"
                .into(),
        );
    }
    let col = tables()["runway_lengths_m"]
        .as_array()
        .unwrap()
        .iter()
        .position(|v| v.as_u64() == Some(length_m as u64))
        .ok_or("11.5.1 runway length is between source columns; no interpolation guessed")?;
    match tables()["runway_use"][cuts as usize - 1][col]
        .as_str()
        .unwrap()
    {
        "X" => Ok(None),
        "STOL" => Ok(Some(RunwayClass::STOL)),
        "Small" => Ok(Some(RunwayClass::Small)),
        "Medium" => Ok(Some(RunwayClass::Medium)),
        "Large" => Ok(Some(RunwayClass::Large)),
        _ => unreachable!(),
    }
}
pub fn runway_salvo(
    bombs: u16,
    level: bool,
    iron: bool,
    altitude_m: f64,
    retarded: bool,
) -> Result<u16, String> {
    if bombs == 0 || !altitude_m.is_finite() || altitude_m < 0. {
        return Err("11.5.1 invalid runway attack".into());
    }
    let source = &tables()["runway_bomb_attack"];
    if retarded && !source["retarded_can_penetrate"].as_bool().unwrap() {
        return Err("11.5.1 retarded/high-drag bombs cannot penetrate concrete runways".into());
    }
    Ok(if level && iron && altitude_m > source["level_iron_above_m"].as_f64().unwrap() {
        (bombs / source["bombs_per_salvo_factor"].as_u64().unwrap() as u16)
            .max(source["minimum_salvo"].as_u64().unwrap() as u16)
    } else {
        bombs
    })
}
pub fn reverse_component(kind: ComplexKind, d100: u8) -> Result<Option<String>, String> {
    if !(1..=100).contains(&d100) {
        return Err("11.6/11.7 explicit D100 required".into());
    }
    let rows = &tables()[if kind == ComplexKind::Airbase {
        "reverse_airbase"
    } else {
        "reverse_port"
    }];
    let value = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|v| d100 as u64 <= v[0].as_u64().unwrap())
        .unwrap()[1]
        .as_str()
        .unwrap();
    Ok((value != "empty").then(|| value.into()))
}
fn find_named(w: &World, name: &str) -> Result<EntityId, String> {
    let matches: Vec<_> = w.entities.values().filter(|e| e.name == name).collect();
    if matches.len() != 1 {
        Err(format!("11 configuration unit name must be unique: {name}"))
    } else {
        Ok(matches[0].id)
    }
}
fn validate_target(t: &Target) -> Result<(), String> {
    if !("ABCDEFGH".contains(t.size))
        || !t.vertical_armor.is_finite()
        || !t.horizontal_armor.is_finite()
        || t.vertical_armor < 0.
        || t.horizontal_armor < 0.
        || t.damage_points.is_some_and(|d| !d.is_finite() || d <= 0.)
        || t.source.trim().is_empty()
    {
        Err("11.1 explicit source size/armor/DP required".into())
    } else {
        Ok(())
    }
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let mut state = State::default();
    if let Some(c) = w.context.ground_2026.clone() {
        if c.source.trim().is_empty() {
            return Err(
                "11 scenario facility configuration requires source/assumption label".into(),
            );
        }
        state.missiles = c.missiles;
        for (name, f) in c.facilities {
            let id = find_named(w, &name)?;
            let target =
                match (&f.template, &f.custom) {
                    (Some(t), None) => template(t)?,
                    (None, Some(t)) => t.clone(),
                    _ => return Err(
                        "11 use exactly one source template or explicit scenario characteristics"
                            .into(),
                    ),
                };
            validate_target(&target)?;
            if f.source.trim().is_empty() {
                return Err("11.1 facility assignment requires explicit scenario source".into());
            }
            if let Some(r) = &f.runway {
                if target.effect != "runway_cut" {
                    return Err("11.5 runway metadata must bind a runway target".into());
                }
                runway_capacity(1, r.length_m)?;
                if let Some(repair) = &r.repair {
                    if repair.teams == 0
                        || repair.seconds_per_cut <= 0
                        || repair.source.trim().is_empty()
                    {
                        return Err(
                            "11.5 repair resources/time require explicit valid scenario input"
                                .into(),
                        );
                    }
                }
            }
            let parent = f.parent.as_ref().map(|n| find_named(w, n)).transpose()?;
            for name in f
                .stored_aircraft
                .iter()
                .chain(f.taxiway.iter().flat_map(|t| t.isolated_parking.iter()))
            {
                find_named(w, name)?;
            }
            if f.repair_rate_factor
                .is_some_and(|x| !x.is_finite() || x <= 0. || x >= 1.)
            {
                return Err(
                    "11 reduced repair factor must be an explicit value between 0 and 1".into(),
                );
            }
            if let Some(dp) = target.damage_points {
                let e = w.entity_mut(id).unwrap();
                e.hp_max = dp;
                e.hp = dp;
            }
            state.facilities.insert(
                id,
                Facility {
                    target,
                    config: f,
                    parent,
                    damage: 0.,
                    disabled: false,
                    destroyed: false,
                    critical_hits: 0,
                    cuts: 0,
                    gun_only_until: None,
                    pending_aircraft_loss: vec![],
                    loss_count: 0,
                },
            );
        }
        for (runway, facility) in &state.facilities {
            if let Some(name) = facility
                .config
                .runway
                .as_ref()
                .and_then(|r| r.repair.as_ref())
                .and_then(|r| r.maintenance_provider.as_deref())
            {
                let owner = w.entity(*runway).ok_or("11 runway absent")?.side;
                maintenance_provider(w, &state, owner, name)?;
            }
        }
        let mut assigned_frames = BTreeSet::new();
        let mut assigned_aircraft = BTreeSet::new();
        for (facility, f) in &state.facilities {
            let owner = w.entity(*facility).ok_or("11 facility absent")?.side;
            for name in &f.config.stored_aircraft {
                let aircraft = find_named(w, name)?;
                if !assigned_aircraft.insert(aircraft)
                    || !w.entity(aircraft).is_some_and(|a| {
                        a.alive
                            && a.side == owner
                            && a.kind == crate::types::PlatformKind::Aircraft
                            && a.speed_kts == 0.
                            && a.altitude_m == 0.
                    })
                {
                    return Err(
                        "11 stored aircraft must be actual own parked aircraft assigned once"
                            .into(),
                    );
                }
            }
            for key in &f.config.stored_airframes {
                if !assigned_frames.insert(key.clone())
                    || !w.rules.aviation.as_ref().is_some_and(|s| {
                        s.airframes.get(key).is_some_and(|a| {
                            a.aircraft.is_none()
                                && w.entity(a.carrier).is_some_and(|c| c.side == owner)
                        })
                    })
                {
                    return Err("11 each stored individual airframe must exist and be assigned to only one facility".into());
                }
            }
        }
        for (name, c) in c.complexes {
            let id = find_named(w, &name)?;
            let mut components = BTreeMap::new();
            for (key, names) in c.components {
                reverse_component(c.kind, 1)?;
                let ids = names
                    .iter()
                    .map(|n| find_named(w, n))
                    .collect::<Result<Vec<_>, _>>()?;
                if ids.iter().any(|id| !state.facilities.contains_key(id)) {
                    return Err("11 reverse-hit components require individually configured facility targets".into());
                }
                components.insert(key, ids);
            }
            state.complexes.insert(
                id,
                Complex {
                    kind: c.kind,
                    components,
                },
            );
        }
        for (name, s) in c.air_services {
            let id = find_named(w, &name)?;
            if s.source.trim().is_empty()
                || s.launch_lanes == 0
                || s.landing_lanes == 0
                || s.launch_interval_s <= 0
                || s.landing_interval_s <= 0
                || !s.heading_deg.is_finite()
                || s.landing_min_altitude_m < 0.
                || s.landing_max_altitude_m < s.landing_min_altitude_m
                || s.landing_speed_tolerance_kts < 0.
                || s.landing_heading_tolerance_deg < 0.
            {
                return Err(
                    "11 airfield RTS servicing parameters require explicit valid author input"
                        .into(),
                );
            }
            state.air_services.insert(id, s);
        }
        for (name, o) in c.observers {
            if !o.radius_nm.is_finite() || o.radius_nm < 0. || o.source.trim().is_empty() {
                return Err("11.4 observer capabilities must be explicit scenario data".into());
            }
            state.observers.insert(find_named(w, &name)?, o);
        }
        state.berth = berth::initialize(w, &state, &c.berth_depots, &c.berth_services)?;
        state.gci = gci::initialize(w, &state, &c.gci_services)?;
        state.drydocks = drydock::initialize(w, &state, &c.drydocks)?;
    }
    w.rules.ground = Some(state);
    if let Some(config) = w.context.ground_2026.as_ref().and_then(|c| c.ngfs.clone()) {
        let state = ngfs::initialize(w, &config)?;
        w.rules.ground.as_mut().unwrap().ngfs = Some(state);
    }
    parking::initialize(w)?;
    aircraft_maintenance::initialize(w)?;
    Ok(())
}
pub fn source_target(w: &World, id: EntityId) -> Result<Target, String> {
    w.rules
        .ground
        .as_ref()
        .and_then(|s| s.facilities.get(&id))
        .map(|f| f.target.clone())
        .ok_or(
            "11.1 facility size/armor/DP not configured; generic Base HP is not a source value"
                .into(),
        )
}
pub fn source_missile(w: &World, weapon: &str) -> Result<MissileSpec, String> {
    w.rules
        .ground
        .as_ref()
        .and_then(|s| s.missiles.get(weapon))
        .cloned()
        .ok_or_else(|| format!("11.2 source guidance family/row missing for {weapon}"))
}
pub fn terminal_probability(w: &World, weapon: &str, target: EntityId) -> Result<f64, String> {
    missile_probability(&source_missile(w, weapon)?, source_target(w, target)?.size)
}
fn roll(w: &World, id: EntityId, purpose: &str, sides: u8) -> u8 {
    let mut r = w.rng.derive(crate::Rng::hash_tag(w.tick, id.0, purpose));
    (r.next_f64() * sides as f64).floor() as u8 + 1
}
fn trace(w: &mut World, id: EntityId, rule: &str, detail: Value) {
    super::aviation2026::trace(w, id, rule, detail)
}
pub fn apply_damage(
    w: &mut World,
    target: EntityId,
    dp: f64,
    critical_count: u8,
    identity: &str,
) -> Result<(), String> {
    if !active(w) {
        return Err("11 source facilities require hv2026-rts-v4".into());
    }
    if !dp.is_finite() || dp < 0. {
        return Err("11 invalid penetrated damage".into());
    }
    let state = w
        .rules
        .ground
        .as_ref()
        .ok_or("11.1 facility characteristics unresolved")?;
    if !state.facilities.contains_key(&target) || w.entity(target).is_none() {
        return Err("11.1 facility characteristics unresolved".into());
    }
    let key = format!("{}:{identity}", target.0);
    if state.resolved.contains(&key) {
        return Ok(());
    }
    // Credit the interval before this casualty at the previous authoritative
    // availability, then apply its new rate only to subsequent elapsed time.
    settle_bound_repairs(w);
    aircraft_maintenance::step(w);
    let before = w.rules.ground.as_ref().unwrap().facilities[&target].clone();
    let mut f = before.clone();
    f.damage += dp;
    f.critical_hits += critical_count as u32;
    f.destroyed = f.target.damage_points.is_some_and(|d| f.damage >= d)
        || f.target.effect == "destroyed" && dp > 0.;
    f.disabled |= critical_count > 0
        || f.target.damage_points.is_some_and(|d| f.damage >= d * 0.5)
        || f.target.damage_points.is_none() && dp > 0.;
    if let Some(total) = f.target.damage_points {
        let e = w.entity_mut(target).ok_or("missing configured facility")?;
        e.hp = (total - f.damage).max(0.);
        e.hp_max = total;
    }
    let zero_dp_hangar = dp == 0.
        && f.target.horizontal_armor == 0. && f.target.vertical_armor == 0.
        && f.config.template.as_deref().is_some_and(|template|tables()["zero_dp_critical_policy"]["eligible_hangar_templates"].as_array().is_some_and(|rows|rows.iter().any(|row|row.as_str()==Some(template))));
    if zero_dp_hangar && critical_count>1 {
        trace(w,target,"11.1.zero_dp_hangar_aircraft_loss_pending",json!({"identity":identity,"critical_count":critical_count,"source":tables()["zero_dp_critical_policy"]["source"],"reason":"multiple-critical aircraft-loss multiplicity unresolved; functional casualty retained without invented repeated loss"}));
    }
    let special = (dp > 0. || zero_dp_hangar && critical_count == 1)
        && (critical_count > 0
            || f.destroyed
            || f.target.damage_points.is_none()
            || f.target.effect == "runway_cut");
    let mut losses = Vec::new();
    if special {
        match f.target.effect.as_str() {
            "runway_cut" => f.cuts += 1,
            "gun_ammo_only_d10_turns" => {
                let die = roll(w, target, &format!("11.magazine:{identity}"), 10);
                let due=w.tick + die as i64 * 180;
                f.gun_only_until = Some(f.gun_only_until.map_or(due,|old|old.max(due)));
                trace(
                    w,
                    target,
                    "11.1.magazine",
                    json!({"d10":die,"until":f.gun_only_until}),
                );
            }
            "aircraft_one" => losses.push((1, 1)),
            "aircraft_all" => losses.push((
                (f.config.stored_aircraft.len() + f.config.stored_airframes.len()).min(255) as u8,
                1,
            )),
            "aircraft_d6" | "aircraft_d6_div2" | "aircraft_d6_div3" => {
                let die = roll(w, target, &format!("11.aircraft:{identity}"), 6);
                let divisor = if f.target.effect.ends_with("div2") {
                    2
                } else if f.target.effect.ends_with("div3") {
                    3
                } else {
                    1
                };
                if die % divisor == 0 {
                    losses.push((die, divisor));
                } else {
                    f.pending_aircraft_loss.push((die, divisor));
                    trace(
                        w,
                        target,
                        "11.1.aircraft_loss_pending",
                        json!({"d6":die,"divisor":divisor,"reason":"fractional aircraft-loss rounding not specified; physical damage and functional loss still applied"}),
                    );
                }
            }
            _ => {}
        }
    }
    for (numerator, denominator) in losses {
        let mut remaining = (numerator / denominator) as usize;
        for name in &f.config.stored_aircraft {
            if remaining == 0 {
                break;
            }
            let id = find_named(w, name)?;
            if w.entity(id).is_some_and(|e| e.alive) {
                crate::damage::sink(w, id);
                remaining -= 1;
                f.loss_count += 1;
            }
        }
        for key in &f.config.stored_airframes {
            if remaining == 0 {
                break;
            }
            let a = w
                .rules
                .aviation
                .as_ref()
                .and_then(|s| s.airframes.get(key))
                .cloned();
            let Some(a) = a.filter(|a| a.aircraft.is_none()) else {
                continue;
            };
            w.rules.aviation.as_mut().unwrap().airframes.remove(key);
            let carrier = w
                .entity_mut(a.carrier)
                .ok_or("11 parked airframe home missing")?;
            if let Some(deck) = carrier.deck.as_mut() {
                if let Some(at) = a.ready_at {
                    if let Some((_, n, _)) = deck
                        .cycling
                        .iter_mut()
                        .find(|(class, _, due)| class == &a.class && *due == at)
                    {
                        *n = n.saturating_sub(1);
                    }
                } else if let Some((_, n)) =
                    deck.ready.iter_mut().find(|(class, _)| class == &a.class)
                {
                    *n = n.saturating_sub(1);
                }
            }
            let side = carrier.side;
            w.losses[side.index()] += 1;
            w.losses_by_kind.entry("aircraft".into()).or_default()[side.index()] += 1;
            remaining -= 1;
            f.loss_count += 1;
            trace(
                w,
                target,
                "11.1.parked_airframe_lost",
                json!({"airframe":key,"class":a.class,"count":1}),
            );
        }
    }
    if f.disabled {
        let e = w.entity_mut(target).ok_or("facility missing")?;
        e.ext.insert("11.disabled".into(), 1.);
        if matches!(f.target.effect.as_str(), "gun_lost" | "launcher_destroyed") {
            e.systems.fire_control = 0.;
            e.fire_on = false;
        }
        if f.target.effect == "radar_destroyed" {
            e.systems.radar = 0.;
            e.sensors.clear();
        }
    }
    if f.destroyed {
        let e = w.entity_mut(target).unwrap();
        e.hp = 0.;
        e.ext.insert("11.rubble".into(), 1.);
        e.fire_on = false;
        e.systems.fire_control = 0.;
        e.systems.radar = 0.;
    }
    w.rules.ground.as_mut().unwrap().resolved.insert(key);
    w.rules
        .ground
        .as_mut()
        .unwrap()
        .facilities
        .insert(target, f.clone());
    trace(
        w,
        target,
        "11.1.damage",
        json!({"identity":identity,"penetrated_dp":dp,"critical_hits":critical_count,"damage":f.damage,"disabled":f.disabled,"rubble":f.destroyed,"cuts":f.cuts}),
    );
    settle_bound_repairs(w);
    Ok(())
}
pub fn operational(w: &World, id: EntityId) -> bool {
    w.rules
        .ground
        .as_ref()
        .and_then(|s| s.facilities.get(&id))
        .is_none_or(|f| !f.disabled && !f.destroyed)
}
fn components<'a>(w: &'a World, base: EntityId) -> impl Iterator<Item = &'a Facility> {
    w.rules
        .ground
        .iter()
        .flat_map(|s| s.facilities.values())
        .filter(move |f| f.parent == Some(base))
}
pub fn gci_available(w: &World, base: EntityId) -> bool {
    components(w, base).all(|f| f.target.effect != "no_gci" || !f.disabled)
}
pub fn resupply_allowed(w: &World, id: EntityId, gun_ammo: bool) -> bool {
    if !operational(w, id) {
        return false;
    }
    !components(w, id).any(|f| {
        f.disabled && f.target.effect == "no_alongside_resupply"
            || !gun_ammo && f.gun_only_until.is_some_and(|until| until > w.tick)
    })
}
fn maintenance_provider(
    w: &World,
    state: &State,
    owner: crate::Side,
    name: &str,
) -> Result<EntityId, String> {
    if name.trim().is_empty() {
        return Err("11.5 maintenance provider name must be explicit".into());
    }
    let matches: Vec<_> = w.entities.values().filter(|e| e.name == name).collect();
    if matches.len() != 1 {
        return Err("11.5 maintenance provider must identify one actual facility".into());
    }
    let entity = matches[0];
    if entity.side != owner {
        return Err("11.5 maintenance provider must belong to the runway's side".into());
    }
    let facility = state
        .facilities
        .get(&entity.id)
        .ok_or("11.5 maintenance provider requires explicit facility characteristics")?;
    if facility.target.effect != "repair_reduced_scenario" {
        return Err(
            "11.5 maintenance provider must be an explicitly configured repair facility".into(),
        );
    }
    Ok(entity.id)
}
fn maintenance_rate(w: &World, provider: EntityId, owner: crate::Side) -> Result<f64, String> {
    let entity = w
        .entity(provider)
        .filter(|e| e.side == owner)
        .ok_or("11.5 bound maintenance provider unavailable")?;
    let facility = w
        .rules
        .ground
        .as_ref()
        .and_then(|s| s.facilities.get(&provider))
        .filter(|f| f.target.effect == "repair_reduced_scenario")
        .ok_or("11.5 bound maintenance facility characteristics unavailable")?;
    if facility.destroyed || !entity.alive {
        return Err(
            "11.1 bound maintenance provider destroyed or unavailable; work pending".into(),
        );
    }
    if !facility.disabled {
        return Ok(1.);
    }
    facility.config.repair_rate_factor
        .filter(|rate| rate.is_finite() && *rate > 0. && *rate < 1.)
        .ok_or_else(|| "11.5 disabled bound maintenance provider has no declared reduced repair rate; work pending".into())
}
fn has_bound_repair(w: &World, runway: EntityId) -> bool {
    w.rules
        .ground
        .as_ref()
        .and_then(|s| s.facilities.get(&runway))
        .and_then(|f| f.config.runway.as_ref())
        .and_then(|r| r.repair.as_ref())
        .is_some_and(|r| r.maintenance_provider.is_some())
}
fn repair_job_view(job: &RepairJob) -> Value {
    if let Some(work) = &job.work {
        // The historical due is not an execution deadline for a bound job.
        // Unknown current rate gives no invented completion time to the owner.
        let due = work
            .last_rate
            .filter(|r| r.is_finite() && *r > 0.)
            .filter(|_| work.pending.is_none())
            .map(|rate| {
                work.last_tick.saturating_add(
                    ((work.required_seconds as f64 - work.completed_seconds).max(0.) / rate).ceil()
                        as i64,
                )
            });
        json!({"runway":job.runway,"due":due,"owner":job.owner,"work":work})
    } else {
        json!(job)
    }
}
/// Settle elapsed work once. Called before a facility casualty changes its
/// availability and by the normal tick. Current authoritative provider state
/// qualifies that interval; no declaration/click can add work at the same tick.
fn settle_bound_repairs(w: &mut World) {
    if !active(w)
        || !w
            .rules
            .ground
            .as_ref()
            .is_some_and(|s| s.repair_jobs.iter().any(|j| j.work.is_some()))
    {
        return;
    }
    let now = w.tick;
    let jobs = std::mem::take(&mut w.rules.ground.as_mut().unwrap().repair_jobs);
    let mut remaining = Vec::with_capacity(jobs.len());
    let mut events = Vec::new();
    for mut job in jobs {
        let Some(work) = job.work.as_mut() else {
            remaining.push(job);
            continue;
        };
        let runway = w.rules.ground.as_ref().unwrap().facilities.get(&job.runway);
        if runway.is_some_and(|f| f.destroyed || f.cuts == 0) {
            events.push((
                job.runway,
                "11.5.repair_cancelled",
                json!({"reason":"runway rubble or no remaining cut","provider":work.provider}),
            ));
            continue;
        }
        let mut rate = maintenance_rate(w, work.provider, job.owner);
        if runway.is_none()
            || !w
                .entity(job.runway)
                .is_some_and(|e| e.side == job.owner && e.alive)
        {
            rate = Err("11.5 bound repair runway unavailable; work pending".into());
        }
        if work.required_seconds <= 0
            || !work.completed_seconds.is_finite()
            || work.completed_seconds < 0.
            || now < work.last_tick
        {
            rate = Err("11.5 bound repair work ledger invalid; work pending".into());
        }
        let old_rate = work.last_rate;
        let old_pending = work.pending.clone();
        if let Ok(value) = rate.as_ref() {
            let elapsed = now.saturating_sub(work.last_tick).max(0);
            work.completed_seconds = (work.completed_seconds + elapsed as f64 * *value)
                .min(work.required_seconds as f64);
        }
        // A paused interval is consumed as zero work, not banked for recovery.
        if now >= work.last_tick {
            work.last_tick = now;
        }
        work.last_rate = rate.as_ref().ok().copied();
        work.pending = rate.err();
        if old_rate != work.last_rate || old_pending != work.pending {
            events.push((job.runway,"11.5.repair_rate",json!({"provider":work.provider,"rate":work.last_rate,"pending":work.pending,"completed_work_seconds":work.completed_seconds,"required_work_seconds":work.required_seconds})));
        }
        if work.pending.is_none() && work.completed_seconds >= work.required_seconds as f64 {
            let facility = w
                .rules
                .ground
                .as_mut()
                .unwrap()
                .facilities
                .get_mut(&job.runway)
                .unwrap();
            facility.cuts -= 1;
            events.push((job.runway,"11.5.cut_repaired",json!({"at":now,"provider":work.provider,"completed_work_seconds":work.completed_seconds,"required_work_seconds":work.required_seconds,"structural_hp_repaired":0})));
        } else {
            remaining.push(job);
        }
    }
    w.rules.ground.as_mut().unwrap().repair_jobs = remaining;
    for (runway, rule, detail) in events {
        trace(w, runway, rule, detail);
    }
}
pub fn flight_rate_factor(w: &World, base: EntityId, landing: bool) -> Result<f64, String> {
    let vis = w
        .context
        .aviation_2026
        .as_ref()
        .and_then(|c| c.visibility_percent);
    let mut half = false;
    for f in components(w, base) {
        if f.disabled {
            if f.target.effect == "flight_rate_half" {
                half = true;
            }
            if landing && f.target.effect == "landing_half_visibility20" {
                let v = vis.ok_or("11.1 radar tower effect requires source visibility percent")?;
                if v <= 20. {
                    half = true;
                }
            }
        }
    }
    Ok(if half { 0.5 } else { 1. })
}
pub fn runway_gate(w: &World, base: EntityId, class: &str) -> Result<(), String> {
    let s = w.rules.ground.as_ref().ok_or("11 ground state missing")?;
    let service = s
        .air_services
        .get(&base)
        .ok_or("11 land airfield servicing metadata not configured")?;
    if !operational(w, base) {
        return Err("11.1 airfield root facility disabled".into());
    }
    let a = super::aviation2026::spec(class)?;
    if matches!(a.engine, super::aviation2026::Engine::TS) {
        return Ok(());
    }
    let mut needed = if service.stol.contains(class) {
        RunwayClass::STOL
    } else {
        match a.size {
            0 | 1 => RunwayClass::Small,
            2 => RunwayClass::Medium,
            3 => RunwayClass::Large,
            _ => return Err("11.5 source aircraft size unavailable".into()),
        }
    };
    if service.lightly_loaded.contains(class) {
        needed = match needed {
            RunwayClass::Large => RunwayClass::Medium,
            RunwayClass::Medium => RunwayClass::Small,
            _ => RunwayClass::STOL,
        };
    }
    let mut found = false;
    let mut blocked_reason = None;
    for f in components(w, base).filter(|f| f.config.runway.is_some()) {
        found = true;
        let r = f.config.runway.as_ref().unwrap();
        if f.destroyed {
            continue;
        }
        let capacity = if f.cuts == 0 {
            Ok(Some(r.baseline))
        } else {
            runway_capacity(f.cuts, r.length_m)
        };
        match capacity {
            Ok(Some(cap)) if cap >= needed => return Ok(()),
            Err(reason) => blocked_reason = Some(reason),
            _ => {}
        }
    }
    if !found {
        return Err("11.5 no explicitly configured runway for this airfield".into());
    }
    Err(blocked_reason
        .unwrap_or("11.5 runway cuts prevent this aircraft class/load from operating".into()))
}
pub fn parking_gate(w: &World, parking: EntityId) -> Result<(), String> {
    let name = &w.entity(parking).ok_or("missing parking area")?.name;
    if w.rules.ground.as_ref().is_some_and(|s| {
        s.facilities.values().any(|f| {
            f.disabled
                && f.config
                    .taxiway
                    .as_ref()
                    .is_some_and(|t| t.isolated_parking.contains(name))
        })
    }) {
        Err("11.5.1 taxiway cut isolates this parking area from runways".into())
    } else {
        Ok(())
    }
}
pub fn own_view(w: &World, id: EntityId) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let s = w.rules.ground.as_ref()?;
    let f = s.facilities.get(&id);
    let launches: Vec<_> = s.launches.iter().filter(|j| j.base == id).collect();
    let side = w.entity(id)?.side;
    let spotting = s.spotting.get(&id).and_then(|spot| {
        let contact = w.sides[side.index()].contacts.values().find(|c| c.about == spot.target)?;
        Some(json!({"contact":contact.id,"method":spot.method,"started":spot.started,"last_fire":spot.last_fire,"beacon_ready":spot.beacon_ready}))
    });
    let mut result = json!({"facility":f,"air_service":s.air_services.get(&id),"launch_queue":launches,"repairs":s.repair_jobs.iter().filter(|j|j.runway==id).map(repair_job_view).collect::<Vec<_>>(),"spotting":spotting});
    if let Some(service) = berth::own_view(w, id) {
        result.as_object_mut().unwrap().insert("berth_resupply".into(), service);
    }
    if let Some(service) = gci::own_view(w, id) {
        result.as_object_mut().unwrap().insert("gci".into(), service);
    }
    if let Some(service) = drydock::own_view(w, id) {
        result.as_object_mut().unwrap().insert("drydocks".into(), service);
    }
    Some(result)
}
fn visual_contact(w: &World, side: crate::Side, target: EntityId) -> bool {
    w.sides[side.index()].contacts.values().any(|c| {
        c.about == target
            && !c.stale
            && c.sensors.iter().any(|s| {
                w.db.sensors
                    .get(s)
                    .is_some_and(|d| d.kind == crate::db::SensorKind::Visual)
            })
    })
}
fn validate_spotter(
    w: &World,
    ship: EntityId,
    target: EntityId,
    method: &Spotting,
) -> Result<(), String> {
    let a = w
        .entity(ship)
        .filter(|e| e.alive)
        .ok_or("11.4 firing ship unavailable")?;
    let t = w.entity(target).ok_or("11.4 target unavailable")?;
    match method {
        Spotting::Ship => {
            if a.pos.range_to(t.pos) >= 5. || !visual_contact(w, a.side, target) {
                return Err("11.4 ship spotting requires a visual target at less than 10 kyds and Short/Medium range".into());
            }
        }
        Spotting::Aerial { aircraft } => {
            let p = w
                .entity(*aircraft)
                .filter(|p| {
                    p.alive && p.side == a.side && p.kind == crate::types::PlatformKind::Aircraft
                })
                .ok_or("11.4 friendly aerial observer unavailable")?;
            if p.pos.range_to(t.pos) > 3.
                || !(60. < p.altitude_m && p.altitude_m <= 7500.)
                || !visual_contact(w, a.side, target)
            {
                return Err("11.4 aerial observer requires visual target within 3 nmi at Low/Medium altitude".into());
            }
        }
        Spotting::Ground { observer } | Spotting::Beacon { observer } => {
            let p = w
                .entity(*observer)
                .filter(|p| p.alive && p.side == a.side)
                .ok_or("11.4 friendly ground observer unavailable")?;
            let cap = w
                .rules
                .ground
                .as_ref()
                .and_then(|s| s.observers.get(observer))
                .ok_or("11.4 ground observer capabilities not declared by scenario")?;
            if p.pos.range_to(t.pos) > cap.radius_nm
                || matches!(method, Spotting::Beacon { .. }) && !cap.beacon
            {
                return Err("11.4 observer outside declared capability or lacks beacon".into());
            }
        }
        Spotting::Map | Spotting::Blind => {}
    }
    Ok(())
}
pub fn apply(w: &mut World, unit: EntityId, action: &Action) -> Result<(), String> {
    if !active(w) {
        return Err("11 requires explicit hv2026-rts-v4".into());
    }
    let owner = w
        .entity(unit)
        .filter(|e| e.alive)
        .ok_or("11 actor unavailable")?
        .side;
    match action {
        Action::GciVector { .. } => return gci::apply(w, unit, action),
        Action::PumpDock { .. } => return drydock::apply(w, unit, action),
        Action::AlongsideResupply { .. } | Action::CancelAlongsideResupply { .. } => {
            return berth::apply(w, unit, action);
        }
        Action::RepairRunway { runway } => {
            let e = w
                .entity(*runway)
                .filter(|e| e.side == owner && e.alive)
                .ok_or("11.5 own runway required")?;
            let f = w
                .rules
                .ground
                .as_ref()
                .and_then(|s| s.facilities.get(&e.id))
                .ok_or("11.5 source runway required")?;
            if f.destroyed || f.cuts == 0 {
                return Err("11.5 rubble cannot be repaired; no cuts need repair".into());
            }
            let spec = f
                .config
                .runway
                .as_ref()
                .and_then(|r| r.repair.as_ref())
                .ok_or("11.5 runway repair resources/time not provided by scenario")?;
            let work = if let Some(name) = spec.maintenance_provider.as_deref() {
                let provider =
                    maintenance_provider(w, w.rules.ground.as_ref().unwrap(), owner, name)?;
                let rate = maintenance_rate(w, provider, owner)?;
                Some(RepairWork {
                    provider,
                    required_seconds: spec.seconds_per_cut,
                    completed_seconds: 0.,
                    last_tick: w.tick,
                    last_rate: Some(rate),
                    pending: None,
                })
            } else {
                None
            };
            let busy = w
                .rules
                .ground
                .as_ref()
                .unwrap()
                .repair_jobs
                .iter()
                .filter(|j| j.runway == *runway)
                .count();
            if busy >= spec.teams as usize || busy >= f.cuts as usize {
                return Err("11.5 all declared repair teams are busy or cuts reserved".into());
            }
            let due = w.tick + spec.seconds_per_cut;
            let detail = if let Some(work) = &work {
                json!({"maintenance_provider":work.provider,"required_work_seconds":work.required_seconds,"rate":work.last_rate,"completed_work_seconds":0,"source":"scenario-declared service binding and rate; not a universal repair multiplier"})
            } else {
                json!({"due":due,"source":"scenario-author repair resources/time; not a universal 2–4-hour constant"})
            };
            w.rules
                .ground
                .as_mut()
                .unwrap()
                .repair_jobs
                .push(RepairJob {
                    runway: *runway,
                    due,
                    owner,
                    work,
                });
            trace(w, *runway, "11.5.repair_started", detail);
        }
        Action::Spot { contact, method } => {
            let c = w.sides[owner.index()]
                .contacts
                .get(contact)
                .ok_or("11.4 own target contact required")?;
            let target = c.about;
            // Sections11.6/11.7 declare the whole configured complex as the
            // spotting target. Individual DP/armor belong to the component
            // selected later by the reverse-hit roll, never to this area.
            if !w.rules.ground.as_ref().is_some_and(|s|s.complexes.contains_key(&target)) {
                source_target(w, target)?;
            }
            validate_spotter(w, unit, target, method)?;
            let shared = match method {
                Spotting::Aerial { aircraft } => Some(*aircraft),
                Spotting::Ground { observer } | Spotting::Beacon { observer } => Some(*observer),
                _ => None,
            };
            if let Some(observer) = shared {
                if w.rules
                    .ground
                    .as_ref()
                    .unwrap()
                    .spotting
                    .iter()
                    .any(|(ship, s)| {
                        *ship != unit
                            && match s.method {
                                Spotting::Aerial { aircraft } => aircraft == observer,
                                Spotting::Ground { observer: o }
                                | Spotting::Beacon { observer: o } => o == observer,
                                _ => false,
                            }
                    })
                {
                    return Err(
                        "11.4 one observer may spot for only one ship against one target".into(),
                    );
                }
            }
            let beacon = if matches!(method, Spotting::Beacon { .. }) {
                Some(w.tick + roll(w, unit, "11.4.beacon_setup", 6) as i64 * 180)
            } else {
                None
            };
            w.rules.ground.as_mut().unwrap().spotting.insert(
                unit,
                SpottingState {
                    target,
                    method: method.clone(),
                    started: w.tick,
                    last_fire: None,
                    beacon_ready: beacon,
                },
            );
            trace(
                w,
                unit,
                "11.4.spotting",
                json!({"contact":contact,"method":method,"beacon_ready":beacon}),
            );
        }
    }
    Ok(())
}
/// Returns additive source gun-index shifts. 8.3 supplies the gun table and
/// first/second-turn range-finding penalties; this API never fabricates them.
pub fn source_spotting(
    w: &mut World,
    ship: EntityId,
    target: EntityId,
    short_or_medium: bool,
) -> Result<i32, String> {
    let mut s = w
        .rules
        .ground
        .as_ref()
        .and_then(|s| s.spotting.get(&ship))
        .cloned()
        .ok_or("11.4 declare a verified spotting or barrage method before NGFS")?;
    if s.target != target {
        return Err("11.4 continuous spotting target changed; declare a new task".into());
    }
    validate_spotter(w, ship, target, &s.method)?;
    if s.last_fire.is_some_and(|last| w.tick - last > 180) {
        s.started = w.tick;
    }
    let turns = (w.tick - s.started).max(0) / 180 + 1;
    let mut modifier = match s.method {
        Spotting::Ship => {
            if !short_or_medium {
                return Err(
                    "11.4 self-spotted destruction fire requires Short/Medium gun range".into(),
                );
            }
            if turns >= 3 {
                1
            } else {
                0
            }
        }
        Spotting::Aerial { .. } | Spotting::Ground { .. } => {
            if turns >= 2 {
                2
            } else {
                0
            }
        }
        Spotting::Beacon { .. } => {
            if !s.beacon_ready.is_some_and(|due| due <= w.tick) {
                return Err("11.4 radar beacon not emplaced; wait its D6 Tactical Turns".into());
            }
            3
        }
        Spotting::Map => -4,
        Spotting::Blind => -6,
    };
    if !matches!(s.method, Spotting::Map | Spotting::Blind) {
        let n = w
            .rules
            .ground
            .as_ref()
            .unwrap()
            .spotting
            .iter()
            .filter(|(_, other)| {
                other.target == target
                    && !matches!(other.method, Spotting::Map | Spotting::Blind)
                    && other.last_fire.is_some_and(|t| w.tick - t < 180)
            })
            .count();
        let own_recent = s.last_fire.is_some_and(|t| w.tick - t < 180);
        modifier -= 2 * (n + usize::from(!own_recent)).saturating_sub(1) as i32;
    }
    s.last_fire = Some(w.tick);
    w.rules.ground.as_mut().unwrap().spotting.insert(ship, s);
    trace(
        w,
        ship,
        "11.4.gun_modifier",
        json!({"index_shift":modifier,"continuous_turn":turns}),
    );
    Ok(modifier)
}
/// A complex must roll the reverse-hit D100 before resolving 8.3 gunnery. A
/// missing configured component is unresolved, never silently rerolled.
pub fn bombardment_target(
    w: &mut World,
    complex: EntityId,
    identity: &str,
) -> Result<Option<EntityId>, String> {
    let c = w
        .rules
        .ground
        .as_ref()
        .and_then(|s| s.complexes.get(&complex))
        .cloned()
        .ok_or("11.6/11.7 complex components not configured")?;
    let d100 = roll(w, complex, &format!("11.reverse:{identity}"), 100);
    let part = reverse_component(c.kind, d100)?;
    trace(
        w,
        complex,
        "11.reverse_hit",
        json!({"identity":identity,"d100":d100,"component":part}),
    );
    let Some(part) = part else { return Ok(None) };
    let ids=c.components.get(&part).filter(|ids|!ids.is_empty()).ok_or_else(||format!("11 reverse-hit rolled {part}, but scenario has no component assignment; adjudication required"))?;
    if ids.len() != 1 {
        return Err("11 reverse-hit category contains multiple facilities; firing allocation within category must be declared explicitly".into());
    }
    Ok(Some(ids[0]))
}
pub fn reserved(w: &World, base: EntityId, class: &str) -> usize {
    w.rules
        .ground
        .as_ref()
        .map(|s| {
            s.launches
                .iter()
                .filter(|j| j.base == base && j.class == class)
                .count()
        })
        .unwrap_or(0)
}
pub fn queue_launch(
    w: &mut World,
    base: EntityId,
    class: &str,
    count: u8,
    mission: &crate::commands::AirMission,
    profile: Option<&crate::commands::LaunchProfile>,
) -> Result<(), String> {
    if count == 0 {
        return Err("11 land launch count must be positive".into());
    }
    runway_gate(w, base, class)?;
    parking_gate(w, base)?;
    parking::gate(w, base, class)?;
    let s = w
        .rules
        .ground
        .as_ref()
        .unwrap()
        .air_services
        .get(&base)
        .cloned()
        .ok_or("11 explicit airfield servicing configuration required")?;
    let initial = *s
        .initial_speed_kts
        .get(class)
        .filter(|v| v.is_finite() && **v > 0.)
        .ok_or("11 author airfield initial climb speed missing")?;
    super::aviation2026::source_speed(class, 1000., super::aviation2026::Throttle::Fmp).and_then(
        |cap| {
            if initial <= cap {
                Ok(())
            } else {
                Err("11 configured takeoff speed exceeds source FMP".into())
            }
        },
    )?;
    let duration = (s.launch_interval_s as f64 / flight_rate_factor(w, base, false)?).ceil() as i64;
    let now = w.tick;
    for _ in 0..count {
        let lane = (0..s.launch_lanes)
            .map(|i| format!("{}:land_launch:{i}", base.0))
            .min_by_key(|key| {
                w.rules
                    .ground
                    .as_ref()
                    .unwrap()
                    .lanes
                    .get(key)
                    .copied()
                    .unwrap_or(0)
            })
            .unwrap();
        let due = (now + 300).max(
            w.rules
                .ground
                .as_ref()
                .unwrap()
                .lanes
                .get(&lane)
                .copied()
                .unwrap_or(now)
                + duration,
        );
        w.rules
            .ground
            .as_mut()
            .unwrap()
            .lanes
            .insert(lane.clone(), due);
        w.rules.ground.as_mut().unwrap().launches.push(LaunchJob {
            base,
            class: class.into(),
            mission: mission.clone(),
            profile: profile.cloned(),
            due,
            lane,
        });
    }
    trace(
        w,
        base,
        "11.airfield_launch_queued",
        json!({"class":class,"count":count,"interval_s":duration,"source":s.source}),
    );
    Ok(())
}
pub fn recovery_ready(w: &mut World, aircraft: EntityId, base: EntityId) -> Option<i64> {
    let a = w.entity(aircraft)?.clone();
    let s = w.rules.ground.as_ref()?.air_services.get(&base)?.clone();
    runway_gate(w, base, &a.class).ok()?;
    parking::gate(w, base, &a.class).ok()?;
    let speed = *s.landing_speed_kts.get(&a.class)?;
    let turnaround = *s.turnaround_s.get(&a.class)?;
    if turnaround <= 0 {
        return None;
    }
    let aligned = a.altitude_m >= s.landing_min_altitude_m
        && a.altitude_m <= s.landing_max_altitude_m
        && (a.speed_kts - speed).abs() <= s.landing_speed_tolerance_kts
        && crate::geom::angle_diff(a.course_deg, s.heading_deg).abs()
            <= s.landing_heading_tolerance_deg;
    if !aligned {
        let e = w.entity_mut(aircraft)?;
        e.ordered_altitude_m = s.landing_max_altitude_m;
        e.ordered_course_deg = s.heading_deg;
        e.ordered_speed_kts = speed;
        return None;
    }
    let duration =
        (s.landing_interval_s as f64 / flight_rate_factor(w, base, true).ok()?).ceil() as i64;
    let lane = (0..s.landing_lanes)
        .map(|i| format!("{}:land_recover:{i}", base.0))
        .min_by_key(|k| {
            w.rules
                .ground
                .as_ref()
                .unwrap()
                .lanes
                .get(k)
                .copied()
                .unwrap_or(0)
        })?;
    if w.rules
        .ground
        .as_ref()?
        .lanes
        .get(&lane)
        .copied()
        .unwrap_or(0)
        > w.tick
    {
        return None;
    }
    let now = w.tick;
    w.rules.ground.as_mut()?.lanes.insert(lane, now + duration);
    trace(
        w,
        base,
        "11.airfield_recovery",
        json!({"aircraft":aircraft,"ready_at":now+turnaround,"interval_s":duration,"source":s.source}),
    );
    Some(now + turnaround)
}
pub fn step(w: &mut World) {
    if !active(w) || w.rules.ground.is_none() {
        return;
    }
    let now = w.tick;
    aircraft_maintenance::step(w);
    settle_bound_repairs(w);
    let jobs = std::mem::take(&mut w.rules.ground.as_mut().unwrap().repair_jobs);
    for j in jobs {
        if j.work.is_some() {
            // A bound job completes only through its accumulated work ledger.
            w.rules.ground.as_mut().unwrap().repair_jobs.push(j);
            continue;
        }
        if has_bound_repair(w, j.runway) {
            // A changed/incomplete saved binding cannot upgrade a historical
            // due tick into a newly qualified service job or invent its work.
            let key = format!("11.5.missing_bound_work:{}:{}", j.runway.0, j.due);
            if w.rules.ground.as_mut().unwrap().diagnostics.insert(key) {
                trace(
                    w,
                    j.runway,
                    "11.5.repair_pending",
                    json!({"reason":"explicit maintenance binding has no original work ledger; no full-speed due completion"}),
                );
            }
            w.rules.ground.as_mut().unwrap().repair_jobs.push(j);
            continue;
        }
        if j.due > now {
            w.rules.ground.as_mut().unwrap().repair_jobs.push(j);
            continue;
        }
        if let Some(f) = w
            .rules
            .ground
            .as_mut()
            .unwrap()
            .facilities
            .get_mut(&j.runway)
        {
            if !f.destroyed && f.cuts > 0 {
                f.cuts -= 1;
                trace(
                    w,
                    j.runway,
                    "11.5.cut_repaired",
                    json!({"due":j.due,"structural_hp_repaired":0}),
                );
            }
        }
    }
    let jobs = std::mem::take(&mut w.rules.ground.as_mut().unwrap().launches);
    for mut j in jobs {
        if j.due > now {
            w.rules.ground.as_mut().unwrap().launches.push(j);
            continue;
        }
        let not_before = w
            .rules
            .ground
            .as_ref()
            .unwrap()
            .launch_not_before
            .get(&j.lane)
            .copied()
            .unwrap_or(0);
        if now < not_before {
            j.due = not_before;
            w.rules.ground.as_mut().unwrap().launches.push(j);
            continue;
        }
        if runway_gate(w, j.base, &j.class).is_err()
            || parking::gate(w, j.base, &j.class).is_err() {
            j.due = now + 1;
            w.rules.ground.as_mut().unwrap().launches.push(j);
            continue;
        }
        let before = w.next_entity;
        match crate::air::complete_queued_launch(
            w,
            j.base,
            &j.class,
            &j.mission,
            j.profile.as_ref(),
        ) {
            Ok(()) => {
                let service = w.rules.ground.as_ref().unwrap().air_services[&j.base].clone();
                let interval = (service.launch_interval_s as f64
                    / flight_rate_factor(w, j.base, false)
                        .expect("configured airfield source rate"))
                .ceil() as i64;
                w.rules
                    .ground
                    .as_mut()
                    .unwrap()
                    .launch_not_before
                    .insert(j.lane.clone(), now + interval);
                let ids: Vec<_> = w
                    .entities
                    .values()
                    .filter(|e| e.id.0 >= before && e.kind == crate::types::PlatformKind::Aircraft)
                    .map(|e| e.id)
                    .collect();
                for id in ids {
                    let e = w.entity_mut(id).unwrap();
                    e.altitude_m = 61.;
                    e.speed_kts = service.initial_speed_kts[&j.class];
                    e.course_deg = service.heading_deg;
                }
                trace(
                    w,
                    j.base,
                    "11.airfield_launch_complete",
                    json!({"class":j.class,"source":service.source}),
                );
            }
            Err(reason) => trace(
                w,
                j.base,
                "11.airfield_launch_cancelled",
                json!({"reason":reason,"class":j.class}),
            ),
        }
    }
}

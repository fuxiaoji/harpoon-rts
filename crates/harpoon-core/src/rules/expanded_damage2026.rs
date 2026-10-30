//! Full-profile expanded criticals and explicitly selected §14.7–14.9 options.
//! Physical fittings are source/scenario declarations, never inferred from a
//! weapon's name or from a proportion of an aggregate magazine. Legacy tables
//! and legacy serialized worlds are deliberately outside this module.
use super::damage_runtime as dr;
use crate::{
    db::{SensorKind, WeaponKind},
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
#[path = "expanded_damage2026/sar_native.rs"]
pub(crate) mod sar_native;
pub mod cas_communications;
pub mod gun_deck;
pub mod personnel_transfer;
#[cfg(test)]
#[path="expanded_damage_soks_source_20261026.rs"]
mod soks_source_consumer;

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../data/rules/expanded-damage-2026.json"
        ))
        .expect("verified expanded-damage table")
    })
}
fn full(w: &World) -> bool {
    w.rules.profile == dr::RuleProfile::Rts2026Full
}
fn source(s: &str) -> bool {
    !s.trim().is_empty()
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub units: BTreeMap<String, UnitFit>,
    pub classes: BTreeMap<String, UnitFit>,
    pub optional: Options,
    pub survivor_current_heading_deg: Option<f64>,
    pub survivor_current_headings_deg: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rescue_victory: Option<RescueVictory>,
}
///14.8 requires rescue to enter victory conditions, but prints no point value.
/// The author must declare the reward explicitly; absent declarations give0VP.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RescueVictory {
    pub source: String,
    pub points_per_survivor: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub cas_pre1955: bool,
    pub serviceability: bool,
    pub serviceability_campaign: bool,
    pub search_and_rescue: bool,
    pub aircraft_damage: AircraftMode,
    pub armed_fueled_aircraft: bool,
    pub closely_parked_aircraft: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AircraftMode {
    #[default]
    Off,
    ImmediateDetailed,
    ImmediateFiftyFifty,
    PostBattleFiftyFifty,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UnitFit {
    pub source: String,
    pub components: BTreeMap<String, ComponentFit>,
    pub holds: BTreeMap<String, CargoFit>,
    pub crew: Option<u32>,
    pub passengers: Option<u32>,
    pub rank: Option<Rank>,
    pub repair_base: RepairBase,
    pub land_based_aircraft: bool,
    pub cruise_speed_kts: Option<f64>,
    pub crush_depth_m: Option<f64>,
    /// Explicit aircraft geometry/armament; a deck's aggregate class counts do
    /// not identify which individual is armed or where it is parked.
    pub parked: BTreeMap<String, ParkedAircraft>,
    pub rescue: Option<RescueFit>,
    #[serde(default)]
    pub manual_personnel_allocation: bool,
    #[serde(default)]
    pub transfer_factors: Option<personnel_transfer::Fit>,
    #[serde(default)]
    pub vtol_launch: Option<VtolLaunch>,
}
/// Explicit experimental deck-zone and vertical spot, not a real carrier fit.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VtolLaunch { pub source:String, pub zone:String, pub pads:u8, pub duration_s:i64 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rank {
    First,
    Second,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairBase {
    #[default]
    AtSea,
    ForwardTender,
    Port,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    WeaponMount,
    Director,
    Sensor,
    Countermeasure,
    Periscope,
    Catapult,
    Arrestor,
    Elevator,
    Communications,
    Engineering,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentFit {
    pub source: String,
    pub kind: ComponentKind,
    #[serde(default)]
    pub binding: Option<String>,
    #[serde(default)]
    pub physical: Option<usize>,
    #[serde(default)]
    pub mount_index: Option<usize>,
    #[serde(default)]
    pub ammunition: Option<u16>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub cas_group: Option<String>,
    #[serde(default)]
    pub local_control: bool,
    #[serde(default)]
    pub manually_operated: bool,
    #[serde(default)]
    pub controls: Vec<String>,
    #[serde(default)]
    pub zone: Option<String>,
    #[serde(default)]
    pub navigation: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CargoKind {
    Ammunition,
    CrudeOil,
    RefinedOil,
    AviationFuel,
    Troops,
    General,
    Vehicles,
    Aircraft,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoFit {
    pub source: String,
    pub kind: CargoKind,
    pub amount: f64,
    #[serde(default)]
    pub adjacent: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParkedAircraft {
    pub source: String,
    pub class: String,
    pub zone: String,
    pub position_m: [f64; 2],
    pub armed_ordnance: Vec<(String, u16)>,
    /// None means this individual aircraft fuel state is unknown.
    #[serde(default)]
    pub fueled: Option<bool>,
    pub stock: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RescueKind {
    Ship,
    Helicopter,
    FlyingBoat,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RescueFit {
    pub source: String,
    pub kind: RescueKind,
    pub capacity: u32,
    #[serde(default)]
    pub station_radius_m: Option<f64>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub config: Configuration,
    #[serde(default)]
    pub transfers: personnel_transfer::State,
    pub units: BTreeMap<EntityId, UnitState>,
    pub survivors: BTreeMap<String, Survivors>,
    pub rescues: BTreeMap<EntityId, RescueTask>,
    #[serde(default)]
    pub applied_sources: BTreeSet<String>,
    #[serde(default)]
    pub source_inputs: BTreeMap<String, CriticalContext>,
    #[serde(default)]
    pub attack_probabilities: BTreeMap<String, f64>,
    #[serde(default)]
    pub survivor_labels: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub loss_witnesses: BTreeMap<String, sar_native::LossWitness>,
    /// Counts actual committed pickups, independently of later entity cleanup.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rescued_by_side: BTreeMap<String, u64>,
    #[serde(skip)]
    pub active_source: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UnitState {
    pub records: BTreeMap<String, CriticalRecord>,
    pub sequence: u64,
    pub disabled: BTreeSet<String>,
    pub original_mounts: BTreeMap<usize, u8>,
    pub ammunition: BTreeMap<String, u16>,
    pub holds: BTreeMap<String, HoldState>,
    pub destroyed_aircraft: BTreeSet<String>,
    pub service_checks: BTreeMap<String, ServiceCheck>,
    pub aircraft: Option<AircraftDamage>,
    #[serde(default)]
    pub aircraft_repairs: BTreeMap<u32, ServiceRepair>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub previous_aircraft_repairs: Vec<BTreeMap<u32,ServiceRepair>>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub catastrophic_loss: Option<String>,
    pub pending: BTreeMap<String, String>,
    pub last_depth_check: Option<i64>,
    pub last_hull_check: Option<i64>,
    pub last_cargo_check: Option<i64>,
    pub survivor_created: bool,
    pub loss_pending: bool,
    #[serde(default)]
    pub survivors_aboard: u32,
    #[serde(default)]
    pub system_counts: BTreeMap<String, u32>,
    #[serde(default)]
    pub fire_history: BTreeMap<String, FireHistory>,
    #[serde(default)]
    pub flooding_dp: f64,
    #[serde(default)]
    pub source_flood_severity: f64,
    #[serde(default)]
    pub patch: Option<Patch>,
    #[serde(default)]
    pub abandoned: BTreeSet<String>,
    #[serde(default)]
    pub personnel_choices: BTreeMap<u8, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FireHistory {
    pub extinguished_at: Option<i64>,
    pub checked_hours: BTreeSet<u8>,
    pub restarted: bool,
    #[serde(default)]
    pub suppressed: bool,
    #[serde(default)]
    pub origin: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Patch {
    pub recovered_dp: f64,
    pub began_at: i64,
    pub last_check: Option<i64>,
    pub day: i64,
    pub maximum_speed: f64,
    pub popped: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CriticalRecord {
    pub source_identity: String,
    pub root_identity: String,
    pub own_id: String,
    pub occurred_at: i64,
    pub effect: String,
    pub component: Option<String>,
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HoldState {
    pub amount: f64,
    pub fire: bool,
    pub flooded: bool,
    pub exploded: bool,
    pub explosion_checked: BTreeSet<String>,
    #[serde(default)]
    pub fire_identity: Option<String>,
    #[serde(default)]
    pub fire_identities: BTreeSet<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceCheck {
    pub checked_at: i64,
    pub d100: u8,
    pub failed: bool,
    pub repaired: bool,
    pub checks: BTreeMap<u32, ServiceRepair>,
    pub sea_unrepairable: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceRepair {
    pub due_at: i64,
    pub chance_percent: f64,
    pub d100: u8,
    pub success: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AirSeverity {
    Light,
    Heavy,
    Critical,
    Destroyed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AircraftDamage {
    pub severity: AirSeverity,
    pub light: BTreeSet<String>,
    pub jettison_failed: Option<bool>,
    pub hits: BTreeMap<String, AirHit>,
    pub post_battle: bool,
    #[serde(default)]
    pub repaired: bool,
    #[serde(default)]
    pub active_hits: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AirHit {
    pub occurred_at: i64,
    pub d10: u8,
    pub light_d6: Option<u8>,
    pub result: AirSeverity,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Survivors {
    pub own_side: crate::Side,
    pub lost_unit: EntityId,
    pub position: crate::geom::Vec2,
    pub created_at: i64,
    pub count: u32,
    pub rescued: u32,
    pub observed_by: BTreeSet<EntityId>,
    pub drift: Option<(f64, f64)>,
    pub source_crew: u32,
    pub casualty_d6: Option<u8>,
    #[serde(default)]
    pub sightings: BTreeMap<EntityId, SurvivorReport>,
    #[serde(default)]
    pub last_drift_tick: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SurvivorReport {
    pub observed_at: i64,
    pub position: crate::geom::Vec2,
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<crate::Side>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RescueTask {
    pub group: String,
    pub ordered_at: i64,
    pub last_turn: Option<i64>,
    pub remaining: u32,
    pub ready_at: Option<i64>,
    pub automatic: bool,
    pub picked_up: u32,
    pub pending: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work: Option<RescueWork>,
}
/// Explicit RTS mapping of a ship/hovering-helicopter Tactical Turn to work.
/// This is source-private persistence, never a caller-supplied command value.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RescueWork {
    pub last_checked_at: i64,
    pub continuous_seconds: i64,
    pub eligible: bool,
}

impl State {
    pub fn new(config: Configuration) -> Result<Self, String> {
        if config.rescue_victory.as_ref().is_some_and(|v|
            !source(&v.source) || !v.points_per_survivor.is_finite() || v.points_per_survivor < 0.
                || v.points_per_survivor > f64::MAX/(u64::MAX as f64)) {
            return Err("rescue victory requires an explicit source and nonnegative reward representable for its survivor counter".into());
        }
        if config
            .survivor_current_headings_deg
            .iter()
            .any(|h| !h.is_finite() || !(0.0..360.0).contains(h))
        {
            return Err("survivor current heading candidates invalid".into());
        }
        let any = config.optional.cas_pre1955
            || config.rescue_victory.is_some()
            || config.optional.serviceability
            || config.optional.search_and_rescue
            || config.optional.aircraft_damage != AircraftMode::Off
            || config.optional.armed_fueled_aircraft
            || config.optional.closely_parked_aircraft;
        if (any || !config.units.is_empty() || !config.classes.is_empty())
            && !source(&config.source)
        {
            return Err("expanded damage needs explicit source/configuration".into());
        }
        for (name, u) in config.units.iter().chain(config.classes.iter()) {
            if name.is_empty() || !source(&u.source) {
                return Err("expanded damage unit source missing".into());
            }
            for (id, c) in &u.components {
                if id.is_empty() || !source(&c.source) {
                    return Err("expanded component source missing".into());
                }
                if matches!(c.kind, ComponentKind::WeaponMount | ComponentKind::Sensor)
                    && c.binding.as_ref().is_none_or(|s| s.is_empty())
                {
                    return Err("expanded physical component binding missing".into());
                }
                if c.kind == ComponentKind::WeaponMount && c.physical.is_none() {
                    return Err("expanded mount physical identity missing".into());
                }
                if c.location
                    .as_ref()
                    .is_some_and(|l| !matches!(l.as_str(), "topside" | "belowdecks"))
                {
                    return Err("expanded magazine location invalid".into());
                }
                if c.controls.iter().any(|id| !u.components.contains_key(id)) {
                    return Err("expanded director control binding missing".into());
                }
            }
            for (id, h) in &u.holds {
                if id.is_empty()
                    || !source(&h.source)
                    || !h.amount.is_finite()
                    || h.amount < 0.
                    || h.adjacent
                        .iter()
                        .any(|n| n == id || !u.holds.contains_key(n))
                {
                    return Err("expanded cargo source/geometry invalid".into());
                }
            }
            for a in u.parked.values() {
                if !source(&a.source)
                    || !matches!(a.stock.as_str(), "ready" | "cycling")
                    || a.position_m.iter().any(|v| !v.is_finite())
                {
                    return Err("expanded parked aircraft source invalid".into());
                }
            }
            if u.cruise_speed_kts
                .is_some_and(|v| !v.is_finite() || v <= 0.)
                || u.crush_depth_m.is_some_and(|v| !v.is_finite() || v <= 0.)
            {
                return Err("expanded source speed/depth invalid".into());
            }
            if u.vtol_launch.as_ref().is_some_and(|v| !v.source.starts_with("scenario_assumption:") || !matches!(v.zone.as_str(),"amidships"|"aft") || v.pads==0 || v.duration_s<=0) { return Err("14.5 VTOL zone/spot/time requires explicit scenario assumption".into()); }
            if u.rescue.as_ref().is_some_and(|r| !source(&r.source)) {
                return Err("expanded rescue fit source missing".into());
            }
            if let Some(r) = &u.rescue {
                if r.kind == RescueKind::Ship
                    && u.crew.is_some_and(|crew| {
                        r.capacity > crew.saturating_add(u.passengers.unwrap_or(0))
                    })
                {
                    return Err(
                        "ship rescue capacity exceeds explicit crew/passenger complement".into(),
                    );
                }
            }
        }
        Ok(Self {
            config,
            ..Self::default()
        })
    }
}
fn fit(w: &World, id: EntityId) -> Option<&UnitFit> {
    let entity = w.entity(id)?;
    let config = &w.rules.expanded_damage.as_ref()?.config;
    config
        .units
        .get(&entity.name)
        .or_else(|| config.classes.get(&entity.class))
}
pub fn validate_world(w: &World) -> Result<(), String> {
    let Some(s) = w.rules.expanded_damage.as_ref() else {
        return Ok(());
    };
    for e in w.entities.values() {
        let Some(f) = fit(w, e.id) else { continue };
        for c in f.components.values() {
            match c.kind {
                ComponentKind::WeaponMount => {
                    let weapon = c
                        .binding
                        .as_deref()
                        .ok_or("physical weapon binding absent")?;
                    let candidates: Vec<_> = e
                        .weapons
                        .iter()
                        .enumerate()
                        .filter(|(i, m)| {
                            m.weapon == weapon && c.mount_index.is_none_or(|index| index == *i)
                        })
                        .collect();
                    if candidates.len() != 1 {
                        return Err(
                            "expanded weapon binding is absent or ambiguous; set exact mount_index"
                                .into(),
                        );
                    }
                    let m = candidates[0].1;
                    if c.physical.is_none_or(|p| p >= m.mounts as usize) {
                        return Err(
                            "expanded physical mount is outside actual installed count".into()
                        );
                    }
                }
                ComponentKind::Sensor => {
                    if c.navigation && !c.binding.as_deref().and_then(|id|w.db.sensors.get(id)).is_some_and(|sensor|sensor.kind == SensorKind::Radar) {
                        return Err("14.5 navigation exception requires a physically fitted radar, not another sensor kind".into());
                    }
                    if !e
                        .sensors
                        .iter()
                        .any(|sensor| Some(sensor.sensor.as_str()) == c.binding.as_deref())
                    {
                        return Err("expanded sensor is not actually fitted".into());
                    }
                }
                _ => {}
            }
        }
        if s.config.optional.aircraft_damage != AircraftMode::Off
            && e.kind == PlatformKind::Aircraft
            && f.cruise_speed_kts.is_none()
        {
            return Err("optional aircraft damage requires explicit source cruise speed".into());
        }
    }
    Ok(())
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    validate_world(w)?;
    personnel_transfer::initialize(w)?;
    if w.rules
        .expanded_damage
        .as_ref()
        .is_some_and(|s| s.config.optional.serviceability)
    {
        let ids: Vec<_> = w
            .entities
            .values()
            .filter(|e| fit(w, e.id).is_some())
            .map(|e| e.id)
            .collect();
        for id in ids {
            serviceability_check(w, id)?;
        }
    }
    Ok(())
}
pub fn aircraft_speed_cap(w: &World, id: EntityId) -> Result<Option<f64>, String> {
    if !aircraft_damage_active(w) {
        return Ok(None);
    }
    let a = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .and_then(|u| u.aircraft.as_ref());
    if let Some(a) = a {
        if a.repaired {
            return Ok(None);
        }
        let needs = a.severity != AirSeverity::Light
            || a.light.contains("engine")
            || a.light.contains("structure");
        if needs && a.severity != AirSeverity::Destroyed {
            return fit(w, id)
                .and_then(|f| f.cruise_speed_kts)
                .map(Some)
                .ok_or("damaged aircraft cruise speed source pending".into());
        }
    }
    Ok(None)
}
fn record(
    w: &mut World,
    id: EntityId,
    identity: &str,
    effect: &str,
    component: Option<String>,
    status: &str,
) {
    let now = w.tick;
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        let root_identity = s.active_source.clone().unwrap_or_else(|| identity.into());
        let u = s.units.entry(id).or_default();
        let key = format!("{identity}|{effect}");
        if !u.records.contains_key(&key) {
            u.sequence += 1;
            u.records.insert(
                key,
                CriticalRecord {
                    source_identity: identity.into(),
                    root_identity,
                    own_id: format!("casualty-{}-{}", id.0, u.sequence),
                    occurred_at: now,
                    effect: effect.into(),
                    component,
                    status: status.into(),
                },
            );
        }
    }
}
fn private_trace(w: &mut World, id: EntityId, rule: &str, detail: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "expanded_damage2026".into(),
            rule: rule.into(),
            side: w.entity(id).map(|e| e.side),
            unit: Some(id),
            contact: None,
            detail: detail.to_string(),
        },
        Visibility::Referee,
    );
}
fn pending(w: &mut World, id: EntityId, identity: &str, domain: &str, reason: &str) {
    record(w, id, identity, domain, None, "pending");
    let mut changed = false;
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        let u = s.units.entry(id).or_default();
        if !u.pending.contains_key(domain) {
            u.pending.insert(domain.into(), reason.into());
            changed = true;
        }
    }
    if changed {
        private_trace(
            w,
            id,
            "14.expanded_pending",
            json!({"identity":identity,"domain":domain,"reason":reason}),
        );
    }
}
fn roll(w: &mut World, id: EntityId, r: &mut Rng, sides: u8, identity: &str, purpose: &str) -> u8 {
    let d = 1 + (r.next_f64() * sides as f64) as u8;
    w.rules
        .expanded_dice
        .entry(id)
        .or_default()
        .push(json!({"tick":w.tick,"identity":identity,"purpose":purpose,"die":sides,"roll":d}));
    private_trace(
        w,
        id,
        "14.expanded_roll",
        json!({"identity":identity,"purpose":purpose,"die":sides,"roll":d}),
    );
    d
}
fn index(
    w: &mut World,
    id: EntityId,
    r: &mut Rng,
    n: usize,
    identity: &str,
    purpose: &str,
) -> usize {
    let d = (r.next_f64() * n as f64) as usize;
    w.rules
        .expanded_dice
        .entry(id)
        .or_default()
        .push(json!({"tick":w.tick,"identity":identity,"purpose":purpose,"die":n,"roll":d+1}));
    private_trace(
        w,
        id,
        "14.expanded_roll",
        json!({"identity":identity,"purpose":purpose,"die":n,"roll":d+1}),
    );
    d
}
pub fn column(w: &World, t: &Entity, ordinary: &str) -> String {
    if ordinary == "pre1955"
        && w.rules
            .expanded_damage
            .as_ref()
            .is_some_and(|s| s.config.optional.cas_pre1955)
    {
        if matches!(dr::size(t), "A" | "B") {
            "cas_major".into()
        } else {
            "cas_minor".into()
        }
    } else {
        ordinary.into()
    }
}
pub fn row(column: &str, d100: u8) -> Option<(String, bool)> {
    tables()["critical_columns"][column]
        .as_array()?
        .iter()
        .find(|r| d100 as u64 <= r["through"].as_u64().unwrap_or(0))
        .map(|r| {
            (
                r["effect"].as_str().unwrap().into(),
                r["armored"].as_bool().unwrap(),
            )
        })
}

/// Source-backed branch for native critical selectors. True means this module
/// consumed the result (including an explicit pending/absent-system outcome).
pub(super) fn resolve_extra(
    w: &mut World,
    t: &Entity,
    h: &dr::Resolved,
    kind: &str,
    d100: Option<u8>,
    r: &mut Rng,
    identity: &str,
) -> bool {
    if !full(w) || w.rules.expanded_damage.is_none() {
        return false;
    }
    match kind {
        "weapon" | "weapon_cargo" => {
            weapon_critical(w, t, h, None, d100, r, identity);
            true
        }
        "sensor" | "radar" => {
            sensor_critical(w, t, h, kind, r, identity);
            true
        }
        "cargo" => {
            cargo_critical(w, t, h, None, r, identity);
            true
        }
        "hangar" => {
            let f = fit(w, t.id).cloned().unwrap_or_default();
            let mut choices: Vec<_> = f
                .parked
                .iter()
                .filter(|(_, a)| a.zone == "hangar")
                .map(|(id, _)| id.clone())
                .collect();
            if choices.is_empty() {
                let stock = t.deck.as_ref().map_or(0usize, |d| {
                    d.ready.iter().map(|(_, n)| *n as usize).sum::<usize>()
                        + d.cycling.iter().map(|(_, n, _)| *n as usize).sum::<usize>()
                });
                if stock > 0 {
                    pending(w,t.id,identity,"hangar","aggregate deck stock does not identify which individuals are actually in hangar");
                } else {
                    record(w, t.id, identity, "hangar", None, "ignored_not_fitted");
                }
            } else {
                let n = roll(w, t.id, r, 6, identity, "hangar_aircraft") as usize;
                for k in 0..n.min(choices.len()) {
                    let selected = index(w, t.id, r, choices.len(), identity, "hangar_individual");
                    let id = choices.remove(selected);
                    parked_aircraft_hit(
                        w,
                        t,
                        h,
                        &f,
                        &id,
                        r,
                        &format!("{identity}:hangar:{k}"),
                        &mut BTreeSet::new(),
                    );
                }
                record(w, t.id, identity, "hangar", None, "resolved");
            }
            true
        }
        "cic_cargo" if t.weapons.iter().all(|m| m.mounts == 0) => {
            cargo_critical(w, t, h, None, r, identity);
            true
        }
        "cic" | "cic_cargo" => {
            if let Some(e) = w.entity_mut(t.id) {
                e.ext.insert("damage.cic".into(), 1.);
            }
            record(w, t.id, identity, "cic", None, "resolved");
            *w.rules
                .expanded_damage
                .as_mut()
                .unwrap()
                .units
                .entry(t.id)
                .or_default()
                .system_counts
                .entry("cic".into())
                .or_default() += 1;
            super::repairs2026::record_system(w, t.id, identity, "cic", None, 1);
            true
        }
        "communications" => {
            let group = match roll(w, t.id, r, 6, identity, "cas_communications") {
                1 | 2 => "long_range",
                3 | 4 => "short_range",
                _ => "aircraft",
            };
            component_critical(
                w,
                t,
                identity,
                ComponentKind::Communications,
                Some(group),
                r,
            );
            true
        }
        "cas_main_director" | "cas_secondary_director" => {
            component_critical(
                w,
                t,
                identity,
                ComponentKind::Director,
                Some(if kind == "cas_main_director" {
                    "main"
                } else {
                    "secondary"
                }),
                r,
            );
            true
        }
        "cas_main_mount" | "cas_secondary_mount" | "cas_other_weapon" => {
            weapon_critical(
                w,
                t,
                h,
                Some(if kind == "cas_main_mount" {
                    "main"
                } else if kind == "cas_secondary_mount" {
                    "secondary"
                } else {
                    "other"
                }),
                None,
                r,
                identity,
            );
            true
        }
        "cas_main_magazine" => {
            record(w, t.id, identity, kind, None, "resolved");
            record_catastrophic_loss(w,t.id,identity);
            if let Some(e) = w.entity_mut(t.id) {
                e.hp = 0.;
                e.ext.insert("damage.instant_sink".into(), 1.);
            }
            true
        }
        "cas_secondary_magazine" | "cas_other_explosion" => {
            cas_explosion(w, t, h, kind, r, identity);
            true
        }
        "cas_light_weapons" => {
            let n = roll(w, t.id, r, 6, identity, "cas_light_weapon_count") / 2;
            for a in 0..n {
                weapon_critical(
                    w,
                    t,
                    h,
                    Some("light"),
                    None,
                    r,
                    &format!("{identity}:light:{a}"),
                );
            }
            record(w, t.id, identity, kind, None, "resolved");
            true
        }
        _ => false,
    }
}
fn component_critical(
    w: &mut World,
    t: &Entity,
    identity: &str,
    kind: ComponentKind,
    group: Option<&str>,
    r: &mut Rng,
) {
    let choices: Vec<_> = fit(w, t.id)
        .map(|f| {
            f.components
                .iter()
                .filter(|(_, c)| {
                    c.kind == kind && group.is_none_or(|g| c.cas_group.as_deref() == Some(g))
                })
                .map(|(k, c)| (k.clone(), c.clone()))
                .collect()
        })
        .unwrap_or_default();
    if choices.is_empty() {
        record(
            w,
            t.id,
            identity,
            "absent_component",
            None,
            "ignored_not_fitted",
        );
        return;
    }
    let (id, c) = choices[index(w, t.id, r, choices.len(), identity, "physical_component")].clone();
    let already = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&t.id))
        .is_some_and(|u| u.disabled.contains(&id));
    record(
        w,
        t.id,
        identity,
        "component",
        Some(id.clone()),
        if already {
            "ignored_already_destroyed"
        } else {
            "resolved"
        },
    );
    if already {
        return;
    }
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        s.units.entry(t.id).or_default().disabled.insert(id.clone());
    }
    if c.kind == ComponentKind::Sensor {
        if let (Some(e), Some(sensor)) = (w.entity_mut(t.id), c.binding.as_deref()) {
            e.ext.insert(format!("damage.sensor.{sensor}"), 1.);
        }
    }
    super::repairs2026::record_system(w, t.id, identity, "component", Some(&id), 1);
    private_trace(
        w,
        t.id,
        "14.5.component_disabled",
        json!({"identity":identity,"component":id,"kind":c.kind,"source":c.source}),
    );
}
fn sensor_critical(
    w: &mut World,
    t: &Entity,
    h: &dr::Resolved,
    kind: &str,
    r: &mut Rng,
    identity: &str,
) {
    let mut choices: Vec<(String, Option<String>, bool)> = t
        .sensors
        .iter()
        .filter(|s| {
            let d = w.db.sensors.get(&s.sensor);
            (kind != "radar" || d.is_some_and(|d| d.kind == SensorKind::Radar))
                && (kind != "radar"
                    || h.hit
                        .targeted_sensor
                        .as_ref()
                        .is_none_or(|target| target == &s.sensor))
                && (dr::caliber(&h.hit.weapon) > 65.
                    || d.is_none_or(|d| {
                        !matches!(d.kind, SensorKind::SonarActive | SensorKind::SonarPassive)
                    }))
        })
        .map(|s| {
            (
                format!("native.sensor:{}", s.sensor),
                Some(s.sensor.clone()),
                w.db.sensors
                    .get(&s.sensor)
                    .is_some_and(|s| s.kind == SensorKind::Radar),
            )
        })
        .collect();
    if kind != "radar" {
        // SOKS is a real independent non-acoustic component, rather than a
        // fabricated Magnetic/Sonar database receiver. Include its actual
        // installed physical identities in the same casualty/repair ledger.
        for (component,binding) in super::detection2026::soks::physical_sensor_components(w,t.id) {
            choices.push((component,Some(binding),false));
        }
        for (component,binding) in super::detection2026::laser::physical_sensor_components(w,t.id) {
            choices.push((component,Some(binding),false));
        }
        if let Some(f) = fit(w, t.id) {
            for (id, c) in &f.components {
                if matches!(
                    c.kind,
                    ComponentKind::Periscope | ComponentKind::Countermeasure
                ) {
                    choices.push((id.clone(), None, false));
                }
            }
        }
    }
    if choices.is_empty() {
        record(w, t.id, identity, kind, None, "ignored_not_fitted");
        return;
    }
    let (id, sensor, radar) = choices[index(
        w,
        t.id,
        r,
        choices.len(),
        identity,
        "sensor_or_countermeasure",
    )]
    .clone();
    let already = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&t.id))
        .is_some_and(|u| u.disabled.contains(&id));
    record(
        w,
        t.id,
        identity,
        kind,
        Some(id.clone()),
        if already {
            "ignored_already_destroyed"
        } else {
            "resolved"
        },
    );
    if already {
        return;
    }
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        s.units.entry(t.id).or_default().disabled.insert(id.clone());
    }
    if let Some(sensor) = sensor {
        if let Some(e) = w.entity_mut(t.id) {
            e.ext.insert(format!("damage.sensor.{sensor}"), 1.);
            if radar {
                e.ext.insert("damage.arm_radar_hit".into(), 1.);
            }
        }
    }
    super::repairs2026::record_system(w, t.id, identity, "component", Some(&id), 1);
    private_trace(
        w,
        t.id,
        "14.5.sensor_disabled",
        json!({"identity":identity,"component":id,"radar":radar}),
    );
}

fn weapon_critical(
    w: &mut World,
    t: &Entity,
    h: &dr::Resolved,
    group: Option<&str>,
    d100: Option<u8>,
    r: &mut Rng,
    identity: &str,
) {
    let fitted = fit(w, t.id).cloned().unwrap_or_default();
    if group.is_none()
        && t.weapons.iter().all(|m| m.mounts == 0)
        && matches!(dr::role(t), "merchant" | "small_cargo" | "landing")
    {
        cargo_critical(w, t, h, None, r, identity);
        return;
    }
    // English2026 14-9 overrides the translated same-D100 subdivision: choose
    // the physical weapon first, then roll D10 only for its fitted director.
    let _ = d100;
    let mut choices = Vec::<(usize, usize, Option<String>)>::new();
    for (i, m) in t.weapons.iter().enumerate() {
        let original = w
            .rules
            .expanded_damage
            .as_ref()
            .and_then(|s| s.units.get(&t.id))
            .and_then(|u| u.original_mounts.get(&i))
            .copied()
            .unwrap_or(m.mounts);
        if let Some(s) = w.rules.expanded_damage.as_mut() {
            s.units
                .entry(t.id)
                .or_default()
                .original_mounts
                .entry(i)
                .or_insert(original);
        }
        for physical in 0..original as usize {
            let explicit = fitted
                .components
                .iter()
                .find(|(_, c)| {
                    c.kind == ComponentKind::WeaponMount
                        && c.binding.as_deref() == Some(m.weapon.as_str())
                        && c.physical == Some(physical)
                        && c.mount_index.is_none_or(|index| index == i)
                })
                .map(|(id, c)| (id.clone(), c));
            let matches = if let Some(g) = group {
                if g == "light" {
                    w.db.weapons.get(&m.weapon).is_some_and(|d| {
                        matches!(d.kind, WeaponKind::Gun | WeaponKind::Ciws)
                            && dr::caliber(d) <= 25.
                    })
                } else {
                    explicit
                        .as_ref()
                        .is_some_and(|(_, c)| c.cas_group.as_deref() == Some(g))
                }
            } else {
                true
            };
            if matches {
                choices.push((i, physical, explicit.map(|(id, _)| id)));
            }
        }
    }
    // Annex-listed launch gear is eligible only when explicitly fitted. No
    // invented catapult/arrestor count is synthesized from carrier capacity.
    let extras: Vec<_> = if group.is_none() {
        fitted
            .components
            .iter()
            .filter(|(_, c)| {
                matches!(
                    c.kind,
                    ComponentKind::Catapult | ComponentKind::Arrestor | ComponentKind::Elevator
                )
            })
            .map(|(id, c)| (id.clone(), c.clone()))
            .collect()
    } else {
        vec![]
    };
    let parked: Vec<_> = if group.is_none() {
        fitted.parked.keys().cloned().collect()
    } else {
        vec![]
    };
    let count = choices.len() + extras.len() + parked.len();
    if count == 0 {
        record(w, t.id, identity, "weapon", None, "ignored_not_fitted");
        return;
    }
    let selected = index(w, t.id, r, count, identity, "physical_weapon_or_gear");
    if selected >= choices.len() + extras.len() {
        let id = &parked[selected - choices.len() - extras.len()];
        parked_aircraft_hit(w, t, h, &fitted, id, r, identity, &mut BTreeSet::new());
        return;
    }
    if selected >= choices.len() {
        let (id, c) = &extras[selected - choices.len()];
        let already = w
            .rules
            .expanded_damage
            .as_ref()
            .and_then(|s| s.units.get(&t.id))
            .is_some_and(|u| u.disabled.contains(id));
        record(
            w,
            t.id,
            identity,
            "component",
            Some(id.clone()),
            if already {
                "ignored_already_destroyed"
            } else {
                "resolved"
            },
        );
        if !already {
            w.rules
                .expanded_damage
                .as_mut()
                .unwrap()
                .units
                .entry(t.id)
                .or_default()
                .disabled
                .insert(id.clone());
            super::repairs2026::record_system(w, t.id, identity, "component", Some(id), 1);
        }
        private_trace(
            w,
            t.id,
            "14.5.deck_gear",
            json!({"identity":identity,"component":id,"kind":c.kind}),
        );
        return;
    }
    let (i, physical, explicit) = choices[selected].clone();
    let component = explicit
        .clone()
        .unwrap_or_else(|| format!("native.mount:{i}:{physical}"));
    let directors: Vec<_> = fitted
        .components
        .iter()
        .filter(|(_, c)| c.kind == ComponentKind::Director && c.controls.contains(&component))
        .map(|(id, _)| id.clone())
        .collect();
    if !directors.is_empty()
        && group.is_none()
        && roll(w, t.id, r, 10, identity, "weapon_director_or_mount") <= 2
    {
        let director =
            directors[index(w, t.id, r, directors.len(), identity, "fitted_director")].clone();
        let u = w
            .rules
            .expanded_damage
            .as_mut()
            .unwrap()
            .units
            .entry(t.id)
            .or_default();
        let new = u.disabled.insert(director.clone());
        record(
            w,
            t.id,
            identity,
            "director",
            Some(director.clone()),
            if new {
                "resolved"
            } else {
                "ignored_already_destroyed"
            },
        );
        if new {
            super::repairs2026::record_system(w, t.id, identity, "component", Some(&director), 1);
        }
        return;
    }
    let already = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&t.id))
        .is_some_and(|u| u.disabled.contains(&component));
    record(
        w,
        t.id,
        identity,
        "weapon",
        Some(component.clone()),
        if already {
            "ignored_already_destroyed"
        } else {
            "resolved"
        },
    );
    if already {
        return;
    }
    let m = &t.weapons[i];
    let live = w.entity(t.id).and_then(|e| e.weapons.get(i)).cloned();
    let Some(live) = live else {
        pending(
            w,
            t.id,
            identity,
            "weapon",
            "original physical mount no longer bound",
        );
        return;
    };
    let c = explicit.as_ref().and_then(|id| fitted.components.get(id));
    let stock = c.and_then(|c| c.ammunition).map(|initial| {
        w.rules
            .expanded_damage
            .as_ref()
            .and_then(|s| s.units.get(&t.id))
            .and_then(|u| u.ammunition.get(&component))
            .copied()
            .unwrap_or(initial)
    });
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        let u = s.units.entry(t.id).or_default();
        u.disabled.insert(component.clone());
        if stock.is_some() {
            u.ammunition.insert(component.clone(), 0);
        }
    }
    let stock_conflict = stock.is_some_and(|n| n > live.ammo);
    if let Some(e) = w.entity_mut(t.id) {
        let mount = &mut e.weapons[i];
        mount.mounts = mount.mounts.saturating_sub(1);
        if let Some(stock) = stock.filter(|_| !stock_conflict) {
            mount.ammo -= stock;
        }
        e.ext.insert(format!("damage.mount.{i}.{physical}"), 1.);
    }
    if stock_conflict {
        pending(
            w,
            t.id,
            identity,
            "weapon_ammunition",
            "physical ammunition exceeds actual aggregate; source allocation missing",
        );
    }
    super::repairs2026::record_system(w, t.id, identity, "component", Some(&component), 1);
    let explosive = w.db.weapons.get(&m.weapon).is_some_and(|d| {
        !matches!(d.kind, WeaponKind::Gun | WeaponKind::Ciws) || dr::caliber(d) >= 20.
    });
    if explosive && stock.is_none() && live.ammo > 0 {
        pending(
            w,
            t.id,
            identity,
            "weapon_ammunition",
            "mount ammunition allocation is not explicitly sourced; no aggregate division",
        );
    }
    let burst = explosive
        && stock.is_some_and(|n| n > 0)
        && !h.airburst
        && dr::val(w.entity(t.id).unwrap(), "damage.magazines_flooded") == 0.
        && roll(w, t.id, r, 10, identity, "mount_ammunition") == 10;
    if burst {
        match c.and_then(|c| c.location.as_deref()) {
            Some("belowdecks") => {
                record_catastrophic_loss(w,t.id,identity);
                if let Some(e) = w.entity_mut(t.id) {
                    e.hp = 0.;
                    e.ext.insert("damage.instant_sink".into(), 1.);
                }
            }
            Some("topside") => {
                let dp =
                    w.db.weapons
                        .get(&m.weapon)
                        .map(|d| d.damage * (stock.unwrap() as f64 / 2.).floor().max(1.))
                        .unwrap_or(0.);
                dr::queue_secondary(w, t, dp, false, format!("{identity}:mount_explosion"));
            }
            _ if t.kind == PlatformKind::Sub => {
                record_catastrophic_loss(w,t.id,identity);
                if let Some(e) = w.entity_mut(t.id) {
                    e.hp = 0.;
                }
            }
            _ => pending(
                w,
                t.id,
                identity,
                "weapon_location",
                "detonated physical magazine needs explicit topside/belowdecks source",
            ),
        }
    }
    private_trace(
        w,
        t.id,
        "14.5.weapon_disabled",
        json!({"identity":identity,"component":component,"weapon":m.weapon,"physical":physical,"ammunition":stock,"detonated":burst,"airburst":h.airburst}),
    );
}

fn cas_explosion(
    w: &mut World,
    t: &Entity,
    h: &dr::Resolved,
    kind: &str,
    r: &mut Rng,
    identity: &str,
) {
    let group = if kind == "cas_secondary_magazine" {
        "secondary"
    } else {
        "other"
    };
    let choices: Vec<_> = fit(w, t.id)
        .map(|f| {
            f.components
                .iter()
                .filter(|(_, c)| {
                    c.kind == ComponentKind::WeaponMount && c.cas_group.as_deref() == Some(group)
                })
                .map(|(id, c)| (id.clone(), c.clone()))
                .collect()
        })
        .unwrap_or_default();
    if choices.is_empty() {
        record(w, t.id, identity, kind, None, "ignored_not_fitted");
        return;
    }
    let (id, c) =
        choices[index(w, t.id, r, choices.len(), identity, "cas_exploding_system")].clone();
    let Some(def) = c
        .binding
        .as_ref()
        .and_then(|id| w.db.weapons.get(id))
        .cloned()
    else {
        pending(
            w,
            t.id,
            identity,
            "cas_ammunition",
            "exact HE/weapon source missing",
        );
        return;
    };
    let dp = if group == "secondary" {
        def.damage * 4.
    } else {
        let Some(stock) = c.ammunition else {
            pending(
                w,
                t.id,
                identity,
                "cas_ammunition",
                "explosive physical stock missing",
            );
            return;
        };
        def.damage * (stock as f64 / 2.).floor().max(1.)
    };
    if !h.airburst && dr::val(w.entity(t.id).unwrap(), "damage.magazines_flooded") == 0. {
        dr::queue_secondary(w, t, dp, false, format!("{identity}:cas_explosion"));
    }
    record(w, t.id, identity, kind, Some(id), "resolved");
}

fn cargo_critical(
    w: &mut World,
    t: &Entity,
    h: &dr::Resolved,
    selected: Option<&str>,
    r: &mut Rng,
    identity: &str,
) {
    let Some(f) = fit(w, t.id).cloned() else {
        pending(
            w,
            t.id,
            identity,
            "cargo",
            "cargo identity, type and actual amount not declared",
        );
        return;
    };
    let choices: Vec<_> = f
        .holds
        .keys()
        .filter(|id| selected.is_none_or(|s| s == id.as_str()))
        .cloned()
        .collect();
    if choices.is_empty() {
        record(w, t.id, identity, "cargo", None, "ignored_not_fitted");
        return;
    }
    let id = choices[index(w, t.id, r, choices.len(), identity, "cargo_hold")].clone();
    let spec = &f.holds[&id];
    let prior = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&t.id))
        .and_then(|u| u.holds.get(&id))
        .cloned()
        .unwrap_or(HoldState {
            amount: spec.amount,
            fire: false,
            flooded: false,
            exploded: false,
            explosion_checked: BTreeSet::new(),
            fire_identity: None,
            fire_identities: BTreeSet::new(),
        });
    if prior.flooded || prior.exploded {
        record(w, t.id, identity, "cargo", Some(id), "ignored_already_lost");
        return;
    }
    let mut after = prior.clone();
    let mut burst = false;
    let mut severity_add = 0.;
    if !matches!(dr::size(t), "F" | "G") {
        match spec.kind {
            CargoKind::Ammunition => {
                let d = roll(w, t.id, r, 10, identity, "ammunition_cargo");
                if d >= 8 {
                    burst = true;
                    after.exploded = true;
                    after.amount = 0.;
                } else {
                    let pct =
                        roll(w, t.id, r, 100, identity, "cargo_ammunition_loss") as f64 / 100.;
                    after.amount *= 1. - pct;
                    if d >= 3 {
                        after.fire = true;
                        severity_add = 1.;
                    }
                }
            }
            CargoKind::CrudeOil | CargoKind::RefinedOil | CargoKind::AviationFuel => {
                after.fire = true;
                severity_add = match spec.kind {
                    CargoKind::CrudeOil => 1.,
                    CargoKind::RefinedOil => 2.,
                    _ => 3.,
                };
            }
            CargoKind::Troops => {
                let d = roll(w, t.id, r, 10, identity, "troop_cargo_loss") as usize;
                after.amount = (after.amount
                    - h.dp
                        * tables()["cargo"]["troops_d10_loss_multipliers"][d - 1]
                            .as_f64()
                            .unwrap())
                .max(0.);
            }
            CargoKind::General => {
                after.amount = (after.amount
                    - h.dp * tables()["cargo"]["general_tons_per_dp"].as_f64().unwrap())
                .max(0.)
            }
            CargoKind::Vehicles => {
                after.amount = (after.amount
                    - (h.dp * tables()["cargo"]["vehicles_per_dp"].as_f64().unwrap()).floor())
                .max(0.)
            }
            CargoKind::Aircraft => {
                after.amount = (after.amount
                    - (h.dp * tables()["cargo"]["aircraft_per_dp"].as_f64().unwrap()).floor())
                .max(0.)
            }
        }
    }
    if matches!(dr::size(t), "F" | "G") {
        after.amount = prior.amount * 0.75;
        after.fire = false;
        after.exploded = false;
        burst = false;
        severity_add = 0.;
    }
    if after.fire {
        let fire_identity = format!("{identity}:hold:{}:{id}", id.len());
        let raw = roll(w, t.id, r, 6, identity, "cargo_fire_severity") as f64
            + severity_add
            + if dr::val(t, "damage.personnel") >= 3. {
                2.
            } else {
                0.
            };
        let severity = raw
            * if h.penetrated { 1. } else { 0.5 }
            * if dr::caliber(&h.hit.weapon) <= 65. {
                0.5
            } else {
                1.
            };
        dr::add_source_casualty(w, t.id, &fire_identity, false, severity, 3 * dr::tt());
        after.fire_identity = Some(fire_identity.clone());
        after.fire_identities.insert(fire_identity);
        if let Some(e) = w.entity_mut(t.id) {
            e.ext
                .insert("damage.cargo_fire_modifier".into(), severity_add);
        }
        dr::sync_casualties(w, t.id);
    }
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        s.units
            .entry(t.id)
            .or_default()
            .holds
            .insert(id.clone(), after.clone());
    }
    record(w, t.id, identity, "cargo", Some(id.clone()), "resolved");
    private_trace(
        w,
        t.id,
        "14.1.3.cargo_damage",
        json!({"identity":identity,"hold":id,"type":spec.kind,"before":prior.amount,"after":after.amount,"exploded":burst,"fire":after.fire,"actual_dp":h.dp}),
    );
    if burst {
        cargo_explosion(
            w,
            t,
            &f,
            &id,
            prior.amount,
            r,
            identity,
            &mut BTreeSet::new(),
        );
    }
}
fn cargo_explosion(
    w: &mut World,
    t: &Entity,
    f: &UnitFit,
    id: &str,
    tons: f64,
    r: &mut Rng,
    identity: &str,
    visited: &mut BTreeSet<String>,
) {
    if !visited.insert(id.into()) {
        return;
    }
    // The printed table gives nearby-ship damage, but does not give a carrier
    // self-damage formula. Do not turn an unknown formula into an instant kill.
    pending(
        w,
        t.id,
        identity,
        "cargo_self_explosion",
        "source specifies nearby damage; carrier self-damage not specified by this cargo row",
    );
    let victims: Vec<_> = w
        .entities
        .values()
        .filter(|e| {
            e.alive
                && e.id != t.id
                && matches!(e.kind, PlatformKind::Ship | PlatformKind::Sub)
                && e.depth_m == 0.
        })
        .filter_map(|e| {
            let yards = e.pos.range_to(t.pos) * 2025.371828521435;
            tables()["cargo"]["ammunition_nearby_dp_yards_divisor"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| yards <= row[0].as_f64().unwrap())
                .map(|row| (e.clone(), tons / row[1].as_f64().unwrap()))
        })
        .collect();
    for (v, dp) in victims {
        dr::queue_secondary(
            w,
            &v,
            dp,
            false,
            format!("{identity}:cargo_neighbor:{}", v.id.0),
        );
    }
    if let Some(spec) = f.holds.get(id) {
        for next in &spec.adjacent {
            if visited.contains(next) {
                continue;
            }
            let Some(n) = f.holds.get(next) else { continue };
            if n.kind != CargoKind::Ammunition {
                continue;
            }
            let old = w
                .rules
                .expanded_damage
                .as_ref()
                .and_then(|s| s.units.get(&t.id))
                .and_then(|u| u.holds.get(next))
                .cloned()
                .unwrap_or(HoldState {
                    amount: n.amount,
                    fire: false,
                    flooded: false,
                    exploded: false,
                    explosion_checked: BTreeSet::new(),
                    fire_identity: None,
                    fire_identities: BTreeSet::new(),
                });
            if old.flooded || old.exploded {
                continue;
            }
            if roll(w, t.id, r, 100, identity, "adjacent_cargo_explosion") <= 70 {
                if let Some(s) = w.rules.expanded_damage.as_mut() {
                    s.units.entry(t.id).or_default().holds.insert(
                        next.clone(),
                        HoldState {
                            amount: 0.,
                            exploded: true,
                            ..old.clone()
                        },
                    );
                }
                cargo_explosion(w, t, f, next, old.amount, r, identity, visited);
            }
        }
    }
}

/// Exact native expenditure hook. The selected physical stock is an explicit
/// assistance policy (ordered component IDs), visible to its owner and never a
/// probability rule. A physical manual choice can call spend_component first.
pub fn record_weapon_expenditure(w: &mut World, id: EntityId, weapon: &str, count: u16) {
    if !full(w) || count == 0 {
        return;
    }
    let Some(f) = fit(w, id).cloned() else { return };
    let Some(s) = w.rules.expanded_damage.as_mut() else {
        return;
    };
    let u = s.units.entry(id).or_default();
    let mut remaining = count;
    for (key, c) in &f.components {
        if c.kind != ComponentKind::WeaponMount
            || c.binding.as_deref() != Some(weapon)
            || u.disabled.contains(key)
        {
            continue;
        }
        if let Some(stock) = c.ammunition {
            let v = u.ammunition.entry(key.clone()).or_insert(stock);
            let n = remaining.min(*v);
            *v -= n;
            remaining -= n;
            if remaining == 0 {
                break;
            }
        }
    }
    if remaining > 0
        && f.components
            .values()
            .any(|c| c.kind == ComponentKind::WeaponMount && c.binding.as_deref() == Some(weapon))
    {
        u.pending
            .entry("weapon_ammunition".into())
            .or_insert("native expenditure exceeds explicitly allocated physical stock".into());
    }
}
fn operational(w: &World, id: EntityId, component: &str) -> Result<bool, String> {
    let Some(u) = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
    else {
        return Ok(true);
    };
    if u.pending.contains_key(component) {
        return Err("expanded damage component source pending".into());
    }
    Ok(!u.disabled.contains(component))
}
pub fn equipment_operational(w: &World, id: EntityId, component: &str) -> Result<bool, String> {
    operational(w, id, component)
}
pub fn binding_operational(
    w: &World,
    id: EntityId,
    kind: ComponentKind,
    binding: &str,
) -> Result<bool, String> {
    let Some(f) = fit(w, id) else { return Ok(true) };
    for (component, c) in &f.components {
        if c.kind == kind
            && (component == binding || c.binding.as_deref() == Some(binding))
            && !operational(w, id, component)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}
pub fn engineering_available(w: &World, id: EntityId) -> Result<bool, String> {
    let Some(f) = fit(w, id) else { return Ok(true) };
    for (component, c) in &f.components {
        if c.kind == ComponentKind::Engineering && !operational(w, id, component)? {
            return Ok(false);
        }
    }
    Ok(true)
}
pub fn sensor_operational(w: &World, id: EntityId, sensor: &str) -> Result<bool, String> {
    if !full(w) {
        return Ok(true);
    }
    if !aircraft_profile(w, id, 0., false)?.sensors {
        return Ok(false);
    }
    if w.entity(id)
        .is_some_and(|e| dr::val(e, "damage.power_lost") > w.tick as f64)
    {
        return Ok(false);
    }
    if !operational(w, id, &format!("native.sensor:{sensor}"))? {
        return Ok(false);
    }
    let f = fit(w, id);
    let cic = w.entity(id).is_some_and(|e| dr::val(e, "damage.cic") > 0.);
    let navigation = f.is_some_and(|f| {
        f.components.values().any(|c| {
            c.kind == ComponentKind::Sensor && c.binding.as_deref() == Some(sensor) && c.navigation
                && w.db.sensors.get(sensor).is_some_and(|d|d.kind == SensorKind::Radar)
        })
    });
    if cic && !navigation {
        return Ok(false);
    }
    let Some(f) = f else { return Ok(true) };
    for (key, c) in &f.components {
        if c.kind == ComponentKind::Sensor
            && c.binding.as_deref() == Some(sensor)
            && !operational(w, id, key)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}
/// Shared hard stops, before checking the individual physical weapon. Full
/// permits a source-fitted local manual autocannon during electrical failure;
/// the per-weapon gate still denies every unfitted or powered weapon.
pub fn platform_weapons_operational(w: &World, id: EntityId) -> bool {
    let Some(e) = w.entity(id) else { return false };
    if !full(w) {
        return dr::weapons_operational(e);
    }
    e.alive
        && !dr::massive(e)
        && dr::val(e, "damage.doomed") == 0.
        && dr::val(e, "damage.personnel") < 4.
        && dr::val(e, "damage.dc_weapons_crew") == 0.
}
pub fn weapon_operational(w: &World, id: EntityId, weapon: &str) -> Result<bool, String> {
    if !full(w) {
        return Ok(true);
    }
    if !platform_weapons_operational(w, id) {
        return Ok(false);
    }
    if !aircraft_profile(w, id, 0., false)?.offensive {
        return Ok(false);
    }
    let Some(e) = w.entity(id) else {
        return Ok(false);
    };
    if w.rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .is_some_and(|u| u.pending.contains_key("personnel_allocation"))
    {
        return Err("choose the required actual abandoned weapon before firing".into());
    }
    let Some(f) = fit(w, id) else {
        return Ok(dr::weapons_operational(e));
    };
    let cic = dr::val(e, "damage.cic") > 0.;
    let power = dr::val(e, "damage.power_lost") > w.tick as f64;
    let fitting: Vec<_> = f
        .components
        .iter()
        .filter(|(_, c)| {
            c.kind == ComponentKind::WeaponMount && c.binding.as_deref() == Some(weapon)
        })
        .collect();
    if fitting.is_empty() {
        return Ok(dr::weapons_operational(e));
    }
    for (key, c) in fitting {
        if !operational(w, id, key)? {
            continue;
        }
        if cic && !c.local_control {
            continue;
        }
        if power
            && !(c.local_control
                && c.manually_operated
                && w.db
                    .weapons
                    .get(weapon)
                    .is_some_and(|d| matches!(d.kind, WeaponKind::Gun | WeaponKind::Ciws)))
        {
            continue;
        }
        let director_lost = f
            .components
            .iter()
            .filter(|(_, d)| {
                d.kind == ComponentKind::Director && d.controls.iter().any(|m| m == key)
            })
            .any(|(d, _)| operational(w, id, d).ok() == Some(false));
        if director_lost && !c.local_control {
            continue;
        }
        return Ok(!dr::massive(e)
            && dr::val(e, "damage.doomed") == 0.
            && dr::val(e, "damage.personnel") < 4.
            && dr::val(e, "damage.dc_weapons_crew") == 0.);
    }
    Ok(false)
}
pub fn operational_mount_count(w: &World, id: EntityId, weapon: &str) -> Result<u8, String> {
    if !weapon_operational(w, id, weapon)? {
        return Ok(0);
    }
    let Some(e) = w.entity(id) else { return Ok(0) };
    let mut count = 0usize;
    let u = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id));
    for (i, m) in e
        .weapons
        .iter()
        .enumerate()
        .filter(|(_, m)| m.weapon == weapon)
    {
        let original = u
            .and_then(|u| u.original_mounts.get(&i))
            .copied()
            .unwrap_or(m.mounts);
        for p in 0..original {
            let native = format!("native.mount:{i}:{p}");
            let explicit = fit(w, id)
                .and_then(|f| {
                    f.components.iter().find(|(_, c)| {
                        c.kind == ComponentKind::WeaponMount
                            && c.binding.as_deref() == Some(weapon)
                            && c.physical == Some(p as usize)
                            && c.mount_index.is_none_or(|index| index == i)
                    })
                })
                .map(|(key, _)| key.as_str())
                .unwrap_or(&native);
            if !u.is_some_and(|u| u.disabled.contains(explicit) || u.abandoned.contains(explicit)) {
                count += 1
            }
        }
    }
    Ok(count.min(u8::MAX as usize) as u8)
}
fn physical_weapon_stock_entry(
    w: &World,
    id: EntityId,
    weapon: &str,
    mount_index: usize,
    physical: usize,
) -> Result<Option<(String, u16)>, String> {
    if !full(w) {
        return Ok(None);
    }
    let e = w.entity(id).ok_or("physical ammunition owner absent")?;
    let mount = e
        .weapons
        .get(mount_index)
        .ok_or("physical ammunition mount absent")?;
    if mount.weapon != weapon {
        return Err("physical ammunition weapon/mount mismatch".into());
    }
    let original = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .and_then(|u| u.original_mounts.get(&mount_index))
        .copied()
        .unwrap_or(mount.mounts);
    if physical >= usize::from(original) {
        return Err("physical ammunition launcher index absent".into());
    }
    let Some(f) = fit(w, id) else { return Ok(None) };
    let bindings: Vec<_> = f
        .components
        .iter()
        .filter(|(_, c)| {
            c.kind == ComponentKind::WeaponMount && c.binding.as_deref() == Some(weapon)
        })
        .collect();
    if bindings.is_empty() {
        return Ok(None);
    }
    // A legacy component may omit mount_index when this weapon has exactly
    // one aggregate mount row; the named component and physical index still
    // identify its inventory without guessing between rows.
    let unique_weapon_row = e.weapons.iter().filter(|m|m.weapon==weapon).count()==1;
    let matches: Vec<_> = bindings
        .into_iter()
        .filter(|(_, c)| (c.mount_index == Some(mount_index) || (c.mount_index.is_none() && unique_weapon_row)) && c.physical == Some(physical))
        .collect();
    if matches.len() != 1 {
        return Err(
            "physical ammunition requires one exact weapon/mount_index/physical component binding"
                .into(),
        );
    }
    let (key, c) = matches[0];
    if !source(&c.source) {
        return Err("physical ammunition component source absent".into());
    }
    let stock = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .and_then(|u| u.ammunition.get(key))
        .copied()
        .or(c.ammunition)
        .ok_or("physical ammunition allocation absent")?;
    Ok(Some((key.clone(), stock)))
}
/// Read the selected physical launcher only; never divide aggregate stock or
/// select another component by identifier order.
pub fn physical_weapon_stock(
    w: &World,
    id: EntityId,
    weapon: &str,
    mount_index: usize,
    physical: usize,
) -> Result<Option<u16>, String> {
    physical_weapon_stock_entry(w, id, weapon, mount_index, physical).map(|v| v.map(|(_, n)| n))
}
/// The caller commits aggregate ammunition separately, after all preflights.
/// Missing unrelated component metadata is a no-op; partial/ambiguous binding
/// is source-blocked rather than charged to another physical launcher.
pub fn spend_physical_weapon_stock(
    w: &mut World,
    id: EntityId,
    weapon: &str,
    mount_index: usize,
    physical: usize,
    count: u16,
) -> Result<(), String> {
    let available = physical_weapon_stock(w, id, weapon, mount_index, physical)?;
    let Some(stock) = available else {
        return Ok(());
    };
    if stock < count {
        return Err("selected physical weapon ammunition exhausted".into());
    }
    let (key, confirmed) = physical_weapon_stock_entry(w, id, weapon, mount_index, physical)?
        .ok_or("physical ammunition binding disappeared")?;
    if confirmed != stock {
        return Err("physical ammunition changed during commit".into());
    }
    w.rules
        .expanded_damage
        .as_mut()
        .ok_or("physical ammunition state absent")?
        .units
        .entry(id)
        .or_default()
        .ammunition
        .insert(key, stock - count);
    Ok(())
}
/// Transfer from an independently debited reserve into one exact physical
/// launcher. Capacity and reserve gates belong to the actual loading producer.
pub fn credit_physical_weapon_stock(w:&mut World,id:EntityId,weapon:&str,mount_index:usize,physical:usize,count:u16)->Result<(),String>{
    let Some((key,stock))=physical_weapon_stock_entry(w,id,weapon,mount_index,physical)? else{return Ok(())};
    let after=stock.checked_add(count).ok_or("physical ammunition counter overflow")?;
    w.rules.expanded_damage.as_mut().ok_or("physical ammunition state absent")?.units.entry(id).or_default().ammunition.insert(key,after);
    Ok(())
}
/// Preserve physical identities when a casualty reduces the aggregate count.
/// A surviving mount1 must not be re-labelled as destroyed mount0.
pub fn physical_weapon_mounts(
    w: &World,
    id: EntityId,
    mount_index: usize,
) -> Result<Vec<usize>, String> {
    let e = w.entity(id).ok_or("physical mount owner absent")?;
    let m = e.weapons.get(mount_index).ok_or("physical mount absent")?;
    if !full(w) {
        return Ok((0..usize::from(m.mounts)).collect());
    }
    if !weapon_operational(w, id, &m.weapon)? {
        return Ok(vec![]);
    }
    let u = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id));
    let count = u
        .and_then(|u| u.original_mounts.get(&mount_index))
        .copied()
        .unwrap_or(m.mounts);
    let mut alive = vec![];
    for physical in 0..usize::from(count) {
        let native = format!("native.mount:{mount_index}:{physical}");
        let binding = fit(w, id).and_then(|f| {
            f.components.iter().find(|(_, c)| {
                c.kind == ComponentKind::WeaponMount
                    && c.binding.as_deref() == Some(m.weapon.as_str())
                    && c.mount_index.is_none_or(|i| i == mount_index)
                    && c.physical == Some(physical)
            })
        });
        let key = binding.map(|(k, _)| k.as_str()).unwrap_or(native.as_str());
        if !operational(w, id, key)? || u.is_some_and(|u| u.abandoned.contains(key)) {
            continue;
        }
        if let Some((_, c)) = binding {
            if dr::val(e, "damage.cic") > 0. && !c.local_control {
                continue;
            }
            if dr::val(e, "damage.power_lost") > w.tick as f64
                && !(c.local_control
                    && c.manually_operated
                    && w.db
                        .weapons
                        .get(&m.weapon)
                        .is_some_and(|d| matches!(d.kind, WeaponKind::Gun | WeaponKind::Ciws)))
            {
                continue;
            }
            if !c.local_control
                && fit(w, id).is_some_and(|f| {
                    f.components.iter().any(|(d, s)| {
                        s.kind == ComponentKind::Director
                            && s.controls.iter().any(|m| m == key)
                            && operational(w, id, d).ok() == Some(false)
                    })
                })
            {
                continue;
            }
        }
        alive.push(physical);
    }
    Ok(alive)
}
pub fn vtol_launch_fit<'a>(w:&'a World,id:EntityId,class:&str)->Option<&'a VtolLaunch> {
    if !full(w) || class!="us_f35b" {return None;}
    let f=fit(w,id)?.vtol_launch.as_ref()?;
    let e=w.entity(id)?;
    let key=if f.zone=="amidships" {"damage.deck_waist"} else {"damage.deck_recover"};
    if dr::val(e,key)>0. {return None;}
    Some(f)
}
/// 14.5 carrier bridge casualty interrupts launch and recovery for its actual clock.
pub fn bridge_flight_operational(w:&World,id:EntityId)->bool {
    let Some(e)=w.entity(id) else {return false};
    !(full(w) && dr::aviation(e) && dr::val(e,"damage.bridge_until") > w.tick as f64)
}
pub fn flight_launch_operational(w:&World,id:EntityId,class:&str)->bool {
    let Some(e)=w.entity(id) else {return false};
    if !bridge_flight_operational(w,id) {return false;}
    if dr::flight_operational(e,true,w.tick) {return true;}
    if vtol_launch_fit(w,id,class).is_none() {return false;}
    let mut copy=e.clone(); copy.ext.remove("damage.deck_launch");
    dr::flight_operational(&copy,true,w.tick)
}
pub fn flight_gear_operational(w: &World, id: EntityId, launch: bool) -> Result<bool, String> {
    let Some(f) = fit(w, id) else { return Ok(true) };
    let k = if launch {
        ComponentKind::Catapult
    } else {
        ComponentKind::Arrestor
    };
    let gear: Vec<_> = f.components.iter().filter(|(_, c)| c.kind == k).collect();
    if gear.is_empty() {
        return Ok(true);
    }
    for (component, _) in gear {
        if operational(w, id, component)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Post-battle successful component repair restores functionality, not spent
/// ammunition, cargo, destroyed aircraft, structural DP or erased source dice.
pub fn restore_component(w: &mut World, id: EntityId, component: &str) -> Result<(), String> {
    let explicit = fit(w, id)
        .and_then(|f| f.components.get(component))
        .cloned();
    let disabled = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .is_some_and(|u| u.disabled.contains(component));
    if !disabled {
        return Ok(());
    }
    let mount = if let Some(c) = &explicit {
        if c.kind == ComponentKind::WeaponMount {
            let weapon = c.binding.as_deref().ok_or("repair mount source absent")?;
            let i = if let Some(i) = c.mount_index {
                if w.entity(id)
                    .and_then(|e| e.weapons.get(i))
                    .is_none_or(|m| m.weapon != weapon)
                {
                    return Err("repair exact mount binding changed".into());
                }
                i
            } else {
                let choices: Vec<_> = w
                    .entity(id)
                    .ok_or("repair unit absent")?
                    .weapons
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.weapon == weapon)
                    .map(|(i, _)| i)
                    .collect();
                if choices.len() != 1 {
                    return Err("repair mount binding ambiguous".into());
                }
                choices[0]
            };
            Some((i, c.physical.ok_or("repair physical mount absent")?))
        } else {
            None
        }
    } else if let Some(raw) = component.strip_prefix("native.mount:") {
        let mut p = raw.split(':');
        Some((
            p.next()
                .and_then(|n| n.parse().ok())
                .ok_or("repair mount index invalid")?,
            p.next()
                .and_then(|n| n.parse().ok())
                .ok_or("repair physical index invalid")?,
        ))
    } else {
        None
    };
    if let Some((i, p)) = mount {
        let e = w.entity_mut(id).ok_or("repair unit absent")?;
        let m = e.weapons.get_mut(i).ok_or("repair mount absent")?;
        m.mounts = m
            .mounts
            .checked_add(1)
            .ok_or("repair mount count overflow")?;
        e.ext.remove(&format!("damage.mount.{i}.{p}"));
    }
    let sensor = component
        .strip_prefix("native.sensor:")
        .map(str::to_string)
        .or_else(|| {
            explicit
                .as_ref()
                .filter(|c| c.kind == ComponentKind::Sensor)
                .and_then(|c| c.binding.clone())
        });
    if let Some(sensor) = sensor {
        if let Some(e) = w.entity_mut(id) {
            e.ext.remove(&format!("damage.sensor.{sensor}"));
        }
    }
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        s.units.entry(id).or_default().disabled.remove(component);
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AircraftProjection {
    pub maneuver_rating: f64,
    pub cruise_only: bool,
    pub offensive: bool,
    pub sensors: bool,
    pub countermeasures: bool,
    pub jettison: bool,
}
fn aircraft_damage_active(w: &World) -> bool {
    full(w)
        && w.rules
            .expanded_damage
            .as_ref()
            .is_some_and(|s| s.config.optional.aircraft_damage != AircraftMode::Off)
}
pub fn aircraft_profile(
    w: &World,
    id: EntityId,
    fitted_mr: f64,
    full_load: bool,
) -> Result<AircraftProjection, String> {
    if !fitted_mr.is_finite() || fitted_mr < 0. {
        return Err("aircraft MR source invalid".into());
    }
    let mut p = AircraftProjection {
        maneuver_rating: fitted_mr,
        cruise_only: false,
        offensive: true,
        sensors: true,
        countermeasures: true,
        jettison: true,
    };
    if !aircraft_damage_active(w) {
        return Ok(p);
    }
    let Some(a) = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .and_then(|u| u.aircraft.as_ref())
    else {
        return Ok(p);
    };
    if a.repaired {
        return Ok(p);
    }
    match a.severity {
        AirSeverity::Destroyed => {
            p.offensive = false;
            p.sensors = false;
            p.countermeasures = false;
            p.jettison = false;
            p.maneuver_rating = 0.;
        }
        AirSeverity::Critical => {
            p.cruise_only = true;
            p.offensive = false;
            p.sensors = false;
            p.countermeasures = false;
            p.maneuver_rating = if full_load { 0. } else { 0.5 };
        }
        AirSeverity::Heavy => {
            p.cruise_only = true;
            p.offensive = false;
            p.sensors = false;
            p.maneuver_rating = (fitted_mr * 0.5).max(0.5);
        }
        AirSeverity::Light => {
            for k in &a.light {
                match k.as_str() {
                    "pilot" => {
                        p.offensive = false;
                        p.maneuver_rating *= 2. / 3.;
                    }
                    "engine" => {
                        p.cruise_only = true;
                        p.maneuver_rating *= 0.5;
                    }
                    "structure" => {
                        p.cruise_only = true;
                        p.maneuver_rating = 0.5;
                    }
                    "offensive_weapons" => {
                        p.offensive = false;
                        p.jettison = a.jettison_failed != Some(true);
                    }
                    _ => return Err("aircraft light damage source pending".into()),
                }
            }
        }
    }
    Ok(p)
}

/// Called once for each committed native aircraft hit, before the generic
/// aircraft kill path. Returns false only when the optional rule is off.
pub fn aircraft_hit(w: &mut World, id: EntityId, identity: &str) -> bool {
    if !full(w) {
        return false;
    }
    let Some(mode) = w
        .rules
        .expanded_damage
        .as_ref()
        .map(|s| s.config.optional.aircraft_damage)
    else {
        return false;
    };
    if mode == AircraftMode::Off {
        return false;
    }
    if w.rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .and_then(|u| u.aircraft.as_ref())
        .is_some_and(|a| a.hits.contains_key(identity))
    {
        return true;
    }
    if w.entity(id)
        .is_none_or(|e| !e.alive || e.kind != PlatformKind::Aircraft)
    {
        return false;
    }
    let mut r = w
        .rng
        .derive(Rng::hash_tag(w.tick, id.0, &format!("14.9:{identity}")));
    let d = if mode == AircraftMode::PostBattleFiftyFifty {
        0
    } else {
        roll(w, id, &mut r, 10, identity, "aircraft_damage")
    };
    let mut incoming = if d <= 5 {
        AirSeverity::Destroyed
    } else if mode != AircraftMode::ImmediateDetailed {
        AirSeverity::Heavy
    } else if d == 6 {
        AirSeverity::Critical
    } else if d <= 8 {
        AirSeverity::Heavy
    } else {
        AirSeverity::Light
    };
    let light_d6 = if incoming == AirSeverity::Light {
        Some(roll(w, id, &mut r, 6, identity, "aircraft_light_type"))
    } else {
        None
    };
    let light = light_d6.map(|d| {
        tables()["optional_aircraft"]["light_d6"][d as usize - 1]
            .as_str()
            .unwrap()
            .to_string()
    });
    let mut a = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .and_then(|u| u.aircraft.clone())
        .unwrap_or(AircraftDamage {
            severity: incoming,
            light: BTreeSet::new(),
            jettison_failed: None,
            hits: BTreeMap::new(),
            post_battle: mode == AircraftMode::PostBattleFiftyFifty,
            repaired: false,
            active_hits: 0,
        });
    if a.repaired {
        a.light.clear();
        a.jettison_failed = None;
    }
    if a.active_hits > 0 {
        incoming = match (a.severity, incoming) {
            (AirSeverity::Critical | AirSeverity::Destroyed, _)
            | (_, AirSeverity::Destroyed)
            | (AirSeverity::Heavy, AirSeverity::Heavy) => AirSeverity::Destroyed,
            (AirSeverity::Light, AirSeverity::Heavy) | (AirSeverity::Heavy, AirSeverity::Light) => {
                AirSeverity::Critical
            }
            (AirSeverity::Light, AirSeverity::Light)
                if light.as_ref().is_some_and(|l| !a.light.contains(l)) =>
            {
                AirSeverity::Heavy
            }
            (_, AirSeverity::Critical) => AirSeverity::Critical,
            (old, _) => old,
        };
    }
    if mode != AircraftMode::ImmediateDetailed && a.active_hits > 0 {
        incoming = AirSeverity::Destroyed
    }
    if let Some(light) = light {
        if light == "offensive_weapons" && a.jettison_failed.is_none() {
            a.jettison_failed =
                Some(roll(w, id, &mut r, 100, identity, "aircraft_jettison_failure") <= 50)
        }
        a.light.insert(light);
    }
    a.severity = incoming;
    a.repaired = false;
    a.active_hits = a.active_hits.saturating_add(1);
    a.hits.insert(
        identity.into(),
        AirHit {
            occurred_at: w.tick,
            d10: d,
            light_d6,
            result: incoming,
        },
    );
    super::repairs2026::optional_rehit(w,id);
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        let u=s.units.entry(id).or_default();
        if !u.aircraft_repairs.is_empty() {
            u.previous_aircraft_repairs.push(std::mem::take(&mut u.aircraft_repairs));
        }
        u.aircraft = Some(a);
    }
    if incoming == AirSeverity::Destroyed || mode == AircraftMode::PostBattleFiftyFifty {
        if let Some(e) = w.entity_mut(id) {
            e.hp = 0.;
        }
    } else {
        let cruise = fit(w, id).and_then(|f| f.cruise_speed_kts);
        if let Some(e) = w.entity_mut(id) {
            if incoming != AirSeverity::Light || light_d6 != Some(1) && light_d6 != Some(6) {
                if let Some(cruise) = cruise {
                    e.ext.insert("damage.aircraft_cruise_cap".into(), cruise);
                    e.ordered_speed_kts = e.ordered_speed_kts.min(cruise);
                }
            }
        }
    }
    record(w, id, identity, "aircraft_damage", None, "resolved");
    private_trace(
        w,
        id,
        "14.9.aircraft_damage",
        json!({"identity":identity,"d10":d,"light_d6":light_d6,"severity":incoming,"post_battle":mode==AircraftMode::PostBattleFiftyFifty}),
    );
    true
}

/// Native committed fire/flood accounting. These are observations of actual
/// production damage, not caller-authored casualty flags or reconstructed DP.
pub(super) fn committed_secondary(w: &mut World, id: EntityId, dp: f64, flood: bool) {
    if !full(w) || !flood || !dp.is_finite() || dp <= 0. {
        return;
    }
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        s.units.entry(id).or_default().flooding_dp += dp;
    }
}
pub(super) fn record_fire(w: &mut World, id: EntityId, identity: &str) {
    if !full(w)
        || w.entity(id)
            .is_some_and(|e| e.kind == PlatformKind::Aircraft)
    {
        return;
    }
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        s.units
            .entry(id)
            .or_default()
            .fire_history
            .entry(identity.into())
            .or_insert(FireHistory {
                extinguished_at: None,
                checked_hours: BTreeSet::new(),
                restarted: false,
                suppressed: false,
                origin: None,
            });
    }
}
pub(super) fn record_flood_severity(w: &mut World, id: EntityId, severity: f64) {
    if !full(w) {
        return;
    }
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        s.units.entry(id).or_default().source_flood_severity += severity;
    }
}
pub fn source_pending(w: &mut World, id: EntityId, identity: &str, domain: &str, reason: &str) {
    if full(w) {
        pending(w, id, identity, domain, reason)
    }
}
pub(super) fn record_builtin(w: &mut World, id: EntityId, identity: &str, kind: &str) {
    if !full(w) || w.rules.expanded_damage.is_none() {
        return;
    }
    record(w, id, identity, kind, None, "resolved");
    if kind == "personnel" {
        personnel_transfer::casualty(w,id);
        personnel(w, id, identity);
    }
    if matches!(kind, "bridge" | "airplot" | "rudder") {
        if let Some(s) = w.rules.expanded_damage.as_mut() {
            *s.units
                .entry(id)
                .or_default()
                .system_counts
                .entry(kind.into())
                .or_default() += 1;
        }
        super::repairs2026::record_system(w, id, identity, kind, None, 1);
    }
}
fn personnel(w: &mut World, id: EntityId, identity: &str) {
    let Some(e) = w.entity(id).cloned() else {
        return;
    };
    let n = dr::val(&e, "damage.personnel") as u8;
    if n < 2 || n > 3 {
        return;
    }
    if w.rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .is_some_and(|u| u.personnel_choices.contains_key(&n))
    {
        return;
    }
    if fit(w, id).is_some_and(|f| f.manual_personnel_allocation) {
        pending(
            w,
            id,
            identity,
            "personnel_allocation",
            "select the actual weapon required by the current personnel loss",
        );
        return;
    }
    let choices = personnel_candidates(w, id, n);
    if let Some(choice) = choices.first() {
        let _ = personnel_choice(w, id, choice);
    } else {
        record(
            w,
            id,
            identity,
            "personnel_abandon",
            None,
            "ignored_no_eligible_weapon",
        );
    }
}
fn personnel_candidates(w: &World, id: EntityId, n: u8) -> Vec<String> {
    let Some(e) = w.entity(id) else { return vec![] };
    let u = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id));
    let mut small = vec![];
    let mut large = vec![];
    for (i, m) in e.weapons.iter().enumerate() {
        let Some(d) =
            w.db.weapons
                .get(&m.weapon)
                .filter(|d| matches!(d.kind, WeaponKind::Gun | WeaponKind::Ciws))
        else {
            continue;
        };
        let cal = dr::caliber(d);
        for p in 0..m.mounts {
            let key = fit(w, id)
                .and_then(|f| {
                    f.components.iter().find(|(_, c)| {
                        c.kind == ComponentKind::WeaponMount
                            && c.binding.as_deref() == Some(m.weapon.as_str())
                            && c.physical == Some(p as usize)
                            && c.mount_index.is_none_or(|index| index == i)
                    })
                })
                .map(|(key, _)| key.clone())
                .unwrap_or_else(|| format!("native.mount:{i}:{p}"));
            if u.is_some_and(|u| u.abandoned.contains(&key) || u.disabled.contains(&key)) {
                continue;
            }
            if cal <= 20. {
                small.push(key)
            } else if n == 3 && cal >= 23. {
                large.push(key)
            }
        }
    }
    small.extend(large);
    small
}
pub fn personnel_choice(w: &mut World, id: EntityId, component: &str) -> Result<(), String> {
    let n = w
        .entity(id)
        .map(|e| dr::val(e, "damage.personnel") as u8)
        .ok_or("personnel unit absent")?;
    if w.rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .and_then(|u| u.personnel_choices.get(&n))
        .is_some_and(|c| c == component)
    {
        return Ok(());
    }
    if !matches!(n, 2 | 3)
        || !personnel_candidates(w, id, n)
            .iter()
            .any(|key| key == component)
    {
        return Err("selected physical weapon is not legal for this personnel loss".into());
    }
    let u = w
        .rules
        .expanded_damage
        .as_mut()
        .ok_or("expanded state absent")?
        .units
        .entry(id)
        .or_default();
    if let Some(old) = u.personnel_choices.insert(n, component.into()) {
        u.abandoned.remove(&old);
    }
    u.abandoned.insert(component.into());
    u.pending.remove("personnel_allocation");
    Ok(())
}
pub(super) fn deck_critical(
    w: &mut World,
    t: &Entity,
    h: &dr::Resolved,
    location: u8,
    r: &mut Rng,
    identity: &str,
) {
    if !full(w) || w.rules.expanded_damage.is_none() {
        return;
    }
    let zone = if location <= 2 {
        "forward"
    } else if location <= 4 {
        "amidships"
    } else {
        "aft"
    };
    let key = if location <= 2 {
        "damage.deck_launch"
    } else if location <= 4 {
        "damage.deck_waist"
    } else {
        "damage.deck_recover"
    };
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        *s.units
            .entry(t.id)
            .or_default()
            .system_counts
            .entry(key.into())
            .or_default() += 1;
    }
    super::repairs2026::record_system(w, t.id, identity, "flight_deck", Some(key), 1);
    let f = fit(w, t.id).cloned().unwrap_or_default();
    let gear: Vec<_> = f
        .components
        .iter()
        .filter(|(_, c)| {
            matches!(c.kind, ComponentKind::Catapult | ComponentKind::Arrestor)
                && c.zone.as_deref() == Some(zone)
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in gear {
        w.rules
            .expanded_damage
            .as_mut()
            .unwrap()
            .units
            .entry(t.id)
            .or_default()
            .disabled
            .insert(id.clone());
        super::repairs2026::record_system(
            w,
            t.id,
            &format!("{identity}:gear:{id}"),
            "component",
            Some(&id),
            1,
        );
    }
    let parked: Vec<_> = f
        .parked
        .iter()
        .filter(|(_, a)| a.zone == zone)
        .map(|(id, _)| id.clone())
        .collect();
    if !parked.is_empty() {
        let n = roll(w, t.id, r, 6, identity, "deck_parked_count") as usize;
        let mut available = parked;
        for k in 0..n.min(available.len()) {
            let i = index(
                w,
                t.id,
                r,
                available.len(),
                identity,
                "deck_parked_aircraft",
            );
            let id = available.remove(i);
            parked_aircraft_hit(
                w,
                t,
                h,
                &f,
                &id,
                r,
                &format!("{identity}:aircraft:{k}"),
                &mut BTreeSet::new(),
            );
        }
    }
}
fn parked_aircraft_hit(
    w: &mut World,
    t: &Entity,
    h: &dr::Resolved,
    f: &UnitFit,
    id: &str,
    r: &mut Rng,
    identity: &str,
    visited: &mut BTreeSet<String>,
) {
    if !visited.insert(id.into()) {
        return;
    }
    let Some(a) = f.parked.get(id) else { return };
    let destroyed = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&t.id))
        .is_some_and(|u| u.destroyed_aircraft.contains(id));
    if destroyed {
        return;
    }
    let removed = if let Some(deck) = w.entity_mut(t.id).and_then(|e| e.deck.as_mut()) {
        if a.stock == "ready" {
            if let Some((_, n)) = deck
                .ready
                .iter_mut()
                .find(|(class, n)| class == &a.class && *n > 0)
            {
                *n -= 1;
                true
            } else {
                false
            }
        } else if let Some((_, n, _)) = deck
            .cycling
            .iter_mut()
            .find(|(class, n, _)| class == &a.class && *n > 0)
        {
            *n -= 1;
            true
        } else {
            false
        }
    } else {
        false
    };
    if !removed {
        pending(
            w,
            t.id,
            identity,
            "parked_aircraft",
            "explicit parked individual has no matching current physical aircraft stock",
        );
        return;
    }
    w.rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .units
        .entry(t.id)
        .or_default()
        .destroyed_aircraft
        .insert(id.into());
    let opts = w
        .rules
        .expanded_damage
        .as_ref()
        .unwrap()
        .config
        .optional
        .clone();
    let armed = !a.armed_ordnance.is_empty();
    if opts.armed_fueled_aircraft && a.stock == "ready" && !armed && a.fueled.is_none() {
        pending(w, t.id, identity, "parked_aircraft_fire",
            "14.5 optional armed-or-fueled fire modifier needs this aircraft fuel status; no die rolled");
        return;
    }
    let high_fire = opts.armed_fueled_aircraft && a.stock == "ready" && (armed || a.fueled == Some(true));
    let raw = roll(w, t.id, r, 6, identity, "aircraft_fire");
    let severity = if high_fire {
        raw as f64 + 2.
    } else {
        raw.saturating_sub(2) as f64
    };
    let exploded = high_fire && armed && severity >= 6.;
    let damage: Option<f64> = a
        .armed_ordnance
        .iter()
        .map(|(weapon, n)| w.db.weapons.get(weapon).map(|d| d.damage * (*n as f64)))
        .sum();
    if exploded {
        if let Some(dp) = damage {
            dr::queue_armed_aircraft_explosion(
                w,
                t,
                dp * 2. / 3.,
                format!("{identity}:armed_aircraft"),
            );
        } else {
            pending(
                w,
                t.id,
                identity,
                "aircraft_ordnance",
                "parked armed aircraft weapon DP source missing",
            );
        }
    }
    if severity > 0. {
        dr::add_source_casualty(w, t.id, identity, false, severity, 3 * dr::tt());
    }
    record(
        w,
        t.id,
        identity,
        "parked_aircraft",
        Some(id.into()),
        "resolved",
    );
    if opts.closely_parked_aircraft {
        let near: Vec<_> = f
            .parked
            .iter()
            .filter(|(other, b)| {
                other.as_str() != id
                    && ((b.position_m[0] - a.position_m[0]).powi(2)
                        + (b.position_m[1] - a.position_m[1]).powi(2))
                    .sqrt()
                        <= 10.
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut near = near;
        for k in 0..4.min(near.len()) {
            let i = index(w, t.id, r, near.len(), identity, "closely_parked_selection");
            let next = near.remove(i);
            if visited.contains(&next) {
                continue;
            }
            let chance = if exploded {
                Some(50.)
            } else if severity > 0. {
                Some(25.)
            } else {
                w.rules
                    .expanded_damage
                    .as_ref()
                    .and_then(|s| s.attack_probabilities.get(&h.hit.identity))
                    .map(|p| p * 50.)
            };
            if let Some(chance) = chance {
                if roll(w, t.id, r, 100, identity, "closely_parked_hit") as f64 <= chance {
                    parked_aircraft_hit(
                        w,
                        t,
                        h,
                        f,
                        &next,
                        r,
                        &format!("{identity}:near:{k}"),
                        visited,
                    );
                }
            } else {
                pending(
                    w,
                    t.id,
                    identity,
                    "closely_parked_aircraft",
                    "original modified hit probability not recorded; no invented half probability",
                );
            }
        }
    }
}
pub fn record_attack_probability(
    w: &mut World,
    identity: &str,
    modified_probability: f64,
) -> Result<(), String> {
    if !modified_probability.is_finite() || !(0.0..=1.).contains(&modified_probability) {
        return Err("original modified hit probability invalid".into());
    }
    if let Some(s) = w.rules.expanded_damage.as_mut() {
        if s.attack_probabilities
            .get(identity)
            .is_some_and(|p| (*p - modified_probability).abs() > 1e-12)
        {
            return Err("original hit probability source conflict".into());
        }
        s.attack_probabilities
            .entry(identity.into())
            .or_insert(modified_probability);
    }
    Ok(())
}
pub fn restore_system(
    w: &mut World,
    id: EntityId,
    kind: &str,
    component: Option<&str>,
) -> Result<(), String> {
    if kind == "component" {
        return restore_component(w, id, component.ok_or("physical repair component absent")?);
    }
    let key = if kind == "flight_deck" {
        component.ok_or("flight deck repair location absent")?
    } else {
        kind
    };
    let keep = if let Some(s) = w.rules.expanded_damage.as_mut() {
        let n = s
            .units
            .entry(id)
            .or_default()
            .system_counts
            .entry(key.into())
            .or_default();
        *n = n.saturating_sub(1);
        *n > 0
    } else {
        false
    };
    if keep {
        return Ok(());
    }
    let e = w.entity_mut(id).ok_or("repair unit absent")?;
    match kind {
        "bridge" => {
            for k in [
                "damage.bridge_until",
                "damage.bridge_order_delay",
                "damage.bridge_course",
                "damage.bridge_speed",
            ] {
                e.ext.remove(k);
            }
        }
        "airplot" => {
            e.ext.remove("damage.airplot_until");
        }
        "rudder" => {
            for k in [
                "rudder_hit",
                "damage.rudder_disabled",
                "damage.rudder_jammed",
                "damage.rudder_rate",
                "damage.rudder_repair_due",
                "damage.rudder_attempts",
                "damage.depth_control_lost",
            ] {
                e.ext.remove(k);
            }
        }
        "cic" => {
            e.ext.remove("damage.cic");
        }
        "flight_deck" => {
            e.ext.remove(key);
            if [
                "damage.deck_launch",
                "damage.deck_waist",
                "damage.deck_recover",
            ]
            .iter()
            .all(|k| dr::val(e, k) == 0.)
            {
                e.ext.remove("flight_deck_hit");
                e.ext.remove("damage.deck_location_known");
            }
        }
        _ => return Err("repair kind has no source-backed restoration".into()),
    }
    Ok(())
}

pub(super) fn bridge_navigation_loss(w: &mut World, id: EntityId, identity: &str) {
    let fitted: Vec<_> = fit(w, id)
        .map(|f| {
            f.components
                .iter()
                .filter(|(_, c)| c.kind == ComponentKind::Sensor && c.navigation)
                .map(|(id, c)| (id.clone(), c.binding.clone()))
                .collect()
        })
        .unwrap_or_default();
    for (component, binding) in fitted {
        if let Some(s) = w.rules.expanded_damage.as_mut() {
            s.units
                .entry(id)
                .or_default()
                .disabled
                .insert(component.clone());
        }
        if let (Some(e), Some(sensor)) = (w.entity_mut(id), binding) {
            e.ext.insert(format!("damage.sensor.{sensor}"), 1.);
        }
        super::repairs2026::record_system(
            w,
            id,
            &format!("{identity}:navigation:{component}"),
            "component",
            Some(&component),
            1,
        );
    }
}

/// Trusted adjudication hook used by actual facilities/nuclear results. It is
/// intentionally not a player command: callers must supply an already resolved
/// source outcome. No table roll, ordinary DP or attack is fabricated here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriticalContext {
    pub source: String,
    pub dp: f64,
    pub penetrated: bool,
    pub underwater: bool,
    pub airburst: bool,
    #[serde(default)]
    pub weapon: Option<String>,
}
pub fn source_critical(
    w: &mut World,
    id: EntityId,
    identity: &str,
    kind: &str,
    context: CriticalContext,
) -> Result<(), String> {
    if !full(w) || w.rules.expanded_damage.is_none() {
        return Err("source critical requires Full expanded state".into());
    }
    if identity.is_empty() || !source(&context.source) || !context.dp.is_finite() || context.dp < 0.
    {
        return Err("source critical context invalid".into());
    }
    if !matches!(
        kind,
        "engineering"
            | "engineering_no_fire"
            | "bridge"
            | "airplot"
            | "rudder"
            | "cic"
            | "sensor"
            | "radar"
            | "weapon"
            | "cargo"
            | "flight_deck"
            | "hangar"
            | "ammo_fuel"
            | "fire"
            | "half_fire"
            | "flooding"
            | "half_flooding"
            | "hull_penetration"
            | "hull_deformation"
            | "battery"
            | "battery_engineering"
            | "control_major"
            | "control_minor"
            | "personnel"
            | "flotation"
    ) {
        return Err("unsupported source critical outcome".into());
    }
    let key = format!("{}:{identity}:{kind}", id.0);
    if w.rules
        .expanded_damage
        .as_ref()
        .unwrap()
        .source_inputs
        .get(&key)
        .is_some_and(|original| original != &context)
    {
        return Err("source critical identity reused with different source inputs".into());
    }
    if w.rules
        .expanded_damage
        .as_ref()
        .unwrap()
        .applied_sources
        .contains(&key)
    {
        return source_result(w, id, identity);
    }
    let t = w
        .entity(id)
        .filter(|e| e.alive)
        .cloned()
        .ok_or("source critical target unavailable")?;
    let def = if let Some(weapon) = &context.weapon {
        w.db.weapons
            .get(weapon)
            .cloned()
            .ok_or("source critical weapon missing")?
    } else {
        let mut d =
            w.db.weapons
                .values()
                .next()
                .cloned()
                .ok_or("source critical requires engine damage carrier")?;
        d.id = "source_adjudicated_critical".into();
        d.damage = 0.;
        d.rule_data = None;
        d
    };
    let h = dr::Resolved {
        hit: dr::Hit {
            target: id,
            weapon: def,
            underwater: context.underwater,
            bearing: None,
            shooter: None,
            identity: identity.into(),
            tick: w.tick,
            secondary: true,
            torpedo_deep: false,
            impact_area: None,
            targeted_sensor: None,
            mine: None,
            navigation: None,
            projector: None,
            asw: None,
        },
        dp: context.dp,
        penetrated: context.penetrated,
        airburst: context.airburst,
        auto_flood: false,
        flood_half: false,
    };
    w.rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .applied_sources
        .insert(key.clone());
    w.rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .source_inputs
        .insert(key, context.clone());
    dr::register_source_cause(w, id, identity);
    let mut r = w.rng.derive(Rng::hash_tag(w.tick, id.0, identity));
    private_trace(
        w,
        id,
        "14.source_critical",
        json!({"identity":identity,"kind":kind,"context":context,"ordinary_dp_added":0,"adjudicated":true}),
    );
    let previous = w
        .rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .active_source
        .replace(identity.into());
    dr::effect(w, &t, &h, kind, &mut r, identity);
    w.rules.expanded_damage.as_mut().unwrap().active_source = previous;
    source_result(w, id, identity)
}
fn source_result(w: &World, id: EntityId, identity: &str) -> Result<(), String> {
    if w.rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .is_some_and(|u| {
            u.records
                .values()
                .any(|r| r.root_identity == identity && r.status == "pending")
        })
    {
        Err("source critical committed original evidence but needs source adjudication; no reroll or automatic completion".into())
    } else {
        Ok(())
    }
}

pub fn control(w: &mut World, id: EntityId, action: &str) -> Option<Result<(), String>> {
    if !matches!(
        action,
        "pump_flooding" | "serviceability_check" | "serviceability_repair" | "fire_restart_due"
    ) && !action.starts_with("flood_hold:")
        && !action.starts_with("abandon_weapon:")
    {
        return None;
    }
    Some((|| {
        if !full(w) || w.rules.expanded_damage.is_none() {
            return Err("expanded source damage requires Full state".into());
        }
        let t = w
            .entity(id)
            .filter(|e| e.alive)
            .cloned()
            .ok_or("source damage unit unavailable")?;
        if let Some(component) = action.strip_prefix("abandon_weapon:") {
            return personnel_choice(w, id, component);
        }
        if let Some(hold) = action.strip_prefix("flood_hold:") {
            let spec = fit(w, id)
                .and_then(|f| f.holds.get(hold))
                .cloned()
                .ok_or("own cargo hold not fitted")?;
            let s = w.rules.expanded_damage.as_mut().unwrap();
            let h = s
                .units
                .entry(id)
                .or_default()
                .holds
                .entry(hold.into())
                .or_insert(HoldState {
                    amount: spec.amount,
                    fire: false,
                    flooded: false,
                    exploded: false,
                    explosion_checked: BTreeSet::new(),
                    fire_identity: None,
                    fire_identities: BTreeSet::new(),
                });
            h.amount = 0.;
            h.flooded = true;
            h.fire = false;
            let fire_identity = h.fire_identity.clone();
            let mut family = h.fire_identities.clone();
            if let Some(identity) = fire_identity {
                family.insert(identity);
            }
            let u = s.units.get_mut(&id).unwrap();
            loop {
                let previous = family.len();
                for (identity, f) in &u.fire_history {
                    if f.origin
                        .as_ref()
                        .is_some_and(|parent| family.contains(parent))
                    {
                        family.insert(identity.clone());
                    }
                }
                if previous == family.len() {
                    break;
                }
            }
            for identity in &family {
                if let Some(f) = u.fire_history.get_mut(identity) {
                    f.suppressed = true;
                }
            }
            if let Some(cs) = w.rules.casualties.get_mut(&id) {
                cs.retain(|c| !family.contains(&c.identity));
            }
            dr::sync_casualties(w, id);
            return Ok(());
        }
        match action {
            "pump_flooding" => {
                if w.rules
                    .repairs
                    .as_ref()
                    .and_then(|s| s.units.get(&id))
                    .and_then(|u| u.session.as_ref())
                    .is_none()
                {
                    return Err(
                        "explicit post-battle begin_repairs declaration required before pumping"
                            .into(),
                    );
                }
                if w.rules
                    .casualties
                    .get(&id)
                    .is_some_and(|cs| cs.iter().any(|c| c.flood && c.severity > 0.))
                {
                    return Err("control flooding before applying a post-battle patch".into());
                }
                let u = w
                    .rules
                    .expanded_damage
                    .as_mut()
                    .unwrap()
                    .units
                    .entry(id)
                    .or_default();
                if u.patch.as_ref().is_some_and(|p| !p.popped) {
                    return Ok(());
                }
                let recovered = u.flooding_dp * 0.5;
                if recovered <= 0. {
                    return Err("no recorded actual flooding DP available to pump".into());
                }
                u.patch = Some(Patch {
                    recovered_dp: recovered,
                    began_at: w.tick,
                    last_check: None,
                    day: w.tick.div_euclid(86400),
                    maximum_speed: t.speed_kts,
                    popped: false,
                });
                if let Some(e) = w.entity_mut(id) {
                    e.hp = (e.hp + recovered).min(e.hp_max);
                }
                private_trace(
                    w,
                    id,
                    "14.6.flooding_pumped",
                    json!({"recovered_dp":recovered,"source":"one half of recorded actual flooding DP"}),
                );
            }
            "serviceability_check" => serviceability_check(w, id)?,
            "serviceability_repair" => serviceability_repair(w, id)?,
            "fire_restart_due" => fire_restarts(w, id),
            _ => unreachable!(),
        }
        Ok(())
    })())
}
pub fn serviceability_check(w: &mut World, id: EntityId) -> Result<(), String> {
    let s = w
        .rules
        .expanded_damage
        .as_ref()
        .ok_or("expanded state absent")?;
    if !s.config.optional.serviceability {
        return Err("optional14.7 not selected".into());
    }
    let f = fit(w, id)
        .cloned()
        .ok_or("serviceability source fit absent")?;
    let rank = f
        .rank
        .ok_or("navy serviceability rank must be explicitly declared")?;
    let day = w.tick.div_euclid(86400);
    let has_today = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .is_some_and(|u| {
            u.service_checks
                .keys()
                .any(|key| key.starts_with(&format!("serviceability:{day}:")))
        });
    if w.tick != 0
        && !(s.config.optional.serviceability_campaign && w.tick.rem_euclid(86400) == 0)
        && !has_today
    {
        return Err(
            "serviceability first check belongs before game or an explicit campaign day boundary"
                .into(),
        );
    }
    let mut components: Vec<_> = f.components.keys().cloned().collect();
    if components.is_empty() {
        return Err(
            "serviceability requires explicit engineering/sensor/weapon system identities".into(),
        );
    }
    components.sort();
    for component in components {
        let c = &f.components[&component];
        if !matches!(
            c.kind,
            ComponentKind::Engineering | ComponentKind::Sensor | ComponentKind::WeaponMount
        ) {
            continue;
        }
        let identity = format!("serviceability:{day}:{component}");
        if w.rules
            .expanded_damage
            .as_ref()
            .and_then(|s| s.units.get(&id))
            .is_some_and(|u| u.service_checks.contains_key(&identity))
        {
            continue;
        }
        let existing = w
            .rules
            .expanded_damage
            .as_ref()
            .and_then(|s| s.units.get(&id))
            .is_some_and(|u| u.disabled.contains(&component));
        if existing {
            continue;
        }
        let mut r = w.rng.derive(Rng::hash_tag(day, id.0, &identity));
        let d = roll(w, id, &mut r, 100, &identity, "daily_serviceability");
        let chance = tables()["serviceability"]["failure_percent"][if rank == Rank::First {
            "first"
        } else {
            "second"
        }]
        .as_u64()
        .unwrap() as u8;
        let failed = d <= chance;
        let u = w
            .rules
            .expanded_damage
            .as_mut()
            .unwrap()
            .units
            .entry(id)
            .or_default();
        u.service_checks.insert(
            identity.clone(),
            ServiceCheck {
                checked_at: w.tick,
                d100: d,
                failed,
                repaired: false,
                checks: BTreeMap::new(),
                sea_unrepairable: false,
            },
        );
        if failed {
            u.disabled.insert(component.clone());
        }
        record(
            w,
            id,
            &identity,
            "serviceability",
            Some(component.clone()),
            if failed { "failed" } else { "serviceable" },
        );
        private_trace(
            w,
            id,
            "14.7.serviceability",
            json!({"identity":identity,"component":component,"rank":rank,"chance_percent":chance,"d100":d,"failed":failed}),
        );
    }
    Ok(())
}
fn serviceability_repair(w: &mut World, id: EntityId) -> Result<(), String> {
    let f = fit(w, id).cloned().ok_or("serviceability fit absent")?;
    let rank = f.rank.ok_or("serviceability rank absent")?;
    let began = super::repairs2026::optional_session(w,id)?;
    let mut state = w
        .rules
        .expanded_damage
        .take()
        .ok_or("expanded state absent")?;
    let result: Result<(), String> = (|| {
        let Some(u) = state.units.get_mut(&id) else {
            return Ok(());
        };
        for (identity, c) in &mut u.service_checks {
            if !c.failed
                || c.repaired
                || c.sea_unrepairable
                    && f.repair_base == RepairBase::AtSea
                    && !f.land_based_aircraft
            {
                continue;
            }
            let component = identity
                .splitn(3, ':')
                .nth(2)
                .ok_or("recorded service identity invalid")?
                .to_string();
            for (index, hour) in [6u32, 12, 24, 48].into_iter().enumerate() {
                let due = began + hour as i64 * 3600;
                if due > w.tick {
                    break;
                }
                if c.checks.contains_key(&hour) {
                    continue;
                }
                let base = tables()["serviceability"]["repair_percent"][if rank == Rank::First {
                    "first"
                } else {
                    "second"
                }][index]
                    .as_f64()
                    .unwrap();
                let bonus = match f.repair_base {
                    RepairBase::AtSea => 0.,
                    RepairBase::ForwardTender => 20.,
                    RepairBase::Port => 30.,
                } + if f.land_based_aircraft { 10. } else { 0. };
                let mut r = w.rng.derive(Rng::hash_tag(
                    due,
                    id.0,
                    &format!("{identity}:repair:{hour}"),
                ));
                let d = roll(w, id, &mut r, 100, identity, "breakdown_repair");
                let chance = (base + bonus).min(100.);
                let success = d as f64 <= chance;
                c.checks.insert(
                    hour,
                    ServiceRepair {
                        due_at: due,
                        chance_percent: chance,
                        d100: d,
                        success,
                    },
                );
                if success {
                    c.repaired = true;
                    u.disabled.remove(&component);
                    break;
                }
                if hour == 48 {
                    c.sea_unrepairable = true;
                }
            }
        }
        Ok(())
    })();
    w.rules.expanded_damage = Some(state);
    result?;
    aircraft_repair(w, id, &f, began)
}
fn aircraft_repair(w: &mut World, id: EntityId, f: &UnitFit, began: i64) -> Result<(), String> {
    let work_elapsed=super::ground2026::aircraft_maintenance::effective_elapsed(w,id,began)?;
    let Some(a) = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .and_then(|u| u.aircraft.clone())
    else {
        return Ok(());
    };
    if a.repaired || a.severity == AirSeverity::Destroyed {
        return Ok(());
    }
    let rank = f
        .rank
        .ok_or("source aircraft repair navy rank not declared")?;
    let mut hours = vec![6u32, 12, 24, 48];
    if f.land_based_aircraft {
        let elapsed = (work_elapsed.unwrap_or((w.tick-began).max(0))/3600) as u32;
        let mut h = 72u32;
        while h <= elapsed {
            hours.push(h);
            let Some(next) = h.checked_add(24) else { break };
            h = next;
        }
    }
    for hour in hours {
        let nominal_due = began + hour as i64 * 3600;
        if work_elapsed.unwrap_or((w.tick-began).max(0)) < hour as i64 *3600 {
            break;
        }
        let due=if work_elapsed.is_some(){w.tick}else{nominal_due};
        let u = &w.rules.expanded_damage.as_ref().unwrap().units[&id];
        if u.aircraft_repairs.contains_key(&hour) {
            continue;
        }
        if !f.land_based_aircraft && u.aircraft_repairs.get(&48).is_some_and(|r| !r.success) {
            break;
        }
        let col = match hour {
            6 => 0,
            12 => 1,
            24 => 2,
            _ => 3,
        };
        let base = tables()["serviceability"]["repair_percent"][if rank == Rank::First {
            "first"
        } else {
            "second"
        }][col]
            .as_f64()
            .unwrap();
        let bonus = match f.repair_base {
            RepairBase::AtSea => 0.,
            RepairBase::ForwardTender => 20.,
            RepairBase::Port => 30.,
        } + if f.land_based_aircraft { 10. } else { 0. };
        let chance = (base + bonus).min(100.);
        let identity = format!("aircraft_repair:{id:?}:{began}:{hour}");
        let mut r = w.rng.derive(Rng::hash_tag(due, id.0, &identity));
        let d = roll(w, id, &mut r, 100, &identity, "aircraft_breakdown_repair");
        let success = d as f64 <= chance;
        let u = w
            .rules
            .expanded_damage
            .as_mut()
            .unwrap()
            .units
            .get_mut(&id)
            .unwrap();
        u.aircraft_repairs.insert(
            hour,
            ServiceRepair {
                due_at: due,
                chance_percent: chance,
                d100: d,
                success,
            },
        );
        if success {
            let a = u.aircraft.as_mut().unwrap();
            a.repaired = true;
            a.active_hits = 0;
            if let Some(e) = w.entity_mut(id) {
                e.ext.remove("damage.aircraft_cruise_cap");
            }
            break;
        }
    }
    Ok(())
}
pub fn has_recorded_repair_work(w: &World, id: EntityId) -> bool {
    w.rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .is_some_and(|u| {
            u.flooding_dp > 0.
                || !u.fire_history.is_empty()
                || u.service_checks.values().any(|c| c.failed && !c.repaired)
                || u.aircraft
                    .as_ref()
                    .is_some_and(|a| a.severity != AirSeverity::Destroyed && !a.repaired)
        })
}
fn fire_restarts(w: &mut World, id: EntityId) {
    let now = w.tick;
    let began = w
        .rules
        .repairs
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .filter(|u| u.pending.is_none())
        .and_then(|u| u.session.as_ref())
        .map(|s| s.began_at);
    let live: BTreeSet<_> = w
        .rules
        .casualties
        .get(&id)
        .map(|v| {
            v.iter()
                .filter(|c| !c.flood && c.severity > 0.)
                .map(|c| c.identity.clone())
                .collect()
        })
        .unwrap_or_default();
    let mut state = match w.rules.expanded_damage.take() {
        Some(s) => s,
        None => return,
    };
    let mut restart = vec![];
    if let Some(u) = state.units.get_mut(&id) {
        for (identity, f) in &mut u.fire_history {
            if f.suppressed {
                continue;
            }
            if live.contains(identity) {
                continue;
            }
            let at = *f.extinguished_at.get_or_insert(now);
            let Some(at) = began.map(|b| b.max(at)) else {
                continue;
            };
            for (i, hour) in [6u8, 12, 24, 48].into_iter().enumerate() {
                if now < at + hour as i64 * 3600 {
                    break;
                }
                if !f.checked_hours.insert(hour) {
                    continue;
                }
                let mut r = w.rng.derive(Rng::hash_tag(
                    at,
                    id.0,
                    &format!("fire_restart:{identity}:{hour}"),
                ));
                let d = roll(w, id, &mut r, 100, identity, "fire_restart");
                let chance = tables()["fire_restart_percent"][i].as_u64().unwrap() as u8;
                if d <= chance {
                    let severity = roll(w, id, &mut r, 6, identity, "restarted_fire_severity")
                        .saturating_sub(2) as f64;
                    if severity > 0. {
                        f.restarted = true;
                        restart.push((
                            identity.clone(),
                            format!("{identity}:restart:{hour}"),
                            severity,
                        ));
                    }
                    break;
                }
            }
        }
    }
    w.rules.expanded_damage = Some(state);
    for (origin, identity, severity) in restart {
        dr::add_source_casualty(w, id, &identity, false, severity, 3 * dr::tt());
        if let Some(f) = w
            .rules
            .expanded_damage
            .as_mut()
            .and_then(|s| s.units.get_mut(&id))
            .and_then(|u| u.fire_history.get_mut(&identity))
        {
            f.origin = Some(origin);
        }
    }
}

pub fn step(w: &mut World) {
    if !full(w) || w.rules.expanded_damage.is_none() {
        return;
    }
    personnel_transfer::step(w);
    let ids: Vec<_> = w
        .entities
        .values()
        .filter(|e| e.alive && !e.kind.is_projectile())
        .map(|e| e.id)
        .collect();
    for id in ids {
        if w.tick > 0
            && w.tick.rem_euclid(86400) == 0
            && w.rules.expanded_damage.as_ref().is_some_and(|s| {
                s.config.optional.serviceability && s.config.optional.serviceability_campaign
            })
            && fit(w, id).is_some()
        {
            if let Err(reason) = serviceability_check(w, id) {
                pending(w, id, "serviceability", "serviceability", &reason);
            }
        }
        fire_restarts(w, id);
        let automatic = w
            .rules
            .repairs
            .as_ref()
            .and_then(|s| s.units.get(&id))
            .filter(|u| u.pending.is_none())
            .and_then(|u| u.session.as_ref())
            .is_some_and(|s| s.automatic);
        let optional_work = w
            .rules
            .expanded_damage
            .as_ref()
            .and_then(|s| s.units.get(&id))
            .is_some_and(|u| {
                u.service_checks.values().any(|c| c.failed && !c.repaired)
                    || u.aircraft
                        .as_ref()
                        .is_some_and(|a| a.severity != AirSeverity::Destroyed && !a.repaired)
            });
        if automatic && optional_work {
            if let Err(reason) = serviceability_repair(w, id) {
                pending(w, id, "optional_repair", "optional_repair", &reason);
            }
        }
        patch_step(w, id);
        cargo_step(w, id);
        if w.tick.rem_euclid(180) == 0 {
            depth_step(w, id);
        }
    }
    rescue_step(w);
}
fn patch_step(w: &mut World, id: EntityId) {
    let Some(t) = w.entity(id).cloned() else {
        return;
    };
    let sea = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| s.config.environment.as_ref())
        .map(|e| e.sea_state);
    let mut popped = false;
    let mut recovered = 0.;
    let mut trace = None;
    if let Some(p) = w
        .rules
        .expanded_damage
        .as_mut()
        .and_then(|s| s.units.get_mut(&id))
        .and_then(|u| u.patch.as_mut())
    {
        if p.popped {
            return;
        }
        let day = w.tick.div_euclid(86400);
        if p.day != day {
            p.day = day;
            p.maximum_speed = t.speed_kts;
        }
        p.maximum_speed = p.maximum_speed.max(t.speed_kts);
        if t.kind == PlatformKind::Sub && t.depth_m > 25. {
            p.popped = true;
            popped = true;
            recovered = p.recovered_dp;
        } else if t.kind != PlatformKind::Sub
            && w.tick.rem_euclid(1800) == 0
            && p.last_check != Some(w.tick)
        {
            p.last_check = Some(w.tick);
            if let Some(sea) = sea {
                let chance = (sea as f64 * 5. - (30. - p.maximum_speed)).clamp(0., 100.);
                trace = Some((chance, p.recovered_dp));
            }
        }
    }
    if let Some((chance, amount)) = trace {
        let identity = format!("flood_patch:{id:?}:{}", w.tick);
        let mut r = w.rng.derive(Rng::hash_tag(w.tick, id.0, &identity));
        let d = roll(w, id, &mut r, 100, &identity, "flood_patch_pop");
        if d as f64 <= chance {
            popped = true;
            recovered = amount;
            if let Some(p) = w
                .rules
                .expanded_damage
                .as_mut()
                .and_then(|s| s.units.get_mut(&id))
                .and_then(|u| u.patch.as_mut())
            {
                p.popped = true;
            }
        }
    }
    if popped {
        if let Some(e) = w.entity_mut(id) {
            e.hp = (e.hp - recovered).max(0.);
        }
        private_trace(
            w,
            id,
            "14.6.flood_patch_popped",
            json!({"restored_flooding_dp":recovered,"depth_m":t.depth_m}),
        );
        let severity = w
            .rules
            .expanded_damage
            .as_ref()
            .and_then(|s| s.units.get(&id))
            .map_or(0., |u| u.source_flood_severity);
        if severity > 0. {
            let identity = format!("popped_patch:{}:{}", id.0, w.tick);
            dr::add_source_casualty(w, id, &identity, true, severity, dr::it());
        }
    }
}
fn cargo_step(w: &mut World, id: EntityId) {
    if w.tick.rem_euclid(1800) != 0 {
        return;
    }
    let Some(t) = w.entity(id).cloned() else {
        return;
    };
    let Some(f) = fit(w, id).cloned() else { return };
    let holds: Vec<_> = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .map(|u| {
            u.holds
                .iter()
                .filter(|(_, h)| h.fire && !h.flooded && !h.exploded)
                .map(|(id, h)| (id.clone(), h.amount))
                .collect()
        })
        .unwrap_or_default();
    for (hold, amount) in holds {
        if f.holds
            .get(&hold)
            .is_none_or(|h| h.kind != CargoKind::Ammunition)
        {
            continue;
        }
        let identity = format!("cargo_check:{}:{hold}", w.tick);
        let already = w
            .rules
            .expanded_damage
            .as_ref()
            .and_then(|s| s.units.get(&id))
            .and_then(|u| u.holds.get(&hold))
            .is_some_and(|h| h.explosion_checked.contains(&identity));
        if already {
            continue;
        }
        if let Some(h) = w
            .rules
            .expanded_damage
            .as_mut()
            .and_then(|s| s.units.get_mut(&id))
            .and_then(|u| u.holds.get_mut(&hold))
        {
            h.explosion_checked.insert(identity.clone());
        }
        let mut r = w.rng.derive(Rng::hash_tag(w.tick, id.0, &identity));
        if roll(
            w,
            id,
            &mut r,
            100,
            &identity,
            "ammunition_cargo_later_explosion",
        ) <= 25
        {
            if let Some(h) = w
                .rules
                .expanded_damage
                .as_mut()
                .and_then(|s| s.units.get_mut(&id))
                .and_then(|u| u.holds.get_mut(&hold))
            {
                h.exploded = true;
                h.amount = 0.;
                h.fire = false;
            }
            cargo_explosion(
                w,
                &t,
                &f,
                &hold,
                amount,
                &mut r,
                &identity,
                &mut BTreeSet::new(),
            );
        }
    }
}
fn depth_step(w: &mut World, id: EntityId) {
    let Some(t) = w
        .entity(id)
        .filter(|e| e.kind == PlatformKind::Sub)
        .cloned()
    else {
        return;
    };
    let deformation = dr::val(&t, "damage.hull_deformation") as u8;
    let lost = dr::val(&t, "damage.depth_control_lost") > 0.;
    let turn = w.tick.div_euclid(180);
    let now = w.tick;
    let mut u = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .cloned()
        .unwrap_or_default();
    if lost && u.last_depth_check != Some(turn) {
        u.last_depth_check = Some(turn);
        let identity = format!("rudder_depth:{turn}");
        let mut r = w.rng.derive(Rng::hash_tag(turn, id.0, &identity));
        let chance = (5. * t.speed_kts).clamp(0., 100.);
        if roll(w, id, &mut r, 100, &identity, "rudder_involuntary_depth") as f64 <= chance {
            let up = roll(w, id, &mut r, 10, &identity, "rudder_depth_direction") <= 5;
            let bands = super::navigation2026::tables()["depth_bands_max_m"]
                .as_array()
                .unwrap();
            let band = bands
                .iter()
                .position(|b| t.depth_m <= b.as_f64().unwrap() + 1e-9)
                .unwrap_or(bands.len() - 1);
            let next = if up {
                band.saturating_sub(1)
            } else {
                (band + 1).min(bands.len() - 1)
            };
            let depth = if band == 0 && up {
                0.
            } else {
                bands[next].as_f64().unwrap()
            };
            let crush = fit(w, id).and_then(|f| f.crush_depth_m);
            if !up && crush.is_none() {
                pending(
                    w,
                    id,
                    &identity,
                    "rudder_depth",
                    "explicit crush depth needed to cap involuntary downward motion",
                );
            } else if let Some(e) = w.entity_mut(id) {
                let depth = crush.map_or(depth, |c| depth.min(c));
                e.depth_m = depth;
                e.ordered_depth_m = if depth == 0. { 25. } else { depth };
                if depth == 0. {
                    e.ext
                        .insert("damage.broach_until".into(), (now + 180) as f64);
                }
            }
        }
    }
    if deformation > 0 && deformation < 3 && u.last_hull_check != Some(turn) {
        u.last_hull_check = Some(turn);
        let bands = super::navigation2026::tables()["depth_bands_max_m"]
            .as_array()
            .unwrap();
        let current = bands
            .iter()
            .position(|b| t.depth_m <= b.as_f64().unwrap() + 1e-9)
            .unwrap_or(bands.len() - 1);
        let safe = if deformation == 1 { 100. } else { 50. };
        let safe_band = bands
            .iter()
            .position(|b| safe <= b.as_f64().unwrap())
            .unwrap_or(0);
        if current > safe_band && t.ordered_depth_m > safe {
            let identity = format!("hull_deformation:{turn}");
            let mut r = w.rng.derive(Rng::hash_tag(turn, id.0, &identity));
            let chance =
                ((current - safe_band) as f64 * if deformation == 1 { 25. } else { 30. }).min(100.);
            if roll(w, id, &mut r, 100, &identity, "deformed_hull_failure") as f64 <= chance {
                let _ = source_critical(
                    w,
                    id,
                    &identity,
                    "hull_penetration",
                    CriticalContext {
                        source: "English2026 14.5 plus per-Tactical-Turn RTS risk epoch".into(),
                        dp: 0.,
                        penetrated: true,
                        underwater: true,
                        airburst: false,
                        weapon: None,
                    },
                );
            }
        }
    }
    // Merge concurrent hook writes (for example a pending depth source or the
    // hull-failure critical) instead of replacing their newly recorded state.
    let current = w
        .rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .units
        .entry(id)
        .or_default();
    current.last_depth_check = u.last_depth_check;
    current.last_hull_check = u.last_hull_check;
}

/// Loss must be called by the native sink path before entity cleanup. A
/// survivor's physical position stays Referee-private until an actual sighting.
pub fn record_catastrophic_loss(w:&mut World,id:EntityId,identity:&str) {
    if !full(w)||identity.is_empty() {return;}
    if let Some(s)=w.rules.expanded_damage.as_mut() {
        s.units.entry(id).or_default().catastrophic_loss.get_or_insert_with(||identity.into());
    }
}
pub fn record_loss(w: &mut World, id: EntityId, catastrophic: bool) {
    let enabled = w
        .rules
        .expanded_damage
        .as_ref()
        .is_some_and(|s| s.config.optional.search_and_rescue);
    if !full(w) || !enabled {
        return;
    }
    let Some(t) = w.entity(id).cloned() else {
        return;
    };
    if !matches!(t.kind,PlatformKind::Ship|PlatformKind::Sub|PlatformKind::Aircraft) {return;}
    let catastrophic=catastrophic||w.rules.expanded_damage.as_ref()
        .and_then(|s|s.units.get(&id)).is_some_and(|u|u.catastrophic_loss.is_some());
    if w.rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .is_some_and(|u| u.survivor_created)
    {
        return;
    }
    let Some(f) = fit(w, id).cloned() else {
        pending(
            w,
            id,
            "loss",
            "survivors",
            "original crew/passenger complement source missing",
        );
        return;
    };
    let Some(crew) = f
        .crew
        .and_then(|n| n.checked_add(f.passengers.unwrap_or(0)))
    else {
        pending(
            w,
            id,
            "loss",
            "survivors",
            "original crew/passenger complement source missing",
        );
        return;
    };
    let identity = format!("survivors:{}:{}", id.0, w.tick);
    let mut r = w.rng.derive(Rng::hash_tag(w.tick, id.0, &identity));
    let d = if t.kind == PlatformKind::Aircraft {
        None
    } else {
        Some(roll(w, id, &mut r, 6, &identity, "ship_survivors"))
    };
    let alongside = w.entities.values().any(|e| {
        e.id != id
            && e.alive
            && e.kind == PlatformKind::Ship
            && e.speed_kts.abs() < 1e-9
            && e.pos.range_to(t.pos) * 2025.371828521435 <= 250.
    });
    let mut count = if let Some(d) = d {
        crew as f64 * d as f64 * if catastrophic { 0.01 } else { 0.1 }
    } else {
        crew as f64
    };
    if alongside && d.is_some() {
        count *= 1.2
    }
    let count = count.floor().min(crew as f64) as u32;
    let s = w.rules.expanded_damage.as_mut().unwrap();
    s.units.entry(id).or_default().survivor_created = true;
    s.survivors.insert(
        identity.clone(),
        Survivors {
            own_side: t.side,
            lost_unit: id,
            position: t.pos,
            created_at: w.tick,
            count,
            rescued: 0,
            observed_by: BTreeSet::new(),
            drift: None,
            source_crew: crew,
            casualty_d6: d,
            sightings: BTreeMap::new(),
            last_drift_tick: None,
        },
    );
    sar_native::witness_loss(w, id, &identity);
    private_trace(
        w,
        id,
        "14.8.survivors_created",
        json!({"identity":identity,"crew":crew,"d6":d,"catastrophic":catastrophic,"alongside":alongside,"count":count,"position":t.pos}),
    );
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SurvivorSightingProfile {
    pub physical_size: usize,
    pub signature: usize,
    pub no_wake_multiplier: f64,
    pub category: String,
}
pub fn survivor_sighting_profile(
    w: &World,
    group: &str,
    flir: bool,
) -> Result<SurvivorSightingProfile, String> {
    let g = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.survivors.get(group))
        .ok_or("survivor physical group absent")?;
    let n = g.count.saturating_sub(g.rescued);
    let row = survivor_sighting_row(n)?;
    let size = row["physical_size"].as_u64().ok_or("14.8 source physical size missing")? as usize;
    Ok(SurvivorSightingProfile {
        physical_size: if flir { size.saturating_sub(1) } else { size },
        signature: row["signature"].as_u64().ok_or("14.8 source signature missing")? as usize,
        no_wake_multiplier: tables()["sar"]["no_wake_range_multiplier"].as_f64().ok_or("14.8 source no-wake multiplier missing")?,
        category: row["category"].as_str().ok_or("14.8 source group category missing")?.into(),
    })
}
fn survivor_sighting_row(n: u32) -> Result<&'static Value, String> {
    tables()["sar"]["sighting_profiles"]
        .as_array()
        .ok_or("14.8 source sighting profiles missing")?
        .iter()
        .find(|row| row["min"].as_u64().is_some_and(|min|u64::from(n)>=min)
            && (row["max"].is_null() || row["max"].as_u64().is_some_and(|max|u64::from(n)<=max)))
        .ok_or_else(||"14.8 empty group or overlapping30/100 people endpoint remains source-pending".into())
}
fn sar_tactical_seconds() -> i64 {
    tables()["sar"]["tactical_turn_seconds"].as_i64().filter(|v|*v>0)
        .expect("verified14.8 Tactical Turn duration")
}
fn sar_drift_after_seconds() -> i64 {
    tables()["sar"]["drift_after_hours"].as_i64().filter(|v|*v>=0)
        .expect("verified14.8 drift threshold") * 3600
}
fn survivor_position_source(w: &World, group: &Survivors) -> Result<(), String> {
    if w.tick < group.created_at {
        return Err("14.8 survivor creation lies in the future".into());
    }
    if w.tick-group.created_at > sar_drift_after_seconds() && group.drift.is_none() {
        return Err("14.8 survivor current consistent with local geography is unknown; position pending".into());
    }
    Ok(())
}
/// Trusted sensing hook: only call after successful native Visual/FLIR scan.
/// A cached loss report or sender warning is not a successful survivor scan.
pub fn record_survivor_sighting(
    w: &mut World,
    observer: EntityId,
    group: &str,
) -> Result<(), String> {
    if !w.entity(observer).is_some_and(|e| e.alive) {
        return Err("survivor observer unavailable".into());
    }
    let profile = survivor_sighting_profile(w, group, false)?;
    let report_side = w.entity(observer).unwrap().side;
    let side = report_side.name().to_string();
    let now = w.tick;
    let s = w
        .rules
        .expanded_damage
        .as_mut()
        .ok_or("survivor source state unavailable")?;
    let g = s
        .survivors
        .get_mut(group)
        .ok_or("survivor physical group unavailable")?;
    g.observed_by.insert(observer);
    g.sightings.insert(
        observer,
        SurvivorReport {
            observed_at: now,
            position: g.position,
            category: profile.category,
            side: Some(report_side),
        },
    );
    let labels = s.survivor_labels.entry(side).or_default();
    if !labels.contains_key(group) {
        let label = format!("survivors-{}", labels.len() + 1);
        labels.insert(group.into(), label);
    }
    Ok(())
}
pub fn survivor_view(w: &World, observer: EntityId) -> Vec<Value> {
    let Some(own) = w.entity(observer) else {
        return vec![];
    };
    let Some(s) = w.rules.expanded_damage.as_ref() else {
        return vec![];
    };
    let Some(labels) = s.survivor_labels.get(own.side.name()) else {
        return vec![];
    };
    labels.iter().filter_map(|(key,label)|{let g=s.survivors.get(key)?;let report=g.sightings.iter().filter(|(id,r)|r.side.map_or_else(||w.entity(**id).is_some_and(|e|e.side==own.side),|side|side==own.side)).map(|(_,r)|r).max_by_key(|r|r.observed_at)?;Some(json!({"label":label,"position":report.position,"observed_at":report.observed_at,"estimated_group_size":report.category,"report_is_live_track":false}))}).collect()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RescueAction {
    Begin { group: String, automatic: bool },
    TransferCrew { receiver: EntityId, healthy: u32, injured: u32 },
    Manual {},
    Automatic {},
    RecoverDue {},
    Cancel {},
}
fn own_survivor_group(w: &World, id: EntityId, label: &str) -> Option<String> {
    let side = w.entity(id)?.side.name();
    w.rules
        .expanded_damage
        .as_ref()?
        .survivor_labels
        .get(side)?
        .iter()
        .find_map(|(key, l)| (l == label).then(|| key.clone()))
}
fn rescue_platform(w: &World, id: EntityId, f: &RescueFit) -> Result<(), String> {
    let t = w.entity(id).filter(|e|e.alive).ok_or("14.8 rescue platform unavailable or lost")?;
    if (f.kind == RescueKind::Ship && t.kind != PlatformKind::Ship)
        || (matches!(f.kind,RescueKind::Helicopter|RescueKind::FlyingBoat) && t.kind != PlatformKind::Aircraft)
    {
        return Err("14.8 rescue fit does not match the actual physical platform kind".into());
    }
    Ok(())
}
fn rescue_station(w: &World, id: EntityId, f: &RescueFit, g: &Survivors) -> Result<(), String> {
    rescue_platform(w,id,f)?;
    survivor_position_source(w,g)?;
    let t = w.entity(id).unwrap();
    let radius=f.station_radius_m.filter(|v|v.is_finite()&&*v>=0.)
        .ok_or("14.8 source rescue station radius missing")?/1852.;
    if t.pos.range_to(g.position)>radius || t.speed_kts.abs()>1e-9 {
        return Err("14.8 rescue requires a stationary platform at the actual survivor station".into());
    }
    if f.kind == RescueKind::FlyingBoat && t.altitude_m>0. {
        return Err("14.8 flying boat must actually be on the water".into());
    }
    if f.kind == RescueKind::Helicopter {
        let flight=w.rules.aviation.as_ref().and_then(|s|s.flights.get(&id))
            .ok_or("14.8 helicopter hoist requires actual fitted source flight state")?;
        if flight.spec.engine!=super::aviation2026::Engine::TS
            || flight.throttle!=super::aviation2026::Throttle::Hover
            || t.altitude_m<=0. || t.altitude_m>2000. || flight.fuel_nm<=0. {
            return Err("14.8 hoist requires an actual fueled helicopter hovering at Low or below".into());
        }
    }
    if f.kind == RescueKind::Ship {
        let wind=w.context.flight_environment.as_ref()
            .ok_or("14.8 source wind needed for upwind recovery")?.wind_from_deg;
        let offset=t.pos.sub(g.position);
        let up=crate::geom::Vec2::from_bearing(wind);
        if offset.x*up.x+offset.y*up.y<0. {
            return Err("14.8 ship is not upwind of survivors".into());
        }
    }
    Ok(())
}
fn settle_rescue_work(w: &mut World, id: EntityId) {
    let Some(task)=w.rules.expanded_damage.as_ref().and_then(|s|s.rescues.get(&id)).cloned() else {return;};
    let Some(f)=fit(w,id).and_then(|f|f.rescue.as_ref()).filter(|f|matches!(f.kind,RescueKind::Ship|RescueKind::Helicopter)).cloned() else {return;};
    let group=w.rules.expanded_damage.as_ref().and_then(|s|s.survivors.get(&task.group));
    let station=group.ok_or_else(||"14.8 survivor group absent".to_string()).and_then(|g|rescue_station(w,id,&f,g));
    let remaining=group.map(|g|g.count.saturating_sub(g.rescued));
    let now=w.tick;
    let previous_eligible=task.work.as_ref().map(|work|work.eligible);
    let mut work=task.work.unwrap_or(RescueWork {last_checked_at:now,continuous_seconds:0,eligible:false});
    if station.is_ok() && work.eligible && work.last_checked_at<=now {
        work.continuous_seconds=work.continuous_seconds.max(0)
            .saturating_add(now-work.last_checked_at).min(sar_tactical_seconds());
    } else {
        // A newly reached station starts now. No interval spent moving, lost,
        // downwind or at an unknown point can be banked as rescue work.
        work.continuous_seconds=0;
    }
    work.last_checked_at=now;
    work.eligible=station.is_ok();
    let eligibility_changed=previous_eligible!=Some(work.eligible);
    let detail=json!({"eligible":work.eligible,"continuous_seconds":work.continuous_seconds,
        "required_seconds":sar_tactical_seconds(),"pending":station.as_ref().err(),
        "source":"explicit RTS continuous eligible ship or helicopter work; time away or not hovering never banks"});
    let task=w.rules.expanded_damage.as_mut().unwrap().rescues.get_mut(&id).unwrap();
    task.work=Some(work);
    task.pending=station.err();
    if let Some(remaining)=remaining {task.remaining=remaining;}
    if eligibility_changed {private_trace(w,id,"14.8.rescue_station_work",detail);}
}
pub fn validate_rescue(w: &World, id: EntityId, action: &RescueAction) -> Result<(), String> {
    if !full(w) || w.over
        || w.entity(id)
            .is_some_and(|e| crate::exercise::locked(w, e.side))
    {
        return Err("rescue command unavailable in current exercise state".into());
    }
    if !w.entity(id).is_some_and(|e| e.alive) {
        return Err("rescue unit unavailable".into());
    }
    if let RescueAction::TransferCrew { receiver, healthy, injured } = action {
        return personnel_transfer::validate(w,id,*receiver,*healthy,*injured);
    }
    let s = w
        .rules
        .expanded_damage
        .as_ref()
        .ok_or("expanded source state absent")?;
    if !s.config.optional.search_and_rescue {
        return Err("optional14.8 not selected".into());
    }
    let f = fit(w, id)
        .and_then(|f| f.rescue.as_ref())
        .ok_or("rescue platform source fit absent")?;
    rescue_platform(w,id,f)?;
    if f.station_radius_m.is_none_or(|v| !v.is_finite() || v < 0.) {
        return Err("explicit scenario rescue station geometry missing".into());
    }
    if let RescueAction::Begin { group, .. } = action {
        own_survivor_group(w, id, group).ok_or("own observed survivor group unavailable")?;
    }
    if matches!(
        action,
        RescueAction::Manual {} | RescueAction::Automatic {} | RescueAction::RecoverDue {}
    ) && !s.rescues.contains_key(&id)
    {
        return Err("rescue task not begun".into());
    }
    Ok(())
}
pub fn rescue_control(w: &mut World, id: EntityId, action: RescueAction) -> Result<(), String> {
    validate_rescue(w, id, &action)?;
    match action {
        RescueAction::TransferCrew { receiver, healthy, injured } => personnel_transfer::begin(w,id,receiver,healthy,injured)?,
        RescueAction::Begin { group, automatic } => {
            let group = own_survivor_group(w, id, &group)
                .ok_or("own observed survivor group unavailable")?;
            let g=&w.rules.expanded_damage.as_ref().unwrap().survivors[&group];
            let count = g.count.saturating_sub(g.rescued);
            let f=fit(w,id).and_then(|f|f.rescue.as_ref()).unwrap();
            let station=rescue_station(w,id,f,g);
            let work=matches!(f.kind,RescueKind::Ship|RescueKind::Helicopter).then(||RescueWork {
                last_checked_at:w.tick,continuous_seconds:0,eligible:station.is_ok()
            });
            w.rules.expanded_damage.as_mut().unwrap().rescues.insert(
                id,
                RescueTask {
                    group,
                    ordered_at: w.tick,
                    last_turn: None,
                    remaining: count,
                    ready_at: None,
                    automatic,
                    picked_up: 0,
                    pending: station.err(),
                    work,
                },
            );
        }
        RescueAction::Cancel {} => {
            w.rules
                .expanded_damage
                .as_mut()
                .unwrap()
                .rescues
                .remove(&id);
        }
        RescueAction::Manual {} | RescueAction::Automatic {} => {
            let t = w
                .rules
                .expanded_damage
                .as_mut()
                .unwrap()
                .rescues
                .get_mut(&id)
                .ok_or("rescue task not begun")?;
            t.automatic = matches!(action, RescueAction::Automatic {});
        }
        RescueAction::RecoverDue {} => recover(w, id)?,
    }
    Ok(())
}
fn recover(w: &mut World, id: EntityId) -> Result<(), String> {
    // Automatic callers use the same life/kind/source gates as public orders.
    validate_rescue(w,id,&RescueAction::RecoverDue {})?;
    settle_rescue_work(w,id);
    let f = fit(w, id)
        .and_then(|f| f.rescue.clone())
        .ok_or("rescue fit absent")?;
    let task = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.rescues.get(&id))
        .cloned()
        .ok_or("rescue task absent")?;
    if task.remaining == 0 {
        return Ok(());
    }
    let g = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.survivors.get(&task.group))
        .cloned()
        .ok_or("survivor group absent")?;
    rescue_station(w,id,&f,&g)?;
    w.rules.expanded_damage.as_mut().unwrap().rescues.get_mut(&id).unwrap().pending=None;
    let turn = w.tick.div_euclid(sar_tactical_seconds());
    if task.last_turn == Some(turn) {
        return Ok(());
    }
    if matches!(f.kind,RescueKind::Ship|RescueKind::Helicopter) {
        if task.work.as_ref().is_none_or(|work|!work.eligible || work.continuous_seconds<sar_tactical_seconds()) {
            return Ok(());
        }
    } else if w.tick < task.ordered_at+sar_tactical_seconds() {
        return Ok(());
    }
    let aboard = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .map_or(0, |u| u.survivors_aboard);
    let available = g
        .count
        .saturating_sub(g.rescued)
        .min(f.capacity.saturating_sub(aboard));
    if available==0 {return Ok(());}
    let identity = format!("rescue:{}:{turn}", task.group);
    let mut r = w.rng.derive(Rng::hash_tag(turn, id.0, &identity));
    let n = match f.kind {
        RescueKind::Helicopter => u32::try_from(tables()["sar"]["helicopter_per_tactical_turn"].as_u64()
            .expect("verified14.8 helicopter hoist capacity")).expect("source hoist count fits u32"),
        RescueKind::FlyingBoat => {
            let due = if let Some(due) = task.ready_at {
                due
            } else {
                let d = roll(w, id, &mut r, 6, &identity, "flying_boat_recovery_turns") as i64;
                let due = w.tick + d * sar_tactical_seconds();
                w.rules
                    .expanded_damage
                    .as_mut()
                    .unwrap()
                    .rescues
                    .get_mut(&id)
                    .unwrap()
                    .ready_at = Some(due);
                return Ok(());
            };
            if w.tick < due {
                return Ok(());
            }
            10
        }
        RescueKind::Ship => {
            let sea = w
                .rules
                .detection
                .as_ref()
                .and_then(|s| s.config.environment.as_ref())
                .ok_or("source sea state needed for rescue")?
                .sea_state;
            let d = roll(w, id, &mut r, 6, &identity, "ship_recovery_groups") as usize;
            ((d.saturating_sub(sea.saturating_sub(2))) as f64 / 2. * 10.).floor() as u32
        }
    }
    .min(available);
    let s = w.rules.expanded_damage.as_mut().unwrap();
    s.survivors.get_mut(&task.group).unwrap().rescued += n;
    let task = s.rescues.get_mut(&id).unwrap();
    task.picked_up += n;
    task.remaining = g.count - g.rescued - n;
    task.last_turn = Some(turn);
    task.ready_at = None;
    task.pending = None;
    if let Some(work)=task.work.as_mut() {
        work.last_checked_at=w.tick;
        work.continuous_seconds=0;
    }
    let group_label = task.group.clone();
    s.units.entry(id).or_default().survivors_aboard += n;
    let side = w.entities[&id].side.name().to_string();
    let total = s.rescued_by_side.entry(side).or_default();
    *total = total.saturating_add(u64::from(n));
    private_trace(
        w,
        id,
        "14.8.survivors_recovered",
        json!({"group":group_label,"turn":turn,"recovered":n,
            "ship_work_seconds_consumed":(f.kind==RescueKind::Ship).then(sar_tactical_seconds),
            "helicopter_work_seconds_consumed":(f.kind==RescueKind::Helicopter).then(sar_tactical_seconds)}),
    );
    Ok(())
}

/// Actual native recovery is the sole producer. A search, order, repeat call,
/// restored game or referee survivor count cannot award rescue victory points.
pub fn rescue_victory_points(w: &World) -> [f64; 2] {
    let Some(s) = w.rules.expanded_damage.as_ref().filter(|s|
        full(w) && s.config.optional.search_and_rescue) else { return [0.; 2]; };
    let Some(v) = s.config.rescue_victory.as_ref().filter(|v|
        source(&v.source) && v.points_per_survivor.is_finite() && v.points_per_survivor >= 0.)
        else { return [0.; 2]; };
    [crate::Side::Blue, crate::Side::Red].map(|side|
        s.rescued_by_side.get(side.name()).copied().unwrap_or(0) as f64 * v.points_per_survivor)
}
fn rescue_step(w: &mut World) {
    let heading = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.config.survivor_current_heading_deg);
    let headings = w
        .rules
        .expanded_damage
        .as_ref()
        .unwrap()
        .config
        .survivor_current_headings_deg
        .clone();
    let now = w.tick;
    let drift_after = sar_drift_after_seconds();
    let mut missing = vec![];
    let mut s = w.rules.expanded_damage.take().unwrap();
    for (key, g) in &mut s.survivors {
        if g.count <= g.rescued || now - g.created_at <= drift_after {
            continue;
        }
        if g.drift.is_none() {
            if !headings.is_empty()
                || heading.is_some_and(|h| h.is_finite() && (0.0..360.0).contains(&h))
            {
                let mut r = w
                    .rng
                    .derive(Rng::hash_tag(g.created_at, g.lost_unit.0, key));
                let speed = roll(w, g.lost_unit, &mut r, 6, key, "survivor_current_speed") as f64;
                let heading = if headings.is_empty() {
                    heading.unwrap()
                } else {
                    headings[index(
                        w,
                        g.lost_unit,
                        &mut r,
                        headings.len(),
                        key,
                        "survivor_current_direction",
                    )]
                };
                g.drift = Some((speed, heading));
            } else {
                missing.push((g.lost_unit, key.clone()));
            }
        }
        if let Some((speed, heading)) = g.drift {
            let last = g.last_drift_tick.unwrap_or(g.created_at + drift_after);
            let elapsed = (now - last).max(0);
            g.position = g.position.advanced(heading, speed * elapsed as f64 / 3600.);
            g.last_drift_tick = Some(now);
        }
    }
    w.rules.expanded_damage = Some(s);
    for (id, key) in missing {
        pending(
            w,
            id,
            &key,
            "survivor_current",
            "source current direction consistent with local geography must be declared",
        );
    }
    let tasks: Vec<_> = w.rules.expanded_damage.as_ref().unwrap().rescues.keys().copied().collect();
    for id in tasks {
        settle_rescue_work(w,id);
    }
    if now.rem_euclid(sar_tactical_seconds()) == 0 {
        let ids: Vec<_> = w
            .rules
            .expanded_damage
            .as_ref()
            .unwrap()
            .rescues
            .iter()
            .filter(|(_, r)| r.automatic && r.remaining > 0)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if let Err(e) = recover(w, id) {
                if let Some(t) = w
                    .rules
                    .expanded_damage
                    .as_mut()
                    .unwrap()
                    .rescues
                    .get_mut(&id)
                {
                    t.pending = Some(e);
                }
            }
        }
    }
}

pub fn own_view(w: &World, id: EntityId) -> Option<Value> {
    let mut view=base_own_view(w,id)?;
    if let Some(maintenance)=super::ground2026::aircraft_maintenance::own_view(w,id) {
        view["aircraft_maintenance"]=maintenance;
    }
    Some(view)
}
fn base_own_view(w: &World, id: EntityId) -> Option<Value> {
    if !full(w) {
        return None;
    }
    let state = w.rules.expanded_damage.as_ref()?;
    let u = state.units.get(&id);
    let f = fit(w, id);
    let rescue=state.rescues.get(&id).map(|r|{let label=w.entity(id).and_then(|e|state.survivor_labels.get(e.side.name())).and_then(|labels|labels.get(&r.group));json!({"group":label,"ordered_at":r.ordered_at,"automatic":r.automatic,"picked_up":r.picked_up,"ready_at":r.ready_at,"pending":r.pending,"work":r.work.as_ref().map(|work|json!({"continuous_seconds":work.continuous_seconds,"required_seconds":sar_tactical_seconds(),"eligible":work.eligible}))})});
    let rescue_victory=w.entity(id).map(|e|json!({
        "rescued":state.rescued_by_side.get(e.side.name()).copied().unwrap_or(0),
        "points":rescue_victory_points(w)[e.side.index()],
        "declared_reward":state.config.rescue_victory.as_ref()}));
    Some(
        json!({"criticals":u.map(|u|u.records.values().map(|r|json!({"id":r.own_id,"occurred_at":r.occurred_at,"effect":r.effect,"component":r.component,"status":r.status})).collect::<Vec<_>>()),"disabled_components":u.map(|u|&u.disabled),"pending":u.map(|u|&u.pending),"holds":u.map(|u|u.holds.iter().map(|(id,h)|(id,json!({"amount":h.amount,"fire":h.fire,"flooded":h.flooded,"exploded":h.exploded}))).collect::<BTreeMap<_,_>>()),"aircraft_damage":u.and_then(|u|u.aircraft.as_ref()).map(|a|json!({"severity":a.severity,"repaired":a.repaired,"light":a.light,"jettison_failed":a.jettison_failed})),"optional":state.config.optional,"fittings":f.map(|f|&f.components),"serviceability":u.map(|u|&u.service_checks),"aircraft_repairs":u.map(|u|&u.aircraft_repairs),"survivors_aboard":u.map(|u|u.survivors_aboard),"rescue_victory":rescue_victory,"rescue_fit":f.and_then(|f|f.rescue.as_ref()),"rescue":rescue}),
    )
}

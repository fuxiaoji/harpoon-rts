//! Source §7.5 and §10.2, opt-in v4. Missing fits/FCS never use legacy Pk.
use super::firecontrol2026::{Placement, Quality};
use crate::{
    db::{WeaponDef, WeaponKind},
    geom::angle_diff,
    types::PlatformKind,
    EntityId, Vec2, World,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
mod helical;
mod initial_acquisition;
mod interference;
mod opportunity;
mod own_status;
mod straight;
mod wake;
pub use helical::HelicalState;
pub use initial_acquisition::{AcquisitionRecord, DepthMotion};
pub use opportunity::{Opportunity, OpportunityCapture, OpportunityKind};
pub use own_status::{
    mine_inventory, occupy_mine_tubes, own_view, register_release, tube_ready, unreported_releases,
};
pub use straight::{Motion, Salvo, SalvoAttack};
pub use wake::{
    Configuration as WakeConfiguration, LaunchOffset as WakeLaunchOffset, State as WakeState,
};
pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/torpedo-2026.json")).unwrap()
    })
}
pub fn active(w: &World) -> bool {
    super::aaw2026::active(w)
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Configuration {
    pub source: String,
    pub weapons: BTreeMap<String, Weapon>,
    pub targets: BTreeMap<String, Target>,
    pub platforms: BTreeMap<String, Platform>,
    pub orders: BTreeMap<String, Order>,
    pub environment: Option<Environment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake: Option<WakeConfiguration>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Environment {
    pub source: String,
    pub layer_m: Option<f64>,
    pub reverberant: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Target {
    pub source: String,
    pub size: usize,
    pub noise: usize,
    pub anechoic: bool,
    pub foil_borne: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Setting {
    pub speed_kts: f64,
    pub range_nm: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Guidance {
    Gyro1,
    Gyro2,
    Passive,
    ActivePassive,
    Wire,
    DualWire,
    Wake,
    Shkval,
}
impl Guidance {
    fn wired(&self) -> bool {
        matches!(self, Self::Wire | Self::DualWire)
    }
    fn acoustic(&self) -> bool {
        matches!(
            self,
            Self::Passive | Self::ActivePassive | Self::Wire | Self::DualWire
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Weapon {
    pub source: String,
    pub guidance: Guidance,
    pub generation: usize,
    pub settings: Vec<Setting>,
    pub max_depth_m: f64,
    pub shallow_water_capable: bool,
    pub surface: bool,
    pub submarine: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_offset: Option<WakeLaunchOffset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_sea_state: Option<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Platform {
    pub source: String,
    pub wire_capacity: u8,
    pub reload_parallel: u8,
    pub tubes: Vec<Tube>,
    /// Explicit source/scenario physical IDs; never infer these from class or
    /// assign a minelayer's independent list to the torpedo vector by position.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tube_ids: BTreeMap<String, usize>,
    pub reserves: BTreeMap<String, u16>,
    #[serde(default)]
    pub hardkill: BTreeMap<String, HardkillFit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HardkillFit {
    pub source: String,
    pub weapon: String,
    pub range_nm: f64,
    pub reload_s: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tube {
    pub bearing_deg: f64,
    pub loaded: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeclaredSolution {
    pub quality: Quality,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Order {
    pub source: String,
    pub speed_index: usize,
    pub depth_m: f64,
    pub enable_run_nm: f64,
    pub bearing_deg: Option<f64>,
    #[serde(default)]
    pub spread_deg: Vec<f64>,
    pub cut_wire: bool,
    pub solution: Option<DeclaredSolution>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Configuration,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interference: Option<interference::Runtime>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plans: BTreeMap<String, Plan>,
    pub flights: BTreeMap<EntityId, Flight>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub launches: BTreeMap<EntityId, LaunchRecord>,
    pub tubes: BTreeMap<EntityId, Platform>,
    pub reloads: Vec<Reload>,
    pub placements: super::firecontrol2026::State,
    pub salvos: BTreeMap<EntityId, Salvo>,
    pub previous_positions: BTreeMap<EntityId, Vec2>,
    pub motion: BTreeMap<EntityId, Motion>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub previous_depths: BTreeMap<EntityId, f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub depth_motion: BTreeMap<EntityId, DepthMotion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wakes: Option<WakeState>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reload {
    pub actor: EntityId,
    pub tube: usize,
    pub weapon: String,
    pub due: i64,
    pub changing: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
}
/// Shooter knowledge kept independently of the physical projectile lifecycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchRecord {
    pub shooter: EntityId,
    pub side: crate::Side,
    pub target: Option<EntityId>,
    pub contact: Option<crate::ContactId>,
    pub weapon: String,
    pub launch_tick: i64,
    pub order: Option<Order>,
    pub guidance: Option<Guidance>,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Flight {
    pub shooter: EntityId,
    pub target: EntityId,
    pub weapon: Weapon,
    pub order: Order,
    pub origin: Vec2,
    pub initial_range: f64,
    pub initial_course: f64,
    pub launch_course: f64,
    pub shooter_depth: f64,
    pub launch_tick: i64,
    pub wire: bool,
    pub wire_check: i64,
    pub quality: Quality,
    pub run_nm: f64,
    pub fuel: f64,
    pub speed_index: usize,
    pub phase: Phase,
    pub pass: u32,
    pub placement: Option<Placement>,
    pub threat: Option<Threat>,
    pub last_probability: Option<f64>,
    pub last_d100: Option<u8>,
    pub tube: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salvo: Option<EntityId>,
    /// Physical entry datum is independent of the shooter/target lifecycle.
    /// The weapon loop processes it after every same-tick drop registered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub airdrop_entry: Option<AirdropEntry>,
    /// H1 private physical progress; absent historical Search states are not
    /// silently migrated, and global pending decisions retain their meaning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical: Option<HelicalState>,
    /// Physical homing target is separate from the immutable release target,
    /// original failed Placement and public declaration history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisition: Option<AcquisitionRecord>,
    /// 10.2.7 opportunity target captured after a failed reacquisition: a
    /// mobile decoy (attacked through its EW object with the Torpedo-ACMs
    /// column) or another real unit (which simply becomes the physical
    /// attack target). None on every historical flight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity: Option<Opportunity>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AirdropEntry {
    pub tick: i64,
    pub position: Vec2,
    pub course_deg: f64,
    pub resolved_at: Option<i64>,
    #[serde(default, skip_serializing_if = "false_value")]
    pub source_standoff_placed: bool,
}
fn false_value(value: &bool) -> bool {
    !*value
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Phase {
    Run,
    Locked,
    Past {
        remaining_nm: f64,
    },
    Search {
        center: Vec2,
        heading: f64,
        angle: f64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Threat {
    pub course: f64,
    pub speed: f64,
    pub depth_band: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub actor: EntityId,
    pub side: crate::Side,
    pub contact: crate::ContactId,
    pub weapon: String,
    pub plotted_at: i64,
    pub speed_index: usize,
    pub depth_m: f64,
    pub enable_run_nm: f64,
    pub bearing_deg: Option<f64>,
    pub spread_deg: Vec<f64>,
    pub cut_wire: bool,
}
impl Plan {
    fn action(&self) -> Action {
        Action::Plot {
            contact: self.contact,
            weapon: self.weapon.clone(),
            speed_index: self.speed_index,
            depth_m: self.depth_m,
            enable_run_nm: self.enable_run_nm,
            bearing_deg: self.bearing_deg,
            spread_deg: self.spread_deg.clone(),
            cut_wire: self.cut_wire,
        }
    }
    fn order(&self) -> Order {
        Order {
            source: "6.3.7/7.5.2 own prelaunch Plot declaration".into(),
            speed_index: self.speed_index,
            depth_m: self.depth_m,
            enable_run_nm: self.enable_run_nm,
            bearing_deg: self.bearing_deg,
            spread_deg: self.spread_deg.clone(),
            cut_wire: self.cut_wire,
            // A declaration supplies no fire-control quality. The shooter's
            // measured source FCS remains an independent release condition.
            solution: None,
        }
    }
}
fn plan_key(actor: EntityId, weapon: &str) -> String {
    format!("{}:{weapon}", actor.0)
}
fn required_optional_bearing<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<f64>::deserialize(deserializer)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Plot {
        contact: crate::ContactId,
        weapon: String,
        speed_index: usize,
        depth_m: f64,
        enable_run_nm: f64,
        #[serde(deserialize_with = "required_optional_bearing")]
        bearing_deg: Option<f64>,
        spread_deg: Vec<f64>,
        cut_wire: bool,
    },
    Reload {
        tube: usize,
        weapon: String,
    },
    CutWire {
        torpedo: EntityId,
    },
    WireOrder {
        torpedo: EntityId,
        speed_index: usize,
        depth_m: f64,
        bearing_deg: f64,
        enable_run_nm: f64,
    },
    Hardkill {
        contact: crate::ContactId,
        system: String,
    },
}
pub fn validate_plot(
    w: &World,
    side: crate::Side,
    actor: EntityId,
    action: &Action,
) -> Result<Plan, String> {
    let Action::Plot {
        contact,
        weapon,
        speed_index,
        depth_m,
        enable_run_nm,
        bearing_deg,
        spread_deg,
        cut_wire,
    } = action
    else {
        return Err("6.3.7 expected a prelaunch Plot declaration".into());
    };
    if !active(w) {
        return Err("6.3.7 source torpedo Plot requiresv4".into());
    }
    let e = w
        .entity(actor)
        .filter(|e| {
            e.side == side
                && e.alive
                && !e.kind.is_projectile()
                && super::damage_runtime::weapons_operational(e)
        })
        .ok_or("6.3.7 own operational Plot actor required")?;
    if !e
        .weapons
        .iter()
        .any(|m| m.weapon == *weapon && m.mounts > 0)
        || !w
            .db
            .weapons
            .get(weapon)
            .is_some_and(|d| matches!(d.kind, WeaponKind::Torpedo | WeaponKind::Lwt))
    {
        return Err("6.3.7 Plot requires an own carried torpedo weapon".into());
    }
    let p = w
        .rules
        .torpedoes
        .as_ref()
        .and_then(|state| state.config.weapons.get(weapon))
        .ok_or("7.5 Plot source torpedo annex binding absent")?;
    validate_weapon(p)?;
    let setting = p
        .settings
        .get(*speed_index)
        .ok_or("7.5.7 Plot source speed_index setting missing")?;
    let c = w.sides[side.index()]
        .contacts
        .get(contact)
        .filter(|c| !c.stale && !c.bearing_only)
        .filter(|c| {
            let point = c.estimate_at(w.tick);
            point.x.is_finite() && point.y.is_finite()
        })
        .ok_or("10.2 Plot requires a selected own current located contact")?;
    // This category is the side's stored knowledge. No target body's alive,
    // position, depth or acoustic metadata is consulted during plotting.
    let known = super::detection2026::known_category(w, side, c.about)
        .ok_or("10.2 Plot target domain not known to shooter")?;
    if !(known == PlatformKind::Sub && p.submarine || known == PlatformKind::Ship && p.surface) {
        return Err("7.5 Plot torpedo cannot attack this known target domain".into());
    }
    if !depth_m.is_finite()
        || !(0. ..=p.max_depth_m).contains(depth_m)
        || !enable_run_nm.is_finite()
        || !(0. ..=setting.range_nm).contains(enable_run_nm)
        || bearing_deg.is_some_and(|v| !v.is_finite() || !(0. ..360.).contains(&v))
        || spread_deg.len() > u8::MAX as usize
        || spread_deg
            .iter()
            .any(|v| !v.is_finite() || !(-180. ..=180.).contains(v))
    {
        return Err("6.3.7/7.5 Plot course/depth/enable/spread outside source limits".into());
    }
    if *cut_wire && !p.guidance.wired() {
        return Err("7.5.4 Plot cut_wire requires source wire guidance".into());
    }
    if p.guidance.acoustic() {
        for (i, a) in spread_deg.iter().enumerate() {
            if spread_deg[..i]
                .iter()
                .any(|b| angle_diff(*a, *b).abs() < 15.)
            {
                return Err("7.5.2 Plot acoustic salvo course separation under15deg".into());
            }
        }
    }
    Ok(Plan {
        actor,
        side,
        contact: *contact,
        weapon: weapon.clone(),
        plotted_at: w.tick,
        speed_index: *speed_index,
        depth_m: *depth_m,
        enable_run_nm: *enable_run_nm,
        bearing_deg: *bearing_deg,
        spread_deg: spread_deg.clone(),
        cut_wire: *cut_wire,
    })
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let mut config = w.context.torpedo_2026.clone().unwrap_or_default();
    if !config.source.is_empty() && config.source.trim().is_empty() {
        return Err("torpedo configuration source empty".into());
    }
    for (id, value) in tables()["weapons"].as_object().unwrap() {
        config
            .weapons
            .entry(id.clone())
            .or_insert(serde_json::from_value(value.clone()).unwrap());
    }
    for p in config.weapons.values() {
        validate_weapon(p)?;
    }
    if let Some(e) = &config.environment {
        if e.source.trim().is_empty() || e.layer_m.is_some_and(|v| !v.is_finite() || v < 0.) {
            return Err("7.5.6 source water environment invalid".into());
        }
    }
    for t in config.targets.values() {
        if t.source.trim().is_empty() || t.size > 7 || t.noise > 4 {
            return Err("10 source target size/noise metadata invalid".into());
        }
    }
    for o in config.orders.values() {
        validate_order(o)?;
    }
    let mut state = State {
        config,
        ..Default::default()
    };
    for e in w.entities.values().filter(|e| !e.kind.is_projectile()) {
        if let Some(p) = state.config.platforms.get(&e.class) {
            if p.source.trim().is_empty()
                || p.reload_parallel == 0
                || p.tubes.iter().any(|t| !t.bearing_deg.is_finite())
                || p.tube_ids
                    .iter()
                    .any(|(id, index)| id.trim().is_empty() || *index >= p.tubes.len())
                || p.tube_ids.values().collect::<BTreeSet<_>>().len() != p.tube_ids.len()
            {
                return Err("10.1 source tube inventory/arc/reload fit missing".into());
            }
            state.tubes.insert(e.id, p.clone());
        }
    }
    if let Some(config) = state.config.wake.as_ref() {
        state.wakes = Some(wake::initialize(w, config)?);
    }
    state.previous_positions = w
        .entities
        .values()
        .filter(|e| !e.kind.is_projectile())
        .map(|e| (e.id, e.pos))
        .collect();
    state.previous_depths = w
        .entities
        .values()
        .filter(|e| !e.kind.is_projectile())
        .map(|e| (e.id, e.depth_m))
        .collect();
    w.rules.torpedoes = Some(state);
    Ok(())
}
fn validate_weapon(p: &Weapon) -> Result<(), String> {
    if p.source.trim().is_empty()
        || p.generation > 4
        || (p.guidance.acoustic() || p.guidance == Guidance::Wake) && p.generation == 0
        || p.settings.is_empty()
        || p.settings.iter().any(|s| {
            !s.speed_kts.is_finite()
                || s.speed_kts <= 0.
                || !s.range_nm.is_finite()
                || s.range_nm <= 0.
        })
        || !p.max_depth_m.is_finite()
        || p.max_depth_m <= 0.
        || p.launch_offset.as_ref().is_some_and(|o| {
            o.source.trim().is_empty()
                || !o.degrees.is_finite()
                || !(0. ..=180.).contains(&o.degrees)
        })
        || p.max_sea_state.is_some_and(|sea| sea > 9)
    {
        Err("7.5 source torpedo parameters invalid".into())
    } else {
        Ok(())
    }
}
fn validate_order(o: &Order) -> Result<(), String> {
    if o.source.trim().is_empty()
        || !o.depth_m.is_finite()
        || o.depth_m < 0.
        || !o.enable_run_nm.is_finite()
        || o.enable_run_nm < 0.
        || o.bearing_deg.is_some_and(|v| !v.is_finite())
        || o.spread_deg.iter().any(|v| !v.is_finite())
        || o.solution
            .as_ref()
            .is_some_and(|s| s.source.trim().is_empty() || s.quality == Quality::NA)
    {
        Err("6.3/10.2 source firing order invalid".into())
    } else {
        Ok(())
    }
}
pub fn homing_probability(
    generation: usize,
    acm: Option<usize>,
    evading: bool,
) -> Result<f64, String> {
    if !(1..=4).contains(&generation) || acm.is_some_and(|g| !(1..=4).contains(&g)) {
        return Err("10.2.5 unverified seeker/ACM generation".into());
    }
    let row = &tables()["homing"][acm.unwrap_or(1) - 1][generation - 1];
    Ok(row[match (acm, evading) {
        (None, false) => 0,
        (Some(_), false) => 1,
        (None, true) => 2,
        (Some(_), true) => 3,
    }]
    .as_f64()
    .unwrap())
}
pub fn wire_probability(speed: f64, turn_or_depth: bool) -> Result<f64, String> {
    if !speed.is_finite() || speed < 0. {
        return Err("7.5.4 invalid platform speed".into());
    }
    let row = tables()["wire"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r[0].is_null() || speed <= r[0].as_f64().unwrap())
        .unwrap();
    Ok(row[if turn_or_depth { 2 } else { 1 }].as_f64().unwrap())
}
pub fn straight_hits(
    size: &str,
    salvo: usize,
    run: f64,
    stationary: bool,
    d100: u8,
) -> Result<u8, String> {
    if !(1..=6).contains(&salvo) || !(1..=100).contains(&d100) || !run.is_finite() || run < 0. {
        return Err("10.2.3 invalid spread/run/die".into());
    }
    let rows = tables()["straight"]
        .get(size)
        .and_then(|s| s.get(salvo.to_string()))
        .and_then(Value::as_array)
        .ok_or("10.2.3 unknown effective target size")?;
    let mut best = 0;
    for index in 1..rows.len() {
        if (rows[index][0].as_f64().unwrap() - run).abs()
            <= (rows[best][0].as_f64().unwrap() - run).abs()
        {
            best = index;
        }
    }
    if stationary {
        best = best.saturating_sub(1);
    }
    Ok((1..=salvo)
        .rev()
        .find(|n| source_percent_hit(rows[best][*n].as_f64().unwrap(), d100))
        .unwrap_or(0) as u8)
}
pub fn effective_size(size: usize, aspect: f64) -> Result<&'static str, String> {
    if size >= 5 {
        return Err("10.2.3 F/G craft cannot be hit by unguided torpedoes".into());
    }
    if !aspect.is_finite() {
        return Err("10.2.3 aspect unknown".into());
    }
    let a = angle_diff(aspect, 0.).abs();
    let column = if a <= 20. || a >= 160. {
        2
    } else if (60. ..=120.).contains(&a) {
        0
    } else {
        1
    };
    tables()["target_aspects"][["broad", "quarter", "narrow"][column]][size]
        .as_str()
        .ok_or("10.2.3 source target-aspect mapping missing".into())
}
pub fn acquisition_nm(
    generation: usize,
    active: bool,
    noise: usize,
    cross_layer: bool,
    reverberant: bool,
    shallow_capable: bool,
) -> Result<f64, String> {
    if !(1..=4).contains(&generation) || noise > 4 {
        return Err("7.5.6 unknown seeker/noise".into());
    }
    let yards = tables()["acquisition_yards"][generation - 1][if active { 0 } else { 1 }]
        .as_f64()
        .unwrap();
    let noise = if active {
        1.
    } else {
        [2., 2., 1., 0.5, 0.25][noise]
    };
    let reverb = if active && reverberant && !shallow_capable {
        if generation <= 3 {
            0.5
        } else {
            0.75
        }
    } else {
        1.
    };
    Ok(yards_nm(yards) * noise * if cross_layer { 0.5 } else { 1. } * reverb)
}
fn yards_nm(yards: f64) -> f64 {
    yards * tables()["constants"]["yards_to_nm"].as_f64().unwrap()
}
pub fn depth_band(depth: f64) -> Result<usize, String> {
    if !depth.is_finite() || depth < 0. {
        return Err("invalid depth".into());
    }
    Ok(tables()["depth_limits_m"]
        .as_array()
        .unwrap()
        .iter()
        .position(|v| depth <= v.as_f64().unwrap())
        .unwrap_or(12))
}
/// §3.2.1/§10.3: runtime depth zero denotes surfaced, a separate operation.
/// This envelope is necessary, not proof of a loaded source missile/VLS fit.
pub fn submerged_launch_legal(depth: f64, speed: f64) -> bool {
    let p = &tables()["submerged_launch"];
    depth.is_finite()
        && speed.is_finite()
        && depth > 0.
        && depth <= p["max_depth_m"].as_f64().unwrap()
        && speed >= p["min_speed_kts"].as_f64().unwrap()
        && speed <= p["max_speed_kts"].as_f64().unwrap()
}
pub fn submerged_launch_gate(w: &World, actor: EntityId, def: &WeaponDef) -> Result<(), String> {
    if !active(w)
        || !matches!(
            def.kind,
            WeaponKind::Ascm | WeaponKind::Slcm | WeaponKind::AswRocket
        )
    {
        return Ok(());
    }
    let e = w.entity(actor).ok_or("10.3 shooter missing")?;
    if e.kind == PlatformKind::Sub && !submerged_launch_legal(e.depth_m, e.speed_kts) {
        return Err("10.3 submerged missile/standoff launch requires Periscope/Shallow depth and 3–10 knots; surfaced launch has no bound source procedure".into());
    }
    Ok(())
}
pub fn evasion(threat: &Threat, target: &crate::world::Entity, wake: bool) -> bool {
    let turn = angle_diff(target.course_deg, threat.course).abs();
    if wake {
        return target.speed_kts
            >= tables()["constants"]["wake_evasion_minimum_kts"]
                .as_f64()
                .unwrap()
            && turn
                >= tables()["constants"]["wake_evasion_minimum_turn_deg"]
                    .as_f64()
                    .unwrap();
    }
    let speed = target.speed_kts >= 20. && target.speed_kts - threat.speed >= 5.;
    let depth = depth_band(target.depth_m).is_ok_and(|b| b != threat.depth_band);
    usize::from(speed) + usize::from(turn >= 45.) + usize::from(depth) >= 2
}
pub fn hardkill_probability(system: &str, acoustic: bool) -> Result<f64, String> {
    tables()["hardkill"]
        .get(system)
        .and_then(|r| r[usize::from(acoustic)].as_f64())
        .ok_or("10.5 hardkill system not source-verified".into())
}
/// Compare the D100 fraction to the printed decimal, without multiplying Ph
/// into a binary value just below the inclusive integer boundary (29%,58%).
fn source_percent_hit(probability: f64, d100: u8) -> bool {
    probability.is_finite()
        && (0. ..=1.).contains(&probability)
        && (1..=100).contains(&d100)
        && d100 as f64 / 100. <= probability
}
fn hardkill(
    w: &mut World,
    actor: EntityId,
    contact: crate::ContactId,
    system: &str,
) -> Result<(), String> {
    let s = w
        .entity(actor)
        .filter(|e| e.alive && e.kind == PlatformKind::Ship)
        .cloned()
        .ok_or("10.5 operational defending ship required")?;
    let target = w.sides[s.side.index()]
        .contacts
        .get(&contact)
        .filter(|c| !c.stale)
        .map(|c| c.about)
        .ok_or("10.5 own detected torpedo required")?;
    if super::detection2026::known_category(w, s.side, target) != Some(PlatformKind::Torpedo) {
        return Err("10.5 torpedo must be classified before hardkill attack".into());
    }
    let state = w
        .rules
        .torpedoes
        .as_ref()
        .ok_or("10 source runtime missing")?;
    let fit = state
        .tubes
        .get(&actor)
        .and_then(|p| p.hardkill.get(system))
        .cloned()
        .ok_or("10.5 exact source hardkill fit/range/ammunition missing")?;
    if fit.source.trim().is_empty()
        || !fit.range_nm.is_finite()
        || fit.range_nm <= 0.
        || !fit.reload_s.is_finite()
        || fit.reload_s <= 0.
    {
        return Err("10.5 source hardkill operating envelope invalid".into());
    }
    let projectile = w
        .entity(target)
        .filter(|e| e.alive)
        .ok_or("10.5 incoming torpedo unavailable")?;
    if s.pos.range_to(projectile.pos) > fit.range_nm {
        return Err("10.5 outside explicitly bound hardkill range".into());
    }
    let acoustic = state
        .flights
        .get(&target)
        .map(|f| f.weapon.guidance.acoustic())
        .ok_or("10.5 incoming guidance not source-bound")?;
    let probability = hardkill_probability(system, acoustic)?;
    let index = s
        .weapons
        .iter()
        .position(|m| m.weapon == fit.weapon && m.mounts > 0 && m.ammo > 0 && m.cooldown_s <= 0.)
        .ok_or("10.5 hardkill launcher empty or busy")?;
    let identity = super::damage_runtime::event_identity(w, actor, "10.5.hardkill");
    let d100 = rng_die(w, actor, &identity, 100);
    let hit = source_percent_hit(probability, d100);
    let mount = &mut w.entity_mut(actor).unwrap().weapons[index];
    mount.ammo -= 1;
    mount.cooldown_s = fit.reload_s;
    if hit {
        retire(w, target);
    }
    referee_trace(
        w,
        actor,
        "10.5.source_hardkill",
        json!({"contact":contact,"system":system,"d100":d100,"probability":probability,"intercepted":hit,"source":fit.source}),
    );
    Ok(())
}
fn rng_die(w: &World, id: EntityId, purpose: &str, sides: u8) -> u8 {
    let mut r = w.rng.derive(crate::Rng::hash_tag(0, id.0, purpose));
    1 + (r.next_f64() * sides as f64) as u8
}
fn trace(w: &mut World, actor: EntityId, rule: &str, detail: Value) {
    super::aaw2026::trace(w, actor, rule, detail)
}
fn referee_trace(w: &mut World, actor: EntityId, rule: &str, detail: Value) {
    let side = w.entity(actor).map(|e| e.side);
    w.emit(
        crate::EventKind::RulesEvent {
            domain: "torpedo2026".into(),
            rule: rule.into(),
            side,
            unit: Some(actor),
            contact: None,
            detail: detail.to_string(),
        },
        crate::Visibility::Referee,
    );
}
fn retire(w: &mut World, id: EntityId) {
    let now = w.tick;
    if let Some(e) = w.entity_mut(id) {
        e.alive = false;
        e.hp = 0.;
        e.last_emit_tick = now - 1000;
    }
}
fn fitted_target_spec(w: &World, state: &State, id: EntityId) -> Result<Target, String> {
    let e = w.entity(id).ok_or("10 target missing")?;
    if let Some(t) = state.config.targets.get(&e.class) {
        return Ok(t.clone());
    }
    if let Some(t) = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| s.config.targets.get(&e.class))
        .filter(|t| !t.source.trim().is_empty())
    {
        return Ok(Target {
            source: t.source.clone(),
            size: t.size,
            noise: t.noise.ok_or("7.5.6 target acoustic signature missing")?,
            anechoic: t.anechoic,
            foil_borne: false,
        });
    }
    Err("10 source target size/noise/coating missing".into())
}
fn effective_target_spec(w: &World, id: EntityId, mut target: Target) -> Result<Target, String> {
    let (noise, coating) =
        super::signature_damage2026::project_acoustic(w, id, Some(target.noise), target.anechoic)?;
    target.noise = noise.ok_or("14.2.3 effective torpedo target noise missing")?;
    target.anechoic = coating;
    Ok(target)
}
fn target_spec(w: &World, state: &State, id: EntityId) -> Result<Target, String> {
    effective_target_spec(w, id, fitted_target_spec(w, state, id)?)
}
pub fn fire_at(
    w: &mut World,
    actor: EntityId,
    target: EntityId,
    mount: usize,
    requested: u8,
) -> Result<u8, String> {
    let side = w.entity(actor).ok_or("10 shooter unavailable")?.side;
    let contacts: Vec<_> = w.sides[side.index()]
        .contacts
        .values()
        .filter(|c| c.about == target && !c.stale && !c.bearing_only)
        .map(|c| c.id)
        .collect();
    if contacts.len() != 1 {
        return Err("10.2 an unambiguous own ContactId is required; referee body identity does not merge reports".into());
    }
    fire_at_contact(w, actor, contacts[0], mount, requested)
}
/// Player fire retains the selected report through admission, aim and ledger.
/// A separate report of the same referee body cannot lend its eligibility.
pub fn fire_at_contact(
    w: &mut World,
    actor: EntityId,
    contact: crate::ContactId,
    mount: usize,
    requested: u8,
) -> Result<u8, String> {
    let e = w
        .entity(actor)
        .filter(|e| e.alive && super::damage_runtime::weapons_operational(e))
        .cloned()
        .ok_or("10 shooter unavailable")?;
    let m = e
        .weapons
        .get(mount)
        .filter(|m| m.mounts > 0 && m.ammo > 0)
        .ok_or("10 empty tube/magazine")?;
    let def =
        w.db.weapons
            .get(&m.weapon)
            .cloned()
            .ok_or("10 weapon missing")?;
    if !matches!(def.kind, WeaponKind::Torpedo | WeaponKind::Lwt) {
        return Err("10 not a torpedo".into());
    }
    let state = w
        .rules
        .torpedoes
        .as_ref()
        .ok_or("10 source runtime missing")?;
    let p = state
        .config
        .weapons
        .get(&m.weapon)
        .cloned()
        .ok_or("10 source torpedo annex binding absent")?;
    let c = w.sides[e.side.index()]
        .contacts
        .get(&contact)
        .filter(|c| !c.stale && !c.bearing_only)
        .filter(|c| {
            let point = c.estimate_at(w.tick);
            point.x.is_finite() && point.y.is_finite()
        })
        .cloned()
        .ok_or("10.2 selected own current located contact required")?;
    let target = c.about;
    let known = super::detection2026::known_category(w, e.side, target)
        .ok_or("10.2 target domain not known to shooter")?;
    if !(known == PlatformKind::Sub && p.submarine || known == PlatformKind::Ship && p.surface) {
        return Err("7.5 source torpedo cannot attack this known target domain".into());
    }
    if e.kind == PlatformKind::Aircraft {
        super::aircombat2026::air_torpedo_gate(w, actor, c.id)?;
    }
    let order = if let Some(plan) = state.plans.get(&plan_key(actor, &def.id)) {
        if plan.actor != actor
            || plan.side != e.side
            || plan.contact != contact
            || plan.weapon != def.id
        {
            return Err("6.3.7 Plot plan is bound to a different actor/selected contact".into());
        }
        validate_plot(w, e.side, actor, &plan.action())?.order()
    } else {
        state
            .config
            .orders
            .get(&format!("{}:{}", e.name, def.id))
            .cloned()
            .ok_or("6.3.7 declare source firing course/speed/depth/enable point")?
    };
    let setting = p
        .settings
        .get(order.speed_index)
        .ok_or("7.5.7 source speed setting missing")?;
    if let Some(maximum) = p.max_sea_state {
        let sea = w
            .context
            .radar_environment
            .as_ref()
            .filter(|e| e.valid())
            .ok_or("10 exact source sea state required for this weapon")?
            .sea_state;
        if sea > maximum {
            return Err("10 source annex sea-state operating limit".into());
        }
    }
    if p.guidance == Guidance::Wake && state.wakes.is_none() {
        return Err("7.5.5 explicit source/scenario wake geometry and prehistory required".into());
    }
    if order.depth_m > p.max_depth_m
        || e.kind != PlatformKind::Aircraft && e.depth_m > p.max_depth_m
    {
        return Err("10.1 launch/run depth exceeds source annex limit".into());
    }
    let env = state
        .config
        .environment
        .as_ref()
        .ok_or("7.5.6 source layer/reverberation environment missing")?;
    let _ = env;
    let quality = if let Some(q) = &order.solution {
        q.quality
    } else {
        super::firecontrol2026::attack_solution(
            w,
            actor,
            target,
            p.generation.max(1),
            setting.speed_kts,
        )?
        .quality
    };
    if quality == Quality::NA {
        return Err("6.3 no attack solution".into());
    }
    let n = if e.kind == PlatformKind::Aircraft {
        requested.min(1)
    } else {
        requested
    }
    .min(m.ammo.min(255) as u8);
    if n == 0 {
        return Err("10 zero requested rounds".into());
    }
    if matches!(p.guidance, Guidance::Gyro1 | Guidance::Gyro2) && n > 6 {
        return Err(
            "10.2.3 source straight-runner table covers at most6 weapons per spread".into(),
        );
    }
    if n > 1 && order.spread_deg.len() < n as usize {
        return Err(
            "7.5.2 active salvo requires declared per-torpedo courses at least15deg apart".into(),
        );
    }
    if p.guidance.acoustic() {
        for a in 0..n as usize {
            for b in 0..a {
                if angle_diff(
                    order.spread_deg.get(a).copied().unwrap_or(0.),
                    order.spread_deg.get(b).copied().unwrap_or(0.),
                )
                .abs()
                    < 15.
                {
                    return Err("7.5.2 acoustic salvo course separation under15deg".into());
                }
            }
        }
    }
    let bearing = order
        .bearing_deg
        .unwrap_or_else(|| e.pos.bearing_to(c.estimate_at(w.tick)));
    if p.guidance.acoustic()
        && e.kind != PlatformKind::Aircraft
        && state.flights.values().any(|f| {
            f.shooter == actor
                && w.tick - f.launch_tick < 180
                && f.weapon.guidance.acoustic()
                && (0..n as usize).any(|i| {
                    angle_diff(
                        f.launch_course,
                        bearing + order.spread_deg.get(i).copied().unwrap_or(0.),
                    )
                    .abs()
                        < 15.
                })
        })
    {
        return Err(
            "7.5.2 successive acoustic salvo courses must be separated by at least15deg".into(),
        );
    }
    let mut tube_indexes = Vec::new();
    if e.kind != PlatformKind::Aircraft {
        let fit = state
            .tubes
            .get(&actor)
            .ok_or("10.1 source physical tube loads/arc not declared")?;
        let offset = if p.guidance == Guidance::Wake {
            p.launch_offset
                .as_ref()
                .ok_or("10.2.1 Wake launch offset needs explicit source/scenario binding")?
                .degrees
        } else {
            tables()["angle_offsets"]
                .get(format!("{:?}", p.guidance))
                .and_then(Value::as_f64)
                .ok_or("10.2.1 launch offset not source-verified")?
        };
        for (i, tube) in fit
            .tubes
            .iter()
            .enumerate()
            .filter(|(_, t)| t.loaded.as_deref() == Some(def.id.as_str()))
        {
            let off = order
                .spread_deg
                .get(tube_indexes.len())
                .copied()
                .unwrap_or(0.);
            if angle_diff(e.course_deg + tube.bearing_deg, bearing + off).abs() <= offset {
                tube_indexes.push(i);
            }
            if tube_indexes.len() == n as usize {
                break;
            }
        }
        if tube_indexes.len() != n as usize {
            return Err(
                "10.1/10.2.1 insufficient loaded tubes bearing within source offset".into(),
            );
        }
        if p.guidance.wired() && !order.cut_wire {
            let guiding = state
                .flights
                .iter()
                .filter(|(_, f)| f.shooter == actor && f.wire)
                .count();
            if guiding + n as usize > fit.wire_capacity as usize {
                return Err("6.4 source wire control channel limit exceeded".into());
            }
        }
    }
    let launch_positions: BTreeMap<_, _> = w
        .entities
        .values()
        .filter(|e| !e.kind.is_projectile())
        .map(|e| (e.id, e.pos))
        .collect();
    let mut fired = 0;
    let mut launched = Vec::new();
    for index in 0..n as usize {
        let launch_bearing = bearing + order.spread_deg.get(index).copied().unwrap_or(0.);
        let mut flight_def = def.clone();
        flight_def.speed_kts = setting.speed_kts;
        flight_def.range_nm = setting.range_nm;
        let id = crate::weapons::launch(
            w,
            actor,
            &flight_def,
            Some(target),
            Some(
                e.pos
                    .advanced(launch_bearing, e.pos.range_to(c.estimate_at(w.tick))),
            ),
        )
        .ok_or("10 source launch failed")?;
        let wire =
            p.guidance.wired() && !order.cut_wire && rng_die(w, id, "7.5.4.launch_wire", 100) > 20;
        let phase = if e.kind == PlatformKind::Aircraft {
            Phase::Search {
                center: e.pos,
                heading: launch_bearing,
                angle: 0.,
            }
        } else {
            Phase::Run
        };
        let flight = Flight {
            shooter: actor,
            target,
            weapon: p.clone(),
            order: order.clone(),
            origin: e.pos,
            initial_range: e.pos.range_to(c.estimate_at(w.tick)),
            initial_course: e.course_deg,
            launch_course: launch_bearing,
            shooter_depth: e.depth_m,
            launch_tick: w.tick,
            wire,
            wire_check: w.tick,
            quality,
            run_nm: 0.,
            fuel: 0.,
            speed_index: order.speed_index,
            phase,
            pass: 0,
            placement: None,
            threat: None,
            last_probability: None,
            last_d100: None,
            tube: tube_indexes.get(index).copied(),
            salvo: None,
            airdrop_entry: (e.kind == PlatformKind::Aircraft).then_some(AirdropEntry {
                tick: w.tick,
                position: e.pos,
                course_deg: launch_bearing,
                resolved_at: None,
                source_standoff_placed: false,
            }),
            helical: None,
            acquisition: None,
            opportunity: None,
        };
        let projectile = w.entity_mut(id).unwrap();
        projectile.pos = e.pos;
        projectile.course_deg = launch_bearing;
        projectile.depth_m = order.depth_m;
        projectile.speed_kts = setting.speed_kts;
        let state = w.rules.torpedoes.as_mut().unwrap();
        state.launches.insert(
            id,
            LaunchRecord {
                shooter: actor,
                side: e.side,
                target: Some(target),
                contact: Some(c.id),
                weapon: def.id.clone(),
                launch_tick: w.tick,
                order: Some(order.clone()),
                guidance: Some(p.guidance.clone()),
                source: p.source.clone(),
            },
        );
        state.flights.insert(id, flight);
        if let Some(tube) = tube_indexes.get(index) {
            state.tubes.get_mut(&actor).unwrap().tubes[*tube].loaded = None;
        }
        let launch_die = if p.guidance.wired() && !order.cut_wire {
            Some(rng_die(w, id, "7.5.4.launch_wire", 100))
        } else {
            None
        };
        let physical_launch=w.entity(id).map(|body|json!({"kind":body.kind,"pos":body.pos,"depth_m":body.depth_m}));
        referee_trace(
            w,
            actor,
            "10.2.source_launch",
            json!({"torpedo":id,"generation":p.generation,"quality":quality,"wire_intact":wire,"source":p.source,"launch_wire_d100":launch_die,"physical_launch":physical_launch}),
        );
        fired += 1;
        launched.push(id);
    }
    if matches!(p.guidance, Guidance::Gyro1 | Guidance::Gyro2) {
        let id = launched[0];
        let state = w.rules.torpedoes.as_mut().unwrap();
        for member in &launched {
            state.flights.get_mut(member).unwrap().salvo = Some(id);
        }
        state.salvos.insert(
            id,
            Salvo {
                id,
                shooter: actor,
                origin: e.pos,
                members: launched,
                launch_tick: w.tick,
                launch_positions,
                last_tick: w.tick - 1,
                split: false,
                visited: BTreeMap::new(),
                attacks: Vec::new(),
                pending: None,
            },
        );
    }
    w.entity_mut(actor).unwrap().weapons[mount].ammo -= fired as u16;
    Ok(fired)
}
pub fn apply(w: &mut World, actor: EntityId, action: &Action) -> Result<(), String> {
    if !active(w) {
        return Err("10 source command requiresv4".into());
    }
    if let Action::Hardkill { contact, system } = action {
        return hardkill(w, actor, *contact, system);
    }
    if matches!(action, Action::Plot { .. }) {
        let side = w.entity(actor).ok_or("6.3.7 Plot actor missing")?.side;
        let plan = validate_plot(w, side, actor, action)?;
        w.rules
            .torpedoes
            .as_mut()
            .ok_or("6.3.7 Plot source runtime missing")?
            .plans
            .insert(plan_key(actor, &plan.weapon), plan);
        return Ok(());
    }
    let e = w
        .entity(actor)
        .filter(|e| e.alive)
        .cloned()
        .ok_or("10 actor missing")?;
    let state = w.rules.torpedoes.as_mut().ok_or("10 runtime missing")?;
    match action {
        Action::Plot { .. } => {
            unreachable!("Plot validated and stored before mutable runtime access")
        }
        Action::Reload { tube, weapon } => {
            let fit = state
                .tubes
                .get_mut(&actor)
                .ok_or("10.1 source tube fit missing")?;
            if state
                .reloads
                .iter()
                .any(|j| j.actor == actor && j.tube == *tube)
            {
                return Err("10.1 tube already being serviced".into());
            }
            let current = fit
                .tubes
                .get(*tube)
                .ok_or("10.1 tube index unknown")?
                .loaded
                .clone();
            let changing = current.is_some();
            let busy = state.reloads.iter().filter(|j| j.actor == actor).count();
            if changing && busy > 0
                || state.reloads.iter().any(|j| j.actor == actor && j.changing)
                || busy >= fit.reload_parallel as usize
            {
                return Err("10.1 source reload/change concurrent crew limit".into());
            }
            if state
                .flights
                .values()
                .any(|f| f.shooter == actor && f.tube == Some(*tube) && f.wire)
            {
                return Err("7.5.4 cut the tube guidance wire before reloading".into());
            }
            let stock = fit
                .reserves
                .get_mut(weapon)
                .filter(|n| **n > 0)
                .ok_or("10.1 declared reserve exhausted")?;
            *stock -= 1;
            if let Some(old) = current {
                *fit.reserves.entry(old).or_default() += 1;
            }
            fit.tubes[*tube].loaded = None;
            state.reloads.push(Reload {
                actor,
                tube: *tube,
                weapon: weapon.clone(),
                due: w.tick
                    + tables()["own_status"][if changing {
                        "change_load_seconds"
                    } else {
                        "reload_seconds"
                    }]
                    .as_i64()
                    .unwrap(),
                changing,
                purpose: None,
            });
        }
        Action::CutWire { torpedo } | Action::WireOrder { torpedo, .. } => {
            let f = state
                .flights
                .get_mut(torpedo)
                .filter(|f| f.shooter == actor && f.wire)
                .ok_or("7.5.4 own intact guidance wire required")?;
            match action {
                Action::CutWire { .. } => {
                    f.wire = false;
                    f.order.cut_wire = true;
                }
                Action::WireOrder {
                    speed_index,
                    depth_m,
                    bearing_deg,
                    enable_run_nm,
                    ..
                } => {
                    if f.weapon.settings.get(*speed_index).is_none()
                        || !depth_m.is_finite()
                        || *depth_m < 0.
                        || *depth_m > f.weapon.max_depth_m
                        || !bearing_deg.is_finite()
                        || !enable_run_nm.is_finite()
                        || *enable_run_nm < 0.
                    {
                        return Err("7.5.4 invalid source wire command".into());
                    }
                    f.speed_index = *speed_index;
                    f.order.speed_index = *speed_index;
                    f.order.depth_m = *depth_m;
                    f.order.bearing_deg = Some(*bearing_deg);
                    f.order.enable_run_nm = *enable_run_nm;
                    // Acceptance records an allowed own command. A secret
                    // cleanup cannot change its receipt; physical application
                    // only occurs when the referee still has the body.
                    if let Some(projectile) = w.entity_mut(*torpedo) {
                        projectile.course_deg = *bearing_deg;
                        projectile.depth_m = *depth_m;
                    }
                }
                _ => unreachable!(),
            }
        }
        Action::Hardkill { .. } => unreachable!(),
    }
    if let Action::CutWire { torpedo } | Action::WireOrder { torpedo, .. } = action {
        let state = w.rules.torpedoes.as_mut().unwrap();
        let order = state.flights[torpedo].order.clone();
        if let Some(launch) = state.launches.get_mut(torpedo) {
            launch.order = Some(order);
        }
    }
    trace(
        w,
        actor,
        "10.source_order",
        json!({"action":action,"side":e.side}),
    );
    Ok(())
}
/// A contact learned in the previous sensing stage must capture the defender's
/// own pose before its next reaction commands and movement, not afterwards.
pub fn begin(w: &mut World) {
    if !active(w) {
        return;
    }
    opportunity::begin(w);
    let captures: Vec<_> = w.rules.torpedoes.as_ref().map_or_else(Vec::new, |state| {
        state
            .flights
            .iter()
            .filter_map(|(id, f)| {
                let snapshot = |target_id| {
                    let target = w.entity(target_id).filter(|e| e.alive)?;
                    if super::detection2026::known_category(w, target.side, *id)
                        != Some(PlatformKind::Torpedo)
                        || !w.sides[target.side.index()]
                            .contacts
                            .values()
                            .any(|c| c.about == *id && !c.stale)
                    {
                        return None;
                    }
                    Some(Threat {
                        course: target.course_deg,
                        speed: target.speed_kts,
                        depth_band: depth_band(target.depth_m).ok()?,
                    })
                };
                let original = if f.threat.is_none() {
                    snapshot(f.target)
                } else {
                    None
                };
                // Source10.2.5 measures the actual defender's reaction after
                // detection. Its pose must be captured before this tick's
                // commands, independently of the historical original target.
                let acquired = f
                    .acquisition
                    .as_ref()
                    .filter(|a| a.threat.is_none() && a.terminal.is_none())
                    .and_then(|a| snapshot(a.target));
                (original.is_some() || acquired.is_some()).then_some((*id, original, acquired))
            })
            .collect()
    });
    if let Some(state) = w.rules.torpedoes.as_mut() {
        for (id, original, acquired) in captures {
            if let Some(flight) = state.flights.get_mut(&id) {
                if flight.threat.is_none() {
                    flight.threat = original;
                }
                if let Some(acquisition) = flight
                    .acquisition
                    .as_mut()
                    .filter(|a| a.threat.is_none() && a.terminal.is_none())
                {
                    acquisition.threat = acquired;
                }
            }
        }
    }
}
pub fn step(w: &mut World) {
    if !active(w) {
        return;
    }
    let positions: BTreeMap<_, _> = w
        .entities
        .values()
        .filter(|e| !e.kind.is_projectile())
        .map(|e| (e.id, e.pos))
        .collect();
    let depths: BTreeMap<_, _> = w
        .entities
        .values()
        .filter(|e| !e.kind.is_projectile())
        .map(|e| (e.id, e.depth_m))
        .collect();
    let wake_ships: BTreeSet<_> = w
        .entities
        .values()
        .filter(|e| e.alive && e.kind == PlatformKind::Ship)
        .map(|e| e.id)
        .collect();
    let Some(state) = w.rules.torpedoes.as_mut() else {
        return;
    };
    state.motion = positions
        .iter()
        .map(|(id, to)| {
            (
                *id,
                Motion {
                    from: state.previous_positions.get(id).copied().unwrap_or(*to),
                    to: *to,
                    tick: w.tick,
                },
            )
        })
        .collect();
    state.previous_positions = positions;
    state.depth_motion = depths
        .iter()
        .map(|(id, to)| {
            (
                *id,
                DepthMotion {
                    from: state.previous_depths.get(id).copied(),
                    to: *to,
                    tick: w.tick,
                },
            )
        })
        .collect();
    state.previous_depths = depths;
    wake::step(w.tick, &wake_ships, state);
    let jobs = std::mem::take(&mut state.reloads);
    for j in jobs {
        if j.due <= w.tick {
            if let Some(p) = state.tubes.get_mut(&j.actor) {
                p.tubes[j.tube].loaded = Some(j.weapon);
            }
        } else {
            state.reloads.push(j);
        }
    }
    interference::prepare(w);
}
/// Trusted §8.5.2 carrier-to-payload transition. A physical carrier body and
/// its original launch ownership are retained; this is no second launch and
/// consumes neither a tube nor another round. A successful source Placement
/// itself establishes acquisition and the immediate entry attack. Failed
/// Placement begins source underwater search at the physical scatter datum.
pub fn accept_standoff_payload(
    w: &mut World,
    id: EntityId,
    payload_weapon: &str,
    speed_index: usize,
    target: EntityId,
    quality: Quality,
    placement: Placement,
) -> Result<(), String> {
    if !active(w) {
        return Err("8.5.2 source profile required".into());
    }
    let body = w
        .entity(id)
        .cloned()
        .ok_or("8.5.2 physical carrier body absent")?;
    let original = body
        .projectile
        .as_ref()
        .ok_or("8.5.2 actual delivery metadata absent")?;
    if !w.db.weapons.get(&original.weapon).is_some_and(|d| {
        d.kind == WeaponKind::AswRocket && d.guidance == crate::db::Guidance::AswBallistic
    }) {
        return Err("8.5.2 payload transition requires actual standoff carrier".into());
    }
    if placement.tick != w.tick
        || !placement.identity.starts_with("8.5.2:")
        || original.target != Some(target)
    {
        return Err("8.5.2 original placement/target mismatch".into());
    }
    let state = w
        .rules
        .torpedoes
        .as_ref()
        .ok_or("8.5.2 payload source runtime absent")?;
    if state.flights.contains_key(&id) {
        return Err("8.5.2 payload already deployed".into());
    }
    let weapon = state
        .config
        .weapons
        .get(payload_weapon)
        .cloned()
        .ok_or("8.5.2 payload Annex fit absent")?;
    if !matches!(weapon.guidance, Guidance::ActivePassive | Guidance::Passive) {
        return Err("8.5.2 payload guidance needs separate source reconciliation".into());
    }
    let setting = weapon
        .settings
        .get(speed_index)
        .cloned()
        .ok_or("8.5.2 payload setting absent")?;
    let def =
        w.db.weapons
            .get(payload_weapon)
            .cloned()
            .ok_or("8.5.2 physical payload definition absent")?;
    let _ = target_spec(w, state, target)?;
    if body.depth_m > weapon.max_depth_m {
        return Err("8.5.2 physical entry depth exceeds payload maximum".into());
    }
    let flight = Flight {
        shooter: original.launched_by,
        target,
        weapon: weapon.clone(),
        order: Order {
            source: "8.5.2 source standoff payload; original placement retained".into(),
            speed_index,
            depth_m: body.depth_m,
            enable_run_nm: 0.0,
            bearing_deg: Some(body.course_deg),
            spread_deg: Vec::new(),
            cut_wire: true,
            solution: None,
        },
        origin: body.pos,
        initial_range: 0.0,
        initial_course: body.course_deg,
        launch_course: body.course_deg,
        shooter_depth: body.depth_m,
        launch_tick: w.tick,
        wire: false,
        wire_check: w.tick,
        quality,
        run_nm: 0.0,
        fuel: 0.0,
        speed_index,
        phase: if placement.placed {
            Phase::Locked
        } else {
            Phase::Search {
                center: body.pos,
                heading: body.course_deg,
                angle: 0.0,
            }
        },
        pass: 0,
        placement: Some(placement.clone()),
        threat: None,
        last_probability: None,
        last_d100: None,
        tube: None,
        salvo: None,
        airdrop_entry: Some(AirdropEntry {
            tick: w.tick,
            position: body.pos,
            course_deg: body.course_deg,
            resolved_at: None,
            source_standoff_placed: placement.placed,
        }),
        helical: None,
        acquisition: None,
        opportunity: None,
    };
    let e = w.entity_mut(id).unwrap();
    e.kind = PlatformKind::Torpedo;
    e.class = format!("weapon:{payload_weapon}");
    e.speed_kts = setting.speed_kts;
    e.altitude_m = 0.0;
    let p = e.projectile.as_mut().unwrap();
    p.weapon = payload_weapon.into();
    p.live_s = 0.0;
    p.enabled = true;
    let s = w.rules.torpedoes.as_mut().unwrap();
    s.launches.insert(
        id,
        LaunchRecord {
            shooter: original.launched_by,
            side: original.launched_by_side,
            target: Some(target),
            contact: None,
            weapon: payload_weapon.into(),
            launch_tick: w.tick,
            order: Some(flight.order.clone()),
            guidance: Some(weapon.guidance.clone()),
            source: "8.5.2 physical payload, no extra expenditure".into(),
        },
    );
    s.flights.insert(id, flight);
    referee_trace(
        w,
        original.launched_by,
        "8.5.2.payload_entry",
        json!({"projectile":id,"carrier":original.weapon,"payload":payload_weapon,"physical_datum":body.pos,"placement":placement,"no_second_placement":true}),
    );
    if let Err(reason) = advance(w, id, &def) {
        defer(w, id, target, &reason);
        return Err(reason);
    }
    Ok(())
}

pub fn advance(w: &mut World, id: EntityId, def: &WeaponDef) -> Result<(), String> {
    let mut state = w
        .rules
        .torpedoes
        .take()
        .ok_or("10 source runtime missing")?;
    // Canonical sim::tick already collects simultaneous damage. Public
    // standalone advancement must also restore the complete source runtime
    // before committing any real hit; taking state is not loss of provenance.
    let own_damage_batch =
        w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full && !w.rules.in_batch;
    if own_damage_batch {
        super::damage_runtime::begin(w);
    }
    let salvo = state.flights.get(&id).and_then(|f| f.salvo);
    let is_wake = state
        .flights
        .get(&id)
        .is_some_and(|f| f.weapon.guidance == Guidance::Wake);
    let result = match interference::advance(w, &mut state, id) {
        Ok(true) => Ok(()),
        Err(reason) => Err(reason),
        Ok(false) => {
            if is_wake {
                wake::advance(w, &mut state, id, def)
            } else if salvo.is_some() {
                straight::advance(w, &mut state, id, def)
            } else {
                advance_inner(w, &mut state, id, def)
            }
        }
    };
    let pending = salvo.and_then(|id| state.salvos.get(&id)).and_then(|s| {
        s.pending
            .as_ref()
            .map(|(target, reason)| (s.members.clone(), *target, reason.clone()))
    });
    let interference_pending = interference::pending_records(&state);
    w.rules.torpedoes = Some(state);
    for (member, target, reason) in interference_pending {
        if w.entity(member).is_some_and(|e| e.alive) {
            defer(w, member, target, &reason);
        }
    }
    if let Some((members, target, reason)) = pending {
        for member in members {
            if w.entity(member).is_some_and(|e| e.alive) {
                defer(w, member, target, &reason);
            }
        }
    }
    if own_damage_batch {
        super::damage_runtime::finish(w);
    }
    result
}
pub fn defer(w: &mut World, id: EntityId, target: EntityId, reason: &str) {
    let target = w
        .rules
        .torpedoes
        .as_ref()
        .and_then(|state| {
            interference::pending_target(state, id).or_else(|| {
                let flight = state.flights.get(&id)?;
                flight
                    .opportunity
                    .as_ref()
                    .map(|opportunity| opportunity.object)
                    .or_else(|| {
                        flight
                            .acquisition
                            .as_ref()
                            .map(|acquisition| acquisition.target)
                    })
            })
        })
        .unwrap_or(target);
    if let Some(projectile) = w.entity(id).cloned() {
        if w.rules.pending_adjudications.contains_key(&id) {
            return;
        }
        let evidence=w.events.iter().filter(|e|matches!(&e.kind,crate::EventKind::RulesEvent{rule,..} if rule.starts_with("10.")||rule.starts_with("7.5."))).cloned().collect();
        w.rules.pending_adjudications.entry(id).or_insert(
            super::damage_runtime::PendingAdjudication {
                tick: w.tick,
                rule: format!("10.source_torpedo: {reason}"),
                target,
                projectile,
                evidence,
            },
        );
        referee_trace(
            w,
            id,
            "10.pending",
            json!({"status":"pending","reason":reason,"target":target}),
        );
    }
}
fn advance_inner(
    w: &mut World,
    state: &mut State,
    id: EntityId,
    def: &WeaponDef,
) -> Result<(), String> {
    let mut f = state
        .flights
        .remove(&id)
        .ok_or("10 unbound legacy torpedo cannot use source adjudication")?;
    let result = advance_flight(w, state, id, def, &mut f);
    state.flights.insert(id, f);
    result
}
fn advance_flight(
    w: &mut World,
    state: &mut State,
    id: EntityId,
    def: &WeaponDef,
    f: &mut Flight,
) -> Result<(), String> {
    // Self-contained underwater search must not depend on a live launcher or
    // on the original target retaining its body/class/signature metadata.
    if f.acquisition.is_some() {
        return initial_acquisition::advance(w, state, id, def, f);
    }
    if f.helical.is_some() {
        return helical::advance(w, state, id, f, 1.);
    }
    if f.opportunity.is_some() {
        return opportunity::advance(w, state, id, def, f);
    }
    let entry = f
        .airdrop_entry
        .as_ref()
        .is_some_and(|e| e.resolved_at.is_none() && f.pass == 0);
    let standoff_placed_entry = entry
        && f.airdrop_entry
            .as_ref()
            .is_some_and(|e| e.source_standoff_placed);
    if let Some(datum) = f.airdrop_entry.as_ref().filter(|_| entry) {
        if datum.tick != w.tick
            || !w.entity(id).is_some_and(|e| e.pos == datum.position)
            || f.run_nm != 0.
        {
            return Err(
                "9.7.1 entry datum was not processed before movement; original flight retained"
                    .into(),
            );
        }
        if f.order.enable_run_nm > 0. {
            return Err("9.7.1 immediate entry attack with a delayed enable declaration requires source reconciliation".into());
        }
    }
    let target = w
        .entity(f.target)
        .filter(|e| e.alive)
        .cloned()
        .ok_or("10 target lost; source search/reassignment required")?;
    let tp = target_spec(w, state, target.id)?;
    let env = state
        .config
        .environment
        .as_ref()
        .ok_or("7.5 source environment missing")?
        .clone();
    if !f.weapon.guidance.acoustic() {
        return Err(
            "10.2.2/10.2.6 source straight/wake motion not yet bound; no legacy fallback".into(),
        );
    }
    if f.wire && w.tick - f.wire_check >= 180 {
        f.wire_check = w.tick;
        if let Some(s) = w.entity(f.shooter) {
            let turn = angle_diff(s.course_deg, f.initial_course).abs() > 45.
                || s.depth_m != f.shooter_depth;
            let probability = if s.kind == PlatformKind::Aircraft {
                if s.speed_kts > 50. {
                    1.
                } else {
                    0.
                }
            } else {
                wire_probability(s.speed_kts, turn)?
            };
            let d100 = rng_die(w, id, &format!("7.5.4.wire:{}", w.tick), 100);
            if source_percent_hit(probability, d100) {
                f.wire = false;
            }
            referee_trace(
                w,
                f.shooter,
                "7.5.4.wire_check",
                json!({"torpedo":id,"d100":d100,"probability":probability,"intact":f.wire}),
            );
        } else {
            f.wire = false;
        }
    }
    if f.threat.is_none()
        && w.sides[target.side.index()]
            .contacts
            .values()
            .any(|c| c.about == id && !c.stale)
    {
        f.threat = Some(Threat {
            course: target.course_deg,
            speed: target.speed_kts,
            depth_band: depth_band(target.depth_m)?,
        });
    }
    let evading = f
        .threat
        .as_ref()
        .is_some_and(|t| evasion(t, &target, false));
    let acms = super::ew2026::active_acms(w, target.id);
    let acm = acms
        .iter()
        .max_by_key(|a| a.generation)
        .map(|a| a.generation as usize);
    // RTS-1 (10.2.7 reattack contract): the 1000yd Past runs its real distance
    // before any reacquisition. RTS-3: on the reattack circle the second
    // Weapon Placement die fires once the swept course points back within the
    // 60-degree off-axis window. The initial placement keeps its existing
    // entry/half-run window and failure semantics untouched.
    let in_past = matches!(f.phase, Phase::Past { .. });
    let in_search = matches!(f.phase, Phase::Search { .. });
    let me = w.entity(id).ok_or("torpedo missing")?;
    let reattack_window = in_search
        && f.pass > 0
        && angle_diff(me.course_deg, me.pos.bearing_to(target.pos)).abs() <= 60.;
    let initial_window = f.pass == 0 && !in_past && (entry || f.run_nm >= f.initial_range / 2.);
    if f.placement.is_none() && (initial_window || reattack_window) {
        let modifier = if tp.anechoic && f.weapon.guidance != Guidance::Passive {
            -2
        } else {
            0
        } + if env.reverberant && !f.weapon.shallow_water_capable {
            if f.weapon.generation <= 3 {
                -2
            } else {
                -1
            }
        } else {
            0
        };
        let reattack = in_search && f.pass > 0;
        let p = super::firecontrol2026::placement(
            w,
            &mut state.placements,
            &format!("10.placement:{}:{}", id.0, f.pass),
            f.quality,
            modifier,
        )?;
        if !p.placed && !reattack {
            // Initial-placement failure keeps its existing deviation and
            // immediate-circle semantics. A failed reacquisition stays on the
            // circle: no deviation, no phase change, no second roll for the
            // same pass (identity cache), until fuel runs out.
            let e = w.entity_mut(id).ok_or("torpedo missing")?;
            e.course_deg += p.course_deviation_deg as f64;
            f.phase = Phase::Search {
                center: e.pos,
                heading: e.course_deg,
                angle: 0.,
            };
        }
        referee_trace(
            w,
            f.shooter,
            "10.2.4.placement",
            json!({"torpedo":id,"pass":f.pass,"placement":p,"reattack":reattack,"airdrop_entry":if entry{f.airdrop_entry.as_ref()}else{None}}),
        );
        f.placement = Some(p.clone());
        if reattack && p.placed {
            // A reacquired target re-enters the 10.2.4 attack directly: the
            // circle's closest approach can exceed a small seeker's acquisition
            // radius, and the Placement die is the source reacquisition
            // decision itself.
            f.phase = Phase::Locked;
        }
    }
    let e = w.entity(id).cloned().ok_or("torpedo missing")?;
    let cross = env
        .layer_m
        .is_some_and(|l| (e.depth_m > l) != (target.depth_m > l));
    let active = f.weapon.guidance != Guidance::Passive && f.run_nm >= f.order.enable_run_nm;
    let acquisition = acquisition_nm(
        f.weapon.generation,
        active,
        tp.noise,
        cross,
        env.reverberant,
        f.weapon.shallow_water_capable,
    )?;
    let physical_domain = match target.combat_kind() {
        PlatformKind::Ship => f.weapon.surface,
        PlatformKind::Sub => f.weapon.submarine,
        _ => false,
    };
    let eligible = physical_domain
        && !tp.foil_borne
        && f.placement.as_ref().is_some_and(|p| p.placed)
        && target.depth_m <= f.weapon.max_depth_m
        && (standoff_placed_entry
            || (e.pos.range_to(target.pos) <= acquisition
                && angle_diff(e.course_deg, e.pos.bearing_to(target.pos)).abs() <= 60.));
    if matches!(f.phase, Phase::Run | Phase::Search { .. }) && eligible {
        f.phase = Phase::Locked;
        f.speed_index = f
            .weapon
            .settings
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.speed_kts.total_cmp(&b.speed_kts))
            .unwrap()
            .0;
        referee_trace(
            w,
            f.shooter,
            "7.5.7.acquired",
            json!({"torpedo":id,"source_acquisition_nm":acquisition,"setting":f.speed_index,"remaining_fraction":1.-f.fuel}),
        );
    }
    if entry {
        f.airdrop_entry.as_mut().unwrap().resolved_at = Some(w.tick);
        if !f.placement.as_ref().is_some_and(|p| p.placed) {
            f.helical = Some(helical::initialize(w, f)?);
            referee_trace(
                w,
                f.shooter,
                "7.5.3.helical_start",
                json!({"torpedo":id,"helical":f.helical,"retained_pass":f.pass,"placement":f.placement}),
            );
            return helical::advance(w, state, id, f, 0.);
        }
        if !eligible {
            f.helical = Some(helical::initialize(w, f)?);
            referee_trace(
                w,
                f.shooter,
                "7.5.3.helical_start",
                json!({"torpedo":id,"helical":f.helical,"retained_pass":f.pass,"placement":f.placement}),
            );
            return helical::advance(w, state, id, f, 0.);
        }
    }
    let setting = &f.weapon.settings[f.speed_index];
    let air_circle = matches!(f.phase, Phase::Search { .. })
        && f.pass == 0
        && w.entity(f.shooter)
            .is_some_and(|shooter| shooter.kind == PlatformKind::Aircraft);
    let endurance_factor = if air_circle {
        500. / (std::f64::consts::PI * 150.)
    } else {
        1.
    };
    let physical_budget = (1. - f.fuel).max(0.) * setting.range_nm / endurance_factor;
    let travel = if entry {
        0.
    } else {
        (setting.speed_kts / 3600.).min(physical_budget)
    };
    let mut endurance_distance = travel;
    let mut pos = e.pos;
    let mut course = e.course_deg;
    match &mut f.phase {
        Phase::Run => pos = pos.advanced(course, travel),
        Phase::Locked => {
            course = pos.bearing_to(target.pos);
            endurance_distance = travel.min(pos.range_to(target.pos));
            pos = pos.advanced(course, endurance_distance);
        }
        Phase::Past { remaining_nm } => {
            pos = pos.advanced(course, travel);
            *remaining_nm -= travel;
            if *remaining_nm <= 0. {
                let diameter = yards_nm(
                    tables()["constants"]["reattack_diameter_yards"][f.weapon.generation - 1]
                        .as_f64()
                        .unwrap(),
                );
                f.phase = Phase::Search {
                    center: pos.advanced(course + 90., diameter / 2.),
                    heading: course,
                    angle: 0.,
                };
            }
        }
        Phase::Search {
            center,
            heading,
            angle,
        } => {
            let air = w
                .entity(f.shooter)
                .is_some_and(|s| s.kind == PlatformKind::Aircraft)
                && f.pass == 0;
            let diameter = if air {
                150.
            } else {
                tables()["constants"]["reattack_diameter_yards"][f.weapon.generation - 1]
                    .as_f64()
                    .unwrap()
            };
            let radius = yards_nm(diameter) / 2.;
            *angle += travel / radius;
            course = *heading + angle.to_degrees();
            pos = center.advanced(course - 90., radius);
            if air {
                endurance_distance = travel * 500. / (std::f64::consts::PI * 150.);
            }
        }
    }
    f.run_nm += endurance_distance;
    f.fuel = (f.fuel + endurance_distance / setting.range_nm).min(1.);
    if f.fuel >= 1. - 1e-12 {
        f.fuel = 1.;
    }
    if let Some(e) = w.entity_mut(id) {
        e.pos = pos;
        e.course_deg = course.rem_euclid(360.);
        e.depth_m = f.order.depth_m;
        e.speed_kts = setting.speed_kts;
        if let Some(p) = e.projectile.as_mut() {
            p.live_s += if entry {
                0.
            } else {
                endurance_distance / endurance_factor / (setting.speed_kts / 3600.)
            };
            p.enabled = active;
        }
    }
    let actual_arrival = matches!(f.phase, Phase::Locked)
        && physical_domain
        && !tp.foil_borne
        && target.depth_m <= f.weapon.max_depth_m
        && (entry || pos.range_to(target.pos) <= 1e-9);
    if f.fuel >= 1. && !actual_arrival {
        referee_trace(
            w,
            f.shooter,
            "10.2.source_runout",
            json!({"torpedo":id,"spent_fraction":f.fuel}),
        );
        retire(w, id);
        return Ok(());
    }
    if !actual_arrival && opportunity::scan(w, state, id, f, target.id)? {
        return Ok(()); // this Search tick already paid its whole movement/endurance
    }
    if actual_arrival {
        let probability = homing_probability(f.weapon.generation, acm, evading)?;
        let d100 = rng_die(w, id, &format!("10.2.5.attack:{}", f.pass), 100);
        f.last_probability = Some(probability);
        f.last_d100 = Some(d100);
        let hit = source_percent_hit(probability, d100);
        referee_trace(
            w,
            f.shooter,
            "10.2.5.source_terminal",
            json!({"torpedo":id,"pass":f.pass,"d100":d100,"probability":probability,"evading":evading,"acm_generation":acm,"hit":hit}),
        );
        if hit {
            crate::damage::apply_projectile_hit(
                w,
                id,
                target.id,
                def,
                true,
                w.entity(id)
                    .and_then(|e| e.projectile.as_ref())
                    .map(|p| p.launched_by_side)
                    .or_else(|| w.entity(f.shooter).map(|s| s.side))
                    .unwrap_or(crate::Side::Blue),
            );
            retire(w, id);
        } else {
            f.pass += 1;
            if acm.is_some() || evading {
                f.quality=match f.quality{Quality::Good=>Quality::Fair,Quality::Fair=>Quality::Poor,_=>return Err("10.2.7 Poor reattack floor conflicts with10-6 example NoAttack; original terminal die retained".into())};
            }
            f.placement = None;
            if f.wire {
                let d = rng_die(w, id, &format!("10.2.7.wire_reattack:{}", f.pass), 100);
                if d <= 40 {
                    f.wire = false;
                }
                referee_trace(
                    w,
                    f.shooter,
                    "10.2.7.wire_reattack",
                    json!({"torpedo":id,"d100":d,"wire_intact":f.wire}),
                );
            }
            f.phase = Phase::Past {
                remaining_nm: yards_nm(
                    tables()["constants"]["reattack_past_yards"]
                        .as_f64()
                        .unwrap(),
                ),
            };
            if entry {
                return Err("9.7.1 immediate attack missed;10.2.7 actual overshoot datum and source reacquisition require reconciliation; original D10/D100 and1000yd Past state retained".into());
            }
        }
    }
    // A miss at the exact affordable endpoint retains its real terminal die,
    // then exhausts. It cannot fund another Past/Search tick.
    if f.fuel >= 1. && w.entity(id).is_some_and(|body| body.alive) {
        referee_trace(
            w,
            f.shooter,
            "10.2.source_runout",
            json!({"torpedo":id,"spent_fraction":f.fuel}),
        );
        retire(w, id);
    }
    Ok(())
}

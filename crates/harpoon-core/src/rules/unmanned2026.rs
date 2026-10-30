//! English2026 §2.5, PDF21/22: hardware modes and explicit control elements.
//! Scenario-assumption fittings are permitted; no crew/FSM flag implies a UAV.
mod launch;
mod inventory;
pub mod annex;
use super::detection2026::{Frequency, SensorReport};
use crate::{ContactId, EntityId, EventKind, Rng, Side, Vec2, Visibility, World};
pub use launch::{register_launched, validate_launch};
pub use inventory::{finalize_initial_stow, UuvLauncherFit};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/unmanned-2026.json"))
            .expect("verified unmanned source/policy")
    })
}
fn n(key: &str) -> i64 {
    tables()["source_values"][key]
        .as_i64()
        .expect("unmanned source constant")
}
fn authored(s: &str) -> bool {
    s == "scenario_assumption" || s.starts_with("scenario_assumption:")
}
fn fingerprint<T: Serialize>(value: &T) -> Result<String, String> {
    let data = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    Ok(format!(
        "fnv64:{:016x}",
        data.iter()
            .fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(*b))
                .wrapping_mul(0x100000001b3))
    ))
}
fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Hitl,
    Autonomous,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VehicleDomain {
    Air,
    Surface,
    Underwater,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlLink {
    LineOfSight,
    Satellite,
    AcousticModem,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LossBehavior {
    ReturnToHome { home: String },
    MoveAwayFromJammer,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub controllers: BTreeMap<String, ControllerFit>,
    pub vehicles: BTreeMap<String, VehicleFit>,
    /// Explicit source-authored launch configuration by exact class ID. This
    /// is never inferred from Aircraft/mission names or a tactical AI flag.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub launch_classes: BTreeMap<String, VehicleFit>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub launchers: BTreeMap<String, BTreeMap<String, UuvLauncherFit>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControllerFit {
    pub source: String,
    pub elements: Vec<String>,
    pub operational: bool,
    #[serde(default)]
    pub los_range_nm: Option<f64>,
    #[serde(default)]
    pub antenna_height_m: Option<f64>,
    #[serde(default)]
    pub satellite: bool,
    #[serde(default)]
    pub satellite_available: bool,
    #[serde(default)]
    pub acoustic_modem: bool,
    #[serde(default)]
    pub overt_recall_transmitter: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VehicleFit {
    pub source: String,
    /// Optional printed control-field qualification; operational choices and
    /// endurance work assumptions remain explicitly scenario-authored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annex_platform: Option<String>,
    pub domain: VehicleDomain,
    pub autonomous_capable: bool,
    pub initial_mode: Mode,
    #[serde(default)]
    pub controller: Option<String>,
    #[serde(default)]
    pub element: Option<String>,
    pub initial_link: ControlLink,
    #[serde(default)]
    pub los_range_nm: Option<f64>,
    #[serde(default)]
    pub antenna_height_m: Option<f64>,
    #[serde(default)]
    pub satellite: bool,
    #[serde(default)]
    pub satellite_available: bool,
    #[serde(default)]
    pub acoustic_modem: bool,
    #[serde(default)]
    pub periscope_depth_m: Option<f64>,
    #[serde(default)]
    pub endurance_s: Option<f64>,
    pub loss_behavior: LossBehavior,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    LaunchUuv { launcher: String },
    RecoverUuv { launcher: String, vehicle: EntityId },
    SwitchMode {
        mode: Mode,
    },
    TakeControl {
        controller: EntityId,
        element: String,
        link: ControlLink,
    },
    AssignAutonomous {
        controller: EntityId,
        element: String,
        link: ControlLink,
    },
    SetLink {
        link: ControlLink,
    },
    PlotMission {
        waypoints: Vec<Vec2>,
        #[serde(default)]
        speed_kts: Option<f64>,
    },
    RequestRadio {
        vehicle: EntityId,
        link: ControlLink,
    },
    ExchangeBasicData {
        #[serde(default)]
        contact: Option<ContactId>,
        #[serde(default)]
        sensor: Option<String>,
    },
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Configuration,
    config_fingerprint: String,
    vehicles: BTreeMap<EntityId, VehicleState>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    launched_fits: BTreeMap<EntityId, VehicleFit>,
    #[serde(default, skip_serializing_if = "inventory::State::is_empty")]
    inventory: inventory::State,
    interference: BTreeMap<EntityId, Interference>,
    /// Independent trusted EW/referee causes. The legacy aggregate above is
    /// retained for old saved states and for the single physical link gate.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    interference_causes: BTreeMap<EntityId, BTreeMap<String, Interference>>,
    disabled: BTreeSet<EntityId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    disabled_causes: BTreeMap<EntityId, BTreeSet<String>>,
    satellite_availability: BTreeMap<EntityId, bool>,
    onboard: BTreeMap<String, Onboard>,
    basic: Vec<BasicTransfer>,
    pending_projection: Vec<BasicProjection>,
    signals: Vec<AcousticCommandEmission>,
    /// Sender-known broadcast admissions, independent of hidden recipient
    /// survival, position, fuel and actual radio-establishment completion.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    recall_broadcasts: BTreeMap<EntityId, i64>,
    telemetry: BTreeMap<EntityId, OwnTelemetry>,
    /// Author roster plus explicitly requested assignments, independent of
    /// hidden automatic release/completion and loss of the vehicle entity.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    known_roster: BTreeMap<EntityId, BTreeSet<EntityId>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    concealed_losses: BTreeMap<EntityId, ConcealedLoss>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    reported_destroyed: BTreeSet<EntityId>,
    last_tick: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct VehicleState {
    name: String,
    side: Side,
    class: String,
    mode: Mode,
    controller: Option<EntityId>,
    element: Option<String>,
    link: ControlLink,
    payload_fingerprint: String,
    transition: Option<Transition>,
    mission: Option<Mission>,
    recall: Option<Recall>,
    mast_since: Option<i64>,
    loss: Option<LossState>,
    last_wander_epoch: Option<i64>,
    fuel_exhausted: bool,
    home_position: Option<Vec2>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Transition {
    ready_at: i64,
    mode: Mode,
    controller: Option<EntityId>,
    element: Option<String>,
    link: ControlLink,
    release: Vec<EntityId>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Mission {
    due: i64,
    waypoints: Vec<Vec2>,
    speed_kts: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Recall {
    requested_at: i64,
    ready_at: i64,
    link: ControlLink,
    dice: [u8; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
enum LossState {
    AutonomousRecovery,
    AwaitingJammerBearing,
    Wandering,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Interference {
    control: bool,
    navigation: bool,
    away_course: Option<f64>,
    source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConcealedLoss {
    side: Side,
    kind: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Onboard {
    report: SensorReport,
    category: Option<crate::types::PlatformKind>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct BasicTransfer {
    vehicle: EntityId,
    controller: EntityId,
    at: i64,
    position: Vec2,
    report: Option<Onboard>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct BasicProjection {
    report: SensorReport,
    category: Option<crate::types::PlatformKind>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcousticCommandEmission {
    pub emitter: EntityId,
    pub at: i64,
    pub until: i64,
    pub frequency: Frequency,
    pub base50_nm: f64,
}
/// Independently timestamped, genuinely received telemetry. A modem position
/// update does not imply a new course/speed/HP/ammunition/status report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnTelemetry {
    pub position: Vec2,
    pub position_at: i64,
    pub status_at: Option<i64>,
    pub unit: Option<crate::observe::UnitObs>,
}
fn own(w: &World, side: Side, id: EntityId) -> Result<&crate::world::Entity, String> {
    w.entity(id)
        .filter(|e| e.alive && e.side == side)
        .ok_or_else(|| "2.5 owned surviving unit required".into())
}
fn named(w: &World, name: &str) -> Result<EntityId, String> {
    let ids: Vec<_> = w
        .entities
        .values()
        .filter(|e| e.name == name)
        .map(|e| e.id)
        .collect();
    if ids.len() != 1 {
        Err("2.5 exact unambiguous author unit name required".into())
    } else {
        Ok(ids[0])
    }
}
fn config_for<'a>(w: &World, s: &'a State, id: EntityId) -> Result<&'a VehicleFit, String> {
    let e = w.entity(id).ok_or("2.5 vehicle absent")?;
    if let Some(fit) = s.launched_fits.get(&id) {
        annex::validate(&e.class, fit)?;
        return Ok(fit);
    }
    let fit = s.config
        .vehicles
        .get(&e.name)
        .ok_or("2.5 explicit unmanned hardware fit absent")?;
    annex::validate(&e.class, fit)?;
    Ok(fit)
}
fn controller_for<'a>(w: &World, s: &'a State, id: EntityId) -> Result<&'a ControllerFit, String> {
    let e = w.entity(id).ok_or("2.5 controller absent")?;
    s.config
        .controllers
        .get(&e.name)
        .filter(|c| authored(&c.source))
        .ok_or_else(|| "2.5 explicit control-element fit absent".into())
}
fn payload(w: &World, id: EntityId) -> Result<String, String> {
    let e = w.entity(id).ok_or("2.5 vehicle absent")?;
    let mut sensors: Vec<_> = e.sensors.iter().map(|i| i.sensor.clone()).collect();
    sensors.sort();
    let mut weapons: Vec<_> = e
        .weapons
        .iter()
        .map(|m| (m.weapon.clone(), m.mounts))
        .collect();
    weapons.sort();
    fingerprint(&(sensors, weapons))
}
fn check(w: &World, s: &State) -> Result<(), String> {
    if !active(w) {
        return Err("2.5 unmanned source rules require Full profile".into());
    }
    if fingerprint(&s.config)? != s.config_fingerprint
        || w.context
            .unmanned_2026
            .as_ref()
            .map(fingerprint)
            .transpose()?
            .as_ref()
            != Some(&s.config_fingerprint)
    {
        return Err("2.5 source configuration changed; reload/revision required".into());
    }
    Ok(())
}
fn validate_fit_numbers(f: &VehicleFit) -> Result<(), String> {
    if !authored(&f.source)
        || [
            f.los_range_nm,
            f.antenna_height_m,
            f.periscope_depth_m,
            f.endurance_s,
        ]
        .into_iter()
        .flatten()
        .any(|v| !v.is_finite() || v < 0.)
    {
        return Err("2.5 unmanned fit missing explicit source or invalid physical value".into());
    }
    if f.initial_mode == Mode::Autonomous && !f.autonomous_capable {
        return Err("2.5 autonomy is hardware capability, not FSM automation".into());
    }
    if f.periscope_depth_m.is_some_and(|d| d > 25.) {
        return Err("2.5 UUV periscope depth outside source PD envelope".into());
    }
    Ok(())
}
fn occupancy(
    s: &State,
    controller: EntityId,
    element: &str,
    exclude: EntityId,
) -> Vec<(EntityId, Mode)> {
    let mut occupants: Vec<_> = s.vehicles
        .iter()
        .filter(|(id, v)| {
            **id != exclude && !s.inventory.initial.contains_key(*id)
                && v.controller == Some(controller)
                && v.element.as_deref() == Some(element)
        })
        .map(|(id, v)| (*id, v.mode))
        .collect();
    occupants.extend(inventory::reservations(s, controller, element, exclude));
    occupants
}
fn capacity(
    s: &State,
    controller: EntityId,
    element: &str,
    id: EntityId,
    mode: Mode,
) -> Result<(), String> {
    let other = occupancy(s, controller, element, id);
    let limit = if mode == Mode::Hitl {
        n("hitl_capacity")
    } else {
        n("autonomous_capacity")
    };
    if other.iter().any(|(_, m)| *m == Mode::Hitl) || other.len() as i64 + 1 > limit {
        return Err("2.5 control element capacity exceeded".into());
    }
    Ok(())
}
pub fn initialize(w: &mut World, config: Configuration) -> Result<(), String> {
    if !active(w) {
        return Err("2.5 unmanned config is Full-only".into());
    }
    if (!config.vehicles.is_empty() || !config.launch_classes.is_empty() || !config.launchers.is_empty())
        && !authored(&config.source)
    {
        return Err("2.5 exact Annex fittings unresolved; explicit scenario_assumption configuration required".into());
    }
    let mut s = State {
        config_fingerprint: fingerprint(&config)?,
        config,
        last_tick: Some(w.tick),
        ..Default::default()
    };
    check(w, &s)?;
    for c in s.config.controllers.values() {
        let names: BTreeSet<_> = c.elements.iter().collect();
        if !authored(&c.source)
            || c.elements.is_empty()
            || names.len() != c.elements.len()
            || names.iter().any(|n| n.trim().is_empty())
            || [c.los_range_nm, c.antenna_height_m]
                .into_iter()
                .flatten()
                .any(|v| !v.is_finite() || v < 0.)
        {
            return Err("2.5 invalid explicit controller elements/fittings".into());
        }
    }
    launch::validate_catalog(w, &s)?;
    for (name, f) in &s.config.vehicles {
        validate_fit_numbers(f)?;
        let id = named(w, name)?;
        let e = w.entity(id).unwrap();
        annex::validate(&e.class, f)?;
        let expected = match f.domain {
            VehicleDomain::Air => crate::types::PlatformKind::Aircraft,
            VehicleDomain::Surface => crate::types::PlatformKind::Ship,
            VehicleDomain::Underwater => crate::types::PlatformKind::Sub,
        };
        if e.kind != expected {
            return Err("2.5 physical platform kind and explicit unmanned domain conflict".into());
        }
        let controller = f
            .controller
            .as_ref()
            .map(|name| named(w, name))
            .transpose()?;
        if let Some(c) = controller {
            own(w, e.side, c)?;
            let cf = controller_for(w, &s, c)?;
            if f.element
                .as_ref()
                .is_none_or(|element| !cf.elements.contains(element))
            {
                return Err("2.5 exact controller element required".into());
            }
            s.known_roster.entry(c).or_default().insert(id);
        }
        if f.initial_mode == Mode::Hitl && controller.is_none() {
            return Err("2.5 HITL requires an actual control element".into());
        }
        let home_position = match &f.loss_behavior {
            LossBehavior::ReturnToHome { home } => {
                let home_id = named(w, home)?;
                Some(own(w, e.side, home_id)?.pos)
            }
            LossBehavior::MoveAwayFromJammer => None,
        };
        s.vehicles.insert(
            id,
            VehicleState {
                name: e.name.clone(),
                side: e.side,
                class: e.class.clone(),
                mode: f.initial_mode,
                controller,
                element: f.element.clone(),
                link: f.initial_link,
                payload_fingerprint: payload(w, id)?,
                transition: None,
                mission: None,
                recall: None,
                mast_since: None,
                loss: None,
                last_wander_epoch: None,
                fuel_exhausted: false,
                home_position,
            },
        );
    }
    inventory::prepare_initial(w, &mut s)?;
    for (id, v) in &s.vehicles {
        if s.inventory.initial.contains_key(id) { continue; }
        if let (Some(c), Some(element)) = (v.controller, v.element.as_ref()) {
            capacity(&s, c, element, *id, v.mode)?;
        }
    }
    let fuel: Vec<_> = s
        .config
        .vehicles
        .iter()
        .filter_map(|(name, f)| f.endurance_s.map(|seconds| (name.clone(), seconds)))
        .collect();
    for (name, seconds) in fuel {
        let id = named(w, &name)?;
        w.entity_mut(id).unwrap().endurance_s = Some(seconds);
    }
    w.rules.unmanned = Some(s);
    finish_telemetry(w);
    Ok(())
}
fn radio_ready_with(w: &World, s: &State, id: EntityId, v: &VehicleState) -> bool {
    let Ok(f) = config_for(w, s, id) else {
        return false;
    };
    if f.domain != VehicleDomain::Underwater {
        return true;
    }
    let Some(e) = w.entity(id) else { return false };
    let _ = v; // A saved private timer never substitutes for physical hardware.
    f.periscope_depth_m.is_some_and(|pd| e.depth_m <= pd)
        && w.rules.detection.as_ref().is_some_and(|d| {
            d.masts.get(&id).is_some_and(|mast| {
                mast.count > 0
                    && d.config
                        .max_masts
                        .get(&e.class)
                        .is_some_and(|max| *max >= mast.count)
                    && mast.raised_since <= w.tick
                    && w.tick - mast.raised_since
                        >= n("radio_establishment_tactical_turns") * n("tactical_seconds")
            })
        })
}
fn link_with(
    w: &World,
    s: &State,
    id: EntityId,
    v: &VehicleState,
    link: ControlLink,
) -> Result<(), String> {
    let e = w
        .entity(id)
        .filter(|e| e.alive)
        .ok_or("2.5 vehicle absent")?;
    let c_id = v
        .controller
        .ok_or("2.5 vehicle has no assigned control element")?;
    let c = own(w, e.side, c_id)?;
    let cf = controller_for(w, s, c_id)?;
    let f = config_for(w, s, id)?;
    if !cf.operational
        || disabled(s, id)
        || disabled(s, c_id)
        || s.interference.get(&id).is_some_and(|j| j.control)
        || s.interference.get(&c_id).is_some_and(|j| j.control)
    {
        return Err("2.5 source control link unavailable".into());
    }
    match link {
        ControlLink::AcousticModem => {
            if f.domain != VehicleDomain::Underwater
                || !f.acoustic_modem
                || !cf.acoustic_modem
                || e.pos.range_to(c.pos) > n("acoustic_modem_range_nm") as f64
            {
                return Err("2.5 UUV acoustic modem outside explicit four-nm fit".into());
            }
        }
        ControlLink::Satellite => {
            if !f.satellite
                || !cf.satellite
                || !s
                    .satellite_availability
                    .get(&id)
                    .copied()
                    .unwrap_or(f.satellite_available)
                || !s
                    .satellite_availability
                    .get(&c_id)
                    .copied()
                    .unwrap_or(cf.satellite_available)
                || !radio_ready_with(w, s, id, v)
                || !controller_radio_exposed(w, c)
            {
                return Err("2.5 satellite fit/bandwidth/raised UUV mast unavailable".into());
            }
        }
        ControlLink::LineOfSight => {
            let (a, b, ah, bh) = (
                f.los_range_nm.ok_or("2.5 annex LOS range absent")?,
                cf.los_range_nm.ok_or("2.5 controller LOS range absent")?,
                f.antenna_height_m
                    .ok_or("2.5 vehicle antenna height absent")?,
                cf.antenna_height_m
                    .ok_or("2.5 controller antenna height absent")?,
            );
            if !radio_ready_with(w, s, id, v)
                || !controller_radio_exposed(w, c)
                || e.pos.range_to(c.pos)
                    > a.min(b).min(crate::geom::horizon_nm(
                        e.altitude_m.max(0.) + ah,
                        c.altitude_m.max(0.) + bh,
                    ))
            {
                return Err("2.5 source LOS/radar horizon or raised UUV mast gate failed".into());
            }
        }
    }
    Ok(())
}
fn disabled(s: &State, id: EntityId) -> bool {
    s.disabled.contains(&id)
        || s.disabled_causes
            .get(&id)
            .is_some_and(|causes| !causes.is_empty())
}
fn controller_radio_exposed(w: &World, e: &crate::world::Entity) -> bool {
    e.depth_m == 0.
        || (e.depth_m <= 25.
            && w.rules
                .detection
                .as_ref()
                .and_then(|d| d.masts.get(&e.id))
                .is_some_and(|m| m.count > 0))
}
fn actor_state(w: &World, side: Side, actor: EntityId) -> Result<(&State, &VehicleState), String> {
    if w.over || crate::exercise::locked(w, side) {
        return Err("2.5 command blocked after game end/exercise lock".into());
    }
    own(w, side, actor)?;
    let s = w
        .rules
        .unmanned
        .as_ref()
        .ok_or("2.5 unmanned runtime absent")?;
    check(w, s)?;
    let v = s
        .vehicles
        .get(&actor)
        .ok_or("2.5 actor is not explicitly fitted unmanned hardware")?;
    config_for(w, s, actor)?;
    if payload(w, actor)? != v.payload_fingerprint {
        return Err("2.5.2 launched sensor/weapon payload cannot change".into());
    }
    if v.fuel_exhausted {
        return Err("2.5 vehicle fuel exhausted".into());
    }
    Ok((s, v))
}
fn known_recall_fit<'a>(
    w: &World,
    s: &'a State,
    side: Side,
    actor: EntityId,
    vehicle: EntityId,
) -> Result<&'a VehicleFit, String> {
    let v = s
        .vehicles
        .get(&vehicle)
        .filter(|v| v.side == side)
        .ok_or("2.5 own source-declared UUV hardware unknown")?;
    if s.reported_destroyed.contains(&vehicle) {
        return Err("2.5 UUV destruction has already been reported".into());
    }
    let sender = own(w, side, actor)?;
    let f = s
        .launched_fits
        .get(&vehicle)
        .or_else(|| s.config.vehicles.get(&v.name))
        .ok_or("2.5 source-declared UUV fit absent")?;
    if !s
        .known_roster
        .get(&actor)
        .is_some_and(|ids| ids.contains(&vehicle))
        && f.controller.as_deref() != Some(sender.name.as_str())
    {
        return Err("2.5 no own declared or explicitly requested UUV assignment".into());
    }
    Ok(f)
}
pub fn validate_action(
    w: &World,
    side: Side,
    actor: EntityId,
    action: &Action,
) -> Result<(), String> {
    if matches!(action, Action::LaunchUuv { .. } | Action::RecoverUuv { .. }) {return inventory::validate(w, side, actor, action);}
    if let Action::RequestRadio { vehicle, link } = action {
        if w.over || crate::exercise::locked(w, side) {
            return Err("2.5 recall unavailable after game end/exercise lock".into());
        }
        let controller = own(w, side, actor)?;
        let s = w
            .rules
            .unmanned
            .as_ref()
            .ok_or("2.5 unmanned runtime absent")?;
        check(w, s)?;
        let f = known_recall_fit(w, s, side, actor, *vehicle)?;
        let cf = controller_for(w, s, actor)?;
        if f.domain != VehicleDomain::Underwater
            || !cf.operational
            || !cf.overt_recall_transmitter
            || disabled(s, actor)
            || s.recall_broadcasts.get(vehicle).is_some_and(|at| {
                at.div_euclid(n("tactical_seconds")) == w.tick.div_euclid(n("tactical_seconds"))
            })
        {
            return Err(
                "2.5 exact assigned UUV/overt acoustic transmitter or idle recall required".into(),
            );
        }
        let _ = controller;
        if f.periscope_depth_m.is_none() {
            return Err("2.5 explicit UUV PD source absent".into());
        }
        match link {
            ControlLink::AcousticModem => {
                return Err("2.5 recall requests LOS/satellite data-volume link".into())
            }
            ControlLink::Satellite if !f.satellite || !cf.satellite => {
                return Err("2.5 satellite hardware absent".into())
            }
            ControlLink::LineOfSight if f.los_range_nm.is_none() || cf.los_range_nm.is_none() => {
                return Err("2.5 explicit LOS hardware absent".into())
            }
            _ => {}
        }
        return Ok(());
    }
    let (s, v) = actor_state(w, side, actor)?;
    let f = config_for(w, s, actor)?;
    if v.transition.is_some() {
        return Err("2.5 one Tactical Turn control switch already pending".into());
    }
    match action {
        Action::SwitchMode { mode } => {
            // This is a received operator instruction, unlike the automatic
            // hardware fallback on loss of command in phase_open.
            link_with(w, s, actor, v, v.link)?;
            if *mode == Mode::Autonomous && !f.autonomous_capable {
                return Err("2.5 hardware has HITL only".into());
            }
            if *mode == Mode::Hitl {
                let c = v.controller.ok_or("2.5 controller absent")?;
                let element = v.element.as_deref().ok_or("2.5 element absent")?;
                if !inventory::reservations(s,c,element,actor).is_empty() {
                    return Err("2.5 control element reserved for physical UUV launch".into());
                }
                if s.vehicles.iter().any(|(id, other)| {
                    *id != actor
                        && other.transition.as_ref().is_some_and(|t| {
                            t.controller == Some(c) && t.element.as_deref() == Some(element)
                        })
                }) {
                    return Err("2.5 control element already reserved for a switch".into());
                }
            }
        }
        Action::TakeControl {
            controller,
            element,
            link,
        }
        | Action::AssignAutonomous {
            controller,
            element,
            link,
        } => {
            own(w, side, *controller)?;
            let cf = controller_for(w, s, *controller)?;
            if !cf.elements.contains(element) {
                return Err("2.5 exact control element absent".into());
            }
            if !inventory::reservations(s,*controller,element,actor).is_empty() {
                return Err("2.5 control element reserved for physical UUV launch".into());
            }
            if s.vehicles.iter().any(|(id, other)| {
                *id != actor
                    && other.transition.as_ref().is_some_and(|t| {
                        t.controller == Some(*controller) && t.element.as_deref() == Some(element)
                    })
            }) {
                return Err("2.5 control element already reserved for a switch".into());
            }
            let mut planned = v.clone();
            planned.controller = Some(*controller);
            planned.element = Some(element.clone());
            link_with(w, s, actor, &planned, *link)?;
            if matches!(action, Action::AssignAutonomous { .. }) {
                if !f.autonomous_capable {
                    return Err(
                        "2.5 autonomous controller assignment needs actual autonomous hardware"
                            .into(),
                    );
                }
                capacity(s, *controller, element, actor, Mode::Autonomous)?;
            }
            if occupancy(s, *controller, element, actor)
                .iter()
                .filter(|(_, m)| *m == Mode::Hitl)
                .count()
                > 1
            {
                return Err("2.5 conflicting control-element source state".into());
            }
        }
        Action::SetLink { link } => link_with(w, s, actor, v, *link)?,
        Action::PlotMission {
            waypoints,
            speed_kts,
        } => {
            link_with(w, s, actor, v, v.link)?;
            if v.link == ControlLink::AcousticModem {
                return Err("2.5 acoustic modem carries basic steering, not a bulk mission-program upload; establish LOS/satellite or use direct steering".into());
            }
            if waypoints
                .iter()
                .any(|p| !p.x.is_finite() || !p.y.is_finite())
                || speed_kts.is_some_and(|x| {
                    !x.is_finite() || x < 0. || x > w.entity(actor).unwrap().max_speed(&w.db)
                })
            {
                return Err("2.5 invalid mission geometry/sourced platform speed".into());
            }
        }
        Action::ExchangeBasicData { contact, sensor } => {
            if v.link != ControlLink::AcousticModem {
                return Err("2.5 basic-data action is the acoustic-modem limited route".into());
            }
            link_with(w, s, actor, v, v.link)?;
            if s.basic.iter().any(|b| {
                b.vehicle == actor
                    && b.at.div_euclid(n("tactical_seconds"))
                        == w.tick.div_euclid(n("tactical_seconds"))
            }) {
                return Err(
                    "2.5 acoustic modem already exchanged one contact this Tactical Turn".into(),
                );
            }
            select_onboard(w, s, side, actor, *contact, sensor.as_deref())?;
        }
        Action::RequestRadio { .. } | Action::LaunchUuv { .. } | Action::RecoverUuv { .. } => unreachable!(),
    }
    Ok(())
}
fn select_onboard(
    w: &World,
    s: &State,
    side: Side,
    actor: EntityId,
    contact: Option<ContactId>,
    sensor: Option<&str>,
) -> Result<Option<Onboard>, String> {
    let target = contact
        .map(|id| {
            w.sides[side.index()]
                .contacts
                .get(&id)
                .map(|c| c.about)
                .ok_or("2.5 selected own contact label absent")
        })
        .transpose()?;
    let report = s
        .onboard
        .values()
        .filter(|r| {
            r.report.observer == actor
                && target.is_none_or(|t| r.report.target == t)
                && sensor.is_none_or(|sensor| r.report.sensor == sensor)
        })
        .max_by_key(|r| (r.report.at, r.report.sensor.clone()))
        .cloned();
    if (contact.is_some() || sensor.is_some()) && report.is_none() {
        return Err("2.5 selected contact/sensor has no real onboard report".into());
    }
    Ok(report)
}
fn emit(w: &mut World, unit: EntityId, rule: &str, detail: Value, private: bool) {
    let side = w.entity(unit).map(|e| e.side);
    // Hardware fallback, fuel exhaustion and delayed completion are not a
    // received status report. A successful explicit modem exchange is the
    // narrow exception carrying the documented basic-data receipt.
    let private = private
        || (is_configured(w, unit)
            && !allow_side_projection(w, unit)
            && rule != "2.5.uuv_basic_data");
    w.emit(
        EventKind::RulesEvent {
            domain: "unmanned2026".into(),
            rule: rule.into(),
            side: if private { None } else { side },
            unit: Some(unit),
            contact: None,
            detail: detail.to_string(),
        },
        if private {
            Visibility::Referee
        } else {
            side.map_or(Visibility::Referee, Visibility::Side)
        },
    );
}
pub fn apply(w: &mut World, side: Side, actor: EntityId, action: &Action) -> Result<(), String> {
    validate_action(w, side, actor, action)?;
    if matches!(action, Action::LaunchUuv { .. } | Action::RecoverUuv { .. }) {return inventory::apply(w, side, actor, action);}
    match action {
        Action::LaunchUuv { .. } | Action::RecoverUuv { .. } => unreachable!(),
        Action::RequestRadio { vehicle, link } => {
            // Broadcast admission is known to its sender. Actual reception is
            // private: a dead, departed, reassigned or otherwise unavailable
            // UUV cannot turn the public return value into a truth oracle.
            let sender_position = w.entity(actor).unwrap().pos;
            let s = w.rules.unmanned.as_mut().unwrap();
            s.recall_broadcasts.insert(*vehicle, w.tick);
            s.signals.push(AcousticCommandEmission {
                emitter: actor,
                at: w.tick,
                until: w.tick + n("tactical_seconds"),
                frequency: Frequency::Lmf,
                base50_nm: n("overt_command_lmf_base50_nm") as f64,
            });
            emit(
                w,
                actor,
                "2.5.uuv_recall",
                json!({"vehicle":vehicle,"request":"establish_radio","link":link,"broadcast_only":true,"reception_not_confirmed":true}),
                false,
            );
            let received = {
                let s = w.rules.unmanned.as_ref().unwrap();
                let v = &s.vehicles[vehicle];
                w.entity(*vehicle).is_some_and(|e| {
                    e.alive && e.pos.range_to(sender_position) <= n("overt_command_range_nm") as f64
                }) && v.controller == Some(actor)
                    && v.transition.is_none()
                    && !v.fuel_exhausted
                    && v.recall.is_none()
                    && payload(w, *vehicle).ok().as_ref() == Some(&v.payload_fingerprint)
            };
            if !received {
                emit(
                    w,
                    actor,
                    "2.5.uuv_recall_no_new_reception",
                    json!({"vehicle":vehicle,"source":"private physical reception/old pending task; no sender acknowledgement"}),
                    true,
                );
                return Ok(());
            }
            let mut rng = Rng::new(w.seed).derive(Rng::hash_tag(
                w.tick,
                vehicle.0,
                "2.5.uuv.overt_radio_request",
            ));
            let dice = [
                1 + (rng.next_f64() * 6.) as u8,
                1 + (rng.next_f64() * 6.) as u8,
            ];
            let ready_at =
                w.tick + (i64::from(dice[0]) + i64::from(dice[1])) * n("tactical_seconds");
            let pd = {
                let s = w.rules.unmanned.as_ref().unwrap();
                config_for(w, s, *vehicle)?.periscope_depth_m.unwrap()
            };
            let s = w.rules.unmanned.as_mut().unwrap();
            s.vehicles.get_mut(vehicle).unwrap().recall = Some(Recall {
                requested_at: w.tick,
                ready_at,
                link: *link,
                dice,
            });
            w.entity_mut(*vehicle).unwrap().ordered_depth_m = pd;
            emit(
                w,
                *vehicle,
                "2.5.uuv_recall_roll",
                json!({"dice":dice,"ready_at":ready_at,"source":"English2026 PDF22/2-6;2D6 TacticalTurns;LMF5nm;command reach20nm"}),
                true,
            );
        }
        Action::ExchangeBasicData { contact, sensor } => {
            let (s, v) = actor_state(w, side, actor)?;
            let report = select_onboard(w, s, side, actor, *contact, sensor.as_deref())?;
            let controller = v.controller.unwrap();
            let position = w.entity(actor).unwrap().pos;
            let s = w.rules.unmanned.as_mut().unwrap();
            if let Some(r) = &report {
                s.pending_projection.push(BasicProjection {
                    report: r.report.clone(),
                    category: r.category,
                });
            }
            s.basic.push(BasicTransfer {
                vehicle: actor,
                controller,
                at: w.tick,
                position,
                report,
            });
            let t = s.telemetry.entry(actor).or_insert(OwnTelemetry {
                position,
                position_at: w.tick,
                status_at: None,
                unit: None,
            });
            t.position = position;
            t.position_at = w.tick;
            emit(
                w,
                actor,
                "2.5.uuv_basic_data",
                json!({"controller":controller,"single_contact_only":true,"bulk_track_history":false}),
                false,
            );
        }
        Action::PlotMission {
            waypoints,
            speed_kts,
        } => {
            let mode = w.rules.unmanned.as_ref().unwrap().vehicles[&actor].mode;
            let due = if mode == Mode::Hitl {
                w.tick
            } else {
                ((w.tick + n("intermediate_seconds") - 1) / n("intermediate_seconds"))
                    * n("intermediate_seconds")
            };
            w.rules
                .unmanned
                .as_mut()
                .unwrap()
                .vehicles
                .get_mut(&actor)
                .unwrap()
                .mission = Some(Mission {
                due,
                waypoints: waypoints.clone(),
                speed_kts: *speed_kts,
            });
            emit(
                w,
                actor,
                "2.5.mission_plotted",
                json!({"effective_at":due,"mode":mode}),
                false,
            );
        }
        Action::SetLink { link } => {
            w.rules
                .unmanned
                .as_mut()
                .unwrap()
                .vehicles
                .get_mut(&actor)
                .unwrap()
                .link = *link;
            emit(
                w,
                actor,
                "2.5.single_control_link",
                json!({"link":link}),
                false,
            );
        }
        Action::SwitchMode { mode } => {
            if *mode == Mode::Autonomous {
                w.rules
                    .unmanned
                    .as_mut()
                    .unwrap()
                    .vehicles
                    .get_mut(&actor)
                    .unwrap()
                    .mode = *mode;
            } else {
                let s = w.rules.unmanned.as_mut().unwrap();
                let old = s.vehicles[&actor].clone();
                let release =
                    if let (Some(c), Some(element)) = (old.controller, old.element.as_deref()) {
                        occupancy(s, c, element, actor)
                            .into_iter()
                            .map(|p| p.0)
                            .collect()
                    } else {
                        Vec::new()
                    };
                let v = s.vehicles.get_mut(&actor).unwrap();
                v.transition = Some(Transition {
                    ready_at: w.tick + n("switch_tactical_turns") * n("tactical_seconds"),
                    mode: *mode,
                    controller: v.controller,
                    element: v.element.clone(),
                    link: v.link,
                    release,
                });
            }
            emit(
                w,
                actor,
                "2.5.mode_switch",
                json!({"requested_mode":mode,"hitl_ready_after_tactical_turn":*mode==Mode::Hitl}),
                false,
            );
        }
        Action::TakeControl {
            controller,
            element,
            link,
        } => {
            let s = w.rules.unmanned.as_mut().unwrap();
            s.known_roster.entry(*controller).or_default().insert(actor);
            let release = occupancy(s, *controller, element, actor)
                .into_iter()
                .map(|p| p.0)
                .collect();
            s.vehicles.get_mut(&actor).unwrap().transition = Some(Transition {
                ready_at: w.tick + n("switch_tactical_turns") * n("tactical_seconds"),
                mode: Mode::Hitl,
                controller: Some(*controller),
                element: Some(element.clone()),
                link: *link,
                release,
            });
            emit(
                w,
                actor,
                "2.5.control_switch",
                json!({"controller":controller,"element":element,"ready_at":w.tick+n("tactical_seconds"),"link":link}),
                false,
            );
        }
        Action::AssignAutonomous {
            controller,
            element,
            link,
        } => {
            let s = w.rules.unmanned.as_mut().unwrap();
            s.known_roster.entry(*controller).or_default().insert(actor);
            s.vehicles.get_mut(&actor).unwrap().transition = Some(Transition {
                ready_at: w.tick + n("tactical_seconds"),
                mode: Mode::Autonomous,
                controller: Some(*controller),
                element: Some(element.clone()),
                link: *link,
                release: Vec::new(),
            });
            emit(
                w,
                actor,
                "2.5.autonomous_assignment",
                json!({"controller":controller,"element":element,"ready_at":w.tick+n("tactical_seconds"),"link":link}),
                false,
            );
        }
    }
    Ok(())
}
pub fn can_control(w: &World, side: Side, id: EntityId) -> Result<(), String> {
    let Some(s) = w.rules.unmanned.as_ref() else {
        return Ok(());
    };
    if !s.vehicles.contains_key(&id) {
        return Ok(());
    }
    let (s, v) = actor_state(w, side, id)?;
    if v.mode != Mode::Hitl
        || v.transition.is_some()
        || v.loss.is_some()
        || s.vehicles.values().any(|other| {
            other
                .transition
                .as_ref()
                .is_some_and(|t| t.release.contains(&id))
        })
    {
        return Err("2.5 direct orders require established HITL control; use mission plotting or one-turn takeover".into());
    }
    link_with(w, s, id, v, v.link)
}
pub fn is_configured(w: &World, id: EntityId) -> bool {
    w.rules
        .unmanned
        .as_ref()
        .is_some_and(|s| s.vehicles.contains_key(&id))
}
pub fn basic_control_only(w: &World, id: EntityId) -> bool {
    w.rules
        .unmanned
        .as_ref()
        .and_then(|s| s.vehicles.get(&id))
        .is_some_and(|v| v.link == ControlLink::AcousticModem)
}
pub fn can_attack(w: &World, id: EntityId) -> bool {
    let Some(s) = w.rules.unmanned.as_ref() else {
        return true;
    };
    if !s.vehicles.contains_key(&id) {
        return true;
    }
    !basic_control_only(w, id)
        && w.entity(id)
            .is_some_and(|e| can_control(w, e.side, id).is_ok())
}
pub fn can_reaction_fire(w: &World, id: EntityId) -> bool {
    !w.rules
        .unmanned
        .as_ref()
        .is_some_and(|s| s.vehicles.contains_key(&id))
}
pub fn detection_allowed(w: &World, id: EntityId) -> bool {
    let Some(s) = w.rules.unmanned.as_ref() else {
        return true;
    };
    let Some(v) = s.vehicles.get(&id) else {
        return true;
    };
    if check(w, s).is_err() || v.fuel_exhausted || matches!(v.loss, Some(LossState::Wandering)) {
        return false;
    }
    v.mode == Mode::Hitl || w.tick.rem_euclid(n("intermediate_seconds")) == 0
}
pub fn radio_ready(w: &World, id: EntityId) -> bool {
    let Some(s) = w.rules.unmanned.as_ref() else {
        return true;
    };
    let Some(v) = s.vehicles.get(&id) else {
        return true;
    };
    radio_ready_with(w, s, id, v)
}
pub fn allow_side_projection(w: &World, id: EntityId) -> bool {
    let Some(s) = w.rules.unmanned.as_ref() else {
        return true;
    };
    let Some(v) = s.vehicles.get(&id) else {
        return true;
    };
    check(w, s).is_ok()
        && v.link != ControlLink::AcousticModem
        && link_with(w, s, id, v, v.link).is_ok()
}
/// Damage/retirement producers sample this BEFORE changing alive/removing the
/// entity. An unseen casualty is not disclosed by a global own-loss counter or
/// event merely because the referee has committed the physical casualty.
pub fn loss_disclosure_allowed(w: &World, id: EntityId) -> bool {
    !is_configured(w, id)
        || w.rules
            .unmanned
            .as_ref()
            .is_some_and(|s| s.reported_destroyed.contains(&id))
        || allow_side_projection(w, id)
}
/// Trusted destruction producer, before entity removal. The simulation keeps
/// its real losses; this ledger records only which physical losses have not
/// actually been reported through the owner's control link.
pub fn record_destroyed(w: &mut World, id: EntityId, owner_notified: bool) {
    if !active(w) || !is_configured(w, id) {
        return;
    }
    let loss = w.entity(id).and_then(|e| {
        World::kill_kind(e.kind).map(|kind| ConcealedLoss {
            side: e.side,
            kind: kind.into(),
        })
    });
    let Some(s) = w.rules.unmanned.as_mut() else {
        return;
    };
    if owner_notified {
        s.concealed_losses.remove(&id);
        s.reported_destroyed.insert(id);
    } else if let Some(loss) = loss {
        s.concealed_losses.entry(id).or_insert(loss);
    }
}
pub fn concealed_losses(w: &World, side: Side) -> (u32, BTreeMap<String, u32>) {
    let mut kinds = BTreeMap::new();
    let mut total = 0;
    if let Some(s) = w.rules.unmanned.as_ref().filter(|_| active(w)) {
        for loss in s.concealed_losses.values().filter(|loss| loss.side == side) {
            total += 1;
            *kinds.entry(loss.kind.clone()).or_default() += 1;
        }
    }
    (total, kinds)
}
pub fn record_onboard_report(
    w: &mut World,
    report: &SensorReport,
    category: Option<crate::types::PlatformKind>,
) {
    if !active(w) || report.at != w.tick {
        return;
    }
    if let Some(s) = w.rules.unmanned.as_mut() {
        if s.vehicles.contains_key(&report.observer) {
            s.onboard.insert(
                format!(
                    "{}:{}:{}",
                    report.observer.0, report.sensor, report.target.0
                ),
                Onboard {
                    report: report.clone(),
                    category,
                },
            );
        }
    }
}
pub fn take_basic_data_reports(
    w: &mut World,
) -> Vec<(SensorReport, Option<crate::types::PlatformKind>)> {
    w.rules
        .unmanned
        .as_mut()
        .map(|s| {
            std::mem::take(&mut s.pending_projection)
                .into_iter()
                .map(|r| (r.report, r.category))
                .collect()
        })
        .unwrap_or_default()
}
pub fn acoustic_command_sources(w: &World) -> Vec<AcousticCommandEmission> {
    w.rules
        .unmanned
        .as_ref()
        .map(|s| {
            s.signals
                .iter()
                .filter(|e| e.at <= w.tick && e.until > w.tick)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}
pub fn telemetry(w: &World, id: EntityId) -> Option<OwnTelemetry> {
    let s = w.rules.unmanned.as_ref()?;
    if check(w, s).is_err() || s.reported_destroyed.contains(&id) {
        return None;
    }
    s.telemetry.get(&id).cloned()
}
/// Includes held historical telemetry when an unseen vehicle no longer exists
/// in World.entities. The observation producer must not turn that removal into
/// an immediate own-side position/BDA update.
pub fn known_units(w: &World, side: Side) -> Vec<EntityId> {
    w.rules
        .unmanned
        .as_ref()
        .map(|s| {
            s.telemetry
                .iter()
                .filter(|(id, t)| {
                    !s.reported_destroyed.contains(*id)
                        && t.unit.as_ref().is_some_and(|e| e.side == side)
                })
                .map(|(id, _)| *id)
                .collect()
        })
        .unwrap_or_default()
}
pub fn finish_telemetry(w: &mut World) {
    if !active(w) {
        return;
    }
    let Some(s) = w.rules.unmanned.as_ref() else {
        return;
    };
    if check(w, s).is_err() {
        return;
    }
    let updates: Vec<_> = s
        .vehicles
        .iter()
        .filter_map(|(id, v)| {
            if v.link == ControlLink::AcousticModem || link_with(w, s, *id, v, v.link).is_err() {
                return None;
            }
            let unit = crate::observe::own_unit_snapshot(w, *id)?;
            Some((
                *id,
                OwnTelemetry {
                    position: unit.pos,
                    position_at: w.tick,
                    status_at: Some(w.tick),
                    unit: Some(unit),
                },
            ))
        })
        .collect();
    let s = w.rules.unmanned.as_mut().unwrap();
    for (id, t) in updates {
        s.telemetry.insert(id, t);
    }
}
/// Referee/EW/critical-hit producer only, never a player command inventing jams.
pub fn set_interference(
    w: &mut World,
    unit: EntityId,
    control: bool,
    navigation: bool,
    away_course: Option<f64>,
    source: &str,
) -> Result<(), String> {
    if source.trim().is_empty() || away_course.is_some_and(|v| !v.is_finite()) {
        return Err("2.5 interference source/bearing required".into());
    }
    let s = w.rules.unmanned.as_mut().ok_or("2.5 runtime absent")?;
    if !s.interference_causes.contains_key(&unit) {
        if let Some(legacy) = s
            .interference
            .get(&unit)
            .filter(|j| j.control || j.navigation)
            .cloned()
        {
            s.interference_causes
                .entry(unit)
                .or_default()
                .insert(legacy.source.clone(), legacy);
        }
    }
    let causes = s.interference_causes.entry(unit).or_default();
    if control || navigation {
        causes.insert(
            source.into(),
            Interference {
                control,
                navigation,
                away_course: away_course.map(crate::geom::normalize_deg),
                source: source.into(),
            },
        );
    } else {
        causes.remove(source);
    }
    if causes.is_empty() {
        s.interference_causes.remove(&unit);
        s.interference.remove(&unit);
    } else {
        // An unambiguous supplied bearing may be used; multiple different
        // bearings require a declared choice, not map order or enemy truth.
        let bearings: BTreeSet<_> = causes
            .values()
            .filter_map(|j| j.away_course.map(f64::to_bits))
            .collect();
        let combined = Interference {
            control: causes.values().any(|j| j.control),
            navigation: causes.values().any(|j| j.navigation),
            away_course: (bearings.len() == 1).then(|| f64::from_bits(*bearings.first().unwrap())),
            source: causes.keys().cloned().collect::<Vec<_>>().join("; "),
        };
        s.interference.insert(unit, combined);
    }
    Ok(())
}
pub fn set_operational(
    w: &mut World,
    unit: EntityId,
    operational: bool,
    source: &str,
) -> Result<(), String> {
    if source.trim().is_empty() {
        return Err("2.5 source cause required".into());
    }
    let s = w.rules.unmanned.as_mut().ok_or("2.5 runtime absent")?;
    if operational {
        if let Some(causes) = s.disabled_causes.get_mut(&unit) {
            causes.remove(source);
            if causes.is_empty() {
                s.disabled_causes.remove(&unit);
            }
        }
    } else {
        s.disabled_causes
            .entry(unit)
            .or_default()
            .insert(source.into());
    }
    Ok(())
}
/// Scenario/referee national-bandwidth producer; the source supplies no generic
/// satellite capacity or random availability percentage to invent here.
pub fn set_satellite_available(
    w: &mut World,
    units: &[EntityId],
    available: bool,
    source: &str,
) -> Result<(), String> {
    if source.trim().is_empty() {
        return Err("2.5 satellite national-asset source required".into());
    }
    let s = w.rules.unmanned.as_mut().ok_or("2.5 runtime absent")?;
    for unit in units {
        s.satellite_availability.insert(*unit, available);
    }
    Ok(())
}
pub fn phase_open(w: &mut World) {
    if !active(w) {
        return;
    }
    inventory::step(w);
    let Some(mut s) = w.rules.unmanned.take() else {
        return;
    };
    if check(w, &s).is_err() {
        w.rules.unmanned = Some(s);
        return;
    }
    let dt = s.last_tick.map_or(0, |at| (w.tick - at).max(0));
    s.last_tick = Some(w.tick);
    let ids: Vec<_> = s.vehicles.keys().copied().collect();
    let mut events = Vec::new();
    for id in ids {
        let Some(e) = w.entity(id).filter(|e| e.alive).cloned() else {
            continue;
        };
        let Ok(f) = config_for(w, &s, id).cloned() else {
            continue;
        };
        let mut v = s.vehicles[&id].clone();
        if f.domain == VehicleDomain::Underwater {
            if f.periscope_depth_m.is_some_and(|pd| e.depth_m <= pd) {
                if v.recall.is_some()
                    && !w
                        .rules
                        .detection
                        .as_ref()
                        .and_then(|d| d.masts.get(&id))
                        .is_some_and(|m| m.count > 0)
                {
                    // A real source-verified mast fit is mandatory. If it is
                    // unknown this recall remains pending, without free radio.
                    let _ = super::detection2026::apply(
                        w,
                        id,
                        &super::detection2026::DetectionAction::Masts { count: 1 },
                    );
                }
                v.mast_since = w
                    .rules
                    .detection
                    .as_ref()
                    .and_then(|d| d.masts.get(&id))
                    .filter(|m| m.count > 0)
                    .map(|m| m.raised_since);
            } else {
                if w.rules
                    .detection
                    .as_ref()
                    .and_then(|d| d.masts.get(&id))
                    .is_some_and(|m| m.count > 0)
                {
                    let _ = super::detection2026::apply(
                        w,
                        id,
                        &super::detection2026::DetectionAction::Masts { count: 0 },
                    );
                }
                v.mast_since = None;
            }
        }
        if f.domain != VehicleDomain::Air {
            if let Some(fuel) = w.entity_mut(id).and_then(|e| e.endurance_s.as_mut()) {
                *fuel = (*fuel - dt as f64).max(0.);
                if *fuel == 0. {
                    v.fuel_exhausted = true;
                }
            }
        }
        if v.fuel_exhausted {
            if let Some(body) = w.entity_mut(id) {
                body.ordered_speed_kts = 0.;
                body.fire_on = false;
            }
            s.vehicles.insert(id, v);
            continue;
        }
        if let Some(recall) = v.recall.clone() {
            if w.tick >= recall.ready_at && radio_ready_with(w, &s, id, &v) {
                if link_with(w, &s, id, &v, recall.link).is_ok() {
                    v.link = recall.link;
                    v.recall = None;
                    events.push((
                        id,
                        "2.5.uuv_radio_established",
                        json!({"link":v.link,"requested_at":recall.requested_at}),
                        false,
                    ));
                }
            }
        }
        if let Some(t) = v.transition.clone().filter(|t| t.ready_at <= w.tick) {
            let mut planned = v.clone();
            planned.mode = t.mode;
            planned.controller = t.controller;
            planned.element = t.element.clone();
            planned.link = t.link;
            planned.loss = None;
            if link_with(w, &s, id, &planned, t.link).is_ok() {
                for release in &t.release {
                    if s.vehicles.contains_key(release) {
                        let old_auto =
                            config_for(w, &s, *release).is_ok_and(|f| f.autonomous_capable);
                        let old = s.vehicles.get_mut(release).unwrap();
                        old.controller = None;
                        old.element = None;
                        old.loss = if old_auto {
                            old.mode = Mode::Autonomous;
                            None
                        } else {
                            Some(LossState::Wandering)
                        };
                    }
                }
                planned.transition = None;
                v = planned;
                events.push((id,"2.5.control_established",json!({"mode":v.mode,"controller":v.controller,"element":v.element,"link":v.link}),false));
            } else {
                v.transition = None;
                events.push((
                    id,
                    "2.5.control_switch_failed",
                    json!({"reason":"own control link no longer available"}),
                    false,
                ));
            }
        }
        let link = link_with(w, &s, id, &v, v.link);
        let jam = s.interference.get(&id).cloned().unwrap_or_default();
        if link.is_err() && jam.navigation && !matches!(v.loss, Some(LossState::Wandering)) {
            v.loss = Some(LossState::Wandering);
            events.push((
                id,
                "2.5.control_and_navigation_lost",
                json!({"source":jam.source}),
                false,
            ));
        }
        if v.mode == Mode::Hitl && link.is_err() && v.loss.is_none() {
            let nav_jammed = jam.navigation;
            if !f.autonomous_capable || nav_jammed {
                v.loss = Some(LossState::Wandering);
            } else {
                v.mode = Mode::Autonomous;
                v.loss = Some(LossState::AutonomousRecovery);
                match &f.loss_behavior {
                    LossBehavior::ReturnToHome { home } => {
                        let position = named(w, home)
                            .ok()
                            .and_then(|home_id| w.entity(home_id))
                            .filter(|h| h.side == e.side)
                            .map(|h| h.pos)
                            .or(v.home_position);
                        if let Some(position) = position {
                            let body = w.entity_mut(id).unwrap();
                            body.waypoints = vec![position].into_iter().collect();
                            body.ordered_course_deg = body.pos.bearing_to(position);
                        }
                    }
                    LossBehavior::MoveAwayFromJammer => {
                        if let Some(course) = jam.away_course {
                            w.entity_mut(id).unwrap().ordered_course_deg =
                                crate::geom::normalize_deg(course);
                        } else {
                            v.loss = Some(LossState::AwaitingJammerBearing);
                        }
                    }
                }
            }
            events.push((id,"2.5.loss_of_command",json!({"mode":v.mode,"state":v.loss,"source":"English2026 2.5.1; no jammer truth-position lookup"}),false));
        }
        if matches!(v.loss, Some(LossState::AwaitingJammerBearing)) {
            if let Some(course) = jam.away_course {
                w.entity_mut(id).unwrap().ordered_course_deg = crate::geom::normalize_deg(course);
                v.loss = Some(LossState::AutonomousRecovery);
            }
        }
        if matches!(v.loss, Some(LossState::Wandering)) {
            let epoch = w.tick.div_euclid(n("intermediate_seconds"));
            if v.last_wander_epoch != Some(epoch) {
                let mut rng =
                    Rng::new(w.seed).derive(Rng::hash_tag(epoch, id.0, "2.5.rts_aimless_course"));
                let draw = rng.next_f64();
                let course = draw * 360.;
                w.entity_mut(id).unwrap().ordered_course_deg = course;
                v.last_wander_epoch = Some(epoch);
                events.push((id,"2.5.wander_policy",json!({"draw":draw,"course":course,"policy":"RTS: independent seeded course per Intermediate window; no invented combat chance; actual fuel unchanged"}),true));
            }
        } else if v.loss.is_some() && link.is_ok() {
            v.loss = None;
        }
        if let Some(m) = v.mission.clone().filter(|m| m.due <= w.tick) {
            if v.loss.is_none() {
                let body = w.entity_mut(id).unwrap();
                body.waypoints = m.waypoints.into_iter().collect();
                if let Some(first) = body.waypoints.front() {
                    body.ordered_course_deg = body.pos.bearing_to(*first);
                }
                if let Some(speed) = m.speed_kts {
                    body.ordered_speed_kts = speed;
                }
                v.mission = None;
                events.push((
                    id,
                    "2.5.mission_effective",
                    json!({"mode":v.mode,"due":m.due}),
                    false,
                ));
            }
        }
        s.vehicles.insert(id, v);
    }
    s.signals.retain(|e| e.until > w.tick);
    w.rules.unmanned = Some(s);
    for (unit, rule, detail, private) in events {
        emit(w, unit, rule, detail, private);
    }
    finish_telemetry(w);
}
pub fn own_view(w: &World, id: EntityId) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let s = w.rules.unmanned.as_ref()?;
    if check(w, s).is_err() || s.reported_destroyed.contains(&id) {
        return None;
    }
    if s.vehicles.contains_key(&id) && !allow_side_projection(w, id) {
        return s
            .telemetry
            .get(&id)
            .and_then(|t| t.unit.as_ref())
            .and_then(|unit| unit.unmanned.clone());
    }
    let name = w
        .entity(id)
        .filter(|e| e.alive)
        .map(|e| e.name.clone())
        .or_else(|| {
            s.telemetry
                .get(&id)
                .and_then(|t| t.unit.as_ref())
                .map(|u| u.name.clone())
        })
        .or_else(|| s.vehicles.get(&id).map(|v| v.name.clone()))?;
    let v = s.vehicles.get(&id);
    let received:Vec<_>=s.basic.iter().filter(|b|b.controller==id||b.vehicle==id).map(|b|json!({"vehicle":b.vehicle,"controller":b.controller,"at":b.at,"position":b.position,"single_contact":b.report.as_ref().map(onboard_public),"bulk_track_history":false})).collect();
    // A controller knows the authored assignment roster even before telemetry.
    // Automatic loss-of-link mode changes and completed hidden mast work are
    // not telemetry: do not expose live mode/link/vehicle status in that roster.
    let controlled: Vec<_> = s
        .vehicles
        .iter()
        .filter(|(vehicle, v)| {
            s.known_roster
                .get(&id)
                .is_some_and(|known| known.contains(*vehicle))
                || s.config
                    .vehicles
                    .get(&v.name)
                    .is_some_and(|f| f.controller.as_deref() == Some(name.as_str()))
        })
        .map(|(vehicle, v)| {
            json!({"unit":vehicle,"name":v.name,"class":v.class,
            "position_not_reported":!s.telemetry.get(vehicle).is_some_and(|t|t.unit.is_some()),
            "hardware_author_record":true})
        })
        .collect();
    if v.is_none() && received.is_empty() && !s.config.controllers.contains_key(&name) && !s.config.launchers.contains_key(&name) {
        return None;
    }
    let mut view=json!({"scope":"explicit_unmanned_hardware","mode":v.map(|v|v.mode),"controller":v.and_then(|v|v.controller),"element":v.and_then(|v|v.element.clone()),"link":v.map(|v|v.link),"switch_ready_at":v.and_then(|v|v.transition.as_ref().map(|t|t.ready_at)),"radio_pending":v.is_some_and(|v|v.recall.is_some()),"loss_of_command":v.is_some_and(|v|v.loss.is_some()),"fuel_exhausted":v.is_some_and(|v|v.fuel_exhausted),"reaction_fire_allowed":false,"basic_data":received,"controlled":controlled,"onboard_unshared_contacts_exposed":false,"source":"English2026 2.5 PDF21/22; explicit scenario fittings"});
    let launchers=inventory::own_view(w,s,id);
    if launchers.as_object().is_some_and(|o|!o.is_empty()) {view["launchers"]=launchers;}
    Some(view)
}
fn onboard_public(r: &Onboard) -> Value {
    let mut view = json!({"sensor":r.report.sensor,"observed_at":r.report.at,"category":r.category,"observer_position":r.report.observer_position,"bearing_deg":r.report.bearing_deg,"measured_position":r.report.measured_position});
    if !r.report.bearing_alternatives.is_empty() {
        view["bearing_alternatives"] = json!(r.report.bearing_alternatives);
    }
    view
}

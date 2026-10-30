//! 5.4.4/5.4.6.3 finite physical sonobuoy -> qualified radio monitor.
//! Float reports retain their real observer/time; laying never grants FCS.
use super::{Frequency, Receiver, SensorReport, SonarMount};
#[path = "sonobuoys/echo_ranging.rs"]
pub mod echo_ranging;
use crate::db::SensorKind;
use crate::rules::firecontrol2026::{AttackSolution, Quality};
use crate::types::PlatformKind;
use crate::world::{Contact, Entity, SensorInstance};
use crate::{ContactId, EntityId, EventKind, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        let mut data: Value = serde_json::from_str(include_str!("../../../../../data/rules/sonobuoys-2026.json")).expect("verified buoy source/policy");
        data["echo_ranging"] = serde_json::from_str(include_str!("../../../../../data/rules/echo-ranging-2026.json")).expect("verified echo source");
        data
    })
}
pub fn fingerprint() -> String {
    let bytes = serde_json::to_vec(tables()).unwrap();
    format!(
        "fnv64:{:016x}",
        bytes
            .iter()
            .fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64)
                .wrapping_mul(0x100000001b3))
    )
}
fn turn_seconds() -> i64 {
    tables()["deployment"]["tactical_turn_seconds"]
        .as_i64()
        .unwrap()
}
fn epoch(t: i64) -> i64 {
    t.div_euclid(turn_seconds())
}
fn sourced(s: &str) -> bool {
    !s.trim().is_empty()
}
fn finite_positive(x: f64) -> bool {
    x.is_finite() && x > 0.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub source: String,
    pub platform_class: String,
    pub sensor: String,
    pub active: bool,
    pub directional: bool,
    pub allowed_depths_m: Vec<f64>,
    pub lifetime_s: i64,
    pub mass_kg: f64,
    pub sea_state6_capable: bool,
    pub radio_antenna_height_m: f64,
    pub radio_channels: Vec<u16>,
    pub protocol: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarrierFit {
    pub source: String,
    pub loads: BTreeMap<String, u16>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitorFit {
    pub source: String,
    pub search_channels: u16,
    pub passive_localization_channels: u16,
    pub active_channels: u16,
    pub processing_per_tactical_turn: u16,
    pub protocol: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub models: BTreeMap<String, Model>,
    pub carriers: BTreeMap<String, CarrierFit>,
    pub monitors: BTreeMap<String, MonitorFit>,
    #[serde(default, skip_serializing_if="BTreeMap::is_empty")]
    pub echo_fits: BTreeMap<String, echo_ranging::Fit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Carrier {
    pub class: String,
    pub side: Side,
    pub carried: BTreeMap<String, u16>,
    pub deployed: Vec<EntityId>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Monitor {
    pub class: String,
    pub side: Side,
    pub fit: MonitorFit,
    pub selected: BTreeSet<EntityId>,
    pub budget_epoch: i64,
    pub processed: BTreeSet<EntityId>,
    #[serde(default)]
    pub purpose: Purpose,
    #[serde(default = "default_listening")]
    pub listening: bool,
    #[serde(default)]
    pub next_selection: Option<Selection>,
}
fn default_listening() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Selection {
    pub selected: BTreeSet<EntityId>,
    pub purpose: Purpose,
    pub effective_turn: i64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    #[default]
    Active,
    Search,
    Localization,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Float {
    pub id: EntityId,
    pub carrier: EntityId,
    pub side: Side,
    pub model: String,
    pub class: String,
    pub sensor: String,
    pub dropped_at: i64,
    pub active_at: i64,
    pub expires_at: i64,
    pub depth_m: f64,
    pub radio_channel: u16,
    pub transmitting: bool,
    pub lost: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fix {
    pub monitor: EntityId,
    pub monitor_class: String,
    pub side: Side,
    pub target: EntityId,
    pub report: SensorReport,
    pub buoys: Vec<EntityId>,
    pub formed_at: i64,
    pub tactical_turn: i64,
    pub contact: ContactId,
    #[serde(default = "default_quality")]
    pub quality: Quality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aim: Option<Vec2>,
}
fn default_quality() -> Quality {
    Quality::Fair
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchAlert {
    pub receiver: EntityId,
    pub receiver_class: String,
    pub side: Side,
    pub report: SensorReport,
    pub received_at: i64,
}
/// Actual normal detector classification, still private until a particular
/// qualified operator really receives this exact source report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClassificationEvidence {
    pub report: SensorReport,
    pub category: PlatformKind,
    pub frequency: Frequency,
    pub classification: Value,
    pub normal_probability: Option<f64>,
    pub alerted: Option<bool>,
}
impl ClassificationEvidence {
    fn matches_report(&self, report: &SensorReport) -> bool {
        self.report.observer == report.observer
            && self.report.sensor == report.sensor
            && self.report.target == report.target
            && self.report.at == report.at
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Received {
    pub receiver: EntityId,
    pub receiver_class: String,
    pub side: Side,
    pub report: SensorReport,
    pub received_at: i64,
    pub purpose: Purpose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification: Option<ClassificationEvidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReceiverTrial {
    pub at: i64,
    pub tactical_turn: i64,
    pub receiver: EntityId,
    pub buoy: EntityId,
    pub target: EntityId,
    pub purpose: Purpose,
    pub base_probability: f64,
    pub probability: f64,
    pub d100: u8,
    pub detected: bool,
    pub alert: Option<SearchAlert>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Option<Configuration>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub echo: Option<echo_ranging::State>,
    pub carriers: BTreeMap<EntityId, Carrier>,
    pub monitors: BTreeMap<EntityId, Monitor>,
    pub floats: BTreeMap<EntityId, Float>,
    /// Private source body / target key. Never projected to a player.
    pub native: BTreeMap<String, SensorReport>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub native_classifications: BTreeMap<String, ClassificationEvidence>,
    pub fixes: BTreeMap<String, Fix>,
    pub received: BTreeMap<String, Received>,
    pub alerts: BTreeMap<String, SearchAlert>,
    pub receiver_trials: BTreeMap<String, ReceiverTrial>,
    pub trial_markers: BTreeMap<String, String>,
    pub pending: BTreeMap<String, String>,
    pub traces: Vec<(EntityId, String, Value)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    EchoRange { initial_buoy: EntityId },
    DropSonobuoy {
        model: String,
        depth_m: f64,
        radio_channel: u16,
    },
    ListenSonobuoys {
        buoys: Vec<EntityId>,
        #[serde(default)]
        purpose: Purpose,
    },
}
fn runtime(w: &World) -> Option<&State> {
    Some(&w.rules.detection.as_ref()?.sonobuoys)
}
fn active(w: &World) -> bool {
    super::active(w)
}
fn exact_body<'a>(w: &'a World, name: &str) -> Result<&'a Entity, String> {
    let mut xs = w.entities.values().filter(|e| e.name == name);
    let e = xs
        .next()
        .ok_or_else(|| format!("5.4 sonobuoy original unit {name} absent"))?;
    if xs.next().is_some() || !e.alive || e.kind != PlatformKind::Aircraft {
        return Err("5.4 sonobuoy fit needs one actual original live aircraft body".into());
    }
    Ok(e)
}
fn bound_body(w: &World, id: EntityId, class: &str, side: Side) -> bool {
    w.entity(id)
        .is_some_and(|e| e.alive && e.class == class && e.side == side)
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let config = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| s.config.sonobuoys.clone());
    let Some(c) = config else {
        return Ok(());
    };
    if !sourced(&c.source) {
        return Err("5.4 sonobuoy source required".into());
    }
    let parent = &w.rules.detection.as_ref().unwrap().config;
    for (key, m) in &c.models {
        if key.trim().is_empty()
            || !sourced(&m.source)
            || !m.directional
            || m.lifetime_s <= 0
            || !finite_positive(m.mass_kg)
            || !m.radio_antenna_height_m.is_finite()
            || m.radio_antenna_height_m < 0.
            || !sourced(&m.protocol)
            || m.allowed_depths_m.is_empty()
            || m.allowed_depths_m.iter().any(|d| !finite_positive(*d))
            || m.radio_channels.is_empty()
            || m.radio_channels.contains(&0)
            || m.radio_channels.iter().collect::<BTreeSet<_>>().len() != m.radio_channels.len()
        {
            return Err("5.4 sonobuoy model requires sourced physical conventional directional parameters; omni/special processors pending".into());
        }
        let p =
            w.db.platforms
                .get(&m.platform_class)
                .ok_or("5.4 physical float class absent")?;
        if p.kind != PlatformKind::Ship
            || !finite_positive(p.hp)
            || p.max_speed_kts != 0.
            || p.vp_value != 0.
            || !p.weapons.is_empty()
            || p.deck.is_some()
            || p.sensors != vec![m.sensor.clone()]
            || !w.db.sensors.get(&m.sensor).is_some_and(|s| {
                s.kind
                    == if m.active {
                        SensorKind::SonarActive
                    } else {
                        SensorKind::SonarPassive
                    }
            })
        {
            return Err(
                "5.4 buoy must be an explicit unarmed stationary noncombat sensor class".into(),
            );
        }
        let binding = parent
            .bindings
            .get(&format!("{}:{}", m.platform_class, m.sensor));
        let valid_binding = match binding {
            Some(Receiver::Sonar {
                source,
                generation,
                mount: SonarMount::Buoy,
                active_range_nm,
                passive_range_nm,
                ..
            }) => {
                sourced(source)
                    && (*generation == 5 || ((6..=7).contains(generation) && active_range_nm.is_some_and(finite_positive)))
                    && (if m.active {
                        *active_range_nm
                    } else {
                        *passive_range_nm
                    })
                    .is_some_and(finite_positive)
            }
            _ => false,
        };
        if !valid_binding {
            return Err("5.4 model requires an actual sourced Gen5 directional Buoy receiver in the selected physical mode; other rows pending".into());
        }
    }
    let mut s = State {
        config: Some(c.clone()),
        ..Default::default()
    };
    for (name, f) in &c.carriers {
        if !sourced(&f.source) {
            return Err("5.4 finite buoy carriage source required".into());
        }
        let e = exact_body(w, name)?;
        let mut mass = 0.;
        for (model, count) in &f.loads {
            let m = c.models.get(model).ok_or("5.4 carriage model absent")?;
            mass += *count as f64 * m.mass_kg;
        }
        mass += crate::rules::aviation2026::remaining_declared_store_mass_kg(w, e.id)?;
        let flight = w
            .rules
            .aviation
            .as_ref()
            .and_then(|a| a.flights.get(&e.id))
            .ok_or("5.4 real source Flight required before finite buoy load")?;
        if !finite_positive(flight.spec.max_payload_kg)
            || !flight.payload_fraction.is_finite()
            || flight.payload_fraction * flight.spec.max_payload_kg + 1e-9 < mass
        {
            return Err(
                "5.4 original Flight payload must include every buoy plus remaining declared weapons and tanks".into(),
            );
        }
        s.carriers.insert(
            e.id,
            Carrier {
                class: e.class.clone(),
                side: e.side,
                carried: f.loads.clone(),
                deployed: Vec::new(),
            },
        );
    }
    for (name, f) in &c.monitors {
        let active_fit = f.search_channels == 0
            && f.passive_localization_channels == 0
            && f.active_channels == 1
            && f.processing_per_tactical_turn == 1;
        let passive_fit = f.search_channels == 1
            && (2..=3).contains(&f.passive_localization_channels)
            && f.active_channels == 0
            && f.processing_per_tactical_turn == f.passive_localization_channels;
        if !sourced(&f.source) || !sourced(&f.protocol) || (!active_fit && !passive_fit) {
            return Err("5.4 monitor needs explicit supported active or separate search/passive-localization channels and processing capacity; other K3 rows pending".into());
        }
        let e = exact_body(w, name)?;
        s.monitors.insert(
            e.id,
            Monitor {
                class: e.class.clone(),
                side: e.side,
                fit: f.clone(),
                selected: BTreeSet::new(),
                budget_epoch: epoch(w.tick),
                processed: BTreeSet::new(),
                purpose: Purpose::Active,
                listening: true,
                next_selection: None,
            },
        );
    }
    if !c.echo_fits.is_empty() { s.echo=Some(echo_ranging::State::default()); echo_ranging::initialize(w,&mut s)?; }
    w.rules.detection.as_mut().unwrap().sonobuoys = s;
    Ok(())
}
fn power(w: &World, id: EntityId) -> bool {
    w.entity(id)
        .is_some_and(|e| e.ext.get("damage.power_lost").copied().unwrap_or(0.) <= w.tick as f64)
        && crate::rules::expanded_damage2026::binding_operational(
            w,
            id,
            crate::rules::expanded_damage2026::ComponentKind::Communications,
            "sonobuoy_radio",
        )
        .unwrap_or(false)
}
fn monitor_available(w: &World, id: EntityId, m: &Monitor) -> bool {
    bound_body(w, id, &m.class, m.side)
        && power(w, id)
        && w.entity(id)
            .is_some_and(|e| e.kind == PlatformKind::Aircraft && e.altitude_m > 0.)
        && crate::rules::unmanned2026::allow_side_projection(w, id)
}
fn float_body(w: &World, f: &Float) -> bool {
    bound_body(w, f.id, &f.class, f.side)
        && w.entity(f.id).is_some_and(|e| {
            e.kind == PlatformKind::Ship
                && e.speed_kts == 0.
                && e.sensors.iter().any(|s| s.sensor == f.sensor)
        })
}
fn live(w: &World, s: &State, f: &Float) -> bool {
    if f.lost
        || !f.transmitting
        || w.tick < f.active_at
        || w.tick >= f.expires_at
        || !float_body(w, f)
        || !power(w, f.id)
    {
        return false;
    }
    let Some(m) = s.config.as_ref().and_then(|c| c.models.get(&f.model)) else {
        return false;
    };
    let sea = w
        .context
        .radar_environment
        .as_ref()
        .map(|e| e.sea_state as u64);
    let unsupported = tables()["deployment"]["unsupported_sea_state"]
        .as_u64()
        .unwrap();
    sea.is_some_and(|sea| sea < unsupported || sea == unsupported && m.sea_state6_capable)
        && w.entity(f.id).is_some_and(|e| {
            e.systems.sonar > 0.05 && !crate::sensors::sensor_damaged(e, &f.sensor)
        })
        && crate::rules::expanded_damage2026::sensor_operational(w, f.id, &f.sensor)
            .unwrap_or(false)
}
fn radio(w: &World, s: &State, id: EntityId, m: &Monitor, f: &Float) -> bool {
    if !m.listening || !monitor_available(w, id, m) || !live(w, s, f) || m.side != f.side {
        return false;
    }
    let Some(model) = s.config.as_ref().and_then(|c| c.models.get(&f.model)) else {
        return false;
    };
    if model.protocol != m.fit.protocol {
        return false;
    }
    let receiver = w.entity(id).unwrap();
    let transmitter = w.entity(f.id).unwrap();
    let horizon = crate::geom::horizon_nm(receiver.altitude_m, model.radio_antenna_height_m);
    receiver.pos.range_to(transmitter.pos)
        <= horizon.min(tables()["monitoring"]["maximum_range_nm"].as_f64().unwrap())
}
fn channels(m: &Monitor, purpose: Purpose) -> u16 {
    match purpose {
        Purpose::Active => m.fit.active_channels,
        Purpose::Search => m.fit.search_channels,
        Purpose::Localization => m.fit.passive_localization_channels,
    }
}
fn purpose_model(m: &Model, purpose: Purpose) -> bool {
    m.directional
        && match purpose {
            Purpose::Active => m.active,
            Purpose::Search | Purpose::Localization => !m.active,
        }
}
fn receiver_key(receiver: EntityId, target: EntityId) -> String {
    format!("{}:{}", receiver.0, target.0)
}
fn receipt_key(receiver: EntityId, buoy: EntityId, target: EntityId) -> String {
    format!("{}:{}:{}", receiver.0, buoy.0, target.0)
}
fn operator_alert<'a>(
    w: &World,
    s: &'a State,
    id: EntityId,
    m: &Monitor,
    target: EntityId,
) -> Option<&'a SearchAlert> {
    let alert = s.alerts.get(&receiver_key(id, target))?;
    let lifetime = tables()["passive_localization"]["prior_detection_tolerance_s"]
        .as_i64()
        .unwrap();
    (alert.receiver_class == m.class
        && alert.side == m.side
        && monitor_available(w, id, m)
        && alert.report.at <= w.tick
        && w.tick - alert.report.at <= lifetime
        && alert.received_at <= w.tick)
        .then_some(alert)
}
fn localization_alert<'a>(
    w: &World,
    s: &'a State,
    id: EntityId,
    m: &Monitor,
    target: EntityId,
) -> Option<&'a SearchAlert> {
    if m.purpose != Purpose::Localization {
        return None;
    }
    let a = operator_alert(w, s, id, m, target)?;
    (a.received_at < w.tick && m.selected.contains(&a.report.observer)).then_some(a)
}
/// Physical float success history is not an operator. Only an actual selected
/// receiver's previously received Search report supplies localization alert.
/// The detector consumes this before its original probability/D100 comparison.
pub fn sonar_alerted(
    w: &World,
    parent: &super::State,
    observer: EntityId,
    target: EntityId,
) -> Option<bool> {
    let s = &parent.sonobuoys;
    let f = s.floats.get(&observer)?;
    Some(s.monitors.iter().any(|(id, m)| {
        m.selected.contains(&observer)
            && radio(w, s, *id, m, f)
            && localization_alert(w, s, *id, m, target).is_some()
    }))
}
/// Preserve the detector's one original float/target/Tac die. Qualification is
/// per receiving operator: another listener's alert cannot leak a +15 success
/// to an unqualified Search or Active monitor. No second bonus or roll occurs.
pub fn after_evaluate(
    w: &World,
    parent: &mut super::State,
    observer: EntityId,
    sensor: &str,
    target: EntityId,
    scan: &mut super::ScanDecision,
) {
    let s = &mut parent.sonobuoys;
    let Some(f) = s.floats.get(&observer) else {
        return;
    };
    if f.sensor != sensor || !live(w, s, f) || scan.trace["rule"] != "5.4.6" {
        return;
    }
    let Some(d100) = scan.trace["d100"]
        .as_u64()
        .filter(|d| (1..=100).contains(d))
        .map(|d| d as u8)
    else {
        return;
    };
    let Some(normal_probability) = scan.trace["probability"].as_f64() else {
        return;
    };
    let Some(modified) = scan.trace["inputs"]["modified50_nm"].as_f64() else {
        return;
    };
    let Some((o, t)) = w.entity(observer).zip(w.entity(target)) else {
        return;
    };
    // This is the existing physical sonar calculation. It is private detector
    // evidence, never a localization aim or player geometry projection.
    let Ok(unalerted) = super::sonar_probability(modified, o.pos.range_to(t.pos), false) else {
        return;
    };
    let baseline = if normal_probability == 0. {
        0.
    } else {
        unalerted
    };
    let bonus = tables()["passive_localization"]["alerted_operator_modifier_percent"]
        .as_f64()
        .unwrap();
    let maximum = super::tables()["sonar"]["probability"][0].as_f64().unwrap();
    let mut trials = Vec::new();
    for (id, m) in &s.monitors {
        if !m.selected.contains(&observer) || !radio(w, s, *id, m, f) {
            continue;
        }
        let Some(model) = s.config.as_ref().and_then(|c| c.models.get(&f.model)) else {
            continue;
        };
        if !purpose_model(model, m.purpose) {
            continue;
        }
        let alert = localization_alert(w, s, *id, m, target).cloned();
        if m.purpose == Purpose::Localization && alert.is_none() {
            continue;
        }
        let probability = if alert.is_some() && normal_probability > 0. {
            (baseline + bonus).min(maximum)
        } else {
            baseline
        };
        let detected = f64::from(d100) <= probability;
        trials.push(ReceiverTrial {
            at: w.tick,
            tactical_turn: epoch(w.tick),
            receiver: *id,
            buoy: observer,
            target,
            purpose: m.purpose,
            base_probability: baseline,
            probability,
            d100,
            detected,
            alert,
        });
    }
    for trial in trials {
        let key = receipt_key(trial.receiver, observer, target);
        if trial.purpose == Purpose::Localization {
            let a = trial.alert.as_ref().unwrap();
            let detail = json!({"receiver":trial.receiver,"buoy":observer,"target":target,
                "tactical_turn":trial.tactical_turn,"base_probability":baseline,"alert_modifier":bonus,
                "probability":trial.probability,"d100":d100,"detected":trial.detected,
                "prior_detection_at":a.report.at,"alert_source_buoy":a.report.observer,
                "source":"5.4.6.3 actual monitor's received Search; original sonar float/Tac die"});
            let marker = detail.to_string();
            if s.trial_markers.get(&key) != Some(&marker) {
                s.trial_markers.insert(key.clone(), marker);
                s.traces
                    .push((trial.receiver, "5.4.6.3.localization_trial".into(), detail));
            }
        }
        s.receiver_trials.insert(key, trial);
    }
}
pub fn validate_action(w: &World, id: EntityId, action: &Action) -> Result<(), String> {
    if !active(w) || w.over {
        return Err("5.4 source buoy actions require active Full simulation".into());
    }
    let s = runtime(w).ok_or("5.4 buoy state absent")?;
    match action {
        Action::EchoRange { initial_buoy } => echo_ranging::validate(w,s,id,*initial_buoy),
        Action::DropSonobuoy {
            model,
            depth_m,
            radio_channel,
        } => {
            let c = s
                .carriers
                .get(&id)
                .ok_or("5.4 no finite original buoy carriage")?;
            if !bound_body(w, id, &c.class, c.side) || !power(w, id) {
                return Err("5.4 original carrier unavailable".into());
            }
            let e = w.entity(id).unwrap();
            let m = s
                .config
                .as_ref()
                .and_then(|c| c.models.get(model))
                .ok_or("5.4 sourced buoy model absent")?;
            if !depth_m.is_finite()
                || !m.allowed_depths_m.contains(depth_m)
                || !m.radio_channels.contains(radio_channel)
            {
                return Err("5.4 unsupported physical hydrophone depth or radio setting".into());
            }
            if c.carried.get(model).copied().unwrap_or(0) == 0 {
                return Err("5.4 actual finite buoy stock exhausted".into());
            }
            let f = w
                .rules
                .aviation
                .as_ref()
                .and_then(|a| a.flights.get(&id))
                .ok_or("5.4 real carrier Flight absent")?;
            let config = s.config.as_ref().unwrap();
            let buoy_mass = c.carried.iter().try_fold(0., |kg, (model, count)| {
                let model = config
                    .models
                    .get(model)
                    .ok_or("5.4 remaining carried model source absent")?;
                Ok::<f64, String>(kg + *count as f64 * model.mass_kg)
            })?;
            let required_mass =
                buoy_mass + crate::rules::aviation2026::remaining_declared_store_mass_kg(w, id)?;
            if e.kind != PlatformKind::Aircraft
                || e.altitude_m <= 0.
                || !finite_positive(f.fuel_nm)
                || !finite_positive(f.spec.max_payload_kg)
                || !f.payload_fraction.is_finite()
                || f.payload_fraction * f.spec.max_payload_kg + 1e-9 < required_mass
            {
                return Err(
                    "5.4 actual airborne carrier, fuel and physical carried mass required".into(),
                );
            }
            if !w
                .context
                .radar_environment
                .as_ref()
                .is_some_and(|e| e.sea_state <= 9)
            {
                return Err("5.4 sourced actual release sea state required".into());
            }
            if w.context
                .terrain
                .as_ref()
                .is_some_and(|t| t.contains(e.pos))
            {
                return Err("5.4 water deployment requires actual overwater position".into());
            }
            // Invalid altitude/speed or unsupported sea kills the expended
            // physical item at impact, rather than granting a free refusal.
            Ok(())
        }
        Action::ListenSonobuoys { buoys, purpose } => {
            let m = s
                .monitors
                .get(&id)
                .ok_or("5.4 actual installed buoy monitor absent")?;
            if !monitor_available(w, id, m) {
                return Err("5.4 original monitoring receiver unavailable".into());
            }
            let xs: BTreeSet<_> = buoys.iter().copied().collect();
            if xs.len() != buoys.len() || xs.len() > channels(m, *purpose) as usize {
                return Err(
                    "5.4 selected purpose channel capacity exceeded or duplicate float".into(),
                );
            }
            let mut used = if m.budget_epoch == epoch(w.tick) {
                m.processed.clone()
            } else {
                BTreeSet::new()
            };
            used.extend(xs.iter().copied());
            if used.len() > m.fit.processing_per_tactical_turn as usize {
                return Err("5.4 distinct current-turn processing capacity exceeded".into());
            }
            for bid in &xs {
                let f = s
                    .floats
                    .get(bid)
                    .ok_or("5.4 selected buoy is not a known actual own deployed body")?;
                let model = s
                    .config
                    .as_ref()
                    .and_then(|c| c.models.get(&f.model))
                    .ok_or("5.4 source model unavailable")?;
                if f.side != m.side
                    || f.lost
                    || !float_body(w, f)
                    || model.protocol != m.fit.protocol
                    || !purpose_model(model, *purpose)
                {
                    return Err("5.4 selected live own compatible float required".into());
                }
            }
            if *purpose == Purpose::Localization
                && !xs.is_empty()
                && !s.alerts.values().any(|a| {
                    a.receiver == id
                        && xs.contains(&a.report.observer)
                        && operator_alert(w, s, id, m, a.report.target).is_some()
                })
            {
                return Err("5.4.6.3 passive localization requires this original monitor's actual prior received field detection".into());
            }
            Ok(())
        }
    }
}
fn event(w: &mut World, unit: EntityId, side: Option<Side>, rule: &str, detail: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "detection2026".into(),
            rule: rule.into(),
            side,
            unit: Some(unit),
            contact: None,
            detail: detail.to_string(),
        },
        side.map(Visibility::Side).unwrap_or(Visibility::Referee),
    );
}
pub fn control(w: &mut World, id: EntityId, action: &Action) -> Result<(), String> {
    validate_action(w, id, action)?;
    match action {
        Action::EchoRange { initial_buoy } => { return echo_ranging::pulse(w,id,*initial_buoy); },
        Action::ListenSonobuoys { buoys, purpose } => {
            let now = epoch(w.tick);
            let at_boundary = w.tick.rem_euclid(turn_seconds()) == 0;
            let s = &mut w.rules.detection.as_mut().unwrap().sonobuoys;
            let m = s.monitors.get_mut(&id).unwrap();
            if m.budget_epoch != now {
                m.budget_epoch = now;
                m.processed.clear();
            }
            let requested: BTreeSet<_> = buoys.iter().copied().collect();
            let immediate =
                requested.is_empty() || requested == m.selected && *purpose == m.purpose;
            if requested.is_empty() {
                m.listening = false;
                m.next_selection = None;
            } else if immediate {
                m.listening = true;
                m.next_selection = None;
                m.processed.extend(requested.iter().copied());
            } else {
                let effective_turn = if at_boundary { now } else { now + 1 };
                m.next_selection = Some(Selection {
                    selected: requested,
                    purpose: *purpose,
                    effective_turn,
                });
            }
            let side = m.side;
            if immediate {
                s.fixes.retain(|_, f| f.monitor != id);
                s.received.retain(|_, r| r.receiver != id);
                s.receiver_trials.retain(|_, t| t.receiver != id);
            }
            event(
                w,
                id,
                Some(side),
                "5.4.6.3.buoys_selected",
                json!({"buoys":buoys,"purpose":purpose,"tactical_turn":now,"effective_turn":if immediate||at_boundary{now}else{now+1},"immediate_stop_or_same_field":immediate}),
            );
        }
        Action::DropSonobuoy {
            model,
            depth_m,
            radio_channel,
        } => {
            let s = runtime(w).unwrap();
            let m = s.config.as_ref().unwrap().models[model].clone();
            let carrier = w.entity(id).unwrap().clone();
            let class = w.db.platforms[&m.platform_class].clone();
            let sea = w.context.radar_environment.as_ref().unwrap().sea_state as u64;
            let deployment = &tables()["deployment"];
            let envelope = carrier.altitude_m >= deployment["minimum_altitude_m"].as_f64().unwrap()
                && carrier.altitude_m <= deployment["maximum_altitude_m"].as_f64().unwrap()
                && carrier.speed_kts >= 0.
                && carrier.speed_kts <= deployment["maximum_speed_kts"].as_f64().unwrap();
            let survives = envelope
                && (sea < deployment["unsupported_sea_state"].as_u64().unwrap()
                    || sea == deployment["unsupported_sea_state"].as_u64().unwrap()
                        && m.sea_state6_capable);
            // Even clock arithmetic must succeed before an indivisible debit.
            let active_at = epoch(w.tick)
                .checked_add(1)
                .and_then(|e| e.checked_mul(turn_seconds()))
                .ok_or("5.4 buoy activation clock overflow")?;
            let expires_at = w
                .tick
                .checked_add(m.lifetime_s)
                .ok_or("5.4 buoy lifetime clock overflow")?;
            // All validation precedes this indivisible item + actual mass debit.
            let f = w
                .rules
                .aviation
                .as_mut()
                .unwrap()
                .flights
                .get_mut(&id)
                .unwrap();
            f.payload_fraction = (f.payload_fraction - m.mass_kg / f.spec.max_payload_kg).max(0.);
            *w.rules
                .detection
                .as_mut()
                .unwrap()
                .sonobuoys
                .carriers
                .get_mut(&id)
                .unwrap()
                .carried
                .get_mut(model)
                .unwrap() -= 1;
            let bid = w.allocate_id();
            let float = Float {
                id: bid,
                carrier: id,
                side: carrier.side,
                model: model.clone(),
                class: m.platform_class.clone(),
                sensor: m.sensor.clone(),
                dropped_at: w.tick,
                active_at,
                expires_at,
                depth_m: *depth_m,
                radio_channel: *radio_channel,
                transmitting: false,
                lost: !survives,
            };
            let mut ext = BTreeMap::new();
            ext.insert(format!("detection.sonar_depth.{}", m.sensor), *depth_m);
            // Role is authenticated by the typed runtime, never by this mirror.
            ext.insert("detection.sensor_float".into(), 1.);
            let entity = Entity {
                rule_data: None,
                id: bid,
                side: carrier.side,
                class: class.id.clone(),
                name: format!("{} buoy {}", carrier.name, bid.0),
                kind: class.kind,
                pos: carrier.pos,
                course_deg: 0.,
                speed_kts: 0.,
                ordered_course_deg: 0.,
                ordered_speed_kts: 0.,
                waypoints: Default::default(),
                altitude_m: 0.,
                ordered_altitude_m: 0.,
                depth_m: 0.,
                ordered_depth_m: 0.,
                hp: if survives { class.hp } else { 0. },
                hp_max: class.hp,
                alive: survives,
                systems: Default::default(),
                fire_on: false,
                assigned: None,
                weapons: Vec::new(),
                sensors: vec![SensorInstance {
                    sensor: m.sensor.clone(),
                    active: false,
                }],
                chaff: 0,
                decoys: 0,
                auto_chaff: false,
                chaff_active_s: 0.,
                endurance_s: None,
                burn_s: 0.,
                flooding_s: 0.,
                ext,
                deck: None,
                mission: None,
                station: None,
                projectile: None,
                last_emit_tick: w.tick,
            };
            w.entities.insert(bid, entity);
            let s = &mut w.rules.detection.as_mut().unwrap().sonobuoys;
            s.carriers.get_mut(&id).unwrap().deployed.push(bid);
            s.floats.insert(bid, float);
            event(
                w,
                id,
                Some(carrier.side),
                "5.4.4.sonobuoy_dropped",
                json!({"buoy":bid,"model":model,"position":carrier.pos,"depth_m":depth_m,"radio_channel":radio_channel,"operational":survives,"active_at":active_at,"mass_kg":m.mass_kg}),
            );
        }
    }
    Ok(())
}
pub fn is_sensor_float(w: &World, id: EntityId) -> bool {
    runtime(w).is_some_and(|s| s.floats.contains_key(&id))
}
pub fn observer_allowed(w: &World, parent: &super::State, id: EntityId) -> bool {
    parent
        .sonobuoys
        .floats
        .get(&id)
        .is_none_or(|f| live(w, &parent.sonobuoys, f))
}
pub fn begin_detection(w: &mut World) {
    if !active(w) {
        return;
    }
    let Some(mut s) = w
        .rules
        .detection
        .as_mut()
        .map(|p| std::mem::take(&mut p.sonobuoys))
    else {
        return;
    };
    if s.config.is_none() {
        w.rules.detection.as_mut().unwrap().sonobuoys = s;
        return;
    }
    let now = w.tick;
    let current_epoch = epoch(now);
    let mut changed = BTreeSet::new();
    for (id, m) in &mut s.monitors {
        if m.next_selection
            .as_ref()
            .is_some_and(|n| n.effective_turn <= current_epoch)
        {
            let next = m.next_selection.take().unwrap();
            m.selected = next.selected;
            m.purpose = next.purpose;
            m.listening = true;
            changed.insert(*id);
            if m.budget_epoch == current_epoch {
                m.processed.extend(m.selected.iter().copied());
            }
        }
        if m.budget_epoch != current_epoch {
            m.budget_epoch = current_epoch;
            m.processed = if m.listening {
                m.selected.clone()
            } else {
                BTreeSet::new()
            };
        }
        // Invalid body cannot bank the original receiver's knowledge.
        if !bound_body(w, *id, &m.class, m.side) {
            m.selected.clear();
            m.next_selection = None;
            m.listening = false;
        }
    }
    s.received.retain(|_, r| !changed.contains(&r.receiver));
    s.fixes.retain(|_, f| !changed.contains(&f.monitor));
    s.receiver_trials
        .retain(|_, t| !changed.contains(&t.receiver));
    let ids: Vec<_> = s.floats.keys().copied().collect();
    for id in ids {
        let f = s.floats[&id].clone();
        let model = &s.config.as_ref().unwrap().models[&f.model];
        let sea = w
            .context
            .radar_environment
            .as_ref()
            .map(|e| e.sea_state as u64);
        let unsupported = tables()["deployment"]["unsupported_sea_state"]
            .as_u64()
            .unwrap();
        let sea_ok = sea
            .is_some_and(|sea| sea < unsupported || sea == unsupported && model.sea_state6_capable);
        let lost = f.lost || now >= f.expires_at || !float_body(w, &f) || !sea_ok;
        let transmitting = !lost && now >= f.active_at;
        let row = s.floats.get_mut(&id).unwrap();
        row.lost = lost;
        row.transmitting = transmitting;
        if let Some(e) = w.entity_mut(id) {
            for inst in &mut e.sensors {
                if inst.sensor == f.sensor {
                    inst.active = transmitting;
                }
            }
            if lost {
                e.alive = false;
                e.hp = 0.;
            }
        }
        if transmitting != f.transmitting || lost != f.lost {
            event(
                w,
                f.carrier,
                Some(f.side),
                "5.4.4.sonobuoy_lifecycle",
                json!({"buoy":id,"transmitting":transmitting,"lost":lost,"at":now}),
            );
        }
    }
    s.native.retain(|_, r| {
        epoch(r.at) == current_epoch && s.floats.get(&r.observer).is_some_and(|f| !f.lost)
    });
    let native = &s.native;
    s.native_classifications.retain(|key, evidence| {
        native
            .get(key)
            .is_some_and(|report| evidence.matches_report(report))
    });
    s.fixes.retain(|_, f| f.tactical_turn == current_epoch);
    s.received.retain(|_, r| {
        epoch(r.report.at) == current_epoch
            && s.floats.get(&r.report.observer).is_some_and(|f| !f.lost)
    });
    s.receiver_trials
        .retain(|_, t| t.tactical_turn == current_epoch);
    let tolerance = tables()["passive_localization"]["prior_detection_tolerance_s"]
        .as_i64()
        .unwrap();
    s.alerts
        .retain(|_, a| a.report.at <= now && now - a.report.at <= tolerance);
    w.rules.detection.as_mut().unwrap().sonobuoys = s;
}
/// Return true for every authenticated float, even a malformed/ineligible
/// report: that body must never escape through generic side/native publication.
pub fn capture_native(
    w: &World,
    parent: &mut super::State,
    r: &SensorReport,
    scan: &super::ScanDecision,
) -> bool {
    let s = &mut parent.sonobuoys;
    let Some(f) = s.floats.get(&r.observer) else {
        return false;
    };
    let model = s.config.as_ref().and_then(|c| c.models.get(&f.model));
    let correct = model.is_some_and(|m| {
        if m.active {
            r.sensor_kind == SensorKind::SonarActive
                && r.measured_position
                    .is_some_and(|p| p.x.is_finite() && p.y.is_finite())
        } else {
            r.sensor_kind == SensorKind::SonarPassive
                && r.measured_position.is_none()
                && r.bearing_deg.is_finite()
        }
    });
    if live(w, s, f)
        && r.sensor == f.sensor
        && r.at == w.tick
        && r.bearing_alternatives.is_empty()
        && correct
    {
        let key = format!("{}:{}", r.observer.0, r.target.0);
        s.native.insert(key.clone(), r.clone());
        // No stale classification can survive replacement by an unknown
        // actual report. The hidden target association never supplies a kind.
        s.native_classifications.remove(&key);
        if let (Some(category), Some(Receiver::Sonar { frequency, .. }), Some(_), Some(_)) = (
            scan.known_category,
            parent
                .config
                .bindings
                .get(&format!("{}:{}", f.class, r.sensor)),
            scan.trace["classification"]["level"]
                .as_u64()
                .filter(|level| *level >= 1),
            scan.trace["classification"]["d100"]
                .as_u64()
                .filter(|roll| (1..=100).contains(roll)),
        ) {
            if scan.detected && r.sensor_kind == SensorKind::SonarPassive {
                s.native_classifications.insert(
                    key,
                    ClassificationEvidence {
                        report: r.clone(),
                        category,
                        frequency: *frequency,
                        classification: scan.trace["classification"].clone(),
                        normal_probability: scan.trace["probability"].as_f64(),
                        alerted: scan.trace["inputs"]["alerted"].as_bool(),
                    },
                );
            }
        }
    }
    true
}
fn publish_contact(w: &mut World, side: Side, r: &SensorReport, aim: Option<Vec2>) -> ContactId {
    let estimate = aim.unwrap_or_else(|| r.plot_position());
    let prior = w.sides[side.index()]
        .contacts
        .values()
        .find(|c| c.about == r.target && !c.stale)
        .map(|c| c.id);
    if let Some(cid) = prior {
        let c = w.sides[side.index()].contacts.get_mut(&cid).unwrap();
        // A new bearing ray cannot erase independently measured side knowledge.
        if aim.is_some() && (c.last_update_tick <= r.at || c.bearing_only) {
            c.estimate = estimate;
            c.bearing_only = false;
            c.last_update_tick = r.at;
        } else if c.bearing_only && c.last_update_tick <= r.at {
            c.estimate = estimate;
            c.last_update_tick = r.at;
        }
        if !c.sensors.contains(&r.sensor) {
            c.sensors.push(r.sensor.clone());
        }
        return cid;
    }
    let category = w
        .entity(r.target)
        .map(|e| e.kind)
        .unwrap_or(PlatformKind::Ship);
    let s = &mut w.sides[side.index()];
    s.next_contact += 1;
    let cid = ContactId(s.next_contact);
    s.contacts.insert(
        cid,
        Contact {
            id: cid,
            about: r.target,
            category,
            class_name: None,
            estimate,
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: aim.is_none(),
            quality: if aim.is_some() { 1. } else { 0.25 },
            last_update_tick: r.at,
            first_tick: r.at,
            stale: false,
            sensors: vec![r.sensor.clone()],
            offset: Vec2::zero(),
            announced: false,
        },
    );
    w.emit(
        EventKind::Detection {
            side,
            contact: cid,
            category: "unknown".into(),
            pos_hint: estimate,
            sensor: r.sensor.clone(),
        },
        Visibility::Side(side),
    );
    cid
}
fn received_live(w: &World, s: &State, m: &Monitor, r: &Received) -> bool {
    r.receiver_class == m.class
        && r.side == m.side
        && r.purpose == m.purpose
        && epoch(r.report.at) == epoch(w.tick)
        && r.report.at <= w.tick
        && m.selected.contains(&r.report.observer)
        && s.floats
            .get(&r.report.observer)
            .is_some_and(|f| radio(w, s, r.receiver, m, f))
}
fn received_classification(
    w: &World,
    s: &State,
    m: &Monitor,
    trial: &ReceiverTrial,
    report: &SensorReport,
) -> Result<Option<ClassificationEvidence>, String> {
    let Some(evidence) = s
        .native_classifications
        .get(&format!("{}:{}", report.observer.0, report.target.0))
        .filter(|evidence| evidence.matches_report(report))
    else {
        return Ok(None);
    };
    // Native unmonitored physical history is not an operator's retained
    // contact. SearchAlert is actual radio reception; a previous qualified
    // classification receipt preserves that established right as Search
    // refreshes its latest report during this same Tactical Turn.
    let prior_search = operator_alert(w, s, trial.receiver, m, report.target)
        .is_some_and(|alert| epoch(alert.report.at) < epoch(w.tick));
    let prior_classified = s.received.values().any(|received| {
        received.receiver == trial.receiver
            && received.report.target == report.target
            && received.received_at < w.tick
            && received
                .classification
                .as_ref()
                .is_some_and(|classification| classification.matches_report(&received.report))
            && received_live(w, s, m, received)
    });
    if !trial.detected || !(prior_search || prior_classified) {
        return Ok(None);
    }
    // The minimum sourced non-narrow-band classifier earns only ship/sub
    // domain once the receiver itself detects and retains contact. LF/VLF
    // probability can depend on A's alerted operator while B did not earn
    // that bonus. Do not copy such a shared native result without complete
    // receiver-specific classification inputs or invent another die.
    if !matches!(
        evidence.frequency,
        Frequency::Lmf | Frequency::Mf | Frequency::Hf
    ) || !matches!(evidence.category, PlatformKind::Ship | PlatformKind::Sub)
    {
        return Err("5.4.6.6 receiver-specific LF/VLF or torpedo classification inputs pending; another operator's native classification is not a receipt".into());
    }
    Ok(Some(evidence.clone()))
}
fn passive_aim(reports: &[Received]) -> Result<Vec2, String> {
    let epsilon = tables()["passive_localization"]["numerical_geometry_epsilon_nm"]
        .as_f64()
        .unwrap();
    let parallel = tables()["passive_localization"]["numerical_parallel_epsilon"]
        .as_f64()
        .unwrap();
    if reports.len() < 2 {
        return Err("5.4.6.3 one passive bearing supplies no range or actionable aim".into());
    }
    let mut rays = Vec::new();
    for r in reports {
        let r = &r.report;
        if r.measured_position.is_some()
            || !r.bearing_alternatives.is_empty()
            || r.bearing_error_deg
                .is_some_and(|e| !e.is_finite() || e != 0.)
        {
            return Err(
                "5.4.6.3 ambiguous/error-cone passive geometry needs source-qualified adjudication"
                    .into(),
            );
        }
        let angle = r.bearing_deg.to_radians();
        rays.push((r.observer_position, Vec2::new(angle.sin(), angle.cos())));
    }
    let cross = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
    let mut candidate = None;
    'pair: for a in 0..rays.len() {
        for b in a + 1..rays.len() {
            let (pa, da) = rays[a];
            let (pb, db) = rays[b];
            if pa.range_to(pb) <= epsilon {
                continue;
            }
            let denominator = cross(da, db);
            if denominator.abs() <= parallel {
                continue;
            }
            let delta = Vec2::new(pb.x - pa.x, pb.y - pa.y);
            let ta = cross(delta, db) / denominator;
            let tb = cross(delta, da) / denominator;
            if ta <= 0. || tb <= 0. {
                continue;
            }
            let p = Vec2::new(pa.x + ta * da.x, pa.y + ta * da.y);
            if p.x.is_finite() && p.y.is_finite() {
                candidate = Some(p);
                break 'pair;
            }
        }
    }
    let p =
        candidate.ok_or("5.4.6.3 passive rays have no unique independent forward intersection")?;
    for (origin, dir) in rays {
        let delta = Vec2::new(p.x - origin.x, p.y - origin.y);
        if delta.x * dir.x + delta.y * dir.y <= 0. || cross(delta, dir).abs() > epsilon {
            return Err("5.4.6.3 actual received passive bearings are inconsistent; no truth point or guessed averaging".into());
        }
    }
    Ok(p)
}
fn uncertainty(q: Quality) -> Value {
    match q {
        Quality::Good => tables()["passive_localization"]["good_uncertainty_description"].clone(),
        Quality::Poor => tables()["passive_localization"]["poor_uncertainty_description"].clone(),
        Quality::Fair => tables()["monitoring"]["fair_uncertainty_description"].clone(),
        Quality::NA => json!("No Attack; actual passive bearing only"),
    }
}
pub fn end_detection(w: &mut World) {
    if !active(w) {
        return;
    }
    let Some(mut s) = w
        .rules
        .detection
        .as_mut()
        .map(|p| std::mem::take(&mut p.sonobuoys))
    else {
        return;
    };
    let now = w.tick;
    let turn = epoch(now);
    let mut received = Vec::new();
    let mut classification_pending = Vec::new();
    for trial in s
        .receiver_trials
        .values()
        .filter(|t| t.at == now && t.detected)
    {
        let Some(m) = s.monitors.get(&trial.receiver) else {
            continue;
        };
        let Some(f) = s.floats.get(&trial.buoy) else {
            continue;
        };
        if m.purpose != trial.purpose
            || !m.selected.contains(&f.id)
            || !radio(w, &s, trial.receiver, m, f)
        {
            continue;
        }
        let Some(report) = s
            .native
            .get(&format!("{}:{}", f.id.0, trial.target.0))
            .filter(|r| r.at == now)
        else {
            continue;
        };
        let classification = match received_classification(w, &s, m, trial, report) {
            Ok(evidence) => evidence,
            Err(reason) => {
                classification_pending.push((trial.receiver, trial.buoy, trial.target, reason));
                None
            }
        };
        received.push(Received {
            receiver: trial.receiver,
            receiver_class: m.class.clone(),
            side: m.side,
            report: report.clone(),
            received_at: now,
            purpose: m.purpose,
            classification,
        });
    }
    for (receiver, buoy, target, reason) in classification_pending {
        let key = format!("classification:{}", receipt_key(receiver, buoy, target));
        if s.pending.get(&key) != Some(&reason) {
            s.traces.push((
                receiver,
                "5.4.6.6.sonobuoy_classification_pending".into(),
                json!({"receiver":receiver,"buoy":buoy,"target":target,"reason":reason}),
            ));
        }
        s.pending.insert(key, reason);
    }
    for r in received {
        let key = receipt_key(r.receiver, r.report.observer, r.report.target);
        if let Some(classification) = &r.classification {
            // Side knowledge keeps its existing shared semantics. Actor FCS
            // still requires that actor's actual qualified fix/receipt.
            w.rules.detection.as_mut().unwrap().categories.insert(
                format!("{}:{}", r.side.index(), r.report.target.0),
                classification.category,
            );
            s.pending.remove(&format!("classification:{key}"));
            let marker_key = format!("classification:{}:{}", r.receiver.0, r.report.target.0);
            let marker = format!("{turn}:{:?}", classification.category);
            if s.trial_markers.get(&marker_key) != Some(&marker) {
                s.trial_markers.insert(marker_key, marker);
                s.traces.push((r.receiver, "5.4.6.6.sonobuoy_classification_received".into(),
                    json!({"receiver":r.receiver,"report":r.report,"category":classification.category,
                        "frequency":classification.frequency,"classification":classification.classification,
                        "normal_probability":classification.normal_probability,"alerted":classification.alerted})));
            }
        }
        if r.purpose == Purpose::Search {
            // Only an actual received search observation can establish this
            // operator's qualification. Private raw or another receiver cannot.
            publish_contact(w, r.side, &r.report, None);
            s.alerts.insert(
                receiver_key(r.receiver, r.report.target),
                SearchAlert {
                    receiver: r.receiver,
                    receiver_class: r.receiver_class.clone(),
                    side: r.side,
                    report: r.report.clone(),
                    received_at: now,
                },
            );
        }
        s.received.insert(key, r);
    }
    let previous = s.fixes.clone();
    s.fixes.clear();
    let mut groups: BTreeMap<String, Vec<Received>> = BTreeMap::new();
    for r in s.received.values() {
        let Some(m) = s.monitors.get(&r.receiver) else {
            continue;
        };
        if r.purpose == Purpose::Search
            || !received_live(w, &s, m, r)
            || r.purpose == Purpose::Active && r.received_at != now
        {
            continue;
        }
        if r.purpose == Purpose::Localization
            && localization_alert(w, &s, r.receiver, m, r.report.target).is_none()
        {
            continue;
        }
        groups
            .entry(receiver_key(r.receiver, r.report.target))
            .or_default()
            .push(r.clone());
    }
    for (key, mut reports) in groups {
        reports.sort_by_key(|r| r.report.observer);
        reports.dedup_by_key(|r| r.report.observer);
        let r = reports[0].clone();
        let quality = if r.purpose == Purpose::Active {
            Quality::Fair
        } else {
            let Some(q) = tables()["passive_localization"]["conventional_directional_counts"]
                [reports.len().to_string()]
            .as_str()
            .and_then(|s| Quality::parse(s).ok()) else {
                continue;
            };
            q
        };
        if quality == Quality::NA {
            publish_contact(w, r.side, &r.report, None);
            continue;
        }
        let aim = if r.purpose == Purpose::Active {
            r.report.measured_position
        } else {
            match passive_aim(&reports) {
                Ok(p) => {
                    s.pending.remove(&key);
                    Some(p)
                }
                Err(reason) => {
                    if s.pending.get(&key) != Some(&reason) {
                        s.traces.push((
                            r.receiver,
                            "5.4.6.3.localization_pending".into(),
                            json!({"reason":reason}),
                        ));
                    }
                    s.pending.insert(key.clone(), reason);
                    None
                }
            }
        };
        let cid = publish_contact(w, r.side, &r.report, aim);
        let buoys: Vec<_> = reports.iter().map(|r| r.report.observer).collect();
        let old = previous.get(&key);
        let changed =
            old.is_none_or(|f| f.tactical_turn != turn || f.quality != quality || f.buoys != buoys);
        s.fixes.insert(
            key,
            Fix {
                monitor: r.receiver,
                monitor_class: r.receiver_class,
                side: r.side,
                target: r.report.target,
                report: r.report.clone(),
                buoys: buoys.clone(),
                formed_at: now,
                tactical_turn: turn,
                contact: cid,
                quality,
                aim,
            },
        );
        if changed {
            event(
                w,
                r.receiver,
                None,
                "5.4.6.3.sonobuoy_fix",
                json!({"receiver":r.receiver,"report":r.report,
            "buoys":buoys,"quality":quality,"tactical_turn":turn,"uncertainty":uncertainty(quality),"purpose":r.purpose,"aim":aim}),
            );
        }
    }
    echo_ranging::integrate(w,&mut s);
    for (unit, rule, detail) in std::mem::take(&mut s.traces) {
        event(w, unit, None, &rule, detail);
    }
    w.rules.detection.as_mut().unwrap().sonobuoys = s;
}
fn usable_fix<'a>(w: &World, s: &'a State, id: EntityId, target: EntityId) -> Option<&'a Fix> {
    let m = s.monitors.get(&id)?;
    let f = s.fixes.get(&receiver_key(id, target))?;
    if f.tactical_turn != epoch(w.tick)
        || w.tick < f.formed_at
        || w.tick - f.formed_at > 1
        || f.monitor_class != m.class
        || !monitor_available(w, id, m)
        || f.quality == Quality::NA
        || f.aim.is_none()
    {
        return None;
    }
    if f.buoys.is_empty()
        || f.buoys.iter().any(|id| {
            !m.selected.contains(id)
                || !s
                    .floats
                    .get(id)
                    .is_some_and(|float| radio(w, s, f.monitor, m, float))
        })
    {
        return None;
    }
    if m.purpose == Purpose::Localization && localization_alert(w, s, id, m, target).is_none() {
        return None;
    }
    Some(f)
}
pub fn located_report(w: &World, id: EntityId, target: EntityId) -> Option<SensorReport> {
    let s = runtime(w)?;
    let f = usable_fix(w, s, id, target)?;
    // A passive fix is derived from multiple bearings, not a ranged native
    // report. Only the existing actual Active report may use this single-report API.
    f.report
        .measured_position
        .is_some()
        .then(|| f.report.clone())
}
pub fn attack_solution(w: &World, id: EntityId, target: EntityId) -> Option<AttackSolution> {
    let s = runtime(w)?;
    let f = usable_fix(w, s, id, target)?;
    Some(AttackSolution {
        quality: f.quality,
        native: None,
        reported_aim: f.aim,
        source: json!({"rule":"5.4.6.3","source":"actual selected conventional directional buoy radio localization",
            "observer":f.report.observer,"receiver":id,"sensor":f.report.sensor,"at":f.report.at,
            "buoys":f.buoys,"uncertainty":uncertainty(f.quality)}),
    })
}
pub fn own_view(w: &World, side: Side, id: EntityId) -> Value {
    let Some(s) = runtime(w) else {
        return Value::Null;
    };
    let carrier = s
        .carriers
        .get(&id)
        .filter(|c| c.side == side && bound_body(w, id, &c.class, side));
    let monitor = s
        .monitors
        .get(&id)
        .filter(|m| m.side == side && bound_body(w, id, &m.class, side));
    if carrier.is_none() && monitor.is_none() {
        return Value::Null;
    }
    let deployed:Vec<_>=carrier.into_iter().flat_map(|c|c.deployed.iter()).filter_map(|id|s.floats.get(id))
        .map(|f|json!({"id":f.id,"model":f.model,"dropped_at":f.dropped_at,"active_at":f.active_at,
            "expires_at":f.expires_at,"depth_m":f.depth_m,"radio_channel":f.radio_channel,"transmitting":live(w,s,f),"lost":f.lost})).collect();
    let observations:Vec<_>=s.received.values().filter(|r|r.receiver==id&&r.side==side
        &&monitor.is_some_and(|m|received_live(w,s,m,r))).map(|r| {
            let report=&r.report;
            json!({"contact":w.sides[side.index()].contacts.values().find(|c|c.about==report.target&&!c.stale).map(|c|c.id),
                "observer":report.observer,"observer_position":report.observer_position,"sensor":report.sensor,
                "at":report.at,"received_at":r.received_at,"bearing_deg":report.bearing_deg,
                "measured_position":report.measured_position,"bearing_only":report.measured_position.is_none(),
                "bearing_error_deg":report.bearing_error_deg,"purpose":r.purpose})
        }).collect();
    let fixes:Vec<_>=s.fixes.values().filter(|f|f.monitor==id&&f.side==side&&usable_fix(w,s,id,f.target).is_some())
        .map(|f|json!({"contact":f.contact,"quality":f.quality,"buoys":f.buoys,"at":f.report.at,
            "observer":f.report.observer,"measured_position":f.aim,"uncertainty":uncertainty(f.quality)})).collect();
    let pending: Vec<_> = s
        .fixes
        .values()
        .filter(|f| f.monitor == id && f.side == side)
        .filter_map(|f| {
            s.pending
                .get(&receiver_key(id, f.target))
                .map(|r| json!({"contact":f.contact,"reason":r}))
        })
        .collect();
    json!({"carried":carrier.map(|c|&c.carried),"deployed":deployed,"selected":monitor.map(|m|&m.selected),
        "purpose":monitor.map(|m|m.purpose),"listening":monitor.map(|m|m.listening),"next_selection":monitor.and_then(|m|m.next_selection.as_ref()),"observations":observations,"fixes":fixes,"pending":pending})
}

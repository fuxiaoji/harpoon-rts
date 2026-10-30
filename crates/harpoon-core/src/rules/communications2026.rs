//! Native report communications, explicitly fitted TDL and independent FCS exchange.
//! Voice remains warning-only. Link capability never follows radar/class names.
mod air;
mod alerts;
mod exchange;
mod links;
pub mod hf_es;
pub mod annex;
pub mod relay;
use super::detection2026::SensorReport;
use crate::types::PlatformKind;
use crate::{ContactId, EntityId, EventKind, Side, Visibility, World};
pub(crate) use air::source_still_live as air_source_still_live;
pub use air::{received_air_solution, AirFcsEvidence};
pub use alerts::{commit_sonar_attempts, sonar_alert, SonarAttempt};
pub use exchange::{
    cue_evidence, received_located_report, received_solution, CueEvidence, CuePurpose, FcsDomain,
    FcsEvidence, MessageRef,
};
pub use links::{LinkKind, Transport};
pub fn air_increment_seconds() -> i64 {
    tables()["tdl_source"]["air_transfer"]["increment_seconds"]
        .as_i64()
        .filter(|n| *n > 0)
        .expect("source30-second air engagement Increment")
}
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static TABLES: OnceLock<Value> = OnceLock::new();
    TABLES.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../data/rules/communications-2026.json"
        ))
        .expect("verified communications scope A source/policy")
    })
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    /// Exact unit.name, never a class/radar-name capability heuristic.
    pub units: BTreeMap<String, UnitFit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitFit {
    pub source: String,
    pub fits: BTreeMap<String, VoiceFit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceFit {
    pub source: String,
    pub protocol: String,
    pub send: bool,
    pub receive: bool,
    pub operational: bool,
    /// Directed sender edge to exact receiver.name. No reciprocal edge inferred.
    pub peers: Vec<String>,
    #[serde(default, skip_serializing_if = "LinkKind::is_voice")]
    pub warning_kind: LinkKind,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub automatic_warning: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub targeting_system: bool,
    #[serde(default, skip_serializing_if = "Transport::is_author_directed")]
    pub transport: Transport,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub radio_qualification: Option<links::RadioQualification>,
    /// Exact optional Annex L source card. Does not grant a platform fit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annex_system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automatic_fcs_weapon: Option<String>,
    /// Explicit gateway/compatibility declaration, never inferred from names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compatible_protocols: Vec<String>,
    /// A peer may use a differently named fitted receiver terminal.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub peer_fits: BTreeMap<String, String>,
}
fn default_true() -> bool {
    true
}
fn is_true(value: &bool) -> bool {
    *value
}
fn is_false(value: &bool) -> bool {
    !*value
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    SendVisualSignal {
        board:super::detection2026::visual_signaling::Board,
        medium:super::detection2026::visual_signaling::Medium,
        purpose:super::detection2026::visual_signaling::Purpose,
        text:String,
        receivers:Vec<EntityId>,
    },
    SendWarning {
        contact: ContactId,
        sensor: String,
        fit: String,
        receivers: Vec<EntityId>,
    },
    SetAutomatic {
        fit: String,
        enabled: bool,
    },
    AbandonAlert {
        contact: ContactId,
    },
    SendSolution {
        contact: ContactId,
        sensor: String,
        fit: String,
        receivers: Vec<EntityId>,
        weapon: String,
        #[serde(default)]
        speed_index: Option<usize>,
    },
    SendAirSolution {
        contact: ContactId,
        sensor: String,
        fit: String,
        receivers: Vec<EntityId>,
    },
    CombineReports {
        contact: ContactId,
        #[serde(default)]
        own_sensor: Option<String>,
        messages: Vec<MessageRef>,
        #[serde(default)]
        aim_source: Option<MessageRef>,
    },
}

pub(crate) fn visual_request(action:&Action)->Option<super::detection2026::visual_signaling::Request>{
    match action {
        Action::SendVisualSignal{board,medium,purpose,text,receivers}=>Some(super::detection2026::visual_signaling::Request{
            board:*board,medium:*medium,purpose:*purpose,text:text.clone(),receivers:receivers.clone()
        }),
        _=>None,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Configuration,
    config_fingerprint: String,
    /// Private native observer/sensor/target key; never an observation field.
    reports: BTreeMap<String, NativeReport>,
    /// Sender-local sequences do not reveal the other side's message activity.
    sequences: BTreeMap<EntityId, u64>,
    messages: BTreeMap<String, Message>,
    #[serde(default, skip_serializing_if = "alerts::AlertState::is_empty")]
    alerts: alerts::AlertState,
    #[serde(default, skip_serializing_if = "exchange::ExchangeState::is_empty")]
    exchange: exchange::ExchangeState,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    automatic: BTreeMap<String, bool>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    auto_sent: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    scans: BTreeMap<String, NativeScan>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    disabled_causes: BTreeMap<EntityId, BTreeSet<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    source_deployments: BTreeMap<EntityId, u64>,
}
impl State {
    pub fn new(config: Configuration) -> Result<Self, String> {
        // Empty/default config still records native reports in ordinary Full
        // scenarios. Author/fit qualification belongs to action validation.
        links::validate_config(&config)?;
        let config_fingerprint = fingerprint(&config)?;
        Ok(Self {
            config,
            config_fingerprint,
            ..Default::default()
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct NativeReport {
    report: SensorReport,
    apparent_category: Option<PlatformKind>,
    source_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    air_altitude_m: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct NativeScan {
    at: i64,
    detected: bool,
}
/// Trusted typed scan hook, including unsuccessful/blocked native outcomes.
/// Called by the sensor batch after returning detection state to World.
pub fn record_scan_outcome(
    w: &mut World,
    observer: EntityId,
    sensor: &str,
    target: EntityId,
    at: i64,
    detected: bool,
) {
    if !active(w) || at != w.tick {
        return;
    }
    if let Some(s) = w.rules.communications.as_mut() {
        s.scans.insert(
            report_key(observer, sensor, target),
            NativeScan { at, detected },
        );
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Queued,
    Delivered,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Message {
    id: u64,
    side: Side,
    sender: EntityId,
    receivers: Vec<EntityId>,
    contact: ContactId,
    fit: String,
    protocol: String,
    queued_at: i64,
    tactical_turn: i64,
    config_fingerprint: String,
    snapshot: NativeReport,
    status: Status,
    terminal_at: Option<i64>,
    failure: Option<String>,
    #[serde(default, skip_serializing_if = "LinkKind::is_voice")]
    kind: LinkKind,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    receiver_kinds: BTreeMap<EntityId, LinkKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    solution: Option<FcsEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    air_solution: Option<AirFcsEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hf_emitter_position: Option<crate::Vec2>,
}

/// A real committed HF transmission, for the native ES producer. This is a
/// one-shot source event identified by reference, not an invented continuous
/// emission duration or an assumed radar-equivalent intercept range.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HfTransmission {
    pub reference: MessageRef,
    pub emitter: EntityId,
    pub position: crate::Vec2,
    pub at: i64,
    pub protocol: String,
}
pub fn hf_transmissions(w: &World) -> Vec<HfTransmission> {
    if !active(w) {
        return Vec::new();
    }
    w.rules
        .communications
        .as_ref()
        .map(|s| {
            s.messages
                .values()
                .filter_map(|m| {
                    m.hf_emitter_position.map(|position| HfTransmission {
                        reference: MessageRef {
                            sender: m.sender,
                            id: m.id,
                        },
                        emitter: m.sender,
                        position,
                        at: m.queued_at,
                        protocol: m.protocol.clone(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full
}
fn source_error() -> String {
    "6.3.10 communications source unavailable or changed".into()
}
fn authored(source: &str) -> bool {
    source == "scenario_assumption" || source.starts_with("scenario_assumption:")
}
fn fingerprint<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|_| source_error())?;
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3)
    });
    Ok(format!("fnv64:{hash:016x}"))
}
fn report_key(observer: EntityId, sensor: &str, target: EntityId) -> String {
    format!("{}:{sensor}:{}", observer.0, target.0)
}
fn message_key(sender: EntityId, sequence: u64) -> String {
    format!("{}:{sequence}", sender.0)
}
fn check_state(w: &World, state: &State) -> Result<(), String> {
    if !active(w) {
        return Err("6.3.10 source warnings require Full rules".into());
    }
    if fingerprint(&state.config)? != state.config_fingerprint {
        return Err(source_error());
    }
    let current = w.context.communications_2026.clone().unwrap_or_default();
    if fingerprint(&current)? != state.config_fingerprint {
        return Err(source_error());
    }
    Ok(())
}
fn owned(w: &World, side: Side, id: EntityId) -> Result<&crate::world::Entity, String> {
    w.entity(id)
        .filter(|e| e.side == side && e.alive)
        .ok_or_else(|| "6.3.10 owned surviving unit required".into())
}
fn unique_own_name(w: &World, side: Side, name: &str) -> bool {
    // Only own names are consulted; matching an enemy name cannot affect this.
    w.entities
        .values()
        .filter(|e| e.side == side && e.name == name)
        .count()
        == 1
}
fn fit<'a>(
    w: &World,
    config: &'a Configuration,
    side: Side,
    unit: EntityId,
    key: &str,
) -> Result<&'a VoiceFit, String> {
    let e = owned(w, side, unit)?;
    if w.rules
        .communications
        .as_ref()
        .and_then(|s| s.disabled_causes.get(&unit))
        .is_some_and(|causes| !causes.is_empty())
    {
        return Err("6.3 communication equipment disabled by an active source damage cause".into());
    }
    if key.trim().is_empty() || !unique_own_name(w, side, &e.name) || !authored(&config.source) {
        return Err(source_error());
    }
    let u = config
        .units
        .get(&e.name)
        .filter(|u| authored(&u.source))
        .ok_or_else(source_error)?;
    let f = u.fits
        .get(key)
        .filter(|f| authored(&f.source) && !f.protocol.trim().is_empty())
        .ok_or_else(source_error)?;
    annex::platform(w, unit, f)?;
    Ok(f)
}
fn direct_edges(
    w: &World,
    config: &Configuration,
    side: Side,
    sender: EntityId,
    key: &str,
    receivers: &[EntityId],
) -> Result<String, String> {
    let tx = fit(w, config, side, sender, key)?;
    if !tx.send || !tx.operational || receivers.is_empty() {
        return Err(source_error());
    }
    let mut seen = BTreeSet::new();
    for receiver in receivers {
        if *receiver == sender || !seen.insert(*receiver) {
            return Err("6.3.10 receivers must be distinct other own units".into());
        }
        let r = owned(w, side, *receiver)?;
        let rx_key = tx.peer_fits.get(&r.name).map(String::as_str).unwrap_or(key);
        let rx = fit(w, config, side, *receiver, rx_key)?;
        if !rx.receive
            || !rx.operational
            || !annex::compatible(tx, rx)?
            || !tx.peers.iter().any(|peer| peer == &r.name)
        {
            return Err(source_error());
        }
        links::route(w, sender, *receiver, tx, rx)?;
    }
    Ok(tx.protocol.clone())
}
fn receiver_fit_key(w: &World, config: &Configuration, side: Side, sender: EntityId, key: &str, receiver: EntityId) -> Result<String, String> {
    match direct_edges(w, config, side, sender, key, &[receiver]) {
        Ok(_) => {
            let tx = fit(w, config, side, sender, key)?;
            let name = &owned(w, side, receiver)?.name;
            Ok(tx.peer_fits.get(name).cloned().unwrap_or_else(|| key.to_string()))
        }
        Err(direct_error) => {
            let eligible = fit(w, config, side, sender, key).ok()
                .and_then(|tx| tx.annex_system.as_deref())
                .is_some_and(|id| relay::relay_enabled(id).is_ok());
            if !eligible { return Err(direct_error); }
            relay::find_path_with_fit(w, sender, receiver, key)
                .map(|(_, final_key)| final_key)
                .map_err(|_| direct_error)
        }
    }
}
fn edges(w: &World, config: &Configuration, side: Side, sender: EntityId, key: &str, receivers: &[EntityId]) -> Result<String, String> {
    let tx = fit(w, config, side, sender, key)?;
    if !tx.send || !tx.operational || receivers.is_empty() { return Err(source_error()); }
    let mut seen = BTreeSet::new();
    for receiver in receivers {
        if *receiver == sender || !seen.insert(*receiver) { return Err(source_error()); }
        owned(w, side, *receiver)?;
        receiver_fit_key(w, config, side, sender, key, *receiver)?;
    }
    Ok(tx.protocol.clone())
}
/// Pure transport qualification for non-contact traffic such as a GCI vector.
/// The caller must revalidate at actual delivery. This neither sends a warning
/// nor creates a Contact/FCS, and adds no inferred radio range or equipment.
pub fn validate_transport(
    w: &World,
    side: Side,
    sender: EntityId,
    fit_key: &str,
    receivers: &[EntityId],
) -> Result<String, String> {
    exchange::validate_control(w, side, sender)?;
    let state = w.rules.communications.as_ref().ok_or_else(source_error)?;
    edges(w, &state.config, side, sender, fit_key, receivers)
}
fn finite_geometry(report: &SensorReport) -> bool {
    let point = |p: crate::Vec2| p.x.is_finite() && p.y.is_finite();
    point(report.observer_position)
        && report.bearing_deg.is_finite()
        && (report.bearing_alternatives.is_empty() || report.bearing_alternatives.len() == 2)
        && report
            .bearing_alternatives
            .iter()
            .all(|bearing| bearing.is_finite())
        && report.measured_position.is_none_or(point)
        && report
            .bearing_error_deg
            .is_none_or(|n| n.is_finite() && n >= 0.)
}
fn report_source_fingerprint(w: &World, report: &SensorReport) -> Result<String, String> {
    // No target lookup: an unseen target moving, changing kind/HP or being
    // removed cannot alter warning validity or disclose its present existence.
    let observer = w.entity(report.observer).ok_or_else(source_error)?;
    let detection = w.rules.detection.as_ref().ok_or_else(source_error)?;
    let sensor = w.db.sensors.get(&report.sensor).ok_or_else(source_error)?;
    if sensor.kind != report.sensor_kind {
        return Err(source_error());
    }
    let receiver = detection
        .config
        .bindings
        .get(&format!("{}:{}", observer.class, report.sensor));
    if serde_json::to_value(receiver).map_err(|_| source_error())?
        != serde_json::to_value(&report.receiver).map_err(|_| source_error())?
    {
        return Err(source_error());
    }
    let mut source=json!({"observer_class":observer.class,"sensor":sensor,
        "receiver":receiver,"detection_config":detection.config,
        "rules_fingerprint":w.rules.table_fingerprint});
    if let Some(epoch)=w.rules.communications.as_ref().and_then(|s|s.source_deployments.get(&report.observer)) {
        source["source_deployment"]=json!(epoch);
    }
    fingerprint(&source)
}

/// A new physical deployment needs a new native source. Received historical
/// snapshots are retained, but cannot become live again when the same ID next
/// scans in the same Tactical epoch.
pub(crate) fn stow_platform(w:&mut World,id:EntityId) {
    if let Some(s)=w.rules.communications.as_mut() {
        let epoch=s.source_deployments.entry(id).or_default();*epoch=epoch.saturating_add(1);
        s.reports.retain(|_,r|r.report.observer!=id);
        s.scans.retain(|key,_|!key.starts_with(&format!("{}:",id.0)));
    }
}

/// Trusted native success hook. Root passes raw scan category copied before
/// fusion and calls here only after returning detection State to World.
pub fn record_detection(
    w: &mut World,
    report: &SensorReport,
    apparent_category: Option<PlatformKind>,
) {
    if !active(w) || report.at != w.tick || !finite_geometry(report) {
        return;
    }
    let Some(observer) = w.entity(report.observer).filter(|e| e.alive) else {
        return;
    };
    if !observer.sensors.iter().any(|s| s.sensor == report.sensor) {
        return;
    }
    let Some(state) = w.rules.communications.as_ref() else {
        return;
    };
    if check_state(w, state).is_err() {
        return;
    }
    let Ok(source_fingerprint) = report_source_fingerprint(w, report) else {
        return;
    };
    let key = report_key(report.observer, &report.sensor, report.target);
    // Sample this value only as part of the same actual native radar report
    // commit. A later message never looks up an unseen target's live altitude.
    let air_altitude_m = air::native_altitude(w, report, apparent_category);
    let state = w.rules.communications.as_mut().unwrap();
    if state
        .reports
        .get(&key)
        .is_some_and(|old| old.report.at > report.at)
    {
        return;
    }
    state.reports.insert(
        key,
        NativeReport {
            report: report.clone(),
            apparent_category,
            source_fingerprint,
            air_altitude_m,
        },
    );
    record_scan_outcome(
        w,
        report.observer,
        &report.sensor,
        report.target,
        report.at,
        true,
    );
}
/// UUV basic-data payload uses the same native provenance gate as normal messages.
pub fn native_report_view(
    w: &World,
    observer: EntityId,
    contact: ContactId,
    sensor: &str,
) -> Result<Value, String> {
    let e = w.entity(observer).ok_or_else(source_error)?;
    let s = w.rules.communications.as_ref().ok_or_else(source_error)?;
    check_state(w, s)?;
    let c = w.sides[e.side.index()]
        .contacts
        .get(&contact)
        .filter(|c| !c.stale)
        .ok_or_else(source_error)?;
    let r = s
        .reports
        .get(&report_key(observer, sensor, c.about))
        .ok_or_else(source_error)?;
    if report_source_fingerprint(w, &r.report)? != r.source_fingerprint {
        return Err(source_error());
    }
    Ok(report_view(r, contact))
}
/// Actual critical-hit/repair producers maintain independent causes. A repair
/// of one source cause cannot restore a radio disabled by another active cause.
pub fn set_communication_cause(
    w: &mut World,
    unit: EntityId,
    cause: &str,
    disabled: bool,
) -> Result<(), String> {
    if cause.trim().is_empty() {
        return Err("6.3 communication damage cause required".into());
    }
    let s = w.rules.communications.as_mut().ok_or_else(source_error)?;
    if disabled {
        s.disabled_causes
            .entry(unit)
            .or_default()
            .insert(cause.into());
    } else if let Some(causes) = s.disabled_causes.get_mut(&unit) {
        causes.remove(cause);
        if causes.is_empty() {
            s.disabled_causes.remove(&unit);
        }
    }
    Ok(())
}

/// Pure preparation is also used by apply; every receiver is checked before any
/// mutation. A held/fused CID alone never supplies another observer's report.
fn prepare(w: &World, side: Side, actor: EntityId, action: &Action) -> Result<Message, String> {
    if w.over || crate::exercise::locked(w, side) {
        return Err("6.3.10 warning commands unavailable after game end or exercise lock".into());
    }
    owned(w, side, actor)?;
    let state = w.rules.communications.as_ref().ok_or_else(source_error)?;
    check_state(w, state)?;
    let (contact, sensor, fit_key, receivers) = match action {
        Action::SendWarning {
            contact,
            sensor,
            fit,
            receivers,
        }
        | Action::SendSolution {
            contact,
            sensor,
            fit,
            receivers,
            ..
        }
        | Action::SendAirSolution {
            contact,
            sensor,
            fit,
            receivers,
        } => (contact, sensor, fit, receivers),
        _ => return Err("6.3 action is not a message".into()),
    };
    let c = w.sides[side.index()]
        .contacts
        .get(contact)
        .filter(|c| !c.stale)
        .ok_or("6.3.10 current own contact required")?;
    let snapshot = state
        .reports
        .get(&report_key(actor, sensor, c.about))
        .ok_or("6.3.10 selected observer/sensor has no native report")?;
    if snapshot.report.observer != actor
        || snapshot.report.target != c.about
        || snapshot.report.sensor != *sensor
        || snapshot.report.at > w.tick
        || report_source_fingerprint(w, &snapshot.report)? != snapshot.source_fingerprint
    {
        return Err(source_error());
    }
    let protocol = edges(w, &state.config, side, actor, fit_key, receivers)?;
    let tx = fit(w, &state.config, side, actor, fit_key)?;
    let receiver_kinds = if tx.warning_kind.is_voice() {
        BTreeMap::new()
    } else {
        receivers
            .iter()
            .map(|id| {
                let rx_key = receiver_fit_key(w, &state.config, side, actor, fit_key, *id)?;
                Ok((*id, links::effective(tx, fit(w, &state.config, side, *id, &rx_key)?)?))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?
    };
    let wants_air = matches!(action, Action::SendAirSolution { .. })
        || matches!(action, Action::SendSolution { weapon, .. }
            if w.db.weapons.get(weapon).is_some_and(|d| matches!(d.kind, crate::db::WeaponKind::Sam | crate::db::WeaponKind::Aam)));
    let air_solution = if wants_air {
        Some(air::prepare(w, actor, snapshot, tx, &receiver_kinds)?)
    } else {
        None
    };
    let solution = match action {
        Action::SendSolution {
            weapon,
            speed_index,
            ..
        } if !wants_air => Some(exchange::prepare_solution(
            w,
            actor,
            snapshot,
            tx,
            &receiver_kinds,
            weapon,
            *speed_index,
        )?),
        _ => None,
    };
    let id = state
        .sequences
        .get(&actor)
        .copied()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("6.3.10 local warning sequence exhausted")?;
    Ok(Message {
        id,
        side,
        sender: actor,
        receivers: receivers.clone(),
        contact: *contact,
        fit: fit_key.clone(),
        protocol,
        queued_at: w.tick,
        tactical_turn: w.tick.div_euclid(180),
        config_fingerprint: state.config_fingerprint.clone(),
        snapshot: snapshot.clone(),
        status: Status::Queued,
        terminal_at: None,
        failure: None,
        kind: tx.warning_kind,
        receiver_kinds,
        solution,
        air_solution,
        hf_emitter_position: matches!(tx.transport, Transport::Hf { .. })
            .then(|| w.entity(actor).unwrap().pos),
    })
}
pub fn validate_action(
    w: &World,
    side: Side,
    actor: EntityId,
    action: &Action,
) -> Result<(), String> {
    if let Some(request)=visual_request(action){return super::detection2026::visual_signaling::validate(w,side,actor,&request);}
    match action {
        Action::SetAutomatic { fit: key, .. } => {
            exchange::validate_control(w, side, actor)?;
            let state = w.rules.communications.as_ref().ok_or_else(source_error)?;
            let f = fit(w, &state.config, side, actor, key)?;
            if f.warning_kind.is_voice() || !f.send {
                return Err(
                    "6.3.9 automatic transmissions require a fitted transmitting TDL".into(),
                );
            }
            Ok(())
        }
        Action::AbandonAlert { contact } => {
            alerts::validate_abandon(w, side, actor, *contact).map(|_| ())
        }
        Action::CombineReports { .. } => {
            exchange::prepare_composition(w, side, actor, action).map(|_| ())
        }
        _ => prepare(w, side, actor, action).map(|_| ()),
    }
}
pub fn apply(w: &mut World, side: Side, actor: EntityId, action: &Action) -> Result<(), String> {
    if let Some(request)=visual_request(action){return super::detection2026::visual_signaling::queue(w,side,actor,request,false);}
    match action {
        Action::SetAutomatic { fit, enabled } => {
            validate_action(w, side, actor, action)?;
            w.rules
                .communications
                .as_mut()
                .unwrap()
                .automatic
                .insert(format!("{}:{fit}", actor.0), *enabled);
            emit(
                w,
                side,
                actor,
                "6.3.9.automatic",
                json!({"fit":fit,"enabled":enabled}),
            );
            return Ok(());
        }
        Action::AbandonAlert { contact } => return alerts::abandon(w, side, actor, *contact),
        Action::CombineReports { .. } => {
            return exchange::apply_composition(w, side, actor, action)
        }
        _ => {}
    }
    let message = prepare(w, side, actor, action)?;
    let state = w.rules.communications.as_mut().unwrap();
    let key = message_key(actor, message.id);
    if state.messages.contains_key(&key) {
        return Err(source_error());
    }
    state.sequences.insert(actor, message.id);
    state.messages.insert(key, message.clone());
    let detail = message_view(w, &message, "sent");
    emit(w, side, actor, "6.3.10.warning_queued", detail);
    Ok(())
}
fn delivery_check(w: &World, message: &Message) -> Result<(), String> {
    if w.tick != message.queued_at {
        return Err("execution_window_closed".into());
    }
    if w.over || crate::exercise::locked(w, message.side) {
        return Err(source_error());
    }
    let state = w.rules.communications.as_ref().ok_or_else(source_error)?;
    check_state(w, state)?;
    if state.config_fingerprint != message.config_fingerprint {
        return Err(source_error());
    }
    if edges(
        w,
        &state.config,
        message.side,
        message.sender,
        &message.fit,
        &message.receivers,
    )? != message.protocol
        || report_source_fingerprint(w, &message.snapshot.report)?
            != message.snapshot.source_fingerprint
    {
        return Err(source_error());
    }
    // No current-CID re-resolution, native report refresh, TTL or target query.
    Ok(())
}
fn emit(w: &mut World, side: Side, unit: EntityId, rule: &str, detail: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "communications2026".into(),
            rule: rule.into(),
            side: Some(side),
            unit: Some(unit),
            contact: None,
            detail: detail.to_string(),
        },
        Visibility::Side(side),
    );
}
/// Same execution-tick Detection-open terminalization, all receivers or none.
/// A missed execution window fails once; a terminal message never replays.
pub fn detection_open(w: &mut World) {
    if !active(w) {
        return;
    }
    exchange::enqueue_automatic(w, false);
    terminalize_pending(w);
}
/// Automatic TDL reports from this native batch are shared only after every
/// observer has committed its report, classification and own FCS evidence.
/// No receiver is rescanned and no trial already made in this batch is changed.
pub fn detection_close(w: &mut World) {
    if !active(w) {
        return;
    }
    exchange::enqueue_automatic(w, true);
    terminalize_pending(w);
}
fn terminalize_pending(w: &mut World) {
    let pending: Vec<_> = w
        .rules
        .communications
        .as_ref()
        .map(|s| {
            s.messages
                .values()
                .filter(|m| m.status == Status::Queued && m.queued_at <= w.tick)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    for mut message in pending {
        let result = delivery_check(w, &message);
        message.status = if result.is_ok() {
            Status::Delivered
        } else {
            Status::Failed
        };
        message.terminal_at = Some(w.tick);
        message.failure = result.err().map(|reason| {
            if reason == "execution_window_closed" {
                reason
            } else {
                "delivery_source_or_own_unit_unavailable".into()
            }
        });
        let Some(state) = w.rules.communications.as_mut() else {
            return;
        };
        let key = message_key(message.sender, message.id);
        let Some(current) = state.messages.get_mut(&key) else {
            continue;
        };
        if current.status != Status::Queued {
            continue;
        }
        *current = message.clone();
        let rule = if message.status == Status::Delivered {
            "6.3.10.warning_delivered"
        } else {
            "6.3.10.warning_failed"
        };
        let detail = message_view(w, &message, "sent");
        emit(w, message.side, message.sender, rule, detail);
        if message.status == Status::Delivered {
            alerts::delivered(w, &message);
            for receiver in &message.receivers {
                if let Some(evidence) = &message.air_solution {
                    crate::rules::operations::receive_rt_air_target(
                        w,
                        *receiver,
                        message.snapshot.report.target,
                        evidence,
                    );
                }
                let detail = message_view(w, &message, "received");
                emit(
                    w,
                    message.side,
                    *receiver,
                    "6.3.10.warning_received",
                    detail,
                );
            }
        }
    }
}

fn report_view(snapshot: &NativeReport, contact: ContactId) -> Value {
    let r = &snapshot.report;
    let mut view = json!({"contact":contact,"sensor":r.sensor,"observed_at":r.at,
        "observed_tactical_turn":r.at.div_euclid(180),"category":snapshot.apparent_category,
        "observer_position":r.observer_position,"bearing_deg":r.bearing_deg,
        "bearing_only":r.measured_position.is_none(),"measured_position":r.measured_position,
        "bearing_error_deg":r.bearing_error_deg});
    if !r.bearing_alternatives.is_empty() {
        view["bearing_alternatives"] = json!(r.bearing_alternatives);
    }
    view
}
fn message_view(w: &World, message: &Message, direction: &str) -> Value {
    let contact_current = w.sides[message.side.index()]
        .contacts
        .get(&message.contact)
        .is_some_and(|c| !c.stale && c.about == message.snapshot.report.target);
    let mut view = json!({"id":message.id,"sender":message.sender,"receivers":message.receivers,
        "fit":message.fit,"protocol":message.protocol,"queued_at":message.queued_at,
        "tactical_turn":message.tactical_turn,"state":message.status,
        "terminal_at":message.terminal_at,"failure":message.failure,"direction":direction,
        "contact_is_source_label":true,"report_is_live_track":false,"contact_current":contact_current,
        "report":report_view(&message.snapshot,message.contact)});
    if !message.kind.is_voice() {
        view["link_kind"] = json!(message.kind);
        view["contains_solution"] =
            json!(message.solution.is_some() || message.air_solution.is_some());
        if message.air_solution.is_some() {
            view["solution_domain"] = json!("air_native_geometry");
        }
    }
    view
}
/// Owner-unit projection only. No private target/key, complete Receiver/config,
/// FCS, quality, chance, failed-scan truth, remote actual position or new CID.
pub fn own_view(w: &World, unit: EntityId) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let e = w.entity(unit).filter(|e| e.alive)?;
    let side = e.side;
    let state = w.rules.communications.as_ref()?;
    let valid = check_state(w, state).is_ok();
    let mut choices = Vec::new();
    if valid {
        for snapshot in state.reports.values().filter(|r| r.report.observer == unit) {
            if report_source_fingerprint(w, &snapshot.report).ok().as_ref()
                != Some(&snapshot.source_fingerprint)
            {
                continue;
            }
            for c in w.sides[side.index()]
                .contacts
                .values()
                .filter(|c| !c.stale && c.about == snapshot.report.target)
            {
                choices.push(report_view(snapshot, c.id));
            }
        }
    }
    // Stable public ordering must not expose the private report-key ordering.
    choices.sort_by_key(|v| {
        (
            v["contact"].as_u64().unwrap_or(0),
            v["sensor"].as_str().unwrap_or("").to_string(),
            v["observed_at"].as_i64().unwrap_or(0),
        )
    });
    let mut fits = Vec::new();
    if valid {
        if let Some(u) = state.config.units.get(&e.name) {
            for key in u.fits.keys() {
                let Ok(voice) = fit(w, &state.config, side, unit, key) else {
                    continue;
                };
                let receivers: Vec<_> = w
                    .entities
                    .values()
                    .filter(|r| r.side == side && r.alive && r.id != unit)
                    .filter(|r| edges(w, &state.config, side, unit, key, &[r.id]).is_ok())
                    .map(|r| r.id)
                    .collect();
                let mut v = json!({"fit":key,"protocol":voice.protocol,"send":voice.send,
                    "receive":voice.receive,"operational":voice.operational,"receivers":receivers});
                if let Some(system) = &voice.annex_system {
                    v["annex_system"] = json!(system);
                    v["source_qualification"] = json!("Annex L terminal and platform class fit; geometry/topology are declared scenario inputs");
                }
                if !voice.warning_kind.is_voice() {
                    v["link_kind"] = json!(voice.warning_kind);
                    v["automatic"] = json!(state
                        .automatic
                        .get(&format!("{}:{key}", unit.0))
                        .copied()
                        .unwrap_or(voice.automatic_warning));
                    v["targeting_system"] = json!(voice.targeting_system);
                }
                fits.push(v);
            }
        }
    }
    let messages: Vec<_> = state
        .messages
        .values()
        .filter(|m| m.side == side)
        .filter_map(|m| {
            if m.sender == unit {
                Some(message_view(w, m, "sent"))
            } else if m.status == Status::Delivered && m.receivers.contains(&unit) {
                Some(message_view(w, m, "received"))
            } else {
                None
            }
        })
        .collect();
    if choices.is_empty() && fits.is_empty() && messages.is_empty() {
        return None;
    }
    let extended = state
        .config
        .units
        .get(&e.name)
        .is_some_and(|u| u.fits.values().any(|f| !f.warning_kind.is_voice()));
    let mut view = json!({"scope":if extended{"fitted_communications"}else{"voice_warning_only"},"fits":fits,"reports":choices,"messages":messages});
    if extended {
        view["alerts"] = alerts::own_alerts(w, unit);
        view["solutions"] = exchange::own_solutions(w, unit);
    }
    Some(view)
}

//!5.8.11: actual optical non-tactical signals, independent of RF/TDL/sensing.
use crate::types::{DayNight, PlatformKind};
use crate::{EntityId, EventKind, Side, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
pub fn tables() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/visual-signals-2026.json"
        ))
        .expect("verified original visual signal table/footnote")
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Board {
    Port,
    Starboard,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Medium {
    FlagHoist,
    FlashingLight,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Formation,
    Maneuvering,
    Administrative,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub board: Board,
    pub medium: Medium,
    pub purpose: Purpose,
    pub text: String,
    pub receivers: Vec<EntityId>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitFit {
    pub source: String,
    pub installed: bool,
    pub receive: BTreeSet<Medium>,
    pub send: BTreeMap<Board, BTreeSet<Medium>>,
    pub height_m: f64,
    pub height_source: String,
    pub los_source: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub units: BTreeMap<String, UnitFit>,
    pub character_policy: String,
    pub phase_policy: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundUnit {
    pub unit: EntityId,
    pub class: String,
    pub side: Side,
    pub name: String,
    pub fit: UnitFit,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Draft {
    pub sender: EntityId,
    pub side: Side,
    pub request: Request,
    pub queued_at: i64,
    pub retain_manual: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub at: i64,
    pub receiver: EntityId,
    pub range_kyd: f64,
    pub permitted_kyd: f64,
    pub los_source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub sender: EntityId,
    pub sender_name: String,
    pub side: Side,
    pub sender_class: String,
    pub request: Request,
    pub sent_at: i64,
    pub source_turn: i64,
    pub processed: bool,
    pub received: BTreeMap<EntityId, Receipt>,
    pub referee_dispositions: BTreeMap<EntityId, String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub config: Configuration,
    pub units: BTreeMap<EntityId, BoundUnit>,
    pub drafts: BTreeMap<String, Vec<Draft>>,
    pub used: BTreeMap<String, String>,
    pub sequences: BTreeMap<EntityId, u64>,
    pub messages: BTreeMap<String, Message>,
    pub outbound: BTreeSet<String>,
    pub closed_at: Option<i64>,
}
fn runtime(w: &World) -> Option<&State> {
    w.rules.detection.as_ref()?.visual_signaling.as_ref()
}
fn take(w: &mut World) -> Option<State> {
    w.rules.detection.as_mut()?.visual_signaling.take()
}
fn put(w: &mut World, s: State) {
    if let Some(d) = w.rules.detection.as_mut() {
        d.visual_signaling = Some(s);
    }
}
fn full(w: &World) -> bool {
    w.rules.profile == crate::rules::damage_runtime::RuleProfile::Rts2026Full
}
fn board(b: Board) -> &'static str {
    match b {
        Board::Port => "port",
        Board::Starboard => "starboard",
    }
}
fn turn(w: &World) -> i64 {
    w.tick
        .div_euclid(tables()["turn_seconds"].as_i64().unwrap())
}
fn slot(sender: EntityId, b: Board, at: i64) -> String {
    format!("{}:{}:{at}", sender.0, board(b))
}
fn event(
    w: &mut World,
    unit: EntityId,
    side: Side,
    rule: &str,
    value: Value,
    visibility: Visibility,
) {
    w.emit(
        EventKind::RulesEvent {
            domain: "visual_signaling".into(),
            rule: rule.into(),
            side: Some(side),
            unit: Some(unit),
            contact: None,
            detail: value.to_string(),
        },
        visibility,
    );
}
pub fn initialize(w: &World, config: Option<&Configuration>) -> Result<Option<State>, String> {
    let Some(c) = config else {
        return Ok(None);
    };
    if c.source.trim().is_empty()
        || c.character_policy != "ascii_only_first_scope"
        || c.phase_policy != "same_tick_normal_detection"
    {
        return Err(
            "5.8.11 explicit source/ASCII/same-tick normal optical phase policy required".into(),
        );
    }
    let mut s = State {
        config: c.clone(),
        ..Default::default()
    };
    for (name, f) in &c.units {
        let bodies: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
        if bodies.len() != 1 {
            return Err("5.8.11 exact named optical station body must resolve uniquely".into());
        }
        let e = bodies[0];
        if e.kind != PlatformKind::Ship
            || !f.source.starts_with("scenario_assumption:")
            || !f.height_source.starts_with("scenario_assumption:")
            || !f.los_source.starts_with("scenario_assumption:")
            || !f.height_m.is_finite()
            || f.height_m <= 0.
            || f.height_source.trim().is_empty()
            || f.los_source.trim().is_empty()
            || f.send.values().any(|m| m.is_empty())
        {
            return Err(
                "5.8.11 actual ship and explicit scenario_assumption station/height/clear-ray qualification required; real Annex fit is not inferred"
                    .into(),
            );
        }
        s.units.insert(
            e.id,
            BoundUnit {
                unit: e.id,
                class: e.class.clone(),
                side: e.side,
                name: e.name.clone(),
                fit: f.clone(),
            },
        );
    }
    Ok(Some(s))
}
fn bound<'a>(w: &World, s: &'a State, id: EntityId, side: Side) -> Result<&'a BoundUnit, String> {
    let b = s
        .units
        .get(&id)
        .ok_or("5.8.11 actual fitted optical station absent")?;
    let e = w.entity(id).ok_or("5.8.11 actual optical body absent")?;
    if !e.alive
        || e.kind != PlatformKind::Ship
        || e.side != side
        || e.side != b.side
        || e.class != b.class
        || e.name != b.name
        || !b.fit.installed
    {
        return Err("5.8.11 source-bound own live optical body changed/unavailable".into());
    }
    Ok(b)
}
fn health(w: &World, id: EntityId, station: &str, medium: Medium) -> Result<(), String> {
    let e = w.entity(id).ok_or("5.8.11 optical body absent")?;
    for key in [
        format!("native.visual_signal:{station}"),
        format!("native.sensor:visual_signal:{station}"),
    ] {
        if !crate::rules::expanded_damage2026::equipment_operational(w, id, &key)? {
            return Err("5.8.11 actual optical station disabled".into());
        }
    }
    if crate::sensors::sensor_damaged(e, &format!("visual_signal:{station}")) {
        return Err("5.8.11 explicit optical station damage binding disabled".into());
    }
    if e.ext.get("damage.personnel").is_some_and(|v| *v > 0.) {
        return Err("5.8.11 remaining signal personnel applicability source pending".into());
    }
    if medium == Medium::FlashingLight
        && e.ext
            .get("damage.power_lost")
            .is_some_and(|v| *v > w.tick as f64)
    {
        return Err("5.8.11 lamp independent backup power source pending".into());
    }
    Ok(())
}
fn permitted(w: &World, s: &State, medium: Medium) -> Result<f64, String> {
    let env = w
        .rules
        .detection
        .as_ref()
        .and_then(|d| d.config.environment.as_ref())
        .ok_or("5.8.11 explicit optical environment absent")?;
    // The child can be temporarily taken, but the normal detection configuration
    // remains authoritative and is never replaced by a radio weather estimate.
    let _ = s;
    let row = tables()["range_rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["visibility_percent"].as_u64() == Some(u64::from(env.visibility_percent)))
        .ok_or("5.8.11 visibility outside printed source rows")?;
    let key = match (w.day_night, medium) {
        (DayNight::Night, Medium::FlagHoist) => {
            return Err("5.8.11 flag hoist requires daylight".into())
        }
        (DayNight::Day, Medium::FlagHoist) => "flag_day_kyd",
        (DayNight::Day, Medium::FlashingLight) => "flash_day_kyd",
        (DayNight::Night, Medium::FlashingLight) if env.precipitation == 0 => {
            "flash_night_clear_kyd"
        }
        (DayNight::Night, Medium::FlashingLight) => "flash_night_precip_kyd",
    };
    row[key].as_f64().ok_or_else(|| {
        "5.8.11 unprinted/ambiguous night cell source pending; no blanket20kyd fallback".into()
    })
}
fn prepare(w: &World, s: &State, side: Side, sender: EntityId, r: &Request) -> Result<(), String> {
    if !full(w) {
        return Err("5.8.11 source optical signaling requires Full".into());
    }
    crate::rules::unmanned2026::can_control(w, side, sender)?;
    let b = bound(w, s, sender, side)?;
    if !b
        .fit
        .send
        .get(&r.board)
        .is_some_and(|m| m.contains(&r.medium))
    {
        return Err("5.8.11 actual sender-side optical medium not installed".into());
    }
    if r.text.is_empty()
        || !r.text.is_ascii()
        || !r.text.bytes().all(|c| (32..=126).contains(&c))
        || r.text.chars().count() > tables()["maximum_characters"].as_u64().unwrap() as usize
    {
        return Err("5.8.11 one printable ASCII signal of1..30characters required; Unicode counting source pending".into());
    }
    if r.receivers.is_empty() {
        return Err("5.8.11 addressed friendly optical recipients required".into());
    }
    if s.used.contains_key(&slot(sender, r.board, turn(w))) {
        return Err(
            "5.8.11 sender side already transmitted its one signal this TacticalTurn".into(),
        );
    }
    health(w, sender, board(r.board), r.medium)?;
    permitted(w, s, r.medium)?;
    for id in &r.receivers {
        if *id == sender {
            return Err(
                "5.8.11 this source scope sends to other actual friendly optical stations".into(),
            );
        }
        let receiver = bound(w, s, *id, side)?;
        if !receiver.fit.receive.contains(&r.medium) {
            return Err("5.8.11 recipient lacks this actual optical reader fit".into());
        }
    }
    Ok(())
}
pub fn validate(w: &World, side: Side, sender: EntityId, r: &Request) -> Result<(), String> {
    let s = runtime(w).ok_or("5.8.11 native optical source configuration absent")?;
    prepare(w, s, side, sender, r)
}
pub fn queue(
    w: &mut World,
    side: Side,
    sender: EntityId,
    mut request: Request,
    retain_manual: bool,
) -> Result<(), String> {
    validate(w, side, sender, &request)?;
    let at = w.tick;
    if runtime(w).is_some_and(|s| s.closed_at == Some(at)) {
        return Err("5.8.11 normal optical declaration barrier already closed this tick".into());
    }
    request.receivers.sort();
    request.receivers.dedup();
    let key = format!("{}:{}", sender.0, board(request.board));
    runtime_mut(w)?.drafts.entry(key).or_default().push(Draft {
        sender,
        side,
        request,
        queued_at: at,
        retain_manual,
    });
    Ok(())
}
fn runtime_mut(w: &mut World) -> Result<&mut State, String> {
    w.rules
        .detection
        .as_mut()
        .and_then(|d| d.visual_signaling.as_mut())
        .ok_or_else(|| "5.8.11 native optical state absent".into())
}
/// One native declaration barrier. No input order decides conflicting texts.
/// Only actual source transmissions return trusted player manual commits.
pub fn begin_detection(w: &mut World) -> Vec<EntityId> {
    if !full(w) {
        return vec![];
    }
    let Some(mut s) = take(w) else {
        return vec![];
    };
    if s.closed_at == Some(w.tick) {
        put(w, s);
        return vec![];
    }
    s.closed_at = Some(w.tick);
    let drafts = std::mem::take(&mut s.drafts);
    let mut manual = BTreeSet::new();
    for (_, group) in drafts {
        let Some(first) = group.first() else {
            continue;
        };
        let first = first.clone();
        if group
            .iter()
            .any(|d| d.sender != first.sender || d.side != first.side || d.request != first.request)
        {
            event(
                w,
                first.sender,
                first.side,
                "5.8.11.visual_signal_refused",
                json!({"board":first.request.board,"reason":"conflicting same-board optical requests refused together","transmitted":false}),
                Visibility::Side(first.side),
            );
            continue;
        }
        if let Err(reason) = prepare(w, &s, first.side, first.sender, &first.request) {
            event(
                w,
                first.sender,
                first.side,
                "5.8.11.visual_signal_refused",
                json!({"board":first.request.board,"reason":reason,"transmitted":false}),
                Visibility::Side(first.side),
            );
            continue;
        }
        let seq = s.sequences.entry(first.sender).or_default();
        *seq += 1;
        let id = format!("visual:{}:{}", first.sender.0, *seq);
        let name = w.entity(first.sender).unwrap().name.clone();
        let source_turn = turn(w);
        s.used.insert(
            slot(first.sender, first.request.board, source_turn),
            id.clone(),
        );
        s.messages.insert(
            id.clone(),
            Message {
                id: id.clone(),
                sender: first.sender,
                sender_name: name,
                side: first.side,
                sender_class: w.entity(first.sender).unwrap().class.clone(),
                request: first.request.clone(),
                sent_at: w.tick,
                source_turn,
                processed: false,
                received: BTreeMap::new(),
                referee_dispositions: BTreeMap::new(),
            },
        );
        s.outbound.insert(id.clone());
        if group.iter().any(|d| d.retain_manual) {
            manual.insert(first.sender);
        }
        event(
            w,
            first.sender,
            first.side,
            "5.8.11.visual_signal_transmitted",
            json!({"id":id,"board":first.request.board,"medium":first.request.medium,"purpose":first.request.purpose,"text":first.request.text,"at":w.tick,"source_turn":source_turn,"rf_transport":false}),
            Visibility::Side(first.side),
        );
    }
    put(w, s);
    manual.into_iter().collect()
}
fn reception(w: &World, s: &State, m: &Message, receiver: EntityId) -> Result<Receipt, String> {
    let sender = bound(w, s, m.sender, m.side)?;
    let r = bound(w, s, receiver, m.side)?;
    if sender.class != m.sender_class || !r.fit.receive.contains(&m.request.medium) {
        return Err("5.8.11 stored optical body/reader binding changed".into());
    }
    health(w, m.sender, board(m.request.board), m.request.medium)?;
    health(w, receiver, "receive", m.request.medium)?;
    let a = w.entity(m.sender).unwrap();
    let b = w.entity(receiver).unwrap();
    let range = a.pos.range_to(b.pos) * tables()["conversion"]["yards_per_nm"].as_f64().unwrap()
        / tables()["conversion"]["yards_per_kyd"].as_f64().unwrap();
    if !range.is_finite() || range <= 0. {
        return Err("5.8.11 nonfinite/collocated optical scene geometry pending".into());
    }
    let permitted = permitted(w, s, m.request.medium)?;
    if range > permitted {
        return Err(
            "5.8.11 actual optical range outside printed visibility/medium/night cell".into(),
        );
    }
    if w.context
        .terrain
        .as_ref()
        .is_some_and(|c| c.blocked(a.pos, b.pos))
    {
        return Err(
            "5.8.11 terrain intersects optical ray; vertical clearance source pending".into(),
        );
    }
    if w.rules
        .detection
        .as_ref()
        .and_then(|d| d.config.environment.as_ref())
        .is_some_and(|e| e.cloud.is_some())
    {
        return Err("5.8.11 cloud-to-surface optical clearance source pending".into());
    }
    // LOS is a declared source-author clear native ray, not a radar-refraction
    // horizon or a guessed real ship aperture. Heights were explicitly validated.
    Ok(Receipt {
        at: w.tick,
        receiver,
        range_kyd: range,
        permitted_kyd: permitted,
        los_source: format!("{}; {}", sender.fit.los_source, r.fit.los_source),
    })
}
/// A distinct optical mailbox, never voice warning, sensor report or FCS.
pub fn deliver(w: &mut World) {
    if !full(w) {
        return;
    }
    let Some(mut s) = take(w) else {
        return;
    };
    let outbound = std::mem::take(&mut s.outbound);
    for id in outbound {
        let Some(mut m) = s.messages.get(&id).cloned() else {
            continue;
        };
        if m.processed {
            continue;
        }
        for receiver in &m.request.receivers {
            let result = if m.sent_at != w.tick {
                Err("5.8.11 delayed midphase restore delivery source pending; no later-world resampling".into())
            } else {
                reception(w, &s, &m, *receiver)
            };
            match result {
                Ok(receipt) => {
                    m.received.insert(*receiver, receipt);
                    event(
                        w,
                        *receiver,
                        m.side,
                        "5.8.11.visual_signal_received",
                        json!({"id":m.id,"sender":m.sender,"sender_name":m.sender_name,"board":m.request.board,"medium":m.request.medium,"purpose":m.request.purpose,"text":m.request.text,"sent_at":m.sent_at,"received_at":w.tick,"rf_transport":false,"sensing_report":false}),
                        Visibility::Side(m.side),
                    );
                }
                Err(reason) => {
                    m.referee_dispositions.insert(*receiver, reason.clone());
                    event(
                        w,
                        m.sender,
                        m.side,
                        "5.8.11.optical_reception_not_adjudicated",
                        json!({"id":m.id,"receiver":receiver,"reason":reason,"quota_refunded":false}),
                        Visibility::Referee,
                    );
                }
            }
        }
        m.processed = true;
        s.messages.insert(id, m);
    }
    put(w, s);
}
pub fn own_view(w: &World, side: Side, unit: EntityId) -> Value {
    let Some(s) = runtime(w) else {
        return Value::Null;
    };
    if bound(w, s, unit, side).is_err() {
        return Value::Null;
    }
    let messages:Vec<_>=s.messages.values().filter(|m|m.side==side).filter_map(|m|{
  let direction=if m.sender==unit{"sent"}else if m.received.contains_key(&unit){"received"}else{return None;};
  Some(json!({"id":m.id,"direction":direction,"sender":m.sender,"sender_name":m.sender_name,"board":m.request.board,"medium":m.request.medium,"purpose":m.request.purpose,"text":m.request.text,"sent_at":m.sent_at,"received_at":m.received.get(&unit).map(|r|r.at)}))
 }).collect();
    let windows:Vec<_>=[Board::Port,Board::Starboard].iter().map(|b|json!({"board":b,"source_turn":turn(w),"used":s.used.contains_key(&slot(unit,*b,turn(w)))})).collect();
    json!({"scope":"native_non_tactical_optical","messages":messages,"windows":windows,"rf_transport":false})
}

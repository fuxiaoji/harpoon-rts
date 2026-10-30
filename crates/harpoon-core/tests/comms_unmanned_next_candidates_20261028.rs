//! ZCODE-COMMS-UNMANNED-NEXT-02 behavior cases, revised per root review
//! (strict current-sending-tick currency; public view without enemy
//! identity). Source-only candidate; root registers and runs. Never includes
//! existing test modules; support code is copied as text.
//!
//! Requires the root-owned parent one-liner in communications2026.rs
//! (`pub mod hf_es;`), the Context field and the root-owned detection2026
//! Receiver::Es integration; after that every case compiles against public
//! JSON/API only.
//!
//! Source: English2026 PDF74/printed 6-6 §6.3.9 (visually verified,
//! source-pages/hf-page-074.png): "Beyond LOS is the range for a TDL's high
//! frequency (HF) radio transmissions that can be detected by ES systems."
use harpoon_core::rules::communications2026::{hf_es, MessageRef};
use harpoon_core::rules::communications2026 as c;
use harpoon_core::{
    Command, ContactId, Database, EntityId, Scenario, Side, World,
};
use serde_json::{json, Value};

const P: &str = "batch_lab_passive";
const HF_PROTOCOL: &str = "batch-tdl";
const HF: &str = "hf";
const ES_SENSOR: &str = "batch_lab_es_sensor";

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn run(w: &mut World, to: i64) {
    while w.tick < to {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
    assert_eq!(w.tick, to);
}
fn database() -> Database {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut p = db.sensors.values().next().unwrap().clone();
    p.id = P.into();
    p.name = format!("scenario_assumption:{P}");
    p.kind = harpoon_core::db::SensorKind::SonarPassive;
    p.air_range_nm = 0.;
    p.surface_range_nm = 0.;
    p.missile_range_nm = 0.;
    p.sub_range_nm = 4.;
    db.sensors.insert(p.id.clone(), p);
    let mut es = db.sensors.values().next().unwrap().clone();
    es.id = ES_SENSOR.into();
    es.name = "scenario_assumption: batch ES listener".into();
    es.kind = harpoon_core::db::SensorKind::Esm;
    es.air_range_nm = 100.;
    es.surface_range_nm = 100.;
    es.missile_range_nm = 0.;
    es.sub_range_nm = 0.;
    db.sensors.insert(es.id.clone(), es);
    let mut sender = db.platforms["us_burke_2a"].clone();
    sender.id = "batch_lab_sender".into();
    sender.name = "scenario_assumption: original hull/stock laboratory sender".into();
    sender.sensors = vec![P.into()];
    db.platforms.insert(sender.id.clone(), sender);
    let mut rx = db.platforms["us_burke_2a"].clone();
    rx.id = "batch_lab_receiver".into();
    rx.name = "scenario_assumption: original hull/stock laboratory receiver".into();
    rx.sensors = Vec::new();
    db.platforms.insert(rx.id.clone(), rx);
    let mut target = db.platforms["cn_093"].clone();
    target.id = "batch_lab_target".into();
    target.sensors.clear();
    db.platforms.insert(target.id.clone(), target);
    let mut listener = db.platforms["us_burke_2a"].clone();
    listener.id = "batch_lab_es".into();
    listener.name = "scenario_assumption: original hull laboratory ES listener".into();
    listener.sensors = vec![ES_SENSOR.into()];
    db.platforms.insert(listener.id.clone(), listener);
    db.validate().unwrap();
    db
}
fn environment() -> Value {
    json!({"source":"scenario_assumption: explicit open-water experiment","sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null})
}
fn passive_sonar(range: f64) -> Value {
    json!({"type":"sonar","source":"scenario_assumption: Gen5 Bow LMF experimental sonar","generation":5,"passive_range_nm":range,"active_range_nm":null,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false})
}
fn es_receiver() -> Value {
    json!({"type":"es","source":"scenario_assumption: explicit experimental ES fit; no real Annex ES claim","generation":5,"targeting_complex":false,"rwr":false,"bearing_error_deg":2.,"els":false})
}
/// transport is the only degree of freedom: hf vs line_of_sight.
fn comm_config(transport: Value) -> Value {
    let make = |peers: Vec<&str>| json!({"source":"scenario_assumption: laboratory link, no claim of actual Annex fitting","protocol":HF_PROTOCOL,"send":true,"receive":true,"operational":true,"peers":peers,"warning_kind":"near_real_time","automatic_warning":false,"targeting_system":false,"transport":transport});
    json!({"source":"scenario_assumption: explicit experiment","units":{
        "SENDER":{"source":"scenario_assumption: original-body test","fits":{"lab":make(vec!["RECEIVER"])}},
        "RECEIVER":{"source":"scenario_assumption: original-body test","fits":{"lab":make(vec![])}}
    }})
}
fn hf_world(seed: u64, transport: Value) -> World {
    let bindings = [
        (format!("batch_lab_sender:{P}"), passive_sonar(4.)),
        (format!("batch_lab_es:{ES_SENSOR}"), es_receiver()),
    ]
    .into_iter()
    .collect::<std::collections::BTreeMap<_, _>>();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"comms-unmanned-next-hf-es",
        "name":"scenario_assumption: laboratory sensors/links only, original DP and stocks",
        "duration_s":10000,"seed":seed,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":2,"precipitation":"None"},
            "communications_2026":comm_config(transport),
            "expanded_damage_2026":{"source":"scenario_assumption: one explicit radio room for a damage-consumer boundary","units":{"SENDER":{"source":"scenario_assumption: author radio room","components":{"radio":{"source":"scenario_assumption: declared short radio room","kind":"communications","cas_group":"short_range"}}}}},
            "hf_es_2026":serde_json::to_value(hf_es_config()).unwrap(),
            "detection_2026":{"bindings":bindings,"environment":environment(),
                "targets":{"batch_lab_target":{"source":"scenario_assumption: controlled acoustic signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
        "units":[
            {"side":"Blue","class":"batch_lab_sender","name":"SENDER","pos":[0,0],"course_deg":90,"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":"batch_lab_receiver","name":"RECEIVER","pos":[0,-1],"course_deg":90,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"batch_lab_es","name":"ES_LISTENER","pos":[3,0],"course_deg":90,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"batch_lab_target","name":"TARGET","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}
        ]}))
    .unwrap();
    harpoon_core::load_scenario(sc, database()).unwrap()
}
/// Seed-search until the sender owns a real native passive report and a
/// current contact about the same target. No report/contact is ever injected.
fn hf_fixture(transport: Value) -> (World, EntityId, ContactId) {
    for seed in 1..=256u64 {
        let mut w = hf_world(seed, transport.clone());
        run(&mut w, 181);
        let sender = id(&w, "SENDER");
        let target = id(&w, "TARGET");
        let Some(contact) = w.sides[0]
            .contacts
            .values()
            .find(|c| !c.stale && c.about == target)
            .map(|c| c.id)
        else {
            continue;
        };
        let Some(view) = c::own_view(&w, sender) else {
            continue;
        };
        let contact_value = serde_json::to_value(contact).unwrap();
        let has_report = view["reports"].as_array().is_some_and(|reports| {
            reports
                .iter()
                .any(|r| r["sensor"] == P && r["contact"] == contact_value)
        });
        if has_report {
            return (w, sender, contact);
        }
    }
    panic!("native source/contact fixture unavailable: fixture failure, never an accepted rule red")
}
fn send(w: &mut World, actor: EntityId, action: Value) {
    let typed: c::Action = serde_json::from_value(action.clone()).unwrap();
    c::validate_action(w, Side::Blue, actor, &typed)
        .expect("actual source-qualified communication checkpoint before queued execution");
    let cmd: Command =
        serde_json::from_value(json!({"cmd":"communications","unit":actor,"action":action}))
            .unwrap();
    harpoon_core::sim::tick(w, &[(Side::Blue, cmd)]);
}
fn warning(contact: ContactId, receiver: EntityId) -> Value {
    json!({"type":"send_warning","contact":contact,"sensor":P,"fit":"lab","receivers":[receiver]})
}
fn hf_es_config() -> hf_es::Configuration {
    serde_json::from_value(json!({
        "source":"scenario_assumption: explicit batch HF-ES experiment",
        "receivers":{"ES_LISTENER":{"source":"scenario_assumption: explicit experimental HF-ES receiver; no real Annex ES hardware HF claim","sensor":ES_SENSOR,"range_nm":60.0,"bearing_error_deg":2.0}}
    }))
    .unwrap()
}

#[test]
fn hf_es_next01_declared_receiver_sample_is_bearing_only_and_deterministic() {
    let (mut w, sender, contact) = hf_fixture(json!({"type":HF,"max_range_nm":500.}));
    let receiver = id(&w, "RECEIVER");
    send(&mut w, sender, warning(contact, receiver));
    // tick advances before command execution: after send, w.tick == queued_at.
    let cfg = hf_es_config();
    let listener = id(&w, "ES_LISTENER");
    let sources = hf_es::es_sources(&w);
    assert_eq!(sources.len(), 1, "exactly the committed TDL HF transmission");
    assert_eq!(sources[0].emitter, sender);
    assert_eq!(sources[0].at, w.tick, "live only at the actual sending tick");
    let first = hf_es::sample(&w, &cfg, listener).unwrap();
    assert_eq!(first.len(), 1, "deterministic in-range detection");
    let b = &first[0];
    assert_eq!(b.emitter, sender);
    assert_eq!(b.sensor, ES_SENSOR);
    let lp = w.entity(listener).unwrap().pos;
    let sp = w.entity(sender).unwrap().pos;
    let expected = harpoon_core::geom::normalize_deg(lp.bearing_to(sp));
    assert!((b.bearing_deg - expected).abs() < 1e-9, "true bearing only");
    assert_eq!(b.bearing_error_deg, 2.0);
    let MessageRef { sender: rs, id: rid } = b.reference;
    assert_eq!(rs, sender);
    assert_eq!(rid, sources[0].reference.id);
    // Deterministic: resampling never rerolls anything.
    let again = hf_es::sample(&w, &cfg, listener).unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&again).unwrap()
    );
    // No default hardware: an undeclared unit has no HF-ES capability.
    assert!(hf_es::sample(&w, &cfg, receiver).unwrap().is_empty());
    // Structured sample carries the parent/referee attribution fields.
    let v = serde_json::to_value(&first[0]).unwrap();
    assert!(v.get("position").is_none());
    assert!(v.get("measured_position").is_none());
    assert!(v.get("quality").is_none());
    assert!(v.get("category").is_none());
    // The PUBLIC view must not publish enemy identity or private references.
    let view = hf_es::own_view(&w, &cfg, listener).unwrap();
    assert_eq!(view["bearing_only"], json!(true));
    let pb = &view["bearings"][0];
    assert_eq!(pb["sensor"], json!(ES_SENSOR));
    assert_eq!(pb["bearing_error_deg"], json!(2.0));
    assert!(pb.get("emitter").is_none(), "no enemy emitter id in public view");
    assert!(pb.get("reference").is_none(), "no message reference in public view");
    assert!(pb.get("position").is_none(), "no enemy position in public view");
    assert!(pb["bearing_deg"].is_number());
    assert!(pb["observed_at"].is_number());
    // An undeclared unit has no public HF-ES view at all.
    assert!(hf_es::own_view(&w, &cfg, receiver).is_none());
}

#[test]
fn hf_es_next02_los_transport_never_becomes_an_es_source() {
    let (mut w_los, sender_los, contact_los) =
        hf_fixture(json!({"type":"line_of_sight","max_range_nm":500.,"antenna_height_m":20.}));
    let receiver_los = id(&w_los, "RECEIVER");
    send(&mut w_los, sender_los, warning(contact_los, receiver_los));
    assert!(hf_es::es_sources(&w_los).is_empty());
    assert!(hf_es::sample(&w_los, &hf_es_config(), id(&w_los, "ES_LISTENER"))
        .unwrap()
        .is_empty());
}

#[test]
fn hf_es_next03_late_receiver_never_detects_a_historical_pulse() {
    // Strict currency: the message ledger keeps history, but the ES source
    // exists only at the actual sending tick. One second later it is gone.
    let (mut w, sender, contact) = hf_fixture(json!({"type":HF,"max_range_nm":500.}));
    let receiver = id(&w, "RECEIVER");
    send(&mut w, sender, warning(contact, receiver));
    let listener = id(&w, "ES_LISTENER");
    assert_eq!(hf_es::es_sources(&w).len(), 1, "live at the sending tick");
    let next_tick = w.tick + 1;
    run(&mut w, next_tick);
    assert_eq!(
        c::hf_transmissions(&w).len(),
        1,
        "the producer ledger still keeps the committed message"
    );
    assert!(
        hf_es::es_sources(&w).is_empty(),
        "a late receiver never sees a historical pulse"
    );
    assert!(hf_es::sample(&w, &hf_es_config(), listener)
        .unwrap()
        .is_empty());
    assert!(hf_es::own_view(&w, &hf_es_config(), listener).is_none());
}

#[test]
fn hf_es_next04_sample_survives_world_serde_restore() {
    let (mut w, sender, contact) = hf_fixture(json!({"type":HF,"max_range_nm":500.}));
    let receiver = id(&w, "RECEIVER");
    send(&mut w, sender, warning(contact, receiver));
    let listener = id(&w, "ES_LISTENER");
    let before = hf_es::sample(&w, &hf_es_config(), listener).unwrap();
    assert_eq!(before.len(), 1);
    let raw = serde_json::to_value(&w).unwrap();
    let restored: World = serde_json::from_value(raw).unwrap();
    assert_eq!(restored.tick, w.tick, "restore keeps the sending tick");
    let after = hf_es::sample(&restored, &hf_es_config(), listener).unwrap();
    assert_eq!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(&after).unwrap(),
        "the leaf is pure over World: restore preserves evidence exactly"
    );
}

#[test]
fn hf_es_next05_declared_sensor_damage_gate_is_real_hardware() {
    // The declared sensor must be fitted AND undamaged on the receiver.
    // Damage is applied as an explicit adjudicated ext input consumed by the
    // real damage-runtime sensor gate, then removed to show recovery.
    let (mut w, sender, contact) = hf_fixture(json!({"type":HF,"max_range_nm":500.}));
    let receiver = id(&w, "RECEIVER");
    send(&mut w, sender, warning(contact, receiver));
    let listener = id(&w, "ES_LISTENER");
    let cfg = hf_es_config();
    assert_eq!(hf_es::sample(&w, &cfg, listener).unwrap().len(), 1);
    {
        let e = w.entity_mut(listener).unwrap();
        e.ext.insert("rules.v2".into(), 1.0);
        e.ext.insert(format!("damage.sensor.{ES_SENSOR}"), 1.0);
    }
    assert!(
        hf_es::sample(&w, &cfg, listener).unwrap().is_empty(),
        "a damaged declared ES sensor hears nothing"
    );
    assert!(hf_es::own_view(&w, &cfg, listener).is_none());
    {
        let e = w.entity_mut(listener).unwrap();
        e.ext.insert(format!("damage.sensor.{ES_SENSOR}"), 0.0);
    }
    assert_eq!(
        hf_es::sample(&w, &cfg, listener).unwrap().len(),
        1,
        "repairing the cause restores the receiver within the same tick"
    );
}

#[test]
fn hf_es_next06_enemy_es_contact_uses_declared_sensor_bearing_only() {
    // Expected RED until the root-owned detection2026 integration consumes
    // hf_es at the sending tick. The only accepted green is a REAL Red ES
    // contact about the emitter contributed by the declared sensor, and it
    // must stay bearing-only: no fix, no class identification.
    let (mut w, sender, contact) = hf_fixture(json!({"type":HF,"max_range_nm":500.}));
    let receiver = id(&w, "RECEIVER");
    send(&mut w, sender, warning(contact, receiver));
    let detected = w.sides[Side::Red.index()]
        .contacts
        .values()
        .find(|c| !c.stale && c.about == sender)
        .expect("C3 real consumer gap: committed TDL HF transmission must be ES-detectable at the actual sending tick (English2026 PDF74/6-6 6.3.9); detection2026 never consults communications2026::hf_es");
    assert!(
        detected.sensors.iter().any(|s| s == ES_SENSOR),
        "the ES contact must be contributed by the declared HF-ES receiver sensor"
    );
    assert!(
        detected.bearing_only,
        "HF-ES evidence is bearing-only; no position fix"
    );
    assert!(
        detected.class_name.is_none(),
        "no class identification from an HF-ES intercept"
    );
}

// Adjudicated damage input boundary, not a natural weapon hit: the real
// public TDL validator must consume the already-disabled physical room.
#[test]
fn cas_short_radio_damage_reaches_real_tdl_route_and_repair_restores_it() {
    let (mut w,sender,contact)=hf_fixture(json!({"type":HF,"max_range_nm":500.}));
    let receiver=id(&w,"RECEIVER");
    let action:c::Action=serde_json::from_value(warning(contact,receiver)).unwrap();
    c::validate_action(&w,Side::Blue,sender,&action).unwrap();
    w.rules.expanded_damage.as_mut().unwrap().units.entry(sender).or_default().disabled.insert("radio".into());
    let before=serde_json::to_vec(&w).unwrap();
    let error=c::validate_action(&w,Side::Blue,sender,&action).unwrap_err();
    assert!(error.contains("short_range") && error.contains("source_pending"),"missing formation qualification cannot bypass the printed damage group: {error}");
    assert_eq!(before,serde_json::to_vec(&w).unwrap(),"read-only admission cannot mutate a disabled room");
    harpoon_core::rules::expanded_damage2026::restore_component(&mut w,sender,"radio").unwrap();
    c::validate_action(&w,Side::Blue,sender,&action).unwrap();
}

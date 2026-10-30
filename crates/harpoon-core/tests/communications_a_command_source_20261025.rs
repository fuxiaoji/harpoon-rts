//! Contract A candidates: existing public APIs + Command serde, no future Rust symbols.
//! Missing communications commands must fail at runtime parsing, never compilation.
//! Original English2026 PDF75/6-7 6.3.10: Plotting -> same Tac Detection.
//! Explicit laboratory voice fit/sonar/environment; not a real Annex link fit.
//! Full source contacts/reports come from actual native scans, no setters.
//! No Contact/FCS/HP/die/classification/Flight/communications state is injected.
//! Core-only hold/serde recovery tests do not claim live-server pause or cross-process validation.
use harpoon_core::rules::firecontrol2026;
use harpoon_core::{
    observe, Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World,
};
use serde_json::{json, Value};

const PASSIVE: &str = "lab_comms_passive_source";
const MARKER: &str = "radar_air_lab_comms_name_only";
const SENDER_CLASS: &str = "lab_comms_sender";
const RECEIVER_CLASS: &str = "lab_comms_marker_receiver";
const PLAIN_CLASS: &str = "lab_comms_plain_receiver";

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn voice_configuration() -> Value {
    let fit = |peers: Vec<&str>| {
        json!({"source":"scenario_assumption: explicit voice laboratory equipment and directed connectivity; no real range claim",
        "protocol":"lab-voice","send":true,"receive":true,"operational":true,"peers":peers})
    };
    json!({"source":"scenario_assumption: Contract A voice warning laboratory",
    "units":{
        "SENDER":{"source":"scenario_assumption: transmitter","fits":{"lab_voice":fit(vec!["PREFIX_RECEIVER","PLAIN_RECEIVER"]) }},
        "PREFIX_RECEIVER":{"source":"scenario_assumption: receiver","fits":{"lab_voice":fit(vec![]) }},
        "PLAIN_RECEIVER":{"source":"scenario_assumption: receiver","fits":{"lab_voice":fit(vec![]) }}
    }})
}
fn laboratory_database() -> Database {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let mut sonar = db.sensors.values().next().unwrap().clone();
    sonar.id = PASSIVE.into();
    sonar.name = "scenario_assumption: Gen5 LMF laboratory passive sonar".into();
    sonar.kind = harpoon_core::db::SensorKind::SonarPassive;
    sonar.air_range_nm = 0.;
    sonar.surface_range_nm = 0.;
    sonar.sub_range_nm = 4.;
    sonar.missile_range_nm = 0.;
    db.sensors.insert(PASSIVE.into(), sonar);
    let mut marker = db.sensors.values().next().unwrap().clone();
    marker.id = MARKER.into();
    marker.name = "scenario_assumption: zero-range name-only radar".into();
    marker.kind = harpoon_core::db::SensorKind::Radar;
    marker.air_range_nm = 0.;
    marker.surface_range_nm = 0.;
    marker.sub_range_nm = 0.;
    marker.missile_range_nm = 0.;
    db.sensors.insert(MARKER.into(), marker);
    for (key, sensors) in [
        (SENDER_CLASS, vec![PASSIVE.into()]),
        (RECEIVER_CLASS, vec![MARKER.into()]),
        (PLAIN_CLASS, vec![]),
    ] {
        let mut hull = db.platforms["us_burke_2a"].clone();
        hull.id = key.into();
        hull.name = format!("scenario_assumption: original-DP/inventory lab hull {key}");
        hull.sensors = sensors;
        db.platforms.insert(key.into(), hull);
    }
    db.validate().unwrap();
    db
}
fn laboratory_world(db: Database, seed: u64, communications: &Value) -> World {
    let receiver = json!({"type":"sonar","source":"scenario_assumption: Gen5 Bow LMF4nm50percent lab sonar",
        "generation":5,"passive_range_nm":4,"active_range_nm":null,"frequency":"lmf","mount":"bow",
        "active_torpedo_detection":false,"high_speed_tow":false});
    let bindings = [(format!("{SENDER_CLASS}:{PASSIVE}"), receiver)]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let sc:Scenario=serde_json::from_value(json!({
        "id":"communications-contract-A-native-command","name":"scenario_assumption: explicit voice warning laboratory",
        "duration_s":10000,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":2,"precipitation":"None"},
            "communications_2026":communications,
            "detection_2026":{"environment":{"source":"scenario_assumption: explicit calm LMF open water",
                "sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
                "water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
                "bindings":bindings,"targets":{"cn_093":{"source":"scenario_assumption: controlled acoustic signature, not a complete real fit",
                    "size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
        "units":[
            {"side":"Blue","class":SENDER_CLASS,"name":"SENDER","pos":[0,0],"course_deg":90,"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":RECEIVER_CLASS,"name":"PREFIX_RECEIVER","pos":[0,-1],"course_deg":90,"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":PLAIN_CLASS,"name":"PLAIN_RECEIVER","pos":[0,-2],"course_deg":90,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}
        ]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let target = id(&w, "TARGET");
    w.entity_mut(target).unwrap().sensors.clear();
    assert!(w.sides[0].contacts.is_empty());
    w
}
fn fixture_with(config: Value) -> (World, ContactId, EntityId) {
    let db = laboratory_database();
    for seed in 1..=128 {
        let mut w = laboratory_world(db.clone(), seed, &config);
        let sender = id(&w, "SENDER");
        let receiver = id(&w, "PREFIX_RECEIVER");
        let plain = id(&w, "PLAIN_RECEIVER");
        let target = id(&w, "TARGET");
        harpoon_core::sim::tick(
            &mut w,
            &[
                (
                    Side::Blue,
                    Command::SetUnitAutomation {
                        unit: sender,
                        enabled: true,
                    },
                ),
                (
                    Side::Blue,
                    Command::SetUnitAutomation {
                        unit: receiver,
                        enabled: false,
                    },
                ),
                (
                    Side::Blue,
                    Command::SetUnitAutomation {
                        unit: plain,
                        enabled: false,
                    },
                ),
            ],
        );
        let Some(contact) = w.sides[0].contacts.values().find(|c| c.about == target) else {
            continue;
        };
        let cid = contact.id;
        assert!(contact.bearing_only);
        let tracks = &w.rules.aaw.as_ref().unwrap().firecontrol.tracks;
        let report = tracks
            .values()
            .find(|r| r.observer == sender && r.target == target)
            .and_then(|r| r.report.as_ref())
            .expect("native source report, not a fixture setter");
        assert_eq!(report.at, 1);
        assert!(report.measured_position.is_none());
        assert_eq!(report.sensor, PASSIVE);
        assert!(!tracks
            .values()
            .any(|r| (r.observer == receiver || r.observer == plain) && r.target == target));
        assert_ne!(
            w.entity(sender).unwrap().ext.get("control.manual"),
            Some(&1.)
        );
        assert_eq!(
            w.entity(receiver).unwrap().ext.get("control.manual"),
            Some(&1.)
        );
        return (w, cid, target);
    }
    panic!("no genuine native source contact; fixture failure must not be counted as a rule-red")
}
fn fixture() -> (World, ContactId, EntityId) {
    fixture_with(voice_configuration())
}
fn run_to(w: &mut World, at: i64) {
    while w.tick < at {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
    assert_eq!(w.tick, at);
}
fn command_value(actor: EntityId, cid: ContactId, receivers: Vec<EntityId>) -> Value {
    json!({"cmd":"communications","unit":actor,"action":{"type":"send_warning","contact":cid,"sensor":PASSIVE,"fit":"lab_voice","receivers":receivers}})
}
fn command(value: Value) -> Command {
    serde_json::from_value(value).expect("Contract A typed communications command must parse; absence is runtime feature-red, never missing Rust symbols")
}
fn own_unit(w: &World, side: Side, unit: EntityId) -> Value {
    let v = serde_json::to_value(observe(w, side, 0, 10000)).unwrap();
    v["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == json!(unit))
        .expect("own unit is in its legitimate observation")
        .clone()
}
fn communication_view(w: &World, unit: EntityId) -> Value {
    let side = w.entity(unit).unwrap().side;
    let value = own_unit(w, side, unit)["communications"].clone();
    assert!(
        value.is_object(),
        "own communications whitelist must be projected for configured unit"
    );
    assert_eq!(value["scope"], "voice_warning_only");
    value
}
fn messages(w: &World, unit: EntityId) -> Vec<Value> {
    communication_view(w, unit)["messages"]
        .as_array()
        .expect("messages array")
        .clone()
}
fn latest_report(w: &World, unit: EntityId, cid: ContactId) -> Value {
    communication_view(w, unit)["reports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["contact"] == json!(cid) && r["sensor"] == PASSIVE)
        .expect("exact observer source choice")
        .clone()
}
fn protected(w: &World) -> Value {
    json!(w.entities.values().filter(|e|e.side==Side::Blue).map(|e|json!({"unit":e.id,
        "ammo":e.weapons.iter().map(|m|json!([m.weapon,m.ammo])).collect::<Vec<_>>(),
        "manual":e.ext.get("control.manual"),"fire_on":e.fire_on,"mission":e.mission,
        "waypoints":e.waypoints,"ordered_course":e.ordered_course_deg,"ordered_speed":e.ordered_speed_kts})).collect::<Vec<_>>())
}
fn physical_detection(w: &World) -> Value {
    let d = w.rules.detection.as_ref().unwrap();
    // The general event-consumption cursor legitimately advances over the
    // extra command/message receipts. Compare actual detector evidence,
    // dice and deployment instead of mistaking that cursor for a bonus.
    json!({"trials":d.trials,"tracks":d.tracks,"arrays":d.arrays,"array_positions":d.array_positions,
        "modes":d.modes,"transients":d.transients,"unresolved_transients":d.unresolved_transients,"fired":d.fired})
}
fn assert_no_new_message(w: &World) {
    for name in ["SENDER", "PREFIX_RECEIVER", "PLAIN_RECEIVER"] {
        assert!(
            messages(w, id(w, name)).is_empty(),
            "invalid action cannot partially deliver"
        );
    }
}
fn reject_actual(mut w: World, value: Value) {
    let cmd = command(value);
    let mut control = w.clone();
    let before = protected(&w);
    harpoon_core::sim::tick(&mut control, &[]);
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
    assert_no_new_message(&w);
    assert_eq!(protected(&w), before);
    assert_eq!(protected(&w), protected(&control));
    assert_eq!(
        serde_json::to_value(&w.sides[0].contacts).unwrap(),
        serde_json::to_value(&control.sides[0].contacts).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&w.rules.aaw.as_ref().unwrap().firecontrol).unwrap(),
        serde_json::to_value(&control.rules.aaw.as_ref().unwrap().firecontrol).unwrap()
    );
    assert_eq!(physical_detection(&w), physical_detection(&control));
    // This is actual rejected-command isolation, not a claim of readonly
    // validation: sim necessarily advances tick and may emit rejection receipts.
}
fn assert_keys(value: &Value, allowed: &[&str]) {
    for key in value.as_object().expect("whitelist object").keys() {
        assert!(
            allowed.contains(&key.as_str()),
            "unexpected own communication field {key}"
        );
    }
}

#[test]
fn a01_native_warning_delivers_to_two_own_receivers_in_same_detection_tick() {
    let (mut w, cid, _) = fixture();
    let sender = id(&w, "SENDER");
    let a = id(&w, "PREFIX_RECEIVER");
    let b = id(&w, "PLAIN_RECEIVER");
    let cmd = command(command_value(sender, cid, vec![a, b]));
    let source = latest_report(&w, sender, cid);
    assert_eq!(source["observed_at"], 1);
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
    let sent = messages(&w, sender);
    assert_eq!(sent.len(), 1);
    let m = &sent[0];
    assert_eq!(m["id"], 1);
    assert_eq!(m["sender"], json!(sender));
    assert_eq!(m["state"], "delivered");
    assert_eq!(m["queued_at"], 2);
    assert_eq!(m["terminal_at"], 2);
    assert_eq!(m["tactical_turn"], 0);
    assert_eq!(m["contact_is_source_label"], true);
    assert_eq!(m["report_is_live_track"], false);
    assert_eq!(m["contact_current"], true);
    assert_eq!(m["report"], source);
    for receiver in [a, b] {
        let received = messages(&w, receiver);
        assert_eq!(received.len(), 1);
        assert_eq!(received[0]["direction"], "received");
        assert_eq!(received[0]["report"], source);
        assert_eq!(received[0]["terminal_at"], 2);
    }
}

#[test]
fn a02_unknown_classification_and_original_report_geometry_are_frozen() {
    let (mut w, cid, target) = fixture();
    let sender = id(&w, "SENDER");
    let receiver = id(&w, "PREFIX_RECEIVER");
    let cmd = command(command_value(sender, cid, vec![receiver]));
    let source = latest_report(&w, sender, cid);
    assert!(
        source["category"].is_null(),
        "first-Tac native sonar report has no classification evidence yet"
    );
    assert_eq!(source["bearing_only"], true);
    assert!(source["measured_position"].is_null());
    let pos_before = w.entity(target).unwrap().pos;
    harpoon_core::sim::tick(
        &mut w,
        &[
            (Side::Blue, cmd),
            (
                Side::Red,
                Command::SetSpeed {
                    unit: target,
                    speed_kts: 6.,
                },
            ),
        ],
    );
    assert!(
        w.entity(target).unwrap().pos.range_to(pos_before) > 0.,
        "actual native movement after source report"
    );
    assert_eq!(messages(&w, receiver)[0]["report"], source);
    run_to(&mut w, 181);
    assert_eq!(
        messages(&w, receiver)[0]["report"],
        source,
        "later own scans/classification cannot rewrite a received snapshot"
    );
    assert!(messages(&w, receiver)[0]["report"]["category"].is_null());
}

#[test]
fn a03_mixed_foreign_receiver_rejects_whole_action_without_taking_control() {
    let (w, cid, target) = fixture();
    let sender = id(&w, "SENDER");
    let own = id(&w, "PREFIX_RECEIVER");
    let mut value = command_value(sender, cid, vec![own, target]);
    value["retain_manual"] = json!(true);
    reject_actual(w, value);
}

#[test]
fn a04_side_fused_contact_cannot_be_sent_by_a_nonobserving_actor() {
    let mut config = voice_configuration();
    config["units"]["PREFIX_RECEIVER"]["fits"]["lab_voice"]["peers"] = json!(["PLAIN_RECEIVER"]);
    let (w, cid, _) = fixture_with(config);
    let actor = id(&w, "PREFIX_RECEIVER");
    let receiver = id(&w, "PLAIN_RECEIVER");
    reject_actual(w, command_value(actor, cid, vec![receiver]));
}

#[test]
fn a05_authoritative_own_cid_never_falls_back_to_private_entity_or_missing_cid() {
    for private_id in [false, true] {
        let (w, cid, target) = fixture();
        let actor = id(&w, "SENDER");
        let receiver = id(&w, "PREFIX_RECEIVER");
        let invalid = if private_id {
            ContactId(target.0)
        } else {
            ContactId(cid.0 + 1000)
        };
        assert_ne!(invalid, cid);
        reject_actual(w, command_value(actor, invalid, vec![receiver]));
    }
}

#[test]
fn a06_exact_sensor_source_cannot_be_replaced_by_a_different_or_missing_sensor() {
    for sensor in [MARKER, "lab_nonexistent_sensor"] {
        let (w, cid, _) = fixture();
        let actor = id(&w, "SENDER");
        let receiver = id(&w, "PREFIX_RECEIVER");
        let mut value = command_value(actor, cid, vec![receiver]);
        value["action"]["sensor"] = json!(sensor);
        reject_actual(w, value);
    }
}

#[test]
fn a07_unknown_fit_or_authored_nonoperational_equipment_rejects() {
    let (w, cid, _) = fixture();
    let actor = id(&w, "SENDER");
    let receiver = id(&w, "PREFIX_RECEIVER");
    let mut value = command_value(actor, cid, vec![receiver]);
    value["action"]["fit"] = json!("lab_unknown_fit");
    reject_actual(w, value);
    for unit in ["SENDER", "PREFIX_RECEIVER"] {
        let mut config = voice_configuration();
        config["units"][unit]["fits"]["lab_voice"]["operational"] = json!(false);
        let (w, cid, _) = fixture_with(config);
        let actor = id(&w, "SENDER");
        let receiver = id(&w, "PREFIX_RECEIVER");
        reject_actual(w, command_value(actor, cid, vec![receiver]));
    }
}

#[test]
fn a08_equal_fit_labels_do_not_override_protocol_incompatibility() {
    let mut config = voice_configuration();
    config["units"]["PREFIX_RECEIVER"]["fits"]["lab_voice"]["protocol"] =
        json!("lab-incompatible-voice");
    let (w, cid, _) = fixture_with(config);
    let actor = id(&w, "SENDER");
    let receiver = id(&w, "PREFIX_RECEIVER");
    reject_actual(w, command_value(actor, cid, vec![receiver]));
}

#[test]
fn a09_receive_capability_does_not_create_an_undeclared_directed_route() {
    let mut config = voice_configuration();
    config["units"]["SENDER"]["fits"]["lab_voice"]["peers"] = json!(["PLAIN_RECEIVER"]);
    let (w, cid, _) = fixture_with(config);
    let actor = id(&w, "SENDER");
    let receiver = id(&w, "PREFIX_RECEIVER");
    reject_actual(w, command_value(actor, cid, vec![receiver]));
}

#[test]
fn a10_repeated_report_sends_are_legal_without_creating_receiver_contact_or_fcs() {
    let (mut w, cid, target) = fixture();
    let actor = id(&w, "SENDER");
    let receiver = id(&w, "PREFIX_RECEIVER");
    let first = command(command_value(actor, cid, vec![receiver]));
    let second = command(command_value(actor, cid, vec![receiver]));
    let mut control = w.clone();
    harpoon_core::sim::tick(&mut control, &[]);
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, first), (Side::Blue, second)]);
    let received = messages(&w, receiver);
    assert_eq!(received.len(), 2);
    assert_eq!(received[0]["id"], 1);
    assert_eq!(received[1]["id"], 2);
    assert_eq!(received[0]["report"], received[1]["report"]);
    assert!(firecontrol2026::world_solution(&w, receiver, target, 4, 45.).is_err());
    assert!(!w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .any(|t| t.observer == receiver && t.target == target));
    assert_eq!(
        serde_json::to_value(&w.sides[0].contacts).unwrap(),
        serde_json::to_value(&control.sides[0].contacts).unwrap()
    );
    assert_eq!(
        physical_detection(&w),
        physical_detection(&control),
        "voice warning cannot create a sonar trial/bonus/track"
    );
}

#[test]
fn a11_holding_a_parsed_command_without_native_time_does_not_deliver_it() {
    let (mut w, cid, _) = fixture();
    let actor = id(&w, "SENDER");
    let receiver = id(&w, "PREFIX_RECEIVER");
    let before = serde_json::to_value(&w).unwrap();
    let cmd = command(command_value(actor, cid, vec![receiver]));
    for _ in 0..3 {
        assert!(messages(&w, actor).is_empty());
        assert!(messages(&w, receiver).is_empty());
    }
    assert_eq!(serde_json::to_value(&w).unwrap(),before,"core held command does not itself advance or deliver; live server pause is separate acceptance");
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
    assert_eq!(messages(&w, receiver).len(), 1);
}

#[test]
fn a12_t179_t180_receipts_use_actual_command_execution_tick() {
    for execute_at in [179_i64, 180_i64] {
        let (mut w, cid, _) = fixture();
        let actor = id(&w, "SENDER");
        let receiver = id(&w, "PREFIX_RECEIVER");
        let cmd = command(command_value(actor, cid, vec![receiver]));
        run_to(&mut w, execute_at - 1);
        harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
        let received = messages(&w, receiver);
        assert_eq!(received.len(), 1);
        assert_eq!(received[0]["queued_at"], execute_at);
        assert_eq!(received[0]["terminal_at"], execute_at);
        assert_eq!(received[0]["tactical_turn"], execute_at.div_euclid(180));
    }
}

#[test]
fn a13_message_snapshot_and_terminal_state_survive_complete_world_serde_resume() {
    let (mut w, cid, _) = fixture();
    let actor = id(&w, "SENDER");
    let receiver = id(&w, "PREFIX_RECEIVER");
    let cmd = command(command_value(actor, cid, vec![receiver]));
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
    assert_eq!(messages(&w, receiver).len(), 1);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    restored.rules.validate_tables().unwrap();
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    for _ in 0..30 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert_eq!(messages(&restored, receiver).len(), 1);
}

#[test]
fn a14_own_message_is_whitelisted_and_opponent_observation_has_no_message_or_private_target() {
    let (mut w, cid, target) = fixture();
    let actor = id(&w, "SENDER");
    let receiver = id(&w, "PREFIX_RECEIVER");
    let cmd = command(command_value(actor, cid, vec![receiver]));
    let seq = w.event_seq;
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
    let own = communication_view(&w, receiver);
    assert_keys(&own, &["scope", "fits", "reports", "messages"]);
    let received = messages(&w, receiver);
    assert_eq!(received.len(), 1);
    assert_keys(
        &received[0],
        &[
            "id",
            "sender",
            "receivers",
            "fit",
            "protocol",
            "queued_at",
            "tactical_turn",
            "state",
            "terminal_at",
            "failure",
            "direction",
            "report",
            "contact_is_source_label",
            "report_is_live_track",
            "contact_current",
        ],
    );
    assert_keys(
        &received[0]["report"],
        &[
            "contact",
            "sensor",
            "observed_at",
            "observed_tactical_turn",
            "category",
            "observer_position",
            "bearing_deg",
            "bearing_only",
            "measured_position",
            "bearing_error_deg",
        ],
    );
    assert!(
        own["reports"].as_array().unwrap().is_empty(),
        "receipt never becomes receiver native evidence"
    );
    let enemy = own_unit(&w, Side::Red, target)["communications"].clone();
    assert!(enemy.is_null() || enemy["messages"].as_array().is_some_and(Vec::is_empty));
    assert!(
        !w.events_for(Side::Red, seq).iter().any(
            |e| matches!(&e.kind,EventKind::RulesEvent{domain,..}if domain=="communications2026")
        ),
        "other side never sees communication event detail"
    );
}

#[test]
fn a15_voice_warning_preserves_real_inventory_manual_control_and_native_navigation() {
    let (mut w, cid, _) = fixture();
    let actor = id(&w, "SENDER");
    let receiver = id(&w, "PREFIX_RECEIVER");
    let cmd = command(command_value(actor, cid, vec![receiver]));
    harpoon_core::sim::tick(
        &mut w,
        &[
            (
                Side::Blue,
                Command::SetSpeed {
                    unit: receiver,
                    speed_kts: 8.,
                },
            ),
            (
                Side::Blue,
                Command::SetCourse {
                    unit: receiver,
                    course_deg: 45.,
                },
            ),
        ],
    );
    let start = w.entity(receiver).unwrap().pos;
    let saved = protected(&w);
    let mut control = w.clone();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
    harpoon_core::sim::tick(&mut control, &[]);
    for _ in 0..20 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut control, &[]);
    }
    assert_eq!(messages(&w, receiver).len(), 1);
    assert_eq!(protected(&w), saved);
    assert_eq!(protected(&w), protected(&control));
    let a = w.entity(receiver).unwrap();
    let b = control.entity(receiver).unwrap();
    assert!(a.pos.range_to(start) > 0., "real native motion continues");
    assert_eq!(a.pos, b.pos);
    assert_eq!(a.course_deg, b.course_deg);
    assert_eq!(a.speed_kts, b.speed_kts);
    assert_eq!(a.ext.get("control.manual"), Some(&1.));
}

#[test]
fn a16_retain_manual_commits_only_after_valid_send_and_only_for_sender() {
    let (mut invalid, cid, _) = fixture();
    let actor = id(&invalid, "SENDER");
    let receiver = id(&invalid, "PREFIX_RECEIVER");
    let mut value = command_value(actor, cid, vec![receiver]);
    value["retain_manual"] = json!(true);
    value["action"]["fit"] = json!("unknown_fit");
    let invalid_cmd = command(value);
    harpoon_core::sim::tick(&mut invalid, &[(Side::Blue, invalid_cmd)]);
    assert_no_new_message(&invalid);
    assert_ne!(
        invalid.entity(actor).unwrap().ext.get("control.manual"),
        Some(&1.)
    );
    let (mut valid, cid, _) = fixture();
    let actor = id(&valid, "SENDER");
    let receiver = id(&valid, "PREFIX_RECEIVER");
    harpoon_core::sim::tick(
        &mut valid,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: receiver,
                speed_kts: 8.,
            },
        )],
    );
    let before_receiver = serde_json::to_value(valid.entity(receiver).unwrap()).unwrap();
    let mut control = valid.clone();
    let mut value = command_value(actor, cid, vec![receiver]);
    value["retain_manual"] = json!(true);
    let valid_cmd = command(value);
    harpoon_core::sim::tick(&mut valid, &[(Side::Blue, valid_cmd)]);
    harpoon_core::sim::tick(&mut control, &[]);
    assert_eq!(
        valid.entity(actor).unwrap().ext.get("control.manual"),
        Some(&1.)
    );
    assert_eq!(messages(&valid, receiver).len(), 1);
    assert_eq!(
        serde_json::to_value(valid.entity(receiver).unwrap()).unwrap(),
        serde_json::to_value(control.entity(receiver).unwrap()).unwrap(),
        "receiver only receives a receipt; no ownership/mission/equipment mutation"
    );
    assert_eq!(before_receiver["ordered_speed_kts"], 8.);
}

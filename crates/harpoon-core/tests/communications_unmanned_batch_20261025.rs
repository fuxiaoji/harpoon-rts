//! Batch candidates: real native sensor reports and actual source command/phase
//! chains. No contact/FCS/classification/HP/Flight/die/grant-state injection.
//! Laboratory sensor/control fittings are explicit scenario_assumption, while
//! original loaded hull HP and weapon stock remain untouched. Not yet executed.
use harpoon_core::rules::{communications2026 as c, firecontrol2026 as f, unmanned2026 as u};
use harpoon_core::{
    Command, ContactId, Database, EntityId, EventKind, Scenario, Side, Vec2, World,
};
use serde_json::{json, Value};

const P: &str = "batch_lab_passive";
const A: &str = "sonar_a_batch_lab_active";
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
fn command(value: Value) -> Command {
    serde_json::from_value(value)
        .expect("typed batch command must parse at runtime; no future Rust-symbol compile-red")
}
fn send(w: &mut World, actor: EntityId, action: Value) {
    let typed: c::Action = serde_json::from_value(action.clone()).unwrap();
    c::validate_action(w, Side::Blue, actor, &typed)
        .expect("actual source-qualified communication checkpoint before queued execution");
    let cmd = command(json!({"cmd":"communications","unit":actor,"action":action}));
    harpoon_core::sim::tick(w, &[(Side::Blue, cmd)]);
}
fn unmanned(w: &mut World, actor: EntityId, action: Value) {
    let typed: u::Action = serde_json::from_value(action.clone()).unwrap();
    u::validate_action(w, Side::Blue, actor, &typed)
        .expect("actual source-qualified hardware checkpoint before queued execution");
    let cmd = command(json!({"cmd":"unmanned","unit":actor,"action":action}));
    harpoon_core::sim::tick(w, &[(Side::Blue, cmd)]);
}
fn database() -> Database {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for (key, kind, range) in [
        (P, harpoon_core::db::SensorKind::SonarPassive, 4.),
        (A, harpoon_core::db::SensorKind::SonarActive, 20.),
    ] {
        let mut s = db.sensors.values().next().unwrap().clone();
        s.id = key.into();
        s.name = format!("scenario_assumption:{key}");
        s.kind = kind;
        s.air_range_nm = 0.;
        s.surface_range_nm = 0.;
        s.missile_range_nm = 0.;
        s.sub_range_nm = range;
        db.sensors.insert(key.into(), s);
    }
    db.platforms.get_mut("us_virginia").unwrap().sensors = vec![P.into(), A.into()];
    let mut rx = db.platforms["us_burke_2a"].clone();
    rx.id = "batch_lab_receiver".into();
    rx.name = "scenario_assumption: original hull/stock laboratory receiver".into();
    rx.sensors = vec![A.into()];
    db.platforms.insert(rx.id.clone(), rx);
    let mut target = db.platforms["cn_093"].clone();
    target.id = "batch_lab_target".into();
    target.sensors.clear();
    db.platforms.insert(target.id.clone(), target);
    db.validate().unwrap();
    db
}
fn environment() -> Value {
    json!({"source":"scenario_assumption: explicit open-water LMF experiment","sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null})
}
fn sonar(active: bool, range: f64) -> Value {
    json!({"type":"sonar","source":"scenario_assumption: Gen5 Bow LMF experimental sonar","generation":5,"passive_range_nm":if active{None}else{Some(range)},"active_range_nm":if active{Some(range)}else{None},"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false})
}
fn fits(kind: &str, rx_kind: &str) -> Value {
    let make = |kind: &str, peers: Vec<&str>| json!({"source":"scenario_assumption: laboratory link, no claim of actual Annex fitting","protocol":"batch-voice-or-data","send":true,"receive":true,"operational":true,"peers":peers,"warning_kind":kind,"automatic_warning":false,"targeting_system":kind=="real_time"});
    json!({"source":"scenario_assumption: explicit experiment","units":{
        "SENDER":{"source":"scenario_assumption: original-body test","fits":{"lab":make(kind,vec!["RECEIVER"]) }},
        "SENDER_B":{"source":"scenario_assumption: original-body test","fits":{"lab":make(kind,vec!["RECEIVER"]) }},
        "RECEIVER":{"source":"scenario_assumption: original-body test","fits":{"lab":make(rx_kind,vec![]) }}
    }})
}
fn comm_world(seed: u64, kind: &str, rx_kind: &str, rx_range: f64) -> World {
    comm_world_config(seed, rx_range, fits(kind, rx_kind))
}
fn comm_world_config(seed: u64, rx_range: f64, communications: Value) -> World {
    let bindings = [
        (format!("us_virginia:{P}"), sonar(false, 4.)),
        (format!("us_virginia:{A}"), sonar(true, 20.)),
        (format!("batch_lab_receiver:{A}"), sonar(true, rx_range)),
    ]
    .into_iter()
    .collect::<std::collections::BTreeMap<_, _>>();
    let sc:Scenario=serde_json::from_value(json!({"id":"batch-communications-native","name":"scenario_assumption: laboratory sensors/links only, original DP and stocks","duration_s":10000,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":2,"precipitation":"None"},"communications_2026":communications,"detection_2026":{"bindings":bindings,"environment":environment(),"targets":{"batch_lab_target":{"source":"scenario_assumption: controlled acoustic signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},"units":[
        {"side":"Blue","class":"us_virginia","name":"SENDER","depth_m":50,"pos":[0,0],"course_deg":90,"speed_kts":0,"fire_on":false},
        {"side":"Blue","class":"us_virginia","name":"SENDER_B","depth_m":50,"pos":[0.1,0],"course_deg":90,"speed_kts":0,"fire_on":false},
        {"side":"Blue","class":"batch_lab_receiver","name":"RECEIVER","pos":[0,-1],"course_deg":90,"speed_kts":0,"fire_on":false},
        {"side":"Red","class":"batch_lab_target","name":"TARGET","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, database()).unwrap();
    let (a, b, r) = (id(&w, "SENDER"), id(&w, "SENDER_B"), id(&w, "RECEIVER"));
    harpoon_core::sim::tick(
        &mut w,
        &[
            (
                Side::Blue,
                Command::SetSonar {
                    unit: a,
                    active: true,
                },
            ),
            (
                Side::Blue,
                Command::SetSonar {
                    unit: b,
                    active: true,
                },
            ),
            (
                Side::Blue,
                Command::SetSonar {
                    unit: r,
                    active: false,
                },
            ),
        ],
    );
    w
}
fn cid(w: &World) -> ContactId {
    let target = id(w, "TARGET");
    w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == target && !c.stale)
        .expect("genuine native source CID checkpoint")
        .id
}
fn source_report(w: &World, sender: EntityId, sensor: &str) -> Option<Value> {
    c::own_view(w, sender)?["reports"]
        .as_array()?
        .iter()
        .find(|r| r["sensor"] == sensor)
        .cloned()
}
fn classified(kind: &str, rx_kind: &str) -> World {
    for seed in 1..=256 {
        let mut w = comm_world(seed, kind, rx_kind, 1.25);
        run(&mut w, 181);
        let sender = id(&w, "SENDER");
        if source_report(&w, sender, P).is_some_and(|v| v["category"] == "Sub")
            && source_report(&w, id(&w, "SENDER_B"), P).is_some()
            && source_report(&w, sender, A).is_some()
            && source_report(&w, id(&w, "SENDER_B"), A).is_some()
        {
            let _ = cid(&w);
            return w;
        }
    }
    panic!("native classification checkpoint unavailable: fixture failure, not accepted rule-red")
}
fn warning(contact: ContactId, sensor: &str, receiver: EntityId) -> Value {
    json!({"type":"send_warning","contact":contact,"sensor":sensor,"fit":"lab","receivers":[receiver]})
}
fn scan(w: &World, receiver: EntityId) -> Value {
    w.events
        .iter()
        .rev()
        .find_map(|event| match &event.kind {
            EventKind::RulesEvent {
                domain,
                rule,
                unit: Some(unit),
                detail,
                ..
            } if domain == "detection2026" && rule == "5.scan" && *unit == receiver => {
                let v: Value = serde_json::from_str(detail).unwrap();
                (v["sensor"] == A && v["result"]["d100"].is_number()).then(|| v["result"].clone())
            }
            _ => None,
        })
        .expect("actual native probability/D100 checkpoint")
}
#[test]
fn b01_native_tl_changes_the_same_actual_receiver_trial_by_fifteen_points() {
    for seed in 1..=256 {
        //1.25nm is exactly between printed1.0/1.5 centres after this
        // broadside(+1)/sea-state2(-1) combination and correctly stays pending.
        let mut w = comm_world(seed, "time_late", "time_late", 1.4);
        run(&mut w, 181);
        let s = id(&w, "SENDER");
        if !source_report(&w, s, P).is_some_and(|r| r["category"] == "Sub") {
            continue;
        }
        let receiver = id(&w, "RECEIVER");
        let target = id(&w, "TARGET");
        let contact = cid(&w);
        let mut control = w.clone();
        harpoon_core::sim::tick(
            &mut control,
            &[(
                Side::Blue,
                Command::SetSonar {
                    unit: receiver,
                    active: true,
                },
            )],
        );
        let before = scan(&control, receiver);
        let p = before["probability"].as_f64().unwrap();
        let die = before["d100"].as_u64().unwrap() as f64;
        if !(p > 0. && p <= 75. && die > p && die <= p + 15.) {
            continue;
        }
        let cmd =
            command(json!({"cmd":"communications","unit":s,"action":warning(contact,P,receiver)}));
        harpoon_core::sim::tick(
            &mut w,
            &[
                (
                    Side::Blue,
                    Command::SetSonar {
                        unit: receiver,
                        active: true,
                    },
                ),
                (Side::Blue, cmd),
            ],
        );
        let after = scan(&w, receiver);
        assert_eq!(after["d100"], before["d100"]);
        assert_eq!(after["probability"].as_f64(), Some(p + 15.));
        assert!(
            w.rules.detection.as_ref().unwrap().tracks[&format!("{}:{A}:{}", receiver.0, target.0)]
                .held
        );
        return;
    }
    panic!("no natural threshold fixture found; do not inject D100 or count as a rule-red");
}
#[test]
fn b02_voice_warning_never_creates_a_tl_opportunity() {
    let mut w = classified("voice", "voice");
    let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
    let contact = cid(&w);
    send(&mut w, s, warning(contact, P, r));
    assert!(!c::sonar_alert(&w, r, t));
    assert_eq!(
        c::cue_evidence(&w, r, t, c::CuePurpose::Acoustic).modifier,
        0
    );
}
#[test]
fn b03_unknown_raw_source_cannot_borrow_the_real_submarine_kind() {
    for seed in 1..=64 {
        let mut w = comm_world(seed, "time_late", "time_late", 1.25);
        let s = id(&w, "SENDER");
        if let Some(report) = source_report(&w, s, P) {
            assert!(report["category"].is_null());
            let (r, t) = (id(&w, "RECEIVER"), id(&w, "TARGET"));
            let contact = cid(&w);
            send(&mut w, s, warning(contact, P, r));
            assert!(!c::sonar_alert(&w, r, t));
            return;
        }
    }
    panic!("no native first-report checkpoint")
}
#[test]
fn b04_nrt_and_rt_cue_are_integer_source_modifiers_not_new_fcs() {
    for (kind, expected) in [("near_real_time", 1), ("real_time", 2)] {
        let mut w = classified(kind, kind);
        let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
        let contact = cid(&w);
        send(&mut w, s, warning(contact, P, r));
        let q = c::cue_evidence(&w, r, t, c::CuePurpose::Acoustic);
        assert_eq!(q.modifier, expected);
        assert_eq!(q.refs.len(), 1);
        assert_eq!(
            c::cue_evidence(&w, r, t, c::CuePurpose::AirDefenseReaction).modifier,
            0
        );
        assert!(c::received_solution(&w, r, t).unwrap().is_none());
    }
}
#[test]
fn b05_repeat_tl_messages_do_not_stack_or_restore_abandoned_opportunity() {
    let mut w = classified("time_late", "time_late");
    let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
    let contact = cid(&w);
    send(&mut w, s, warning(contact, P, r));
    assert!(c::sonar_alert(&w, r, t));
    send(&mut w, r, json!({"type":"abandon_alert","contact":contact}));
    send(&mut w, s, warning(contact, P, r));
    assert!(!c::sonar_alert(&w, r, t));
}
#[test]
fn b06_native_existing_rt_fcs_is_frozen_and_nrt_loses_exactly_one_quality_level() {
    for (rx_kind, quality) in [
        ("real_time", f::Quality::Good),
        ("near_real_time", f::Quality::Fair),
    ] {
        let mut w = classified("real_time", rx_kind);
        let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
        let contact = cid(&w);
        let p = &w.rules.torpedoes.as_ref().unwrap().config.weapons["us_mk48"];
        let original =
            f::own_world_solution(&w, s, t, p.generation.max(1), p.settings[0].speed_kts)
                .expect("native own motion/position FCS checkpoint");
        assert_eq!(original.quality, f::Quality::Good);
        send(
            &mut w,
            s,
            json!({"type":"send_solution","contact":contact,"sensor":A,"fit":"lab","receivers":[r],"weapon":"us_mk48","speed_index":0}),
        );
        let remote = c::received_solution(&w, r, t)
            .unwrap()
            .expect("actual delivered existing FCS");
        assert_eq!(remote.quality, quality);
        assert_eq!(remote.solution.unwrap().quality, quality);
        assert!(remote.aim.is_some());
        assert_eq!(remote.refs.len(), 1);
        assert!(!w
            .rules
            .aaw
            .as_ref()
            .unwrap()
            .firecontrol
            .tracks
            .values()
            .any(|track| track.observer == r && track.target == t));
    }
}
#[test]
fn b07_non_tdl_two_passive_reports_produce_na_without_fake_fcs_track() {
    let mut w = classified("voice", "voice");
    let (a, b, r, t) = (
        id(&w, "SENDER"),
        id(&w, "SENDER_B"),
        id(&w, "RECEIVER"),
        id(&w, "TARGET"),
    );
    let contact = cid(&w);
    send(&mut w, a, warning(contact, P, r));
    send(&mut w, b, warning(contact, P, r));
    send(
        &mut w,
        r,
        json!({"type":"combine_reports","contact":contact,"messages":[{"sender":a,"id":1},{"sender":b,"id":1}]}),
    );
    let q = c::received_solution(&w, r, t)
        .unwrap()
        .expect("declared real two-source composition");
    assert_eq!(q.quality, f::Quality::NA);
    assert!(q.solution.is_none());
    assert!(q.aim.is_none());
}
#[test]
fn b08_non_tdl_two_active_reports_produce_poor_with_selected_measured_aim() {
    let mut w = classified("voice", "voice");
    let (a, b, r, t) = (
        id(&w, "SENDER"),
        id(&w, "SENDER_B"),
        id(&w, "RECEIVER"),
        id(&w, "TARGET"),
    );
    let contact = cid(&w);
    let measured = source_report(&w, a, A).unwrap()["measured_position"].clone();
    send(&mut w, a, warning(contact, A, r));
    send(&mut w, b, warning(contact, A, r));
    send(
        &mut w,
        r,
        json!({"type":"combine_reports","contact":contact,"messages":[{"sender":a,"id":1},{"sender":b,"id":1}],"aim_source":{"sender":a,"id":1}}),
    );
    let q = c::received_solution(&w, r, t).unwrap().unwrap();
    assert_eq!(q.quality, f::Quality::Poor);
    assert_eq!(serde_json::to_value(q.aim).unwrap(), measured);
}
#[test]
fn b09_invalid_source_preflight_is_completely_readonly() {
    let w = classified("real_time", "real_time");
    let (s, r) = (id(&w, "SENDER"), id(&w, "RECEIVER"));
    let before = serde_json::to_value(&w).unwrap();
    let action: c::Action =
        serde_json::from_value(warning(cid(&w), "not-own-native-sensor", r)).unwrap();
    assert!(c::validate_action(&w, Side::Blue, s, &action).is_err());
    assert_eq!(before, serde_json::to_value(w).unwrap());
}
#[test]
fn b10_receiver_target_tl_state_and_provenance_roundtrip_without_new_opportunity() {
    let mut w = classified("time_late", "time_late");
    let (s, r) = (id(&w, "SENDER"), id(&w, "RECEIVER"));
    let contact = cid(&w);
    send(&mut w, s, warning(contact, P, r));
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    for _ in 0..200 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_value(w).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
}
#[test]
fn b11_failed_first_tl_search_is_consumed_without_a_hidden_ordinary_success() {
    for seed in 1..=256 {
        let mut w = comm_world(seed, "time_late", "time_late", 1.4);
        run(&mut w, 181);
        let s = id(&w, "SENDER");
        if !source_report(&w, s, P).is_some_and(|r| r["category"] == "Sub") {
            continue;
        }
        let (r, t) = (id(&w, "RECEIVER"), id(&w, "TARGET"));
        let contact = cid(&w);
        let mut control = w.clone();
        harpoon_core::sim::tick(
            &mut control,
            &[(
                Side::Blue,
                Command::SetSonar {
                    unit: r,
                    active: true,
                },
            )],
        );
        let base = scan(&control, r);
        let p = base["probability"].as_f64().unwrap();
        let roll = base["d100"].as_u64().unwrap() as f64;
        if !(p > 0. && p <= 75. && roll > p + 15.) {
            continue;
        }
        let cmd = command(json!({"cmd":"communications","unit":s,"action":warning(contact,P,r)}));
        harpoon_core::sim::tick(
            &mut w,
            &[
                (
                    Side::Blue,
                    Command::SetSonar {
                        unit: r,
                        active: true,
                    },
                ),
                (Side::Blue, cmd),
            ],
        );
        assert_eq!(scan(&w, r)["probability"].as_f64(), Some(p + 15.));
        assert!(!w.rules.detection.as_ref().unwrap().tracks[&format!("{}:{A}:{}", r.0, t.0)].held);
        assert!(
            c::sonar_alert(&w, r, t),
            "Applied remains active for cached current-epoch evaluations"
        );
        send(&mut w, s, warning(contact, P, r));
        run(&mut w, 360);
        assert!(!c::sonar_alert(&w, r, t));
        assert_eq!(scan(&w, r)["probability"].as_f64(), Some(p));
        return;
    }
    panic!("no natural failing enhanced trial checkpoint; never inject last_success or D100");
}
#[test]
fn b12_late_warning_cannot_upgrade_a_failed_trial_in_the_same_tactical_epoch() {
    for seed in 1..=256 {
        let mut w = comm_world(seed, "time_late", "time_late", 1.4);
        run(&mut w, 181);
        let s = id(&w, "SENDER");
        if !source_report(&w, s, P).is_some_and(|r| r["category"] == "Sub") {
            continue;
        }
        let (r, t) = (id(&w, "RECEIVER"), id(&w, "TARGET"));
        let contact = cid(&w);
        harpoon_core::sim::tick(
            &mut w,
            &[(
                Side::Blue,
                Command::SetSonar {
                    unit: r,
                    active: true,
                },
            )],
        );
        let before = scan(&w, r);
        let p = before["probability"].as_f64().unwrap();
        let roll = before["d100"].as_u64().unwrap() as f64;
        if !(p > 0. && p <= 75. && roll > p && roll <= p + 15.) {
            continue;
        }
        send(&mut w, s, warning(contact, P, r));
        assert!(!c::sonar_alert(&w, r, t));
        assert_eq!(scan(&w, r)["d100"], before["d100"]);
        assert!(!w.rules.detection.as_ref().unwrap().tracks[&format!("{}:{A}:{}", r.0, t.0)].held);
        run(&mut w, 360);
        assert_eq!(scan(&w, r)["probability"].as_f64(), Some(p + 15.));
        return;
    }
    panic!("no actual late-window threshold checkpoint");
}
#[test]
fn b13_automatic_tl_uses_a_completed_native_batch_and_respects_sender_override() {
    let mut w = classified("time_late", "time_late");
    let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
    assert!(!c::sonar_alert(&w, r, t));
    send(
        &mut w,
        s,
        json!({"type":"set_automatic","fit":"lab","enabled":true}),
    );
    assert!(c::sonar_alert(&w, r, t));
    let count = c::own_view(&w, r).unwrap()["messages"]
        .as_array()
        .unwrap()
        .len();
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(
        c::own_view(&w, r).unwrap()["messages"]
            .as_array()
            .unwrap()
            .len(),
        count
    );
    send(
        &mut w,
        s,
        json!({"type":"set_automatic","fit":"lab","enabled":false}),
    );
    assert_eq!(c::own_view(&w, s).unwrap()["fits"][0]["automatic"], false);
}

#[test]
fn b14_different_receiver_terminal_and_protocol_require_explicit_authored_compatibility() {
    for compatible in [false, true] {
        let mut config = fits("real_time", "near_real_time");
        let mut receiver = config["units"]["RECEIVER"]["fits"]
            .as_object_mut()
            .unwrap()
            .remove("lab")
            .unwrap();
        receiver["protocol"] = json!("scenario-assumption-nrt-gateway");
        config["units"]["RECEIVER"]["fits"]["receiver_terminal"] = receiver;
        config["units"]["SENDER"]["fits"]["lab"]["peer_fits"] =
            json!({"RECEIVER":"receiver_terminal"});
        if compatible {
            config["units"]["SENDER"]["fits"]["lab"]["compatible_protocols"] =
                json!(["scenario-assumption-nrt-gateway"]);
        }
        let mut w = (1..=256)
            .find_map(|seed| {
                let mut w = comm_world_config(seed, 1.25, config.clone());
                run(&mut w, 181);
                source_report(&w, id(&w, "SENDER"), P)
                    .is_some_and(|v| v["category"] == "Sub")
                    .then_some(w)
            })
            .expect("native raw source checkpoint before gateway test");
        let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
        let action: c::Action = serde_json::from_value(warning(cid(&w), P, r)).unwrap();
        let before = serde_json::to_value(&w).unwrap();
        if compatible {
            c::validate_action(&w, Side::Blue, s, &action).unwrap();
            assert_eq!(before, serde_json::to_value(&w).unwrap());
            let contact = cid(&w);
            send(&mut w, s, warning(contact, P, r));
            assert_eq!(
                c::cue_evidence(&w, r, t, c::CuePurpose::Acoustic).modifier,
                1
            );
        } else {
            assert!(c::validate_action(&w, Side::Blue, s, &action).is_err());
            assert_eq!(before, serde_json::to_value(&w).unwrap());
            assert_eq!(
                c::cue_evidence(&w, r, t, c::CuePurpose::Acoustic).modifier,
                0
            );
        }
    }
}

#[test]
fn b15_automatic_tdl_shares_a_first_native_report_in_its_actual_detection_tick() {
    let mut config = fits("near_real_time", "near_real_time");
    config["units"]["SENDER"]["fits"]["lab"]["automatic_warning"] = json!(true);
    let w = (1..=64)
        .find_map(|seed| {
            let w = comm_world_config(seed, 1.25, config.clone());
            source_report(&w, id(&w, "SENDER"), P)
                .is_some()
                .then_some(w)
        })
        .expect("first genuine source report checkpoint before automatic delivery assertion");
    let (sender, receiver) = (id(&w, "SENDER"), id(&w, "RECEIVER"));
    let view = c::own_view(&w, receiver).unwrap();
    let message = view["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["sender"] == sender.0)
        .expect("same-batch automatic message, no next-tick or next-Tac delay");
    assert_eq!(message["state"], "delivered");
    assert_eq!(message["queued_at"], w.tick);
    assert_eq!(message["terminal_at"], w.tick);
    assert_eq!(message["report"]["observed_at"], w.tick);
    assert!(message["report_is_live_track"] == false);
}

fn unmanned_world(
    domain: &str,
    mode: &str,
    count: usize,
    endurance: Option<f64>,
) -> Result<World, String> {
    unmanned_world_seed(domain, mode, count, endurance, 19)
}
fn unmanned_world_seed(
    domain: &str,
    mode: &str,
    count: usize,
    endurance: Option<f64>,
    seed: u64,
) -> Result<World, String> {
    let mut db = database();
    db.platforms.get_mut("us_burke_2a").unwrap().sensors.clear();
    db.platforms.get_mut("us_virginia").unwrap().sensors = vec![P.into()];
    let class = if domain == "underwater" {
        "us_virginia"
    } else {
        "batch_lab_receiver"
    };
    let link = if domain == "underwater" {
        "acoustic_modem"
    } else {
        "line_of_sight"
    };
    let mut units = vec![
        json!({"side":"Blue","class":"us_burke_2a","name":"CONTROLLER","pos":[0,0],"speed_kts":0,"fire_on":false}),
    ];
    let mut vehicles = serde_json::Map::new();
    for index in 0..count {
        let name = format!("VEHICLE{index}");
        units.push(json!({"side":"Blue","class":class,"name":name,"pos":[0,index as f64+1.],"course_deg":90,"speed_kts":0,"depth_m":if domain=="underwater"{50.}else{0.},"fire_on":false}));
        vehicles.insert(name,json!({"source":"scenario_assumption: laboratory unmanned conversion; not real platform hardware","domain":domain,"autonomous_capable":true,"initial_mode":mode,"controller":"CONTROLLER","element":"console","initial_link":link,"los_range_nm":100,"antenna_height_m":30,"satellite":true,"satellite_available":true,"acoustic_modem":true,"periscope_depth_m":15,"endurance_s":endurance,"loss_behavior":{"type":"return_to_home","home":"CONTROLLER"}}));
    }
    units.push(json!({"side":"Red","class":"batch_lab_target","name":"TARGET","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}));
    let sc:Scenario=serde_json::from_value(json!({"id":"unmanned-batch-lab","name":"scenario_assumption: explicit experimental control hardware and original hull/weapon data","duration_s":10000,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","unmanned_2026":{"source":"scenario_assumption: exact lab author inputs","controllers":{"CONTROLLER":{"source":"scenario_assumption: one physical control element","elements":["console"],"operational":true,"los_range_nm":100,"antenna_height_m":30,"satellite":true,"satellite_available":true,"acoustic_modem":true,"overt_recall_transmitter":true}},"vehicles":vehicles},"detection_2026":{"environment":environment(),"bindings":{format!("us_virginia:{P}"):sonar(false,4.)},"targets":{"batch_lab_target":{"source":"scenario_assumption: experimental acoustic target","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},"units":units})).unwrap();
    harpoon_core::load_scenario(sc, db)
}
fn private_vehicle(w: &World, id: EntityId) -> Value {
    serde_json::to_value(w.rules.unmanned.as_ref().unwrap()).unwrap()["vehicles"][id.0.to_string()]
        .clone()
}
#[test]
fn u01_source_controller_capacity_is_one_hitl_or_three_autonomous() {
    assert!(unmanned_world("underwater", "autonomous", 3, None).is_ok());
    assert!(unmanned_world("underwater", "autonomous", 4, None).is_err());
    assert!(unmanned_world("underwater", "hitl", 2, None).is_err());
}
#[test]
fn u02_autonomous_takeover_requires_a_real_tactical_turn_and_keeps_reaction_fire_forbidden() {
    let mut w = unmanned_world("underwater", "autonomous", 1, None).unwrap();
    let v = id(&w, "VEHICLE0");
    let controller = id(&w, "CONTROLLER");
    unmanned(
        &mut w,
        v,
        json!({"type":"take_control","controller":controller,"element":"console","link":"acoustic_modem"}),
    );
    assert!(u::can_control(&w, Side::Blue, v).is_err());
    run(&mut w, 180);
    assert!(u::can_control(&w, Side::Blue, v).is_err());
    run(&mut w, 181);
    assert!(u::can_control(&w, Side::Blue, v).is_ok());
    assert!(!u::can_reaction_fire(&w, v));
    assert!(!u::can_attack(&w, v), "modem is steering/basic-data only");
}
#[test]
fn u03_autonomous_mission_commits_at_actual_intermediate_boundary() {
    // A full LOS link carries the uploaded program. A UUV modem may steer but
    // cannot silently become an unlimited mission-data upload route.
    let mut w = unmanned_world("surface", "autonomous", 1, None).unwrap();
    let v = id(&w, "VEHICLE0");
    unmanned(
        &mut w,
        v,
        json!({"type":"plot_mission","waypoints":[{"x":2.,"y":2.}],"speed_kts":0}),
    );
    assert!(w.entity(v).unwrap().waypoints.is_empty());
    run(&mut w, 1799);
    assert!(w.entity(v).unwrap().waypoints.is_empty());
    run(&mut w, 1800);
    assert_eq!(
        w.entity(v).unwrap().waypoints.front(),
        Some(&Vec2::new(2., 2.))
    );
}
#[test]
fn u04_modem_onboard_native_report_is_not_automatically_projected_to_side() {
    let mut w = (1..=64)
        .find_map(|seed| {
            let mut w = unmanned_world_seed("underwater", "hitl", 1, None, seed).unwrap();
            run(&mut w, 2);
            let state = serde_json::to_value(w.rules.unmanned.as_ref().unwrap()).unwrap();
            state["onboard"]
                .as_object()
                .is_some_and(|r| !r.is_empty())
                .then_some(w)
        })
        .expect("genuine native onboard report checkpoint; no report setter");
    let v = id(&w, "VEHICLE0");
    assert!(!u::allow_side_projection(&w, v));
    assert!(w.sides[0].contacts.is_empty());
    assert!(u::telemetry(&w, v).is_none());
    unmanned(&mut w, v, json!({"type":"exchange_basic_data"}));
    let t = u::telemetry(&w, v).unwrap();
    assert!(t.unit.is_none());
    assert_eq!(t.position_at, w.tick);
    assert_eq!(w.sides[0].contacts.len(), 1);
    assert!(u::own_view(&w, id(&w, "CONTROLLER")).unwrap()["basic_data"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["single_contact"]
        .is_object());
    let action: u::Action = serde_json::from_value(json!({"type":"exchange_basic_data"})).unwrap();
    let before = serde_json::to_value(&w).unwrap();
    assert!(u::validate_action(&w, Side::Blue, v, &action).is_err());
    assert_eq!(before, serde_json::to_value(&w).unwrap());
}
#[test]
fn u05_overt_uuv_recall_rolls_once_and_emits_real_source_signal() {
    let mut w = unmanned_world("underwater", "hitl", 1, None).unwrap();
    let v = id(&w, "VEHICLE0");
    let controller = id(&w, "CONTROLLER");
    unmanned(
        &mut w,
        controller,
        json!({"type":"request_radio","vehicle":v,"link":"satellite"}),
    );
    let signals = u::acoustic_command_sources(&w);
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].base50_nm, 5.);
    assert_eq!(signals[0].emitter, controller);
    assert_eq!(w.entity(v).unwrap().ordered_depth_m, 15.);
    let before = serde_json::to_value(&w).unwrap();
    let action: u::Action =
        serde_json::from_value(json!({"type":"request_radio","vehicle":v,"link":"satellite"}))
            .unwrap();
    assert!(u::validate_action(&w, Side::Blue, controller, &action).is_err());
    assert_eq!(before, serde_json::to_value(&w).unwrap());
}
#[test]
fn u06_source_loss_of_command_returns_via_preprogrammed_home_without_enemy_jammer_truth() {
    let mut w = unmanned_world("surface", "hitl", 1, None).unwrap();
    let v = id(&w, "VEHICLE0");
    let home = w.entity(id(&w, "CONTROLLER")).unwrap().pos;
    u::set_interference(
        &mut w,
        v,
        true,
        false,
        None,
        "referee_contract: actual producer interface; not a claimed native EW attack",
    )
    .unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(private_vehicle(&w, v)["mode"], "autonomous");
    assert_eq!(
        u::own_view(&w, v).unwrap()["mode"],
        "hitl",
        "unreceived fallback is not telemetry"
    );
    assert!(!u::can_attack(&w, v));
    assert_eq!(w.entity(v).unwrap().waypoints.front(), Some(&home));
}
#[test]
fn u07_two_link_failures_wander_until_actual_authored_fuel_exhaustion() {
    let mut w = unmanned_world("surface", "hitl", 1, Some(3.)).unwrap();
    let v = id(&w, "VEHICLE0");
    u::set_interference(
        &mut w,
        v,
        true,
        true,
        None,
        "referee_contract: control and navigation independently unavailable",
    )
    .unwrap();
    run(&mut w, 3);
    assert_eq!(w.entity(v).unwrap().endurance_s, Some(0.));
    assert_eq!(w.entity(v).unwrap().ordered_speed_kts, 0.);
    assert!(!u::can_attack(&w, v));
    assert_eq!(private_vehicle(&w, v)["fuel_exhausted"], true);
    assert_eq!(
        u::own_view(&w, v).unwrap()["fuel_exhausted"],
        false,
        "unreceived fuel state stays historical"
    );
}
#[test]
fn u08_complete_telemetry_stays_frozen_after_link_loss_and_committed_damage() {
    let mut w = unmanned_world("surface", "hitl", 1, None).unwrap();
    let v = id(&w, "VEHICLE0");
    run(&mut w, 2);
    let old = u::telemetry(&w, v).unwrap();
    let old_unit = serde_json::to_value(old.unit.clone().unwrap()).unwrap();
    u::set_interference(
        &mut w,
        v,
        true,
        false,
        None,
        "referee_contract: lost telemetry channel",
    )
    .unwrap();
    // An adjudicated real source damage queue exercises committed HP/runtime
    // changes; it is not a fabricated HP setter or a natural missile flight.
    let weapon = w.db.weapons["us_mk48"].clone();
    let prior = w.entity(v).unwrap().hp;
    harpoon_core::damage::apply_hit_at(&mut w, v, &weapon, true, Some(90.));
    assert!(w.entity(v).unwrap().hp < prior);
    harpoon_core::sim::tick(&mut w, &[]);
    let held = u::telemetry(&w, v).unwrap();
    assert_eq!(held.status_at, old.status_at);
    assert_eq!(serde_json::to_value(held.unit.unwrap()).unwrap(), old_unit);
    let observation =
        serde_json::to_value(harpoon_core::observe(&w, Side::Blue, 0, 10000)).unwrap();
    let unit = observation["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == v.0)
        .expect("held own telemetry does not disappear on unseen damage/removal");
    assert_eq!(unit["hp"], old_unit["hp"]);
    assert_eq!(unit["pos"], old_unit["pos"]);
}
#[test]
fn u09_unmanned_mode_pending_recall_and_native_state_roundtrip() {
    let mut w = unmanned_world("underwater", "hitl", 1, None).unwrap();
    let (v, controller) = (id(&w, "VEHICLE0"), id(&w, "CONTROLLER"));
    unmanned(
        &mut w,
        controller,
        json!({"type":"request_radio","vehicle":v,"link":"satellite"}),
    );
    let mut copy: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    for _ in 0..400 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut copy, &[]);
    }
    assert_eq!(
        serde_json::to_value(w).unwrap(),
        serde_json::to_value(copy).unwrap()
    );
}
#[test]
fn u10_uuv_radio_establishment_requires_actual_motion_mast_and_native_source_delay() {
    let mut w = unmanned_world("underwater", "hitl", 1, None).unwrap();
    let (v, controller) = (id(&w, "VEHICLE0"), id(&w, "CONTROLLER"));
    unmanned(
        &mut w,
        controller,
        json!({"type":"request_radio","vehicle":v,"link":"satellite"}),
    );
    assert!(!u::radio_ready(&w, v));
    assert!(!u::allow_side_projection(&w, v));
    run(&mut w, 2300);
    assert!(w.entity(v).unwrap().depth_m <= 15.);
    assert!(u::radio_ready(&w, v));
    assert!(u::allow_side_projection(&w, v));
    assert_eq!(u::own_view(&w, v).unwrap()["link"], "satellite");
    assert!(u::telemetry(&w, v).unwrap().unit.is_some());
}
#[test]
fn u11_controller_roster_can_name_a_modem_uuv_without_disclosing_unreported_telemetry() {
    let w = unmanned_world("underwater", "hitl", 1, None).unwrap();
    let (v, controller) = (id(&w, "VEHICLE0"), id(&w, "CONTROLLER"));
    assert!(u::telemetry(&w, v).is_none());
    let view = u::own_view(&w, controller).unwrap();
    let item = &view["controlled"][0];
    assert_eq!(item["unit"], v.0);
    assert!(item.get("pos").is_none());
    assert!(item.get("hp").is_none());
    assert!(item.get("contacts").is_none());
    assert!(item.get("mode").is_none());
    assert!(item.get("link").is_none());
}

#[test]
fn u12_actual_control_switch_releases_old_assignments_then_rebuilds_three_autonomous_bindings() {
    let mut w = unmanned_world("underwater", "autonomous", 3, None).unwrap();
    let controller = id(&w, "CONTROLLER");
    let vehicles = [id(&w, "VEHICLE0"), id(&w, "VEHICLE1"), id(&w, "VEHICLE2")];
    unmanned(
        &mut w,
        vehicles[0],
        json!({"type":"take_control","controller":controller,"element":"console","link":"acoustic_modem"}),
    );
    run(&mut w, 181);
    assert!(u::can_control(&w, Side::Blue, vehicles[0]).is_ok());
    assert_eq!(
        u::own_view(&w, controller).unwrap()["controlled"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    for vehicle in &vehicles[1..] {
        assert!(private_vehicle(&w, *vehicle)["controller"].is_null());
    }
    unmanned(
        &mut w,
        vehicles[0],
        json!({"type":"switch_mode","mode":"autonomous"}),
    );
    assert!(u::can_control(&w, Side::Blue, vehicles[0]).is_err());
    for vehicle in &vehicles[1..] {
        let executed_at = w.tick + 1;
        unmanned(
            &mut w,
            *vehicle,
            json!({"type":"assign_autonomous","controller":controller,"element":"console","link":"acoustic_modem"}),
        );
        assert_eq!(
            u::own_view(&w, controller).unwrap()["controlled"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        run(&mut w, executed_at + 180);
        assert_eq!(private_vehicle(&w, *vehicle)["mode"], "autonomous");
        assert_eq!(private_vehicle(&w, *vehicle)["controller"], controller.0);
        assert!(
            u::own_view(&w, *vehicle).is_none(),
            "modem binding does not transmit complete status"
        );
    }
    assert_eq!(
        u::own_view(&w, controller).unwrap()["controlled"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn u13_lost_control_does_not_accept_operator_switch_or_publish_automatic_mode_through_roster() {
    let mut w = unmanned_world("surface", "hitl", 1, None).unwrap();
    let (vehicle, controller) = (id(&w, "VEHICLE0"), id(&w, "CONTROLLER"));
    u::set_interference(
        &mut w,
        vehicle,
        true,
        false,
        None,
        "referee_contract: unavailable link; not a native EW attack",
    )
    .unwrap();
    let action: u::Action =
        serde_json::from_value(json!({"type":"switch_mode","mode":"autonomous"})).unwrap();
    let before = serde_json::to_value(&w).unwrap();
    assert!(u::validate_action(&w, Side::Blue, vehicle, &action).is_err());
    assert_eq!(before, serde_json::to_value(&w).unwrap());
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(private_vehicle(&w, vehicle)["mode"], "autonomous");
    assert_eq!(
        u::own_view(&w, vehicle).unwrap()["mode"],
        "hitl",
        "automatic fallback is unreceived status"
    );
    let roster = u::own_view(&w, controller).unwrap();
    assert!(roster["controlled"][0].get("mode").is_none());
    assert!(roster["controlled"][0].get("link").is_none());
}

#[test]
fn u14_modem_allows_native_vertical_steering_but_rejects_a_bulk_mission_program() {
    let mut w = unmanned_world("underwater", "hitl", 1, None).unwrap();
    let vehicle = id(&w, "VEHICLE0");
    let program: u::Action = serde_json::from_value(
        json!({"type":"plot_mission","waypoints":[{"x":1,"y":1},{"x":2,"y":2}],"speed_kts":0}),
    )
    .unwrap();
    let before = serde_json::to_value(&w).unwrap();
    assert!(u::validate_action(&w, Side::Blue, vehicle, &program).is_err());
    assert_eq!(before, serde_json::to_value(&w).unwrap());
    let steering = Command::SetDepth {
        unit: vehicle,
        depth_m: 75.,
    };
    steering
        .validate_source(&w, Side::Blue)
        .expect("basic acoustic steering includes depth; no fire/control-history access");
    assert_eq!(before, serde_json::to_value(&w).unwrap());
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, steering)]);
    assert_eq!(w.entity(vehicle).unwrap().ordered_depth_m, 75.);
}

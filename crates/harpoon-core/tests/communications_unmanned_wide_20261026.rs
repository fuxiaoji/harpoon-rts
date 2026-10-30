//! Wide batch source regressions. Candidate only; root registers and runs once.
//! Existing support copied as text, never includes/runs old tests or writers.
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
fn poor_remote_fixture(seed: u64) -> World {
    let bindings = std::collections::BTreeMap::from([
        (format!("us_virginia:{P}"), sonar(false, 4.)),
        (format!("us_virginia:{A}"), sonar(true, 20.)),
    ]);
    let sc:Scenario=serde_json::from_value(json!({"id":"native-poor-remote-own-good","name":"Sourced Virginia CS and original Mk48 stocks / explicit acoustic experiment","duration_s":10000,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","communications_2026":fits("real_time","near_real_time"),"detection_2026":{"environment":environment(),"bindings":bindings,"targets":{"batch_lab_target":{"source":"scenario_assumption: controlled acoustic target","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},"units":[
        {"side":"Blue","class":"us_virginia","name":"SENDER","pos":[0,0],"depth_m":50,"speed_kts":0,"fire_on":false},
        {"side":"Blue","class":"us_virginia","name":"RECEIVER","pos":[0,16],"depth_m":50,"speed_kts":0,"fire_on":false},
        {"side":"Red","class":"batch_lab_target","name":"TARGET","pos":[0,17.5],"course_deg":90,"depth_m":50,"speed_kts":4,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, database()).unwrap();
    let (s, r) = (id(&w, "SENDER"), id(&w, "RECEIVER"));
    harpoon_core::sim::tick(
        &mut w,
        &[
            (
                Side::Blue,
                Command::SetSonar {
                    unit: s,
                    active: true,
                },
            ),
            (
                Side::Blue,
                Command::SetSonar {
                    unit: r,
                    active: true,
                },
            ),
        ],
    );
    run(&mut w, 181);
    // With CS6 and the original Mk48 Gen4 seeker, one turn gives the distant
    // source Fair. Use an actual target course change and a second native
    // motion window to earn the printed zig penalty, producing Poor remotely
    // while the close receiver still has Good. No quality/zig state is set.
    let target = id(&w, "TARGET");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Red,
            Command::SetCourse {
                unit: target,
                course_deg: 0.,
            },
        )],
    );
    run(&mut w, 361);
    w
}

// PDF70 §6.3.1 explicitly orders four quality levels. PDF74 §6.3.9
// reduces an NRT-received surface/ASW solution by one level, including Poor.
fn send_native_poor(w: &mut World) -> Option<(EntityId, EntityId, EntityId, usize, f64)> {
    let (s, r, t) = (id(w, "SENDER"), id(w, "RECEIVER"), id(w, "TARGET"));
    let p = &w.rules.torpedoes.as_ref().unwrap().config.weapons["us_mk48"];
    let (generation, speed) = (p.generation, p.settings[0].speed_kts);
    let (Ok(src), Ok(own)) = (
        f::own_sensor_solution(w, s, t, A, generation, speed),
        f::own_sensor_solution(w, r, t, A, generation, speed),
    ) else {
        return None;
    };
    if src.quality != f::Quality::Poor || own.quality != f::Quality::Good {
        return None;
    }
    let action:c::Action=serde_json::from_value(json!({"type":"send_solution","contact":cid(w),"sensor":A,"fit":"lab","receivers":[r],"weapon":"us_mk48","speed_index":0})).unwrap();
    c::apply(w, Side::Blue, s, &action).unwrap();
    c::detection_close(w);
    Some((s, r, t, generation, speed))
}
#[test]
fn wide_c01_nrt_native_poor_degrades_to_the_printed_fourth_level_no_attack() {
    for seed in 1..=64 {
        let mut w = poor_remote_fixture(seed);
        let Some((_, r, t, _, _)) = send_native_poor(&mut w) else {
            continue;
        };
        let before = serde_json::to_value(&w).unwrap();
        let received = c::received_solution(&w, r, t)
            .expect("6.3.1 four printed levels resolve the former artificial Poor-floor pending")
            .expect("the real delivered NRT solution must retain explicit NA evidence");
        assert_eq!(received.quality, f::Quality::NA);
        assert_eq!(received.solution.as_ref().unwrap().quality, f::Quality::NA);
        assert_eq!(
            before,
            serde_json::to_value(&w).unwrap(),
            "quality read is pure"
        );
        return;
    }
    panic!("native Poor source / Good receiver checkpoint unavailable; fixture error");
}
#[test]
fn wide_c02_nrt_no_attack_evidence_never_downgrades_a_real_native_good_solution() {
    for seed in 1..=64 {
        let mut w = poor_remote_fixture(seed);
        let Some((_, r, t, generation, speed)) = send_native_poor(&mut w) else {
            continue;
        };
        assert_eq!(
            c::received_solution(&w, r, t).unwrap().unwrap().quality,
            f::Quality::NA
        );
        assert_eq!(
            f::world_solution(&w, r, t, generation, speed)
                .unwrap()
                .quality,
            f::Quality::Good
        );
        let attack = f::attack_solution(&w, r, t, generation, speed).unwrap();
        assert_eq!(attack.quality, f::Quality::Good);
        assert!(attack.reported_aim.is_none());
        return;
    }
    panic!("native Poor source / Good receiver checkpoint unavailable; fixture error");
}
#[test]
fn wide_u01_separate_control_and_navigation_jam_causes_combine_in_actual_fallback() {
    let mut w = unmanned_world("surface", "hitl", 1, Some(900.)).unwrap();
    let v = id(&w, "VEHICLE0");
    u::set_interference(&mut w, v, true, false, None, "referee_contract:control-A").unwrap();
    u::set_interference(
        &mut w,
        v,
        false,
        true,
        None,
        "referee_contract:navigation-B",
    )
    .unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(
        private_vehicle(&w, v)["loss"],
        "Wandering",
        "2.5.1 two simultaneous independently sourced jams cannot overwrite each other"
    );
    assert!(u::can_control(&w, Side::Blue, v).is_err());
    assert!(!u::detection_allowed(&w, v));
}
#[test]
fn wide_u02_clearing_one_jammer_cause_does_not_repair_another_active_control_cause() {
    let mut w = unmanned_world("surface", "hitl", 1, None).unwrap();
    let v = id(&w, "VEHICLE0");
    for source in ["referee_contract:jam-A", "referee_contract:jam-B"] {
        u::set_interference(&mut w, v, true, false, None, source).unwrap();
    }
    u::set_interference(&mut w, v, false, false, None, "referee_contract:jam-A").unwrap();
    assert!(
        u::can_control(&w, Side::Blue, v).is_err(),
        "B remains active before the actual phase"
    );
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(private_vehicle(&w, v)["loss"], "AutonomousRecovery");
}
#[test]
fn wide_u03_multiple_cause_fallback_roundtrips_with_original_wander_draw_and_real_fuel() {
    let mut w = unmanned_world("surface", "hitl", 1, Some(4000.)).unwrap();
    let v = id(&w, "VEHICLE0");
    u::set_interference(&mut w, v, true, false, None, "referee_contract:control-A").unwrap();
    u::set_interference(
        &mut w,
        v,
        false,
        true,
        None,
        "referee_contract:navigation-B",
    )
    .unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(private_vehicle(&w, v)["loss"], "Wandering");
    let encoded = serde_json::to_value(&w).unwrap();
    let mut restored: World = serde_json::from_value(encoded).unwrap();
    run(&mut w, 5);
    run(&mut restored, 5);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert_eq!(w.entity(v).unwrap().endurance_s, Some(3995.));
    assert!(!u::can_attack(&w, v));
}

fn radio_world(seed: u64, at_pd: bool) -> World {
    radio_world_frequency(seed, at_pd, "lmf")
}
fn radio_world_frequency(seed: u64, at_pd: bool, frequency: &str) -> World {
    let mut db = database();
    db.platforms.get_mut("us_burke_2a").unwrap().sensors.clear();
    let mut vehicle = db.platforms["us_virginia"].clone();
    vehicle.id = "wide_uuv_body".into();
    vehicle.name = "scenario_assumption: original Virginia body/stock UUV radio lab".into();
    vehicle.sensors.clear();
    db.platforms.insert(vehicle.id.clone(), vehicle);
    let mut listener = db.platforms["us_virginia"].clone();
    listener.id = "wide_listener".into();
    listener.name = "scenario_assumption: original body LMF interception lab".into();
    listener.sensors = vec![P.into()];
    db.platforms.insert(listener.id.clone(), listener);
    db.validate().unwrap();
    let mut listener_binding = sonar(false, 4.);
    listener_binding["frequency"] = json!(frequency);
    let sc:Scenario=serde_json::from_value(json!({
      "id":"wide-uuv-radio-source","name":"scenario_assumption: explicit mast and control fittings; original body/stocks",
      "seed":seed,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","unmanned_2026":{
        "source":"scenario_assumption: independent radio establishment experiment",
        "controllers":{"CONTROLLER":{"source":"scenario_assumption: exact console/radio fit","elements":["console"],"operational":true,"los_range_nm":100,"antenna_height_m":30,"satellite":true,"satellite_available":true,"acoustic_modem":true,"overt_recall_transmitter":true}},
        "vehicles":{"VEHICLE0":{"source":"scenario_assumption: original hull used as explicit UUV lab","domain":"underwater","autonomous_capable":true,"initial_mode":"hitl","controller":"CONTROLLER","element":"console","initial_link":"acoustic_modem","los_range_nm":100,"antenna_height_m":3,"satellite":true,"satellite_available":true,"acoustic_modem":true,"periscope_depth_m":15,"loss_behavior":{"type":"return_to_home","home":"CONTROLLER"}}}},
        "detection_2026":{"environment":environment(),"bindings":{format!("wide_listener:{P}"):listener_binding},
          "max_masts":{"wide_uuv_body":1},"targets":{"us_burke_2a":{"source":"scenario_assumption: controlled Extremely Quiet sender signature","size":2,"noise":4,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
      "units":[
        {"side":"Blue","class":"us_burke_2a","name":"CONTROLLER","pos":[0,0],"speed_kts":0,"fire_on":false},
        {"side":"Blue","class":"wide_uuv_body","name":"VEHICLE0","pos":[0,1],"depth_m":if at_pd{15.}else{50.},"speed_kts":0,"fire_on":false},
        {"side":"Red","class":"wide_listener","name":"LISTENER","pos":[4,0],"depth_m":50,"speed_kts":0,"fire_on":false}]})).unwrap();
    harpoon_core::load_scenario(sc, db).unwrap()
}
#[test]
fn wide_u04_actual_raised_mast_establishes_radio_after_one_turn_without_fabricated_overt_recall() {
    let mut w = radio_world(19, true);
    let v = id(&w, "VEHICLE0");
    harpoon_core::rules::detection2026::apply(
        &mut w,
        v,
        &harpoon_core::rules::detection2026::DetectionAction::Masts { count: 1 },
    )
    .unwrap();
    run(&mut w, 179);
    assert!(!u::radio_ready(&w, v));
    run(&mut w, 180);
    assert!(
        u::radio_ready(&w, v),
        "PDF22 direct actual PD/mast establishment is not conditional on a prior 2D6 overt recall"
    );
    assert!(u::acoustic_command_sources(&w).is_empty());
    let action: u::Action =
        serde_json::from_value(json!({"type":"set_link","link":"satellite"})).unwrap();
    u::validate_action(&w, Side::Blue, v, &action).unwrap();
}
fn recalled_radio() -> World {
    let mut w = radio_world(19, true);
    let controller = id(&w, "CONTROLLER");
    let v = id(&w, "VEHICLE0");
    unmanned(
        &mut w,
        controller,
        json!({"type":"request_radio","vehicle":v,"link":"satellite"}),
    );
    let due = private_vehicle(&w, v)["recall"]["ready_at"]
        .as_i64()
        .unwrap();
    run(&mut w, due.max(181));
    assert!(u::radio_ready(&w, v), "actual recall completion checkpoint");
    w
}
#[test]
fn wide_u05_successful_uuv_recall_commits_a_real_native_mast_producer() {
    let w = recalled_radio();
    let v = id(&w, "VEHICLE0");
    assert!(
        w.rules
            .detection
            .as_ref()
            .unwrap()
            .masts
            .get(&v)
            .is_some_and(|m| m.count > 0),
        "private mast_since cannot bypass the real radar/visual mast consumer"
    );
}
#[test]
fn wide_u06_lowering_the_actual_mast_closes_radio_before_any_new_telemetry() {
    let mut w = recalled_radio();
    let v = id(&w, "VEHICLE0");
    harpoon_core::rules::detection2026::apply(
        &mut w,
        v,
        &harpoon_core::rules::detection2026::DetectionAction::Masts { count: 0 },
    )
    .unwrap();
    assert!(
        !u::radio_ready(&w, v),
        "actual mast lowered, not a permanent private readiness bit"
    );
    assert!(!u::allow_side_projection(&w, v));
}
fn native_listener_has_sender(w: &World) -> bool {
    let (listener, sender) = (id(w, "LISTENER"), id(w, "CONTROLLER"));
    w.rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .any(|t| {
            t.observer == listener
                && t.target == sender
                && t.report.as_ref().is_some_and(|r| {
                    r.sensor == P
                        && r.sensor_kind == harpoon_core::db::SensorKind::SonarPassive
                        && r.at == w.tick
                })
        })
}
#[test]
fn wide_u07_overt_lmf_recall_is_consumed_by_actual_enemy_passive_sonar() {
    let mut checked_unheard_control = false;
    for seed in 1..=128 {
        let mut control = radio_world(seed, false);
        harpoon_core::sim::tick(&mut control, &[]);
        if native_listener_has_sender(&control) {
            continue;
        }
        checked_unheard_control = true;
        let mut signal = radio_world(seed, false);
        let controller = id(&signal, "CONTROLLER");
        let v = id(&signal, "VEHICLE0");
        unmanned(
            &mut signal,
            controller,
            json!({"type":"request_radio","vehicle":v,"link":"satellite"}),
        );
        assert_eq!(u::acoustic_command_sources(&signal).len(), 1);
        if native_listener_has_sender(&signal) {
            assert!(signal.sides[Side::Red.index()]
                .contacts
                .values()
                .any(|c| c.about == controller));
            return;
        }
    }
    assert!(
        checked_unheard_control,
        "ordinary-noise fixture failed to isolate a command signal"
    );
    panic!("PDF22 LMF5nm overt command exists, but no real enemy native SensorReport consumes it");
}
fn listener_sender_report(w: &World) -> Option<Value> {
    let (listener, sender) = (id(w, "LISTENER"), id(w, "CONTROLLER"));
    w.rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .filter(|t| t.observer == listener && t.target == sender)
        .filter_map(|t| t.report.as_ref())
        .find(|r| r.sensor == P)
        .map(|r| serde_json::to_value(r).unwrap())
}
fn listener_sender_scan(w: &World) -> Value {
    let (listener, sender) = (id(w, "LISTENER"), id(w, "CONTROLLER"));
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
            } if domain == "detection2026" && rule == "5.scan" && *unit == listener => {
                let value: Value = serde_json::from_str(detail).unwrap();
                (value["sensor"] == P && value["truth_target"] == sender.0)
                    .then(|| value["result"].clone())
            }
            _ => None,
        })
        .expect("actual native listener/sender probability trace checkpoint")
}
#[test]
fn wide_u08_lmf_overt_command_never_improves_incompatible_mf_hf_or_vlf_listener() {
    for frequency in ["mf", "hf", "vlf"] {
        for seed in 1..=8 {
            let mut control = radio_world_frequency(seed, false, frequency);
            let mut signal = radio_world_frequency(seed, false, frequency);
            let controller = id(&signal, "CONTROLLER");
            let v = id(&signal, "VEHICLE0");
            harpoon_core::sim::tick(&mut control, &[]);
            unmanned(
                &mut signal,
                controller,
                json!({"type":"request_radio","vehicle":v,"link":"satellite"}),
            );
            assert_eq!(listener_sender_report(&control),listener_sender_report(&signal),
                "PDF55 same or immediately lower receive band; LMF command cannot improve {frequency}");
        }
    }
}
#[test]
fn wide_u09_expired_overt_command_does_not_grant_a_new_native_sonar_epoch() {
    for seed in 1..=8 {
        let mut control = radio_world(seed, false);
        let mut signal = radio_world(seed, false);
        let controller = id(&signal, "CONTROLLER");
        let v = id(&signal, "VEHICLE0");
        unmanned(
            &mut signal,
            controller,
            json!({"type":"request_radio","vehicle":v,"link":"satellite"}),
        );
        run(&mut control, 360);
        run(&mut signal, 360);
        assert!(u::acoustic_command_sources(&signal).is_empty());
        let a = listener_sender_scan(&control)["inputs"].clone();
        let b = listener_sender_scan(&signal)["inputs"].clone();
        assert!(a["target_modifier"].is_number() && b["target_modifier"].is_number());
        // A genuine earlier detection may legitimately leave an alerted
        // operator +15. Only reject a new active-emission signature/range boost.
        for field in [
            "base50_nm",
            "target_modifier",
            "relative_speed",
            "environment",
            "layer",
            "total",
            "modified50_nm",
        ] {
            assert_eq!(
                a[field], b[field],
                "spent command cannot grant new {field} at the later native epoch"
            );
        }
    }
}

fn launched_world(declared: bool) -> World {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let fit = json!({"source":"scenario_assumption: original F18 body/stock converted solely for unmanned launch mechanism test","domain":"air","autonomous_capable":true,"initial_mode":"autonomous","controller":"CV","element":"console","initial_link":"line_of_sight","los_range_nm":100,"antenna_height_m":3,"loss_behavior":{"type":"return_to_home","home":"CV"}});
    let launch = if declared {
        json!({"us_f18e":fit})
    } else {
        json!({})
    };
    let sc:Scenario=serde_json::from_value(json!({"id":"wide-native-unmanned-launch","name":"scenario_assumption: native launch / explicit unmanned hardware","duration_s":10000,"seed":23,"context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":3,"precipitation":"None"},"air_wings":{"CV":[{"class":"us_f18e","count":8}]},"aviation_2026":{"source":"scenario_assumption: mechanism load only","loads":{"us_f18e":{"payload_kg":1000,"tanks":[],"source":"scenario_assumption: explicit existing airframe load"}},"parking":{"CV":0}},"unmanned_2026":{"source":"scenario_assumption: exact launch hardware","controllers":{"CV":{"source":"scenario_assumption: source console and LOS fit","elements":["console"],"operational":true,"los_range_nm":100,"antenna_height_m":30}},"launch_classes":launch}},"weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"us_ford","name":"CV","pos":[0,0],"course_deg":0,"speed_kts":32,"fire_on":false}]})).unwrap();
    harpoon_core::load_scenario(sc, db).unwrap()
}
fn actual_launch(w: &mut World) -> EntityId {
    let cv = id(w, "CV");
    harpoon_core::air::launch_aircraft(
        w,
        cv,
        "us_f18e",
        1,
        &harpoon_core::commands::AirMission::Recon {
            point: Vec2::new(0., 10.),
        },
    )
    .unwrap();
    let before = w
        .entities
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    for _ in 0..1200 {
        harpoon_core::sim::tick(w, &[]);
        if let Some(id) = w.entities.keys().find(|id| !before.contains(id)).copied() {
            return id;
        }
    }
    panic!("actual source launch failed to produce an aircraft; fixture error");
}
#[test]
fn wide_u10_real_native_launch_registers_declared_hardware_before_any_attack_phase() {
    let mut w = launched_world(true);
    let v = actual_launch(&mut w);
    assert!(u::is_configured(&w, v));
    assert_eq!(private_vehicle(&w, v)["mode"], "autonomous");
    assert!(!u::can_attack(&w, v));
    assert!(!u::can_reaction_fire(&w, v));
    let def = &w.db.platforms["us_f18e"];
    let body = w.entity(v).unwrap();
    assert_eq!(body.hp, def.hp);
    for mount in &body.weapons {
        let source = def
            .weapons
            .iter()
            .find(|m| m.weapon == mount.weapon)
            .unwrap();
        assert_eq!(mount.ammo, source.ammo * u16::from(source.mounts));
    }
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    let to = w.tick + 2;
    run(&mut w, to);
    run(&mut restored, to);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
#[test]
fn wide_u11_ordinary_native_launch_stays_manned_without_an_explicit_fit() {
    let mut w = launched_world(false);
    let v = actual_launch(&mut w);
    assert!(!u::is_configured(&w, v));
    assert!(u::can_reaction_fire(&w, v));
}
#[test]
fn wide_u12_native_launch_over_declared_control_capacity_rejects_before_queue_or_stock() {
    let mut w = launched_world(true);
    let cv = id(&w, "CV");
    let before = serde_json::to_value(&w).unwrap();
    assert!(harpoon_core::air::launch_aircraft(
        &mut w,
        cv,
        "us_f18e",
        4,
        &harpoon_core::commands::AirMission::Recon {
            point: Vec2::new(0., 10.)
        }
    )
    .is_err());
    assert_eq!(before, serde_json::to_value(&w).unwrap());
}
#[test]
fn wide_u13_hidden_real_uuv_casualty_cannot_change_broadcast_admission_or_sender_observation() {
    let mut dead = radio_world(17, false);
    let v = id(&dead, "VEHICLE0");
    let actor = id(&dead, "CONTROLLER");
    let weapon = dead.db.weapons["us_mk48"].clone();
    for _ in 0..20 {
        if !dead.entity(v).is_some_and(|e| e.alive) {
            break;
        }
        harpoon_core::damage::apply_hit_at(&mut dead, v, &weapon, true, Some(90.));
        harpoon_core::sim::tick(&mut dead, &[]);
    }
    assert!(
        !dead.entity(v).is_some_and(|e| e.alive),
        "actual committed hidden casualty fixture"
    );
    let mut alive = radio_world(17, false);
    run(&mut alive, dead.tick);
    let obs =
        |w: &World| serde_json::to_value(harpoon_core::observe(w, Side::Blue, 0, 4096)).unwrap();
    assert_eq!(
        obs(&alive),
        obs(&dead),
        "same sender information before public command"
    );
    let action: u::Action =
        serde_json::from_value(json!({"type":"request_radio","vehicle":v,"link":"satellite"}))
            .unwrap();
    u::validate_action(&alive, Side::Blue, actor, &action).unwrap();
    u::validate_action(&dead, Side::Blue, actor, &action).unwrap();
    u::apply(&mut alive, Side::Blue, actor, &action).unwrap();
    u::apply(&mut dead, Side::Blue, actor, &action).unwrap();
    let mut visible_alive = obs(&alive);
    let mut visible_dead = obs(&dead);
    // integration-1 full structural audit found exactly /events/1/seq 4 vs30:
    // the global opaque resume cursor counts referee events. Preserve every
    // event content/game field and normalize only that transport audit cursor.
    for observation in [&mut visible_alive, &mut visible_dead] {
        for event in observation["events"].as_array_mut().unwrap() {
            assert!(event["seq"].is_number());
            event["seq"] = json!(0);
        }
    }
    assert_eq!(visible_alive,visible_dead,
        "broadcast is a commitment, not confirmation of hidden reception; only opaque audit cursor normalized");
    assert_eq!(u::acoustic_command_sources(&alive).len(), 1);
    assert_eq!(u::acoustic_command_sources(&dead).len(), 1);
}
#[test]
fn wide_u14_broadcast_in_a_new_window_never_rerolls_an_existing_hidden_recall() {
    let mut w = radio_world(19, false);
    let v = id(&w, "VEHICLE0");
    let actor = id(&w, "CONTROLLER");
    unmanned(
        &mut w,
        actor,
        json!({"type":"request_radio","vehicle":v,"link":"satellite"}),
    );
    let old = private_vehicle(&w, v)["recall"].clone();
    run(&mut w, 181);
    let action: u::Action =
        serde_json::from_value(json!({"type":"request_radio","vehicle":v,"link":"satellite"}))
            .unwrap();
    u::apply(&mut w, Side::Blue, actor, &action).unwrap();
    assert_eq!(private_vehicle(&w, v)["recall"], old);
}

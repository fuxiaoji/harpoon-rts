//! Shared-consumer candidates (implemented_unverified): real native sensor reports and actual source command/phase
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

fn radar_scenario(seed: u64, kind: &str, esm: bool) -> Scenario {
    let mut bindings = serde_json::Map::new();
    if esm {
        for class in ["us_ford", "us_nimitz"] {
            bindings.insert(format!("{class}:esm_std"),json!({"type":"es","source":"scenario_assumption: explicit experimental ES fit; no real Annex ES claim","generation":5,"targeting_complex":false,"rwr":false,"bearing_error_deg":2.,"els":false}));
        }
    }
    let emissions = if esm {
        json!({"TARGET":{
            "source":"scenario_assumption: explicit experimental continuously emitting AI targeting mode; not actual F18 hardware parameters",
            "sensor":"radar_air_med","weapon_radar":true,"lpi":false,"radar_role":"AI","max_range_nm":80.,"antenna_height_m":3000.
        }})
    } else {
        json!({})
    };
    serde_json::from_value(json!({"id":"shared-native-radar-cue","name":"Original radar profiles / explicit communication and ES experiment","duration_s":10000,"seed":seed,
        "weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},"communications_2026":fits(kind,kind),
            "detection_2026":{"environment":environment(),"bindings":bindings,"emissions":emissions}},"units":[
            {"side":"Blue","class":"us_ford","name":"SENDER","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":"us_nimitz","name":"RECEIVER","pos":[0,-1],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_f18e","name":"TARGET","pos":[0,5],"course_deg":90,"speed_kts":300,"altitude_m":3000,"fire_on":false}]})).unwrap()
}
fn radar_world(seed: u64, kind: &str, esm: bool) -> World {
    let mut w = harpoon_core::load_scenario(
        radar_scenario(seed, kind, esm),
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap(),
    )
    .unwrap();
    let r = id(&w, "RECEIVER");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetRadar {
                unit: r,
                active: false,
            },
        )],
    );
    run(&mut w, 91);
    assert!(
        source_report(&w, id(&w, "SENDER"), "radar_air_long").is_some(),
        "actual source native radar checkpoint"
    );
    w
}
fn clock(
    w: &World,
    receiver: EntityId,
    target: EntityId,
) -> harpoon_core::rules::operations::RadarTrack {
    w.rules.operations.as_ref().unwrap().tracks
        [&format!("{}:radar_air_long:{}", receiver.0, target.0)]
        .clone()
}
fn clock_trace(w: &World, receiver: EntityId) -> Value {
    w.events
        .iter()
        .rev()
        .find_map(|e| match &e.kind {
            EventKind::RulesEvent {
                domain,
                rule,
                unit: Some(u),
                detail,
                ..
            } if domain == "operations" && rule == "5.2.1.reaction_clock" && *u == receiver => {
                let v: Value = serde_json::from_str(detail).unwrap();
                (v["sensor"] == "radar_air_long").then_some(v)
            }
            _ => None,
        })
        .expect("actual native reaction-clock trace")
}
fn radar_on_and_warn(w: &mut World, sensor: &str, with_warning: bool) {
    let (s, r) = (id(w, "SENDER"), id(w, "RECEIVER"));
    let mut cmds = vec![(
        Side::Blue,
        Command::SetRadar {
            unit: r,
            active: true,
        },
    )];
    if with_warning {
        cmds.push((
            Side::Blue,
            command(json!({"cmd":"communications","unit":s,"action":warning(cid(w),sensor,r)})),
        ));
    }
    harpoon_core::sim::tick(w, &cmds);
}

#[test]
fn shared01_real_native_radar_tdl_changes_reaction_table_without_a_new_raw_roll() {
    for (kind, modification) in [("near_real_time", 1), ("real_time", 2)] {
        let mut checked = false;
        for seed in 1..=64 {
            let mut shared = radar_world(seed, kind, true);
            let (r, t) = (id(&shared, "RECEIVER"), id(&shared, "TARGET"));
            let mut control = shared.clone();
            radar_on_and_warn(&mut control, "radar_air_long", false);
            radar_on_and_warn(&mut shared, "radar_air_long", true);
            let a = clock(&control, r, t);
            let b = clock(&shared, r, t);
            let trace = clock_trace(&shared, r);
            assert_eq!(a.roll, b.roll);
            assert_eq!(trace["2d6"], b.roll);
            assert_eq!(trace["own_es_modifier"], 1);
            assert_eq!(trace["tdl_modifier"], modification);
            assert_eq!(trace["modified_2d6"], (b.roll + 1 + modification).min(12));
            assert!(!trace["tdl_refs"].as_array().unwrap().is_empty());
            let (extra, normal) = harpoon_core::rules::operations::reaction(
                6,
                true,
                (b.roll + 1 + modification).min(12),
            )
            .unwrap();
            assert_eq!(b.recognized, shared.tick + extra.max(0) * 30);
            assert_eq!(b.engage, shared.tick + (normal + extra).max(0) * 30);
            if b.recognized == a.recognized && b.engage == a.engage {
                continue;
            }
            let before = witness_clock_events(&shared, r);
            harpoon_core::sim::tick(&mut shared, &[]);
            let after = clock(&shared, r, t);
            assert_eq!(after.roll, b.roll);
            assert_eq!(after.seen, b.seen);
            assert_eq!(witness_clock_events(&shared, r), before);
            checked = true;
            break;
        }
        assert!(
            checked,
            "natural source table boundary fixture absent; do not inject clock or dice"
        );
    }
}
fn witness_clock_events(w: &World, r: EntityId) -> usize {
    w.events
        .iter()
        .filter(|e| {
            matches!(&e.kind,EventKind::RulesEvent{domain,rule,unit:Some(u),..}
        if domain=="operations"&&rule=="5.2.1.reaction_clock"&&*u==r)
        })
        .count()
}

#[test]
fn shared02_passive_shared_es_cannot_become_an_active_sensor_aaw_cue() {
    let mut w = radar_world(19, "real_time", true);
    let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
    assert!(
        source_report(&w, s, "esm_std").is_some(),
        "actual ES emission interception checkpoint"
    );
    radar_on_and_warn(&mut w, "esm_std", true);
    assert_eq!(
        c::cue_evidence(&w, r, t, c::CuePurpose::AirDefenseReaction).modifier,
        0
    );
    assert_eq!(c::cue_evidence(&w, r, t, c::CuePurpose::Rf).modifier, 2);
    let trace = clock_trace(&w, r);
    assert_eq!(trace["own_es_modifier"], 1);
    assert_eq!(trace["tdl_modifier"], 0);
    assert!(trace["tdl_refs"].as_array().unwrap().is_empty());
}

#[test]
fn shared03_reaction_modifiers_use_last_source_row_and_keep_the_raw_die_trace() {
    let mut found = false;
    for seed in 1..=64 {
        let mut w = radar_world(seed, "real_time", true);
        let r = id(&w, "RECEIVER");
        radar_on_and_warn(&mut w, "radar_air_long", true);
        let trace = clock_trace(&w, r);
        if trace["2d6"].as_u64().unwrap() < 10 {
            continue;
        }
        assert_eq!(trace["modified_2d6"], 12);
        assert_eq!(trace["own_es_modifier"], 1);
        assert_eq!(trace["tdl_modifier"], 2);
        assert!(w
            .events
            .iter()
            .filter(
                |e| matches!(&e.kind,EventKind::RulesEvent{domain,rule,unit:Some(u),..}
            if domain=="operations"&&rule=="5.2.1.reaction_clock"&&*u==r)
            )
            .all(|e| e.vis == harpoon_core::Visibility::Referee));
        found = true;
        break;
    }
    assert!(found, "natural high 2D6 fixture absent");
}

#[test]
fn shared04_detection_close_delivers_new_native_report_then_next_tick_consumer_uses_cue() {
    let mut cfg = fits("real_time", "real_time");
    cfg["units"]["SENDER"]["fits"]["lab"]["automatic_warning"] = json!(true);
    let mut w = comm_world_config(19, 1.25, cfg);
    let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
    let messages = c::own_view(&w, r).unwrap()["messages"]
        .as_array()
        .unwrap()
        .clone();
    let message = messages
        .iter()
        .find(|m| m["sender"] == s.0)
        .expect("actual same-batch delivery");
    assert_eq!(message["state"], "delivered");
    assert_eq!(message["queued_at"], w.tick);
    assert_eq!(message["terminal_at"], w.tick);
    assert_eq!(message["report"]["observed_at"], w.tick);
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(
        c::cue_evidence(&w, r, t, c::CuePurpose::Acoustic).modifier,
        2
    );
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

#[test]
fn shared05_real_selected_sensor_quality_and_aim_reach_the_common_attack_consumer() {
    let mut w = classified("real_time", "real_time");
    let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
    let p = &w.rules.torpedoes.as_ref().unwrap().config.weapons["us_mk48"];
    let (generation, speed) = (p.generation, p.settings[0].speed_kts);
    let own = f::own_sensor_solution(&w, s, t, A, generation, speed).unwrap();
    let action:c::Action=serde_json::from_value(json!({"type":"send_solution","contact":cid(&w),"sensor":A,"fit":"lab","receivers":[r],"weapon":"us_mk48","speed_index":0})).unwrap();
    c::apply(&mut w, Side::Blue, s, &action).unwrap();
    c::detection_close(&mut w);
    let remote = c::received_solution(&w, r, t).unwrap().unwrap();
    let attack = f::attack_solution(&w, r, t, generation, speed).unwrap();
    assert_eq!(remote.source_sensor, A);
    assert_eq!(remote.quality, own.quality);
    assert_eq!(attack.quality, remote.quality);
    assert_eq!(attack.reported_aim, remote.aim);
    assert_eq!(attack.source["sensor"], A);
    assert_eq!(attack.native.unwrap().modifiers, own.modifiers);
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
#[test]
fn shared06_nrt_poor_becomes_na_and_cannot_veto_a_native_good_own_solution() {
    let mut checked = false;
    for seed in 1..=64 {
        let mut w = poor_remote_fixture(seed);
        let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
        let p = &w.rules.torpedoes.as_ref().unwrap().config.weapons["us_mk48"];
        let (generation, speed) = (p.generation, p.settings[0].speed_kts);
        let (Ok(src), Ok(own)) = (
            f::own_sensor_solution(&w, s, t, A, generation, speed),
            f::own_sensor_solution(&w, r, t, A, generation, speed),
        ) else {
            continue;
        };
        if src.quality != f::Quality::Poor || own.quality != f::Quality::Good {
            continue;
        }
        let action:c::Action=serde_json::from_value(json!({"type":"send_solution","contact":cid(&w),"sensor":A,"fit":"lab","receivers":[r],"weapon":"us_mk48","speed_index":0})).unwrap();
        c::apply(&mut w, Side::Blue, s, &action).unwrap();
        c::detection_close(&mut w);
        assert_eq!(c::received_solution(&w,r,t).unwrap().unwrap().quality,f::Quality::NA,
            "PDF70 four grades and PDF74 lower one level: NRT Poor becomes NA");
        assert_eq!(
            f::world_solution(&w, r, t, generation, speed)
                .unwrap()
                .quality,
            own.quality
        );
        let attack = f::attack_solution(&w, r, t, generation, speed).unwrap();
        assert_eq!(attack.quality, own.quality);
        assert!(attack.reported_aim.is_none());
        checked = true;
        break;
    }
    assert!(
        checked,
        "genuine native Poor/Good fixture checkpoint absent; no FCS injection allowed"
    );
}

fn complete_action_samples() -> Vec<Value> {
    let mut rows = vec![];
    for action in [
        json!({"type":"send_warning","contact":0,"sensor":"actual","fit":"lab","receivers":[2]}),
        json!({"type":"set_automatic","fit":"lab","enabled":true}),
        json!({"type":"abandon_alert","contact":0}),
        json!({"type":"send_solution","contact":0,"sensor":"actual","fit":"lab","receivers":[2],"weapon":"us_mk48","speed_index":0}),
        json!({"type":"combine_reports","contact":0,"own_sensor":"actual","messages":[{"sender":2,"id":1}],"aim_source":{"sender":2,"id":1}}),
    ] {
        rows.push(json!({"cmd":"communications","unit":1,"action":action}));
    }
    for action in [
        json!({"type":"switch_mode","mode":"hitl"}),
        json!({"type":"take_control","controller":2,"element":"console","link":"acoustic_modem"}),
        json!({"type":"assign_autonomous","controller":2,"element":"console","link":"acoustic_modem"}),
        json!({"type":"set_link","link":"satellite"}),
        json!({"type":"plot_mission","waypoints":[{"x":1.,"y":2.}],"speed_kts":2.}),
        json!({"type":"request_radio","vehicle":2,"link":"satellite"}),
        json!({"type":"exchange_basic_data","contact":0,"sensor":"actual"}),
    ] {
        rows.push(json!({"cmd":"unmanned","unit":1,"action":action}));
    }
    for action in [
        json!({"type":"set_yield","weapon":"actual","yield_kt":1}),
        json!({"type":"authorize","weapon":"actual","enabled":true}),
    ] {
        rows.push(json!({"cmd":"nuclear","unit":1,"action":action}));
    }
    for action in [
        json!({"type":"start_charging"}),
        json!({"type":"stop_charging"}),
        json!({"type":"raise_snorkel"}),
        json!({"type":"retract_masts"}),
        json!({"type":"set_aip_operating","enabled":true}),
        json!({"type":"set_power_mode","mode":"battery"}),
    ] {
        rows.push(json!({"cmd":"navigation","unit":1,"action":{"type":"battery","action":action}}));
    }
    rows
}
#[test]
fn shared07_every_extended_action_is_discoverable_typed_and_rejects_private_source_fields() {
    let schema = harpoon_core::commands::command_schema();
    for raw in complete_action_samples() {
        let typed: Command =
            serde_json::from_value(raw.clone()).expect("complete legal external shape");
        assert_eq!(typed.unit_id(), EntityId(1));
        assert_ne!(typed.manual_intent(), Some(true));
        let cmd = raw["cmd"].as_str().unwrap();
        let branch = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["properties"]["cmd"]["const"] == cmd)
            .unwrap();
        assert!(branch["properties"].get("retain_manual").is_none());
        let action = if cmd == "navigation" {
            &raw["action"]["action"]
        } else {
            &raw["action"]
        };
        let actions = if cmd == "navigation" {
            &branch["properties"]["action"]["oneOf"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["properties"]["type"]["const"] == "battery")
                .unwrap()["properties"]["action"]
        } else {
            &branch["properties"]["action"]
        };
        assert!(
            actions["oneOf"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["properties"]["type"]["const"] == action["type"]),
            "missing discoverable shape {raw}"
        );
        for key in [
            "source",
            "d100",
            "success",
            "quality",
            "fcs",
            "world_target",
            "telemetry",
            "cause",
        ] {
            let mut forged = raw.clone();
            let a = if cmd == "navigation" {
                &mut forged["action"]["action"]
            } else {
                &mut forged["action"]
            };
            a[key] = json!(1);
            assert!(
                serde_json::from_value::<Command>(forged).is_err(),
                "private key {key} parsed for {raw}"
            );
        }
    }
}

#[test]
fn shared08_modem_vertical_steering_and_unknown_equipment_use_the_common_readonly_gate() {
    let w = unmanned_world("underwater", "hitl", 1, None).unwrap();
    let v = id(&w, "VEHICLE0");
    let before = serde_json::to_value(&w).unwrap();
    Command::SetDepth {
        unit: v,
        depth_m: 75.,
    }
    .validate_source(&w, Side::Blue)
    .unwrap();
    assert!(Command::SetDepth {
        unit: v,
        depth_m: 75.
    }
    .validate_source(&w, Side::Red)
    .is_err());
    for raw in complete_action_samples().into_iter().filter(|r| {
        r["cmd"] == "nuclear" || r["cmd"] == "communications" || r["cmd"] == "navigation"
    }) {
        let mut raw = raw;
        raw["unit"] = json!(v);
        let cmd: Command = serde_json::from_value(raw).unwrap();
        assert!(
            cmd.validate_source(&w, Side::Blue).is_err(),
            "unknown equipment / modem bulk cannot pass shared source gate"
        );
    }
    assert_eq!(before, serde_json::to_value(&w).unwrap());
}

#[test]
fn shared09_emitter_load_requires_source_unique_unit_and_actual_installed_radar_atomically() {
    let original = serde_json::to_value(radar_scenario(19, "real_time", true)).unwrap();
    for (key, value) in [
        ("source", json!("")),
        ("sensor", json!("missing-radar")),
        ("sensor", json!("esm_std")),
        ("max_range_nm", json!(0.)),
        ("antenna_height_m", json!(-1.)),
        ("radar_role", json!("invented-role")),
    ] {
        let mut raw = original.clone();
        raw["context"]["detection_2026"]["emissions"]["TARGET"][key] = value;
        let sc: Scenario = serde_json::from_value(raw).unwrap();
        assert!(
            harpoon_core::load_scenario(
                sc,
                Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
            )
            .is_err(),
            "invalid emitter source/hardware must reject new_game: {key}"
        );
    }
    let mut forged = original.clone();
    forged["context"]["detection_2026"]["emissions"]["TARGET"]["emitting"] = json!(true);
    assert!(
        serde_json::from_value::<Scenario>(forged).is_err(),
        "author emitting=true is not a source or actual hardware state"
    );
    let mut unknown = original.clone();
    let source = unknown["context"]["detection_2026"]["emissions"]["TARGET"].clone();
    unknown["context"]["detection_2026"]["emissions"] = json!({"ABSENT-UNIT":source});
    assert!(harpoon_core::load_scenario(
        serde_json::from_value(unknown).unwrap(),
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
    )
    .is_err());
    let mut duplicate = original;
    let target = duplicate["units"][2].clone();
    duplicate["units"].as_array_mut().unwrap().push(target);
    assert!(harpoon_core::load_scenario(
        serde_json::from_value(duplicate).unwrap(),
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
    )
    .is_err());

    let mut w = radar_world(19, "real_time", true);
    let mut invalid = w.rules.detection.as_ref().unwrap().config.emissions["TARGET"].clone();
    invalid.sensor = "absent-radar".into();
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .emissions
        .insert("ZZ-MISSING".into(), invalid);
    let before = serde_json::to_value(&w).unwrap();
    assert!(harpoon_core::rules::detection2026::initialize_emissions(&mut w).is_err());
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        before,
        "invalid later row must not partially replace valid native emitter metadata"
    );
}

#[test]
fn shared10_actual_radar_off_stops_native_es_and_on_restores_it_without_author_success_flag() {
    let mut w = radar_world(19, "real_time", true);
    let (r, t) = (id(&w, "RECEIVER"), id(&w, "TARGET"));
    let report = source_report(&w, r, "esm_std")
        .expect("actual loaded emitter must produce a native ES report");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Red,
            Command::SetRadar {
                unit: t,
                active: false,
            },
        )],
    );
    assert_eq!(
        source_report(&w, r, "esm_std").unwrap()["observed_at"],
        report["observed_at"]
    );
    assert!(
        w.events.iter().any(|e| match &e.kind {
            EventKind::RulesEvent {
                domain,
                rule,
                unit: Some(u),
                detail,
                ..
            } if e.tick == w.tick && domain == "detection2026" && rule == "5.scan" && *u == r => {
                let trace: Value = serde_json::from_str(detail).unwrap();
                trace["sensor"] == "esm_std"
                    && trace["result"]["reason"] == "not_emitting_or_RWR_search"
            }
            _ => false,
        }),
        "actual scan must acknowledge the physical transmitter stopped"
    );
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Red,
            Command::SetRadar {
                unit: t,
                active: true,
            },
        )],
    );
    assert_eq!(
        source_report(&w, r, "esm_std").unwrap()["observed_at"],
        w.tick
    );
}

#[test]
fn shared11_actual_source_radar_off_invalidates_aaw_cue_without_erasing_received_history() {
    let mut w = radar_world(19, "real_time", true);
    let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
    radar_on_and_warn(&mut w, "radar_air_long", true);
    assert_eq!(
        c::cue_evidence(&w, r, t, c::CuePurpose::AirDefenseReaction).modifier,
        2,
        "a real delivered original-radar warning must first be usable"
    );
    let received = c::own_view(&w, r).unwrap()["messages"].clone();
    assert!(!received.as_array().unwrap().is_empty());
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetRadar {
                unit: s,
                active: false,
            },
        )],
    );
    assert_eq!(
        c::cue_evidence(&w, r, t, c::CuePurpose::AirDefenseReaction).modifier,
        0,
        "a sonar-only active-bit fix must still enforce the actual radar emission switch"
    );
    assert_eq!(
        c::own_view(&w, r).unwrap()["messages"],
        received,
        "source contact loss stops a current cue without deleting the frozen historical receipt"
    );
    let switched_on = w.tick + 1;
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetRadar {
                unit: s,
                active: true,
            },
        )],
    );
    let mut fresh = false;
    for _ in 0..180 {
        if source_report(&w, s, "radar_air_long").is_some_and(|report| {
            report["observed_at"]
                .as_i64()
                .is_some_and(|at| at >= switched_on)
        }) {
            fresh = true;
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(
        fresh,
        "radar reactivation must pass its real reaction clock and make a fresh native scan"
    );
    assert_eq!(
        c::own_view(&w, r).unwrap()["messages"],
        received,
        "reactivating the source does not rewrite the original receipt"
    );
    let contact = cid(&w);
    send(&mut w, s, warning(contact, "radar_air_long", r));
    assert_eq!(
        c::cue_evidence(&w, r, t, c::CuePurpose::AirDefenseReaction).modifier,
        2,
        "a new source-qualified post-reactivation warning restores the cue"
    );
}

//! Resume candidates (implemented_unverified): real native sensor reports and actual source command/phase
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

#[test]
fn r01_automatic_warning_does_not_consume_later_native_fcs_in_same_epoch() {
    let mut config = fits("real_time", "near_real_time");
    config["units"]["SENDER"]["fits"]["lab"]["automatic_warning"] = json!(true);
    config["units"]["SENDER"]["fits"]["lab"]["automatic_fcs_weapon"] = json!("us_mk48");
    let mut w = comm_world_config(19, 1.25, config);
    let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
    // The real first native report has no 180-second own motion history.
    assert!(c::own_view(&w, r).unwrap()["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["sender"] == s.0 && m["contains_solution"] == false));
    let p = &w.rules.torpedoes.as_ref().unwrap().config.weapons["us_mk48"];
    let (generation, speed) = (p.generation, p.settings[0].speed_kts);
    assert!(f::own_sensor_solution(&w, s, t, A, generation, speed).is_err());
    // A real source off/on interval places eventual formation inside a
    // Tactical epoch, after that epoch has already sent its warning.
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSonar {
                unit: s,
                active: false,
            },
        )],
    );
    run(&mut w, 20);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSonar {
                unit: s,
                active: true,
            },
        )],
    );
    run(&mut w, 220);
    let own = f::own_sensor_solution(&w, s, t, A, generation, speed)
        .expect("actual selected sensor native motion/FCS checkpoint");
    let remote = c::received_solution(&w, r, t)
        .unwrap()
        .expect("automatic actual FCS delivery");
    assert_eq!(own.quality, f::Quality::Good);
    assert_eq!(remote.quality, f::Quality::Fair);
    assert_eq!(remote.source_observer, s);
    assert_eq!(remote.source_sensor, A);
    assert!(w.events.iter().any(
        |e| e.tick.div_euclid(180) == remote.formed_at.div_euclid(180)
            && matches!(&e.kind,EventKind::RulesEvent{rule,unit:Some(u),..}
            if rule=="6.3.9.auto_fcs_pending" && *u==s)
    ));
    let messages = c::own_view(&w, r).unwrap()["messages"]
        .as_array()
        .unwrap()
        .clone();
    let solutions: Vec<_> = messages
        .iter()
        .filter(|m| {
            m["sender"] == s.0
                && m["contains_solution"] == true
                && m["tactical_turn"] == remote.formed_at.div_euclid(180)
        })
        .collect();
    assert!(!solutions.is_empty());
    let before = solutions.len();
    run(&mut w, 230);
    let messages = c::own_view(&w, r).unwrap()["messages"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(
        messages
            .iter()
            .filter(|m| m["sender"] == s.0
                && m["contains_solution"] == true
                && m["tactical_turn"] == remote.formed_at.div_euclid(180))
            .count(),
        before
    );
}

#[test]
fn r02_selected_source_requires_its_own_unique_measured_native_fix() {
    let mut w = classified("real_time", "real_time");
    let (s, r, t) = (id(&w, "SENDER"), id(&w, "RECEIVER"), id(&w, "TARGET"));
    let p = &w.rules.torpedoes.as_ref().unwrap().config.weapons["us_mk48"];
    let q = f::own_sensor_solution(&w, s, t, A, p.generation, p.settings[0].speed_kts)
        .expect("actual active selected sensor own FCS");
    let action: c::Action =
        serde_json::from_value(json!({"type":"send_solution","contact":cid(&w),
        "sensor":A,"fit":"lab","receivers":[r],"weapon":"us_mk48","speed_index":0}))
        .unwrap();
    c::validate_action(&w, Side::Blue, s, &action).unwrap();
    c::apply(&mut w, Side::Blue, s, &action).unwrap();
    c::detection_open(&mut w);
    let remote = c::received_solution(&w, r, t).unwrap().unwrap();
    assert_eq!(remote.source_sensor, A);
    assert_eq!(remote.quality, q.quality);
    assert_eq!(
        remote.aim,
        source_report(&w, s, A).unwrap()["measured_position"]
            .clone()
            .as_object()
            .map(|p| Vec2 {
                x: p["x"].as_f64().unwrap(),
                y: p["y"].as_f64().unwrap()
            })
    );
    let before = serde_json::to_value(&w).unwrap();
    assert!(f::own_sensor_solution(&w, s, t, "not-fitted-source", 1, 40.).is_err());
    assert_eq!(before, serde_json::to_value(&w).unwrap());
}

#[test]
fn r03_lost_link_fallback_keeps_old_status_and_hardware_events_private() {
    let mut w = unmanned_world("surface", "hitl", 1, None).unwrap();
    let v = id(&w, "VEHICLE0");
    run(&mut w, 2);
    let old = u::own_view(&w, v).unwrap();
    let at = w.tick;
    u::set_interference(
        &mut w,
        v,
        true,
        false,
        None,
        "referee_contract: control channel lost",
    )
    .unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(
        serde_json::to_value(&w.rules.unmanned).unwrap()["vehicles"][v.0.to_string()]["mode"],
        "autonomous"
    );
    assert_eq!(u::own_view(&w, v).unwrap(), old);
    let fallback: Vec<_> = w
        .events
        .iter()
        .filter(|e| {
            e.tick > at
                && matches!(&e.kind,
        EventKind::RulesEvent{domain,rule,unit:Some(unit),..}
        if domain=="unmanned2026" && rule=="2.5.loss_of_command" && *unit==v)
        })
        .collect();
    assert_eq!(fallback.len(), 1);
    assert!(fallback
        .iter()
        .all(|e| e.vis == harpoon_core::Visibility::Referee));
    assert!(!u::loss_disclosure_allowed(&w, v));
}

#[test]
fn r04_known_controller_roster_does_not_disclose_hidden_assignment_release() {
    let mut w = unmanned_world("underwater", "autonomous", 3, None).unwrap();
    let (v, controller) = (id(&w, "VEHICLE0"), id(&w, "CONTROLLER"));
    let before = u::own_view(&w, controller).unwrap()["controlled"].clone();
    assert_eq!(before.as_array().unwrap().len(), 3);
    unmanned(
        &mut w,
        v,
        json!({"type":"take_control","controller":controller,"element":"console","link":"acoustic_modem"}),
    );
    run(&mut w, 181);
    assert!(u::can_control(&w, Side::Blue, v).is_ok());
    assert_eq!(u::own_view(&w, controller).unwrap()["controlled"], before);
    assert!(u::own_view(&w, v).is_none());
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    harpoon_core::sim::tick(&mut restored, &[]);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn r05_actual_unseen_casualty_preserves_telemetry_and_conceals_only_owner_loss_count() {
    let mut w = unmanned_world("surface", "hitl", 1, None).unwrap();
    let v = id(&w, "VEHICLE0");
    run(&mut w, 2);
    let before = harpoon_core::observe(&w, Side::Blue, 0, 4096);
    let old = u::telemetry(&w, v).unwrap();
    u::set_interference(
        &mut w,
        v,
        true,
        false,
        None,
        "referee_contract: control channel lost before damage",
    )
    .unwrap();
    let weapon = w.db.weapons["us_mk48"].clone();
    for _ in 0..20 {
        if !w.entity(v).is_some_and(|e| e.alive && e.hp > 0.) {
            break;
        }
        // Source adjudicated hit queue, not fabricated HP/casualty state.
        harpoon_core::damage::apply_hit_at(&mut w, v, &weapon, true, Some(90.));
        harpoon_core::sim::tick(&mut w, &[]);
    }
    // A destroyed surface hull follows the original14.3 D10-minute sinking
    // clock. HP<=0 is combat ineffective; it is not yet a physical loss.
    if let Some(sink_at) = w
        .entity(v)
        .and_then(|e| e.ext.get("damage.sink_at"))
        .copied()
    {
        assert!(sink_at >= w.tick as f64);
        run(&mut w, sink_at as i64);
    }
    assert!(
        !w.entity(v).is_some_and(|e| e.alive),
        "actual committed casualty checkpoint"
    );
    assert_eq!(w.losses[Side::Blue.index()], 1);
    assert_eq!(
        u::concealed_losses(&w, Side::Blue),
        (1, std::collections::BTreeMap::from([("ship".into(), 1)]))
    );
    let after = harpoon_core::observe(&w, Side::Blue, 0, 4096);
    assert_eq!(
        after.victory.as_ref().unwrap().own_losses,
        before.victory.as_ref().unwrap().own_losses
    );
    assert_eq!(
        after.victory.as_ref().unwrap().own_losses_by_kind,
        before.victory.as_ref().unwrap().own_losses_by_kind
    );
    assert_eq!(
        serde_json::to_value(u::telemetry(&w, v).unwrap()).unwrap(),
        serde_json::to_value(old).unwrap()
    );
    assert!(after.units.iter().any(|u| u.id == v));
    assert!(
        !after.events.iter().any(|e| matches!(&e.kind,
        EventKind::Impact{unit,..}|EventKind::CriticalHit{unit,..}|EventKind::Sunk{unit,..}
            |EventKind::AirShotDown{unit,..}|EventKind::Ditched{unit,..} if *unit==v)),
        "owner-visible damage/casualty event is also unreceived telemetry"
    );
    let restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    assert_eq!(
        u::concealed_losses(&w, Side::Blue),
        u::concealed_losses(&restored, Side::Blue)
    );
    // Explicit trusted later casualty receipt: unlike a hidden physical sink,
    // it removes the stale held unit and does not duplicate physical scoring.
    u::record_destroyed(&mut w, v, true);
    assert_eq!(u::concealed_losses(&w, Side::Blue).0, 0);
    assert!(u::telemetry(&w, v).is_none());
    assert!(!u::known_units(&w, Side::Blue).contains(&v));
    assert_eq!(w.losses[Side::Blue.index()], 1);
}

#[test]
fn r06_repairing_one_source_control_cause_keeps_another_failure_active() {
    let mut w = unmanned_world("surface", "hitl", 1, None).unwrap();
    let v = id(&w, "VEHICLE0");
    assert!(u::allow_side_projection(&w, v));
    u::set_operational(&mut w, v, false, "critical:radio-A").unwrap();
    u::set_operational(&mut w, v, false, "critical:radio-B").unwrap();
    u::set_operational(&mut w, v, true, "critical:radio-A").unwrap();
    assert!(!u::allow_side_projection(&w, v));
    let mut copy: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    u::set_operational(&mut w, v, true, "critical:radio-B").unwrap();
    u::set_operational(&mut copy, v, true, "critical:radio-B").unwrap();
    assert!(u::allow_side_projection(&w, v));
    assert_eq!(
        serde_json::to_value(w).unwrap(),
        serde_json::to_value(copy).unwrap()
    );
}

#[test]
fn r07_modem_basic_position_does_not_publish_hidden_mode_or_status() {
    let mut w = unmanned_world("underwater", "hitl", 1, Some(3.)).unwrap();
    let (v, controller) = (id(&w, "VEHICLE0"), id(&w, "CONTROLLER"));
    unmanned(&mut w, v, json!({"type":"exchange_basic_data"}));
    assert!(u::telemetry(&w, v).unwrap().unit.is_none());
    assert!(u::own_view(&w, v).is_none());
    assert_eq!(
        u::own_view(&w, controller).unwrap()["controlled"][0]["position_not_reported"],
        true
    );
    u::set_interference(
        &mut w,
        v,
        true,
        true,
        None,
        "referee_contract: both channels lost",
    )
    .unwrap();
    run(&mut w, 10);
    assert_eq!(w.entity(v).unwrap().ordered_speed_kts, 0.);
    assert!(u::own_view(&w, v).is_none());
    assert!(u::telemetry(&w, v).unwrap().unit.is_none());
}

#[test]
fn r08_invalid_initialize_context_is_atomic_before_source_fuel_assignment() {
    let mut w = unmanned_world("surface", "hitl", 1, None).unwrap();
    let before = serde_json::to_value(&w).unwrap();
    let mut config = w.context.unmanned_2026.clone().unwrap();
    config.vehicles.get_mut("VEHICLE0").unwrap().endurance_s = Some(7.);
    assert!(u::initialize(&mut w, config).is_err());
    assert_eq!(before, serde_json::to_value(w).unwrap());
}

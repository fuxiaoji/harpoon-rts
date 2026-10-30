//! New broad-batch candidates, not yet executed. Native radar -> typed RT
//! delivery -> real SAM/AAM stock and projectile. Only the communication
//! equipment is a declared laboratory fit; no Contact/FCS/clock/dice injection.
use harpoon_core::rules::{aaw2026, communications2026 as c, operations};
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn run(w: &mut World, to: i64) {
    while w.tick < to {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn world(kind: &str, receiver_class: &str) -> World {
    let fit = |peers: Vec<&str>| json!({"source":"scenario_assumption: explicit laboratory compatible targeting link; no real Annex fit claim","protocol":"wide-air-rt","send":true,"receive":true,"operational":true,"peers":peers,"warning_kind":kind,"automatic_warning":false,"targeting_system":true});
    let sc: Scenario = serde_json::from_value(json!({
        "id":"wide-native-air-rt","name":"Original platforms, sensors, HP and ammunition / explicit laboratory communication hardware",
        "seed":20261026,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "aviation_2026":{"source":"scenario_assumption: explicit laboratory aircraft load for source MR and native fuel accounting; installed weapon fits unchanged",
                "loads":{"us_f18e":{"payload_kg":3000,"tanks":[],"source":"scenario_assumption: declared 3000kg payload, below the actual source capacity"}},"parking":{},
                "initial_fuel_nm":if receiver_class=="us_f18e"{json!({"TARGET":500,"RECEIVER":500})}else{json!({"TARGET":500})}},
            "communications_2026":{"source":"scenario_assumption: explicit laboratory radio fit","units":{
                "SENDER":{"source":"scenario_assumption: laboratory link only","fits":{"lab":fit(vec!["RECEIVER"])}},
                "RECEIVER":{"source":"scenario_assumption: laboratory link only","fits":{"lab":fit(vec![])}}
            }},
            "detection_2026":{"environment":{"source":"scenario_assumption: clear laboratory environment","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
        "units":[
            {"side":"Blue","class":"us_ford","name":"SENDER","pos":[0,0],"course_deg":90,"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":receiver_class,"name":"RECEIVER","pos":[0,-1],"course_deg":0,"speed_kts":if receiver_class=="us_f18e"{300}else{0},"altitude_m":if receiver_class=="us_f18e"{3000}else{0},"fire_on":false},
            {"side":"Red","class":"us_f18e","name":"TARGET","pos":[0,20],"course_deg":180,"speed_kts":300,"altitude_m":3000,"fire_on":false}
        ]})).unwrap();
    let mut w = harpoon_core::load_scenario(
        sc,
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap(),
    )
    .unwrap();
    for aircraft in w
        .entities
        .values()
        .filter(|e| e.kind == harpoon_core::types::PlatformKind::Aircraft)
    {
        assert!(
            w.rules
                .aviation
                .as_ref()
                .unwrap()
                .flights
                .contains_key(&aircraft.id),
            "laboratory aircraft {} must have its actual explicit load and initial fuel",
            aircraft.name
        );
    }
    let receiver = id(&w, "RECEIVER");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetRadar {
                unit: receiver,
                active: false,
            },
        )],
    );
    run(&mut w, 91);
    let sender = id(&w, "SENDER");
    let target = id(&w, "TARGET");
    assert!(
        operations::native_air_target_ready(&w, sender, target, "radar_air_long"),
        "fixture must naturally earn a native targeting clock"
    );
    assert!(c::native_report_view(&w, sender, cid(&w), "radar_air_long").is_ok());
    assert!(c::received_air_solution(&w, receiver, target)
        .unwrap()
        .is_none());
    w
}
fn cid(w: &World) -> ContactId {
    let target = id(w, "TARGET");
    w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == target && !c.stale)
        .unwrap()
        .id
}
fn action(w: &World) -> c::Action {
    serde_json::from_value(json!({"type":"send_air_solution","contact":cid(w),"sensor":"radar_air_long","fit":"lab","receivers":[id(w,"RECEIVER")]})).unwrap()
}
fn send(w: &mut World) {
    let sender = id(w, "SENDER");
    let a = action(w);
    c::validate_action(w, Side::Blue, sender, &a).unwrap();
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::Communications {
                unit: sender,
                action: a,
                retain_manual: false,
            },
        )],
    );
    assert!(
        c::received_air_solution(w, id(w, "RECEIVER"), id(w, "TARGET"))
            .unwrap()
            .is_some()
    );
}
fn rt_clock(w: &World) -> operations::RadarTrack {
    w.rules.operations.as_ref().unwrap().rt_air_tracks
        [&format!("{}:{}", id(w, "RECEIVER").0, id(w, "TARGET").0)]
        .clock
        .clone()
}
fn launch_trace(w: &World, rule: &str) -> Value {
    let receiver = id(w, "RECEIVER");
    w.events
        .iter()
        .rev()
        .find_map(|e| match &e.kind {
            EventKind::RulesEvent {
                domain,
                rule: r,
                unit: Some(unit),
                detail,
                ..
            } if domain == "aaw2026" && r == rule && *unit == receiver => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .expect("actual source launch trace")
}

#[test]
fn wide_air01_native_rt_geometry_real_sam_and_once_receiver_clock_restore() {
    let mut w = world("real_time", "us_ford");
    send(&mut w);
    let (receiver, target) = (id(&w, "RECEIVER"), id(&w, "TARGET"));
    let first = rt_clock(&w);
    let evidence = c::received_air_solution(&w, receiver, target)
        .unwrap()
        .unwrap();
    assert_eq!(evidence.altitude_m, Some(3000.));
    assert!(serde_json::to_value(&evidence)
        .unwrap()
        .get("quality")
        .is_none());
    send(&mut w);
    let repeat = rt_clock(&w);
    assert_eq!(
        (repeat.roll, repeat.seen, repeat.engage),
        (first.roll, first.seen, first.engage)
    );
    let raw = serde_json::to_value(&w).unwrap();
    let restored: World = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(&restored).unwrap(), raw);
    assert_eq!(rt_clock(&restored).roll, first.roll);
    let due = first.engage.max(w.tick);
    run(&mut w, due);
    send(&mut w); // current Increment geometry, without restarting reaction.
    let mount = w
        .entity(receiver)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_essm_i")
        .unwrap();
    let stock = w.entity(receiver).unwrap().weapons[mount].ammo;
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetRadar {
                unit: receiver,
                active: true,
            },
        )],
    );
    assert!(operations::sam_reaction_ready(
        &w,
        receiver,
        target,
        &w.db.weapons["us_essm_i"]
    ));
    let released = aaw2026::fire_at(&mut w, receiver, target, mount, 1)
        .expect("real source SAM launch after own receiver reaction");
    assert_eq!(released, 1);
    assert_eq!(w.entity(receiver).unwrap().weapons[mount].ammo, stock - 1);
    assert!(w.entities.values().any(|e| e.alive
        && e.projectile
            .as_ref()
            .is_some_and(|p| p.launched_by == receiver && p.target == Some(target))));
    assert!(!launch_trace(&w, "8.1.1.source_launch")["rt_air_refs"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(rt_clock(&w).roll, first.roll);
}

#[test]
fn wide_air02_nrt_air_reject_is_readonly_and_native_source_off_invalidates_rt() {
    let w = world("near_real_time", "us_ford");
    let raw = serde_json::to_value(&w).unwrap();
    assert!(c::validate_action(&w, Side::Blue, id(&w, "SENDER"), &action(&w)).is_err());
    assert_eq!(serde_json::to_value(&w).unwrap(), raw);
    let mut rt = world("real_time", "us_ford");
    send(&mut rt);
    let (sender, receiver, target) = (id(&rt, "SENDER"), id(&rt, "RECEIVER"), id(&rt, "TARGET"));
    let history = c::own_view(&rt, receiver).unwrap()["messages"].clone();
    harpoon_core::sim::tick(
        &mut rt,
        &[(
            Side::Blue,
            Command::SetRadar {
                unit: sender,
                active: false,
            },
        )],
    );
    assert!(c::received_air_solution(&rt, receiver, target)
        .unwrap()
        .is_none());
    assert_eq!(c::own_view(&rt, receiver).unwrap()["messages"], history);
}

#[test]
fn wide_air03_native_rt_geometry_reaches_actual_aam_without_invented_quality() {
    let mut w = world("real_time", "us_f18e");
    send(&mut w);
    let (receiver, target) = (id(&w, "RECEIVER"), id(&w, "TARGET"));
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetRadar {
                unit: receiver,
                active: true,
            },
        )],
    );
    let mount = w
        .entity(receiver)
        .unwrap()
        .weapons
        .iter()
        .position(|m| {
            w.db.weapons[&m.weapon].kind == harpoon_core::db::WeaponKind::Aam && m.ammo > 0
        })
        .unwrap();
    let stock = w.entity(receiver).unwrap().weapons[mount].ammo;
    let released = aaw2026::fire_at(&mut w, receiver, target, mount, 1)
        .expect("actual original AAM weapon, guidance and envelope gates");
    assert_eq!(released, 1);
    assert_eq!(w.entity(receiver).unwrap().weapons[mount].ammo, stock - 1);
    assert!(!launch_trace(&w, "9.5.source_launch")["rt_air_refs"]
        .as_array()
        .unwrap()
        .is_empty());
}

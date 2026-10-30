//! Artifact-only12.2 source candidates. Normal scenario/Command/movement/native
//! radar and ES, with finite author nm stock. No private runtime/Contact writes.
use harpoon_core::db::SensorKind;
use harpoon_core::rules::{detection2026 as d, ew2026 as ew};
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

const CAPACITY: f64 = 0.05;
fn seed() -> u64 {
    (0..10_000)
        .find(|s| {
            [0, 180, 360, 540, 720].iter().all(|at| {
                d::trial(&mut d::State::default(), *s, *at, "5.3:2:esm_std:1", 180).0 <= 90
            })
        })
        .expect("bounded original ES trial fixture, never a replaced live die")
}
fn fixture(altitude_m: f64, wind_speed_kts: f64) -> World {
    let input = json!({"id":"source-airborne-chaff-ray","name":"explicit author nm capacity; source horizontal radar and reverse ES through an actual flight-laid corridor","seed":seed(),"duration_s":900,"weather":{"visibility_nm":40,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
      "aviation_2026":{"source":"scenario_assumption: source-verified MH60R flight card plus explicit experimental installed corridor kit/material load","loads":{"us_mh60r":{"source":"scenario_assumption: complete100kg experiment hardware/material mass within published1480kg capacity; not a real chaff system fit or inferred nm-to-mass conversion","payload_kg":100,"tanks":[],"external_weapon_rounds":{}}},"parking":{},"initial_fuel_nm":{"LAYER":100}},
      "ew_2026":{"fits":{},"loadouts":{},"environment":{"source":"scenario_assumption: ordinary constant wind, unambiguous open sea; no very-high-wind claim","sea_state":0,"wind_from_deg":180,"wind_speed_kts":wind_speed_kts},
      "airborne_chaff":{"source":"English2026 PDF119/12-1 section12.2; explicit experimental actual aircraft pod, no real MH60R Annex corridor-capacity claim","wind_update_policy":"source_600s_movement","units":{"LAYER":{"source":"scenario_assumption: installed experimental airborne corridor dispenser","fit":"lab_corridor","installed":true,"capacity_nm":CAPACITY,"capacity_source":"scenario_assumption: finite0.05 nautical miles of flight, not defensive chaff bursts","lifetime_s":720,"lifetime_source":"scenario_assumption: exact720s material life within printed up-to1800s maximum"}}}},
      "detection_2026":{"bindings":{"us_ford:esm_std":{"type":"es","source":"scenario_assumption: actual fitted ES receiver, explicit Gen6 non-ELS experiment","generation":6,"targeting_complex":false,"rwr":false,"bearing_error_deg":2,"els":false}},"targets":{},
      "emissions":{"RADAR":{"source":"scenario_assumption: actual installed LJQ364 source-bound search emitter, switch remains physical","sensor":"cn_ljq364","weapon_radar":false,"lpi":false,"radar_role":"LAS","max_range_nm":35,"antenna_height_m":25}},
      "environment":{"source":"scenario_assumption: controlled clear open-water actual radar/ES rays","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
      "units":[{"side":"Blue","class":"cn_001","name":"RADAR","pos":[0,0],"speed_kts":0,"fire_on":false},
      {"side":"Red","class":"us_ford","name":"CENTER","pos":[10,0],"speed_kts":0,"fire_on":false},
      {"side":"Red","class":"us_ford","name":"OFFRAY","pos":[20,-0.02],"speed_kts":0,"fire_on":false},
      {"side":"Red","class":"us_mh60r","name":"LAYER","pos":[4.975,0.124],"course_deg":90,"speed_kts":60,"altitude_m":altitude_m,"fire_on":false}]});
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    assert!(db.platforms["cn_001"]
        .sensors
        .iter()
        .any(|s| s == "cn_ljq364"));
    assert!(db.platforms["us_ford"]
        .sensors
        .iter()
        .any(|s| s == "esm_std"));
    assert_eq!(db.sensors["cn_ljq364"].kind, SensorKind::Radar);
    assert_eq!(db.sensors["esm_std"].kind, SensorKind::Esm);
    db.platforms
        .get_mut("cn_001")
        .unwrap()
        .sensors
        .retain(|s| s == "cn_ljq364");
    db.platforms
        .get_mut("us_ford")
        .unwrap()
        .sensors
        .retain(|s| s == "esm_std");
    db.platforms.get_mut("us_mh60r").unwrap().sensors.clear();
    for class in ["cn_001", "us_ford", "us_mh60r"] {
        let p = db.platforms.get_mut(class).unwrap();
        p.weapons.clear();
        p.deck = None;
    }
    let mut w = harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(), db)
        .unwrap();
    assert!(w.sides.iter().all(|s| s.contacts.is_empty()));
    assert_eq!(
        w.entity(EntityId(4)).unwrap().chaff,
        0,
        "author nm inventory must work independently of zero legacy defensive burst count"
    );
    assert!(w.rules.aviation.as_ref().unwrap().flights.contains_key(&EntityId(4)),
        "source flight card + explicit load and initial remaining range must create the real normal Flight");
    harpoon_core::rules::aviation2026::movement_limits(&w, EntityId(4)).expect(
        "actual installed MH60R source flight row/MR must support the declared level60kt movement",
    );
    let before_move = w.entity(EntityId(4)).unwrap().pos;
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetRadar {
                unit: EntityId(1),
                active: true,
            },
        )],
    );
    assert!((before_move.range_to(w.entity(EntityId(4)).unwrap().pos)-60.0/3600.0).abs()<1e-8,
        "pre-start physical movement must really happen; an unsupported inert aircraft cannot stand in for a flight-laid cloud");
    assert_eq!(
        native_at(&w, 1, "cn_ljq364", 2),
        Some(1),
        "real center ship radar prerequisite"
    );
    assert_eq!(
        native_at(&w, 1, "cn_ljq364", 3),
        Some(1),
        "real off-ray source radar prerequisite"
    );
    assert_eq!(
        native_at(&w, 2, "esm_std", 1),
        Some(1),
        "real installed ES + physical emitter prerequisite, no injected emission or Contact"
    );
    w
}
fn action(kind: &str) -> Command {
    let a = if kind == "start_chaff_barrier" {
        json!({"type":kind,"fit":"lab_corridor"})
    } else {
        json!({"type":kind})
    };
    serde_json::from_value(json!({"cmd":"electronic_warfare","unit":4,"action":a}))
        .expect("12.2 missing actual airborne corridor declaration after real radar/ES prerequisites; no future Rust enum compile failure")
}
fn native_at(w: &World, observer: u32, sensor: &str, target: u32) -> Option<i64> {
    w.rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .get(&format!("{observer}:{sensor}:{target}"))
        .and_then(|t| t.report.as_ref())
        .map(|r| r.at)
}
fn own(w: &World) -> Value {
    ew::own_view(w, Side::Red, EntityId(4)).unwrap()["airborne_chaff"].clone()
}
fn segments(w: &World) -> Vec<Value> {
    let runtime = serde_json::to_value(w.rules.ew.as_ref().unwrap()).unwrap();
    runtime["airborne_chaff"]["segments"].as_object()
        .expect("12.2 actual movement must persist firstclass finite corridor segments, not surface false-echo objects")
        .values().cloned().collect()
}
fn check_world(w: &World) {
    assert_eq!(w.entities.len(), 4, "no chaff combat platform created");
    assert!(
        w.rules.ew.as_ref().unwrap().objects.is_empty(),
        "12.2 is independent of12.3 long-range surface false echoes"
    );
    assert_eq!(w.entity(EntityId(4)).unwrap().chaff, 0);
}
fn same(w: &World, r: &World) {
    assert_eq!(serde_json::to_value(w).unwrap(),serde_json::to_value(r).unwrap(),
        "complete storedWorld preserves actual nm inventory, paid segments, radar/ES measurements and original lifecycle clocks");
}
fn pair_tick(w: &mut World, r: &mut World, commands: &[(Side, Command)]) {
    harpoon_core::sim::tick(w, commands);
    harpoon_core::sim::tick(r, commands);
    check_world(w);
    check_world(r);
    same(w, r);
}
fn pair_until(w: &mut World, r: &mut World, end: i64) {
    while w.tick < end {
        harpoon_core::sim::tick(w, &[]);
        harpoon_core::sim::tick(r, &[]);
    }
    check_world(w);
    check_world(r);
    same(w, r);
}
fn paid(w: &World) {
    let v = own(w);
    assert!(v["remaining_nm"].as_f64().unwrap().abs() < 1e-9);
    assert!((v["laid_nm"].as_f64().unwrap()-CAPACITY).abs()<1e-9,
        "actual flown nm is clipped exactly to finite author capacity; never convert burst count or mint a full last step");
    assert_eq!(
        v["laying"], false,
        "empty capacity stops material release, not the aircraft"
    );
}
fn current_controls(w: &World) {
    assert_eq!(native_at(w,1,"cn_ljq364",3),Some(w.tick),
        "off-ray target remains sensed: cloud width is.25nm full, not half-width and not a global jammer");
    assert_eq!(
        native_at(w, 2, "esm_std", 1),
        Some(w.tick),
        "12.2 explicitly leaves the real reverse ES path open through the same cloud"
    );
}
#[test]
fn air_chaff01_finite_actual_flight_nm_blocks_only_crossing_radar_ray_without_false_echo_or_es_block(
) {
    let mut w = fixture(200., 0.);
    let mut replay: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    pair_tick(
        &mut w,
        &mut replay,
        &[(Side::Red, action("start_chaff_barrier"))],
    );
    assert_eq!(native_at(&w,1,"cn_ljq364",2),Some(1),
        "Low whole-band corridor at y.124 with half-width.125 intersects real center radar LOS; no new radar measurement");
    current_controls(&w);
    assert!(
        w.entity(EntityId(4)).unwrap().pos.x > 4.991,
        "actual movement, not a caller-plotted instant barrier"
    );
    assert!(own(&w)["remaining_nm"].as_f64().unwrap() < CAPACITY);
    pair_until(&mut w, &mut replay, 4);
    paid(&w);
    let length = segments(&w).len();
    assert!(length > 0);
    pair_until(&mut w, &mut replay, 8);
    assert_eq!(
        segments(&w).len(),
        length,
        "further aircraft flight cannot add unpaid corridor"
    );
    paid(&w);
    current_controls(&w);
    pair_tick(
        &mut w,
        &mut replay,
        &[(Side::Red, action("stop_chaff_barrier"))],
    );
    pair_tick(
        &mut w,
        &mut replay,
        &[(Side::Red, action("start_chaff_barrier"))],
    );
    paid(&w);
    assert_eq!(
        segments(&w).len(),
        length,
        "stop/start is not a stock refill or old-flight replay"
    );
}
#[test]
fn air_chaff02_actual_band_descent_full_wind_drift_and_authored_lifetime_change_native_radar_los_with_restore(
) {
    let mut w = fixture(3000., 6.);
    harpoon_core::sim::tick(&mut w, &[(Side::Red, action("start_chaff_barrier"))]);
    assert_eq!(native_at(&w,1,"cn_ljq364",2),Some(2),
        "Medium corridor cannot block a physically low ship radar ray solely from a two-dimensional crossing");
    while w.tick < 539 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    paid(&w);
    check_world(&w);
    let mut replay: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    pair_until(&mut w, &mut replay, 541);
    assert_eq!(native_at(&w, 1, "cn_ljq364", 2), Some(541));
    assert!(segments(&w)
        .iter()
        .all(|s| s["initial_band"] == 3 && s["band"] == 3));
    pair_until(&mut w, &mut replay, 544);
    assert!(
        native_at(&w, 1, "cn_ljq364", 2).is_some_and(|at| at < 544),
        "by each segment's540s age Medium has fallen to Low and now blocks actual radar refresh"
    );
    assert!(segments(&w)
        .iter()
        .all(|s| s["band"] == 2 && s["expired"] == false));
    current_controls(&w);
    pair_until(&mut w, &mut replay, 604);
    assert!(segments(&w).iter().all(|s| s["expired"] == false));
    for s in segments(&w) {
        assert!((s["from"]["y"].as_f64().unwrap()-1.124).abs()<1e-6,
            "source600s Movement drift is6kt*600s=1nm north: full wind speed, not12.4 floating-decoy half speed");
    }
    assert_eq!(
        native_at(&w, 1, "cn_ljq364", 2),
        Some(604),
        "actual current cloud has drifted clear; radar must resume from the real normal ray"
    );
    current_controls(&w);
    paid(&w);
    pair_until(&mut w, &mut replay, 724);
    assert!(
        segments(&w).iter().all(|s| s["expired"] == true),
        "last paid laid_at4+author720s expires by724 without a lifetime reset"
    );
    assert_eq!(native_at(&w, 1, "cn_ljq364", 2), Some(724));
    current_controls(&w);
    paid(&w);
}

//! Source candidates: English2026 PDF129/14-3 automatic carrier deck critical, root visual review.
//! Normal scenario, lookout, release/stock/mass and common land-hit consumer.
use harpoon_core::rules::{aircombat2026 as ac, aviation2026 as av, detection2026 as d};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};

const CLASS: &str = "us_f18e_mk82";
const WEAPON: &str = "us_mk82";
const ALTITUDE: f64 = 6000.;

fn seed() -> u64 {
    (20261028..20261028 + 100_000).find(|seed| {
        let (lookout, _) = d::trial(&mut d::State::default(), *seed, 1, "5.8.visual:2:visual", 180);
        let mut r = Rng::new(*seed).derive(Rng::hash_tag(2, 1, "9.6.Mk82:us_mk82:0:0"));
        lookout <= 10 && 1 + (r.next_f64() * 100.) as u8 == 1
    }).expect("ordinary preconstruction Scenario seed for actual lookout and source original D100=1 hit; no live state/die edits")
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn fixture(target_class: &str) -> World {
    let speed = av::source_speed(CLASS, ALTITUDE, av::Throttle::Cruise).unwrap();
    let profile = |size| json!({"source":"scenario_assumption: controlled clear-range native visual profile, not a fabricated report","size":size,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    let mut input = json!({"id":"source-runway-six-mk82","name":"English2026 actual Medium bomb hit on source carrier","seed":seed(),"duration_s":1000,
        "weather":{"visibility_nm":40,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
        "aviation_2026":{"source":"US2025 B44/H1-3/H4-2 existing six-Mk82 source strike load; initial fuel explicitly authored",
            "loads":{(CLASS):{"payload_kg":6395,"tanks":[{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"}],"external_weapon_rounds":{(WEAPON):6},"source":"US2025 B44/H1-3: actual six-Mk82 three-DT source load"}},"parking":{},"initial_fuel_nm":{"STRIKE":1000},
            "no_aa_surface_attack_source":"scenario_assumption: source weapons-hold range; real shared defensive scan still governs release",
            "surface_aimpoints":{"range":{"source":"scenario_assumption: fixed Blue prebriefed control point","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
        "detection_2026":{"bindings":{"us_ford:visual":{"type":"visual","source":"scenario_assumption: fitted actual friendly range lookout"}},"targets":{"us_ford":profile(0),(target_class):profile(0)},
            "environment":{"source":"scenario_assumption: clear daylight range","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
        "units":[{"side":"Blue","class":CLASS,"name":"STRIKE","pos":[0,-(3.+speed/3600.)],"course_deg":0,"speed_kts":speed,"altitude_m":ALTITUDE,"fire_on":false},
            {"side":"Blue","class":"us_ford","name":"LOOKOUT","pos":[0,-1],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":target_class,"name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}]});
    let mut db = Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../database/consolidated.json"))).unwrap();
    ac::configure_database(&mut db);
    db.platforms.get_mut("us_f18e").unwrap().sensors.clear();
    for class in ["us_ford", target_class] { db.platforms.get_mut(class).unwrap().weapons.clear(); }
    db.platforms.get_mut("us_ford").unwrap().sensors = vec!["visual".into()];
    if target_class != "us_ford" { db.platforms.get_mut(target_class).unwrap().sensors.clear(); }
    let mut w = harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(), db).unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    let target = id(&w, "TARGET");
    assert!(w.find_contact(Side::Blue, target).is_some(), "genuine native friendly lookout must first discover the land target");
    assert_eq!(d::known_category(&w, Side::Blue, target), Some(harpoon_core::types::PlatformKind::Ship));
    assert!((w.entity(id(&w, "STRIKE")).unwrap().pos.range_to(harpoon_core::Vec2::zero()) - 3.).abs() < 1e-9);
    w
}
fn fire(w: &mut World) {
    fire_delivery(w, "Level");
}
fn fire_delivery(w: &mut World, delivery: &str) {
    let unit = id(w, "STRIKE");
    let contact = w.find_contact(Side::Blue, id(w, "TARGET")).unwrap();
    let cmd: Command = serde_json::from_value(json!({"cmd":"air_combat","unit":unit,
        "action":{"type":"surface_attack","contact":contact,"weapon":WEAPON,"rounds":6,"delivery":delivery,"aimpoint":"range"}})).unwrap();
    harpoon_core::sim::tick(w, &[(Side::Blue, cmd)]);
}
fn receipt(w: &World) -> Value {
    w.events.iter().find_map(|e| match &e.kind {
        EventKind::RulesEvent {rule,detail,..} if rule=="9.6.source_bomb_attack" => Some(serde_json::from_str(detail).unwrap()),
        _=>None,
    }).expect("normal queued release/defense/budget must produce actual source adjudication")
}
#[test]
fn deck01_normal_241kg_bomb_hit_on_unarmored_carrier_adds_actual_once_deck_critical() {
    let mut w=fixture("cn_001");let target=id(&w,"TARGET");
    let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    fire(&mut w);fire(&mut restored);
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&restored).unwrap());
    let d=receipt(&w);assert_eq!(d["total_hits"],1,"original normal source D100=1 must actually hit before testing automatic casualties");
    let auto:Vec<_>=w.events.iter().filter_map(|e|match &e.kind {
        EventKind::RulesEvent{rule,unit,detail,..} if rule=="14.1.2.automatic_flight_deck"&&*unit==Some(target)=>Some(serde_json::from_str::<Value>(detail).unwrap()),_=>None,
    }).collect();
    assert_eq!(auto.len(),1,"actual >=50kg bomb hit must create one source automatic carrier flight-deck critical in addition to ratio criticals");
    assert_eq!(auto[0]["weapon"],WEAPON);
    assert_eq!(auto[0]["hang_weight_kg"],241.);
    assert_eq!(auto[0]["applied"],true);
    assert_eq!(w.entity(target).unwrap().ext.get("flight_deck_hit"),Some(&1.));
    let count=w.events.iter().filter(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="14.1.2.automatic_flight_deck")).count();
    harpoon_core::sim::tick(&mut w,&[]);
    assert_eq!(w.events.iter().filter(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="14.1.2.automatic_flight_deck")).count(),count,"no recurring automatic damage without another actual hit");
}
#[test]
fn deck02_nonpenetrating_normal_bomb_hit_does_not_bypass_source_carrier_deck_armor() {
    let mut w=fixture("us_ford");let target=id(&w,"TARGET");fire(&mut w);
    assert_eq!(receipt(&w)["total_hits"],1);
    assert!(!w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,unit,detail,..}if rule=="14.1.2.automatic_flight_deck"&&*unit==Some(target)&&serde_json::from_str::<Value>(detail).unwrap()["applied"]==true)),"the automatic clause explicitly retains penetration qualification");
    assert_ne!(w.entity(target).unwrap().ext.get("flight_deck_hit"),Some(&1.));
}

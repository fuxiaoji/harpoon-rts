//! Source candidate only: English2026 PDF105-107 /9.6.4-9.6.5.
//! Real High source flight, explicit prebriefed range and a genuine friendly
//! lookout report; no High-altitude Visual capability is invented.
use harpoon_core::rules::{aircombat2026 as ac, aviation2026 as av, detection2026 as d};
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, Vec2, World};
use serde_json::{json, Value};

const CLASS: &str = "us_f18e_mk82";
const WEAPON: &str = "us_mk82";
const ALTITUDE: f64 = 8000.0;

fn seed() -> u64 {
    (20261028..20261028 + 10_000)
        .find(|seed| {
            let (draw, _) = d::trial(
                &mut d::State::default(),
                *seed,
                1,
                "5.8.visual:2:visual",
                180,
            );
            draw <= 10
        })
        .expect("bounded ordinary Scenario seed for the actual friendly lookout; no stored die replacement")
}

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

fn fixture() -> World {
    let speed = av::source_speed(CLASS, ALTITUDE, av::Throttle::Cruise)
        .expect("the exact existing F18E source fit must support genuine High Cruise");
    let target = |size| json!({"source":"scenario_assumption: explicit controlled visual target/observer sizes","size":size,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    let input = json!({"id":"high-level-conventional-source","name":"prebriefed fixed range, exact High6nm release, real finite Mk82 and ordinary source zero hit","seed":seed(),"duration_s":1000,
        "weather":{"visibility_nm":40.0,"jam":[0.0,0.0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "aviation_2026":{"source":"Existing verified US2025 B44/H1-3 six-Mk82 source strike fit; controlled initial fuel",
                "no_aa_surface_attack_source":"scenario_assumption: explicitly prebriefed unopposed fixed range; normal actual defensive scan still governs release",
                "surface_aimpoints":{"range":{"source":"scenario_assumption: this Blue pilot was briefed before takeoff on the stationary controlled range point","point":{"x":0.0,"y":0.0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}},
                "loads":{(CLASS):{"payload_kg":6395.0,"tanks":[{"range_add_nm":290.0,"fuel_weight_kg":1485.0,"source":"America Aircraft2025 B44"},{"range_add_nm":290.0,"fuel_weight_kg":1485.0,"source":"America Aircraft2025 B44"},{"range_add_nm":290.0,"fuel_weight_kg":1485.0,"source":"America Aircraft2025 B44"}],"external_weapon_rounds":{(WEAPON):6},"source":"America Aircraft2025 B44/H4-2/H1-3: existing source six-Mk82 three-DT strike load"}},
                "parking":{},"initial_fuel_nm":{"STRIKE":1000.0}},
            "detection_2026":{"bindings":{"us_ford:visual":{"type":"visual","source":"scenario_assumption: actual fitted friendly range lookout"}},"targets":{"us_ford":target(0),"cn_001":target(0)},
                "environment":{"source":"scenario_assumption: clear daylight stationary range with a genuine Low surface lookout","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000.0,"layer":null,"ice":0,"cloud":null}}},
        "units":[{"side":"Blue","class":CLASS,"name":"STRIKE","pos":[0.0,-(6.0+speed/3600.0)],"course_deg":0.0,"speed_kts":speed,"altitude_m":ALTITUDE,"fire_on":false},
            {"side":"Blue","class":"us_ford","name":"RANGE_LOOKOUT","pos":[0.0,-4.0],"speed_kts":0.0,"fire_on":false},
            {"side":"Red","class":"cn_001","name":"TARGET","pos":[0.0,0.0],"speed_kts":0.0,"fire_on":false}]});
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    ac::configure_database(&mut db);
    db.platforms.get_mut(CLASS).unwrap().sensors.clear();
    db.platforms.get_mut("us_ford").unwrap().sensors = vec!["visual".into()];
    db.platforms.get_mut("us_ford").unwrap().weapons.clear();
    db.platforms.get_mut("cn_001").unwrap().sensors.clear();
    db.platforms.get_mut("cn_001").unwrap().weapons.clear();
    let mut w = harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(), db)
        .unwrap();
    let actor = id(&w, "STRIKE");
    let start = w.entity(actor).unwrap().pos;
    assert!(
        w.rules
            .aviation
            .as_ref()
            .unwrap()
            .flights
            .contains_key(&actor),
        "normal explicit load/fuel must initialize an actual Flight, not a source helper only"
    );
    av::movement_limits(&w, actor).expect("normal physical High movement prerequisites");
    harpoon_core::sim::tick(&mut w, &[]);
    let a = w.entity(actor).unwrap();
    assert!(
        (start.range_to(a.pos) - speed / 3600.0).abs() < 1e-9,
        "the real aircraft actually flies the source one-second distance before the declaration"
    );
    assert!((a.pos.range_to(Vec2::zero()) - 6.0).abs() < 1e-9);
    assert_eq!(a.altitude_m, ALTITUDE);
    let lookout = id(&w, "RANGE_LOOKOUT");
    let target = id(&w, "TARGET");
    let report=w.rules.aaw.as_ref().unwrap().firecontrol.tracks
        .get(&format!("{}:visual:{}",lookout.0,target.0)).and_then(|t|t.report.as_ref())
        .expect("actual friendly lookout native Visual report; no Contact injection and no High Visual search exception");
    assert_eq!(report.observer, lookout);
    assert_eq!(report.at, 1);
    assert_eq!(report.measured_position, Some(Vec2::zero()));
    assert_eq!(
        d::known_category(&w, Side::Blue, target),
        Some(harpoon_core::types::PlatformKind::Ship)
    );
    assert!(w.find_contact(Side::Blue, target).is_some());
    w
}

fn same(w: &World, r: &World) {
    assert_eq!(serde_json::to_value(w).unwrap(),serde_json::to_value(r).unwrap(),"full World restore retains real source flight, queued defense/budget, stock, mass, zero-hit dice receipt and maneuver state");
}

#[test]
fn high01_real_six_nm_level_release_spends_one_241kg_mk82_has_zero_hits_and_frees_maneuver_after_restore(
) {
    let mut w = fixture();
    let actor = id(&w, "STRIKE");
    let target = id(&w, "TARGET");
    let contact = w.find_contact(Side::Blue, target).unwrap();
    let before_hp = w.entity(target).unwrap().hp;
    let before_stock = w
        .entity(actor)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == WEAPON)
        .unwrap()
        .ammo;
    let before_flight = &w.rules.aviation.as_ref().unwrap().flights[&actor];
    let before_mass = before_flight.payload_fraction * before_flight.spec.max_payload_kg;
    assert_eq!(before_stock, 6);
    assert_eq!(before_flight.external_weapon_rounds[WEAPON], 6);
    assert!(ac::target_maneuver(&w, actor).unwrap() > 0.0);
    let command:Command=serde_json::from_value(json!({"cmd":"air_combat","unit":actor,"action":{"type":"surface_attack","contact":contact,"weapon":WEAPON,"rounds":1,"delivery":"Level","aimpoint":"range"}})).unwrap();
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for state in [&mut w, &mut restored] {
        harpoon_core::sim::tick(state, &[(Side::Blue, command.clone())]);
    }
    same(&w, &restored);
    let receipts: Vec<_> = w
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RulesEvent {
                domain,
                rule,
                unit: Some(unit),
                detail,
                ..
            } if domain == "command"
                && *unit == actor
                && event.tick == w.tick
                && detail.starts_with("air_combat:") =>
            {
                Some((rule.clone(), detail.clone()))
            }
            _ => None,
        })
        .collect();
    assert!(receipts.iter().any(|(rule,_)|rule=="executed"),"9.6.4 explicitly permits High Level bombing at exact6nm after normal source aircraft/load/range/contact/flight prerequisites; ordinary command receipts: {receipts:?}");
    let attacks: Vec<Value> = w
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RulesEvent {
                domain,
                rule,
                unit: Some(unit),
                detail,
                ..
            } if domain == "aircombat2026"
                && rule == "9.6.source_bomb_attack"
                && *unit == actor =>
            {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        attacks.len(),
        1,
        "one real normal defensive scan and budget must lead to one physical release"
    );
    let attack = &attacks[0];
    assert_eq!(attack["delivery"], "Level");
    assert_eq!(attack["rounds"], 1);
    assert_eq!(attack["resolution"], "source_fixed_range");
    assert_eq!(attack["probability"].as_f64(),Some(0.0),"above7500m conventional Level Ph is zero before favorable sight/stationary/NoAA modifiers; nuclear bombing is a separate domain");
    assert_eq!(attack["total_hits"], 0);
    assert_eq!(attack["groups"].as_array().unwrap().len(), 1);
    assert_eq!(
        attack["groups"][0]["salvo"], 1,
        "Level table salvo is one attacking aircraft, not its six-bomb fitted inventory"
    );
    assert!((1..=100).contains(&attack["groups"][0]["d100"].as_u64().unwrap()));
    assert_eq!(attack["groups"][0]["hits"], 0);
    let anchor: Vec2 =
        serde_json::from_value(attack["logical_release_anchor"]["position"].clone()).unwrap();
    assert!((anchor.range_to(Vec2::zero()) - 6.0).abs() < 1e-9);
    assert_eq!(
        attack["logical_release_anchor"]["altitude_m"].as_f64(),
        Some(ALTITUDE)
    );
    assert!(
        w.entity(actor).unwrap().pos.range_to(anchor) > 0.0,
        "normal movement continues; the logical source release anchor never teleports the plane"
    );
    assert_eq!(
        w.entity(actor)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == WEAPON)
            .unwrap()
            .ammo,
        before_stock - 1
    );
    let flight = &w.rules.aviation.as_ref().unwrap().flights[&actor];
    assert_eq!(flight.external_weapon_rounds[WEAPON], 5);
    assert!(
        (before_mass - flight.payload_fraction * flight.spec.max_payload_kg - 241.0).abs() < 1e-6,
        "actual source241kg store removed from the real shared Flight mass"
    );
    assert_eq!(w.entity(target).unwrap().hp, before_hp);
    assert_eq!(w.losses, [0, 0]);
    assert!(w.rules.aircombat.as_ref().unwrap().pending_surface_attacks.is_empty(),"known conventional zero-hit outcome must resolve, not borrow unknown nuclear/penetration/dispersion effects");
    assert!(ac::target_maneuver(&w,actor).unwrap()>0.0,"9.6.4 High release at6nm explicitly lets the aircraft maneuver freely after the drop; no residual generic30s Level MR0 lock");
    let before_course = w.entity(actor).unwrap().course_deg;
    let turn = Command::SetCourse {
        unit: actor,
        course_deg: 30.0,
    };
    for state in [&mut w, &mut restored] {
        harpoon_core::sim::tick(state, &[(Side::Blue, turn.clone())]);
    }
    same(&w, &restored);
    assert!(
        w.entity(actor).unwrap().course_deg > before_course,
        "actual normal post-release turn, no forced course mutation"
    );
    assert!(ac::target_maneuver(&w, actor).unwrap() > 0.0);
    assert_eq!(w.entity(target).unwrap().hp, before_hp);
    assert_eq!(w.events.iter().filter(|event|matches!(&event.kind,EventKind::WeaponsRelease{shooter,weapon:_,count:1,..}if *shooter==actor)).count(),1);
}

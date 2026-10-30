//! Source candidates: English2026 PDF117/11-2 and118/11-3, root visual review.
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
        lookout <= 10 && 1 + (r.next_f64() * 100.) as u8 <= 22
    }).expect("ordinary preconstruction Scenario seed for actual lookout and source22% hit; no live state/die edits")
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn fixture(template: &str) -> World {
    fixture_at(template, ALTITUDE, 3.)
}
fn fixture_at(template: &str, altitude: f64, release_nm: f64) -> World {
    let speed = av::source_speed(CLASS, altitude, av::Throttle::Cruise).unwrap();
    let profile = |size| json!({"source":"scenario_assumption: controlled clear-range native visual profile, not a fabricated report","size":size,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    let mut input = json!({"id":"source-runway-six-mk82","name":"English2026 actual Medium level attack on source concrete runway","seed":seed(),"duration_s":1000,
        "weather":{"visibility_nm":40,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
        "aviation_2026":{"source":"US2025 B44/H1-3/H4-2 existing six-Mk82 source strike load; initial fuel explicitly authored",
            "loads":{(CLASS):{"payload_kg":6395,"tanks":[{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"}],"external_weapon_rounds":{(WEAPON):6},"source":"US2025 B44/H1-3: actual six-Mk82 three-DT source load"}},"parking":{},"initial_fuel_nm":{"STRIKE":1000},
            "no_aa_surface_attack_source":"scenario_assumption: source weapons-hold range; real shared defensive scan still governs release",
            "surface_aimpoints":{"range":{"source":"scenario_assumption: fixed Blue prebriefed control point","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
        "ground_2026":{"source":"English2026 PDF117/118 source target, author layout","facilities":{"TARGET":{"template":template,"custom":null,"source":"English2026 PDF118 Generic Land Targets; exact existing source template"}}},
        // Visual land structures use the printed C-H recognition table. The
        // authored C visual footprint is independent of11.5.1's A attack area.
        "detection_2026":{"bindings":{"us_ford:visual":{"type":"visual","source":"scenario_assumption: fitted actual friendly range lookout"}},"targets":{"us_ford":profile(0),"airfield":profile(2)},"camouflage":{"airfield":0},
            "environment":{"source":"scenario_assumption: clear daylight range","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
        "units":[{"side":"Blue","class":CLASS,"name":"STRIKE","pos":[0,-(release_nm+speed/3600.)],"course_deg":0,"speed_kts":speed,"altitude_m":altitude,"fire_on":false},
            {"side":"Blue","class":"us_ford","name":"LOOKOUT","pos":[0,-1],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"airfield","name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}]});
    if template == "runway" {
        input["context"]["ground_2026"]["facilities"]["TARGET"]["runway"] = json!({"length_m":500,"baseline":"Large"});
    }
    let mut db = Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../database/consolidated.json"))).unwrap();
    ac::configure_database(&mut db);
    db.platforms.get_mut("us_f18e").unwrap().sensors.clear();
    for class in ["us_ford", "airfield"] { db.platforms.get_mut(class).unwrap().weapons.clear(); }
    db.platforms.get_mut("us_ford").unwrap().sensors = vec!["visual".into()];
    db.platforms.get_mut("airfield").unwrap().sensors.clear();
    let mut w = harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(), db).unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    let target = id(&w, "TARGET");
    assert!(w.find_contact(Side::Blue, target).is_some(), "genuine native friendly lookout must first discover the land target");
    assert_eq!(d::known_category(&w, Side::Blue, target), Some(harpoon_core::types::PlatformKind::Base));
    assert!((w.entity(id(&w, "STRIKE")).unwrap().pos.range_to(harpoon_core::Vec2::zero()) - release_nm).abs() < 1e-9);
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
fn runway01_actual_source_concrete_runway_takes_one_cut_from_six_bombs_and_retains_original_die() {
    let mut w = fixture("runway");
    let actor=id(&w,"STRIKE"); let target=id(&w,"TARGET");
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&target].target.horizontal_armor,2.);
    let before = &w.rules.aviation.as_ref().unwrap().flights[&actor];
    let mass = before.payload_fraction*before.spec.max_payload_kg;
    let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    fire(&mut w);fire(&mut restored);
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&restored).unwrap());
    let d=receipt(&w);
    assert_eq!(d["probability"],0.22,"A Level0.08*0.25; source stationary+2/Advanced+5/land+2/noAA+1 =0.22; protected runway is source qualified");
    assert_eq!(d["groups"].as_array().unwrap().len(),1);
    assert_eq!(d["groups"][0]["salvo"],1,"floor6/4=1 above5000m");
    assert_eq!(d["total_hits"],1,"original preconstructed source die hits, at most one bomb hits a runway salvo");
    let f=&w.rules.ground.as_ref().unwrap().facilities[&target];
    assert_eq!(f.cuts,1,"actual normal LandHit must invoke the runway-cut consumer");
    assert_eq!(f.damage,35.);
    assert_eq!(w.entity(target).unwrap().hp,65.);
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::Impact{unit,damage,..} if *unit==target&&*damage==35.)));
    let after=&w.rules.aviation.as_ref().unwrap().flights[&actor];
    assert_eq!(after.external_weapon_rounds[WEAPON],0);
    assert!((mass-after.payload_fraction*after.spec.max_payload_kg-6.*241.).abs()<1e-8);
    assert_eq!(w.entity(actor).unwrap().weapons.iter().find(|m|m.weapon==WEAPON).unwrap().ammo,0);
}
#[test]
fn runway02_unrelated_armored_structure_keeps_its_existing_specific_pending_guard_after_physical_release() {
    let mut w=fixture("has_nato"); let target=id(&w,"TARGET");
    fire(&mut w);let d=receipt(&w);
    assert_eq!(d["resolution"],"pending_source_target_properties");
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&target].damage,0.);
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&target].cuts,0);
}
#[test]
fn runway03_glide_salvo_caps_real_multiple_bomb_hits_to_one_cut_per_salvo() {
    let mut w=fixture_at("runway",1000.,0.);let target=id(&w,"TARGET");
    fire_delivery(&mut w,"Glide");let d=receipt(&w);
    let groups=d["groups"].as_array().expect("actual qualified runway attack groups");
    assert_eq!(groups.len(),2);
    assert_eq!(groups[0]["salvo"],4);
    assert_eq!(groups[1]["salvo"],2);
    let raw=ac::air_attack_hits(d["probability"].as_f64().unwrap(),4,groups[0]["d100"].as_u64().unwrap() as u8).unwrap();
    assert!(raw>1,"original normal D100 must otherwise produce multiple bomb hits; this exercises the runway cap");
    assert_eq!(groups[0]["hits"],1);
    assert!(groups.iter().all(|g|g["hits"].as_u64().unwrap()<=1));
    let hits=d["total_hits"].as_u64().unwrap();
    assert!((1..=2).contains(&hits));
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&target].cuts,u32::try_from(hits).unwrap());
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&target].damage,35.*hits as f64);
}

//! ZCODE-FACILITY-NEXT-02 behavior examples for the owned 11.4 ngfs module
//! (v2 ABI): one attack per ship-gun per Tac with order-consistent
//! conflicts, shells declared and debited exactly once before any die,
//! unknown necessary inputs pending instead of implicit zeros, and qualified
//! hits handed to the common aaw.surface.land_hits path (single 11.1.2
//! ratio window; no per-path dice and no criticals lost to a leading miss).
//!
//! Public scenario JSON + public commands + the module's own public ABI.
//! The spotting task uses a real native lookout (seed 26 convention; seed is
//! the only recalibration knob). No hidden state is injected: ammunition and
//! damage flow through the normal ledgers. Not compiled/run/accepted.

use harpoon_core::{Command, Database, Scenario, Side, World};
use harpoon_core::rules::ground2026 as ground;
use harpoon_core::rules::detection2026 as detection;
use harpoon_core::rules::ground2026::ngfs;

fn gun_bands() -> serde_json::Value {
    serde_json::json!({
        "name":"PRC PJ-45A 130mm/70","shell":"HE",
        "source":"China's Navy Annex C C-1, visually verified 2026-10-28",
        "bands":[
            {"max_range_kyd":8.5,"belt_pen_cm":2,"damage":25},
            {"max_range_kyd":19.1,"belt_pen_cm":1,"damage":21},
            {"max_range_kyd":25.4,"belt_pen_cm":1,"deck_pen_cm":1,"damage":19},
            {"max_range_kyd":31.8,"belt_pen_cm":1,"deck_pen_cm":1,"damage":17}],
        "aa_max_range_kyd":16.4,"aa_rating":0.85})
}

fn configuration(under_fire: Option<bool>) -> serde_json::Value {
    let mut service = serde_json::json!({
        "source":"scenario_assumption: explicit ngfs lab service, not a real fitting",
        "ship":"SHIP","gun":"cn_pj45a","gunnery_standard":"gs5","barrels":2,
        "track_ammo":true,"ammo_per_attack":2,
        "target":"TARGET","sea_state":2});
    if let Some(under) = under_fire {
        service["ship_under_fire"] = serde_json::json!(under);
    }
    serde_json::json!({
     "id":"facility-ngfs-behavior-lab","name":"scenario_assumption: 11.4 ngfs behavior lab",
     "seed":(0..100000).find(|seed| detection::trial(&mut detection::State::default(),*seed,1,"5.8.visual:1:visual",180).0 <= 5).unwrap(),"duration_s":20000,
     "weather":{"visibility_nm":10,"jam":[0,0]},
     "context":{"rules_profile":"hv2026-rts-v4",
      "detection_2026":{"bindings":{"cn_052d:visual":{"type":"visual","source":"scenario_assumption: native experimental lookout"}},"targets":{
        "cn_052d":{"source":"scenario_assumption: observer profile","size":1,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2},
        "airfield":{"source":"scenario_assumption: E visual footprint independent of G target DP profile","size":4,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}},
        "environment":{"source":"scenario_assumption: clear daylight lab","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
      "aviation_2026":{"source":"scenario_assumption: lab visibility declaration","loads":{},"parking":{},"visibility_percent":100.0},
      "ground_2026":{"source":"scenario_assumption: explicit lab target",
       "facilities":{"TARGET":{"source":"scenario_assumption: explicit experimental zero-armor point target",
         "custom":{"size":"G","damage_points":40,"vertical_armor":0,"horizontal_armor":0,
                   "effect":"destroyed","source":"scenario_assumption: explicit characteristics"}}},
       "ngfs":{"source":"scenario_assumption: explicit ngfs lab",
        "guns":{"cn_pj45a":gun_bands()},
        "services":{"witness":service}}}},
     "units":[
      {"side":"Blue","class":"cn_052d","name":"SHIP","pos":[0,0],"speed_kts":0,"fire_on":false},
      {"side":"Red","class":"airfield","name":"TARGET","pos":[2,0],"fire_on":false}]})
}

fn load(under_fire: Option<bool>) -> World {
    let scenario: Scenario = serde_json::from_value(configuration(under_fire)).unwrap();
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    db.platforms.get_mut("cn_052d").unwrap().sensors=vec!["visual".into()];
    db.platforms.get_mut("airfield").unwrap().sensors.clear();
    harpoon_core::load_scenario(scenario, db)
        .expect("explicit ngfs lab configuration must load without ignored fields")
}

fn advance(w: &mut World, ticks: i64) {
    for _ in 0..ticks {
        harpoon_core::sim::tick(w, &[]);
    }
}

/// Real native lookout earns the visual contact, then the public Spot command
/// creates the 11.4 spotting task the executor consumes.
fn declare_spotting(w: &mut World) {
    advance(w, 60);
    let target = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    let contact = w.sides[Side::Blue.index()]
        .contacts
        .values()
        .find(|c| c.about == target && !c.stale && c.sensors.iter().any(|s| s == "visual"))
        .expect("seed 26 must give the lookout a fresh visual contact; recalibrate seed only")
        .id;
    let ship = w.entities.values().find(|e| e.name == "SHIP").unwrap().id;
    let cmd = Command::Ground {
        unit: ship,
        action: ground::Action::Spot {
            contact,
            method: ground::Spotting::Ship,
        },
    };
    harpoon_core::sim::tick(w, &[(Side::Blue, cmd)]);
    advance(w,361); // actual third spotting turn; first two penalties are unprinted
}

fn ship_id(w: &World) -> harpoon_core::EntityId {
    w.entities.values().find(|e| e.name == "SHIP").unwrap().id
}
fn target_id(w: &World) -> harpoon_core::EntityId {
    w.entities.values().find(|e| e.name == "TARGET").unwrap().id
}

#[test]
fn b1_one_attack_per_tac_and_order_consistent_conflicts() {
    let mut w = load(Some(false));
    declare_spotting(&mut w);
    let ship = ship_id(&w);
    let target = target_id(&w);
    ngfs::validate_from_fire(&w, ship, target, "cn_pj45a").expect("first shot passes the read-only precheck");
    ngfs::prepare_from_fire(&mut w, ship, target, "cn_pj45a").expect("first shot queues");
    // Duplicate declarations coalesce before the shared commit window.
    ngfs::prepare_from_fire(&mut w,ship,target,"cn_pj45a").unwrap();
    assert_eq!(w.rules.ground.as_ref().unwrap().ngfs.as_ref().unwrap().jobs.len(),1);
    // The queued job keeps its own identity; a full World roundtrip preserves it.
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    advance(&mut w, 5);
    advance(&mut restored, 5);
    assert_eq!(serde_json::to_vec(&w).unwrap(), serde_json::to_vec(&restored).unwrap());
    assert!(ngfs::validate_from_fire(&w,ship,target,"cn_pj45a").unwrap_err().contains("Tactical Turn"));
}

#[test]
fn b2_shells_are_declared_and_debited_once_before_any_die() {
    let mut w = load(Some(false));
    declare_spotting(&mut w);
    let ship = ship_id(&w);
    let target = target_id(&w);
    let mount = |w: &World| {
        w.entity(ship)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "cn_pj45a")
            .unwrap()
            .ammo
    };
    let before = mount(&w);
    ngfs::prepare_from_fire(&mut w, ship, target, "cn_pj45a").expect("declaration consumes the slot");
    assert_eq!(before,mount(&w),"intent collection never spends before shared arbitration");
    ngfs::resolve_referee(&mut w);
    let after=mount(&w);
    assert_eq!(before-after,2,"declared shells debited once before the referee die");
    ngfs::late_batch(&mut w);
    assert_eq!(mount(&w), after, "resolution never debits again, hit or miss");
}

#[test]
fn b3_unknown_necessary_inputs_pend_without_implicit_zero() {
    // ship_under_fire is required: without it the die is retained unrolled,
    // the common path receives nothing, and the shells stay spent.
    let mut w = load(None);
    declare_spotting(&mut w);
    let ship = ship_id(&w);
    let target = target_id(&w);
    ngfs::prepare_from_fire(&mut w, ship, target, "cn_pj45a").expect("declaration itself is legal");
    ngfs::resolve_referee(&mut w);
    let state = w.rules.ground.as_ref().unwrap().ngfs.as_ref().unwrap();
    assert_eq!(state.resolutions.len(), 1, "the attack is resolved exactly once");
    let r = &state.resolutions[0];
    assert!(r.pending.as_deref().unwrap_or_default().contains("under-fire"), "the missing under-fire state is the named gap");
    assert!(!r.hit, "a pended die never invents a hit");
    ngfs::late_batch(&mut w);
    assert!(
        w.rules.aaw.as_ref().unwrap().surface.land_hits.is_empty(),
        "pended resolutions never reach the common damage path"
    );
}

#[test]
fn b4_qualified_hit_reaches_the_single_common_land_window() {
    let mut w = load(Some(false));
    declare_spotting(&mut w);
    let ship = ship_id(&w);
    let target = target_id(&w);
    ngfs::prepare_from_fire(&mut w, ship, target, "cn_pj45a").expect("declaration queues the attack");
    ngfs::resolve_referee(&mut w);
    ngfs::late_batch(&mut w);
    let hits = &w.rules.aaw.as_ref().unwrap().surface.land_hits;
    // Short band (2 nm = 4 kyd), zero-armor point target: the qualified hit,
    // if the original die granted one, is exactly one common LandHit entry.
    let damage: f64 = hits.values().filter(|h| h.target == target).map(|h| h.damage).sum();
    assert_eq!(hits.len(),1,"decisive source fixture must reach the real common land ledger");
    assert_eq!(damage, 50.0, "Short-band 25 DP, one Size E+ hit, printed point-blank doubles damage");
    // The common flush stays the only 11.1.2 ratio window.
    harpoon_core::rules::surface2026::flush_land_hits(&mut w);
    assert!(w.rules.aaw.as_ref().unwrap().surface.land_hits.is_empty(), "the common flush consumed the hit");
    let hp = w.entity(target).unwrap().hp;
    if damage > 0.0 {
        assert!(hp < 40.0, "source damage reached the target through the common path");
    }
}

#[test]
fn b5_public_fire_reaches_actual_source_resolution_without_legacy_fallback() {
    let mut w=load(Some(false));
    declare_spotting(&mut w);
    let ship=ship_id(&w);let target=target_id(&w);
    let contact=w.sides[0].contacts.values().find(|c|c.about==target && !c.stale).unwrap().id;
    let fire=Command::Fire{unit:ship,contact,weapon:"cn_pj45a".into(),rounds:1};
    let before=serde_json::to_vec(&w).unwrap();
    fire.validate_source(&w,Side::Blue).unwrap();
    assert_eq!(before,serde_json::to_vec(&w).unwrap(),"preflight is pure");
    let ammo=w.entity(ship).unwrap().weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().ammo;
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,fire)]);
    assert_eq!(ammo-w.entity(ship).unwrap().weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().ammo,2);
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..} if rule=="11.4.ngfs.resolution")),"native Fire must settle its original source die");
}

// Source 11.4/PDF117: (N-1)*-2 applies to EACH firing ship, once.
#[test]
fn b6_two_native_ships_get_the_same_once_only_concentration_penalty() {
    let mut input=configuration(Some(false));
    input["seed"]=serde_json::json!((0..1000000).find(|seed|[1u32,2].into_iter().all(|actor| {
        detection::trial(&mut detection::State::default(),*seed,1,&format!("5.8.visual:{actor}:visual"),180).0<=5
    })).unwrap());
    input["units"][1]["pos"]=serde_json::json!([3,0]);
    let second=serde_json::json!({"side":"Blue","class":"cn_052d","name":"SHIP2","pos":[0,0],"speed_kts":0,"fire_on":false});
    input["units"].as_array_mut().unwrap().insert(1,second);
    let mut service=input["context"]["ground_2026"]["ngfs"]["services"]["witness"].clone();
    service["ship"]=serde_json::json!("SHIP2");
    input["context"]["ground_2026"]["ngfs"]["services"]["second"]=service;
    let mut db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    db.platforms.get_mut("cn_052d").unwrap().sensors=vec!["visual".into()];
    db.platforms.get_mut("airfield").unwrap().sensors.clear();
    let mut w=harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(),db).unwrap();
    advance(&mut w,60);let target=target_id(&w);
    let mut orders=Vec::new();
    for name in ["SHIP","SHIP2"] {
        let ship=w.entities.values().find(|e|e.name==name).unwrap().id;
        let contact=w.sides[0].contacts.values().find(|c|c.about==target && !c.stale).unwrap().id;
        orders.push((Side::Blue,Command::Ground{unit:ship,action:ground::Action::Spot{contact,method:ground::Spotting::Ship}}));
    }
    harpoon_core::sim::tick(&mut w,&orders);advance(&mut w,361);
    let mut fire=Vec::new();
    for name in ["SHIP","SHIP2"] {
        let ship=w.entities.values().find(|e|e.name==name).unwrap().id;
        let contact=w.sides[0].contacts.values().find(|c|c.about==target && !c.stale).unwrap().id;
        fire.push((Side::Blue,Command::Fire{unit:ship,contact,weapon:"cn_pj45a".into(),rounds:1}));
    }
    let mut reversed:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let since=w.events.len();harpoon_core::sim::tick(&mut w,&fire);
    fire.reverse();harpoon_core::sim::tick(&mut reversed,&fire);
    let results=|w:&World| {
        let mut rows=std::collections::BTreeMap::new();
        for event in w.events.iter().skip(since) {
            if let harpoon_core::EventKind::RulesEvent{rule,unit:Some(ship),detail,..}=&event.kind {
                if rule=="11.4.ngfs.resolution" {let value:serde_json::Value=serde_json::from_str(detail).unwrap();rows.insert(*ship,(value["chance"].as_f64().unwrap(),value["d100"].clone()));}
            }
        }rows
    };
    let actual=results(&w);assert_eq!(actual.len(),2,"both native ship-gun attacks must resolve");
    assert_eq!(actual,results(&reversed),"client arrival order cannot change the original die or modifier");
    for (chance,_) in actual.values() {assert!((*chance-0.8).abs()<1e-9,"GS5 Short .7 +(.1 ship spotting +1 safe +2 stationary -2 other ship)*.05 = .8; no point blank and no double concentration: {chance}");}
}

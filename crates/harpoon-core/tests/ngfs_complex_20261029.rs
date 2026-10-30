//! ZCODE-FACILITY-20261029: 11.6/11.7 whole-airfield naval bombardment.
//!
//! Source (visually verified 2026-10-29, English 2026-01-01):
//! - PDF117/11-2 §11.6: "first roll D100 to see what part of the base the
//!   shells come closest to... If the die roll result is 'empty space,' then
//!   the attack has automatically failed to hit anything. If ... an air base
//!   component is rolled, then make the naval gunfire attack normally."
//!   §11.7 resolves ports "using the same method".
//! - PDF116/11-1 §11.4: spotting tasks and the -2x(N-1) destruction-fire
//!   concentration penalty belong to "the same target" — the declared
//!   airfield — never to the reverse-selected component; Map/Blind are
//!   Neutralization Fire whose degradation is unprinted (F14) and must not
//!   borrow destruction damage.
//!
//! The examples use the public scenario JSON, public commands and the
//! module's public ABI. The native lookout earns every contact; seeds are
//! only recalibrated so the original dice fall in the boundary window the
//! case names. No component result is ever injected into own state.

use harpoon_core::{Command, Database, Scenario, Side, World};
use harpoon_core::rules::detection2026 as detection;
use harpoon_core::rules::ground2026::{self as ground, ngfs};

/// Cross-domain stock conservation: NGFS uses the same physical weapon store
/// as other native expenditure paths. The allocation is an explicit laboratory
/// assumption, not an Annex magazine-capacity claim. Written before the fix.
#[test]
fn n8_native_fire_keeps_physical_and_aggregate_ammunition_in_sync() {
    let mut input=complex_input(serde_json::json!({"runways":["RUNWAY1"],"aircraft_storage":["STORE1"]}),false);
    input["context"]["expanded_damage_2026"]=serde_json::json!({
        "source":"scenario_assumption: one explicitly allocated physical test stock",
        "units":{"SHIP":{"source":"scenario_assumption: laboratory gun magazine, not a real Type052D capacity",
            "components":{"test_stock":{"source":"scenario_assumption: explicit ten-round physical allocation",
                "kind":"weapon_mount","binding":"cn_pj45a","physical":0,"ammunition":10}}}}});
    let seed=visual_seeds(1)[0];
    let mut w=load_with_seed(input,seed);
    declare_spotting(&mut w,&["SHIP"]);
    let ship=id_of(&w,"SHIP"); let complex=id_of(&w,"AIRBASE");
    let contact=w.sides[0].contacts.values().find(|c|!c.stale && c.about==complex).unwrap().id;
    let initial=w.entity(ship).unwrap().weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().ammo;
    let fire=Command::Fire{unit:ship,contact,weapon:"cn_pj45a".into(),rounds:1};
    fire.validate_source(&w,Side::Blue).expect("actual public whole-complex Fire admission");
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,fire)]);
    assert_eq!(w.entity(ship).unwrap().weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().ammo,initial-2);
    let physical=w.rules.expanded_damage.as_ref().unwrap().units.get(&ship)
        .and_then(|u|u.ammunition.get("test_stock")).copied().unwrap_or(10);
    assert_eq!(physical,8,"the committed real NGFS attack must debit the declared physical stock exactly once");
    let restored:World=serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(w).unwrap(),serde_json::to_value(restored).unwrap());
}

/// The existing RTS atomic ship-gun contract applies across the §8.3/§11.4
/// consumers. Its one-Tac cadence/conflict policy is an engine conversion,
/// not a new printed rate of fire. Changing target domain cannot buy a slot.
fn dual_domain_world() -> World {dual_domain_starshell_world(None)}
fn dual_domain_starshell_world(ngfs_physical:Option<usize>) -> World {
    let mut input=complex_input(serde_json::json!({"runways":["RUNWAY1"],"aircraft_storage":["STORE1"]}),false);
    input["seed"]=serde_json::json!(visual_seeds(1)[0]);
    input["context"]["surface_gunnery_2026"]=serde_json::json!({"source":"scenario_assumption: dual-domain test service",
        "units":{"SHIP":{"source":"scenario_assumption: explicit test director, not real Type052D certification",
            "weapon":"cn_pj45a","standard":"gs5","barrels":2,"arcs":[[0,360]],"under_fire":true,"local_control":false,
            "radar_gfc":false,"reaction_only":false,"terrain_clear":true,"other_conditions_clear":true,"ammunition_per_attack":2}}});
    if let Some(index)=ngfs_physical {
        input["context"]["ground_2026"]["ngfs"]["services"]["witness"]["physical_index"]=serde_json::json!(index);
        let fit=&mut input["context"]["surface_gunnery_2026"]["units"]["SHIP"];
        fit["starshell_stock"]=serde_json::json!(4); fit["starshell_per_turn"]=serde_json::json!(2);
        fit["starshell_mount_role"]=serde_json::json!("primary"); fit["starshell_physical_index"]=serde_json::json!(1);
    }
    input["context"]["detection_2026"]["targets"]["cn_001"]=serde_json::json!({"source":"scenario_assumption: A visual footprint only",
        "size":0,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    input["units"].as_array_mut().unwrap().push(serde_json::json!({"side":"Red","class":"cn_001","name":"NAVAL","pos":[0,2.5],"course_deg":90,"speed_kts":0,"fire_on":false}));
    let mut db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    db.platforms.get_mut("cn_052d").unwrap().rule_data=Some(harpoon_core::rules::damage_runtime::PlatformDamage{
        source:"scenario_assumption: unarmored C laboratory hull for dual-domain consumer test; no Type052D Annex claim".into(),
        size:"C".into(),role:"surface".into(),belt_cm:0.,deck_cm:0.,..Default::default()});
    db.platforms.get_mut("cn_052d").unwrap().sensors=vec!["visual".into()];
    if ngfs_physical.is_some() {db.platforms.get_mut("cn_052d").unwrap().weapons.iter_mut().find(|m|m.weapon=="cn_pj45a").unwrap().mounts=2;}
    db.platforms.get_mut("airfield").unwrap().sensors.clear();
    db.platforms.get_mut("cn_001").unwrap().sensors.clear();
    let mut w=harpoon_core::load_scenario(serde_json::from_value(input).unwrap(),db).unwrap();
    declare_spotting(&mut w,&["SHIP"]); w
}
fn dual_fire(w:&World,name:&str)->Command {
    let target=id_of(w,name);
    let contact=w.sides[0].contacts.values().find(|c|!c.stale && c.about==target && c.sensors.iter().any(|s|s=="visual"))
        .expect("native visual contact for each declared domain").id;
    Command::Fire{unit:id_of(w,"SHIP"),contact,weapon:"cn_pj45a".into(),rounds:1}
}
#[test]
fn n9_committed_land_or_surface_attack_consumes_the_same_physical_turn_slot() {
    for (first,second) in [("AIRBASE","NAVAL"),("NAVAL","AIRBASE")] {
        let mut w=dual_domain_world();
        let fire=dual_fire(&w,first);fire.validate_source(&w,Side::Blue).unwrap();
        harpoon_core::sim::tick(&mut w,&[(Side::Blue,fire)]);
        let second=dual_fire(&w,second);let before=serde_json::to_value(&w).unwrap();
        assert!(second.validate_source(&w,Side::Blue).is_err(),"one gun must not buy a second attack by changing target domain");
        assert_eq!(before,serde_json::to_value(&w).unwrap());
    }
}
#[test]
fn n10_simultaneous_conflicting_target_domains_spend_no_slot_or_ammunition_in_either_order() {
    for reverse in [false,true] {
        let mut w=dual_domain_world();let ship=id_of(&w,"SHIP");
        let stock=w.entity(ship).unwrap().weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().ammo;
        let mut commands=vec![(Side::Blue,dual_fire(&w,"AIRBASE")),(Side::Blue,dual_fire(&w,"NAVAL"))];
        if reverse {commands.reverse();}
        for (side,c) in &commands {c.validate_source(&w,*side).unwrap();}
        harpoon_core::sim::tick(&mut w,&commands);
        assert_eq!(w.entity(ship).unwrap().weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().ammo,stock,
            "conflicting simultaneous declarations must be arbitrated before either domain commits");
        assert!(!w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..} if rule=="8.3.source_resolution")));
        assert!(dual_fire(&w,"NAVAL").validate_source(&w,Side::Blue).is_ok(),"a rejected conflicting window spends no slot");
    }
}

/// §11.7 uses the port reverse table, not the airfield table. The generic
/// Base body and component placement are explicitly authored; maintenance
/// DP/armor come from the already verified Generic Land Targets template.
fn port_ready(seed:u64)->World {
    let mut input=complex_input(serde_json::json!({"aircraft_storage":["STORE1"]}),false);
    input["units"][1]["name"]=serde_json::json!("PORT");
    let mut complex=input["context"]["ground_2026"]["complexes"].as_object_mut().unwrap().remove("AIRBASE").unwrap();
    complex["kind"]=serde_json::json!("Port");
    complex["source"]=serde_json::json!("English2026 PDF117/11-2 §11.7 port reverse table; explicit authored Port placement");
    complex["components"]=serde_json::json!({"workshops_stores":["STORE1"]});
    input["context"]["ground_2026"]["complexes"]["PORT"]=complex;
    input["context"]["ground_2026"]["ngfs"]["services"]["witness"]["complex"]=serde_json::json!("PORT");
    input["context"]["ground_2026"]["facilities"]["STORE1"]["template"]=serde_json::json!("maintenance_small");
    let mut w=load_with_seed(input,seed);
    advance(&mut w,60);
    let port=id_of(&w,"PORT");let ship=id_of(&w,"SHIP");
    let contact=w.sides[0].contacts.values().find(|c|!c.stale && c.about==port && c.sensors.iter().any(|s|s=="visual")).unwrap().id;
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::Ground{unit:ship,action:ground::Action::Spot{contact,method:ground::Spotting::Ship}})]);
    advance(&mut w,361);w
}
fn port_fire(w:&World)->Command {
    let target=id_of(w,"PORT");let contact=w.sides[0].contacts.values().find(|c|!c.stale && c.about==target).unwrap().id;
    Command::Fire{unit:id_of(w,"SHIP"),contact,weapon:"cn_pj45a".into(),rounds:1}
}
fn port_seed(hit:bool)->u64 {
    visual_seeds(1).into_iter().find(|seed|{
        let mut reverse=harpoon_core::Rng::new(*seed).derive(harpoon_core::Rng::hash_tag(423,2,"11.reverse:ngfs:witness"));
        let die=1+(reverse.next_f64()*100.)as u8;
        if !hit {return die>90}
        let mut shot=harpoon_core::Rng::new(*seed).derive(harpoon_core::Rng::hash_tag(423,1,"11.4.ngfs:witness:4"));
        (31..=50).contains(&die) && 1+(shot.next_f64()*100.)as u8<=60
    }).expect("ordinary original dice seed for an independently verified port table band")
}
#[test]
fn n11_actual_port_spot_fire_reverse_workshop_and_common_damage_survive_restore() {
    let mut w=port_ready(port_seed(true));let ship=id_of(&w,"SHIP");let port=id_of(&w,"PORT");let store=id_of(&w,"STORE1");
    assert_eq!(w.rules.ground.as_ref().unwrap().complexes[&port].kind,ground::ComplexKind::Port);
    let ammo=w.entity(ship).unwrap().weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().ammo;
    let fire=port_fire(&w);fire.validate_source(&w,Side::Blue).unwrap();
    let mut restored:World=serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,fire.clone())]);
    harpoon_core::sim::tick(&mut restored,&[(Side::Blue,fire)]);
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(restored).unwrap());
    let (die,category)=reverse_event(&w);assert!((31..=50).contains(&die));assert_eq!(category.as_deref(),Some("workshops_stores"));
    let resolution=w.events.iter().find_map(|e|match &e.kind {
        harpoon_core::EventKind::RulesEvent{rule,detail,..} if rule=="11.4.ngfs.resolution"=>serde_json::from_str::<serde_json::Value>(detail).ok(),_=>None}).unwrap();
    assert_eq!(resolution["land"],serde_json::json!(store));assert_eq!(resolution["hit"],true);assert_eq!(resolution["damage"],25.);
    assert!(w.entity(store).unwrap().hp<=w.entity(store).unwrap().hp_max-25.);
    assert_eq!(w.entity(port).unwrap().hp,w.entity(port).unwrap().hp_max,"declared complex has no invented aggregate DP hit");
    assert_eq!(w.entity(ship).unwrap().weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().ammo,ammo-2);
    assert!(w.rules.aaw.as_ref().unwrap().surface.land_hits.is_empty(),"actual simulation consumed its common land window");
    assert_eq!(w.rules.ground.as_ref().unwrap().spotting[&ship].target,port);
}
#[test]
fn n12_actual_port_empty_space_consumes_attack_but_never_rerolls_or_damages_a_component() {
    let mut w=port_ready(port_seed(false));let ship=id_of(&w,"SHIP");
    let fire=port_fire(&w);fire.validate_source(&w,Side::Blue).unwrap();
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,fire)]);
    let (die,category)=reverse_event(&w);assert!(die>90);assert!(category.is_none());
    let retained=&w.rules.ground.as_ref().unwrap().ngfs.as_ref().unwrap().retained;
    assert_eq!(retained.len(),1);assert!(!retained[0].hit);assert_eq!(retained[0].damage,0.);assert!(retained[0].pending.is_none());
    assert_eq!(retained[0].reverse.as_ref().unwrap().component,"empty");
    for name in ["PORT","STORE1","RUNWAY1"] {let e=w.entity(id_of(&w,name)).unwrap();assert_eq!(e.hp,e.hp_max);}
    let before=serde_json::to_value(&w).unwrap();assert!(port_fire(&w).validate_source(&w,Side::Blue).is_err());assert_eq!(before,serde_json::to_value(&w).unwrap());
    let mut restored:World=serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    harpoon_core::sim::tick(&mut w,&[]);harpoon_core::sim::tick(&mut restored,&[]);
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(restored).unwrap());
    assert_eq!(w.events.iter().filter(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..} if rule=="11.reverse_hit")).count(),1);
    assert!(w.rules.aaw.as_ref().unwrap().surface.land_hits.is_empty());
    assert!(w.rules.ground.as_ref().unwrap().spotting.contains_key(&ship));
}

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

/// One airfield complex with individually configured facility targets
/// (11.6 requires reverse-hit components to be real land targets).
/// `components` selects which categories the scenario declares; a reverse
/// roll on an undeclared category must stay pending adjudication.
fn complex_input(components: serde_json::Value, second_ship: bool) -> serde_json::Value {
    let mut units = serde_json::json!([
      {"side":"Blue","class":"cn_052d","name":"SHIP","pos":[0,0],"speed_kts":0,"fire_on":false},
      {"side":"Red","class":"airfield","name":"AIRBASE","pos":[3,0],"fire_on":false},
      {"side":"Red","class":"airfield","name":"RUNWAY1","pos":[3.1,0],"fire_on":false},
      {"side":"Red","class":"airfield","name":"STORE1","pos":[3.2,0],"fire_on":false}]);
    let has_buildings=components["buildings"].is_array();
    if has_buildings {
        units.as_array_mut().unwrap().push(
            serde_json::json!({"side":"Red","class":"airfield","name":"BLDG1","pos":[3.3,0],"fire_on":false}));
    }
    if second_ship {
        units.as_array_mut().unwrap().insert(1,
            serde_json::json!({"side":"Blue","class":"cn_052d","name":"SHIP2","pos":[0,0.5],"speed_kts":0,"fire_on":false}));
    }
    let mut services = serde_json::json!({
        "witness":{"source":"scenario_assumption: explicit ngfs lab service, not a real fitting",
            "ship":"SHIP","gun":"cn_pj45a","gunnery_standard":"gs5","barrels":2,
            "track_ammo":true,"ammo_per_attack":2,
            "complex":"AIRBASE","sea_state":2,"ship_under_fire":false}});
    if second_ship {
        services["second"] = serde_json::json!({
            "source":"scenario_assumption: explicit ngfs lab service, not a real fitting",
            "ship":"SHIP2","gun":"cn_pj45a","gunnery_standard":"gs5","barrels":2,
            "track_ammo":true,"ammo_per_attack":2,
            "complex":"AIRBASE","sea_state":2,"ship_under_fire":false});
    }
    let mut input=serde_json::json!({
     "id":"facility-ngfs-complex-lab","name":"scenario_assumption: 11.6/11.7 complex ngfs lab",
     "seed":0,"duration_s":20000,
     "weather":{"visibility_nm":10,"jam":[0,0]},
     "context":{"rules_profile":"hv2026-rts-v4",
      "detection_2026":{"bindings":{"cn_052d:visual":{"type":"visual","source":"scenario_assumption: native experimental lookout"}},"targets":{
        "cn_052d":{"source":"scenario_assumption: observer profile","size":1,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2},
        "airfield":{"source":"scenario_assumption: E visual footprint independent of component DP profiles","size":4,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}},
        "environment":{"source":"scenario_assumption: clear daylight lab","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
      "aviation_2026":{"source":"scenario_assumption: lab visibility declaration","loads":{},"parking":{},"visibility_percent":100.0},
      "ground_2026":{"source":"scenario_assumption: explicit lab complex and components",
       "facilities":{
         "RUNWAY1":{"source":"English2026 PDF118/11-3 Generic Land Targets, visually verified","template":"runway"},
         "STORE1":{"source":"English2026 PDF118/11-3 Generic Land Targets, visually verified","template":"hangar_large"},
         "BLDG1":{"source":"English2026 PDF118/11-3 Generic Land Targets, visually verified","template":"maintenance_small"}},
       "complexes":{"AIRBASE":{"kind":"Airbase",
         "source":"English2026 PDF117/11-2 11.6 Air Base Component table; component layout is a scenario assumption",
         "components":components}},
       "ngfs":{"source":"scenario_assumption: explicit ngfs lab",
        "guns":{"cn_pj45a":gun_bands()},
        "services":services}}},
     "units":units});
    if !has_buildings {input["context"]["ground_2026"]["facilities"].as_object_mut().unwrap().remove("BLDG1");}
    input
}

fn load_with_seed(mut input: serde_json::Value, seed: u64) -> World {
    input["seed"] = serde_json::json!(seed);
    let scenario: Scenario = serde_json::from_value(input).unwrap();
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    db.platforms.get_mut("cn_052d").unwrap().sensors = vec!["visual".into()];
    db.platforms.get_mut("airfield").unwrap().sensors.clear();
    harpoon_core::load_scenario(scenario, db).expect("explicit complex lab must load")
}

fn id_of(w: &World, name: &str) -> harpoon_core::EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

fn advance(w: &mut World, ticks: i64) {
    for _ in 0..ticks {
        harpoon_core::sim::tick(w, &[]);
    }
}

/// Seeds whose native lookouts hold fresh visual contacts for every actor,
/// the same calibration convention as facility_ngfs_behavior_20261028.
fn visual_seeds(actors: u32) -> Vec<u64> {
    (0..100000)
        .filter(|seed| {
            (1..=actors).all(|actor| {
                detection::trial(
                    &mut detection::State::default(),
                    *seed,
                    1,
                    &format!("5.8.visual:{actor}:visual"),
                    180,
                )
                .0
                    <= 5
            })
        })
        .collect()
}

/// Real native lookouts earn the contacts, the public Spot command declares
/// the 11.4 spotting task on the AIRFIELD entity, and 361 s put both ships
/// into their third continuous spotting turn.
fn declare_spotting(w: &mut World, ships: &[&str]) {
    advance(w, 60);
    let airbase = id_of(w, "AIRBASE");
    let mut orders = Vec::new();
    for name in ships {
        let ship = id_of(w, name);
        let contact = w.sides[Side::Blue.index()]
            .contacts
            .values()
            .find(|c| c.about == airbase && !c.stale && c.sensors.iter().any(|s| s == "visual"))
            .expect("seed calibration must give every lookout a fresh airfield visual contact")
            .id;
        orders.push((
            Side::Blue,
            Command::Ground {
                unit: ship,
                action: ground::Action::Spot {
                    contact,
                    method: ground::Spotting::Ship,
                },
            },
        ));
    }
    harpoon_core::sim::tick(w, &orders);
    advance(w, 361);
}

/// Public declarations plus the referee window; the state then holds the
/// resolutions of this Tac before late_batch runs.
fn fire_and_resolve(input: &serde_json::Value, seed: u64, ships: &[&str]) -> World {
    let mut w = load_with_seed(input.clone(), seed);
    declare_spotting(&mut w, ships);
    let airbase = id_of(&w, "AIRBASE");
    for name in ships {
        let ship = id_of(&w, name);
        ngfs::prepare_from_fire(&mut w, ship, airbase, "cn_pj45a")
            .expect("a declared whole-airfield attack must queue");
    }
    ngfs::resolve_referee(&mut w);
    w
}

/// The single 11.reverse_hit event of this attack: (D100, category or None).
fn reverse_event(w: &World) -> (u8, Option<String>) {
    w.events
        .iter()
        .find_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent { rule, detail, .. } if rule == "11.reverse_hit" => {
                let v: serde_json::Value = serde_json::from_str(detail).ok()?;
                Some((
                    v["d100"].as_u64()? as u8,
                    v["component"].as_str().map(|s| s.to_string()),
                ))
            }
            _ => None,
        })
        .expect("a complex bombardment must roll its reverse hit exactly once")
}

fn resolutions(w: &World) -> &Vec<ngfs::Resolution> {
    &w.rules
        .ground
        .as_ref()
        .unwrap()
        .ngfs
        .as_ref()
        .unwrap()
        .resolutions
}

/// §11.6: a rolled component is attacked "normally" through the same NGFS
/// resolution and the single common LandHit window; the component result
/// never redefines the own spotting task (which stays on the airfield).
#[test]
fn n1_reverse_component_reaches_ngfs_and_the_common_land_window() {
    let input = complex_input(
        serde_json::json!({
            "runways":["RUNWAY1"],
            "aircraft_storage":["STORE1"],
            "buildings":["BLDG1"]}),
        false,
    );
    let mut chosen = None;
    for seed in visual_seeds(1) {
        let w = fire_and_resolve(&input, seed, &["SHIP"]);
        let (d100, component) = reverse_event(&w);
        if !matches!(component.as_deref(), Some("aircraft_storage") | Some("buildings")) {
            continue; // zero-armor component window for a clean penetration
        }
        let state = resolutions(&w);
        if state.len() == 1 && state[0].hit && state[0].pending.is_none() {
            chosen = Some((w, seed, d100, component.unwrap()));
            break;
        }
    }
    let (mut w, seed, reverse_d100, category) =
        chosen.expect("a natural-dice seed with a zero-armor component hit must exist in range");
    let ship = id_of(&w, "SHIP");
    let airbase = id_of(&w, "AIRBASE");
    let state = resolutions(&w);
    assert_eq!(state.len(), 1, "one declared attack resolves exactly once");
    let r = &state[0];
    assert_ne!(r.land, airbase, "the attack binds the reverse-hit component");
    assert!(
        (r.chance-0.9).abs()<1e-9,
        "GS5 short band at the third spotting turn: .7 + (1 spotting +1 not-under-fire +2 stationary)*.05 caps at the printed 90%"
    );
    assert_eq!(r.band, 0, "the declared 3nm lab is inside Short and outside Point Blank");
    let reverse = r.reverse.as_ref().expect("the reverse roll is recorded");
    assert_eq!(reverse.d100, reverse_d100);
    assert_eq!(reverse.component, category);
    assert_eq!(reverse.declared, airbase);
    let component_id = r.land;
    assert!(
        !w.events.iter().any(|e| matches!(&e.kind,
            harpoon_core::EventKind::RulesEvent { rule, detail, .. }
            if rule == "11.4.ngfs.cancelled" && detail.contains("spotting target changed"))),
        "the component result must never be fed back into the declared spotting task",
    );
    ngfs::late_batch(&mut w);
    let hits = &w.rules.aaw.as_ref().unwrap().surface.land_hits;
    assert_eq!(hits.len(), 1, "exactly one entry in the common land window");
    let hit = hits.values().next().unwrap();
    assert_eq!(hit.target, component_id, "the common hit targets the component");
    assert_eq!(hit.shooter_side, Side::Blue);
    assert_eq!(
        hit.damage, 25.0,
        "short-band 25 DP; barrels 2 are multiplier row 1-2 = x1 and there is no point blank at 2 nm"
    );
    harpoon_core::rules::surface2026::flush_land_hits(&mut w);
    assert!(w.rules.aaw.as_ref().unwrap().surface.land_hits.is_empty());
    let e = w.entity(component_id).unwrap();
    assert!(e.hp < e.hp_max, "source damage reached the component through the common path");
    let spotting = &w.rules.ground.as_ref().unwrap().spotting;
    assert_eq!(
        spotting.get(&ship).unwrap().target, airbase,
        "the own spotting task keeps its declared airfield target"
    );
    let _ = seed;
    advance(&mut w, 180);
    ngfs::validate_from_fire(&w, ship, airbase, "cn_pj45a")
        .expect("next Tactical Turn: the unchanged declared task still qualifies");
}

/// §11.6: "empty space" is an automatic failure — recorded as a final miss,
/// never rerolled, and the spent Tac slot is not refunded.
#[test]
fn n2_reverse_empty_space_is_a_final_recorded_miss_without_reroll() {
    let input = complex_input(
        serde_json::json!({
            "runways":["RUNWAY1"],
            "aircraft_storage":["STORE1"],
            "buildings":["BLDG1"]}),
        false,
    );
    let mut chosen = None;
    for seed in visual_seeds(1) {
        let w = fire_and_resolve(&input, seed, &["SHIP"]);
        let (d100, component) = reverse_event(&w);
        if component.is_none() {
            chosen = Some((w, d100));
            break;
        }
    }
    let (w, reverse_d100) =
        chosen.expect("a natural empty-space seed must exist in the searched range");
    let ship = id_of(&w, "SHIP");
    let airbase = id_of(&w, "AIRBASE");
    assert_eq!(
        w.events
            .iter()
            .filter(|e| matches!(&e.kind,
                harpoon_core::EventKind::RulesEvent { rule, .. } if rule == "11.reverse_hit"))
            .count(),
        1,
        "the automatic failure is final; no component reroll in the same attack"
    );
    let state = resolutions(&w);
    assert_eq!(state.len(), 1, "the miss itself is recorded");
    let r = &state[0];
    assert!(!r.hit, "empty space can never hit");
    assert_eq!(r.damage, 0.0);
    assert_eq!(r.d100, reverse_d100, "no gunnery die is rolled; the reverse die is the attack's die");
    let reverse = r.reverse.as_ref().expect("the reverse roll is recorded");
    assert_eq!(reverse.component, "empty");
    assert_eq!(reverse.declared, airbase);
    assert!(
        ngfs::validate_from_fire(&w, ship, airbase, "cn_pj45a")
            .unwrap_err()
            .contains("Tactical Turn"),
        "the attack happened and consumed its once-per-Tac slot"
    );
    let mut w = w;
    ngfs::late_batch(&mut w);
    assert!(w.rules.aaw.as_ref().unwrap().surface.land_hits.is_empty());
    let state = w.rules.ground.as_ref().unwrap().ngfs.as_ref().unwrap();
    assert_eq!(state.retained.len(), 1, "the unapplied miss stays in the serialized state");
}

/// An 11.6/11.7 category the scenario never declared keeps the whole attack
/// pending adjudication (die retained); it is neither dropped nor invented.
#[test]
fn n3_undeclared_reverse_component_stays_pending_adjudication() {
    let input = complex_input(
        serde_json::json!({
            "runways":["RUNWAY1"],
            "aircraft_storage":["STORE1"]}),
        false,
    );
    let mut chosen = None;
    for seed in visual_seeds(1) {
        let mut w = load_with_seed(input.clone(), seed);
        declare_spotting(&mut w, &["SHIP"]);
        let ship = id_of(&w, "SHIP");
        let airbase = id_of(&w, "AIRBASE");
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
        ngfs::prepare_from_fire(&mut w, ship, airbase, "cn_pj45a").unwrap();
        ngfs::resolve_referee(&mut w);
        let (d100, component) = reverse_event(&w);
        if !matches!(component.as_deref(), Some("buildings") | Some("taxiways") | Some("defenses")) {
            continue;
        }
        let after = mount(&w);
        chosen = Some((w, d100, component.unwrap(), before - after));
        break;
    }
    let (w, reverse_d100, category, shells_spent) =
        chosen.expect("a natural undeclared-component seed must exist in the searched range");
    assert_eq!(shells_spent, 2, "the shot was made; its adjudication is what pends");
    let state = resolutions(&w);
    assert_eq!(state.len(), 1, "the pending attack is retained, not dropped");
    let r = &state[0];
    assert!(!r.hit);
    let pending = r
        .pending
        .as_deref()
        .expect("an undeclared category is a named adjudication gap");
    assert!(
        pending.contains("no component assignment"),
        "the gap names the missing scenario component assignment: {pending}"
    );
    let reverse = r.reverse.as_ref().expect("the reverse roll is recorded");
    assert_eq!(reverse.d100, reverse_d100);
    assert_eq!(reverse.component, category);
    let mut w = w;
    ngfs::late_batch(&mut w);
    assert!(w.rules.aaw.as_ref().unwrap().surface.land_hits.is_empty());
    assert!(w.events.iter().any(|e| matches!(&e.kind,
        harpoon_core::EventKind::RulesEvent { rule, .. } if rule == "11.4.adjudication_pending")));
    let state = w.rules.ground.as_ref().unwrap().ngfs.as_ref().unwrap();
    assert_eq!(state.retained.len(), 1, "the pending die survives serialization");
}

/// §11.4 Map/Blind are Neutralization Fire: kept explicitly blocked for
/// destruction damage (F14 degradation is unprinted; destroyed DP is not a
/// substitute). Green guard for the existing boundary.
#[test]
fn n4_map_blind_neutralization_stays_blocked_without_borrowed_destruction_dp() {
    let input = complex_input(
        serde_json::json!({"runways":["RUNWAY1"],"aircraft_storage":["STORE1"],"buildings":["BLDG1"]}),
        false,
    );
    let mut w = load_with_seed(input, visual_seeds(1)[0]);
    advance(&mut w, 60);
    let airbase = id_of(&w, "AIRBASE");
    let ship = id_of(&w, "SHIP");
    let contact = w.sides[Side::Blue.index()]
        .contacts
        .values()
        .find(|c| c.about == airbase && !c.stale)
        .expect("the lookout holds an airfield contact")
        .id;
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Ground {
                unit: ship,
                action: ground::Action::Spot {
                    contact,
                    method: ground::Spotting::Map,
                },
            },
        )],
    );
    advance(&mut w, 10);
    let ammo = |w: &World| {
        w.entity(ship)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "cn_pj45a")
            .unwrap()
            .ammo
    };
    let before = ammo(&w);
    let err = ngfs::prepare_from_fire(&mut w, ship, airbase, "cn_pj45a")
        .expect_err("Map-fire neutralization must not resolve destruction damage");
    assert!(err.contains("neutralization"), "{err}");
    assert_eq!(ammo(&w), before, "a blocked neutralization attack spends nothing");
    assert!(resolutions(&w).is_empty());
    assert!(w
        .rules
        .ground
        .as_ref()
        .unwrap()
        .ngfs
        .as_ref()
        .unwrap()
        .jobs
        .is_empty());
}

/// §11.4/PDF117: "modify the hit chance for each ship by the number of ships
/// firing at that target, minus one times the -2 overconcentration penalty."
/// For a whole airfield the target is the declared complex, so both ships
/// take the penalty exactly once even when their reverse rolls select
/// different components.
#[test]
fn n5_two_ships_on_one_airfield_take_the_n_minus_1_penalty_once_each() {
    let input = complex_input(
        serde_json::json!({
            "runways":["RUNWAY1"],
            "aircraft_storage":["STORE1"],
            "buildings":["BLDG1"]}),
        true,
    );
    let mut chosen = None;
    for seed in visual_seeds(2) {
        let w = fire_and_resolve(&input, seed, &["SHIP", "SHIP2"]);
        if resolutions(&w).len() == 2 && resolutions(&w).iter().all(|r|r.pending.is_none()&&r.chance>0.) {
            chosen = Some(w);
            break;
        }
    }
    let w = chosen.expect("both native attacks must resolve for a calibrated seed");
    let mut chances = Vec::new();
    let mut lands = Vec::new();
    for r in resolutions(&w) {
        chances.push(r.chance);
        lands.push(r.land);
        assert!(r.pending.is_none(), "no necessary input may pend in this lab");
    }
    assert_eq!(chances.len(), 2);
    for chance in &chances {
        assert!(
            (chance - 0.8).abs() < 1e-9,
            "GS5 short .7 + (1 spotting +1 not-under-fire +2 stationary -2 overconcentration)*.05 = .8 for EACH ship against the DECLARED airfield; got {chances:?} with components {lands:?}"
        );
    }
}
#[test]
fn n6_shells_landing_in_empty_space_still_count_as_firing_at_the_same_complex() {
    let input=complex_input(serde_json::json!({"runways":["RUNWAY1"],"aircraft_storage":["STORE1"],"buildings":["BLDG1"]}),true);
    // Preconstruction original dice, not injected rolls or state: two native
    // lookouts, then actual Ship1->runway and Ship2->empty at the same tick.
    let seed=visual_seeds(2).into_iter().find(|seed|{
        let roll=|service:&str|{let mut r=harpoon_core::Rng::new(*seed).derive(harpoon_core::Rng::hash_tag(422,3,&format!("11.reverse:ngfs:{service}")));1+(r.next_f64()*100.)as u8};
        roll("witness")<=20&&roll("second")>=61
    }).expect("preconstruction original dice in the required boundary");
    let w=fire_and_resolve(&input,seed,&["SHIP","SHIP2"]);let rs=resolutions(&w);
    assert_eq!(rs.len(),2);assert_eq!(rs.iter().filter(|r|r.reverse.as_ref().is_some_and(|v|v.component=="empty")).count(),1);
    let attack=rs.iter().find(|r|r.chance>0.).unwrap();
    // A nonpenetrating land hit may remain pending; that damage uncertainty
    // cannot change which ships actually fired or their source hit chances.
    assert!((attack.chance-0.8).abs()<1e-9,"both actual firing ships count before reverse location: {}",attack.chance);
}
#[test]
fn n7_original_d100_equal_to_printed_ninety_percent_is_a_hit() {
    let input=complex_input(serde_json::json!({"runways":["RUNWAY1"],"aircraft_storage":["STORE1"],"buildings":["BLDG1"]}),false);
    let seed=visual_seeds(1).into_iter().find(|seed|{
        let mut a=harpoon_core::Rng::new(*seed).derive(harpoon_core::Rng::hash_tag(422,2,"11.reverse:ngfs:witness"));
        let mut b=harpoon_core::Rng::new(*seed).derive(harpoon_core::Rng::hash_tag(422,1,"11.4.ngfs:witness:3"));
        1+(a.next_f64()*100.)as u8<=20&&1+(b.next_f64()*100.)as u8==90
    }).expect("ordinary seed yielding native lookout, runway reverse roll and original D10090");
    let w=fire_and_resolve(&input,seed,&["SHIP"]);let rs=resolutions(&w);let r=&rs[0];
    assert_eq!(r.d100,90);assert!((r.chance-0.9).abs()<1e-9);
    assert!(r.hit,"floating arithmetic must not turn printed 90% with original D10090 into a miss");
}

#[test]
fn ngfs_and_starshell_share_only_the_selected_physical_mount() {
    for index in [0,1] {
        let mut w=dual_domain_starshell_world(Some(index)); let ship=id_of(&w,"SHIP");
        let target=id_of(&w,"NAVAL");
        let cid=w.sides[0].contacts.values().find(|c|!c.stale && c.about==target).unwrap().id;
        let star=Command::Detection{unit:ship,action:detection::DetectionAction::FireStarshell{contact:cid,weapon:"cn_pj45a".into()}};
        let land=dual_fire(&w,"AIRBASE");
        let initial=w.entity(ship).unwrap().weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().ammo;
        harpoon_core::sim::tick(&mut w,&[(Side::Blue,star),(Side::Blue,land)]);
        let rules:Vec<_>=w.events.iter().filter_map(|e|match &e.kind {harpoon_core::EventKind::RulesEvent{rule,..}=>Some(rule.as_str()),_=>None}).collect();
        if index==1 {
            assert!(!rules.contains(&"5.8.8.starshell_resolution"));
            assert_eq!(w.entity(ship).unwrap().weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().ammo,initial);
            assert_eq!(w.rules.aaw.as_ref().unwrap().surface.gunnery.fits[&ship].starshell_stock,Some(4));
        } else {
            assert!(rules.contains(&"5.8.8.starshell_resolution"));
            assert_eq!(w.entity(ship).unwrap().weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().ammo,initial-2);
            assert_eq!(w.rules.aaw.as_ref().unwrap().surface.gunnery.fits[&ship].starshell_stock,Some(2));
        }
    }
}

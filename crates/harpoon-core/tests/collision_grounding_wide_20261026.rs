//! §3.3/3.4 first source candidates: real motion and common damage only.
//! Author-labelled navigation/shore/armor fits, no private contact/HP/dice/ground flag writes.
use harpoon_core::{Database,EntityId,EventKind,Rng,Scenario,Side,World};
use harpoon_core::rules::navigation2026 as n;
use serde_json::{json,Value};
fn fit(size:&str)->Value {json!({"source":"scenario_assumption: explicit controlled navigation fit; no real CPP/readiness inference",
    "size":size,"merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,
    "undamaged_max_kts":30,"propulsion":"NonSteam","readiness":"Underway"})}
fn hull()->Value {json!({"source":"scenario_assumption: explicit no-armor/no-reinforced-bow/no-civilian controlled hull",
    "civilian":false,"armor_points":0,"reinforced_bow":false})}
fn collision_seed()->u64 {(0..100000).find(|s|{let mut r=Rng::new(*s).derive(Rng::hash_tag(1,1,"3.3.contact.1:2.0"));
    let a=(r.next_f64()*10.).floor() as u8+1;let b=(r.next_f64()*10.).floor() as u8+1;a+b>=16}).unwrap()}
fn grounding_seed()->u64 {(0..100000).find(|s|{let mut r=Rng::new(*s).derive(Rng::hash_tag(180,1,"3.4.grounding.0"));
    (r.next_f64()*100.).floor() as u8+1<=30}).unwrap()}
fn value(collision:bool)->Value {
    let mut v=json!({"id":"wide-collision-grounding","name":"scenario_assumption: source physical movement hazard experiment",
      "seed":if collision{collision_seed()}else{grounding_seed()},"duration_s":40000,"weather":{"visibility_nm":40,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
        "navigation_2026":{"source":"scenario_assumption: no guessed real hull equipment",
          "units":{"A":fit("C")},"hazards":{"source":"English2026 PDF30-32 §3.3/3.4; explicit experiment geometry and armor",
            "collisions":collision,"visibility_percent":100,"units":{"A":hull()},
            "grounding":if collision{Value::Null}else{json!({"source":"scenario_assumption: straight declared shore; no bathymetry or draft inference",
              "shorelines":[[[-0.05,-5],[-0.05,5]]],"safe_areas":[]})}}}},
      "units":[{"side":"Blue","class":"us_burke_2a","name":"A","pos":[0,0],"course_deg":0,"speed_kts":5,"fire_on":false},
        {"side":"Red","class":"cn_039a","name":"DISTANT","pos":[50,50],"course_deg":0,"speed_kts":0,"depth_m":100,"fire_on":false}]});
    if collision {
        v["units"][0]["pos"]=json!([0,-0.0625]);v["units"][0]["speed_kts"]=json!(10);
        v["units"].as_array_mut().unwrap().insert(1,json!({"side":"Blue","class":"us_burke_2a","name":"B","pos":[0,0.0625],"course_deg":180,"speed_kts":10,"fire_on":false}));
        v["context"]["navigation_2026"]["units"]["B"]=fit("C");v["context"]["navigation_2026"]["hazards"]["units"]["B"]=hull();
    }
    v
}
fn load(v:Value)->World {
    let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
    for class in ["us_burke_2a","cn_039a"] {let p=db.platforms.get_mut(class).unwrap();p.weapons.clear();p.sensors.clear();}
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(v).unwrap(),db).unwrap()
}
fn id(w:&World,name:&str)->EntityId {w.entities.values().find(|e|e.name==name).unwrap().id}
fn run(w:&mut World,to:i64) {while w.tick<to {assert!(!w.over);harpoon_core::sim::tick(w,&[]);}}
fn events(w:&World,rule:&str)->Vec<Value> {w.events.iter().filter_map(|e|match &e.kind {
    EventKind::RulesEvent{rule:r,detail,..} if r==rule=>Some(serde_json::from_str(detail).unwrap()),_=>None}).collect()}
#[test]
fn c01_actual_close_pass_resolves_one_collision_and_commits_both_sides_to_common_damage() {
    let mut w=load(value(true));let (a,b)=(id(&w,"A"),id(&w,"B"));let hp=[w.entity(a).unwrap().hp,w.entity(b).unwrap().hp];
    run(&mut w,1);assert!(w.entity(a).unwrap().hp<hp[0],"source2D10>=16 at the actual250yd swept close pass must reach common damage");
    assert!(w.entity(b).unwrap().hp<hp[1],"a physical collision damages both source hulls in the same batch");
    let e=events(&w,"3.3.collision");assert_eq!(e.len(),1);assert_eq!(e[0]["collision_type"],"glancing");
    run(&mut w,3);assert_eq!(events(&w,"3.3.collision").len(),1,"one persistent encounter is not rerolled every second");
}
#[test]
fn c02_real_nearshore_turn_uses_grounding_damage_stops_and_retains_original_refloat_clock() {
    let mut w=load(value(false));let a=id(&w,"A");let hp=w.entity(a).unwrap().hp;run(&mut w,180);
    assert!(w.entity(a).unwrap().hp<hp,"sourceD6 times actual speed is committed underwater grounding damage");
    assert_eq!(w.entity(a).unwrap().speed_kts,0.0);let own=n::own_view(&w,a).unwrap();
    assert_eq!(own["hazards"]["grounding"]["grounded"],true);
    let due=own["hazards"]["grounding"]["refloat_at"].as_i64().unwrap();assert!((180+3600..=180+21600).contains(&due));
    let c=harpoon_core::Command::SetSpeed{unit:a,speed_kts:20.0};c.validate_source(&w,Side::Blue).unwrap();harpoon_core::sim::tick(&mut w,&[(Side::Blue,c)]);
    assert_eq!(w.entity(a).unwrap().speed_kts,0.0,"ordinary speed orders cannot erase a real grounding");
    assert_eq!(n::own_view(&w,a).unwrap()["hazards"]["grounding"]["refloat_at"],due);
}
#[test]
fn c03_real_exposure_and_grounding_clock_roundtrip_without_a_new_roll() {
    let mut w=load(value(false));run(&mut w,179);let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    run(&mut w,181);run(&mut restored,181);assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&restored).unwrap());
    assert_eq!(events(&w,"3.4.grounded").len(),1);
}
#[test]
fn c04_explicit_actual_harbor_polygon_is_exempt_from_shoal_rolls() {
    let mut v=value(false);v["context"]["navigation_2026"]["hazards"]["grounding"]["safe_areas"]=json!([
      {"source":"scenario_assumption: explicit mapped safe harbor, not inferred from a base name","kind":"harbor","polygon":[[-1,-1],[1,-1],[1,1],[-1,1]]}]);
    let mut w=load(v);let a=id(&w,"A");let hp=w.entity(a).unwrap().hp;run(&mut w,180);
    assert_eq!(w.entity(a).unwrap().hp,hp);assert!(events(&w,"3.4.grounded").is_empty());
}

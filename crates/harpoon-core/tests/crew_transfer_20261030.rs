use harpoon_core::{Command,Database,Scenario,Side,World,EntityId};
use serde_json::json;
fn id(w:&World,n:&str)->EntityId {w.entities.values().find(|e|e.name==n).unwrap().id}
fn fixture(speed:f64)->World {
 let sc:Scenario=serde_json::from_value(json!({"id":"transfer-test","name":"scenario_assumption: factor geometry","seed":20261030,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","expanded_damage_2026":{"source":"scenario_assumption: factor inventories","units":{"A":{"source":"scenario_assumption: A","transfer_factors":{"source":"scenario_assumption: factor inventory and 40yd station","healthy":3,"injured":2,"capacity":12,"alongside_yards":40}},"B":{"source":"scenario_assumption: B","transfer_factors":{"source":"scenario_assumption: factor inventory and 40yd station","healthy":0,"injured":0,"capacity":12,"alongside_yards":40}}}}},"units":[{"side":"Blue","class":"us_burke_2a","name":"A","pos":[0,0],"course_deg":0,"speed_kts":speed,"fire_on":false},{"side":"Blue","class":"us_burke_2a","name":"B","pos":[0,0.01],"course_deg":0,"speed_kts":speed,"fire_on":false},{"side":"Red","class":"cn_093","name":"FAR","pos":[200,200],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
 harpoon_core::load_scenario(sc,Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap()).unwrap()
}
fn command(w:&World)->Command {serde_json::from_value(json!({"cmd":"search_and_rescue","unit":id(w,"A"),"action":{"type":"transfer_crew","receiver":id(w,"B"),"healthy":3,"injured":2}})).unwrap()}
#[test]
fn stationary_public_transfer_preserves_factors_and_roundtrip() {
 let mut w=fixture(0.);let a=id(&w,"A");let b=id(&w,"B");let c=command(&w);c.validate_source(&w,Side::Blue).unwrap();harpoon_core::sim::tick(&mut w,&[(Side::Blue,c)]);
 for _ in 0..210 {harpoon_core::sim::tick(&mut w,&[]);}
 let s=&w.rules.expanded_damage.as_ref().unwrap().transfers;assert_eq!((s.inventories[&a].healthy,s.inventories[&a].injured),(0,0));assert_eq!((s.inventories[&b].healthy,s.inventories[&b].injured),(3,2));
 let saved=serde_json::to_vec(&w).unwrap();let restored:World=serde_json::from_slice(&saved).unwrap();assert_eq!(saved,serde_json::to_vec(&restored).unwrap());
}
#[test]
fn public_transfer_rejects_overspeed_without_mutating_world() {let w=fixture(11.);let before=serde_json::to_vec(&w).unwrap();assert!(command(&w).validate_source(&w,Side::Blue).is_err());assert_eq!(before,serde_json::to_vec(&w).unwrap());}
#[test]
fn moving_transfer_conserves_factors_on_success_or_overboard() {
 let mut w=fixture(4.);let a=id(&w,"A");let b=id(&w,"B");let c=command(&w);c.validate_source(&w,Side::Blue).unwrap();harpoon_core::sim::tick(&mut w,&[(Side::Blue,c)]);
 for _ in 0..61 {harpoon_core::sim::tick(&mut w,&[]);}
 let s=&w.rules.expanded_damage.as_ref().unwrap().transfers;
 let afloat:u32=s.overboard.iter().map(|x|x.healthy+x.injured).sum();
 assert_eq!(s.inventories[&a].healthy+s.inventories[&a].injured+s.inventories[&b].healthy+s.inventories[&b].injured+afloat,5);
 assert!(s.jobs.is_empty());
}
#[test]
fn speed_gap_is_source_pending_and_no_world_mutation() {
 let w=fixture(5.5);let before=serde_json::to_vec(&w).unwrap();
 assert!(command(&w).validate_source(&w,Side::Blue).unwrap_err().contains("source_pending"));
 assert_eq!(before,serde_json::to_vec(&w).unwrap());
}

#[test]
fn old_profiles_load_without_expanded_state() {
 for profile in ["rts-legacy-v1","hv2026-rts-v3"] {
  let sc:Scenario=serde_json::from_value(json!({"id":"legacy-transfer-control","name":"old profile without expanded state","seed":1,"duration_s":100,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":profile},"units":[{"side":"Blue","class":"us_burke_2a","name":"A","pos":[0,0],"speed_kts":0,"fire_on":false}]})).unwrap();
  let db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
  let w=harpoon_core::load_scenario(sc,db).expect("old profiles must load without expanded state");
  assert!(w.rules.expanded_damage.is_none());
 }
}

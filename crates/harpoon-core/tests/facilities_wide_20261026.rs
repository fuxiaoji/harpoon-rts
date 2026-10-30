//! Candidates for one combined source-red batch; no new Rust type is needed.
//! English2026 PDF116/118 source casualties; all service assignments/timing
//! and independent stock are declared experimental setup, not actual-base fits.
use harpoon_core::{Command,Database,EntityId,Scenario,Side,World};
use harpoon_core::rules::ground2026 as ground;
use serde_json::{json,Value};
fn id(w:&World,name:&str)->EntityId{w.entities.values().find(|e|e.name==name).unwrap().id}
fn configuration(magazine:bool)->Value{
 let mut depot=json!({"source":"scenario_assumption: independent experimental ammunition ledger","stocks":{"cn_yj18":3,"cn_pj45a":3}});
 if magazine{depot["kind"]=json!("magazine_bunker");}
 let mut facilities=json!({"BERTH":{"source":"English2026 PDF118 Ship Berth, experimental placement","template":"ship_berth"},"BUNKER":{"source":"English2026 PDF118 Command Bunker, experimental assignment","template":"command_bunker"}});
 if magazine{facilities["MAG"]=json!({"source":"English2026 PDF118 Magazine Bunker, explicit laboratory stock","template":"magazine_bunker"});}
 let link=|peers:Value|json!({"source":"scenario_assumption: explicit functioning voice control radio, not real Annex fit","protocol":"laboratory-gci-radio","send":true,"receive":true,"operational":true,"peers":peers,"warning_kind":"voice"});
 json!({"id":"facilities-wide-source-lab","name":"scenario_assumption: actual GCI vector and special magazine consumers","seed":26,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},
  "context":{"rules_profile":"hv2026-rts-v4","communications_2026":{"source":"scenario_assumption: explicit same-side radio assignments","units":{
   "GCI":{"source":"scenario_assumption","fits":{"lab":link(json!(["AIR"]))}},"AIR":{"source":"scenario_assumption","fits":{"lab":link(json!([]))}}}},
  "ground_2026":{"source":"scenario_assumption: explicit service assignments","facilities":facilities,
   "gci_services":{"control":{"source":"scenario_assumption: exact GCI controller/bunker/aircraft/radio binding","controller":"GCI","command_bunker":"BUNKER","fit":"lab","aircraft":["AIR"]}},
   "berth_depots":{"MAG":depot},"berth_services":{
    "missiles":{"source":"scenario_assumption","depot":"MAG","berth":"BERTH","recipient":"SHIP","mount_index":1,"weapon":"cn_yj18","capacity":18,"radius_nm":0.01,"max_speed_kts":0,"seconds_per_round":3},
    "guns":{"source":"scenario_assumption","depot":"MAG","berth":"BERTH","recipient":"SHIP","mount_index":5,"weapon":"cn_pj45a","capacity":44,"radius_nm":0.01,"max_speed_kts":0,"seconds_per_round":3}}}},
 "units":[{"side":"Blue","class":"airfield","name":"GCI","pos":[0,0],"fire_on":false},
 {"side":"Blue","class":"airfield","name":"BUNKER","pos":[0,0],"fire_on":false},
 {"side":"Blue","class":"airfield","name":"MAG","pos":[0,0],"fire_on":false},
 {"side":"Blue","class":"airfield","name":"BERTH","pos":[0,0],"fire_on":false},
 {"side":"Blue","class":"cn_052d","name":"SHIP","pos":[0,0],"speed_kts":0,"fire_on":false},
 {"side":"Blue","class":"us_f18e","name":"AIR","pos":[0,1],"course_deg":0,"speed_kts":300,"altitude_m":1000,"fire_on":false}]})
}
fn load(magazine:bool)->World{
 let scenario:Scenario=serde_json::from_value(configuration(magazine)).unwrap();
 let db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
 harpoon_core::load_scenario(scenario,db).expect("explicit source facility configuration must load without ignored fields")
}
fn gci(w:&World,course:f64)->Command{serde_json::from_value(json!({"cmd":"ground","unit":id(w,"GCI"),"action":{"type":"gci_vector","service":"control","aircraft":id(w,"AIR"),"course_deg":course}})).unwrap()}
fn supply(w:&World,key:&str)->Command{serde_json::from_value(json!({"cmd":"ground","unit":id(w,"SHIP"),"action":{"type":"alongside_resupply","service":key,"rounds":1}})).unwrap()}
fn step(w:&mut World,commands:&[(Side,Command)]){harpoon_core::sim::tick(w,commands)}
fn advance(w:&mut World,to:i64){while w.tick<to{step(w,&[]);}}

#[test]
fn f01_exact_gci_assignment_survives_and_the_real_aircraft_receives_a_vector(){
 let mut w=load(false);
 let cfg=serde_json::to_value(w.context.ground_2026.as_ref().unwrap()).unwrap();
 assert_eq!(cfg["gci_services"]["control"]["controller"],"GCI","source GCI service cannot be silently discarded");
 let cmd=gci(&w,90.);let before=serde_json::to_vec(&w).unwrap();
 assert!(cmd.validate_source(&w,Side::Blue).is_ok());assert_eq!(before,serde_json::to_vec(&w).unwrap());
 step(&mut w,&[(Side::Blue,cmd)]);assert_eq!(w.entity(id(&w,"AIR")).unwrap().ordered_course_deg,90.);
}
#[test]
fn f02_real_command_bunker_casualty_prevents_control_and_leaves_aircraft_unchanged(){
 let mut w=load(false);let bunker=id(&w,"BUNKER");
 ground::apply_damage(&mut w,bunker,1.,1,"scenario_assumption: adjudicated command bunker critical").unwrap();
 let cmd=gci(&w,90.);let before=serde_json::to_vec(&w).unwrap();
 assert!(cmd.validate_source(&w,Side::Blue).is_err());assert_eq!(before,serde_json::to_vec(&w).unwrap());
 step(&mut w,&[(Side::Blue,cmd)]);assert_eq!(w.entity(id(&w,"AIR")).unwrap().ordered_course_deg,0.);
}
#[test]
fn f03_gci_never_overwrites_explicit_manual_aircraft_or_accepts_private_keys(){
 let mut w=load(false);let a=id(&w,"AIR");step(&mut w,&[(Side::Blue,Command::SetUnitAutomation{unit:a,enabled:false})]);
 assert!(gci(&w,90.).validate_source(&w,Side::Blue).is_err());
 let bad=json!({"cmd":"ground","unit":id(&w,"GCI"),"action":{"type":"gci_vector","service":"control","aircraft":a,"course_deg":90,"quality":"Good"}});
 assert!(serde_json::from_value::<Command>(bad).is_err());
}
#[test]
fn f04_magazine_critical_restricts_actual_service_to_gun_ammunition_for_the_original_d10(){
 let mut w=load(true);let mag=id(&w,"MAG");let ship=id(&w,"SHIP");
 ground::apply_damage(&mut w,mag,1.,1,"scenario_assumption: adjudicated magazine critical").unwrap();
 let due=w.rules.ground.as_ref().unwrap().facilities[&mag].gun_only_until.unwrap();assert!((180..=1800).contains(&due));
 assert!(supply(&w,"missiles").validate_source(&w,Side::Blue).is_err());
 let gun=supply(&w,"guns");assert!(gun.validate_source(&w,Side::Blue).is_ok());
 step(&mut w,&[(Side::Blue,gun)]);advance(&mut w,4);assert_eq!(w.entity(ship).unwrap().weapons[5].ammo,41);
 advance(&mut w,due);assert!(supply(&w,"missiles").validate_source(&w,Side::Blue).is_ok());
}
#[test]
fn f05_structural_half_dp_and_rubble_are_not_cleared_by_a_magazine_temporary_timer(){
 let mut w=load(true);let mag=id(&w,"MAG");
 ground::apply_damage(&mut w,mag,30.,0,"scenario_assumption: adjudicated source halfDP").unwrap();
 assert!(supply(&w,"guns").validate_source(&w,Side::Blue).is_err());advance(&mut w,1801);
 assert!(supply(&w,"missiles").validate_source(&w,Side::Blue).is_err());
}
#[test]
fn f06_pending_gci_and_magazine_restrictions_roundtrip_the_complete_world(){
 let mut w=load(true);let mag=id(&w,"MAG");ground::apply_damage(&mut w,mag,1.,1,"scenario_assumption: adjudicated magazine critical").unwrap();
 let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
 let order=gci(&w,90.);step(&mut w,&[(Side::Blue,order.clone())]);step(&mut restored,&[(Side::Blue,order)]);
 advance(&mut w,181);advance(&mut restored,181);assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
}

#[test]
fn f07_queued_gci_restores_then_rechecks_a_real_bunker_casualty_before_delivery(){
 let mut w=load(false);let controller=id(&w,"GCI");let aircraft=id(&w,"AIR");let bunker=id(&w,"BUNKER");
 let Command::Ground{action,..}=gci(&w,90.) else {unreachable!()};
 ground::apply(&mut w,controller,&action).unwrap();
 let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
 for world in [&mut w,&mut restored]{ground::apply_damage(world,bunker,1.,1,"scenario_assumption: actual post-request bunker casualty").unwrap();step(world,&[]);assert_eq!(world.entity(aircraft).unwrap().ordered_course_deg,0.);}
 assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
}
#[test]
fn f08_gci_radio_damage_is_independent_of_the_bunker_and_rechecked_at_delivery(){
 let mut w=load(false);let controller=id(&w,"GCI");let aircraft=id(&w,"AIR");
 let Command::Ground{action,..}=gci(&w,90.) else {unreachable!()};ground::apply(&mut w,controller,&action).unwrap();
 harpoon_core::rules::communications2026::set_communication_cause(&mut w,aircraft,"scenario_assumption: actual receive radio casualty",true).unwrap();
 step(&mut w,&[]);assert_eq!(w.entity(aircraft).unwrap().ordered_course_deg,0.);
 harpoon_core::rules::communications2026::set_communication_cause(&mut w,aircraft,"scenario_assumption: actual receive radio casualty",false).unwrap();
 let cmd=gci(&w,90.);step(&mut w,&[(Side::Blue,cmd)]);assert_eq!(w.entity(aircraft).unwrap().ordered_course_deg,90.);
}
#[test]
fn f09_conflicting_simultaneous_vectors_never_select_the_first_client(){
 let mut w=load(false);let mut other=w.clone();
 let a=gci(&w,90.);let b=gci(&w,270.);
 step(&mut w,&[(Side::Blue,a.clone()),(Side::Blue,b.clone())]);
 step(&mut other,&[(Side::Blue,b),(Side::Blue,a)]);
 assert_eq!(w.entity(id(&w,"AIR")).unwrap().ordered_course_deg,0.);
 // The audit keeps original client input ordering. Compare actual simulation
 // state and events after removing only that intentionally distinct input log.
 w.command_log.clear();other.command_log.clear();
 assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&other).unwrap());
}

#[test]
fn f10_a_second_magazine_casualty_never_shortens_an_existing_source_restriction(){
 let mut w=load(true);let mag=id(&w,"MAG");
 ground::apply_damage(&mut w,mag,1.,1,"scenario_assumption: first independent critical").unwrap();
 let first=w.rules.ground.as_ref().unwrap().facilities[&mag].gun_only_until.unwrap();
 ground::apply_damage(&mut w,mag,1.,1,"scenario_assumption: second independent critical").unwrap();
 assert!(w.rules.ground.as_ref().unwrap().facilities[&mag].gun_only_until.unwrap()>=first);
}

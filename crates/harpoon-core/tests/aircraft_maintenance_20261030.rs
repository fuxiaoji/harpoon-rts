//! §11.5 maintenance work under real §14.9 repair; aircraft hit and facility
//! critical are explicit adjudicated inputs, not natural combat claims.
use harpoon_core::{Database,Scenario,World,EntityId,Command,Side};
use harpoon_core::rules::{expanded_damage2026 as e,ground2026 as g};
use serde_json::json;
fn id(w:&World,name:&str)->EntityId {w.entities.values().find(|e|e.name==name).unwrap().id}
fn fixture(seed:u64,rate:Option<f64>)->(World,EntityId,EntityId) {
 let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();db.platforms.get_mut("us_f18e").unwrap().sensors.clear();
 let mut base=db.platforms["us_ford"].clone();base.id="exercise_airbase".into();base.name="scenario_assumption: abstract building body, source target from configured facility".into();base.kind=harpoon_core::types::PlatformKind::Base;base.deck=None;base.weapons.clear();base.sensors.clear();base.rule_data=None;db.platforms.insert(base.id.clone(),base);
 let sc:Scenario=serde_json::from_value(json!({"id":"air-maintenance","name":"scenario_assumption: declared station and facility","seed":seed,"duration_s":100000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","ground_2026":{"source":"scenario_assumption: laboratory buildings","facilities":{"HANGAR":{"template":"hangar_large","custom":null,"source":"scenario_assumption: actual parked individual","stored_aircraft":["AIR"]},"MAINT":{"template":"maintenance_small","custom":null,"source":"scenario_assumption: source maintenance building","repair_rate_factor":rate}},"aircraft_maintenance":{"AIR":{"source":"scenario_assumption: explicit maintenance station","provider":"MAINT","maximum_distance_nm":0.1}}},"expanded_damage_2026":{"source":"scenario_assumption: detailed damage and first-rank land repair","optional":{"aircraft_damage":"immediate_detailed"},"units":{"AIR":{"source":"scenario_assumption: first-rank land aircraft","rank":"first","land_based_aircraft":true,"cruise_speed_kts":600,"crew":2}}}},"units":[{"side":"Blue","class":"exercise_airbase","name":"HANGAR","pos":[0,0],"speed_kts":0,"fire_on":false},{"side":"Blue","class":"exercise_airbase","name":"MAINT","pos":[0,0],"speed_kts":0,"fire_on":false},{"side":"Blue","class":"us_f18e","name":"AIR","pos":[0,0],"speed_kts":0,"altitude_m":0,"fire_on":false},{"side":"Red","class":"cn_093","name":"FAR","pos":[200,200],"depth_m":50,"speed_kts":0,"fire_on":false}]})).unwrap();
 let w=harpoon_core::load_scenario(sc,db).unwrap();let a=id(&w,"AIR");let m=id(&w,"MAINT");(w,a,m)
}
fn damaged(rate:Option<f64>,disabled:bool)->(World,EntityId,EntityId) {
 for seed in 1..=32 {let (mut w,a,m)=fixture(seed,rate);if !e::aircraft_hit(&mut w,a,"maintenance-input") || !w.entity(a).is_some_and(|x|x.alive) || !e::has_recorded_repair_work(&w,a) {continue;}
  if disabled {g::apply_damage(&mut w,m,0.,1,"maintenance-critical").unwrap();assert!(!g::operational(&w,m));}
  let cmd=Command::DamageControl{unit:a,action:"begin_repairs".into()};harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);
  assert!(w.rules.repairs.as_ref().unwrap().units[&a].session.is_some(),"repair premise/order: {:?}",w.events.iter().filter_map(|ev|match &ev.kind {harpoon_core::EventKind::RulesEvent{rule,detail,..} if rule.contains("repair")||rule.contains("damage_control")=>Some((rule.clone(),detail.clone())),_=>None}).collect::<Vec<_>>());
  harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::DamageControl{unit:a,action:"repairs_auto".into()})]);
  assert!(w.rules.repairs.as_ref().unwrap().units[&a].session.as_ref().unwrap().automatic);
  return(w,a,m);
 }panic!("fixture precondition: no surviving detailed aircraft damage in 32 seeds")
}
fn advance(w:&mut World,n:usize) {for _ in 0..n {assert!(!w.over);harpoon_core::sim::tick(w,&[]);}}
fn work(w:&World,a:EntityId)->(f64,Option<String>) {let x=w.rules.ground.as_ref().unwrap().aircraft_maintenance.as_ref().unwrap().bindings[&a].work.as_ref().unwrap();(x.earned_seconds,x.pending.clone())}
fn checks(w:&World,a:EntityId)->usize {w.rules.expanded_damage.as_ref().unwrap().units[&a].aircraft_repairs.len()}
#[test]
fn healthy_and_half_rate_use_real_elapsed_work_and_repair_checkpoint() {
 let (mut healthy,a,_)=damaged(Some(0.5),false);let (mut half,b,_)=damaged(Some(0.5),true);
 advance(&mut healthy,21601);advance(&mut half,21601);
 assert!(work(&healthy,a).0>=21600.);assert!(work(&half,b).0>=10800. && work(&half,b).0<21600.);
 assert!(checks(&healthy,a)>0);assert_eq!(checks(&half,b),0);
 let bytes=serde_json::to_vec(&half).unwrap();let restored:World=serde_json::from_slice(&bytes).unwrap();assert_eq!(bytes,serde_json::to_vec(&restored).unwrap());
 advance(&mut half,21600);assert!(work(&half,b).0>=21600.);assert!(checks(&half,b)>0);
}
#[test]
fn unknown_damaged_facility_pauses_without_forging_work() {
 let (mut w,a,_)=damaged(None,true);advance(&mut w,600);
 let (earned,pending)=work(&w,a);assert_eq!(earned,0.);assert!(pending.is_some());assert_eq!(checks(&w,a),0);
 let bytes=serde_json::to_vec(&w).unwrap();let restored:World=serde_json::from_slice(&bytes).unwrap();assert_eq!(bytes,serde_json::to_vec(&restored).unwrap());
}

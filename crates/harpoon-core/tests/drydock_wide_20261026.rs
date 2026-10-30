//! English2026 PDF118 exact pump/gate casualties. Location, initial water
//! state and cycle duration are explicit scenario operating assumptions.
use harpoon_core::{Command,Database,EntityId,Scenario,Side,World};
use harpoon_core::rules::ground2026 as g;
use serde_json::{json,Value};
fn id(w:&World,n:&str)->EntityId{w.entities.values().find(|e|e.name==n).unwrap().id}
fn load()->World{
 let scenario:Scenario=serde_json::from_value(json!({"id":"dock-source-lab","name":"scenario_assumption: actual drydock water-cycle consumer","seed":26,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},
 "context":{"rules_profile":"hv2026-rts-v4","ground_2026":{"source":"scenario_assumption: exact experimental dock assignment","facilities":{
 "PUMP":{"source":"English2026 PDF118, explicit placement","template":"drydock_pump"},"GATE":{"source":"English2026 PDF118, explicit placement","template":"drydock_gate"}},
 "drydocks":{"dock":{"source":"scenario_assumption: initial water/geometry/cycle duration, not real facility performance","controller":"DOCK","pump":"PUMP","gate":"GATE","initial_flooded":true,"seconds_per_cycle":3}}}},
 "units":[{"side":"Blue","class":"airfield","name":"DOCK","pos":[0,0],"fire_on":false},{"side":"Blue","class":"airfield","name":"PUMP","pos":[0,0],"fire_on":false},{"side":"Blue","class":"airfield","name":"GATE","pos":[0,0],"fire_on":false}]})).unwrap();
 harpoon_core::load_scenario(scenario,Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()).unwrap()
}
fn action()->g::Action{serde_json::from_value(json!({"type":"pump_dock","service":"dock","flooded":false})).unwrap()}
fn order(w:&World)->Command{Command::Ground{unit:id(w,"DOCK"),action:action()}}
fn view(w:&World)->Value{g::own_view(w,id(w,"DOCK")).unwrap()["drydocks"]["services"]["dock"].clone()}
fn advance(w:&mut World,to:i64){while w.tick<to{harpoon_core::sim::tick(w,&[])}}
#[test]
fn dock01_explicit_water_cycle_is_an_actual_timed_operation(){
 let mut w=load();let cfg=serde_json::to_value(w.context.ground_2026.as_ref().unwrap()).unwrap();assert_eq!(cfg["drydocks"]["dock"]["pump"],"PUMP");
 let cmd=order(&w);assert!(cmd.validate_source(&w,Side::Blue).is_ok());harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);
 assert_eq!(view(&w)["flooded"],true);advance(&mut w,4);assert_eq!(view(&w)["flooded"],false);assert_eq!(view(&w)["usable"],true);
}
#[test]
fn dock02_real_pump_casualty_blocks_pumping_without_inventing_a_flood(){
 let mut w=load();let controller=id(&w,"DOCK");g::apply(&mut w,controller,&action()).unwrap();let pump=id(&w,"PUMP");
 g::apply_damage(&mut w,pump,1.,1,"scenario_assumption: actual pump critical").unwrap();advance(&mut w,5);assert_eq!(view(&w)["flooded"],true);
 assert!(order(&w).validate_source(&w,Side::Blue).is_err());
}
#[test]
fn dock03_gate_casualty_physically_floods_a_dry_dock_and_cannot_be_repaired_by_timer(){
 let mut w=load();let cmd=order(&w);harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);advance(&mut w,4);assert_eq!(view(&w)["flooded"],false);
 let gate=id(&w,"GATE");g::apply_damage(&mut w,gate,1.,1,"scenario_assumption: source gate critical").unwrap();advance(&mut w,5);
 assert_eq!(view(&w)["flooded"],true);assert_eq!(view(&w)["usable"],false);advance(&mut w,500);assert_eq!(view(&w)["usable"],false);
}
#[test]
fn dock04_a_pending_pump_cycle_roundtrips_without_resetting_its_elapsed_time(){
 let mut w=load();let cmd=order(&w);harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);
 let mut other:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();advance(&mut w,5);advance(&mut other,5);
 assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&other).unwrap());assert_eq!(view(&w)["flooded"],false);
}

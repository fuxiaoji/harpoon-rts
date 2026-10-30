//! 14.5 optional fueled aircraft source cases; source_critical is pre-adjudicated,
//! not a natural attack. Explicit individual aircraft stock and fuel status.
use harpoon_core::{Database,Scenario,World,EntityId};
use harpoon_core::rules::expanded_damage2026 as e;
use serde_json::{json,Value};
fn fixture(seed:u64,fuel:Value,ordnance:Value,optional:bool)->(World,EntityId) {
 let db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
 let sc:Scenario=serde_json::from_value(json!({"id":"fueled-aircraft","name":"scenario_assumption: individual hangar aircraft","seed":seed,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","air_wings":{"CV":[{"class":"us_f18e","count":1}]},"expanded_damage_2026":{"source":"scenario_assumption: explicit aircraft inventory","optional":{"armed_fueled_aircraft":optional},"units":{"CV":{"source":"scenario_assumption: one real inventory identity","parked":{"p1":{"source":"scenario_assumption: individual in hangar with declared fuel","class":"us_f18e","zone":"hangar","position_m":[0,0],"armed_ordnance":ordnance,"fueled":fuel,"stock":"ready"}}}}}},"units":[{"side":"Blue","class":"us_ford","name":"CV","pos":[0,0],"speed_kts":0,"fire_on":false}]})).unwrap();
 let w=harpoon_core::load_scenario(sc,db).unwrap();let id=w.entities.values().find(|x|x.name=="CV").unwrap().id;(w,id)
}
fn hit(w:&mut World,id:EntityId) {
 e::source_critical(w,id,"e05-fuel","hangar",e::CriticalContext{source:"scenario_assumption: adjudicated hangar critical".into(),dp:0.,penetrated:true,underwater:false,airburst:false,weapon:None}).unwrap();
}
fn fire_die(w:&World,id:EntityId)->Option<u64> {w.rules.expanded_dice.get(&id)?.iter().find(|d|d["purpose"]=="aircraft_fire")?["roll"].as_u64()}
fn fire_severity(w:&World,id:EntityId)->f64 {w.rules.casualties.get(&id).into_iter().flatten().filter(|c|!c.flood).map(|c|c.severity).sum()}
#[test]
fn fueled_unarmed_uses_original_die_plus_two_and_never_invents_ordnance_dp() {
 let (mut yes,a)=fixture(4,json!(true),json!([]),true);let (mut no,b)=fixture(4,json!(false),json!([]),true);
 let hp=yes.entity(a).unwrap().hp;hit(&mut yes,a);hit(&mut no,b);
 let roll=fire_die(&yes,a).unwrap();assert_eq!(fire_die(&no,b),Some(roll));
 assert_eq!(fire_severity(&yes,a),roll as f64+2.);assert_eq!(fire_severity(&no,b),roll.saturating_sub(2) as f64);
 assert_eq!(yes.entity(a).unwrap().hp,hp,"fuel alone must not invent explosive ordnance DP");
 let saved=serde_json::to_vec(&yes).unwrap();let restored:World=serde_json::from_slice(&saved).unwrap();assert_eq!(saved,serde_json::to_vec(&restored).unwrap());
}
#[test]
fn unknown_fuel_stays_pending_without_fabricated_fire_roll() {
 let (mut w,id)=fixture(4,Value::Null,json!([]),true);
 let result=e::source_critical(&mut w,id,"e05-fuel","hangar",e::CriticalContext{source:"scenario_assumption: adjudicated hangar critical".into(),dp:0.,penetrated:true,underwater:false,airburst:false,weapon:None});
 assert!(result.unwrap_err().contains("needs source adjudication"));
 assert!(fire_die(&w,id).is_none());let state=&w.rules.expanded_damage.as_ref().unwrap().units[&id];
 assert!(state.pending.contains_key("parked_aircraft_fire"));assert!(state.destroyed_aircraft.contains("p1"));
 assert_eq!(fire_severity(&w,id),0.);
}
#[test]
fn known_armed_does_not_need_fuel_and_optional_off_keeps_base_modifier() {
 let (mut armed,id)=fixture(4,Value::Null,json!([["us_mk82",1]]),true);hit(&mut armed,id);assert!(fire_die(&armed,id).is_some());
 let (mut off,id)=fixture(4,Value::Null,json!([]),false);hit(&mut off,id);let d=fire_die(&off,id).unwrap();assert_eq!(fire_severity(&off,id),d.saturating_sub(2) as f64);
}

//! Source-adjudicated bridge critical, public aircraft launch consumer.
use harpoon_core::{Database,Scenario,World,EntityId,Command,Side,Vec2};
use harpoon_core::commands::AirMission;
use harpoon_core::rules::expanded_damage2026 as e;
use serde_json::json;
fn fixture(seed:u64)->(World,EntityId) {
 let db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
 let sc:Scenario=serde_json::from_value(json!({"id":"bridge-flight","name":"scenario_assumption: carrier bridge source critical","seed":seed,"duration_s":5000,"weather":{"visibility_nm":10,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":30},"radar_environment":{"sea_state":0,"precipitation":"None"},"air_wings":{"CV":[{"class":"us_f18e","count":1}]},"aviation_2026":{"source":"scenario_assumption: test load","parking":{},"loads":{"us_f18e":{"payload_kg":1000,"tanks":[],"source":"scenario_assumption: test"}}}},"units":[{"side":"Blue","class":"us_ford","name":"CV","pos":[0,0],"speed_kts":0,"fire_on":false},{"side":"Red","class":"cn_093","name":"FAR","pos":[200,200],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
 let w=harpoon_core::load_scenario(sc,db).unwrap();let id=w.entities.values().find(|e|e.name=="CV").unwrap().id;(w,id)
}
#[test]
fn carrier_bridge_actual_critical_stops_public_launch_until_source_clock() {
 for seed in 1..=32 {let (mut w,id)=fixture(seed);e::source_critical(&mut w,id,"bridge-flight","bridge",e::CriticalContext{source:"scenario_assumption: already adjudicated bridge critical".into(),dp:0.,penetrated:true,underwater:false,airburst:false,weapon:None}).unwrap();
 let due=w.entity(id).unwrap().ext.get("damage.bridge_until").copied().unwrap_or(0.) as i64;if due<=w.tick {continue;}
 assert!(!e::bridge_flight_operational(&w,id));let before=w.entity(id).unwrap().deck.as_ref().unwrap().ready.clone();
 let cmd=Command::LaunchAircraft{unit:id,class:"us_f18e".into(),count:1,mission:AirMission::Cap{station:Vec2::new(0.,0.),radius_nm:5.,follow:true},profile:None};harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);
 assert_eq!(before,w.entity(id).unwrap().deck.as_ref().unwrap().ready);assert!(w.rules.operations.as_ref().unwrap().launches.is_empty());
 let saved=serde_json::to_vec(&w).unwrap();let restored:World=serde_json::from_slice(&saved).unwrap();assert_eq!(saved,serde_json::to_vec(&restored).unwrap());
 while w.tick<due {harpoon_core::sim::tick(&mut w,&[]);} assert!(e::bridge_flight_operational(&w,id));return;
 }panic!("fixture precondition: no positive original D6/2 clock");
}

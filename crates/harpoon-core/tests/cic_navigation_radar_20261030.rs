//! 14.5 CIC exception: only an actual navigation radar survives; the CIC
//! critical is an explicitly adjudicated input, not a natural attack.
use harpoon_core::{Database,Scenario,World,EntityId};
use harpoon_core::db::SensorKind;
use harpoon_core::rules::expanded_damage2026 as e;
use serde_json::json;
fn fixture(forge:bool)->Result<(World,EntityId),String> {
 let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
 let radar=db.sensors.values().find(|s|s.kind==SensorKind::Radar).unwrap().id.clone();
 let sonar=db.sensors.values().find(|s|s.kind==SensorKind::SonarPassive).unwrap().id.clone();
 db.platforms.get_mut("us_burke_2a").unwrap().sensors=vec![radar.clone(),sonar.clone()];
 let sc:Scenario=serde_json::from_value(json!({"id":"cic-nav-radar","name":"scenario_assumption: explicit laboratory sensors","seed":42,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","expanded_damage_2026":{"source":"scenario_assumption: sensor identity","units":{"SHIP":{"source":"scenario_assumption: navigation radar and sonar","components":{"nav":{"source":"scenario_assumption: navigation radar","kind":"sensor","binding":radar,"navigation":true},"sonar":{"source":"scenario_assumption: other sensor","kind":"sensor","binding":sonar,"navigation":forge}}}}}},"units":[{"side":"Blue","class":"us_burke_2a","name":"SHIP","pos":[0,0],"speed_kts":0,"fire_on":false}]})).unwrap();
 let w=harpoon_core::load_scenario(sc,db)?;let id=w.entities.values().find(|e|e.name=="SHIP").unwrap().id;Ok((w,id))
}
#[test]
fn cic_preserves_navigation_radar_and_disables_real_sonar() {
 let (mut w,id)=fixture(false).unwrap();let sensors=w.entity(id).unwrap().sensors.clone();let nav=sensors.iter().find(|s|w.db.sensors[&s.sensor].kind==SensorKind::Radar).unwrap().sensor.clone();let sonar=sensors.iter().find(|s|w.db.sensors[&s.sensor].kind==SensorKind::SonarPassive).unwrap().sensor.clone();
 e::source_critical(&mut w,id,"cic-radar","cic",e::CriticalContext{source:"scenario_assumption: adjudicated CIC critical".into(),dp:0.,penetrated:true,underwater:false,airburst:false,weapon:None}).unwrap();
 assert!(e::sensor_operational(&w,id,&nav).unwrap());assert!(!e::sensor_operational(&w,id,&sonar).unwrap());
 let bytes=serde_json::to_vec(&w).unwrap();let restored:World=serde_json::from_slice(&bytes).unwrap();assert_eq!(bytes,serde_json::to_vec(&restored).unwrap());
}
#[test]
fn sonar_cannot_claim_navigation_radar_exception() {assert!(fixture(true).err().unwrap().contains("navigation exception"));}

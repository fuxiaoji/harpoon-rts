//! Actual visual consumer under an authored squall, not a table-only test.
use harpoon_core::{Database, Scenario, World};
use harpoon_core::rules::detection2026 as d;
use serde_json::json;
fn world(squall: bool)->World {
 let mut db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
 let mut sensor=db.sensors.values().next().unwrap().clone(); sensor.id="squall_lab_visual".into(); sensor.name="authored lookout".into();
 sensor.kind=harpoon_core::db::SensorKind::Visual; db.sensors.insert(sensor.id.clone(),sensor);
 db.platforms.get_mut("us_burke_2a").unwrap().sensors=vec!["squall_lab_visual".into()];
 let mut detection=json!({"bindings":{"us_burke_2a:squall_lab_visual":{"type":"visual","source":"scenario_assumption: lookout"}},
 "environment":{"source":"scenario_assumption: open sea","sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
 "targets":{"us_burke_2a":{"source":"scenario_assumption: test size","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}});
 if squall {detection["squalls"]=json!({"source":"scenario_assumption: initial positions and wind","centers":[{"x":0.0,"y":0.0}],"wind_kts":{"x":0.0,"y":0.0}});}
 let sc:Scenario=serde_json::from_value(json!({"id":"squall-lab","name":"authored experiment","duration_s":1000,"seed":8,"weather":{"visibility_nm":20,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","detection_2026":detection},"units":[
 {"side":"Blue","class":"us_burke_2a","name":"O","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false},
 {"side":"Red","class":"us_burke_2a","name":"T","pos":[0,1],"course_deg":0,"speed_kts":0,"fire_on":false}]})).unwrap();
 harpoon_core::load_scenario(sc,db).unwrap()
}
#[test]
fn squall_changes_actual_visual_and_restores() {
 let clear=world(false); let wet=world(true);
 let ids=|w:&World| {let o=w.entities.values().find(|e|e.name=="O").unwrap().id; let t=w.entities.values().find(|e|e.name=="T").unwrap().id;(o,t)};
 let scan=|w:&World| {let (o,t)=ids(w); let mut s=w.rules.detection.clone().unwrap(); d::evaluate_world(w,&mut s,o,"squall_lab_visual",t).unwrap()};
 let a=scan(&clear); let b=scan(&wet);
 assert!(b.trace["base_kyd"].as_f64().unwrap()<a.trace["base_kyd"].as_f64().unwrap());
 let restored:World=serde_json::from_value(serde_json::to_value(&wet).unwrap()).unwrap();
 assert_eq!(serde_json::to_value(scan(&wet)).unwrap(),serde_json::to_value(scan(&restored)).unwrap());
}

//! Actual source sightings feed the shared queued transport and manual policy.
use super::*;

fn fixture() -> Session {
    let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
    db.platforms.get_mut("us_ford").unwrap().sensors=vec!["visual".into()];
    db.platforms.get_mut("us_burke_2a").unwrap().sensors.clear();
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    let sc:Scenario=serde_json::from_value(json!({
        "id":"mcp-sar-source","name":"scenario_assumption: adjudicated loss then native search",
        "seed":20261025,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "flight_environment":{"wind_from_deg":270,"wind_kts":3},
            "expanded_damage_2026":{"source":"scenario_assumption: optional SAR and explicit complements","optional":{"search_and_rescue":true},"units":{
                "LOOK":{"source":"scenario_assumption: own rescue station","crew":10,"rescue":{"source":"scenario_assumption: station/capacity","kind":"ship","capacity":10,"station_radius_m":500}},
                "LOST":{"source":"scenario_assumption: ten-person laboratory complement","crew":10}}},
            "detection_2026":{"environment":{"source":"scenario_assumption: clear daytime open water","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
        "units":[{"side":"Blue","class":"us_ford","name":"LOOK","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"LOST","pos":[0.01,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DUMMY","pos":[2000,0],"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut w=load_scenario(sc,db).unwrap();
    let lost=w.entities.values().find(|e|e.name=="LOST").unwrap().id;
    harpoon_core::damage::sink_by(&mut w,lost,Side::Blue);
    harpoon_core::sim::tick(&mut w,&[]);
    assert_eq!(observe(&w,Side::Blue,0,0).units[0].survivors.len(),1);
    Session{world:Some(w),..Session::new()}
}
fn send(s:&mut Session,unit:u32,action:Value)->Result<Value,String>{
    s.call("send_command",&json!({"side":"Blue","command":{"cmd":"search_and_rescue","unit":unit,"action":action}}))
}
#[test]
fn sar_source_rejections_do_not_queue_or_acquire_manual_control() {
    let mut s=fixture();
    let before=serde_json::to_vec(s.world.as_ref().unwrap()).unwrap();
    for (unit,action) in [(1,json!({"type":"begin","group":"referee-lost-unit-2","automatic":true})),
        (3,json!({"type":"cancel"})),(1,json!({"type":"manual"})),
        (1,json!({"type":"cancel","picked_up":100}))] {
        assert!(send(&mut s,unit,action).is_err());
        assert!(s.inbox.is_empty());
        assert_eq!(serde_json::to_vec(s.world.as_ref().unwrap()).unwrap(),before);
    }
    assert!(s.call("send_command",&json!({"side":"Blue","command":{"cmd":"search_and_rescue","unit":1,"action":{"type":"cancel"},"retain_manual":true}})).is_err());
    assert!(s.inbox.is_empty());
}
#[test]
fn sar_real_sighting_queues_then_applies_and_remains_manual_until_explicit_resume() {
    let mut s=fixture();
    let group=observe(s.world.as_ref().unwrap(),Side::Blue,0,0).units[0].survivors[0]["label"].clone();
    let before=serde_json::to_vec(s.world.as_ref().unwrap()).unwrap();
    assert!(send(&mut s,1,json!({"type":"begin","group":group,"automatic":false})).unwrap()["queued"]==true);
    assert_eq!(serde_json::to_vec(s.world.as_ref().unwrap()).unwrap(),before);
    s.call("step",&json!({"ticks":1})).unwrap();
    let w=s.world.as_ref().unwrap();
    assert!(w.rules.expanded_damage.as_ref().unwrap().rescues.contains_key(&harpoon_core::EntityId(1)));
    assert!(observe(w,Side::Blue,0,0).units[0].manual_control);
    s.call("step",&json!({"ticks":30})).unwrap();
    assert!(observe(s.world.as_ref().unwrap(),Side::Blue,0,0).units[0].manual_control);
}

//! New source candidates: actual witnessed loss and scenario-declared SAR VP.
//! The2VP/person reward is an author parameter;14.8 supplies no universal score.
use harpoon_core::rules::expanded_damage2026 as ex;
use harpoon_core::{Database,EntityId,Scenario,World,Side,Command};
use serde_json::{json,Value};
fn id(w:&World,name:&str)->EntityId {w.entities.values().find(|e|e.name==name).unwrap().id}
fn fixture(distance:f64,rewards:bool)->World {
    let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
    db.platforms.get_mut("us_ford").unwrap().sensors=vec!["visual".into()];
    db.platforms.get_mut("us_burke_2a").unwrap().sensors.clear();db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    let mut expanded=json!({"source":"scenario_assumption: explicit SAR option, crew and station only","optional":{"search_and_rescue":true},"units":{
        "LOOK":{"source":"scenario_assumption: four crew/four survivor capacity","crew":4,"rescue":{"source":"scenario_assumption: declared500m station","kind":"ship","capacity":4,"station_radius_m":500}},
        "LOST":{"source":"scenario_assumption: ten-person laboratory complement","crew":10}}});
    if rewards {expanded["rescue_victory"]=json!({"source":"scenario_assumption: two victory points per actually rescued person, no book default score","points_per_survivor":2.0});}
    let sc:Scenario=serde_json::from_value(json!({"id":"sar-wide-20261026","name":"scenario_assumption: actual lookout and declared rescue scoring","seed":20261026,"duration_s":2000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":270,"wind_kts":3},"radar_environment":{"sea_state":0,"precipitation":"None"},"expanded_damage_2026":expanded,
            "detection_2026":{"environment":{"source":"scenario_assumption: clear daytime open water","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
        "units":[{"side":"Blue","class":"us_ford","name":"LOOK","pos":[0,0],"speed_kts":0,"course_deg":90,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"LOST","pos":[distance,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DUMMY","pos":[2000,0],"depth_m":50,"speed_kts":0,"fire_on":false}]})).unwrap();
    harpoon_core::load_scenario(sc,db).unwrap()
}
fn action(w:&mut World,v:Value) {let cmd:Command=serde_json::from_value(json!({"cmd":"search_and_rescue","unit":id(w,"LOOK"),"action":v})).unwrap();harpoon_core::sim::tick(w,&[(Side::Blue,cmd)]);}
#[test]
fn s01_actual_visual_witness_of_loss_places_a_marker_outside_survivor_search_range() {
    let mut w=fixture(8.,false);let lost=id(&w,"LOST");let look=id(&w,"LOOK");
    harpoon_core::sim::tick(&mut w,&[]);
    assert!(w.sides[0].contacts.values().any(|c|c.about==lost&&c.sensors.iter().any(|s|s=="visual")),"a true native visual sighting of the original large ship is a prerequisite");
    harpoon_core::damage::sink_by(&mut w,lost,Side::Blue);
    let own=ex::survivor_view(&w,look);
    assert_eq!(own.len(),1,"14.8 witnessed loss places the marker; it does not require a second G-size survivor search");
    assert_eq!(own[0]["observed_at"],w.tick);assert_eq!(own[0]["report_is_live_track"],false);
    let before=own.clone();
    let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..181 {harpoon_core::sim::tick(&mut w,&[]);harpoon_core::sim::tick(&mut restored,&[]);}
    assert_eq!(ex::survivor_view(&w,look),before,"a marker is a fixed own measurement; ordinary survivor scans at8nm cannot refresh it");
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
}
#[test]
fn s02_only_actual_recovery_earns_declared_rescue_victory_points_once() {
    let mut w=fixture(0.2,true);let lost=id(&w,"LOST");let look=id(&w,"LOOK");
    harpoon_core::damage::sink_by(&mut w,lost,Side::Blue);harpoon_core::sim::tick(&mut w,&[]);
    let reports=ex::survivor_view(&w,look);assert_eq!(reports.len(),1);
    action(&mut w,json!({"type":"begin","group":reports[0]["label"],"automatic":false}));
    let began=w.tick;
    while w.tick<began+180 {harpoon_core::sim::tick(&mut w,&[]);}
    let before=w.victory_points();
    action(&mut w,json!({"type":"recover_due"}));
    let picked=ex::own_view(&w,look).unwrap()["survivors_aboard"].as_u64().unwrap();assert!(picked>0&&picked<=4);
    let after=w.victory_points();assert_eq!(after.0-before.0,picked as f64*2.,"source14.8 actual pickup is consumed by victory, at explicit author reward");
    action(&mut w,json!({"type":"recover_due"}));assert_eq!(w.victory_points(),after);
    let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..181 {harpoon_core::sim::tick(&mut w,&[]);harpoon_core::sim::tick(&mut restored,&[]);}
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
    assert_eq!(w.victory_points(),after);
}

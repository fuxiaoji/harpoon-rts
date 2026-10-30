//! Exact Annex A Flight IIA and Zumwalt terminal qualification; topology is authored.
use harpoon_core::rules::communications2026 as c;
use harpoon_core::{Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

const P: &str = "tdl_annex_lab_passive";

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

fn fit(system: &str, kind: &str, transport: Value, peers: Vec<&str>) -> Value {
    json!({"source":"scenario_assumption: declared operational topology only; real hardware is separately source-qualified",
        "annex_system":system,"protocol":system,"send":true,"receive":true,
        "operational":true,"peers":peers,"warning_kind":kind,"automatic_warning":false,
        "targeting_system":false,"transport":transport})
}

fn los() -> Value {
    json!({"type":"line_of_sight","max_range_nm":900.,"antenna_height_m":1000.})
}

fn ford(side: &str, name: &str, y: f64) -> Value {
    json!({"side":side,"class":"us_ford","name":name,"pos":[0,y],"course_deg":90,"speed_kts":0,"fire_on":false})
}

/// Standard lab: three Blue Ford units 0/20/40 nm plus the inert Red lab
/// contact; declared Link 16 LOS chain SENDER -> RELAY -> RECEIVER.
fn units() -> Value {
    json!([
        ford("Blue","SENDER",0.),
        ford("Blue","RELAY",20.),
        ford("Blue","RECEIVER",40.),
        {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}])
}

fn comm_units() -> Value {
    json!({
        "SENDER":{"source":"scenario_assumption: declared position","fits":{"lab":fit("us_link_16","real_time",los(),vec!["RELAY"])}},
        "RELAY":{"source":"scenario_assumption: declared position","fits":{"lab":fit("us_link_16","real_time",los(),vec!["RECEIVER"])}},
        "RECEIVER":{"source":"scenario_assumption: declared position","fits":{"lab":fit("us_link_16","real_time",los(),vec![])}}})
}

fn scenario(units_json: Value, comm: Value) -> Scenario {
    serde_json::from_value(json!({"id":"tdl-relay-lab","name":"scenario_assumption: TDL relay lab",
        "duration_s":10000,"seed":1,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":2,"precipitation":"None"},
            "communications_2026":{"source":"scenario_assumption: declared routes","units":comm},
            "detection_2026":{"bindings":{format!("us_ford:{P}"):{"type":"sonar","source":"scenario_assumption: laboratory passive sensor, not a real Ford fit",
                "generation":5,"passive_range_nm":4.,"active_range_nm":null,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false}},
                "environment":{"source":"scenario_assumption: controlled open water","sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
                "targets":{"cn_093":{"source":"scenario_assumption: controlled acoustic signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
        "units":units_json})).unwrap()
}

fn database() -> Database {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut sensor = db.sensors.values().next().unwrap().clone();
    sensor.id = P.into();
    sensor.name = "scenario_assumption: laboratory passive source".into();
    sensor.kind = harpoon_core::db::SensorKind::SonarPassive;
    sensor.air_range_nm = 0.;
    sensor.surface_range_nm = 0.;
    sensor.missile_range_nm = 0.;
    sensor.sub_range_nm = 4.;
    db.sensors.insert(P.into(), sensor);
    db.platforms.get_mut("us_ford").unwrap().sensors = vec![P.into()];
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    db.validate().unwrap();
    db
}

fn load(units_json: Value, comm: Value) -> World {
    harpoon_core::load_scenario(scenario(units_json, comm), database())
        .expect("the declared relay lab must load")
}


#[test]
fn real_class_fits_qualify_and_restore_without_granting_other_systems() {
    for class in ["us_burke_2a", "us_zumwalt"] {
        let mut us = units();
        for n in 0..3 { us[n]["class"] = json!(class); }
        let w = load(us.clone(), comm_units());
        let sender = id(&w, "SENDER");
        let receiver = id(&w, "RECEIVER");
        assert!(c::validate_transport(&w, Side::Blue, sender, "lab", &[receiver]).is_ok());
        let restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
        assert!(c::validate_transport(&restored, Side::Blue, sender, "lab", &[receiver]).is_ok());
        let mut wrong = comm_units();
        for name in ["SENDER", "RELAY", "RECEIVER"] {
            wrong[name]["fits"]["lab"]["annex_system"] = json!("us_madl");
            wrong[name]["fits"]["lab"]["protocol"] = json!("us_madl");
        }
        assert!(harpoon_core::load_scenario(scenario(us, wrong), database()).is_err());
    }
}

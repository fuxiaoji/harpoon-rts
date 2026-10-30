//! New independent candidate for persistent old survivor reports.
//! Reuses the source-declared laboratory setup, not any private survivor writes.
//! Not registered or run by this author; original four-candidate file unchanged.

use harpoon_core::rules::expanded_damage2026 as ex;
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::json;

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

fn fixture(rescue_class: &str) -> Result<World, String> {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .map_err(|e| e.to_string())?;
    // Restrict only experiment receivers; original hull/DP/weapons stay intact.
    db.platforms.get_mut("us_ford").unwrap().sensors = vec!["visual".into()];
    for class in ["us_burke_2a", "cn_093", "airfield"] {
        db.platforms.get_mut(class).unwrap().sensors.clear();
    }
    let sc: Scenario = serde_json::from_value(json!({
        "id":"independent-sar-recovery-guards-20261026",
        "name":"scenario_assumption: actual platform kind and loss guard laboratory",
        "seed":20261026,"duration_s":1000,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{
            "rules_profile":"hv2026-rts-v4",
            "flight_environment":{"wind_from_deg":270,"wind_kts":3},
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "expanded_damage_2026":{
                "source":"scenario_assumption: optional SAR, laboratory complements and station radius, not actual Annex fits",
                "optional":{"search_and_rescue":true},
                "units":{
                    "LOOK":{"source":"scenario_assumption: lookout only"},
                    "RESCUE":{
                        "source":"scenario_assumption: experimental rescue complement, no actual equipment claim",
                        "crew":100,
                        "rescue":{"source":"scenario_assumption: declared500m rescue station and100-person capacity","kind":"ship","capacity":100,"station_radius_m":500}
                    },
                    "LOST":{"source":"scenario_assumption: ten-person complement for unambiguous1-9 survivor sighting","crew":10}
                }
            },
            "detection_2026":{"environment":{
                "source":"scenario_assumption: clear daytime open water without terrain/cloud/ice",
                "sea_state":0,"precipitation":0,"visibility_percent":100,
                "shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null
            }}
        },
        "units":[
            {"side":"Blue","class":"us_ford","name":"LOOK","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":rescue_class,"name":"RESCUE","pos":[0,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"LOST","pos":[0.01,0],"course_deg":0,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DUMMY","pos":[2000,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}
        ]
    }))
    .map_err(|e| e.to_string())?;
    let mut w = harpoon_core::load_scenario(sc, db)?;
    let lost = id(&w, "LOST");
    harpoon_core::damage::sink_by(&mut w, lost, Side::Blue);
    assert!(ex::survivor_view(&w, id(&w, "LOOK")).is_empty());
    // T1 normal sensor phase performs the successful close-range native scan.
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(w.tick, 1);
    let reports = ex::survivor_view(&w, id(&w, "LOOK"));
    assert_eq!(reports.len(), 1, "real surface lookout must discover the loss group");
    assert_eq!(reports[0]["estimated_group_size"], "less_than_ten");
    let label = reports[0]["label"].as_str().unwrap();
    let command: Command = serde_json::from_value(json!({
        "cmd":"search_and_rescue","unit":id(&w,"RESCUE"),
        "action":{"type":"begin","group":label,"automatic":true}
    }))
    .map_err(|e| e.to_string())?;
    // This is the normal public shared command path, at T2.
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, command)]);
    assert_eq!(w.tick, 2);
    Ok(w)
}

fn advance_to(w: &mut World, to: i64) {
    while w.tick < to {
        let old = w.tick;
        harpoon_core::sim::tick(w, &[]);
        assert_eq!(w.tick, old + 1, "fixture must not end before its guard boundary");
    }
}


#[test]
fn r05_last_real_sighting_survives_native_observer_death_and_entity_cleanup() {
    let mut w = fixture("us_burke_2a").unwrap();
    let lookout = id(&w, "LOOK");
    let rescuer = id(&w, "RESCUE");
    assert!(w.entity(rescuer).unwrap().alive);
    assert!(!w.entity(rescuer).unwrap().sensors.iter().any(|s| {
        w.db.sensors.get(&s.sensor).is_some_and(|d| d.kind == harpoon_core::db::SensorKind::Visual)
    }), "the recipient must not refresh the old report using its own lookout");
    let before = ex::survivor_view(&w, rescuer);
    assert_eq!(before.len(), 1, "native LOOK report must already be shared to the living recipient");
    assert_eq!(before[0]["label"], "survivors-1");
    assert_eq!(before[0]["report_is_live_track"], false);
    assert_eq!(before[0]["observed_at"], 2);

    // Actual loss at T2, then the normal120-second Full wreck lifetime.
    // Neither the group nor its historical measured position is edited.
    harpoon_core::damage::sink_by(&mut w, lookout, Side::Red);
    assert!(!w.entity(lookout).unwrap().alive);
    advance_to(&mut w, 121);
    assert!(w.entity(lookout).is_some());
    assert_eq!(ex::survivor_view(&w, rescuer), before,
        "death itself must not invent an update to the last measured snapshot");
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    harpoon_core::sim::tick(&mut restored, &[]);
    assert_eq!(w.tick, 122);
    assert!(w.entity(lookout).is_none(), "normal Full cleanup must really remove the observer");
    assert!(w.entity(rescuer).unwrap().alive);
    assert_eq!(ex::survivor_view(&w, rescuer), before,
        "a living side keeps its last real measured survivor report after the observing entity is removed");
    assert_eq!(ex::survivor_view(&restored, rescuer), before);
    assert_eq!(serde_json::to_vec(&w).unwrap(), serde_json::to_vec(&restored).unwrap());
}

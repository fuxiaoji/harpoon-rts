//! English Jan2026 PDF137/14-11: two survivors per Tactical Turn while
//! hovering. Loss setup uses the adjudicated native sink_by boundary; the
//! witness, marker, orders, motion, fuel and recovery are normal production.
//! Complement, capacity and station radius are explicit laboratory inputs.
use harpoon_core::rules::expanded_damage2026 as ex;
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn step(w: &mut World) {
    harpoon_core::sim::tick(w, &[]);
}
fn to(w: &mut World, at: i64) {
    while w.tick < at {
        assert!(!w.over);
        step(w);
    }
}
fn fixture(remote: bool, flight: bool) -> World {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    db.platforms.get_mut("us_ford").unwrap().sensors = vec!["visual".into()];
    for class in ["us_mh60r", "us_burke_2a", "cn_093"] {
        db.platforms.get_mut(class).unwrap().sensors.clear();
    }
    let mut value = json!({
        "id":"sar-air-work-source", "name":"scenario_assumption: helicopter hoist work laboratory",
        "seed":20261107,"duration_s":5000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "flight_environment":{"wind_from_deg":270,"wind_kts":0},
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "aviation_2026":{"source":"scenario_assumption: empty MH60R laboratory aircraft",
                "loads":{"us_mh60r":{"payload_kg":0,"tanks":[],"source":"scenario_assumption: empty laboratory load"}},
                "parking":{},"initial_fuel_nm":{"AIR":200}},
            "expanded_damage_2026":{"source":"scenario_assumption: experimental complement and hoist reach",
                "optional":{"search_and_rescue":true},"units":{
                    "LOOK":{"source":"scenario_assumption: real visual witness"},
                    "AIR":{"source":"scenario_assumption: experimental hoist fit, no real MH60R capacity claim",
                        "rescue":{"source":"scenario_assumption: capacity4 and500m experiment geometry","kind":"helicopter","capacity":4,"station_radius_m":500}},
                    "LOST":{"source":"scenario_assumption:90 person laboratory complement","crew":90}}},
            "detection_2026":{"environment":{"source":"scenario_assumption: clear daylight open water",
                "sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
                "water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
        "units":[
            {"side":"Blue","class":"us_ford","name":"LOOK","pos":[0.5,0],"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":"us_mh60r","name":"AIR","pos":[if remote {0.} else {0.5},0],
                "course_deg":90,"speed_kts":0,"altitude_m":100,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"LOST","pos":[0.51,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DUMMY","pos":[2000,0],"speed_kts":0,"depth_m":50,"fire_on":false}]
    });
    if !flight {
        value["context"]
            .as_object_mut()
            .unwrap()
            .remove("aviation_2026");
    }
    let sc: Scenario = serde_json::from_value(value).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let lost = id(&w, "LOST");
    harpoon_core::damage::sink_by(&mut w, lost, Side::Blue);
    step(&mut w);
    assert_eq!(
        ex::survivor_view(&w, id(&w, "AIR")).len(),
        1,
        "actual same-side native visual witness"
    );
    assert_eq!(
        w.rules
            .aviation
            .as_ref()
            .unwrap()
            .flights
            .contains_key(&id(&w, "AIR")),
        flight
    );
    w
}
fn issue(w: &mut World, action: Value) {
    let command: Command = serde_json::from_value(
        json!({"cmd":"search_and_rescue","unit":id(w,"AIR"),"action":action}),
    )
    .unwrap();
    command.validate_source(w, Side::Blue).unwrap();
    harpoon_core::sim::tick(w, &[(Side::Blue, command)]);
}
fn begin(w: &mut World) {
    let label = ex::survivor_view(w, id(w, "AIR"))[0]["label"].clone();
    issue(w, json!({"type":"begin","group":label,"automatic":false}));
}
fn hover(w: &mut World) {
    let command: Command = serde_json::from_value(
        json!({"cmd":"aviation","unit":id(w,"AIR"),"action":{"type":"throttle","setting":"Hover"}}),
    )
    .unwrap();
    command.validate_source(w, Side::Blue).unwrap();
    harpoon_core::sim::tick(w, &[(Side::Blue, command)]);
}
fn aboard(w: &World) -> u32 {
    w.rules
        .expanded_damage
        .as_ref()
        .unwrap()
        .units
        .get(&id(w, "AIR"))
        .map_or(0, |u| u.survivors_aboard)
}
#[test]
fn h01_stationary_aircraft_without_actual_hover_cannot_hoist() {
    let mut w = fixture(false, false);
    begin(&mut w);
    to(&mut w, 200);
    let air = id(&w, "AIR");
    assert_eq!(w.entity(air).unwrap().speed_kts, 0.);
    assert!(
        w.entity(air)
            .unwrap()
            .pos
            .range_to(harpoon_core::Vec2::new(0.51, 0.))
            * 1852.
            < 500.
    );
    issue(&mut w, json!({"type":"recover_due"}));
    assert_eq!(
        aboard(&w),
        0,
        "zero speed alone does not establish a hovering helicopter"
    );
    // A new, fully fitted control uses the existing admitted Hover command.
    // The missing-flight negative cannot acquire a fabricated TS flight fit.
    let mut w = fixture(false, true);
    hover(&mut w);
    begin(&mut w);
    let start = w.tick;
    issue(&mut w, json!({"type":"recover_due"}));
    assert_eq!(aboard(&w), 0, "old order age cannot bank non-hover time");
    to(&mut w, start + 180);
    issue(&mut w, json!({"type":"recover_due"}));
    assert_eq!(
        aboard(&w),
        2,
        "one real hovering Tactical Turn hoists exactly two"
    );
    issue(&mut w, json!({"type":"recover_due"}));
    assert_eq!(aboard(&w), 2);
}
#[test]
fn h02_remote_order_time_cannot_replace_hover_work_at_the_survivor_station() {
    let mut w = fixture(true, true);
    hover(&mut w);
    begin(&mut w);
    to(&mut w, 200);
    let air = id(&w, "AIR");
    let cruise: Command = serde_json::from_value(
        json!({"cmd":"aviation","unit":air,"action":{"type":"throttle","setting":"Cruise"}}),
    )
    .unwrap();
    harpoon_core::sim::tick(
        &mut w,
        &[
            (Side::Blue, cruise),
            (
                Side::Blue,
                Command::SetSpeed {
                    unit: air,
                    speed_kts: 10.,
                },
            ),
        ],
    );
    while w.entity(air).unwrap().pos.x < 0.3 {
        assert!(w.tick < 2000);
        step(&mut w);
    }
    // Hover caps actual source flight speed at zero. Generic Stop while the
    // throttle is Cruise cannot override that source minimum flight speed.
    hover(&mut w);
    while w.entity(air).unwrap().speed_kts > 1e-9 {
        assert!(w.tick < 2100);
        step(&mut w);
    }
    hover(&mut w);
    let arrival = w.tick;
    assert!(
        w.entity(air)
            .unwrap()
            .pos
            .range_to(harpoon_core::Vec2::new(0.51, 0.))
            * 1852.
            < 500.
    );
    issue(&mut w, json!({"type":"recover_due"}));
    assert_eq!(
        aboard(&w),
        0,
        "180s spent hovering at a remote order point earns no hoist"
    );
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    to(&mut w, arrival + 181);
    to(&mut restored, arrival + 181);
    issue(&mut w, json!({"type":"recover_due"}));
    issue(&mut restored, json!({"type":"recover_due"}));
    assert_eq!(aboard(&w), 2);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

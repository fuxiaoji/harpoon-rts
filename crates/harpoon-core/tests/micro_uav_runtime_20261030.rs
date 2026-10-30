//! Author-declared experimental micro control on a source flight class; not a real equipment claim.
use harpoon_core::{Database, Scenario};
use harpoon_core::rules::{aviation2026 as a, unmanned2026 as u};
use serde_json::json;

#[test]
fn live_hitl_exempts_actual_low_flight_then_loss_restores_roll() {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"micro-runtime", "name":"scenario_assumption: micro HITL experiment", "duration_s":4000,
        "seed":20261040, "day_night":"Day", "weather":{"visibility_nm":20,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "aviation_2026":{"source":"scenario_assumption: experiment", "visibility_percent":100,
                "loads":{"us_f18e":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"}},
                "parking":{}, "initial_fuel_nm":{"MICRO":700},
                "micro_uav":{"source":"scenario_assumption: experimental micro qualification", "micro_classes":{
                    "us_f18e":{"source":"scenario_assumption: experimental body, not actual F18 qualification"}}}},
            "unmanned_2026":{"source":"scenario_assumption: control experiment",
                "controllers":{"CTRL":{"source":"scenario_assumption: control", "elements":["ctrl"],"operational":true,"satellite":true,"satellite_available":true}},
                "vehicles":{"MICRO":{"source":"scenario_assumption: experimental control", "domain":"air", "autonomous_capable":false,
                    "initial_mode":"hitl", "controller":"CTRL", "element":"ctrl", "initial_link":"satellite", "satellite":true,
                    "satellite_available":true, "loss_behavior":{"type":"return_to_home","home":"CTRL"}}}}},
        "units":[{"side":"Blue","class":"us_burke_2a","name":"CTRL","pos":[0,0],"course_deg":90,"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":"us_f18e","name":"MICRO","pos":[0.2,0],"course_deg":90,"speed_kts":300,"altitude_m":10,"fire_on":false}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let micro = w.entities.values().find(|e| e.name == "MICRO").unwrap().id;
    let ctrl = w.entities.values().find(|e| e.name == "CTRL").unwrap().id;
    a::safety_step(&mut w);
    let first = serde_json::to_string(&w).unwrap();
    assert!(first.contains("4.5.3.micro_uav_exempt"));
    assert!(!first.contains("4.5.2.low_crash"));
    u::set_satellite_available(&mut w, &[micro, ctrl], false, "scenario_assumption: bandwidth withdrawn").unwrap();
    a::safety_step(&mut w);
    let after = serde_json::to_string(&w).unwrap();
    assert!(after.contains("4.5.2.low_crash"));
    assert!(after.contains("roll_uniform") && after.contains("chance"));
}

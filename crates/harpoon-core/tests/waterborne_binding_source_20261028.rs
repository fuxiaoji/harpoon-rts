//! Artifact-only exact-body admission boundary for the explicit waterborne card.
//! This is an RTS configuration contract, not a Book ban on duplicate callsigns.
//! Only normal Scenario JSON and load_scenario are used; no runtime injection.
use harpoon_core::{Database, Scenario, World};
use serde_json::{json, Value};

const CLASS: &str = "lab_seaplane_max240_fmp200";

fn card() -> Value {
    json!({
        "source":"scenario_assumption: complete authored Small turboprop seaplane card; no real Annex identity or Ka27 water capability claim",
        "seaplane":true,
        "maximum_speed_kts":240.0,
        "fmp_low_kts":200.0,
        "low_altitude_m":300.0,
        "maneuver_light":1.0,"maneuver_heavy":1.0,"ceiling_m":2000.0,
        "flight_spec":{"source":"scenario_assumption: authored laboratory engine/fuel/mass and Low-only flight row, not real Annex","engine":"TP","size":1,"cruise_kts":120.0,"cruise_range_nm":800.0,"fuel_kg":1200.0,"max_payload_kg":1000.0,"receive":null,"tanker_slots":0,"centerline":false},
        "load":{"source":"scenario_assumption: unarmed author laboratory with zero carried stores","payload_kg":0.0,"tanks":[],"buddy":false,"external_weapon_rounds":{},"internal_weapon_rounds":{}},
        "platform":{"id":CLASS,"name":"Author Small turboprop floatplane max240 FMP200","kind":"Aircraft","hp":10.0,"max_speed_kts":240.0,"cruise_speed_kts":120.0,"turn_rate_deg_s":20.0,"accel_kts_per_s":10.0,"rcs":"Small","noise_db":50.0,"visual_factor":1.0,"vp_value":1.0,"sensors":[],"weapons":[],"chaff":0,"decoys":0,"endurance_s":24000.0,"max_depth_m":0.0,"dipping_sonar":false,"is_base":false,"deck":null}
    })
}

fn binding() -> Value {
    json!({"source":"scenario_assumption: exact initial same-body water binding with explicit remaining fuel","phase":"waterborne","location":"open","initial_fuel_nm":800.0,"mooring":null})
}

fn scenario_input() -> Value {
    json!({
        "id":"waterborne-exact-body-admission",
        "name":"author exact-name waterborne binding admission, no new physical rule",
        "seed":20261028,"duration_s":3600,
        "weather":{"visibility_nm":0.0,"jam":[0,0]},
        "context":{
            "rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":3,"precipitation":"None"},
            "aviation_2026":{"loads":{},"parking":{},"source":"scenario_assumption: ordinary aviation state plus complete separately declared waterborne cards","visibility_percent":100.0},
            "waterborne_2026":{
                "source":"scenario_assumption: exact body/Flight/fuel identity bindings, not a global callsign restriction",
                "cards":{(CLASS):card()},
                "units":{"DUP":binding(),"SPARE":binding()},
                "environment":{"source":"scenario_assumption: safe open water and wind from east12kt","sea_state":3,"wind_from_deg":90.0,"wind_speed_kts":12.0,"locations":{"open":{"source":"scenario_assumption: explicitly open water with no shelter","shelter_states":0}}},
                "rts_mapping":{"source":"scenario_assumption: source Movement-end completion maps to normal RTS barrier without inferred elapsed operation seconds","completion":"normal_movement_end","resting_motion":"hold_author_position","airborne_scope":"Low level flight only"}
            }
        },
        "units":[
            {"side":"Blue","class":CLASS,"name":"DUP","pos":[0.0,0.0],"course_deg":90.0,"speed_kts":0.0,"altitude_m":0.0,"fire_on":false},
            {"side":"Blue","class":CLASS,"name":"SPARE","pos":[0.0,10.0],"course_deg":90.0,"speed_kts":0.0,"altitude_m":0.0,"fire_on":false},
            {"side":"Blue","class":"us_virginia","name":"CONTROL","pos":[40.0,0.0],"speed_kts":0.0,"depth_m":50.0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"OTHER","pos":[80.0,0.0],"speed_kts":0.0,"depth_m":50.0,"fire_on":false}
        ]
    })
}

fn load(input: Value) -> Result<World, String> {
    let scenario: Scenario = serde_json::from_value(input.clone())
        .expect("current normal Scenario schema accepts the complete source-qualified author card");
    assert_eq!(
        serde_json::to_value(&scenario).unwrap()["context"]["waterborne_2026"],
        input["context"]["waterborne_2026"],
        "retain the exact explicit binding and card; no future schema or serde-shape failure"
    );
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    db.platforms.insert(
        CLASS.into(),
        serde_json::from_value(card()["platform"].clone()).unwrap(),
    );
    for class in ["us_virginia", "cn_093"] {
        db.platforms.get_mut(class).unwrap().sensors.clear();
    }
    harpoon_core::load_scenario(scenario, db)
}

#[test]
fn wb01_exact_waterborne_name_binding_rejects_two_live_aircraft_instead_of_first_body_selection() {
    let input = scenario_input();
    let control = load(input.clone())
        .expect("control: two uniquely named, independently declared real bodies must initialize");
    let aviation = control.rules.aviation.as_ref().unwrap();
    assert_eq!(aviation.flights.len(), 2);
    let water = serde_json::to_value(aviation).unwrap()["waterborne"].clone();
    assert_eq!(water["units"].as_object().unwrap().len(), 2);
    for name in ["DUP", "SPARE"] {
        let body = control.entities.values().find(|e| e.name == name).unwrap();
        assert_eq!(aviation.flights[&body.id].fuel_nm, 800.0);
        assert_eq!(body.altitude_m, 0.0);
    }

    // Change only normal source input. The single exact-name declaration now
    // resolves to two actual live aircraft of the same declared class.
    let mut ambiguous = input;
    ambiguous["units"][1]["name"] = json!("DUP");
    ambiguous["context"]["waterborne_2026"]["units"]
        .as_object_mut()
        .unwrap()
        .remove("SPARE");
    match load(ambiguous) {
        Err(reason) => {
            let lower = reason.to_lowercase();
            assert!(
                lower.contains("unique")
                    || lower.contains("ambiguous")
                    || lower.contains("exactly one"),
                "reject the ambiguous exact-body binding, not unrelated card/flight data: {reason}"
            );
        }
        Ok(world) => {
            let selected: Vec<_> = world
                .rules
                .aviation
                .as_ref()
                .unwrap()
                .flights
                .keys()
                .copied()
                .collect();
            let candidates: Vec<_> = world
                .entities
                .values()
                .filter(|e| e.name == "DUP" && e.alive)
                .map(|e| e.id)
                .collect();
            panic!("exact-name waterborne configuration must bind uniquely; actual same-name live bodies={candidates:?}, silently selected Flight bodies={selected:?}");
        }
    }
}

use harpoon_core::{Database,Scenario};
use serde_json::json;
#[test]
fn a_parked_aircraft_binding_rejects_an_enemy_surface_ship(){
    let db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc:Scenario=serde_json::from_value(json!({"id":"storage-lab","name":"scenario_assumption: storage binding invalid","duration_s":100,"seed":1,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","ground_2026":{"source":"scenario_assumption: individual parking","facilities":{"PARK":{"template":null,"custom":{"size":"F","damage_points":1,"vertical_armor":0,"horizontal_armor":0,"effect":"aircraft_one","source":"scenario_assumption: parking"},"source":"scenario_assumption: parking","stored_aircraft":["ENEMY"]}}}},
        "units":[{"side":"Blue","class":"us_ford","name":"PARK","pos":[0,0],"speed_kts":0,"fire_on":false},{"side":"Red","class":"cn_052d","name":"ENEMY","pos":[0,10],"speed_kts":0,"fire_on":false}]})).unwrap();
    assert!(harpoon_core::load_scenario(sc,db).is_err(),"facility aircraft-loss bindings must name actual own parked aircraft");
}

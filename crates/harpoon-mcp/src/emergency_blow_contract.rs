//! Actual MCP Session dispatch; stdio framing is checked separately by root.
use super::*;
use harpoon_core::EntityId;

const ACTOR: EntityId = EntityId(1);

fn fixture() -> Session {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut hull = db.platforms["us_virginia"].clone();
    hull.id = "scenario_assumption_emergency_protocol_hull".into();
    hull.name = "Authored emergency protocol hull".into();
    hull.max_depth_m = 500.;
    hull.max_speed_kts = 34.;
    hull.weapons.clear();
    hull.sensors.clear();
    db.platforms.insert(hull.id.clone(), hull);
    let sc: Scenario = serde_json::from_value(json!({
        "id":"mcp-emergency-protocol","name":"Authored MCP emergency geometry",
        "seed":20261024,"duration_s":5000,
        "context":{"rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "navigation_2026":{"source":"scenario_assumption: protocol mechanism fixture","units":{
                "OWN":{"source":"scenario_assumption: explicit own movement fit","size":"C",
                    "merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,
                    "undamaged_max_kts":34,"propulsion":"Nuclear","readiness":"Underway",
                    "submarine":{"source":"scenario_assumption: controlled hull and water",
                        "maximum_depth_m":500,"surface_max_kts":12,"water_depth_m":1000,"very_small":false}}
            }}},
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[
            {"side":"Blue","class":"scenario_assumption_emergency_protocol_hull","name":"OWN",
                "pos":[0,0],"depth_m":300,"speed_kts":20,"fire_on":false},
            {"side":"Red","class":"us_ford","name":"REMOTE","pos":[1000,0],"fire_on":false}
        ]
    })).unwrap();
    let mut w = load_scenario(sc, db.clone()).unwrap();
    for e in w.entities.values_mut() {
        e.sensors.clear();
        e.fire_on = false;
    }
    assert!(harpoon_core::rules::navigation2026::submarine::configured_depth(&w, ACTOR));
    Session {
        world: Some(w),
        inbox: vec![],
        speed: 0.,
        scenario_dir: PathBuf::from("scenarios"),
        database: db,
    }
}
fn blow(side: &str, target: f64) -> Value {
    json!({"side":side,"command":{"cmd":"navigation","unit":ACTOR,
        "action":{"type":"emergency_blow","depth_m":target}}})
}

#[test]
fn emergency_mcp_invalid_source_blow_is_not_reported_queued_or_mutated() {
    let mut s = fixture();
    for target in [0., 25., 100., 200., 300., 400.] {
        let before = serde_json::to_value(s.world.as_ref().unwrap()).unwrap();
        let result = s.call("send_command", &blow("Blue", target));
        assert!(
            result.is_err(),
            "target {target} must fail before an MCP queued receipt: {result:?}"
        );
        assert!(s.inbox.is_empty());
        assert_eq!(
            before,
            serde_json::to_value(s.world.as_ref().unwrap()).unwrap()
        );
        assert!(!observe(s.world.as_ref().unwrap(), Side::Blue, 0, 0).units[0].manual_control);
    }
}

#[test]
fn emergency_mcp_valid_blow_keeps_explicit_manual_authority_and_actual_operation() {
    let mut s = fixture();
    let manual = s
        .call(
            "send_command",
            &json!({"side":"Blue","command":{
        "cmd":"set_unit_automation","unit":ACTOR,"enabled":false}}),
        )
        .unwrap();
    assert_eq!(manual["queued"], true);
    let release = s.call("send_command", &blow("Blue", 50.)).unwrap();
    assert_eq!(release["queued"], true);
    assert_eq!(
        s.inbox.len(),
        2,
        "MCP keeps its existing explicit-control policy"
    );
    let step = s.call("step", &json!({"ticks":1})).unwrap();
    assert_eq!(step["commands_applied"], 2);
    let w = s.world.as_ref().unwrap();
    assert!(!w.over, "the protocol fixture must survive its first tick");
    assert!(w.entity(ACTOR).unwrap().depth_m < 300.);
    let own = s.call("get_state", &json!({"side":"Blue"})).unwrap();
    let u = own["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == ACTOR.0)
        .unwrap();
    assert_eq!(u["manual_control"], true);
    assert_eq!(
        u["navigation"]["source_rules"]["depth_operation"]["kind"],
        "EmergencyBlow"
    );
    let enemy = s.call("get_state", &json!({"side":"Red"})).unwrap();
    assert!(!enemy["units"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u["id"] == ACTOR.0));
    assert!(!serde_json::to_string(&enemy)
        .unwrap()
        .contains("EmergencyBlow"));
}

#[test]
fn emergency_mcp_foreign_actor_error_does_not_consult_hidden_source_state() {
    let mut a = fixture();
    let mut b = fixture();
    {
        let e = b.world.as_mut().unwrap().entity_mut(ACTOR).unwrap();
        e.name = "PRIVATE_CHANGED_HULL".into();
        e.depth_m = 100.;
        e.speed_kts = 5.;
    }
    let first = a.call("send_command", &blow("Red", 50.)).unwrap_err();
    let second = b.call("send_command", &blow("Red", 50.)).unwrap_err();
    assert_eq!(first, second);
    assert!(!first.contains("PRIVATE_CHANGED_HULL"));
    assert!(a.inbox.is_empty() && b.inbox.is_empty());
}

#[test]
fn emergency_mcp_discovers_the_same_nested_action_from_the_shared_contract() {
    let schema = command_schema();
    let nav = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["properties"]["cmd"]["const"] == "navigation")
        .unwrap();
    let action = nav["properties"]["action"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["properties"]["type"]["const"] == "emergency_blow")
        .expect("new source action must be discoverable without a second static list");
    assert_eq!(action["properties"]["depth_m"]["type"], "number");
    assert_eq!(action["properties"]["depth_m"]["minimum"], 0);
    assert_eq!(action["required"], json!(["type", "depth_m"]));
    assert_eq!(action["additionalProperties"], false);
}

#[test]
fn emergency_mcp_set_depth_is_not_queued_when_it_would_cancel_a_live_blow_before_three_bands() {
    let mut s = fixture();
    // An autonomous MCP client uses the same real source command, without an
    // explicit player takeover. The operation itself must not invent manual.
    s.call("send_command", &blow("Blue", 50.)).unwrap();
    s.call("step", &json!({"ticks":1})).unwrap();
    let w = s.world.as_ref().unwrap();
    let own = observe(w, Side::Blue, 0, 100);
    assert!(!own.units[0].manual_control);
    assert_eq!(
        own.units[0]
            .navigation
            .as_ref()
            .unwrap()
            .source_rules
            .as_ref()
            .unwrap()["depth_operation"]["kind"],
        "EmergencyBlow",
        "must reach a real source operation before testing cancellation"
    );
    let current = w.entity(ACTOR).unwrap().depth_m;
    assert!(current < 300. && current > 200.);
    let before = serde_json::to_value(w).unwrap();
    let result = s.call(
        "send_command",
        &json!({"side":"Blue","command":{
        "cmd":"set_depth","unit":ACTOR,"depth_m":current}}),
    );
    assert!(
        result.is_err(),
        "a core-invalid cancellation must not receive queued: {result:?}"
    );
    assert!(s.inbox.is_empty());
    assert_eq!(
        before,
        serde_json::to_value(s.world.as_ref().unwrap()).unwrap()
    );
}

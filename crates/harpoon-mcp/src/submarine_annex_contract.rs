//! Real Session command/observation dispatch; stdio is verified separately.
use super::*;
use harpoon_core::EntityId;

fn fixture() -> Session {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let scenario:Scenario=serde_json::from_value(json!({
        "id":"annex-mcp","name":"Exact source band, no invented runtime fit", "seed":20261024,"duration_s":5000,
        "context":{"rules_profile":"hv2026-rts-v4"},"weather":{"visibility_nm":40,"jam":[0,0]},
        "units":[{"side":"Blue","class":"cn_kilo","name":"OWN","pos":[0,0],"depth_m":50,"speed_kts":0,"fire_on":false},
                 {"side":"Red","class":"us_ford","name":"REMOTE","pos":[1000,0],"fire_on":false}]
    })).unwrap();
    let mut world = load_scenario(scenario, db.clone()).unwrap();
    for entity in world.entities.values_mut() {
        entity.sensors.clear();
        entity.fire_on = false;
    }
    Session {
        world: Some(world),
        inbox: vec![],
        speed: 0.,
        scenario_dir: PathBuf::from("scenarios"),
        database: db,
    }
}

#[test]
fn annex_mcp_source_limit_is_checked_before_inbox_or_any_world_mutation() {
    let mut session = fixture();
    let before = serde_json::to_value(session.world.as_ref().unwrap()).unwrap();
    let result = session.call(
        "send_command",
        &json!({"side":"Blue","command":{"cmd":"set_depth","unit":1,"depth_m":401}}),
    );
    assert!(
        result.is_err(),
        "MCP must reject above Int IV before saying queued: {result:?}"
    );
    assert!(session.inbox.is_empty());
    assert_eq!(
        before,
        serde_json::to_value(session.world.as_ref().unwrap()).unwrap()
    );
}

#[test]
fn annex_mcp_legal_manual_depth_crosses_the_old_scalar_without_inventing_source_motion_fit() {
    let mut session = fixture();
    session.call("send_command",&json!({"side":"Blue","command":{"cmd":"set_unit_automation","unit":1,"enabled":false}})).unwrap();
    session
        .call(
            "send_command",
            &json!({"side":"Blue","command":{"cmd":"set_depth","unit":1,"depth_m":400}}),
        )
        .unwrap();
    session.call("step", &json!({"ticks":130})).unwrap();
    let world = session.world.as_ref().unwrap();
    assert_eq!(world.entity(EntityId(1)).unwrap().depth_m, 400.);
    assert_eq!(world.db.platforms["cn_kilo"].max_depth_m, 200.);
    assert!(!harpoon_core::rules::navigation2026::submarine::configured_depth(world, EntityId(1)));
    let observation = serde_json::to_value(observe(world, Side::Blue, 0, 0)).unwrap();
    assert_eq!(observation["units"][0]["manual_control"], true);
    assert_eq!(
        observation["units"][0]["navigation"]["source_rules"]["annex_capability"]
            ["maximum_game_depth_m"],
        400.
    );
    assert!(observation["units"][0]["navigation"]["source_rules"]
        .get("fit")
        .is_none());
    assert!(observation["units"][0]["battery"].is_null());
}

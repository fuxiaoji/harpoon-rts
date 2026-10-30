//! Implemented-unverified candidate. Root registers under tools.rs; no stdio/HTTP run.
use super::*;
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

fn fixture_world(fitted: bool) -> World {
    let mut context = json!({"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"}});
    if fitted {
        context["communications_2026"] = json!({"source":"scenario_assumption: explicit contract fit","units":{"CONTROL":{"source":"scenario_assumption: laboratory comms","fits":{"lab":{"source":"scenario_assumption: lab data terminal","protocol":"contract","send":true,"receive":true,"operational":true,"peers":[],"warning_kind":"real_time","automatic_warning":false}}}}});
        context["unmanned_2026"] = json!({"source":"scenario_assumption: original hulls / explicit unmanned conversion","controllers":{"CONTROL":{"source":"scenario_assumption: control console","elements":["console"],"operational":true,"acoustic_modem":true,"overt_recall_transmitter":true}},"vehicles":{"BOT":{"source":"scenario_assumption: underwater hardware conversion","domain":"underwater","autonomous_capable":true,"initial_mode":"autonomous","controller":"CONTROL","element":"console","initial_link":"acoustic_modem","acoustic_modem":true,"periscope_depth_m":15,"loss_behavior":{"type":"return_to_home","home":"CONTROL"}}}});
        context["nuclear_2026"] = json!({"source":"scenario_assumption: table laboratory, not actual Tomahawk nuclear fit","units":{"CONTROL":{"us_tomahawk":{"source":"scenario_assumption: laboratory nuclear warhead","source_variant":"scenario_assumption: source table test only","allowed_yields_kt":[1],"selected_yield_kt":1,"medium":"surface","fuze":"terminal_seeker","authorized":false}}}});
    }
    let sc:Scenario=serde_json::from_value(json!({"id":"extended-player-source-contract","name":"Explicit command contract laboratory with original hulls/stocks","duration_s":10000,"seed":19,"context":context,"weather":{"visibility_nm":0,"jam":[0,0]},"units":[
        {"side":"Blue","class":"us_burke_2a","name":"CONTROL","pos":[0,0],"speed_kts":0,"fire_on":false},
        {"side":"Blue","class":"us_virginia","name":"BOT","pos":[0,1],"speed_kts":0,"depth_m":50,"fire_on":false},
        {"side":"Red","class":"us_virginia","name":"ENEMY","pos":[1000,1000],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    harpoon_core::load_scenario(
        sc,
        Database::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../database/consolidated.json"
        )))
        .unwrap(),
    )
    .unwrap()
}

fn samples() -> Vec<Value> {
    let mut rows = vec![];
    for action in [
        json!({"type":"send_warning","contact":0,"sensor":"actual","fit":"lab","receivers":[2]}),
        json!({"type":"set_automatic","fit":"lab","enabled":true}),
        json!({"type":"abandon_alert","contact":0}),
        json!({"type":"send_solution","contact":0,"sensor":"actual","fit":"lab","receivers":[2],"weapon":"us_mk48","speed_index":0}),
        json!({"type":"combine_reports","contact":0,"own_sensor":"actual","messages":[{"sender":2,"id":1}],"aim_source":{"sender":2,"id":1}}),
    ] {
        rows.push(json!({"cmd":"communications","unit":1,"action":action}));
    }
    for action in [
        json!({"type":"switch_mode","mode":"hitl"}),
        json!({"type":"take_control","controller":2,"element":"console","link":"acoustic_modem"}),
        json!({"type":"assign_autonomous","controller":2,"element":"console","link":"acoustic_modem"}),
        json!({"type":"set_link","link":"satellite"}),
        json!({"type":"plot_mission","waypoints":[{"x":1.,"y":2.}],"speed_kts":2.}),
        json!({"type":"request_radio","vehicle":2,"link":"satellite"}),
        json!({"type":"exchange_basic_data","contact":0,"sensor":"actual"}),
    ] {
        rows.push(json!({"cmd":"unmanned","unit":1,"action":action}));
    }
    for action in [
        json!({"type":"set_yield","weapon":"actual","yield_kt":1}),
        json!({"type":"authorize","weapon":"actual","enabled":true}),
    ] {
        rows.push(json!({"cmd":"nuclear","unit":1,"action":action}));
    }
    for action in [
        json!({"type":"start_charging"}),
        json!({"type":"stop_charging"}),
        json!({"type":"raise_snorkel"}),
        json!({"type":"retract_masts"}),
        json!({"type":"set_aip_operating","enabled":true}),
        json!({"type":"set_power_mode","mode":"battery"}),
    ] {
        rows.push(json!({"cmd":"navigation","unit":1,"action":{"type":"battery","action":action}}));
    }
    rows
}
fn fixture(fitted: bool) -> Session {
    Session {
        world: Some(fixture_world(fitted)),
        ..Session::new()
    }
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn send(s: &mut Session, raw: Value) -> Result<Value, String> {
    s.call("send_command", &json!({"side":"Blue","command":raw}))
}
fn set_auto(actor: EntityId) -> Value {
    json!({"cmd":"communications","unit":actor,"action":{"type":"set_automatic","fit":"lab","enabled":true}})
}
#[test]
fn extended_mcp_unknown_sources_reject_all_shapes_without_queue_or_world_mutation() {
    let mut s = fixture(false);
    let before = serde_json::to_value(s.world.as_ref().unwrap()).unwrap();
    for raw in samples() {
        assert!(
            send(&mut s, raw.clone()).is_err(),
            "unknown source accepted: {raw}"
        );
        assert!(s.inbox.is_empty());
        assert_eq!(
            before,
            serde_json::to_value(s.world.as_ref().unwrap()).unwrap()
        );
    }
}
#[test]
fn extended_mcp_rejects_internal_manual_flags_and_private_source_fields_before_queue() {
    let mut s = fixture(true);
    let before = serde_json::to_value(s.world.as_ref().unwrap()).unwrap();
    for raw in samples() {
        if raw["cmd"] != "navigation" {
            let mut forged = raw.clone();
            forged["retain_manual"] = json!(true);
            assert!(send(&mut s, forged).is_err());
        }
        for key in [
            "source",
            "d100",
            "success",
            "quality",
            "fcs",
            "world_target",
            "telemetry",
            "cause",
        ] {
            let mut forged = raw.clone();
            if forged["cmd"] == "navigation" {
                forged["action"]["action"][key] = json!(1);
            } else {
                forged["action"][key] = json!(1);
            }
            assert!(
                send(&mut s, forged).is_err(),
                "private key {key} accepted for {raw}"
            );
        }
        assert!(s.inbox.is_empty());
        assert_eq!(
            before,
            serde_json::to_value(s.world.as_ref().unwrap()).unwrap()
        );
    }
}
#[test]
fn extended_mcp_manual_commit_waits_for_actual_source_application_and_failure_does_not_take_over() {
    let mut s = fixture(true);
    let actor = id(s.world.as_ref().unwrap(), "CONTROL");
    send(&mut s, set_auto(actor)).unwrap();
    assert_ne!(
        s.world
            .as_ref()
            .unwrap()
            .entity(actor)
            .unwrap()
            .ext
            .get("control.manual"),
        Some(&1.)
    );
    s.call("step", &json!({"ticks":1})).unwrap();
    assert_eq!(
        s.world
            .as_ref()
            .unwrap()
            .entity(actor)
            .unwrap()
            .ext
            .get("control.manual"),
        Some(&1.)
    );
    let mut failed = fixture(true);
    let actor = id(failed.world.as_ref().unwrap(), "CONTROL");
    send(&mut failed, set_auto(actor)).unwrap();
    harpoon_core::rules::communications2026::set_communication_cause(
        failed.world.as_mut().unwrap(),
        actor,
        "referee_contract: source disabled after queue",
        true,
    )
    .unwrap();
    failed.call("step", &json!({"ticks":1})).unwrap();
    assert_ne!(
        failed
            .world
            .as_ref()
            .unwrap()
            .entity(actor)
            .unwrap()
            .ext
            .get("control.manual"),
        Some(&1.)
    );
}
#[test]
fn extended_mcp_known_modem_roster_can_explicitly_resume_ai_but_cannot_target_an_enemy_controller()
{
    let mut s = fixture(true);
    let (controller, bot, enemy) = (
        id(s.world.as_ref().unwrap(), "CONTROL"),
        id(s.world.as_ref().unwrap(), "BOT"),
        id(s.world.as_ref().unwrap(), "ENEMY"),
    );
    let before = serde_json::to_value(s.world.as_ref().unwrap()).unwrap();
    assert!(send(&mut s,json!({"cmd":"unmanned","unit":bot,"action":{"type":"assign_autonomous","controller":enemy,"element":"console","link":"acoustic_modem"}})).is_err());
    assert!(s.inbox.is_empty());
    assert_eq!(
        before,
        serde_json::to_value(s.world.as_ref().unwrap()).unwrap()
    );
    send(&mut s,json!({"cmd":"unmanned","unit":bot,"action":{"type":"take_control","controller":controller,"element":"console","link":"acoustic_modem"}})).unwrap();
    s.call("step", &json!({"ticks":181})).unwrap();
    assert_eq!(
        s.world
            .as_ref()
            .unwrap()
            .entity(bot)
            .unwrap()
            .ext
            .get("control.manual"),
        Some(&1.)
    );
    assert!(
        !s.call("get_state", &json!({"side":"Blue"})).unwrap()["units"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u["id"] == bot.0)
    );
    send(
        &mut s,
        json!({"cmd":"set_unit_automation","unit":bot,"enabled":true}),
    )
    .unwrap();
    s.call("step", &json!({"ticks":1})).unwrap();
    assert_ne!(
        s.world
            .as_ref()
            .unwrap()
            .entity(bot)
            .unwrap()
            .ext
            .get("control.manual"),
        Some(&1.)
    );
}

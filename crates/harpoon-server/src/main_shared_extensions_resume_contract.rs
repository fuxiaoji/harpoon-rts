//! Implemented-unverified candidate. Root registers under main.rs; no HTTP run.
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
fn fixture(fitted: bool) -> Game {
    let w = fixture_world(fitted);
    let mut g = Game::new(w.db.clone());
    for slot in &mut g.tactical {
        slot.enabled = false;
    }
    g.world = Some(w);
    g
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn decode(raw: Value) -> Command {
    serde_json::from_value(raw).unwrap()
}
fn set_auto(unit: EntityId) -> Command {
    decode(
        json!({"cmd":"communications","unit":unit,"action":{"type":"set_automatic","fit":"lab","enabled":true}}),
    )
}
fn tick(g: &mut Game) {
    let commands = std::mem::take(&mut g.inbox);
    g.step(&commands);
}

#[test]
fn extended_player_unknown_sources_reject_every_new_shape_before_queue_or_manual_change() {
    let mut g = fixture(false);
    let before = serde_json::to_value(g.world.as_ref().unwrap()).unwrap();
    for raw in samples() {
        assert!(
            g.queue_player_command(Side::Blue, decode(raw.clone()))
                .is_err(),
            "unknown source accepted: {raw}"
        );
        assert!(g.inbox.is_empty());
        assert_eq!(
            serde_json::to_value(g.world.as_ref().unwrap()).unwrap(),
            before
        );
    }
}
#[test]
fn extended_player_internal_manual_flag_is_rejected_across_all_three_new_groups() {
    let mut g = fixture(true);
    let before = serde_json::to_value(g.world.as_ref().unwrap()).unwrap();
    for mut raw in samples().into_iter().filter(|r| r["cmd"] != "navigation") {
        raw["retain_manual"] = json!(true);
        assert!(g.queue_player_command(Side::Blue, decode(raw)).is_err());
        assert!(g.inbox.is_empty());
        assert_eq!(
            serde_json::to_value(g.world.as_ref().unwrap()).unwrap(),
            before
        );
    }
}
#[test]
fn extended_player_manual_authority_commits_only_after_actual_source_apply() {
    let mut g = fixture(true);
    let actor = id(g.world.as_ref().unwrap(), "CONTROL");
    g.queue_player_command(Side::Blue, set_auto(actor)).unwrap();
    assert_ne!(
        g.world
            .as_ref()
            .unwrap()
            .entity(actor)
            .unwrap()
            .ext
            .get("control.manual"),
        Some(&1.)
    );
    tick(&mut g);
    assert_eq!(
        g.world
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
    failed
        .queue_player_command(Side::Blue, set_auto(actor))
        .unwrap();
    harpoon_core::rules::communications2026::set_communication_cause(
        failed.world.as_mut().unwrap(),
        actor,
        "referee_contract: source failed while queued",
        true,
    )
    .unwrap();
    tick(&mut failed);
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
fn extended_player_modem_known_roster_supports_explicit_resume_without_releasing_to_ai_early() {
    let mut g = fixture(true);
    let (controller, bot) = (
        id(g.world.as_ref().unwrap(), "CONTROL"),
        id(g.world.as_ref().unwrap(), "BOT"),
    );
    let obs = harpoon_core::observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    assert!(!obs.units.iter().any(|u| u.id == bot));
    assert!(harpoon_core::observe::known_unmanned_unit(&obs, bot));
    let action = decode(
        json!({"cmd":"unmanned","unit":bot,"action":{"type":"take_control","controller":controller,"element":"console","link":"acoustic_modem"}}),
    );
    g.queue_player_command(Side::Blue, action).unwrap();
    tick(&mut g);
    while g.world.as_ref().unwrap().tick < 181 {
        g.step(&[]);
    }
    assert_eq!(
        g.world
            .as_ref()
            .unwrap()
            .entity(bot)
            .unwrap()
            .ext
            .get("control.manual"),
        Some(&1.)
    );
    let proposal = decode(
        json!({"cmd":"unmanned","unit":bot,"action":{"type":"switch_mode","mode":"autonomous"}}),
    );
    let (allowed, rejected) = g.supervise_ai(Side::Blue, vec![proposal]);
    assert!(allowed.is_empty());
    assert!(!rejected.is_empty());
    g.queue_player_command(
        Side::Blue,
        Command::SetUnitAutomation {
            unit: bot,
            enabled: true,
        },
    )
    .unwrap();
    tick(&mut g);
    assert_ne!(
        g.world
            .as_ref()
            .unwrap()
            .entity(bot)
            .unwrap()
            .ext
            .get("control.manual"),
        Some(&1.)
    );
}
#[test]
fn extended_agent_referenced_controllers_and_recall_vehicles_are_observation_owned() {
    let g = fixture(true);
    let w = g.world.as_ref().unwrap();
    let (controller, bot, enemy) = (id(w, "CONTROL"), id(w, "BOT"), id(w, "ENEMY"));
    let obs = harpoon_core::observe(w, Side::Blue, 0, 0);
    for raw in [
        json!({"cmd":"unmanned","unit":bot,"action":{"type":"assign_autonomous","controller":enemy,"element":"console","link":"acoustic_modem"}}),
        json!({"cmd":"unmanned","unit":controller,"action":{"type":"request_radio","vehicle":enemy,"link":"satellite"}}),
    ] {
        let (accepted, rejected) = agent::validate_commands(&obs, Side::Blue, &[decode(raw)]);
        assert!(accepted.is_empty());
        assert_eq!(rejected.len(), 1);
    }
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

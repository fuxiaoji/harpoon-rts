// Artifact-only candidate. Register as a child module of MCP tools.rs.
// These are actual Session API tests; final stdio transport is a separate gate.
use super::*;
use harpoon_core::{Command, Database, EntityId, Rng, Scenario, Side, World};
use serde_json::{json, Value};

const ACTOR: EntityId = EntityId(1);
fn native_die(seed: u64, purpose: &str) -> u8 {
    let mut rng = Rng::new(seed).derive(Rng::hash_tag(0, ACTOR.0, purpose));
    1 + (rng.next_f64() * 100.) as u8
}
fn seed() -> u64 {
    (1..=200_000)
        .find(|seed| {
            native_die(*seed, "3.1.4.hard:1:1:risk") == 5
                && native_die(*seed, "3.1.4.hard:1:1:free:0") > 25
                && native_die(*seed, "3.1.4.hard:1:1:free:1") <= 25
        })
        .expect("choose native seed before creating World; never overwrite dice")
}
fn fixture_world(with_fit: bool) -> World {
    let db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let mut context = json!({"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"}});
    if with_fit {
        context["navigation_2026"] = json!({"source":"scenario_assumption: controlled source manoeuvre geometry, not complete real Burke hardware",
            "units":{"SHIP":{"source":"scenario_assumption: explicit size/readiness fixture","size":"B",
                "merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,"undamaged_max_kts":30,
                "propulsion":"NonSteam","readiness":"Underway"}}});
    }
    let scenario:Scenario=serde_json::from_value(json!({
        "id":"hard-rudder-controller-candidate","name":"Authored hard-rudder shared-entry fixture","seed":seed(),"duration_s":5000,
        "context":context,"weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[{"side":"Blue","class":"us_burke_2a","name":"SHIP","pos":[0,0],"course_deg":0,"speed_kts":20,"fire_on":false},
                 {"side":"Red","class":"cn_093","name":"REMOTE RETAINER","pos":[500,500],"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut world = harpoon_core::load_scenario(scenario, db).unwrap();
    for entity in world.entities.values_mut() {
        entity.sensors.clear();
        entity.weapons.clear();
        entity.deck = None;
        entity.fire_on = false;
    }
    assert_eq!(world.entity(ACTOR).unwrap().name, "SHIP");
    world
}
fn decode(action: Value) -> Command {
    let value = json!({"cmd":"navigation","unit":ACTOR,"action":action});
    let result = serde_json::from_value(value.clone());
    assert!(result.is_ok(),"source-legal new shape must parse before its real-behaviour assertion: {value}; {result:?}");
    result.unwrap()
}
fn hard() -> Command {
    decode(json!({"type":"hard_rudder","course_deg":45,"direction":"starboard"}))
}
fn recovery(automatic: bool) -> Command {
    decode(json!({"type":"rudder_recovery","automatic":automatic}))
}
fn free() -> Command {
    decode(json!({"type":"free_rudder"}))
}
fn own(world: &World) -> harpoon_core::observe::Observation {
    harpoon_core::observe(world, Side::Blue, 0, 1000)
}
fn steering(world: &World) -> Value {
    serde_json::to_value(own(world)).unwrap()["units"][0]["navigation"]["source_rules"]["steering"]
        .clone()
}
fn jammed(world: &World) -> bool {
    steering(world)["jammed"] == true
}
fn start_real_jam(world: &mut World) {
    harpoon_core::sim::tick(world, &[(Side::Blue, hard())]);
    for _ in 0..30 {
        if jammed(world) {
            break;
        }
        harpoon_core::sim::tick(world, &[]);
    }
    assert!(
        jammed(world),
        "natural risk5 must create an actual own jam before controller/privacy tests"
    );
    assert_eq!(steering(world)["mode"], "jammed");
}
fn assert_public_steering(world: &World) {
    let value = steering(world);
    assert!(value.is_object(), "real own steering whitelist is missing");
    let expected = std::collections::BTreeSet::from([
        "mode",
        "target_course_deg",
        "direction",
        "jammed",
        "recovery",
        "pending",
    ]);
    let actual: std::collections::BTreeSet<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(actual, expected);
    let r = &value["recovery"];
    let expected = std::collections::BTreeSet::from(["automatic", "armed", "next_resolution_at"]);
    let actual: std::collections::BTreeSet<_> =
        r.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(actual, expected);
    fn check(value: &Value) {
        match value {
            Value::Object(map) => {
                for key in [
                    "episodes",
                    "active_episode",
                    "next_episode",
                    "cause",
                    "risk",
                    "checks",
                    "purpose",
                    "d100",
                ] {
                    assert!(
                        !map.contains_key(key),
                        "private hard-rudder key leaked: {key}"
                    );
                }
                for nested in map.values() {
                    check(nested);
                }
            }
            Value::Array(values) => {
                for nested in values {
                    check(nested);
                }
            }
            _ => {}
        }
    }
    let projected = serde_json::to_value(own(world)).unwrap();
    check(&projected["units"][0]["navigation"]["source_rules"]);
    for event in projected["events"].as_array().unwrap() {
        assert!(!matches!(
            event["rule"].as_str(),
            Some("3.1.4.hard_rudder_risk" | "3.1.4.hard_rudder_free" | "3.1.4.hard_rudder_pivot")
        ));
    }
    let red = serde_json::to_value(harpoon_core::observe(world, Side::Red, 0, 1000)).unwrap();
    assert!(
        red["contacts"].as_array().unwrap().is_empty(),
        "fixture creates no free enemy report"
    );
    assert!(!red.to_string().contains("3.1.4.hard:1:1"));
}

fn fixture(with_fit: bool) -> Session {
    let world = fixture_world(with_fit);
    let database = world.db.clone();
    Session {
        world: Some(world),
        inbox: vec![],
        speed: 0.,
        scenario_dir: PathBuf::from("scenarios"),
        database,
    }
}
fn send(session: &mut Session, side: &str, command: Command) -> Result<Value, String> {
    session.call("send_command", &json!({"side":side,"command":command}))
}
#[test]
fn hard_rudder_mcp_missing_fit_or_foreign_actor_never_enters_inbox_or_mutates_world() {
    for (with_fit, side) in [(false, "Blue"), (true, "Red")] {
        let mut session = fixture(with_fit);
        let before = serde_json::to_value(session.world.as_ref().unwrap()).unwrap();
        assert!(send(&mut session, side, hard()).is_err());
        assert!(session.inbox.is_empty());
        assert_eq!(
            before,
            serde_json::to_value(session.world.as_ref().unwrap()).unwrap()
        );
    }
}
#[test]
fn hard_rudder_mcp_has_real_jam_and_explicit_manual_one_slot_then_auto_recovery_without_extra_trial(
) {
    let mut session = fixture(true);
    send(&mut session, "Blue", hard()).unwrap();
    session.call("step", &json!({"ticks":30})).unwrap();
    assert!(jammed(session.world.as_ref().unwrap()));
    assert!(
        !own(session.world.as_ref().unwrap()).units[0].manual_control,
        "MCP does not silently add HTTP-style player takeover"
    );
    send(
        &mut session,
        "Blue",
        Command::SetUnitAutomation {
            unit: ACTOR,
            enabled: false,
        },
    )
    .unwrap();
    send(&mut session, "Blue", free()).unwrap();
    session.call("step", &json!({"ticks":1})).unwrap();
    assert!(
        jammed(session.world.as_ref().unwrap()),
        "manual free arms one future Resolution and cannot succeed immediately"
    );
    assert!(own(session.world.as_ref().unwrap()).units[0].manual_control);
    assert_eq!(
        steering(session.world.as_ref().unwrap())["recovery"]["armed"],
        true
    );
    session.call("step", &json!({"ticks":149})).unwrap();
    assert!(
        jammed(session.world.as_ref().unwrap()),
        "first natural recovery roll was selected>25"
    );
    assert_eq!(
        steering(session.world.as_ref().unwrap())["recovery"]["armed"],
        false
    );
    send(&mut session, "Blue", recovery(true)).unwrap();
    send(&mut session, "Blue", free()).unwrap();
    session.call("step", &json!({"ticks":180})).unwrap();
    assert!(
        !jammed(session.world.as_ref().unwrap()),
        "second natural<=25 recovery executes despite persistent manual"
    );
    assert!(own(session.world.as_ref().unwrap()).units[0].manual_control);
    assert_public_steering(session.world.as_ref().unwrap());
    let ledger = serde_json::to_value(
        session
            .world
            .as_ref()
            .unwrap()
            .rules
            .navigation
            .as_ref()
            .unwrap(),
    )
    .unwrap();
    let checks = &ledger["units"]["1"]["hard_rudder"]["episodes"]["3.1.4.hard:1:1"]["checks"];
    assert_eq!(
        checks.as_object().unwrap().len(),
        2,
        "manual+auto must consume the same Resolution slots"
    );
}
#[test]
fn hard_rudder_mcp_rejects_forged_success_without_a_physical_or_control_side_effect() {
    let mut session = fixture(true);
    let before = serde_json::to_value(session.world.as_ref().unwrap()).unwrap();
    // First require the new legal grammar, so absent-feature rejection cannot
    // make every forged-action sample look like supported behavior.
    let _ = free();
    let result=session.call("send_command",&json!({"side":"Blue","command":{"cmd":"navigation","unit":1,"action":{"type":"free_rudder","success":true,"d100":1}}}));
    assert!(result.is_err() && session.inbox.is_empty());
    assert_eq!(
        before,
        serde_json::to_value(session.world.as_ref().unwrap()).unwrap()
    );
}

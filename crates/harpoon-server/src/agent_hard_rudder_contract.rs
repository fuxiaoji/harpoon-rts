// Artifact-only candidate. Register as a child module of server agent.rs.
// Uses current private prompt functions only through the normal child-module scope.
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

fn context(language: &str) -> TurnContext {
    let mut settings = Settings::from_env();
    settings.language = language.into();
    TurnContext {
        supervision: Value::Null,
        side: Side::Blue,
        trigger: "test".into(),
        settings,
        autonomy: "advise".into(),
        memory: Memory::default(),
        recent: vec![],
        generation: 0,
        scenario_id: "hard-rudder-controller-candidate".into(),
        seed: seed(),
        tick: 0,
        director: false,
        wake_rules: vec![],
        trigger_detail: String::new(),
    }
}
#[test]
fn hard_rudder_llm_bilingual_prompt_describes_distinct_explicit_recovery_authority() {
    for language in ["zh-CN", "en-US"] {
        let text = build_system_prompt(&context(language));
        for term in [
            "hard_rudder",
            "rudder_recovery",
            "free_rudder",
            "25%",
            "20/40/60",
        ] {
            assert!(
                text.contains(term),
                "missing source recovery distinction {term} in {language}"
            );
        }
        if language == "zh-CN" {
            assert!(text.contains("授权") && text.contains("手动"));
        } else {
            let text = text.to_lowercase();
            assert!(text.contains("authoriz") && text.contains("manual"));
        }
    }
}
#[test]
fn hard_rudder_llm_receives_actual_own_white_projection_and_no_episode_or_raw_dice() {
    let mut world = fixture_world(true);
    start_real_jam(&mut world);
    assert_public_steering(&world);
    let obs = own(&world);
    assert!(world.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..}if rule=="3.1.4.hard_rudder_risk")),"privacy must inspect a real risk event, not an empty trace");
    for language in ["zh-CN", "en-US"] {
        let prompt = build_user_prompt(&context(language), &obs, &world.db, None);
        assert!(prompt.contains("\"steering\"") && prompt.contains("\"jammed\":true"));
        for forbidden in [
            "3.1.4.hard:1:1",
            "\"cause\":",
            "\"risk\":",
            "\"checks\":",
            "\"episodes\":",
            "\"d100\":",
        ] {
            assert!(
                !prompt.contains(forbidden),
                "actual private hard-rudder data leaked for {language}: {forbidden}"
            );
        }
    }
}
#[test]
fn hard_rudder_new_actions_preserve_the_existing_ai_manual_gate() {
    let mut world = fixture_world(true);
    start_real_jam(&mut world);
    harpoon_core::sim::tick(
        &mut world,
        &[(
            Side::Blue,
            Command::SetUnitAutomation {
                unit: ACTOR,
                enabled: false,
            },
        )],
    );
    let obs = own(&world);
    for command in [hard(), recovery(true), free()] {
        let (accepted, rejected) = validate_commands(&obs, Side::Blue, &[command]);
        assert!(
            accepted.is_empty() && !rejected.is_empty(),
            "no new action may bypass manual=true"
        );
    }
}

// Artifact-only candidate. Register as a child module of server main.rs only
// after root approves; no production registration or Cargo execution performed.
use super::*;
use crate::ai::CommandStaff as _;
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

fn fixture(with_fit: bool) -> Game {
    let world = fixture_world(with_fit);
    let mut game = Game::new(world.db.clone());
    for controller in &mut game.tactical {
        controller.enabled = false;
    }
    game.recording = Some(persistence::Recording::new(&world));
    game.world = Some(world);
    game
}
fn queued_tick(game: &mut Game) {
    let commands = std::mem::take(&mut game.inbox);
    game.step(&commands);
}
#[test]
fn hard_rudder_shared_schema_has_three_typed_actions_and_rejects_private_input() {
    let schema = harpoon_core::commands::command_schema();
    let nav = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["properties"]["cmd"]["const"] == "navigation")
        .unwrap();
    for (kind, action) in [
        (
            "hard_rudder",
            json!({"type":"hard_rudder","course_deg":45,"direction":"starboard"}),
        ),
        (
            "rudder_recovery",
            json!({"type":"rudder_recovery","automatic":true}),
        ),
        ("free_rudder", json!({"type":"free_rudder"})),
    ] {
        assert!(
            nav["properties"]["action"]["oneOf"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["properties"]["type"]["const"] == kind),
            "missing shared action {kind}"
        );
        let command = decode(action.clone());
        assert_eq!(command.unit_id(), ACTOR);
        for key in [
            "episode", "cause", "d100", "chance", "success", "jammed", "attempts",
        ] {
            let mut forged = json!({"cmd":"navigation","unit":ACTOR,"action":action.clone()});
            forged["action"][key] = json!(1);
            assert!(
                serde_json::from_value::<Command>(forged).is_err(),
                "client private key {key} must not parse"
            );
        }
    }
    assert!(serde_json::from_value::<Command>(json!({"cmd":"navigation","unit":1,"action":{"type":"hard_rudder","course_deg":45,"direction":"guess"}})).is_err());
}
#[test]
fn hard_rudder_player_source_rejection_precedes_inbox_and_manual_takeover() {
    let mut game = fixture(false);
    let before = serde_json::to_value(game.world.as_ref().unwrap()).unwrap();
    let rejected = game.queue_player_command(Side::Blue, hard());
    assert!(
        rejected.is_err(),
        "missing own source fit cannot queue hard rudder"
    );
    assert!(game.inbox.is_empty());
    assert_eq!(
        before,
        serde_json::to_value(game.world.as_ref().unwrap()).unwrap()
    );
    assert!(!own(game.world.as_ref().unwrap()).units[0].manual_control);
}
#[test]
fn hard_rudder_ai_preflight_rejects_missing_fit_before_reserving_a_task() {
    let mut game = fixture(false);
    let obs = own(game.world.as_ref().unwrap());
    let (candidates, mut errors) = agent::validate_commands(&obs, Side::Blue, &[hard()]);
    let (accepted, rejected) = game.supervise_ai(Side::Blue, candidates);
    errors.extend(rejected);
    assert!(accepted.is_empty() && !errors.is_empty());
    assert!(game
        .ai_supervision
        .as_ref()
        .is_none_or(|p| !p[0].tasks.contains_key(&ACTOR)));
    assert!(game.inbox.is_empty());
    assert!(!own(game.world.as_ref().unwrap()).units[0].manual_control);
}
#[test]
fn hard_rudder_player_legally_takes_manual_control_without_stopping_native_jam_or_authorized_recovery(
) {
    let mut game = fixture(true);
    game.queue_player_command(Side::Blue, recovery(true))
        .unwrap();
    game.queue_player_command(Side::Blue, hard()).unwrap();
    assert_eq!(
        game.inbox
            .iter()
            .filter(
                |(_, c)| matches!(c,Command::SetUnitAutomation{unit,enabled:false}if *unit==ACTOR)
            )
            .count(),
        1
    );
    queued_tick(&mut game);
    for _ in 0..30 {
        if jammed(game.world.as_ref().unwrap()) {
            break;
        }
        game.step(&[]);
    }
    assert!(jammed(game.world.as_ref().unwrap()));
    assert!(own(game.world.as_ref().unwrap()).units[0].manual_control);
    assert_eq!(
        steering(game.world.as_ref().unwrap())["recovery"]["automatic"],
        true
    );
    while game.world.as_ref().unwrap().tick < 180 {
        game.step(&[]);
    }
    assert!(
        jammed(game.world.as_ref().unwrap()),
        "first native authorized25% trial was selected to fail"
    );
    game.queue_player_command(Side::Blue, free()).unwrap();
    queued_tick(&mut game);
    while game.world.as_ref().unwrap().tick < 360 {
        game.step(&[]);
    }
    assert!(
        !jammed(game.world.as_ref().unwrap()),
        "second native trial succeeds without manual takeover cancelling prior authorization"
    );
    assert!(own(game.world.as_ref().unwrap()).units[0].manual_control);
    assert_public_steering(game.world.as_ref().unwrap());
    assert!(game
        .recording
        .as_ref()
        .unwrap()
        .verify(game.world.as_ref().unwrap())
        .is_ok());
}
#[test]
fn hard_rudder_fsm_consumes_real_own_jam_and_emits_no_substitute_actor_command() {
    let mut world = fixture_world(true);
    let mut staff = tactical::TacticalStaff::with_db(Side::Blue, world.db.clone());
    assert!(
        staff
            .decide(&own(&world))
            .iter()
            .any(|c| c.unit_id() == ACTOR),
        "ordinary control observation must be active"
    );
    staff.take_notes();
    start_real_jam(&mut world);
    assert!(
        !own(&world).units[0].manual_control,
        "this is an AI-observable actual jam, not a manual empty fixture"
    );
    assert_public_steering(&world);
    let commands = staff.decide(&own(&world));
    assert!(commands.iter().all(|c|c.unit_id()!=ACTOR),"global/role/queued salvos must all hold actor, not emit ClearStation/WeaponsFree or new route: {}",serde_json::to_string(&commands).unwrap());
    let mut actual = world.clone();
    let mut baseline = world.clone();
    let first: Vec<_> = commands.into_iter().map(|c| (Side::Blue, c)).collect();
    for step in 0..30 {
        harpoon_core::sim::tick(&mut actual, if step == 0 { &first } else { &[] });
        harpoon_core::sim::tick(&mut baseline, &[]);
    }
    assert_eq!(
        serde_json::to_value(&actual).unwrap(),
        serde_json::to_value(&baseline).unwrap()
    );
    assert!(
        actual
            .entity(ACTOR)
            .unwrap()
            .pos
            .range_to(world.entity(ACTOR).unwrap().pos)
            > 0.
    );
    assert!(
        jammed(&actual),
        "no authorization means no automatic free trial"
    );
}
#[test]
fn hard_rudder_fsm_keeps_persistent_manual_and_observationally_hidden_enemy_changes_out_of_its_decision(
) {
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
    let mut a = tactical::TacticalStaff::with_db(Side::Blue, world.db.clone());
    assert!(a.decide(&own(&world)).iter().all(|c| c.unit_id() != ACTOR));
    harpoon_core::sim::tick(
        &mut world,
        &[(
            Side::Blue,
            Command::SetUnitAutomation {
                unit: ACTOR,
                enabled: true,
            },
        )],
    );
    let mut changed = world.clone();
    let enemy = changed.entity_mut(EntityId(2)).unwrap();
    enemy.name = "PRIVATE ENEMY CHANGE".into();
    enemy.depth_m = 333.;
    enemy.speed_kts = 19.;
    let mut b = tactical::TacticalStaff::with_db(Side::Blue, world.db.clone());
    let oa = a.decide(&own(&world));
    let ob = b.decide(&own(&changed));
    assert!(oa.iter().all(|c| c.unit_id() != ACTOR) && ob.iter().all(|c| c.unit_id() != ACTOR));
    assert_eq!(
        serde_json::to_value(oa).unwrap(),
        serde_json::to_value(ob).unwrap()
    );
    assert!(jammed(&world));
}

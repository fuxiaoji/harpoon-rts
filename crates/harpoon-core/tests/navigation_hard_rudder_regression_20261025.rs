//! Artifact-only source/native candidates, not registered or run here.
//! Copy to crates/harpoon-core/tests/ only in the parent's Cargo window.
//! Every pivot below comes from a legal command and actual advance distance;
//! no clock, heading, turn marker, runtime ledger or risk die is injected.
use harpoon_core::rules::navigation2026 as nav;
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
use std::sync::OnceLock;

fn no_jam_seed() -> u64 {
    (1..=20_000)
        .find(|seed| {
            let mut rng = Rng::new(*seed).derive(Rng::hash_tag(0, 1, "3.1.4.hard:1:1:risk"));
            1 + (rng.next_f64() * 100.) as u8 == 6
        })
        .expect("a natural D100=6 seed exists")
}
fn fixture(size: char, waterjet: bool) -> (World, EntityId) {
    static DB: OnceLock<Database> = OnceLock::new();
    let db = DB
        .get_or_init(|| {
            Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
        })
        .clone();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"source-hard-rudder-regression-20261025",
        "name":"scenario_assumption source-table geometry, not real Burke propulsion/size claim",
        "seed":no_jam_seed(),"duration_s":20000,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "navigation_2026":{"source":"scenario_assumption: controlled source advance/pivot mechanism",
                "units":{"SHIP":{"source":"scenario_assumption: explicit size/waterjet/readiness",
                    "size":size,"merchant":false,"passenger_liner":false,
                    "cpp":false,"waterjet":waterjet,"undamaged_max_kts":30,
                    "propulsion":"NonSteam","readiness":"Underway"}}}},
        "units":[
            {"side":"Blue","class":"us_burke_2a","name":"SHIP","pos":[0,0],
                "course_deg":0,"speed_kts":20,"depth_m":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DISTANT RETAINER","pos":[500,500],
                "course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut world = harpoon_core::load_scenario(sc, db).unwrap();
    let id = world
        .entities
        .values()
        .find(|e| e.name == "SHIP")
        .unwrap()
        .id;
    assert_eq!(id, EntityId(1));
    // Sensors are irrelevant fixture equipment, not a fabricated track.
    for entity in world.entities.values_mut() {
        entity.sensors.clear();
        entity.fire_on = false;
    }
    (world, id)
}
fn tick(world: &mut World, commands: &[(Side, Command)]) {
    assert!(
        !world.over,
        "both sides must survive the navigation fixture"
    );
    harpoon_core::sim::tick(world, commands);
}
fn until(world: &mut World, wanted: i64) {
    while world.tick < wanted {
        tick(world, &[]);
    }
}
fn hard(course_deg: f64) -> nav::Action {
    serde_json::from_value(json!({"type":"hard_rudder","course_deg":course_deg,
        "direction":"starboard"}))
    .unwrap()
}
fn details(world: &World, wanted: &str) -> Vec<(i64, Value)> {
    world
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == wanted => {
                Some((event.tick, serde_json::from_str(detail).unwrap()))
            }
            _ => None,
        })
        .collect()
}
fn close(actual: f64, expected: f64, why: &str) {
    assert!(
        (actual - expected).abs() < 1e-10,
        "{why}: actual={actual}, source expected={expected}"
    );
}
fn command_hard(world: &mut World, id: EntityId, course_deg: f64) {
    tick(
        world,
        &[(
            Side::Blue,
            Command::Navigation {
                unit: id,
                action: hard(course_deg),
            },
        )],
    );
}
fn native_b_pivot_at(pivot_tick: i64) -> (World, EntityId) {
    let (mut world, id) = fixture('B', false);
    // Source B hard advance = 200yd. At constant20kt the first pivot is on
    // movement tick18 after the order (200*0.0004937365010799136*3600/20).
    // This schedules a command, rather than forcing the resulting pivot.
    until(&mut world, pivot_tick - 18);
    command_hard(&mut world, id, 45.);
    until(&mut world, pivot_tick);
    let pivots = details(&world, "3.1.4.hard_rudder_pivot");
    assert_eq!(pivots.len(), 1, "exactly one real hard pivot must happen");
    assert_eq!(pivots[0].0, pivot_tick, "pivot time must be a native event");
    assert_eq!(pivots[0].1["angle_deg"], 45.);
    assert_eq!(pivots[0].1["advance_yd"], 200.);
    assert_eq!(pivots[0].1["turn_window"], pivot_tick / 180);
    close(
        world.entity(id).unwrap().course_deg,
        45.,
        "native target course",
    );
    close(
        world.entity(id).unwrap().speed_kts,
        17.,
        "source B hard loss is3kt",
    );
    let risk = details(&world, "3.1.4.hard_rudder_risk");
    assert_eq!(risk.len(), 1);
    assert_eq!(risk[0].1["d100"], 6, "natural no-jam control seed");
    (world, id)
}
fn accelerate_once(world: &mut World, id: EntityId) -> f64 {
    let before = world.entity(id).unwrap().speed_kts;
    tick(
        world,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: id,
                speed_kts: 30.,
            },
        )],
    );
    world.entity(id).unwrap().speed_kts - before
}

#[test]
fn h11_hard_native_pivot_at179_does_not_move_half_acceleration_into180() {
    // A pivot made at179 belongs to the closing turn. A pivot at180 must
    // still halve181; compare both native cases, not only a cleared marker.
    let (mut at179, id) = native_b_pivot_at(179);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&at179).unwrap()).unwrap();
    restored.rules.validate_tables().unwrap();
    let gain180 = accelerate_once(&mut at179, id);
    let restored_gain = accelerate_once(&mut restored, id);
    assert_eq!(at179.tick, 180);
    assert_eq!(
        serde_json::to_value(&at179).unwrap(),
        serde_json::to_value(&restored).unwrap(),
        "complete World at the transition must restore identically"
    );
    close(
        restored_gain,
        gain180,
        "saved old-window pivot must keep the same effect",
    );

    let (mut at180, control_id) = native_b_pivot_at(180);
    let gain181 = accelerate_once(&mut at180, control_id);
    assert_eq!(at180.tick, 181);
    // Source Ship Speed Change B (non-CPP) acceleration10kt/Tac; a45deg
    // turn halves the rate in its own turn. Both cases are below75% max.
    close(gain181, 5. / 180., "same-turn positive penalty control");
    close(
        gain180,
        10. / 180.,
        "old-window pivot must not halve new-window acceleration",
    );
    let motion180 = details(&at179, "3.1.motion_limits");
    let (_, latest) = motion180.last().unwrap();
    assert_eq!(latest["turning45"], false);
}

#[test]
fn h12_waterjet_g_missing_source_row_is_rejected_by_readonly_preflight_before_plan_or_dice() {
    let action = hard(60.);
    let (mut smallest, id) = fixture('G', true);
    let before = serde_json::to_vec(&smallest).unwrap();
    let validation = nav::validate_action(&smallest, id, &action);
    assert_eq!(
        before,
        serde_json::to_vec(&smallest).unwrap(),
        "readonly check must not mutate World"
    );
    assert!(validation.is_err(),
        "waterjet G needs an unprinted class belowG; reject before shared HTTP queue/manual ownership: {validation:?}");
    let applied = nav::apply(&mut smallest, id, &action);
    assert!(
        applied.is_err(),
        "the core apply entry must use the identical source-row gate"
    );
    assert_eq!(
        before,
        serde_json::to_vec(&smallest).unwrap(),
        "unsupported fit must not leave an episode, plan, risk die or order trace"
    );
    assert!(details(&smallest, "3.1.4.hard_rudder_risk").is_empty());

    // The gate must retain legal waterjets: A uses the printed B row and a
    // real60deg pivot; this is explicit author hardware, not a Burke claim.
    let (mut legal, legal_id) = fixture('A', true);
    let before_legal = serde_json::to_vec(&legal).unwrap();
    assert!(nav::validate_action(&legal, legal_id, &action).is_ok());
    assert_eq!(before_legal, serde_json::to_vec(&legal).unwrap());
    command_hard(&mut legal, legal_id, 60.);
    until(&mut legal, 30);
    let pivots = details(&legal, "3.1.4.hard_rudder_pivot");
    assert_eq!(pivots.len(), 1);
    assert_eq!(pivots[0].1["advance_yd"], 200.);
    assert_eq!(pivots[0].1["angle_deg"], 60.);
    assert_eq!(pivots[0].1["waterjet"], true);
    close(
        legal.entity(legal_id).unwrap().course_deg,
        60.,
        "legal waterjet actual movement",
    );
    assert_eq!(details(&legal, "3.1.4.hard_rudder_risk").len(), 1);
}

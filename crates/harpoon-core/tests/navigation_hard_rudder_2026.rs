//! Artifact-only native candidates. Copy to core/tests only after the parent's
//! next-window registration. All new Actions are decoded through existing
//! serde types; an absent feature is an assertion failure, not a missing type.
//! Size/propulsion fits are explicit author mechanism inputs, not hull claims.
use harpoon_core::rules::navigation2026 as nav;
use harpoon_core::{
    Command, Database, EntityId, EventKind, Rng, Scenario, Side, Visibility, World,
};
use serde_json::{json, Value};
use std::sync::OnceLock;

fn identity(id: EntityId) -> String {
    format!("3.1.4.hard:{}:1", id.0)
}
fn die(seed: u64, id: EntityId, purpose: &str) -> u8 {
    let mut r = Rng::new(seed).derive(Rng::hash_tag(0, id.0, purpose));
    1 + (r.next_f64() * 100.) as u8
}
fn risk(seed: u64) -> u8 {
    die(
        seed,
        EntityId(1),
        &format!("{}:risk", identity(EntityId(1))),
    )
}
fn recovery(seed: u64, window: i64) -> u8 {
    die(
        seed,
        EntityId(1),
        &format!("{}:free:{window}", identity(EntityId(1))),
    )
}
fn seed_for(test: impl Fn(u64) -> bool) -> u64 {
    (1..=2_000_000)
        .find(|seed| test(*seed))
        .expect("pre-World native seed exists")
}
fn fixture(seed: u64, size: char, speed: f64, sub: bool) -> (World, EntityId) {
    static DB: OnceLock<Database> = OnceLock::new();
    let db = DB
        .get_or_init(|| {
            Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
        })
        .clone();
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"source-hard-rudder-candidate","name":"scenario_assumption controlled hard-rudder mechanism geometry",
        "seed":seed,"duration_s":20000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "navigation_2026":{"source":"scenario_assumption: source hard-rudder mechanism, not real hull size/propulsion fit",
                "units":{"SHIP":{"source":"scenario_assumption: explicit geometry/readiness fit","size":size,
                    "merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,"undamaged_max_kts":30,
                    "propulsion":"NonSteam","readiness":"Underway"}}}},
        "units":[{"side":"Blue","class":if sub{"us_virginia"}else{"us_burke_2a"},"name":"SHIP",
                "pos":[0,0],"course_deg":0,"speed_kts":speed,"depth_m":if sub{50}else{0},"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DISTANT_RET AINER","pos":[500,500],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let id = w.entities.values().find(|e| e.name == "SHIP").unwrap().id;
    assert_eq!(id, EntityId(1));
    for e in w.entities.values_mut() {
        e.sensors.clear();
        e.fire_on = false;
    }
    (w, id)
}
fn action(value: Value) -> nav::Action {
    let decoded = serde_json::from_value(value.clone());
    assert!(
        decoded.is_ok(),
        "source-legal proposed navigation action must be accepted: {value}; {decoded:?}"
    );
    decoded.unwrap()
}
fn hard(course_deg: f64, direction: &str) -> nav::Action {
    action(json!({"type":"hard_rudder","course_deg":course_deg,"direction":direction}))
}
fn auto(enabled: bool) -> nav::Action {
    action(json!({"type":"rudder_recovery","automatic":enabled}))
}
fn free() -> nav::Action {
    action(json!({"type":"free_rudder"}))
}
fn command(id: EntityId, action: nav::Action) -> (Side, Command) {
    (Side::Blue, Command::Navigation { unit: id, action })
}
fn tick(w: &mut World, commands: &[(Side, Command)]) {
    assert!(
        !w.over,
        "fixture must retain both sides while physical navigation runs"
    );
    harpoon_core::sim::tick(w, commands);
}
fn until(w: &mut World, wanted: i64) {
    while w.tick < wanted {
        tick(w, &[]);
    }
}
fn close(a: f64, b: f64, why: &str) {
    assert!((a - b).abs() < 1e-8, "{why}: {a} != {b}");
}
fn private(w: &World, id: EntityId) -> Value {
    serde_json::to_value(&w.rules.navigation.as_ref().unwrap().units[&id]).unwrap()["hard_rudder"]
        .clone()
}
fn episode(w: &World, id: EntityId) -> Value {
    let value = private(w, id)["episodes"][identity(id)].clone();
    assert!(
        value.is_object(),
        "actual activated hard-rudder episode must be persisted, not only traced"
    );
    value
}
fn details(w: &World, rule_name: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == rule_name => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn pivots(w: &World) -> Vec<Value> {
    details(w, "3.1.4.hard_rudder_pivot")
}
fn checks(w: &World, id: EntityId) -> Value {
    episode(w, id)["checks"].clone()
}
fn restored(w: &World) -> World {
    let copy: World = serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap();
    copy.rules.validate_tables().unwrap();
    copy
}
fn same_world(a: &World, b: &World) {
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
}
fn legacy_rolls(w: &World, purpose: &str) -> Vec<Value> {
    details(w, "damage_roll")
        .into_iter()
        .filter(|d| d["purpose"] == purpose)
        .collect()
}

#[test]
fn h01_actual_hard_advance_and_loss_use_the_source_column_and_do_not_spend_past_straight_travel() {
    let seed = seed_for(|seed| risk(seed) == 6);
    let (mut w, id) = fixture(seed, 'B', 20., false);
    until(&mut w, 10);
    let start = w.entity(id).unwrap().pos;
    let ordered_at = w.tick;
    tick(&mut w, &[command(id, hard(45., "starboard"))]);
    while pivots(&w).is_empty() && w.tick < ordered_at + 200 {
        close(
            w.entity(id).unwrap().course_deg,
            0.,
            "full source advance precedes pivot",
        );
        close(
            w.entity(id).unwrap().pos.x,
            start.x,
            "the advance is on the prior course",
        );
        tick(&mut w, &[]);
    }
    let p = pivots(&w);
    assert_eq!(p.len(), 1, "one genuine45deg hard pivot must occur");
    close(
        p[0]["advance_yd"].as_f64().unwrap(),
        200.,
        "sourceB hard advance",
    );
    close(
        p[0]["speed_loss_kts"].as_f64().unwrap(),
        3.,
        "sourceB hard loss",
    );
    close(
        p[0]["pivot_position"]["y"].as_f64().unwrap() - start.y,
        200. * 0.0004937365010799136,
        "actual post-order advance",
    );
    close(
        w.entity(id).unwrap().course_deg,
        45.,
        "real hard pivot reached target",
    );
    let risk_record = episode(&w, id);
    assert_eq!(risk_record["risk"]["d100"], 6);
    assert_eq!(risk_record["risk"]["jammed"], false);
    let hard_seconds = w.tick - ordered_at;
    let (mut standard, sid) = fixture(seed, 'B', 20., false);
    until(&mut standard, 10);
    tick(
        &mut standard,
        &[(
            Side::Blue,
            Command::SetCourse {
                unit: sid,
                course_deg: 45.,
            },
        )],
    );
    while details(&standard, "3.1.4.pivot").is_empty() && standard.tick < 210 {
        tick(&mut standard, &[]);
    }
    let p = details(&standard, "3.1.4.pivot");
    assert_eq!(p.len(), 1);
    assert_eq!(p[0]["advance_yd"], 300.);
    assert_eq!(p[0]["speed_loss_kts"], 2.);
    assert!(
        hard_seconds < standard.tick - 10,
        "both columns must reach the actual movement path"
    );
}

#[test]
fn h02_natural_five_jam_crosses_the_target_and_full_circle_on_the_same_side_without_extra_risk() {
    let seed = seed_for(|seed| risk(seed) == 5);
    let (mut w, id) = fixture(seed, 'F', 30., false);
    let hp = w.entity(id).unwrap().hp;
    tick(&mut w, &[command(id, hard(45., "starboard"))]);
    for i in 0..90 {
        let commands = if i % 7 == 0 {
            vec![command(id, hard(90., "starboard"))]
        } else if i == 10 {
            vec![(
                Side::Blue,
                Command::SetUnitAutomation {
                    unit: id,
                    enabled: false,
                },
            )]
        } else if i == 11 {
            vec![(
                Side::Blue,
                Command::SetCourse {
                    unit: id,
                    course_deg: 270.,
                },
            )]
        } else {
            vec![]
        };
        tick(&mut w, &commands);
    }
    let p = pivots(&w);
    assert!(
        p.len() > 8,
        "jam must continue beyond the requested45deg and at least one full circle"
    );
    let mut physical_path = 0.;
    let mut prior = harpoon_core::Vec2::zero();
    for event in p.iter().take(9) {
        assert_eq!(event["advance_yd"], 50.);
        assert_eq!(event["angle_deg"], 45.);
        let from = event["from_course_deg"].as_f64().unwrap();
        let to = event["to_course_deg"].as_f64().unwrap();
        close(
            (to - from).rem_euclid(360.),
            45.,
            "jam side cannot flip at target or north",
        );
        let point: harpoon_core::Vec2 =
            serde_json::from_value(event["pivot_position"].clone()).unwrap();
        close(
            prior.range_to(point),
            50. * 0.0004937365010799136,
            "each pivot follows a real50yd leg",
        );
        physical_path += prior.range_to(point);
        prior = point;
    }
    assert!(physical_path >= 8. * 50. * 0.0004937365010799136);
    close(
        p.iter()
            .filter(|p| p["turn_window"] == 0)
            .map(|p| p["speed_loss_kts"].as_f64().unwrap())
            .sum(),
        4.,
        "only four source45deg losses perTac, circle continues",
    );
    assert_eq!(details(&w, "3.1.4.hard_rudder_risk").len(), 1);
    assert_eq!(episode(&w, id)["risk"]["d100"], 5);
    assert_eq!(episode(&w, id)["phase"], "jammed");
    close(
        w.entity(id).unwrap().hp,
        hp,
        "hard-rudder jam is not a weapon DP hit",
    );
    assert!(legacy_rolls(&w, "rudder_mode").is_empty());
    assert!(legacy_rolls(&w, "rudder_first_unjam_delay").is_empty());
    assert!(
        details(&w, "3.1.4.hard_rudder_free").is_empty(),
        "no crew-recovery authorization means no private attempt"
    );
}

#[test]
fn h03_four_actual_failed_resolutions_keep_twenty_five_percent_eligibility_and_the_fifth_twenty_five_frees(
) {
    let seed = seed_for(|seed| {
        risk(seed) == 5
            && (0..4).all(|window| recovery(seed, window) > 25)
            && recovery(seed, 4) == 25
    });
    let (mut w, id) = fixture(seed, 'F', 30., false);
    tick(
        &mut w,
        &[command(id, auto(true)), command(id, hard(45., "starboard"))],
    );
    for window in 0..4 {
        until(&mut w, (window + 1) * 180);
        let saved = checks(&w, id);
        assert_eq!(saved.as_object().unwrap().len(), (window + 1) as usize);
        let check = &saved[window.to_string()];
        assert_eq!(check["d100"], recovery(seed, window));
        assert_eq!(check["freed"], false);
        assert_eq!(check["closed_window"], window);
        assert_eq!(episode(&w, id)["phase"], "jammed");
    }
    assert!(
        pivots(&w).iter().any(|p| p["turn_window"] == 3),
        "still physically circles after ordinary three-attempt cutoff"
    );
    until(&mut w, 900);
    assert_eq!(checks(&w, id).as_object().unwrap().len(), 5);
    assert_eq!(checks(&w, id)["4"]["d100"], 25);
    assert_eq!(checks(&w, id)["4"]["freed"], true);
    assert_eq!(episode(&w, id)["phase"], "freed");
    let heading = w.entity(id).unwrap().course_deg;
    until(&mut w, 920);
    close(
        w.entity(id).unwrap().course_deg,
        heading,
        "recovery stabilizes actual heading, not obsolete requested heading",
    );
    assert!(legacy_rolls(&w, "14.6.rudder_unjam").is_empty());
}

#[test]
fn h04_manual_authorization_waits_for_one_slot_and_repeats_or_takeover_never_reroll_it() {
    let seed =
        seed_for(|seed| risk(seed) == 5 && recovery(seed, 0) == 26 && recovery(seed, 2) == 25);
    let (mut w, id) = fixture(seed, 'F', 30., false);
    tick(&mut w, &[command(id, hard(45., "starboard"))]);
    until(&mut w, 100);
    tick(
        &mut w,
        &[
            command(id, free()),
            command(id, free()),
            (
                Side::Blue,
                Command::SetUnitAutomation {
                    unit: id,
                    enabled: false,
                },
            ),
        ],
    );
    assert!(
        details(&w, "3.1.4.hard_rudder_free").is_empty(),
        "FreeRudder is authorization, not an immediate die"
    );
    until(&mut w, 180);
    assert_eq!(checks(&w, id)["0"]["d100"], 26);
    assert_eq!(checks(&w, id)["0"]["freed"], false);
    assert_eq!(private(&w, id)["recovery_armed"], false);
    until(&mut w, 359);
    assert_eq!(
        checks(&w, id).as_object().unwrap().len(),
        1,
        "no automatic attempt was authorized"
    );
    tick(&mut w, &[command(id, free()), command(id, free())]);
    assert_eq!(
        checks(&w, id).as_object().unwrap().len(),
        1,
        "tick360 Resolution precedes its new commands"
    );
    until(&mut w, 540);
    // The order issued at360 belongs to the next legal Resolution slot2.
    assert_eq!(checks(&w, id).as_object().unwrap().len(), 2);
    assert!(
        checks(&w, id).get("1").is_none(),
        "a command cannot retroactively consume the closed slot"
    );
    assert_eq!(checks(&w, id)["2"]["d100"], 25);
    assert_eq!(checks(&w, id)["2"]["freed"], true);
    assert_eq!(episode(&w, id)["phase"], "freed");
}

#[test]
fn h05_jam_at179_can_attempt_at180_but_jam_at180_waits_until360() {
    let seed =
        seed_for(|seed| risk(seed) == 5 && recovery(seed, 0) <= 25 && recovery(seed, 1) <= 25);
    for activated in [179, 180] {
        let (mut w, id) = fixture(seed, 'F', 30., false);
        until(&mut w, activated - 1);
        tick(
            &mut w,
            &[command(id, auto(true)), command(id, hard(45., "starboard"))],
        );
        assert_eq!(episode(&w, id)["activated_at"], activated);
        assert_eq!(episode(&w, id)["jam_window"], activated / 180);
        assert!(checks(&w, id).as_object().unwrap().is_empty());
        let due = if activated == 179 { 180 } else { 360 };
        until(&mut w, due - 1);
        assert!(checks(&w, id).as_object().unwrap().is_empty());
        tick(&mut w, &[]);
        assert_eq!(checks(&w, id).as_object().unwrap().len(), 1);
        let closed = activated / 180;
        assert_eq!(checks(&w, id)[closed.to_string()]["at_tick"], due);
        assert_eq!(
            checks(&w, id)[closed.to_string()]["d100"],
            recovery(seed, closed)
        );
        assert_eq!(episode(&w, id)["phase"], "freed");
    }
}

#[test]
fn h06_zero_speed_plan_has_no_risk_until_real_movement_actuates_the_rudder() {
    let seed = seed_for(|seed| risk(seed) == 6);
    let (mut w, id) = fixture(seed, 'B', 0., false);
    tick(&mut w, &[command(id, hard(45., "starboard"))]);
    until(&mut w, 20);
    assert!(details(&w, "3.1.4.hard_rudder_risk").is_empty());
    assert!(private(&w, id)["episodes"]
        .as_object()
        .is_none_or(|map| map.is_empty()));
    assert_eq!(w.entity(id).unwrap().pos, harpoon_core::Vec2::zero());
    close(
        w.entity(id).unwrap().course_deg,
        0.,
        "zero speed cannot pivot",
    );
    tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: id,
                speed_kts: 20.,
            },
        )],
    );
    assert!(w.entity(id).unwrap().pos.y > 0.);
    assert_eq!(details(&w, "3.1.4.hard_rudder_risk").len(), 1);
    assert_eq!(episode(&w, id)["risk"]["d100"], 6);
}

#[test]
fn h07_complete_world_restore_during_circle_and_on_resolution_keeps_physics_and_original_dice() {
    let seed =
        seed_for(|seed| risk(seed) == 5 && recovery(seed, 0) > 25 && recovery(seed, 1) <= 25);
    let (mut w, id) = fixture(seed, 'F', 30., false);
    tick(
        &mut w,
        &[command(id, auto(true)), command(id, hard(45., "starboard"))],
    );
    let base = restored(&w);
    for checkpoint in [1, 20, 179, 180, 359, 360] {
        let mut branch = restored(&base);
        until(&mut branch, checkpoint);
        assert_eq!(
            branch.tick, checkpoint,
            "each independent checkpoint must name its actual saved tick"
        );
        let mut copy = restored(&branch);
        for _ in 0..12 {
            let orders = vec![(
                Side::Blue,
                Command::SetUnitAutomation {
                    unit: id,
                    enabled: false,
                },
            )];
            tick(&mut branch, &orders);
            tick(&mut copy, &orders);
            same_world(&branch, &copy);
        }
    }
    until(&mut w, 372);
    assert_eq!(episode(&w, id)["risk"]["d100"], 5);
    assert_eq!(checks(&w, id)["0"]["d100"], recovery(seed, 0));
    assert_eq!(checks(&w, id)["1"]["d100"], recovery(seed, 1));
}

fn forbidden(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            ["episodes", "d100", "risk", "checks", "purpose"].contains(&key.as_str())
                || forbidden(value)
        }),
        Value::Array(values) => values.iter().any(forbidden),
        _ => false,
    }
}
#[test]
fn h08_actual_risk_and_recovery_remain_referee_while_own_equipment_status_is_visible() {
    let seed = seed_for(|seed| risk(seed) == 5 && recovery(seed, 0) == 26);
    let (mut w, id) = fixture(seed, 'F', 30., false);
    tick(
        &mut w,
        &[command(id, auto(true)), command(id, hard(45., "starboard"))],
    );
    until(&mut w, 180);
    for wanted in ["3.1.4.hard_rudder_risk", "3.1.4.hard_rudder_free"] {
        let events: Vec<_> = w
            .events
            .iter()
            .filter(|e| matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule==wanted))
            .collect();
        assert_eq!(events.len(), 1, "inspect a real nonempty source event");
        assert_eq!(events[0].vis, Visibility::Referee);
        for side in [Side::Blue, Side::Red] {
            assert!(!harpoon_core::observe(&w, side, 0, 10000)
                .events
                .iter()
                .any(|e| matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule==wanted)));
        }
    }
    let own = serde_json::to_value(harpoon_core::observe(&w, Side::Blue, 0, 10000)).unwrap();
    let ship = own["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|unit| unit["id"] == json!(id))
        .unwrap();
    let projection = &ship["navigation"]["source_rules"];
    assert_eq!(projection["steering"]["jammed"], true);
    assert_eq!(projection["steering"]["recovery"]["automatic"], true);
    assert!(
        !forbidden(projection),
        "own navigation must be a whitelist, not serialized private Unit"
    );
    assert!(harpoon_core::observe(&w, Side::Red, 0, 10000)
        .units
        .iter()
        .all(|unit| unit.id != id));
}

#[test]
fn h09_ordinary_source_rudder_damage_keeps_d6_delay_and_three_twenty_forty_sixty_attempts() {
    // Ordinary cause control deliberately enters the existing source damage
    // consumer, not a full weapon-flight fixture. No casualty flag is written.
    // Fixture correction during registration: the 14-2 columns in the current
    // damage tables only list Rudder in the surface/underwater/merchant bands,
    // so an airbreathing Harpoon never rolls it. The candidate's authored
    // Harpoon control is corrected to a source Mk48 underwater hit, which is
    // the real ordinary path to the same Rudder critical arm.
    let mut chosen = None;
    for seed in 1..=20_000 {
        let (mut w, id) = fixture(seed, 'B', 20., false);
        let def = w.db.weapons["us_mk48"].clone();
        harpoon_core::damage::apply_hit_from(&mut w, id, &def, true, Side::Red);
        let e = w.entity(id).unwrap();
        let due = e.ext.get("damage.rudder_repair_due").copied().unwrap_or(0.) as i64;
        if !e.alive
            || due <= 0
            || e.ext.get("damage.rudder_jammed") != Some(&1.)
            || e.ext.get("damage.rudder_disabled") == Some(&1.)
        {
            continue;
        }
        if legacy_rolls(&w, "rudder_mode").len() != 1
            || legacy_rolls(&w, "rudder_first_unjam_delay").len() != 1
        {
            continue;
        }
        let fails = (1..=3).all(|attempt| {
            let at = due + (attempt - 1) * 180;
            let mut rng = w.rng.derive(Rng::hash_tag(
                at,
                id.0,
                &format!("rudder_unjam:{}:{attempt}", id.0),
            ));
            1 + (rng.next_f64() * 10.) as u8 > 2 * attempt as u8
        });
        if !fails {
            continue;
        }
        // Concurrent fire/flood casualties are legitimate ordinary outcomes;
        // require the hull to survive the whole repair clock so the three
        // attempts themselves remain observable.
        until(&mut w, due + 540);
        let survived = w.entity(id).is_some_and(|e| e.alive)
            && details(&w, "rudder_unjam_result").len() == 3
            && details(&w, "rudder_unjam_result")
                .iter()
                .all(|event| event["success"] == false)
            && w.entity(id).unwrap().ext.get("damage.rudder_jammed") == Some(&1.);
        if survived {
            // Re-run from the same seed so the kept world is the pristine
            // post-hit state the final assertions below describe.
            let (w, id, due) = {
                let (mut fresh, id) = fixture(seed, 'B', 20., false);
                let def = fresh.db.weapons["us_mk48"].clone();
                harpoon_core::damage::apply_hit_from(&mut fresh, id, &def, true, Side::Red);
                let due = fresh
                    .entity(id)
                    .unwrap()
                    .ext
                    .get("damage.rudder_repair_due")
                    .copied()
                    .unwrap_or(0.) as i64;
                (fresh, id, due)
            };
            chosen = Some((w, id, due));
            break;
        }
    }
    let (mut w, id, due) =
        chosen.expect("bounded natural ordinary Rudder critical and three repair failures");
    let mode = legacy_rolls(&w, "rudder_mode");
    assert!(!mode.is_empty());
    assert_eq!(mode[0]["die"], 6);
    assert!(mode[0]["roll"].as_u64().unwrap() <= 3);
    let delay = legacy_rolls(&w, "rudder_first_unjam_delay");
    assert_eq!(delay.len(), 1);
    assert_eq!(delay[0]["die"], 6);
    assert_eq!(due, delay[0]["roll"].as_i64().unwrap() * 180);
    until(&mut w, due + 540);
    let attempts = details(&w, "rudder_unjam_result");
    assert_eq!(
        attempts.len(),
        3,
        "ordinary cause still stops after its three failed attempts"
    );
    for (index, event) in attempts.iter().enumerate() {
        assert_eq!(event["chance_percent"], 20 * (index + 1));
        assert_eq!(event["success"], false);
    }
    assert_eq!(
        w.entity(id).unwrap().ext.get("damage.rudder_jammed"),
        Some(&1.)
    );
    assert!(details(&w, "3.1.4.hard_rudder_risk").is_empty());
    assert!(details(&w, "3.1.4.hard_rudder_free").is_empty());
}

#[test]
fn h10_readonly_rejections_and_submerged_depth_control_gap_never_allocate_a_risk_episode() {
    for field in [
        "d100",
        "chance",
        "episode_id",
        "jammed",
        "success",
        "source",
    ] {
        let mut value = json!({"type":"hard_rudder","course_deg":45.,"direction":"starboard"});
        value[field] = json!(1);
        assert!(
            serde_json::from_value::<nav::Action>(value).is_err(),
            "client source/random input must be rejected"
        );
    }
    let (w, _id) = fixture(77, 'B', 20., false);
    let invalid = hard(45., "starboard");
    let before = serde_json::to_value(&w).unwrap();
    assert!(nav::validate_action(&w, EntityId(u32::MAX), &invalid).is_err());
    assert_eq!(serde_json::to_value(&w).unwrap(), before);
    let (sub, sid) = fixture(77, 'C', 20., true);
    let before = serde_json::to_value(&sub).unwrap();
    assert!(
        nav::validate_action(&sub, sid, &hard(45., "starboard")).is_err(),
        "no crush/depth-control implementation means this combination stays explicitly unsupported"
    );
    assert_eq!(serde_json::to_value(&sub).unwrap(), before);
}

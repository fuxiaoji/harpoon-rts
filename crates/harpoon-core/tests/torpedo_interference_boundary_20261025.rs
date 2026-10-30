//! Independent native mutual-guidance boundaries, English2026 PDF79/7-4.
//! Shared author Good/contact/tube geometry isolates post-release physics;
//! there is no Flight/fuel/RNG injection. Exact Mk48 source speed settings are
//! retained except the explicitly labelled identical-payload identity control.
#[path = "support/torpedo_interference_fixture.rs"]
mod fixture_support;

use fixture_support::{events, fixture_setup, until};
use harpoon_core::{rules::torpedo2026, EntityId, World};
use serde_json::json;

fn close(actual: f64, expected: f64, why: &str) {
    assert!(
        (actual - expected).abs() < 1e-10,
        "{why}: actual={actual}, expected={expected}"
    );
}

#[derive(Clone, Copy, Debug)]
struct Physical {
    x: f64,
    y: f64,
    course: f64,
    speed: f64,
    run_nm: f64,
    fuel: f64,
    live_s: f64,
    setting: usize,
}
fn physical(world: &World, id: EntityId) -> Physical {
    let body = world.entity(id).expect("real live projectile body");
    assert!(body.alive, "source meeting must not invent destruction");
    let flight = &world.rules.torpedoes.as_ref().unwrap().flights[&id];
    Physical {
        x: body.pos.x,
        y: body.pos.y,
        course: body.course_deg,
        speed: body.speed_kts,
        run_nm: flight.run_nm,
        fuel: flight.fuel,
        live_s: body.projectile.as_ref().unwrap().live_s,
        setting: flight.speed_index,
    }
}
fn same_physics(a: Physical, b: Physical, why: &str) {
    for (actual, expected, field) in [
        (a.x, b.x, "x"),
        (a.y, b.y, "y"),
        (a.speed, b.speed, "speed"),
        (a.run_nm, b.run_nm, "run"),
        (a.fuel, b.fuel, "fuel"),
        (a.live_s, b.live_s, "live_s"),
    ] {
        close(actual, expected, &format!("{why}/{field}"));
    }
    close(
        harpoon_core::geom::angle_diff(a.course, b.course),
        0.,
        &format!("{why}/course"),
    );
    assert_eq!(a.setting, b.setting, "{why}/setting");
}
fn one_motion(before: Physical, after: Physical, speed: f64, range_nm: f64, elapsed: f64) {
    let travel = speed / 3600. * elapsed;
    close(
        (after.x - before.x).hypot(after.y - before.y),
        travel,
        "each member moves once by its own rated speed and common elapsed time",
    );
    close(
        after.run_nm - before.run_nm,
        travel,
        "actual source run increment",
    );
    close(
        after.fuel - before.fuel,
        travel / range_nm,
        "source endurance fraction increment; no reset on speed change",
    );
    close(
        after.live_s - before.live_s,
        elapsed,
        "one common physical time increment",
    );
    close(after.speed, speed, "actual selected source speed");
}
fn native_step(world: &mut World) {
    assert!(!world.over);
    harpoon_core::sim::tick(world, &[]);
}

#[test]
fn source_slow_capture_tick_keeps_40kt_motion_then_next_tick_uses_65kt_without_free_endurance() {
    // Source Mk48 Mod7 settings: index1=40kt/22.6nm; index0=65kt/15nm.
    // Both source-enabled weapons are well outside a one-tick meeting at
    // 0.5nm, so the capture epoch and next epoch each pay a full second.
    let (mut world, ids, _) = fixture_setup(0.5, 2, false, 0., false, None, false, |value| {
        for name in ["LAUNCHER0", "LAUNCHER1"] {
            value["context"]["torpedo_2026"]["orders"][format!("{name}:us_mk48")]["speed_index"] =
                json!(1);
        }
    });
    assert_eq!(world.tick, 1);
    assert!(events(&world, "7.5.3.same_type_interference").is_empty());
    let before: Vec<_> = ids.iter().map(|id| physical(&world, *id)).collect();
    assert!(before.iter().all(|p| p.setting == 1));
    native_step(&mut world);
    assert_eq!(world.tick, 2);
    assert_eq!(events(&world, "7.5.3.same_type_interference").len(), 2);
    let captured: Vec<_> = ids.iter().map(|id| physical(&world, *id)).collect();
    for index in 0..2 {
        assert_eq!(
            captured[index].setting, 1,
            "capture does not grant retroactive fast motion"
        );
        one_motion(before[index], captured[index], 40., 22.6, 1.);
    }
    native_step(&mut world);
    assert_eq!(world.tick, 3);
    for (index, id) in ids.iter().enumerate() {
        let fast = physical(&world, *id);
        assert_eq!(fast.setting, 0);
        one_motion(captured[index], fast, 65., 15., 1.);
        close(
            fast.fuel,
            before[index].fuel + (40. / 3600.) / 22.6 + (65. / 3600.) / 15.,
            "both source costs accumulate on the pre-capture fraction",
        );
    }
}

#[test]
fn source_equal_capability_payloads_with_distinct_actual_weapon_ids_do_not_form_a_pair() {
    // Explicit laboratory counterfactual, not a claim that Mk54 has Mk48
    // performance: copy the whole model capability into both scenario keys.
    // The real database weapon IDs, tubes, mounts and actual Fire stay distinct.
    let (mut world, ids, _) = fixture_setup(0.5, 2, true, 0., false, None, false, |value| {
        let mut equal = torpedo2026::tables()["weapons"]["us_mk48"].clone();
        equal["source"] = json!("scenario_assumption: identical laboratory capability payload for ID discrimination; not real Mk54 performance");
        value["context"]["torpedo_2026"]["weapons"] =
            json!({"us_mk48":equal.clone(),"us_mk54":equal});
    });
    assert_eq!(world.entity(ids[0]).unwrap().class, "us_mk48");
    assert_eq!(world.entity(ids[1]).unwrap().class, "us_mk54");
    let state = world.rules.torpedoes.as_ref().unwrap();
    assert_eq!(
        serde_json::to_value(&state.flights[&ids[0]].weapon).unwrap(),
        serde_json::to_value(&state.flights[&ids[1]].weapon).unwrap(),
        "counterfactual must actually present identical complete capability payloads",
    );
    let before: Vec<_> = ids.iter().map(|id| physical(&world, *id)).collect();
    until(&mut world, 20);
    assert!(events(&world, "7.5.3.same_type_interference").is_empty());
    assert!(events(&world, "7.5.3.interference_pending").is_empty());
    for (index, id) in ids.iter().enumerate() {
        let after = physical(&world, *id);
        assert!(
            after.run_nm > before[index].run_nm,
            "different-ID weapons really continue moving"
        );
        close(
            after.x,
            before[index].x,
            "unpaired Run keeps the original northbound course",
        );
        assert!(!world.rules.pending_adjudications.contains_key(id));
    }
}

fn reordered_pair(reverse_scene_and_fire: bool) -> (World, [EntityId; 2]) {
    let (world, ids, _) = fixture_setup(0.5, 2, false, 0., false, None, false, |value| {
        if reverse_scene_and_fire {
            // Preserve each physical platform's class/position while reversing
            // the scenario array. Swap only names, so the shared helper's
            // LAUNCHER0-then1 real Fire order becomes right-then-left.
            let units = value["units"].as_array_mut().unwrap();
            units.swap(0, 1);
            units[0]["name"] = json!("LAUNCHER0");
            units[1]["name"] = json!("LAUNCHER1");
        }
        value["context"]["torpedo_2026"]["orders"]["LAUNCHER0:us_mk48"]["speed_index"] =
            json!(if reverse_scene_and_fire { 0 } else { 1 });
        value["context"]["torpedo_2026"]["orders"]["LAUNCHER1:us_mk48"]["speed_index"] =
            json!(if reverse_scene_and_fire { 1 } else { 0 });
    });
    // Compare by immutable launch geometry, excluding identities, log labels
    // and RNG-purpose strings. No physical or private runtime state is edited.
    let state = world.rules.torpedoes.as_ref().unwrap();
    let at = |x: f64| {
        ids.iter()
            .copied()
            .find(|id| (state.flights[id].origin.x - x).abs() < 1e-12)
            .unwrap()
    };
    let roles = [at(0.), at(0.5)];
    assert_eq!(state.flights[&roles[0]].speed_index, 1);
    assert_eq!(state.flights[&roles[1]].speed_index, 0);
    if reverse_scene_and_fire {
        assert!(
            roles[1] < roles[0],
            "right-hand physical weapon must really launch first"
        );
    } else {
        assert!(
            roles[0] < roles[1],
            "left-hand physical weapon must really launch first"
        );
    }
    (world, roles)
}

#[test]
fn source_swapped_scene_and_actual_fire_order_preserve_every_tick_of_unequal_speed_pair_physics() {
    let (mut forward, forward_roles) = reordered_pair(false);
    let (mut reversed, reversed_roles) = reordered_pair(true);
    assert_ne!(
        forward_roles[0], reversed_roles[0],
        "left weapon IDs must actually differ"
    );
    assert_ne!(
        forward_roles[1], reversed_roles[1],
        "right weapon IDs must actually differ"
    );
    let mut met_at = None;
    for _ in 0..35 {
        assert_eq!(forward.tick, reversed.tick);
        let before: Vec<_> = forward_roles
            .iter()
            .map(|id| physical(&forward, *id))
            .collect();
        for role in 0..2 {
            same_physics(
                before[role],
                physical(&reversed, reversed_roles[role]),
                "matched physical role before tick",
            );
        }
        let held = forward_roles
            .iter()
            .all(|id| forward.rules.pending_adjudications.contains_key(id));
        let distance = (before[0].x - before[1].x).hypot(before[0].y - before[1].y);
        let capture_tick = forward.tick + 1 == 2;
        let speeds = if capture_tick { [40., 65.] } else { [65., 65.] };
        let ranges = if capture_tick {
            [22.6, 15.]
        } else {
            [15., 15.]
        };
        let elapsed = if held {
            0.
        } else {
            (distance / ((speeds[0] + speeds[1]) / 3600.)).min(1.)
        };
        native_step(&mut forward);
        native_step(&mut reversed);
        for role in 0..2 {
            let after = physical(&forward, forward_roles[role]);
            same_physics(
                after,
                physical(&reversed, reversed_roles[role]),
                "order-independent role after tick",
            );
            if held {
                same_physics(
                    after,
                    before[role],
                    "terminal pending preserves all physical state",
                );
            } else {
                one_motion(before[role], after, speeds[role], ranges[role], elapsed);
            }
        }
        let forward_held = forward_roles
            .iter()
            .all(|id| forward.rules.pending_adjudications.contains_key(id));
        let reverse_held = reversed_roles
            .iter()
            .all(|id| reversed.rules.pending_adjudications.contains_key(id));
        assert_eq!(
            forward_held, reverse_held,
            "meeting epoch cannot depend on Fire/ID order"
        );
        if forward_held && met_at.is_none() {
            met_at = Some(forward.tick);
            let left = physical(&forward, forward_roles[0]);
            let right = physical(&forward, forward_roles[1]);
            close(
                (left.x - right.x).hypot(left.y - right.y),
                0.,
                "actual common meeting point",
            );
        }
    }
    assert!(
        met_at.is_some(),
        "both orders must reach an actual physical meeting"
    );
    assert_eq!(events(&forward, "7.5.3.same_type_interference").len(), 2);
    assert_eq!(events(&reversed, "7.5.3.same_type_interference").len(), 2);
    assert!(events(&forward, "10.2.5.source_terminal").is_empty());
    assert!(events(&reversed, "10.2.5.source_terminal").is_empty());
}

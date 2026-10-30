//! Native source regression cases. Shared fixture does not inject flights or dice.
#[path = "support/torpedo_interference_fixture.rs"]
mod fixture_support;
use fixture_support::*;
use harpoon_core::{EventKind, Side, World};
use serde_json::{json, Value};

#[test]
fn source_unique_pair_really_homes_then_preserves_alive_terminal_pending_without_invented_explosion(
) {
    let (mut w, ids, target) = fixture(0.5, 2, false, 0., false);
    let hp = w.entity(target).unwrap().hp;
    let releases: Value =
        serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().launches).unwrap();
    actual_lock(&mut w);
    let start = w
        .entity(ids[0])
        .unwrap()
        .pos
        .range_to(w.entity(ids[1]).unwrap().pos);
    until(&mut w, 6);
    assert!(
        w.entity(ids[0])
            .unwrap()
            .pos
            .range_to(w.entity(ids[1]).unwrap().pos)
            < start
    );
    until(&mut w, 100);
    let pending = events(&w, "7.5.3.interference_pending");
    assert!(
        !pending.is_empty(),
        "physical meeting must preserve the unsourced terminal for adjudication"
    );
    for id in ids {
        assert!(w.entity(id).unwrap().alive);
        assert!(w.rules.pending_adjudications.contains_key(&id));
    }
    assert_eq!(w.entity(target).unwrap().hp, hp);
    assert!(events(&w, "10.2.5.source_terminal").is_empty());
    assert!(events(&w, "7.5.3.same_type_collision").is_empty());
    assert_eq!(
        serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().launches).unwrap(),
        releases
    );
}
#[test]
fn source_distance_boundary_is_5000_yards_inclusive_with_two_real_weapons() {
    let boundary = 5000. * 0.0004937365010799136;
    for (distance, expected) in [
        (boundary - 1e-7, true),
        (boundary, true),
        (boundary + 1e-7, false),
    ] {
        let (mut w, _, _) = fixture(distance, 2, false, 0., false);
        until(&mut w, 3);
        assert_eq!(
            !events(&w, "7.5.3.same_type_interference").is_empty(),
            expected,
            "distance={distance}"
        );
    }
}
#[test]
fn source_different_actual_weapon_types_do_not_interfere() {
    let (mut w, ids, _) = fixture(0.5, 2, true, 0., false);
    assert_ne!(
        w.entity(ids[0]).unwrap().class,
        w.entity(ids[1]).unwrap().class
    );
    until(&mut w, 30);
    assert!(events(&w, "7.5.3.same_type_interference").is_empty());
}
#[test]
fn source_mutual_lock_waits_for_actual_active_enable_point() {
    let (mut w, _, _) = fixture(0.5, 2, false, 1., false);
    until(&mut w, 20);
    assert!(events(&w, "7.5.3.same_type_interference").is_empty());
    until(&mut w, 100);
    assert_eq!(events(&w, "7.5.3.same_type_interference").len(), 2);
}
#[test]
fn source_three_competitors_preserve_pending_without_id_pairing_or_damage() {
    let (mut w, ids, target) = fixture(0.3, 3, false, 0., false);
    let hp = w.entity(target).unwrap().hp;
    until(&mut w, 5);
    assert!(!events(&w, "7.5.3.interference_pending").is_empty());
    assert!(events(&w, "7.5.3.same_type_interference").is_empty());
    for id in ids {
        assert!(w.entity(id).unwrap().alive);
        assert!(w.rules.pending_adjudications.contains_key(&id));
    }
    assert_eq!(w.entity(target).unwrap().hp, hp);
}
#[test]
fn source_different_depths_do_not_teleport_or_guess_a_vertical_mutual_attack() {
    let (mut w, ids, _) = fixture(0.5, 2, false, 0., true);
    until(&mut w, 5);
    assert!(!events(&w, "7.5.3.interference_pending").is_empty());
    assert_eq!(w.entity(ids[0]).unwrap().depth_m, 50.);
    assert_eq!(w.entity(ids[1]).unwrap().depth_m, 100.);
    assert!(events(&w, "10.2.5.source_terminal").is_empty());
}
#[test]
fn source_real_lock_and_pending_are_referee_only_and_do_not_give_free_contacts() {
    let (mut w, _, _) = fixture(0.5, 2, false, 0., false);
    actual_lock(&mut w);
    until(&mut w, 100);
    for rule_name in ["7.5.3.same_type_interference", "7.5.3.interference_pending"] {
        let real: Vec<_> = w
            .events
            .iter()
            .filter(|e| matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule==rule_name))
            .collect();
        assert!(!real.is_empty(), "privacy must inspect an actual event");
        assert!(real
            .iter()
            .all(|e| e.vis == harpoon_core::Visibility::Referee));
        for side in [Side::Blue, Side::Red] {
            assert!(
                !serde_json::to_string(&harpoon_core::observe(&w, side, 0, 10000))
                    .unwrap()
                    .contains(rule_name)
            );
        }
    }
    assert!(w.sides.iter().all(|s| s
        .contacts
        .values()
        .all(|c| c.category != harpoon_core::types::PlatformKind::Torpedo)));
}
#[test]
fn source_full_world_restore_preserves_approach_and_pending_with_no_extra_lock() {
    let (base, _, _) = fixture(0.5, 2, false, 0., false);
    for checkpoint in [1, 3, 10, 100] {
        let mut a = base.clone();
        until(&mut a, checkpoint);
        let mut b: World = serde_json::from_slice(&serde_json::to_vec(&a).unwrap()).unwrap();
        b.rules.validate_tables().unwrap();
        for _ in 0..15 {
            harpoon_core::sim::tick(&mut a, &[]);
            harpoon_core::sim::tick(&mut b, &[]);
            assert_eq!(
                serde_json::to_value(&a).unwrap(),
                serde_json::to_value(&b).unwrap()
            );
        }
        assert_eq!(events(&b, "7.5.3.same_type_interference").len(), 2);
    }
}

#[test]
fn source_mutual_epoch_cannot_overspend_the_real_remaining_endurance_to_reach_a_partner() {
    let (mut w, ids, _) = fixture_options(0.5, 2, false, 0., false, Some(0.04), false);
    until(&mut w, 10);
    assert_eq!(events(&w, "7.5.3.same_type_interference").len(), 2);
    assert!(
        events(&w, "7.5.3.interference_pending").is_empty(),
        "fuel must run out before the distant physical meeting"
    );
    for id in ids {
        let flight = &w.rules.torpedoes.as_ref().unwrap().flights[&id];
        assert!(
            flight.fuel <= 1. + 1e-12,
            "a synchronized pair cannot invent free fuel or overrun: {}",
            flight.fuel
        );
        assert!(
            flight.run_nm <= 0.04 + 1e-12,
            "travel cannot exceed the explicit laboratory endurance"
        );
        assert!(!w.entity(id).is_some_and(|e| e.alive));
    }
}

#[test]
fn source_newly_enabled_third_weapon_cannot_be_hidden_by_an_existing_pair() {
    let (mut w, ids, _) = fixture_options(2.4, 3, false, 0., false, None, true);
    until(&mut w, 3);
    assert_eq!(events(&w, "7.5.3.same_type_interference").len(), 2);
    until(&mut w, 65);
    let pending = events(&w, "7.5.3.interference_pending");
    assert!(
        pending.iter().any(|p| p["members"]
            .as_array()
            .is_some_and(|members| members.len() == 3)),
        "late active contender must preserve the whole competing group"
    );
    for id in ids {
        assert!(w.entity(id).unwrap().alive);
        assert!(w.rules.pending_adjudications.contains_key(&id));
    }
}

#[test]
fn source_existing_native_adjudication_is_not_overridden_by_mutual_homing() {
    let (mut w, ids, _) = fixture_setup(0.5, 2, false, 0., false, None, false, |value| {
        value["context"]["torpedo_2026"]["targets"] = json!({});
    });
    assert!(
        ids.iter()
            .all(|id| w.rules.pending_adjudications.contains_key(id)),
        "actual missing source target must create native pending"
    );
    let poses: Vec<_> = ids.iter().map(|id| w.entity(*id).unwrap().pos).collect();
    until(&mut w, 6);
    assert!(
        events(&w, "7.5.3.same_type_interference").is_empty(),
        "a new private seeker cannot override an unresolved original adjudication"
    );
    for (index, id) in ids.iter().enumerate() {
        assert_eq!(w.entity(*id).unwrap().pos, poses[index]);
    }
}

//! English2026 PDF79/printed7-4 §7.5.3: same-type active homers within
//! 5000yd home on each other. No source provides mutual destruction at meeting.
//! Original four coverage purposes are retained with genuine native launches.
#[path = "support/torpedo_interference_fixture.rs"]
mod fixture_support;
use fixture_support::{actual_lock, events, fixture, until};
use harpoon_core::{EventKind, Side, Visibility, World};

/// I1 source correction: actual reciprocal pursuit ends in alive pending,
/// preserving the original target and both weapons rather than inventing DP.
#[test]
fn i1_close_same_type_torpedoes_home_and_preserve_unknown_terminal() {
    let (mut world, ids, target) = fixture(0.5, 2, false, 0., false);
    let original_hp = world.entity(target).unwrap().hp;
    actual_lock(&mut world);
    let initial_gap = world.entity(ids[0]).unwrap().pos.range_to(world.entity(ids[1]).unwrap().pos);
    until(&mut world, 6);
    assert!(world.entity(ids[0]).unwrap().pos.range_to(world.entity(ids[1]).unwrap().pos) < initial_gap);
    until(&mut world, 100);
    assert_eq!(events(&world, "7.5.3.same_type_interference").len(), 2);
    assert!(!events(&world, "7.5.3.interference_pending").is_empty());
    for id in ids {
        assert!(world.entity(id).unwrap().alive);
        assert!(world.rules.pending_adjudications.contains_key(&id));
        assert!(world.rules.torpedoes.as_ref().unwrap().flights.contains_key(&id));
    }
    assert_eq!(world.entity(target).unwrap().hp, original_hp);
    assert!(events(&world, "7.5.3.same_type_collision").is_empty());
    assert!(events(&world, "10.2.5.source_terminal").is_empty());
}

/// I2 now really uses two identical weapons, separated beyond 5000yd.
#[test]
fn i2_distant_same_type_torpedoes_do_not_interfere() {
    let (mut world, ids, _) = fixture(3., 2, false, 0., false);
    assert_eq!(world.entity(ids[0]).unwrap().class, world.entity(ids[1]).unwrap().class);
    let before = ids.iter().map(|id| world.entity(*id).unwrap().pos).collect::<Vec<_>>();
    until(&mut world, 30);
    assert!(events(&world, "7.5.3.same_type_interference").is_empty());
    assert!(events(&world, "7.5.3.interference_pending").is_empty());
    for (id, position) in ids.iter().zip(before) {
        assert!(world.entity(*id).unwrap().alive);
        assert!(world.entity(*id).unwrap().pos.range_to(position) > 0.);
    }
}

/// I4 inspects nonempty actual lock and pending events for both sides.
#[test]
fn i4_interference_traces_remain_referee_private() {
    let (mut world, _, _) = fixture(0.5, 2, false, 0., false);
    actual_lock(&mut world);
    until(&mut world, 100);
    for wanted in ["7.5.3.same_type_interference", "7.5.3.interference_pending"] {
        let actual: Vec<_> = world.events.iter().filter(|event| {
            matches!(&event.kind, EventKind::RulesEvent {rule, ..} if rule == wanted)
        }).collect();
        assert!(!actual.is_empty(), "privacy must inspect real {wanted} events");
        assert!(actual.iter().all(|event| event.vis == Visibility::Referee));
        for side in [Side::Blue, Side::Red] {
            assert!(!serde_json::to_string(&harpoon_core::observe(&world, side, 0, 10000)).unwrap().contains(wanted));
        }
    }
    assert!(world.sides.iter().all(|side| side.contacts.values().all(|c| c.category != harpoon_core::types::PlatformKind::Torpedo)));
}

/// I5 preserves the complete private guidance state across a real lock.
#[test]
fn i5_restore_after_lock_reproduces_terminal_state() {
    let (mut reference, ids, _) = fixture(0.5, 2, false, 0., false);
    actual_lock(&mut reference);
    let mut branch: World = serde_json::from_slice(&serde_json::to_vec(&reference).unwrap()).unwrap();
    branch.rules.validate_tables().unwrap();
    until(&mut reference, 130);
    until(&mut branch, 130);
    assert_eq!(serde_json::to_vec(&branch).unwrap(), serde_json::to_vec(&reference).unwrap());
    assert_eq!(events(&branch, "7.5.3.same_type_interference").len(), 2);
    assert!(!events(&branch, "7.5.3.interference_pending").is_empty());
    assert!(ids.iter().all(|id| branch.entity(*id).unwrap().alive));
}

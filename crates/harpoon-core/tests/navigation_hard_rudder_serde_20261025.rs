//! Persistence schema regressions; these do not inject gameplay state or prove risk dice.
use harpoon_core::rules::navigation2026::Unit;
use serde_json::{json, Value};
fn original() -> Value {
    serde_json::from_str(include_str!(
        "fixtures/navigation_unit_before_hard_rudder_20261024.json"
    ))
    .unwrap()
}
fn roundtrip(value: Value) -> Value {
    serde_json::to_value(serde_json::from_value::<Unit>(value).unwrap()).unwrap()
}
fn explicit_empty() -> Value {
    let mut unit = original();
    unit["hard_rudder"] =
        json!({"next_episode":0,"recovery_automatic":false,"recovery_armed":false,"episodes":{}});
    unit
}
#[test]
fn old_navigation_unit_without_hard_field_keeps_its_original_serialized_shape() {
    let old = original();
    assert!(old.get("hard_rudder").is_none());
    assert_eq!(
        roundtrip(old.clone()),
        old,
        "default hard state must not corrupt pre-hard archive checksums"
    );
}
#[test]
fn archive_with_explicit_empty_hard_state_keeps_its_present_field() {
    let old = explicit_empty();
    assert_eq!(
        roundtrip(old.clone()),
        old,
        "omitting all pristine hard states would corrupt already explicit empty hard archives"
    );
}
#[test]
fn newly_authorized_recovery_from_old_absent_state_is_serialized() {
    let mut unit: Unit = serde_json::from_value(original()).unwrap();
    unit.hard_rudder.recovery_automatic = true;
    let saved = serde_json::to_value(unit).unwrap();
    assert_eq!(saved["hard_rudder"]["recovery_automatic"], true);
    assert_eq!(roundtrip(saved.clone()), saved);
}
#[test]
fn empty_episode_map_cannot_hide_plans_authority_clocks_or_pending_state() {
    for (key, value) in [
        ("planned", json!({"course_deg":90.,"direction":"starboard"})),
        ("recovery_armed", json!(true)),
        ("last_resolution", json!(180)),
        ("pending", json!("source decision retained")),
        ("next_episode", json!(1)),
    ] {
        let mut unit: Unit = serde_json::from_value(original()).unwrap();
        let mut hard = serde_json::to_value(unit.hard_rudder).unwrap();
        hard[key] = value.clone();
        unit.hard_rudder = serde_json::from_value(hard).unwrap();
        let saved = serde_json::to_value(unit).unwrap();
        assert_eq!(
            saved["hard_rudder"][key], value,
            "{key} must survive without an episode"
        );
        assert_eq!(roundtrip(saved.clone()), saved);
    }
}

#[test]
fn authorized_save_then_reset_keeps_identical_presence_on_both_branches() {
    let mut native: Unit = serde_json::from_value(original()).unwrap();
    native.hard_rudder.recovery_automatic = true;
    let mut restored: Unit =
        serde_json::from_value(serde_json::to_value(&native).unwrap()).unwrap();
    native.hard_rudder.recovery_automatic = false;
    restored.hard_rudder.recovery_automatic = false;
    let a = serde_json::to_value(native).unwrap();
    let b = serde_json::to_value(restored).unwrap();
    assert!(
        a.get("hard_rudder").is_some(),
        "an already materialized field cannot disappear after reset"
    );
    assert_eq!(
        a, b,
        "reset after a save must not branch on historical field presence"
    );
}

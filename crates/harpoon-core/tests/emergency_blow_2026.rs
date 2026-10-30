//! English2026 PDF28/3-6: up one speed bracket, at least three bands,
//! 31+ unchanged, passive Loud, surface is still a separate operation.
use harpoon_core::rules::navigation2026::{self as nav, submarine};
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

fn fixture(depth: f64, speed: f64) -> (World, EntityId) {
    let sc: Scenario = serde_json::from_value(json!({
        "id":"emergency-blow-source","name":"Author geometry, source depth procedure","duration_s":5000,"seed":20261024,
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "navigation_2026":{"source":"scenario_assumption: explicit mechanism hull/water, not complete hardware evidence","units":{
                "OWN":{"source":"scenario_assumption: source movement mechanism fit","size":"C","merchant":false,
                    "passenger_liner":false,"cpp":false,"waterjet":false,"undamaged_max_kts":34,
                    "propulsion":"Nuclear","readiness":"Underway","submarine":{
                        "source":"scenario_assumption: controlled max/surface speed/water","maximum_depth_m":500,
                        "surface_max_kts":12,"water_depth_m":1000,"very_small":false}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"us_virginia","name":"OWN",
            "pos":[0,0],"course_deg":0,"speed_kts":speed,"depth_m":depth,"fire_on":false}]
    })).unwrap();
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    // Explicit author hull envelope for mechanism geometry, not a real hull fit.
    db.platforms.get_mut("us_virginia").unwrap().max_depth_m = 500.;
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let id = EntityId(1);
    w.entity_mut(id).unwrap().sensors.clear();
    (w, id)
}

fn command(id: EntityId, depth: f64) -> Command {
    serde_json::from_value(
        json!({"cmd":"navigation","unit":id,"action":{"type":"emergency_blow","depth_m":depth}}),
    )
    .unwrap()
}
fn advance(w: &mut World, n: usize) {
    for _ in 0..n {
        harpoon_core::sim::tick(w, &[]);
    }
}
fn start(w: &mut World, id: EntityId, depth: f64) {
    harpoon_core::sim::tick(w, &[(Side::Blue, command(id, depth))]);
}
fn operation(w: &World, id: EntityId) -> Value {
    serde_json::to_value(&w.rules.navigation.as_ref().unwrap().units[&id].depth_operation).unwrap()
}

#[test]
fn actual_twenty_knot_blow_uses_next_five_band_bracket_and_real_108_second_clock() {
    let (mut w, id) = fixture(300., 20.);
    assert!(submarine::validate_emergency_blow(&w, id, 50.).is_ok());
    start(&mut w, id, 50.);
    assert_eq!(operation(&w, id)["kind"], "EmergencyBlow");
    assert!(!w.rules.navigation.as_ref().unwrap().units[&id].quiet);
    assert!(w.entity(id).unwrap().depth_m < 300. && w.entity(id).unwrap().depth_m > 290.);
    advance(&mut w, 106);
    assert_eq!(w.tick, 107);
    assert!(w.entity(id).unwrap().depth_m > 50.);
    advance(&mut w, 1);
    assert!((w.entity(id).unwrap().depth_m - 50.).abs() < 1e-8);
    assert!(operation(&w, id).is_null());
    assert_eq!(
        w.entity(id).unwrap().depth_m,
        50.,
        "Blow cannot silently finish at P/S or surface"
    );
}

#[test]
fn thirty_and_thirty_one_knots_have_the_same_eight_band_blow_rate() {
    let (mut thirty, id) = fixture(300., 30.);
    let (mut thirty_one, _) = fixture(300., 31.);
    start(&mut thirty, id, 50.);
    start(&mut thirty_one, id, 50.);
    assert_eq!(operation(&thirty, id)["kind"], "EmergencyBlow");
    assert_eq!(operation(&thirty_one, id)["kind"], "EmergencyBlow");
    advance(&mut thirty, 66);
    advance(&mut thirty_one, 66);
    assert!(thirty.entity(id).unwrap().depth_m > 50.);
    assert!(
        (thirty.entity(id).unwrap().depth_m - thirty_one.entity(id).unwrap().depth_m).abs() < 1e-8
    );
    advance(&mut thirty, 1);
    advance(&mut thirty_one, 1);
    assert_eq!(thirty.entity(id).unwrap().depth_m, 50.);
    assert_eq!(thirty_one.entity(id).unwrap().depth_m, 50.);
}

#[test]
fn five_knot_blow_can_cross_tactical_turns_and_three_bands_is_total_not_per_turn() {
    let (mut w, id) = fixture(300., 5.);
    start(&mut w, id, 50.);
    advance(&mut w, 179);
    assert!((w.entity(id).unwrap().depth_m - 100.).abs() < 1e-8);
    assert_eq!(operation(&w, id)["kind"], "EmergencyBlow");
    advance(&mut w, 89);
    assert!(w.entity(id).unwrap().depth_m > 50.);
    advance(&mut w, 1);
    assert_eq!(w.entity(id).unwrap().depth_m, 50.);
}

#[test]
fn source_blow_invalid_targets_and_unknown_fits_have_no_readonly_side_effects() {
    let (w, id) = fixture(300., 20.);
    for target in [
        0.,
        20.,
        25.,
        100.,
        200.,
        300.,
        400.,
        f64::NAN,
        f64::INFINITY,
    ] {
        let before = serde_json::to_value(&w).unwrap();
        assert!(
            submarine::validate_emergency_blow(&w, id, target).is_err(),
            "target{target}"
        );
        assert_eq!(before, serde_json::to_value(&w).unwrap());
    }
    let (zero, id) = fixture(300., 0.);
    assert!(submarine::validate_emergency_blow(&zero, id, 50.).is_err());
    let mut unknown = w.clone();
    unknown
        .rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap()
        .fit
        .submarine = None;
    assert!(submarine::validate_emergency_blow(&unknown, id, 50.).is_err());
    let mut deep = w.clone();
    deep.entity_mut(id).unwrap().depth_m = 1201.;
    assert!(submarine::validate_emergency_blow(&deep, id, 50.).is_err());
}

#[test]
fn repeated_order_does_not_restart_work_and_normal_order_cannot_cancel_three_band_commitment() {
    let (mut w, id) = fixture(300., 20.);
    start(&mut w, id, 50.);
    advance(&mut w, 39);
    let before = operation(&w, id);
    assert_eq!(before["kind"], "EmergencyBlow");
    let before_depth = w.entity(id).unwrap().depth_m;
    nav::apply(
        &mut w,
        id,
        &serde_json::from_value(json!({"type":"emergency_blow","depth_m":50})).unwrap(),
    )
    .unwrap();
    assert_eq!(before, operation(&w, id));
    assert!(submarine::order_depth(&mut w, id, before_depth).is_err());
    assert_eq!(before, operation(&w, id));
    assert_eq!(before_depth, w.entity(id).unwrap().depth_m);
}

#[test]
fn own_declared_blow_and_continuous_work_survive_complete_world_save_restore() {
    let (mut a, id) = fixture(300., 5.);
    start(&mut a, id, 50.);
    advance(&mut a, 190);
    assert_eq!(operation(&a, id)["kind"], "EmergencyBlow");
    let own = harpoon_core::observe(&a, Side::Blue, 0, 1000);
    assert!(serde_json::to_string(&own.units[0].navigation)
        .unwrap()
        .contains("EmergencyBlow"));
    let mut b: World = serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
    advance(&mut a, 120);
    advance(&mut b, 120);
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    assert_eq!(a.entity(id).unwrap().depth_m, 50.);
}

#[test]
fn reaching_shallow_still_requires_a_separate_180_seconds_to_surface() {
    let (mut w, id) = fixture(300., 20.);
    start(&mut w, id, 50.);
    advance(&mut w, 107);
    assert_eq!(w.entity(id).unwrap().depth_m, 50.);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetDepth {
                unit: id,
                depth_m: 0.,
            },
        )],
    );
    advance(&mut w, 178);
    assert_eq!(w.entity(id).unwrap().depth_m, 50.);
    advance(&mut w, 1);
    assert_eq!(w.entity(id).unwrap().depth_m, 0.);
}

#[test]
fn actual_speed_command_changes_blow_bracket_without_restarting_its_work() {
    let (mut w, id) = fixture(300., 20.);
    start(&mut w, id, 50.);
    let started = operation(&w, id)["started_at"].clone();
    advance(&mut w, 9);
    let before = w.entity(id).unwrap().depth_m;
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: id,
                speed_kts: 21.,
            },
        )],
    );
    assert!(w.entity(id).unwrap().speed_kts > 20.);
    assert!((before - w.entity(id).unwrap().depth_m - 100. * 8. / 180.).abs() < 1e-8);
    assert_eq!(operation(&w, id)["started_at"], started);
    assert_eq!(operation(&w, id)["worked_seconds"], 11.);
}

#[test]
fn player_can_change_a_longer_blow_order_after_the_actual_three_band_minimum() {
    let (mut w, id) = fixture(500., 20.);
    start(&mut w, id, 50.);
    advance(&mut w, 106);
    assert!(submarine::order_depth(&mut w, id, 150.).is_err());
    advance(&mut w, 1);
    assert!((w.entity(id).unwrap().depth_m - 200.).abs() < 1e-8);
    submarine::order_depth(&mut w, id, 150.).unwrap();
    assert_eq!(operation(&w, id)["kind"], "Bands");
    assert_eq!(operation(&w, id)["target_m"], 150.);
}

#[test]
fn same_target_normal_depth_order_really_exits_blow_after_three_bands() {
    let (mut w, id) = fixture(500., 20.);
    start(&mut w, id, 50.);
    advance(&mut w, 107);
    submarine::order_depth(&mut w, id, 50.).unwrap();
    assert_eq!(
        operation(&w, id)["kind"],
        "Bands",
        "same target does not mean same navigation mode"
    );
    let before = w.entity(id).unwrap().depth_m;
    advance(&mut w, 1);
    assert!((before - w.entity(id).unwrap().depth_m - 100. * 3. / 180.).abs() < 1e-8);
}

#[test]
fn emergency_blow_cannot_silently_treat_a_forged_cancel_or_success_flag_as_an_ascend_order() {
    for (field, value) in [
        ("enabled", json!(false)),
        ("force_success", json!(true)),
        ("noise_bonus", json!(99)),
    ] {
        let mut raw =
            json!({"cmd":"navigation","unit":1,"action":{"type":"emergency_blow","depth_m":50}});
        raw["action"][field] = value;
        assert!(serde_json::from_value::<Command>(raw).is_err(),"unsupported {field} must reject before queue/physics; enabled:false is not a Blow cancellation API");
    }
}

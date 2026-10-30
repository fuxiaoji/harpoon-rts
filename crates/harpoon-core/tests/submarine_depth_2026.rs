use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::json;

fn fixture(depth: f64, speed: f64, water: f64, very_small: bool) -> (World, EntityId) {
    let sc: Scenario = serde_json::from_value(json!({
        "id":"depth-source", "name":"Source depth mechanism fixture", "seed":71,
        "duration_s":20000,
        "context": {"rules_profile":"hv2026-rts-v4", "radar_environment":{"sea_state":0,"precipitation":"None"},
            "navigation_2026":{"source":"scenario_assumption: controlled depth mechanism, no real hull fit claim", "units":{
                "Sub":{"source":"scenario_assumption: explicit size/speed fit", "size":"C", "merchant":false,
                    "passenger_liner":false,"cpp":false,"waterjet":false,"undamaged_max_kts":34,
                    "propulsion":"Nuclear","readiness":"Underway",
                    "submarine":{"source":"scenario_assumption: explicit test depth/surface speed/water", "maximum_depth_m":1200,
                        "surface_max_kts":12,"water_depth_m":water,"very_small":very_small}}
            }}},
        "weather":{"visibility_nm":40,"jam":[0,0]},
        "units":[{"side":"Blue","class":"fixture_sub_depth_hull","name":"Sub","pos":[0,0],"course_deg":90,
            "speed_kts":speed,"depth_m":depth}]
    })).unwrap();
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    // Independent fictional test ID: verified real Virginia source limits must
    // remain binding on their stable ID, including this author mechanism test.
    let mut hull = db.platforms["us_virginia"].clone();
    hull.id = "fixture_sub_depth_hull".into();
    hull.name = "Scenario assumption: fictional depth mechanism hull".into();
    hull.max_depth_m = 1200.;
    db.platforms.insert("fixture_sub_depth_hull".into(), hull);
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let id = *w.entities.keys().next().unwrap();
    w.entity_mut(id).unwrap().sensors.clear();
    w.entity_mut(id).unwrap().fire_on = false;
    (w, id)
}
fn tick(w: &mut World, n: usize) {
    for _ in 0..n {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn order(w: &mut World, id: EntityId, depth: f64) {
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::SetDepth {
                unit: id,
                depth_m: depth,
            },
        )],
    );
}

#[test]
fn source_speed_bands_change_real_depth_in_one_tactical_turn() {
    // PDF28: one/two/three/five/eight bands per180s. Fifty->100 is one band,
    // not a fixed3m/s. Endpoints are independent printed-source expectations.
    for (speed, end) in [
        (5., 100.),
        (10., 200.),
        (20., 300.),
        (25., 500.),
        (30., 500.),
        (31., 900.),
    ] {
        let (mut w, id) = fixture(50., speed, 2000., false);
        order(&mut w, id, 1200.);
        tick(&mut w, 179);
        assert!(
            (w.entity(id).unwrap().depth_m - end).abs() < 1e-8,
            "speed{speed}: {:?}",
            w.entity(id).unwrap().depth_m
        );
        assert!(
            (w.entity(id).unwrap().speed_kts - speed).abs() < 1e-8,
            "verified depth fit must not inherit the generic60m/0.6 speed penalty"
        );
        assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{domain,rule,..} if domain=="navigation2026" && rule=="3.2.1.depth_started")));
    }
}
#[test]
fn source_submerging_holds_surface_target_and_surface_speed_for180_seconds() {
    let (mut w, id) = fixture(0., 12., 2000., false);
    w.entity_mut(id).unwrap().ordered_speed_kts = 30.;
    order(&mut w, id, 50.);
    tick(&mut w, 178);
    assert_eq!(w.entity(id).unwrap().depth_m, 0.);
    assert!(w.entity(id).unwrap().speed_kts <= 12.);
    tick(&mut w, 1);
    assert_eq!(w.entity(id).unwrap().depth_m, 50.);
    // Completion is180s after first real work tick; maximum submerged speed
    // becomes available to the next movement step.
    tick(&mut w, 1);
    assert!(w.entity(id).unwrap().speed_kts > 12.);
}
#[test]
fn source_periscope_shift_uses_one_full_turn_even_at_high_speed() {
    let (mut w, id) = fixture(50., 25., 2000., false);
    order(&mut w, id, 25.);
    tick(&mut w, 178);
    assert_eq!(w.entity(id).unwrap().depth_m, 50.);
    tick(&mut w, 1);
    assert_eq!(w.entity(id).unwrap().depth_m, 25.);
    order(&mut w, id, 50.);
    tick(&mut w, 178);
    assert_eq!(w.entity(id).unwrap().depth_m, 25.);
    tick(&mut w, 1);
    assert_eq!(w.entity(id).unwrap().depth_m, 50.);
}
#[test]
fn source_dive_admission_rejects_shoal_and_skipping_surface_operation_without_motion() {
    for (water, small, depth) in [
        (34.999, false, 25.),
        (14.999, true, 10.),
        (2000., false, 100.),
    ] {
        let (mut w, id) = fixture(0., 5., water, small);
        order(&mut w, id, depth);
        tick(&mut w, 190);
        assert_eq!(w.entity(id).unwrap().depth_m, 0.);
        assert_eq!(w.entity(id).unwrap().ordered_depth_m, 0.);
        assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{domain,rule,detail,..} if domain=="command" && rule=="rejected" && detail.contains("set_depth"))));
    }
    for (water, small) in [(35., false), (15., true)] {
        let (mut w, id) = fixture(0., 5., water, small);
        order(&mut w, id, 10.);
        tick(&mut w, 179);
        assert_eq!(w.entity(id).unwrap().depth_m, 10.);
    }
}
#[test]
fn source_surface_is_separate_operation_and_deep_order_cannot_skip_shallow() {
    let (mut w, id) = fixture(200., 10., 2000., false);
    order(&mut w, id, 0.);
    tick(&mut w, 190);
    assert_eq!(w.entity(id).unwrap().depth_m, 200.);
    let (mut w, id) = fixture(50., 10., 2000., false);
    order(&mut w, id, 0.);
    tick(&mut w, 178);
    assert_eq!(w.entity(id).unwrap().depth_m, 50.);
    tick(&mut w, 1);
    assert_eq!(w.entity(id).unwrap().depth_m, 0.);
}
#[test]
fn source_depth_mid_band_save_repeated_order_does_not_restart_and_enemy_has_no_clock() {
    let (mut w, id) = fixture(50., 5., 2000., false);
    order(&mut w, id, 200.);
    tick(&mut w, 66);
    let blue = serde_json::to_value(harpoon_core::observe(&w, Side::Blue, 0, 0)).unwrap();
    assert!(blue.to_string().contains("depth_operation"));
    let red = serde_json::to_value(harpoon_core::observe(&w, Side::Red, 0, 0)).unwrap();
    assert!(!red.to_string().contains("depth_operation"));
    let mut saved: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    saved.rules.validate_tables().unwrap();
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetDepth {
                unit: id,
                depth_m: 200.,
            },
        )],
    );
    harpoon_core::sim::tick(
        &mut saved,
        &[(
            Side::Blue,
            Command::SetDepth {
                unit: id,
                depth_m: 200.,
            },
        )],
    );
    tick(&mut w, 292);
    tick(&mut saved, 292);
    assert!((w.entity(id).unwrap().depth_m - 200.).abs() < 1e-8);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}
#[test]
fn source_depth_zero_speed_and_unverified_deep_orders_do_not_use_generic_three_mps() {
    let (mut w, id) = fixture(50., 0., 2000., false);
    order(&mut w, id, 100.);
    tick(&mut w, 180);
    assert_eq!(w.entity(id).unwrap().depth_m, 50.);
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..} if rule=="3.2.1.depth_blocked")));
    let (mut w, id) = fixture(50., 5., 2000., false);
    order(&mut w, id, 1201.);
    tick(&mut w, 180);
    assert_eq!(w.entity(id).unwrap().depth_m, 50.);
}

#[test]
fn source_normal_ascent_uses_band_rate_then_separate_ps_shift() {
    let (mut w, id) = fixture(500., 25., 2000., false);
    order(&mut w, id, 50.);
    tick(&mut w, 179);
    assert!((w.entity(id).unwrap().depth_m - 50.).abs() < 1e-8);
    order(&mut w, id, 25.);
    tick(&mut w, 178);
    assert!((w.entity(id).unwrap().depth_m - 50.).abs() < 1e-8);
    tick(&mut w, 1);
    assert_eq!(w.entity(id).unwrap().depth_m, 25.);
}

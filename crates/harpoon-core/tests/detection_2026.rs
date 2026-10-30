use harpoon_core::rules::detection2026 as d;
use harpoon_core::types::PlatformKind;
use harpoon_core::{Database, EntityId, Scenario};
use serde_json::json;
fn env() -> d::Environment {
    d::Environment {
        source: "scenario explicit test conditions".into(),
        sea_state: 3,
        precipitation: 0,
        visibility_percent: 60,
        shipping: 0,
        biologics: 0,
        water_depth_m: 3000.0,
        layer: Some(1),
        ice: 0,
        cloud: None,
    }
}
#[test]
fn printed_table_shapes_and_independent_cells() {
    let t = d::tables();
    assert_eq!(
        t["sonar"]["range_probability_rows"]
            .as_array()
            .unwrap()
            .len(),
        75
    );
    assert_eq!(t["visual"]["surface"].as_array().unwrap().len(), 12);
    assert_eq!(t["visual"]["detection_rows"].as_array().unwrap().len(), 71);
    assert_eq!(t["sonar"]["active_strength"][1], 1); // Table, not incorrect5-13B/Medium+2 example.
    assert_eq!(t["visual"]["surface"][0][4][0], 29.0); // asymmetric E-size wake visibility
    assert_eq!(t["visual"]["surface"][0][0][4], 28.0);
}
#[test]
fn group_signature_uplift_and_external_store_floor() {
    assert_eq!(d::group_signature(4, false, 2).unwrap(), 4);
    assert_eq!(d::group_signature(4, false, 3).unwrap(), 3);
    assert_eq!(d::group_signature(4, false, 9).unwrap(), 2);
    assert_eq!(d::group_signature(4, true, 9).unwrap(), 0);
    assert_eq!(d::group_signature(0, false, 100).unwrap(), 0);
    assert!(d::group_signature(5, false, 1).is_err());
    assert!(d::group_signature(1, false, 0).is_err());
}
#[test]
fn mast_fleeting_continuous_and_sea_boundaries() {
    assert_eq!(d::radar_mast_probability(4, false, 179).unwrap(), 25);
    assert_eq!(d::radar_mast_probability(4, true, 179).unwrap(), 50);
    assert_eq!(d::radar_mast_probability(4, false, 180).unwrap(), 100);
    assert_eq!(d::radar_mast_probability(5, true, 180).unwrap(), 0);
    assert_eq!(d::periscope_visual_probability(6, 4.9, false).unwrap(), 5.0);
    assert_eq!(d::periscope_visual_probability(6, 5.0, true).unwrap(), 0.0);
    assert_eq!(d::periscope_visual_probability(7, 10.0, true).unwrap(), 0.0);
}
#[test]
fn radar_recognition_capabilities_do_not_imply_omniscience() {
    assert!(!d::radar_identifies(
        false,
        false,
        false,
        PlatformKind::Ship
    ));
    assert!(d::radar_identifies(false, true, false, PlatformKind::Ship));
    assert!(!d::radar_identifies(
        false,
        true,
        false,
        PlatformKind::Aircraft
    ));
    assert!(d::radar_identifies(
        false,
        false,
        true,
        PlatformKind::Aircraft
    ));
}
#[test]
fn source_ducts_and_es_examples() {
    assert_eq!(d::duct_horizon(false, 2, 1).unwrap(), 55.0);
    assert_eq!(d::duct_horizon(true, 2, 1).unwrap(), 115.0);
    assert_eq!(d::evaporative_max_size(71, 1, 11.0).unwrap(), 2);
    assert_eq!(d::evaporative_max_size(100, 0, 30.0).unwrap(), 0);
    assert!(d::evaporative_max_size(100, 0, 30.1).is_err());
    assert_eq!(d::surface_duct_chance(0, true, 1, 5.0, 25).unwrap(), 40);
    assert_eq!(d::surface_duct_chance(0, true, 1, 5.1, 25).unwrap(), 0);
    assert_eq!(
        d::es_range(21.0, 3, true, "AS", 150.0, false).unwrap(),
        42.0
    );
    assert_eq!(
        d::es_range(55.0, 3, true, "AS", 150.0, false).unwrap(),
        110.0
    );
    assert!((d::es_range(100.0, 3, false, "AS", 150.0, true).unwrap() - 30.0).abs() < 1e-9);
}
#[test]
fn all_speed_matrix_cells_and_blind_column() {
    for active in [false, true] {
        for (ti, t) in [0., 9., 16., 21., 26., 31.].iter().enumerate() {
            for (oi, o) in [0., 9., 16., 21., 26., 31.].iter().enumerate() {
                assert_eq!(
                    d::sonar_speed_modifier(active, *t, *o, false).unwrap(),
                    d::tables()["sonar"][if active {
                        "active_speed"
                    } else {
                        "passive_speed"
                    }][ti][oi]
                        .as_i64()
                        .map(|v| v as i32)
                );
            }
        }
    }
    assert_eq!(
        d::sonar_speed_modifier(false, 8.0, 15.0, true).unwrap(),
        Some(0)
    );
}
#[test]
fn sonar_printed_rows_round_nearest_and_range_up() {
    assert_eq!(d::sonar_probability(1.65, 4.2, false).unwrap(), 0.0);
    assert_eq!(d::sonar_probability(2.7, 4.2, false).unwrap(), 15.0);
    assert_eq!(d::sonar_probability(2.9, 5.2, false).unwrap(), 10.0);
    assert_eq!(d::sonar_probability(4.3, 5.2, false).unwrap(), 40.0);
    assert_eq!(d::sonar_probability(3.0, 4.0, true).unwrap(), 45.0);
    assert_eq!(d::sonar_probability(3.0, 0.0, true).unwrap(), 90.0);
    assert!(d::sonar_probability(2.75, 4.2, false).is_err());
    assert!(d::sonar_range_modifier(-11).is_err());
    assert_eq!(d::sonar_range_modifier(11).unwrap(), 10.0);
}
#[test]
fn layer_frequency_exemptions_and_actual_array_depth() {
    use d::Frequency::*;
    assert_eq!(
        d::layer_modifier(Lmf, false, 0.0, 90.0, Some(1)).unwrap(),
        -5
    );
    assert_eq!(
        d::layer_modifier(Lmf, false, 80.0, 90.0, Some(1)).unwrap(),
        -2
    );
    assert_eq!(d::layer_modifier(Lf, true, 80.0, 90.0, Some(1)).unwrap(), 0);
    assert_eq!(
        d::layer_modifier(Vlf, true, 80.0, 10.0, Some(1)).unwrap(),
        0
    );
    assert_eq!(
        d::layer_modifier(Vlf, true, 180.0, 10.0, Some(2)).unwrap(),
        -2
    );
    assert_eq!(
        d::layer_modifier(Vlf, true, 180.0, 150.0, Some(2)).unwrap(),
        0
    );
}
#[test]
fn noisiest_environment_is_not_sum_of_all_noise() {
    let mut e = env();
    e.sea_state = 4;
    e.shipping = 1;
    e.precipitation = 3;
    e.biologics = 3;
    assert_eq!(
        d::sonar_environment(false, d::Frequency::Lmf, &e).unwrap(),
        -4
    ); // max(sea+shipping=-2,rain=-4,bio=-3)
    e.precipitation = 0;
    e.biologics = 0;
    e.sea_state = 0;
    e.shipping = 0;
    assert_eq!(
        d::sonar_environment(false, d::Frequency::Lmf, &e).unwrap(),
        1
    );
    e.ice = 2;
    e.shipping = 3;
    e.precipitation = 5;
    assert_eq!(
        d::sonar_environment(false, d::Frequency::Lmf, &e).unwrap(),
        3
    );
}
#[test]
fn cavitation_and_frequency_of_active_emitter() {
    assert!(d::cavitating(2, 16.0, 50.0, false).unwrap());
    assert!(!d::cavitating(2, 16.0, 51.0, false).unwrap());
    assert!(!d::cavitating(0, 40.0, 0.0, true).unwrap());
    assert!(d::hears_active(d::Frequency::Lmf, d::Frequency::Mf));
    assert!(!d::hears_active(d::Frequency::Lf, d::Frequency::Mf));
}
#[test]
fn all_baffle_mounts_and_flank_generation() {
    use d::SonarMount::*;
    assert!(!d::sonar_bearing_eligible(Bow, 6, 180.0));
    assert!(d::sonar_bearing_eligible(Bow, 6, 149.9));
    assert!(!d::sonar_bearing_eligible(Keel, 6, 135.0));
    assert!(!d::sonar_bearing_eligible(Flank, 3, 90.0));
    assert!(d::sonar_bearing_eligible(Flank, 4, 90.0));
    assert!(d::sonar_bearing_eligible(Towed, 1, 180.0));
}
#[test]
fn bottom_bounce_and_cz_source_examples() {
    use d::Frequency::*;
    assert_eq!(
        d::bottom_bounce_interval(2800.0, true, Lmf).unwrap(),
        Some((5.0, 10.0))
    );
    assert_eq!(d::bottom_bounce_interval(2800.0, true, Mf).unwrap(), None);
    assert_eq!(d::cz_count(false, 4, Lmf, 1, 10.0, 12.0, false).unwrap(), 1);
    assert_eq!(d::cz_count(false, 4, Vlf, 1, 10.0, 12.0, false).unwrap(), 2);
    assert_eq!(d::cz_count(false, 4, Mf, 1, 10.0, 12.0, false).unwrap(), 0);
    assert_eq!(
        d::cz_interval(2, false, false, 2100.0).unwrap(),
        Some((57.0, 63.0))
    );
    assert_eq!(
        d::propagation_probability(1, 5, Some(1), false).unwrap(),
        65.0
    );
    assert_eq!(
        d::propagation_probability(1, 5, Some(2), false).unwrap(),
        55.0
    );
}
#[test]
fn acoustic_intercept_information_bounds() {
    let x = d::acoustic_intercept(5, d::Frequency::Hf, 4.0, 12.0, true).unwrap();
    assert_eq!(x, (true, Some(3.0), true, true));
    assert_eq!(
        d::acoustic_intercept(5, d::Frequency::Hf, 4.0, 12.01, true)
            .unwrap()
            .3,
        false
    );
    assert_eq!(
        d::acoustic_intercept(5, d::Frequency::Hf, 4.0, 12.0, false)
            .unwrap()
            .1,
        None
    );
    assert!(d::acoustic_intercept(1, d::Frequency::Lf, 4.0, 2.0, true).is_err());
}
#[test]
fn classification_source_example_and_decoy_penalty() {
    use d::Frequency::*;
    assert_eq!(
        d::sonar_classification(60, 45.0, Vlf, false, false, None).unwrap(),
        3
    );
    assert_eq!(
        d::sonar_classification(61, 45.0, Vlf, false, false, None).unwrap(),
        2
    );
    assert_eq!(
        d::sonar_classification(71, 45.0, Vlf, false, false, None).unwrap(),
        1
    );
    assert_eq!(
        d::sonar_classification(81, 45.0, Vlf, false, false, None).unwrap(),
        0
    );
    assert_eq!(
        d::sonar_classification(40, 45.0, Vlf, false, false, Some(3)).unwrap(),
        4
    );
    assert_eq!(
        d::sonar_classification(41, 45.0, Vlf, false, false, Some(3)).unwrap(),
        3
    );
    assert_eq!(
        d::sonar_classification(1, 45.0, Mf, false, false, None).unwrap(),
        1
    );
}
#[test]
fn passive_masking_uses_received_probability_and_path() {
    assert!(d::passive_masked(0., 40., 5., 4.9, 50., 10., false, true));
    assert!(!d::passive_masked(0., 40., 5., 5.1, 50., 10., false, true));
    assert!(!d::passive_masked(0., 40., 5., 4., 50., 10., true, true));
    assert!(!d::passive_masked(0., 40., 5., 4., 50., 10., false, false));
    assert!(d::passive_masked(0., 40., 5., 4., 40., 4., false, true));
}
#[test]
fn ir_source_ranges_moisture_and_laser() {
    for g in 1..=5 {
        for c in 0..=2 {
            for rain in 0..=5 {
                assert_eq!(
                    d::ir_range(g, c, rain).unwrap(),
                    d::tables()["ir"]["ranges_nm"][g - 1][c].as_f64().unwrap()
                        * d::tables()["ir"]["precipitation_multiplier"][rain]
                            .as_f64()
                            .unwrap()
                );
            }
        }
    }
    assert_eq!(d::laser_range(true, None, 0).unwrap(), 10.);
    assert_eq!(d::laser_range(false, None, 3).unwrap(), 2.5);
}
#[test]
fn mad_and_wake_book_examples() {
    assert_eq!(
        d::mad_probability(2, false, 4, false, false, true).unwrap(),
        40.0
    );
    assert_eq!(
        d::mad_probability(2, false, 4, false, false, false).unwrap(),
        20.0
    );
    assert_eq!(
        d::mad_probability(0, true, 4, false, false, true).unwrap(),
        30.0
    );
    assert_eq!(
        d::mad_probability(0, false, 7, false, false, true).unwrap(),
        0.0
    );
    assert_eq!(d::wide_mad_probability(900.0, 4, 10).unwrap(), 8.0);
    assert_eq!(d::wide_mad_probability(900.0, 4, 5).unwrap(), 4.0);
    assert_eq!(
        d::wake_probability(1, 10., 8., 8., 3, 2, true, false).unwrap(),
        25.0
    );
    assert_eq!(
        d::wake_probability(1, 10., 8., 16., 3, 2, true, false).unwrap(),
        0.0
    );
}
#[test]
fn visual_tables_book_example_round_down_and_priority() {
    let m = d::VisualModifiers::default();
    assert_eq!(
        d::visual_base(60, 0, 1, false, false, false, &m).unwrap(),
        23.0
    );
    assert_eq!(d::visual_detection_range(23., 58, 0).unwrap(), 17.0);
    assert_eq!(d::visual_detection_range(31.8, 16, 0).unwrap(), 29.5);
    assert_eq!(d::land_visibility(16., 100, 1, false, false).unwrap(), 8.0);
    assert_eq!(d::land_visibility(16., 100, 2, false, false).unwrap(), 4.0);
    assert_eq!(d::land_visibility(16., 100, 3, false, false).unwrap(), 0.0);
    assert_eq!(d::land_visibility(16., 100, 3, true, false).unwrap(), 4.0);
    assert!(d::visual_detection_range(60., 10, 0).is_err());
}
#[test]
fn reciprocal_cloud_gap_is_one_shared_roll() {
    let mut s = d::State::default();
    let c = d::Cloud {
        base_m: 1000.,
        top_m: 3000.,
        coverage_percent: 75,
    };
    let a = d::cloud_effect(&mut s, 9, 1, EntityId(1), EntityId(2), 0., 5000., Some(&c)).unwrap();
    let b = d::cloud_effect(
        &mut s,
        9,
        100,
        EntityId(2),
        EntityId(1),
        5000.,
        0.,
        Some(&c),
    )
    .unwrap();
    assert_eq!(a, b);
    assert_eq!(a.1, 0.1);
    let clear = d::cloud_effect(&mut s, 9, 1, EntityId(1), EntityId(2), 0., 0., Some(&c)).unwrap();
    assert_eq!(clear, (true, 1.0, None));
}
#[test]
fn buoy_lifecycle_and_localization_source_limits() {
    assert!(d::buoy_drop_valid(61., 325.));
    assert!(!d::buoy_drop_valid(60., 325.));
    assert!(!d::buoy_drop_valid(2000., 325.1));
    assert_eq!(d::buoy_monitor_range(50.).unwrap(), 40.0);
    assert_eq!(d::array_stabilization_seconds(true, 10.).unwrap(), 360);
    assert_eq!(d::buoy_fix(false, true, 2, "").unwrap(), "poor");
    assert_eq!(d::buoy_fix(false, true, 2, "eer").unwrap(), "fair");
    assert_eq!(d::buoy_fix(true, true, 2, "").unwrap(), "good");
}
#[test]
fn stable_window_rng_order_and_snapshot() {
    let mut a = d::State::default();
    let first = d::trial(&mut a, 17, 1, "mast:1:2", 180);
    assert!(!first.1);
    assert_eq!(d::trial(&mut a, 17, 179, "mast:1:2", 180), (first.0, true));
    let bytes = serde_json::to_vec(&a).unwrap();
    let mut b: d::State = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        d::trial(&mut a, 17, 180, "mast:1:2", 180),
        d::trial(&mut b, 17, 180, "mast:1:2", 180)
    );
    d::trial(&mut b, 17, 181, "irrelevant", 180);
    assert_eq!(
        d::trial(&mut a, 17, 360, "mast:1:2", 180),
        d::trial(&mut b, 17, 360, "mast:1:2", 180)
    );
}
fn fixture() -> harpoon_core::World {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc:Scenario=serde_json::from_value(json!({"id":"detection-v4","name":"detection","duration_s":10000,"seed":23,"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"}},"weather":{"visibility_nm":12.,"jam":[0.,0.]},"units":[{"side":"Blue","class":"us_ford","name":"observer","pos":[0.,0.],"speed_kts":10.},{"side":"Red","class":"cn_001","name":"hidden","pos":[2.,0.],"speed_kts":10.}]})).unwrap();
    harpoon_core::load_scenario(sc, db).unwrap()
}
#[test]
fn real_world_visual_adapter_has_no_legacy_estimated_range() {
    let w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let mut s = d::State::default();
    s.config.environment = Some(env());
    let sensor = w
        .entity(ids[0])
        .unwrap()
        .sensors
        .iter()
        .find(|s| w.db.sensors[&s.sensor].kind == harpoon_core::db::SensorKind::Visual)
        .unwrap()
        .sensor
        .clone();
    let result = d::evaluate_world(&w, &mut s, ids[0], &sensor, ids[1]).unwrap();
    assert!(result.detected);
    assert!(result.identify_class);
    assert!(result.trace.get("d100").is_some());
    let mut w = w;
    w.entity_mut(ids[1]).unwrap().class = "unknown".into();
    assert!(d::evaluate_world(&w, &mut s, ids[0], &sensor, ids[1]).is_err());
}
#[test]
fn real_world_unknown_esm_fit_blocks_without_contact() {
    let w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let mut s = d::State::default();
    s.config.environment = Some(env());
    let sensor = w
        .entity(ids[0])
        .unwrap()
        .sensors
        .iter()
        .find(|s| w.db.sensors[&s.sensor].kind == harpoon_core::db::SensorKind::Esm)
        .unwrap()
        .sensor
        .clone();
    assert!(d::evaluate_world(&w, &mut s, ids[0], &sensor, ids[1]).is_err());
    assert!(s.tracks.is_empty());
}

#[test]
fn sim_tick_v4_preserves_detection_state_and_hides_failed_scans() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    for e in w.entities.values_mut() {
        e.weapons.clear();
        e.fire_on = false;
        e.sensors
            .retain(|s| w.db.sensors[&s.sensor].kind == harpoon_core::db::SensorKind::Visual);
    }
    w.rules.detection.as_mut().unwrap().config.environment = Some(env());
    w.entity_mut(ids[1]).unwrap().pos = harpoon_core::Vec2::new(100.0, 0.0);
    harpoon_core::sim::tick(&mut w, &[]);
    assert!(w.sides[0].contacts.is_empty());
    assert!(w.sides[1].contacts.is_empty());
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{domain,..}if domain=="detection2026")));
    assert!(w.events.iter().filter(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{domain,..}if domain=="detection2026")).all(|e|matches!(e.vis,harpoon_core::Visibility::Referee)));
    assert!(d::known_category(&w, harpoon_core::Side::Blue, ids[1]).is_none());
    let bytes = serde_json::to_vec(&w).unwrap();
    let mut restored: harpoon_core::World = serde_json::from_slice(&bytes).unwrap();
    w.entity_mut(ids[1]).unwrap().pos = harpoon_core::Vec2::new(2.0, 0.0);
    restored.entity_mut(ids[1]).unwrap().pos = harpoon_core::Vec2::new(2.0, 0.0);
    harpoon_core::sim::tick(&mut w, &[]);
    harpoon_core::sim::tick(&mut restored, &[]);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert_eq!(w.sides[0].contacts.len(), 1);
    let c = w.sides[0].contacts.values().next().unwrap();
    assert_eq!(c.est_course_deg, 0.0);
    assert_eq!(c.est_speed_kts, 0.0);
    assert_eq!(
        d::known_category(&w, harpoon_core::Side::Blue, ids[1]),
        Some(PlatformKind::Ship)
    );
}
#[test]
fn sim_tick_unknown_esm_binding_cannot_reuse_legacy_range() {
    let mut w = fixture();
    for e in w.entities.values_mut() {
        e.weapons.clear();
        e.fire_on = false;
        e.sensors
            .retain(|s| w.db.sensors[&s.sensor].kind == harpoon_core::db::SensorKind::Esm);
    }
    w.rules.detection.as_mut().unwrap().config.environment = Some(env());
    harpoon_core::sim::tick(&mut w, &[]);
    assert!(w.sides.iter().all(|s| s.contacts.is_empty()));
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{domain,detail,..}if domain=="detection2026"&&detail.contains("not source-verified"))));
}
#[test]
fn es_bearing_only_world_scan_never_imparts_truth_range_or_class() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    for e in w.entities.values_mut() {
        e.weapons.clear();
        e.fire_on = false;
        e.sensors
            .retain(|s| w.db.sensors[&s.sensor].kind == harpoon_core::db::SensorKind::Esm);
    }
    let sensor = w.entity(ids[0]).unwrap().sensors[0].sensor.clone();
    let s = w.rules.detection.as_mut().unwrap();
    s.config.environment = Some(env());
    s.config.bindings.insert(
        format!("us_ford:{sensor}"),
        d::Receiver::Es {
            source: "source fixture,5.3 sixth-generation ES".into(),
            generation: 6,
            targeting_complex: false,
            rwr: false,
            bearing_error_deg: Some(2.0),
            els: false,
        },
    );
    s.emissions.insert(
        ids[1],
        d::Emission {
            source: "source fixture active firecontrol".into(),
            emitting: true,
            weapon_radar: true,
            lpi: false,
            radar_role: "MFC".into(),
            max_range_nm: 20.,
            antenna_height_m: 40.,
        },
    );
    harpoon_core::sim::tick(&mut w, &[]);
    let c = w.sides[0].contacts.values().next().unwrap();
    assert!(c.bearing_only);
    assert!(c.class_name.is_none());
    assert_eq!(c.est_speed_kts, 0.0);
    assert_eq!(c.est_course_deg, 0.0);
    assert!(d::known_category(&w, harpoon_core::Side::Blue, ids[1]).is_none());
    assert!((w.entity(ids[0]).unwrap().pos.range_to(c.estimate) - 1.0).abs() < 1e-8);
}

fn explicit_target() -> d::TargetProfile {
    d::TargetProfile {
        source: "mechanism fixture: physical A, Quiet source cells; no actual platform fit claim"
            .into(),
        size: 0,
        noise: Some(2),
        anechoic: false,
        titanium: false,
        pumpjet: false,
        stealthy_ir: false,
        radar_signature: Some(0),
    }
}
fn install_receiver(
    w: &mut harpoon_core::World,
    oid: EntityId,
    kind: harpoon_core::db::SensorKind,
    receiver: d::Receiver,
) -> String {
    let id = format!("acceptance_sensor_{}", oid.0);
    let mut def = w.db.sensors.values().next().unwrap().clone();
    def.id = id.clone();
    def.kind = kind;
    def.air_range_nm = 0.0;
    def.surface_range_nm = 0.0;
    def.sub_range_nm = 0.0;
    def.missile_range_nm = 0.0;
    w.db.sensors.insert(id.clone(), def);
    let class = w.entity(oid).unwrap().class.clone();
    w.entity_mut(oid).unwrap().sensors = vec![harpoon_core::world::SensorInstance {
        sensor: id.clone(),
        active: true,
    }];
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .bindings
        .insert(format!("{class}:{id}"), receiver);
    id
}
#[test]
fn real_ir_scan_uses_printed_generation_range_even_with_zero_legacy_range() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let sensor = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::Ir,
        d::Receiver::Ir {
            source: "5.5 mechanism fixture Gen4 scanner".into(),
            generation: 4,
            mode: d::IrMode::Scanning,
            arc_deg: 360.0,
        },
    );
    let mut state = w.rules.detection.take().unwrap();
    state.config.environment = Some(env());
    state
        .config
        .targets
        .insert("cn_001".into(), explicit_target());
    w.entity_mut(ids[1]).unwrap().pos = harpoon_core::Vec2::new(1.0, 0.0);
    let key = format!("5.5:{}:{sensor}:{}", ids[0].0, ids[1].0);
    while d::trial(&mut d::State::default(), w.seed, 0, &key, 180).0 > 80 {
        w.seed += 1
    }
    let scan = d::evaluate_world(&w, &mut state, ids[0], &sensor, ids[1]).unwrap();
    assert!(scan.detected);
    assert!(scan.bearing_only);
    assert!(scan.identify_class);
    assert_eq!(scan.known_category, Some(PlatformKind::Ship));
    w.entity_mut(ids[1]).unwrap().pos = harpoon_core::Vec2::new(20.01, 0.0);
    assert!(
        !d::evaluate_world(&w, &mut state, ids[0], &sensor, ids[1])
            .unwrap()
            .detected
    );
}
#[test]
fn sonar_scan_classifies_only_following_tactical_turn_and_preserves_mode_lock() {
    use d::{Frequency::Lmf, SonarMount::Bow};
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let sensor = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::SonarPassive,
        d::Receiver::Sonar {
            source: "5.4 same system mechanism fixture".into(),
            generation: 4,
            passive_range_nm: Some(10.0),
            active_range_nm: Some(10.0),
            frequency: Lmf,
            mount: Bow,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    w.entity_mut(ids[0]).unwrap().course_deg = 90.0;
    w.entity_mut(ids[0]).unwrap().speed_kts = 0.0;
    w.entity_mut(ids[1]).unwrap().speed_kts = 0.0;
    w.entity_mut(ids[1]).unwrap().pos = harpoon_core::Vec2::new(0.1, 0.0);
    let mut s = w.rules.detection.take().unwrap();
    let mut e = env();
    e.layer = None;
    e.sea_state = 2;
    s.config.environment = Some(e);
    s.config.targets.insert("cn_001".into(), explicit_target());
    let key = format!("5.4.6:{}:{sensor}:{}", ids[0].0, ids[1].0);
    while d::trial(&mut d::State::default(), w.seed, 0, &key, 180).0 > 90 {
        w.seed += 1
    }
    let first = d::evaluate_world(&w, &mut s, ids[0], &sensor, ids[1]).unwrap();
    assert!(first.detected);
    assert!(first.known_category.is_none());
    assert!(first.bearing_only);
    w.tick = 180;
    let next = d::evaluate_world(&w, &mut s, ids[0], &sensor, ids[1]).unwrap();
    if next.detected {
        assert_eq!(next.known_category, Some(PlatformKind::Ship));
        assert!(
            !next.identify_class,
            "LMF may tell Ship vs Sub, never individual class"
        )
    }
    w.db.sensors.get_mut(&sensor).unwrap().kind = harpoon_core::db::SensorKind::SonarActive;
    assert!(d::evaluate_world(&w, &mut s, ids[0], &sensor, ids[1])
        .unwrap_err()
        .contains("locked"));
    w.tick = 360;
    assert!(d::evaluate_world(&w, &mut s, ids[0], &sensor, ids[1]).is_ok());
}
#[test]
fn els_second_fix_requires_another_detection_window_and_actual_motion() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    w.entity_mut(ids[0]).unwrap().class = "us_f18e".into();
    w.entity_mut(ids[0]).unwrap().kind = PlatformKind::Aircraft;
    w.entity_mut(ids[0]).unwrap().altitude_m = 1000.0;
    let sensor = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::Esm,
        d::Receiver::Es {
            source: "5.3.1 Gen5 ELS mechanism fixture".into(),
            generation: 5,
            targeting_complex: false,
            rwr: false,
            bearing_error_deg: Some(2.),
            els: true,
        },
    );
    let mut s = w.rules.detection.take().unwrap();
    s.config.environment = Some(env());
    s.emissions.insert(
        ids[1],
        d::Emission {
            source: "5.3 fire control emitter fixture".into(),
            emitting: true,
            weapon_radar: true,
            lpi: false,
            radar_role: "MFC".into(),
            max_range_nm: 20.,
            antenna_height_m: 40.,
        },
    );
    let a = d::evaluate_world(&w, &mut s, ids[0], &sensor, ids[1]).unwrap();
    assert_eq!(a.fix_quality.as_deref(), Some("fair"));
    w.entity_mut(ids[0]).unwrap().pos = harpoon_core::Vec2::new(0., 1.);
    w.tick = 1;
    assert_eq!(
        d::evaluate_world(&w, &mut s, ids[0], &sensor, ids[1])
            .unwrap()
            .fix_quality
            .as_deref(),
        Some("fair")
    );
    w.tick = 180;
    let b = d::evaluate_world(&w, &mut s, ids[0], &sensor, ids[1]).unwrap();
    assert_eq!(b.fix_quality.as_deref(), Some("good"));
    assert!(b.bearing_only);
    assert!(!b.range_known);
}
#[test]
fn environment_illegal_layer_is_blocked_and_strength_aspect_is_source_based() {
    let mut e = env();
    e.sea_state = 6;
    assert!(e.validate().is_err());
    e.layer = None;
    assert!(e.validate().is_ok());
    assert_eq!(d::active_aspect_modifier(0.), -1);
    assert_eq!(d::active_aspect_modifier(90.), 1);
    assert_eq!(d::active_aspect_modifier(45.), 0);
}
#[test]
fn actual_towed_deployment_is_not_instant_and_seabed_loss_persists() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let sensor = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::SonarPassive,
        d::Receiver::Sonar {
            source: "5.4.2 lifecycle mechanism fixture".into(),
            generation: 5,
            passive_range_nm: Some(4.0),
            active_range_nm: Some(4.0),
            frequency: d::Frequency::Lmf,
            mount: d::SonarMount::Vds,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    w.rules.detection.as_mut().unwrap().config.environment = Some(env());
    d::order_array(&mut w, ids[0], &sensor, true, 100.0).unwrap();
    assert_eq!(d::array_speed_limit(&w, ids[0]), Some(15.0));
    let mut s = w.rules.detection.take().unwrap();
    w.tick = 1259;
    assert!(d::update_lifecycle(&w, &mut s).is_empty());
    assert!(!s.arrays[&format!("{}:{sensor}", ids[0].0)].deployed);
    w.tick = 1260;
    assert!(!d::update_lifecycle(&w, &mut s).is_empty());
    assert!(s.arrays[&format!("{}:{sensor}", ids[0].0)].deployed);
    w.rules.detection = Some(s);
    assert_eq!(d::array_speed_limit(&w, ids[0]), Some(25.0));
    let mut s = w.rules.detection.take().unwrap();
    s.config.environment.as_mut().unwrap().water_depth_m = 99.0;
    assert!(!d::update_lifecycle(&w, &mut s).is_empty());
    assert!(s.arrays[&format!("{}:{sensor}", ids[0].0)].lost);
    let mut restored: d::State = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
    assert!(d::update_lifecycle(&w, &mut restored).is_empty());
}
#[test]
fn starshell_exact_source_ranges_and_named_diagram_conflict() {
    let hit = d::starshell_effect(127., 5., true, 1).unwrap();
    assert!(hit.works && hit.illuminated);
    assert_eq!(hit.radius_yards, 400.0);
    let short = d::starshell_effect(127., 3.9, true, 1).unwrap();
    assert!(!short.works);
    let long = d::starshell_effect(127., 10., false, 7).unwrap();
    assert!(long.silhouetted);
    assert!(!long.illuminated);
    assert!(d::starshell_effect(127., 10., false, 16)
        .unwrap_err()
        .contains("pending adjudication"));
    assert!(!d::starshell_effect(127., 10., false, 96).unwrap().works);
    assert_eq!(d::starshell_detection_range(2, false).unwrap(), 12.0);
}
#[test]
fn real_radar_group_changes_annex_signature_before_horizon() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    {
        let o = w.entity_mut(ids[0]).unwrap();
        o.class = "us_f18e".into();
        o.kind = PlatformKind::Aircraft;
        o.altitude_m = 3000.;
        o.course_deg = 90.;
    }
    {
        let t = w.entity_mut(ids[1]).unwrap();
        t.class = "us_f35c".into();
        t.kind = PlatformKind::Aircraft;
        t.altitude_m = 3000.;
        t.course_deg = 270.;
    }
    let def = w.db.sensors.get("radar_air_med").unwrap().clone();
    let o = w.entity(ids[0]).unwrap().clone();
    let t = w.entity(ids[1]).unwrap().clone();
    let mut state = d::State::default();
    assert_eq!(
        d::radar_range_world(&w, &state, &o, &def, &t).unwrap(),
        13.0
    );
    let mut group = vec![ids[1]];
    for j in 0..8 {
        let mut member = t.clone();
        member.id = EntityId(500 + j);
        group.push(member.id);
        w.entities.insert(member.id, member);
    }
    state.groups.insert("flight".into(), group[..3].to_vec());
    assert_eq!(
        d::radar_range_world(&w, &state, &o, &def, &t).unwrap(),
        42.0
    );
    state.groups.insert("flight".into(), group);
    assert_eq!(
        d::radar_range_world(&w, &state, &o, &def, &t).unwrap(),
        106.0
    );
}
#[test]
fn real_mast_radar_uses_signature_and_continuous_exposure() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    {
        let t = w.entity_mut(ids[1]).unwrap();
        t.kind = PlatformKind::Sub;
        t.depth_m = 10.;
        t.speed_kts = 5.;
        t.pos = harpoon_core::Vec2::new(2., 0.);
    }
    let def = w.db.sensors.get("cn_ljq364").unwrap().clone();
    let mut state = d::State::default();
    let mut e = env();
    e.sea_state = 0;
    state.config.environment = Some(e);
    state.masts.insert(
        ids[1],
        d::Mast {
            count: 1,
            raised_since: 0,
        },
    );
    w.tick = 180;
    let r = d::evaluate_world(&w, &mut state, ids[0], &def.id, ids[1]).unwrap();
    assert!(r.detected);
    assert!(r.known_category.is_none());
    state.masts.get_mut(&ids[1]).unwrap().count = 0;
    assert!(
        !d::evaluate_world(&w, &mut state, ids[0], &def.id, ids[1])
            .unwrap()
            .detected
    );
}
#[test]
fn array_actual_movement_cap_and_own_only_view() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let sensor = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::SonarPassive,
        d::Receiver::Sonar {
            source: "5.4.2 mechanism fixture".into(),
            generation: 5,
            passive_range_nm: Some(4.),
            active_range_nm: Some(4.),
            frequency: d::Frequency::Lmf,
            mount: d::SonarMount::Vds,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    w.rules.detection.as_mut().unwrap().config.environment = Some(env());
    d::order_array(&mut w, ids[0], &sensor, true, 100.).unwrap();
    w.entity_mut(ids[0]).unwrap().ordered_speed_kts = 30.;
    for _ in 0..100 {
        harpoon_core::movement::update_movement(&mut w);
        assert!(w.entity(ids[0]).unwrap().speed_kts <= 15.)
    }
    assert!(d::own_view(&w, harpoon_core::Side::Blue, ids[0]).is_some());
    assert!(d::own_view(&w, harpoon_core::Side::Red, ids[0]).is_none());
}
#[test]
fn real_passive_scan_applies_masking_before_receiving_contact() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let sensor = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::SonarPassive,
        d::Receiver::Sonar {
            source: "5.4.6.5 masking fixture".into(),
            generation: 5,
            passive_range_nm: Some(10.),
            active_range_nm: None,
            frequency: d::Frequency::Lmf,
            mount: d::SonarMount::Bow,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    {
        let o = w.entity_mut(ids[0]).unwrap();
        o.course_deg = 90.;
        o.speed_kts = 0.;
    }
    {
        let t = w.entity_mut(ids[1]).unwrap();
        t.pos = harpoon_core::Vec2::new(0.2, 0.);
        t.speed_kts = 0.;
    }
    let mut near = w.entity(ids[1]).unwrap().clone();
    near.id = EntityId(500);
    near.pos = harpoon_core::Vec2::new(0.1, 0.);
    w.entities.insert(near.id, near);
    let mut state = w.rules.detection.take().unwrap();
    let mut e = env();
    e.layer = None;
    e.sea_state = 2;
    state.config.environment = Some(e);
    state
        .config
        .targets
        .insert("cn_001".into(), explicit_target());
    let far = d::evaluate_world(&w, &mut state, ids[0], &sensor, ids[1]).unwrap();
    assert!(!far.detected);
    assert_eq!(far.trace["probability"], 0.0);
}
#[test]
fn tv_requires_cue_and_source_mode_while_ordinary_radar_never_identifies() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let mut state = d::State::default();
    state.config.environment = Some(env());
    let sensor = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::Ir,
        d::Receiver::Tv {
            source: "5.5.4 fixture magnifier; no fitted platform claim".into(),
            low_light: false,
            range_nm: 10.,
            arc_deg: 360.,
        },
    );
    state.config = w.rules.detection.as_ref().unwrap().config.clone();
    state.config.environment = Some(env());
    let scan = d::evaluate_world(&w, &mut state, ids[0], &sensor, ids[1]).unwrap();
    assert!(
        !scan.detected,
        "TV cannot search without a contact to cue it"
    );
}
#[test]
fn mast_actions_require_source_fit_and_physical_envelope() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    w.entity_mut(id).unwrap().kind = PlatformKind::Sub;
    w.entity_mut(id).unwrap().depth_m = 10.;
    assert!(d::apply(&mut w, id, &d::DetectionAction::Masts { count: 1 }).is_err());
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .max_masts
        .insert("us_ford".into(), 2);
    d::apply(&mut w, id, &d::DetectionAction::Masts { count: 2 }).unwrap();
    assert_eq!(w.rules.detection.as_ref().unwrap().masts[&id].count, 2);
    w.entity_mut(id).unwrap().speed_kts = 15.1;
    assert!(d::apply(&mut w, id, &d::DetectionAction::Masts { count: 1 }).is_err());
    d::apply(&mut w, id, &d::DetectionAction::Masts { count: 0 }).unwrap();
}
#[test]
fn ultraquiet_has_source_noise_and_actual_speed_cost() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    {
        let e = w.entity_mut(id).unwrap();
        e.kind = PlatformKind::Sub;
        e.speed_kts = 3.;
    }
    assert!(d::apply(
        &mut w,
        id,
        &d::DetectionAction::UltraQuiet { enabled: true }
    )
    .is_err());
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .propulsion
        .insert("us_ford".into(), "nuclear".into());
    d::apply(
        &mut w,
        id,
        &d::DetectionAction::UltraQuiet { enabled: true },
    )
    .unwrap();
    assert_eq!(d::array_speed_limit(&w, id), Some(3.));
    d::apply(
        &mut w,
        id,
        &d::DetectionAction::UltraQuiet { enabled: false },
    )
    .unwrap();
    assert_eq!(d::array_speed_limit(&w, id), None);
}
#[test]
fn legacy_id_release_noise_is_not_rerolled_or_lost_on_save() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    w.entity_mut(id).unwrap().kind = PlatformKind::Sub;
    let weapon =
        w.db.weapons
            .values()
            .find(|d| d.kind == harpoon_core::db::WeaponKind::Torpedo)
            .unwrap()
            .id
            .clone();
    w.emit(
        harpoon_core::EventKind::WeaponsRelease {
            side: harpoon_core::Side::Blue,
            shooter: id,
            shooter_name: "fixture".into(),
            weapon: weapon.clone(),
            count: 1,
        },
        harpoon_core::Visibility::Side(harpoon_core::Side::Blue),
    );
    let mut s = w.rules.detection.take().unwrap();
    let evidence = d::update_lifecycle(&w, &mut s);
    assert_eq!(s.transients[&id], (180, 3));
    assert_eq!(evidence.len(), 1);
    let mut restored: d::State = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
    assert!(d::update_lifecycle(&w, &mut restored).is_empty());
    w.tick = 180;
    d::update_lifecycle(&w, &mut restored);
    assert!(!restored.transients.contains_key(&id));
}
#[test]
fn actual_submarine_releases_keep_legal_noise_and_reject_unpermitted_ascm() {
    use harpoon_core::db::WeaponKind;
    // These four cases test the production release producer/consumer interface.
    // Reusing a hull as a submarine fixture makes no claim about actual fit.
    for (kind, quiet, expected) in [
        (WeaponKind::Torpedo, false, 3),
        (WeaponKind::Torpedo, true, 0),
        (WeaponKind::Lwt, false, 3),
    ] {
        let mut w = fixture();
        let shooter = *w.entities.keys().next().unwrap();
        {
            let e = w.entity_mut(shooter).unwrap();
            e.kind = PlatformKind::Sub;
            e.depth_m = 10.;
            e.ordered_depth_m = 10.;
            e.speed_kts = 5.;
            e.ordered_speed_kts = 5.;
        }
        let def =
            w.db.weapons
                .values()
                .find(|d| d.kind == kind)
                .unwrap()
                .clone();
        assert_ne!(def.id, def.name);
        let projectile = harpoon_core::weapons::launch(
            &mut w,
            shooter,
            &def,
            None,
            Some(harpoon_core::Vec2::new(30., 0.)),
        )
        .unwrap_or_else(|| {
            panic!("production launch rejected {kind:?} in explicit Shallow/5kt mechanism fixture")
        });
        assert_eq!(
            w.entity(projectile)
                .unwrap()
                .projectile
                .as_ref()
                .unwrap()
                .weapon,
            def.id
        );
        assert!(w.events.iter().any(|event| matches!(&event.kind,
            harpoon_core::EventKind::WeaponsRelease { weapon, .. } if weapon == &def.name)));
        let mut state = w.rules.detection.take().unwrap();
        if quiet {
            state
                .config
                .quiet_launches
                .insert(w.entity(shooter).unwrap().class.clone());
        }
        d::update_lifecycle(&w, &mut state);
        assert_eq!(
            state.transients.get(&shooter),
            Some(&(180, expected)),
            "{kind:?}, quiet={quiet}"
        );
    }
    // Full ASW carriers now require a real source FCS/physical payload permit.
    // The positive submarine ASW noise producer is covered by the independent
    // asw_weapons_resume test; this naked release must create no false noise.
    let mut bare = fixture();
    let actor=*bare.entities.keys().next().unwrap();
    let asw=bare.db.weapons.values().find(|d|d.kind==WeaponKind::AswRocket).unwrap().clone();
    assert!(harpoon_core::weapons::launch(&mut bare,actor,&asw,None,Some(harpoon_core::Vec2::new(30.,0.))).is_none());
    assert_eq!(bare.stats.projectiles_launched,0);
    let mut source_state=bare.rules.detection.take().unwrap();
    assert!(d::update_lifecycle(&bare,&mut source_state).is_empty());
    assert!(source_state.transients.is_empty());
    // The generic ASCM lacks an exact submarine source release permit. A
    // display-name consumer test must not bypass that production gate, invent
    // a submarine IC variant, or switch rule versions to make it fire.
    let mut w = fixture();
    let shooter = *w.entities.keys().next().unwrap();
    {
        let e = w.entity_mut(shooter).unwrap();
        e.kind = PlatformKind::Sub;
        e.depth_m = 10.;
        e.ordered_depth_m = 10.;
        e.speed_kts = 5.;
        e.ordered_speed_kts = 5.;
    }
    assert!(w.context.antiship_2026.is_none());
    let def = w.db.weapons["cn_yj83"].clone();
    let before_entities = w.entities.len();
    let before_next = w.next_entity;
    let before_mounts = serde_json::to_value(&w.entity(shooter).unwrap().weapons).unwrap();
    assert!(harpoon_core::weapons::launch(
        &mut w,
        shooter,
        &def,
        None,
        Some(harpoon_core::Vec2::new(30., 0.)),
    )
    .is_none());
    assert_eq!(w.entities.len(), before_entities);
    assert_eq!(w.next_entity, before_next);
    assert_eq!(
        serde_json::to_value(&w.entity(shooter).unwrap().weapons).unwrap(),
        before_mounts
    );
    assert!(!w
        .events
        .iter()
        .any(|event| matches!(&event.kind, harpoon_core::EventKind::WeaponsRelease { .. })));
    assert!(w.events.iter().any(|event| matches!(&event.kind,
        harpoon_core::EventKind::RulesEvent { rule, .. }
            if rule == "8.4.source_permit_required")));
    let mut state = w.rules.detection.take().unwrap();
    assert!(d::update_lifecycle(&w, &mut state).is_empty());
    assert!(!state.transients.contains_key(&shooter));
    assert!(!state.fired.contains(&shooter));
    assert!(state.release_weapons.is_empty());
}
#[test]
fn controlled_ascm_release_consumer_keeps_six_noise_without_claiming_a_launch() {
    // scenario_assumption: provide a completed release event and its exact
    // projectile solely as consumer inputs. This does not execute or certify
    // an ASCM firing procedure, submarine weapon fit, or source release permit.
    let mut w = fixture();
    let shooter = *w.entities.keys().next().unwrap();
    w.entity_mut(shooter).unwrap().kind = PlatformKind::Sub;
    let def = w.db.weapons["cn_yj83"].clone();
    assert_ne!(def.id, def.name);
    let mut projectile = w.entity(shooter).unwrap().clone();
    projectile.id = w.allocate_id();
    let id = projectile.id;
    projectile.kind = PlatformKind::Missile;
    projectile.class = def.id.clone();
    projectile.name = "controlled completed-release consumer input".into();
    projectile.rule_data = None;
    projectile.weapons.clear();
    projectile.sensors.clear();
    projectile.deck = None;
    projectile.mission = None;
    projectile.station = None;
    projectile.projectile = Some(harpoon_core::world::ProjectileData {
        targeted_sensor: None,
        weapon: def.id.clone(),
        target: None,
        aim: harpoon_core::Vec2::new(30., 0.),
        launched_by: shooter,
        launched_by_side: harpoon_core::Side::Blue,
        live_s: 0.,
        enabled: false,
        decoyed: false,
        decoy_point: None,
        chaff_checked: false,
        ciws_checked: false,
        passes: 0,
    });
    w.entities.insert(id, projectile);
    w.emit(
        harpoon_core::EventKind::WeaponsRelease {
            side: harpoon_core::Side::Blue,
            shooter,
            shooter_name: "controlled completed-release consumer input".into(),
            weapon: def.name.clone(),
            count: 1,
        },
        harpoon_core::Visibility::Side(harpoon_core::Side::Blue),
    );
    let release_seq = w.event_seq;
    d::register_projectile_release(&mut w, id);
    let mut state = w.rules.detection.take().unwrap();
    assert_eq!(state.release_weapons[&release_seq].projectile, id);
    assert_eq!(state.release_weapons[&release_seq].weapon, def.id);
    let evidence = d::update_lifecycle(&w, &mut state);
    assert_eq!(state.transients.get(&shooter), Some(&(180, 6)));
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].1["weapon_id"], def.id);
    assert_eq!(evidence[0].1["projectile"], id.0);
    assert_eq!(evidence[0].1["noise_floor"], 6);
}
#[test]
fn actual_submarine_launch_is_consumed_by_source_passive_scan_after_world_restore() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    for e in w.entities.values_mut() {
        e.weapons.clear();
        e.sensors.clear();
        e.fire_on = false;
        e.speed_kts = 0.;
        e.ordered_speed_kts = 0.;
    }
    let target = ids[1];
    {
        let e = w.entity_mut(target).unwrap();
        e.kind = PlatformKind::Sub;
        e.depth_m = 100.;
    }
    let sensor = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::SonarPassive,
        d::Receiver::Sonar {
            source: "5.4.6.5 launch transient mechanism fixture".into(),
            generation: 5,
            passive_range_nm: Some(10.),
            active_range_nm: None,
            frequency: d::Frequency::Lf,
            mount: d::SonarMount::Bow,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    w.entity_mut(ids[0]).unwrap().course_deg = 90.;
    let state = w.rules.detection.as_mut().unwrap();
    state.config.environment = Some(env());
    let mut profile = explicit_target();
    profile.noise = Some(4);
    state.config.targets.insert("cn_001".into(), profile);
    let key = format!("5.4.6:{}:{sensor}:{}", ids[0].0, target.0);
    while d::trial(&mut d::State::default(), w.seed, 0, &key, 180).0 > 50 {
        w.seed += 1;
    }
    let mut state = w.rules.detection.take().unwrap();
    let before = d::evaluate_world(&w, &mut state, ids[0], &sensor, target).unwrap();
    assert_eq!(before.trace["inputs"]["target_modifier"], -6);
    w.rules.detection = Some(state);
    let def = w.db.weapons["us_mk48"].clone();
    harpoon_core::weapons::launch(
        &mut w,
        target,
        &def,
        None,
        Some(harpoon_core::Vec2::new(30., 0.)),
    )
    .unwrap();
    let mut restored: harpoon_core::World =
        serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for item in [&mut w, &mut restored] {
        harpoon_core::sensors::update_sensors(item);
        assert_eq!(
            item.rules
                .detection
                .as_ref()
                .unwrap()
                .transients
                .get(&target),
            Some(&(180, 3))
        );
        let mut state = item.rules.detection.take().unwrap();
        let scan = d::evaluate_world(item, &mut state, ids[0], &sensor, target).unwrap();
        assert_eq!(scan.trace["inputs"]["target_modifier"], 3);
        assert!(item.sides[0].contacts.values().any(|c| c.about == target));
        item.rules.detection = Some(state);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    for item in [&mut w, &mut restored] {
        item.tick = 180;
        harpoon_core::sensors::update_sensors(item);
        assert!(!item
            .rules
            .detection
            .as_ref()
            .unwrap()
            .transients
            .contains_key(&target));
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
#[test]
fn exact_projectile_hook_disambiguates_names_and_unmapped_ambiguity_blocks() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let target = ids[1];
    for e in w.entities.values_mut() {
        e.weapons.clear();
        e.sensors.clear();
        e.fire_on = false;
        e.speed_kts = 0.;
    }
    w.entity_mut(target).unwrap().kind = PlatformKind::Sub;
    let sensor = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::SonarPassive,
        d::Receiver::Sonar {
            source: "source identity ambiguity fixture".into(),
            generation: 5,
            passive_range_nm: Some(10.),
            active_range_nm: None,
            frequency: d::Frequency::Lf,
            mount: d::SonarMount::Bow,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    w.entity_mut(ids[0]).unwrap().course_deg = 90.;
    w.rules.detection.as_mut().unwrap().config.environment = Some(env());
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .targets
        .insert("cn_001".into(), explicit_target());
    let def = w.db.weapons["us_mk48"].clone();
    let mut same_name = def.clone();
    same_name.id = "ambiguous_source_fixture".into();
    same_name.kind = harpoon_core::db::WeaponKind::Ascm;
    w.db.weapons.insert(same_name.id.clone(), same_name);
    let mut mapped = w.clone();
    let projectile = harpoon_core::weapons::launch(
        &mut mapped,
        target,
        &def,
        None,
        Some(harpoon_core::Vec2::new(30., 0.)),
    )
    .unwrap();
    // The map retains identity even if the projectile is removed before the
    // next sensor pass or the World is saved with its pending release event.
    mapped.entities.remove(&projectile);
    let mut mapped: harpoon_core::World =
        serde_json::from_value(serde_json::to_value(mapped).unwrap()).unwrap();
    harpoon_core::sensors::update_sensors(&mut mapped);
    assert_eq!(
        mapped
            .rules
            .detection
            .as_ref()
            .unwrap()
            .transients
            .get(&target),
        Some(&(180, 3))
    );
    w.emit(
        harpoon_core::EventKind::WeaponsRelease {
            side: harpoon_core::Side::Red,
            shooter: target,
            shooter_name: "source ambiguity fixture".into(),
            weapon: def.name,
            count: 1,
        },
        harpoon_core::Visibility::Side(harpoon_core::Side::Red),
    );
    harpoon_core::sensors::update_sensors(&mut w);
    assert!(!w
        .rules
        .detection
        .as_ref()
        .unwrap()
        .transients
        .contains_key(&target));
    assert!(w
        .rules
        .detection
        .as_ref()
        .unwrap()
        .unresolved_transients
        .contains_key(&target));
    assert!(w.sides[0].contacts.is_empty());
    let mut state = w.rules.detection.take().unwrap();
    assert!(d::evaluate_world(&w, &mut state, ids[0], &sensor, target)
        .unwrap_err()
        .contains("release transient pending"));
    w.rules.detection = Some(state);
    let mut copy: harpoon_core::World =
        serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    for item in [&mut w, &mut copy] {
        item.tick = 180;
        harpoon_core::sensors::update_sensors(item);
        assert!(item
            .rules
            .detection
            .as_ref()
            .unwrap()
            .unresolved_transients
            .is_empty());
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&copy).unwrap()
    );
}
#[test]
fn actual_navigation_acceleration_penalty_enters_passive_scan_once_and_clears() {
    use harpoon_core::rules::navigation2026 as nav;
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let target = ids[1];
    for e in w.entities.values_mut() {
        e.weapons.clear();
        e.sensors.clear();
        e.fire_on = false;
        e.speed_kts = 0.;
        e.ordered_speed_kts = 0.;
    }
    {
        let e = w.entity_mut(target).unwrap();
        e.kind = PlatformKind::Sub;
        e.depth_m = 100.;
        e.ordered_depth_m = 100.;
        e.course_deg = 90.;
        e.ordered_course_deg = 90.;
    }
    let sensor = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::SonarPassive,
        d::Receiver::Sonar {
            source: "3.1.1/5.4.6.5 source cross-domain fixture".into(),
            generation: 5,
            passive_range_nm: Some(10.),
            active_range_nm: Some(10.),
            frequency: d::Frequency::Lf,
            mount: d::SonarMount::Bow,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    {
        let e = w.entity_mut(ids[0]).unwrap();
        e.course_deg = 90.;
        e.ordered_course_deg = 90.;
    }
    w.rules.detection.as_mut().unwrap().config.environment = Some(env());
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .targets
        .insert("cn_001".into(), explicit_target());
    w.context.navigation_2026 = Some(nav::Configuration {
        hazards: None,
        source: "scenario_assumption: source size-C navigation mechanism only, no hull fit claim"
            .into(),
        units: std::collections::BTreeMap::from([(
            "hidden".into(),
            nav::Fit {
                source: "source mechanism fixture".into(),
                size: 'C',
                merchant: false,
                passenger_liner: false,
                cpp: false,
                waterjet: false,
                undamaged_max_kts: 29.,
                propulsion: nav::Propulsion::Nuclear,
                readiness: nav::Readiness::Underway,
                submarine: None,
            },
        )]),
    });
    nav::initialize(&mut w).unwrap();
    let initial = w.clone();
    // Normal quiet acceleration uses half the table rate, with no +4.
    harpoon_core::sim::tick(
        &mut w,
        &[(
            harpoon_core::Side::Red,
            harpoon_core::Command::SetSpeed {
                unit: target,
                speed_kts: 20.,
            },
        )],
    );
    let mut state = w.rules.detection.take().unwrap();
    assert_eq!(
        d::evaluate_world(&w, &mut state, ids[0], &sensor, target)
            .unwrap()
            .trace["inputs"]["target_modifier"],
        0
    );
    w.rules.detection = Some(state);
    // The real movement hook creates the condition; the source scanner reads
    // it. Saving while the condition is live preserves the complete outcome.
    let mut fast = initial.clone();
    harpoon_core::sim::tick(
        &mut fast,
        &[
            (
                harpoon_core::Side::Red,
                harpoon_core::Command::Navigation {
                    unit: target,
                    action: nav::Action::Quiet { enabled: false },
                },
            ),
            (
                harpoon_core::Side::Red,
                harpoon_core::Command::SetSpeed {
                    unit: target,
                    speed_kts: 20.,
                },
            ),
        ],
    );
    assert_eq!(
        fast.entity(target)
            .unwrap()
            .ext
            .get("navigation.cavitation"),
        Some(&1.)
    );
    let mut copy: harpoon_core::World =
        serde_json::from_value(serde_json::to_value(&fast).unwrap()).unwrap();
    for item in [&mut fast, &mut copy] {
        let mut state = item.rules.detection.take().unwrap();
        assert_eq!(
            d::evaluate_world(item, &mut state, ids[0], &sensor, target)
                .unwrap()
                .trace["inputs"]["target_modifier"],
            4
        );
        item.rules.detection = Some(state);
        let speed = item.entity(target).unwrap().speed_kts;
        item.entity_mut(target).unwrap().ordered_speed_kts = speed;
        harpoon_core::sim::tick(item, &[]);
        assert!(!item
            .entity(target)
            .unwrap()
            .ext
            .contains_key("navigation.cavitation"));
        let mut state = item.rules.detection.take().unwrap();
        assert_eq!(
            d::evaluate_world(item, &mut state, ids[0], &sensor, target)
                .unwrap()
                .trace["inputs"]["target_modifier"],
            0
        );
        item.rules.detection = Some(state);
    }
    assert_eq!(
        serde_json::to_value(&fast).unwrap(),
        serde_json::to_value(&copy).unwrap()
    );
    let mut both = initial;
    {
        let e = both.entity_mut(target).unwrap();
        e.speed_kts = 26.;
        e.depth_m = 0.;
        e.ordered_depth_m = 0.;
        e.ordered_speed_kts = 0.;
    }
    nav::apply(&mut both, target, &nav::Action::Quiet { enabled: false }).unwrap();
    harpoon_core::sim::tick(&mut both, &[]);
    let mut state = both.rules.detection.take().unwrap();
    assert_eq!(
        d::evaluate_world(&both, &mut state, ids[0], &sensor, target)
            .unwrap()
            .trace["inputs"]["target_modifier"],
        4,
        "speed cavitation and acceleration/deceleration use one source +4 modifier"
    );
}
#[test]
fn actual_successful_sensor_scans_preserve_each_observers_fcs_evidence() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let observer = ids[0];
    let target = ids[1];
    let visual = w
        .entity(observer)
        .unwrap()
        .sensors
        .iter()
        .find(|s| w.db.sensors[&s.sensor].kind == harpoon_core::db::SensorKind::Visual)
        .unwrap()
        .clone();
    for e in w.entities.values_mut() {
        e.weapons.clear();
        e.sensors.clear();
        e.fire_on = false;
        e.speed_kts = 0.;
        e.ordered_speed_kts = 0.;
    }
    w.entity_mut(target).unwrap().pos = harpoon_core::Vec2::new(0.1, 0.);
    let receiver = || d::Receiver::Ir {
        source: "5.1/5.5/6.3 real observer evidence mechanism fixture".into(),
        generation: 4,
        mode: d::IrMode::Irst,
        arc_deg: 360.,
    };
    let first_sensor = install_receiver(
        &mut w,
        observer,
        harpoon_core::db::SensorKind::Ir,
        receiver(),
    );
    w.entity_mut(observer).unwrap().sensors.push(visual.clone());
    let mut second = w.entity(observer).unwrap().clone();
    second.id = w.allocate_id();
    second.name = "second real observer".into();
    second.sensors.clear();
    second.pos = harpoon_core::Vec2::new(0., 0.01);
    let second_id = second.id;
    w.entities.insert(second_id, second);
    let second_sensor = install_receiver(
        &mut w,
        second_id,
        harpoon_core::db::SensorKind::Ir,
        receiver(),
    );
    let mut shared_only = w.entity(observer).unwrap().clone();
    shared_only.id = w.allocate_id();
    shared_only.name = "shared contact only".into();
    shared_only.sensors.clear();
    let shared_id = shared_only.id;
    w.entities.insert(shared_id, shared_only);
    // A fitted but broken third receiver must never earn a successful track.
    let mut broken_def = w.db.sensors[&first_sensor].clone();
    broken_def.id = "broken_evidence_receiver".into();
    w.db.sensors.insert(broken_def.id.clone(), broken_def);
    w.entity_mut(observer)
        .unwrap()
        .sensors
        .push(harpoon_core::world::SensorInstance {
            sensor: "broken_evidence_receiver".into(),
            active: true,
        });
    w.entity_mut(observer)
        .unwrap()
        .ext
        .insert("damage.sensor.broken_evidence_receiver".into(), 1.);
    let class = w.entity(observer).unwrap().class.clone();
    let state = w.rules.detection.as_mut().unwrap();
    state.config.environment = Some(env());
    state
        .config
        .targets
        .insert("cn_001".into(), explicit_target());
    state
        .config
        .bindings
        .insert(format!("{class}:broken_evidence_receiver"), receiver());
    let keys = [
        format!("5.5:{}:{first_sensor}:{}", observer.0, target.0),
        format!("5.5:{}:{second_sensor}:{}", second_id.0, target.0),
    ];
    while keys
        .iter()
        .any(|key| d::trial(&mut d::State::default(), w.seed, 0, key, 180).0 > 80)
    {
        w.seed += 1;
    }
    harpoon_core::sim::tick(&mut w, &[]);
    let first_key = format!("{}:{first_sensor}:{}", observer.0, target.0);
    let second_key = format!("{}:{second_sensor}:{}", second_id.0, target.0);
    let visual_key = format!("{}:{}:{}", observer.0, visual.sensor, target.0);
    let tracks = &w.rules.aaw.as_ref().unwrap().firecontrol.tracks;
    for key in [&first_key, &second_key, &visual_key] {
        assert_eq!(
            tracks.get(key).map(|t| t.last),
            Some(1),
            "actual observer/sensor evidence missing: {key}"
        );
    }
    assert!(!tracks
        .values()
        .any(|t| t.observer == shared_id || t.sensor == "broken_evidence_receiver"));
    // The side contact is fused to the better visual result, but the successful
    // IR receiver still owns its separate evidence. Damage stops its clock.
    w.entity_mut(observer)
        .unwrap()
        .ext
        .insert(format!("damage.sensor.{first_sensor}"), 1.);
    let mut restored: harpoon_core::World =
        serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    for item in [&mut w, &mut restored] {
        harpoon_core::sim::tick(item, &[]);
        let tracks = &item.rules.aaw.as_ref().unwrap().firecontrol.tracks;
        assert_eq!(tracks[&first_key].last, 1);
        assert_eq!(tracks[&second_key].last, 2);
        assert_eq!(tracks[&visual_key].last, 2);
        assert!(!tracks
            .values()
            .any(|t| t.observer == shared_id || t.sensor == "broken_evidence_receiver"));
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
fn fcs_motion_fixture() -> (harpoon_core::World, EntityId, EntityId, String) {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let observer = ids[0];
    let target = ids[1];
    let visual = w
        .entity(observer)
        .unwrap()
        .sensors
        .iter()
        .find(|s| w.db.sensors[&s.sensor].kind == harpoon_core::db::SensorKind::Visual)
        .unwrap()
        .clone();
    for e in w.entities.values_mut() {
        e.weapons.clear();
        e.sensors.clear();
        e.fire_on = false;
        e.speed_kts = 0.;
        e.ordered_speed_kts = 0.;
    }
    {
        let e = w.entity_mut(target).unwrap();
        e.pos = harpoon_core::Vec2::new(0.1, 0.);
        e.course_deg = 90.;
        e.ordered_course_deg = 90.;
        e.speed_kts = 10.;
        e.ordered_speed_kts = 10.;
    }
    let sensor = install_receiver(
        &mut w,
        observer,
        harpoon_core::db::SensorKind::Ir,
        d::Receiver::Ir {
            source: "5.1/5.5/6.3 single-platform FCS source fixture".into(),
            generation: 4,
            mode: d::IrMode::Irst,
            arc_deg: 360.,
        },
    );
    let mut friend = w.entity(observer).unwrap().clone();
    friend.id = w.allocate_id();
    friend.name = "independent ranging observer without source TDL".into();
    friend.sensors = vec![visual];
    friend.pos = harpoon_core::Vec2::new(0., 0.01);
    w.entities.insert(friend.id, friend);
    let state = w.rules.detection.as_mut().unwrap();
    state.config.environment = Some(env());
    state
        .config
        .targets
        .insert("cn_001".into(), explicit_target());
    let key = format!("5.5:{}:{sensor}:{}", observer.0, target.0);
    while [0, 180]
        .iter()
        .any(|at| d::trial(&mut d::State::default(), w.seed, *at, &key, 180).0 > 80)
    {
        w.seed += 1;
    }
    (w, observer, target, sensor)
}
#[test]
fn actual_fcs_bearing_only_observer_cannot_borrow_friends_fused_motion() {
    let (mut w, observer, target, sensor) = fcs_motion_fixture();
    for _ in 0..181 {
        assert!(!w.over);
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(
        harpoon_core::rules::operations::motion_known(&w, harpoon_core::Side::Blue, target),
        Some(true)
    );
    let contact = w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == target)
        .unwrap();
    assert!(!contact.bearing_only);
    assert!(contact.est_speed_kts > 9.9);
    let track = &w.rules.aaw.as_ref().unwrap().firecontrol.tracks
        [&format!("{}:{sensor}:{}", observer.0, target.0)];
    assert_eq!(track.first, 1);
    assert_eq!(track.last, 181);
    assert!(harpoon_core::rules::firecontrol2026::world_solution(&w, observer, target, 3, 561.).is_err(),
        "a successful local bearing receiver plus a friend's located side picture is not a source single-platform range/motion solution or declared TDL");
}
#[test]
fn actual_fcs_new_own_ranging_scan_cannot_borrow_friends_180_second_clock() {
    let (mut w, observer, target, _) = fcs_motion_fixture();
    w.entity_mut(observer).unwrap().sensors.clear();
    for _ in 0..180 {
        assert!(!w.over);
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let visual = w
        .entities
        .values()
        .find(|e| e.name == "independent ranging observer without source TDL")
        .unwrap()
        .sensors[0]
        .clone();
    w.entity_mut(observer).unwrap().sensors = vec![visual];
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(
        harpoon_core::rules::operations::motion_known(&w, harpoon_core::Side::Blue, target),
        Some(true)
    );
    assert!(harpoon_core::rules::firecontrol2026::world_solution(&w, observer, target, 3, 561.).is_err(),
        "one new local range sample cannot inherit the shared picture's completed source movement observation window");
}
fn fcs_own_range_fixture() -> (harpoon_core::World, EntityId, EntityId, String) {
    let (mut w, observer, target, _) = fcs_motion_fixture();
    let friend = w
        .entities
        .values()
        .find(|e| e.name == "independent ranging observer without source TDL")
        .unwrap();
    let friend_id = friend.id;
    let visual = friend.sensors[0].clone();
    let sensor = visual.sensor.clone();
    w.entity_mut(observer).unwrap().sensors = vec![visual];
    while [observer, friend_id].iter().any(|oid| {
        [0, 180, 360].iter().any(|at| {
            d::trial(
                &mut d::State::default(),
                w.seed,
                *at,
                &format!("5.8:{}:{sensor}:{}", oid.0, target.0),
                180,
            )
            .0 > 50
        })
    }) {
        w.seed += 1;
    }
    (w, observer, target, sensor)
}
#[test]
fn actual_fcs_own_range_clock_passes_at_180_seconds_restores_and_ignores_fused_motion() {
    let (mut w, observer, target, sensor) = fcs_own_range_fixture();
    for _ in 0..90 {
        assert!(!w.over);
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let mut copy: harpoon_core::World =
        serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    for item in [&mut w, &mut copy] {
        for _ in 0..90 {
            assert!(!item.over);
            harpoon_core::sim::tick(item, &[]);
        }
        assert!(
            harpoon_core::rules::firecontrol2026::world_solution(item, observer, target, 3, 561.)
                .is_err(),
            "tick180 is only179 seconds after first local sample"
        );
        harpoon_core::sim::tick(item, &[]);
        let track = &item.rules.aaw.as_ref().unwrap().firecontrol.tracks
            [&format!("{}:{sensor}:{}", observer.0, target.0)];
        let motion = track.motion.as_ref().unwrap();
        assert!(motion.known);
        assert!((motion.speed_kts.unwrap() - 10.).abs() < 1e-8);
        let before = serde_json::to_value(
            harpoon_core::rules::firecontrol2026::world_solution(item, observer, target, 3, 561.)
                .unwrap(),
        )
        .unwrap();
        let contact = item.sides[0]
            .contacts
            .values_mut()
            .find(|c| c.about == target)
            .unwrap();
        contact.estimate = harpoon_core::Vec2::new(10000., 10000.);
        contact.est_speed_kts = 999.;
        contact.est_course_deg = 270.;
        item.rules.operations.as_mut().unwrap().motion.clear();
        let after = serde_json::to_value(
            harpoon_core::rules::firecontrol2026::world_solution(item, observer, target, 3, 561.)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            before, after,
            "single-platform range/speed/zig must come from its own projected reports"
        );
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&copy).unwrap()
    );
}
#[test]
fn actual_fcs_lost_own_ranging_restarts_local_motion_despite_friends_continued_picture() {
    let (mut w, observer, target, sensor) = fcs_own_range_fixture();
    for _ in 0..181 {
        assert!(!w.over);
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(
        harpoon_core::rules::firecontrol2026::world_solution(&w, observer, target, 3, 561.).is_ok()
    );
    w.entity_mut(observer)
        .unwrap()
        .ext
        .insert(format!("damage.sensor.{sensor}"), 1.);
    harpoon_core::sim::tick(&mut w, &[]);
    assert!(
        harpoon_core::rules::firecontrol2026::world_solution(&w, observer, target, 3, 561.)
            .is_err()
    );
    w.entity_mut(observer)
        .unwrap()
        .ext
        .remove(&format!("damage.sensor.{sensor}"));
    harpoon_core::sim::tick(&mut w, &[]);
    let key = format!("{}:{sensor}:{}", observer.0, target.0);
    assert!(
        !w.rules.aaw.as_ref().unwrap().firecontrol.tracks[&key]
            .motion
            .as_ref()
            .unwrap()
            .known
    );
    assert_eq!(
        harpoon_core::rules::operations::motion_known(&w, harpoon_core::Side::Blue, target),
        Some(true)
    );
    assert!(
        harpoon_core::rules::firecontrol2026::world_solution(&w, observer, target, 3, 561.)
            .is_err()
    );
    let mut copy: harpoon_core::World =
        serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    for item in [&mut w, &mut copy] {
        for _ in 0..179 {
            assert!(!item.over);
            harpoon_core::sim::tick(item, &[]);
        }
        assert!(harpoon_core::rules::firecontrol2026::world_solution(
            item, observer, target, 3, 561.
        )
        .is_err());
        harpoon_core::sim::tick(item, &[]);
        assert!(harpoon_core::rules::firecontrol2026::world_solution(
            item, observer, target, 3, 561.
        )
        .is_ok());
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&copy).unwrap()
    );
}
#[test]
fn legacy_fcs_track_without_report_defaults_preserve_serialized_shape_and_block_geometry() {
    let old = json!({"observer":1,"target":2,"sensor":"old_fixture","first":0,"last":180,"turn_course":0.,"turn_speed":0.,"last_turn":1,"zig_until":0});
    let track: harpoon_core::rules::firecontrol2026::Track =
        serde_json::from_value(old.clone()).unwrap();
    assert!(track.report.is_none() && track.motion.is_none());
    assert_eq!(serde_json::to_value(track).unwrap(), old);
}
#[test]
fn visual_continuous_periscope_applies_printed_d100_modifier_literally() {
    //5.8.5 says add20 to D100, not add20 to probability. Sea4 feathering20%
    // therefore has no successful raw D100 value when continuously exposed.
    assert_eq!(d::periscope_visual_probability(4, 5., true).unwrap(), 0.0);
}
#[test]
fn actual_land_visual_scan_consumes_structure_class_and_camouflage() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    w.entity_mut(ids[1]).unwrap().kind = PlatformKind::Base;
    let sensor = w
        .entity(ids[0])
        .unwrap()
        .sensors
        .iter()
        .find(|s| w.db.sensors[&s.sensor].kind == harpoon_core::db::SensorKind::Visual)
        .unwrap()
        .sensor
        .clone();
    let mut state = w.rules.detection.take().unwrap();
    let mut environment = env();
    environment.visibility_percent = 100;
    state.config.environment = Some(environment);
    let mut structure = explicit_target();
    structure.size = 4;
    state.config.targets.insert("cn_001".into(), structure);
    state.config.camouflage.insert("cn_001".into(), 3);
    assert!(
        !d::evaluate_world(&w, &mut state, ids[0], &sensor, ids[1])
            .unwrap()
            .detected
    );
    state.fired.insert(ids[1]);
    w.entity_mut(ids[1]).unwrap().pos = harpoon_core::Vec2::new(0.5, 0.0);
    let seen = d::evaluate_world(&w, &mut state, ids[0], &sensor, ids[1]).unwrap();
    assert!(seen.detected);
    assert_eq!(seen.trace["base_kyd"], 4.0);
}
#[test]
fn source_isar_capability_enables_ship_class_without_changing_detection_range() {
    let w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let sensor = "cn_ljq364";
    let mut state = d::State::default();
    state.config.environment = Some(env());
    let ordinary = d::evaluate_world(&w, &mut state, ids[0], sensor, ids[1]).unwrap();
    assert!(ordinary.detected);
    assert!(!ordinary.identify_class);
    state.config.bindings.insert(
        format!("us_ford:{sensor}"),
        d::Receiver::RadarFeatures {
            source: "5.2.3 capability fixture only; no actual Ford fit claim".into(),
            sar: false,
            isar: true,
            nctr: false,
        },
    );
    let recognized = d::evaluate_world(&w, &mut state, ids[0], sensor, ids[1]).unwrap();
    assert!(recognized.detected && recognized.identify_class);
    assert_eq!(ordinary.trace["range_nm"], recognized.trace["range_nm"]);
}

#[test]
fn actual_formation_radar_reports_one_unresolved_echo_and_survives_save() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    {
        let o = w.entity_mut(ids[0]).unwrap();
        o.class = "us_f18e".into();
        o.kind = PlatformKind::Aircraft;
        o.altitude_m = 3000.;
        o.course_deg = 90.;
        o.ordered_altitude_m = 3000.;
        o.ordered_course_deg = 90.;
        o.weapons.clear();
        o.sensors = vec![harpoon_core::world::SensorInstance {
            sensor: "radar_air_med".into(),
            active: true,
        }];
    }
    {
        let t = w.entity_mut(ids[1]).unwrap();
        t.class = "us_f35c".into();
        t.kind = PlatformKind::Aircraft;
        t.altitude_m = 3000.;
        t.ordered_altitude_m = 3000.;
        t.course_deg = 270.;
        t.pos = harpoon_core::Vec2::new(50., 0.);
        t.weapons.clear();
        t.sensors.clear();
    }
    let template = w.entity(ids[1]).unwrap().clone();
    let mut group = vec![ids[1]];
    for j in 0..8 {
        let mut a = template.clone();
        a.id = EntityId(500 + j);
        group.push(a.id);
        w.entities.insert(a.id, a);
    }
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .groups
        .insert("enemy-private-flight-mission-name".into(), group);
    harpoon_core::sensors::update_sensors(&mut w);
    assert_eq!(
        w.sides[0].contacts.len(),
        1,
        "nine aircraft must not disclose nine separate blips from one group signature"
    );
    let c = w.sides[0].contacts.values().next().unwrap();
    assert!(c.class_name.is_none());
    let cid = c.id;
    let mut restored: harpoon_core::World =
        serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    w.tick = 1;
    restored.tick = 1;
    harpoon_core::sensors::update_sensors(&mut w);
    harpoon_core::sensors::update_sensors(&mut restored);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert_eq!(w.sides[0].contacts.values().next().unwrap().id, cid);
}

#[test]
fn active_sonar_echo_never_runs_passive_classification() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let sensor = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::SonarActive,
        d::Receiver::Sonar {
            source: "5.4.6.6 active/passive boundary fixture".into(),
            generation: 5,
            passive_range_nm: Some(10.),
            active_range_nm: Some(10.),
            frequency: d::Frequency::Lf,
            mount: d::SonarMount::Bow,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    w.entity_mut(ids[0]).unwrap().course_deg = 90.;
    w.entity_mut(ids[0]).unwrap().speed_kts = 0.;
    w.entity_mut(ids[1]).unwrap().speed_kts = 0.;
    w.entity_mut(ids[1]).unwrap().pos = harpoon_core::Vec2::new(0.1, 0.);
    let mut state = w.rules.detection.take().unwrap();
    let mut environment = env();
    environment.layer = None;
    state.config.environment = Some(environment);
    state
        .config
        .targets
        .insert("cn_001".into(), explicit_target());
    let first = d::evaluate_world(&w, &mut state, ids[0], &sensor, ids[1]).unwrap();
    assert!(first.detected);
    w.tick = 180;
    let echo = d::evaluate_world(&w, &mut state, ids[0], &sensor, ids[1]).unwrap();
    assert!(echo.detected);
    assert!(
        !echo.identify_class,
        "active echo is not passive narrowband machinery analysis"
    );
    assert!(echo.trace.get("classification").is_none());
}

#[test]
fn air_uses_live_deployed_array_depth_and_rejects_recovered_emitter() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let air = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::SonarPassive,
        d::Receiver::AcousticIntercept {
            source: "5.4.5 AIR fixture".into(),
            generation: 5,
        },
    );
    let ping = install_receiver(
        &mut w,
        ids[1],
        harpoon_core::db::SensorKind::SonarActive,
        d::Receiver::Sonar {
            source: "5.4.2/5.4.5 VDS fixture".into(),
            generation: 5,
            passive_range_nm: Some(4.),
            active_range_nm: Some(4.),
            frequency: d::Frequency::Lmf,
            mount: d::SonarMount::Vds,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    let mut state = w.rules.detection.take().unwrap();
    let mut environment = env();
    environment.layer = None;
    state.config.environment = Some(environment);
    state.arrays.insert(
        format!("{}:{ping}", ids[1].0),
        d::ArrayJob {
            recovering: false,
            ready_at: 0,
            deployed: true,
            lost: false,
            depth_m: 100.,
        },
    );
    let hit = d::evaluate_world(&w, &mut state, ids[0], &air, ids[1]).unwrap();
    assert!(hit.detected && hit.range_known);
    state
        .arrays
        .get_mut(&format!("{}:{ping}", ids[1].0))
        .unwrap()
        .deployed = false;
    let off = d::evaluate_world(&w, &mut state, ids[0], &air, ids[1]).unwrap();
    assert!(!off.detected, "a recovered VDS cannot radiate");
}

fn dipping_fixture() -> (harpoon_core::World, EntityId, EntityId, String) {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    let sonar = install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::SonarActive,
        d::Receiver::Sonar {
            source: "5.4.3 dipping mechanism fixture; not an actual Ford fit".into(),
            generation: 5,
            passive_range_nm: Some(10.),
            active_range_nm: Some(10.),
            frequency: d::Frequency::Lmf,
            mount: d::SonarMount::Dipping,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    w.entity_mut(ids[0]).unwrap().kind = PlatformKind::Aircraft;
    w.entity_mut(ids[0]).unwrap().altitude_m = 30.;
    w.entity_mut(ids[0]).unwrap().speed_kts = 0.;
    w.entity_mut(ids[1]).unwrap().speed_kts = 0.;
    w.entity_mut(ids[1]).unwrap().pos = harpoon_core::Vec2::new(0.1, 0.);
    let state = w.rules.detection.as_mut().unwrap();
    let mut environment = env();
    environment.layer = None;
    state.config.environment = Some(environment);
    state
        .config
        .dipping_platforms
        .insert("us_ford".into(), d::DippingPlatform::Helicopter);
    state
        .config
        .targets
        .insert("cn_001".into(), explicit_target());
    (w, ids[0], ids[1], sonar)
}

#[test]
fn dipping_same_tick_deployment_scan_and_recovery_use_real_action_path() {
    let (mut w, observer, target, sonar) = dipping_fixture();
    harpoon_core::sensors::update_sensors(&mut w);
    assert!(w.sides[0].contacts.is_empty(), "stowed sonar cannot scan");
    d::apply(
        &mut w,
        observer,
        &d::DetectionAction::Dip {
            sensor: sonar.clone(),
            deploy: true,
            depth_m: 100.,
        },
    )
    .unwrap();
    assert_eq!(
        d::array_speed_limit(&w, observer),
        None,
        "a dipping cable snaps if moved; it does not impose the towed-array 25kt limit"
    );
    harpoon_core::sensors::update_sensors(&mut w);
    assert_eq!(w.sides[0].contacts.len(), 1);
    assert!(w.sides[0]
        .contacts
        .values()
        .any(|c| c.about == target && !c.bearing_only));
    assert!(w.events.iter().filter(|e| matches!(&e.kind, harpoon_core::EventKind::RulesEvent { domain, .. } if domain == "detection2026")).all(|e| matches!(e.vis, harpoon_core::Visibility::Referee)));
    d::apply(
        &mut w,
        observer,
        &d::DetectionAction::Dip {
            sensor: sonar.clone(),
            deploy: false,
            depth_m: 100.,
        },
    )
    .unwrap();
    let mut state = w.rules.detection.take().unwrap();
    assert!(d::evaluate_world(&w, &mut state, observer, &sonar, target).is_err());
}

#[test]
fn dipping_motion_break_survives_full_world_save_and_cannot_be_redeployed() {
    let (mut w, observer, _, sonar) = dipping_fixture();
    d::apply(
        &mut w,
        observer,
        &d::DetectionAction::Dip {
            sensor: sonar.clone(),
            deploy: true,
            depth_m: 100.,
        },
    )
    .unwrap();
    let mut restored: harpoon_core::World =
        serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for world in [&mut w, &mut restored] {
        world.tick = 1;
        // Position change matters even when a controller has stopped the aircraft
        // again before the next scan pass.
        world.entity_mut(observer).unwrap().pos = harpoon_core::Vec2::new(0.001, 0.);
        harpoon_core::sensors::update_sensors(world);
        assert!(
            world.rules.detection.as_ref().unwrap().arrays[&format!("{}:{sonar}", observer.0)].lost
        );
        assert!(d::apply(
            world,
            observer,
            &d::DetectionAction::Dip {
                sensor: sonar.clone(),
                deploy: true,
                depth_m: 100.
            }
        )
        .is_err());
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn dipping_airframe_and_stationary_envelopes_are_explicit() {
    let (mut w, observer, _, sonar) = dipping_fixture();
    let action = d::DetectionAction::Dip {
        sensor: sonar,
        deploy: true,
        depth_m: 100.,
    };
    w.entity_mut(observer).unwrap().speed_kts = 0.01;
    assert!(d::apply(&mut w, observer, &action).is_err());
    w.entity_mut(observer).unwrap().speed_kts = 0.;
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .dipping_platforms
        .clear();
    assert!(d::apply(&mut w, observer, &action).is_err());
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .dipping_platforms
        .insert("us_ford".into(), d::DippingPlatform::FlyingBoat);
    assert!(d::apply(&mut w, observer, &action).is_err());
    w.entity_mut(observer).unwrap().altitude_m = 0.;
    d::apply(&mut w, observer, &action).unwrap();
}

#[test]
fn dipping_command_tick_and_save_restore_share_identical_physical_state() {
    use harpoon_core::{Command, Side};
    let (mut w, observer, _, sonar) = dipping_fixture();
    w.entity_mut(observer).unwrap().kind = PlatformKind::Ship;
    for entity in w.entities.values_mut() {
        entity.ordered_speed_kts = 0.;
        entity.speed_kts = 0.;
        entity.weapons.clear();
        entity.fire_on = false;
    }
    let commands = [(
        Side::Blue,
        Command::Detection {
            unit: observer,
            action: d::DetectionAction::Dip {
                sensor: sonar.clone(),
                deploy: true,
                depth_m: 100.,
            },
        },
    )];
    let mut replay: harpoon_core::World =
        serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    harpoon_core::sim::tick(&mut w, &commands);
    harpoon_core::sim::tick(&mut replay, &commands);
    assert_eq!(w.sides[0].contacts.len(), 1);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&replay).unwrap()
    );
    let mut restored: harpoon_core::World =
        serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let commands = [(
        Side::Blue,
        Command::Detection {
            unit: observer,
            action: d::DetectionAction::Dip {
                sensor: sonar,
                deploy: false,
                depth_m: 100.,
            },
        },
    )];
    harpoon_core::sim::tick(&mut w, &commands);
    harpoon_core::sim::tick(&mut restored, &commands);
    assert!(w.rules.detection.as_ref().unwrap().arrays.is_empty());
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn fifth_generation_air_ranging_does_not_cancel_printed_bearing_uncertainty() {
    let mut w = fixture();
    let ids: Vec<_> = w.entities.keys().copied().collect();
    install_receiver(
        &mut w,
        ids[0],
        harpoon_core::db::SensorKind::SonarPassive,
        d::Receiver::AcousticIntercept {
            source: "5.4.5 Gen5 AIR projection fixture".into(),
            generation: 5,
        },
    );
    let sonar = install_receiver(
        &mut w,
        ids[1],
        harpoon_core::db::SensorKind::SonarActive,
        d::Receiver::Sonar {
            source: "5.4.5 active emitter fixture".into(),
            generation: 5,
            passive_range_nm: None,
            active_range_nm: Some(4.),
            frequency: d::Frequency::Lmf,
            mount: d::SonarMount::Bow,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    let _ = sonar;
    let mut environment = env();
    environment.layer = None;
    w.rules.detection.as_mut().unwrap().config.environment = Some(environment);
    harpoon_core::sensors::update_sensors(&mut w);
    let c = w.sides[0].contacts.values().next().unwrap();
    let observer = w.entity(ids[0]).unwrap();
    let target = w.entity(ids[1]).unwrap();
    assert!(!c.bearing_only);
    let report = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .find(|t| t.observer == ids[0] && t.target == ids[1])
        .unwrap()
        .report
        .as_ref()
        .unwrap();
    assert_eq!(
        report.measured_position,
        Some(c.estimate),
        "public icon and private receiver record must reuse one angular projection"
    );
    assert_eq!(report.bearing_error_deg, Some(3.));
    assert!(
        c.estimate.range_to(target.pos) > 1e-8,
        "12nm ranging capability must not remove the printed ±3degree bearing error"
    );
    assert!((observer.pos.range_to(c.estimate) - observer.pos.range_to(target.pos)).abs() < 1e-10);
    assert!(
        harpoon_core::geom::angle_diff(
            observer.pos.bearing_to(c.estimate),
            observer.pos.bearing_to(target.pos)
        )
        .abs()
            <= 3.
    );
    assert!(w.events.iter().any(|e|matches!(&e.kind, harpoon_core::EventKind::Detection { side: harpoon_core::Side::Blue, category, .. } if category == "unknown")), "range evidence does not identify the acoustic emitter's host category");
    assert!(harpoon_core::observe(&w, harpoon_core::Side::Blue, 0, 100)
        .contacts
        .iter()
        .all(|c| c.category.is_none()));
}

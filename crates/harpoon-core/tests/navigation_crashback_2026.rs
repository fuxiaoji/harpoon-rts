//! English2026 PDF23/3-1 §3.1.2, speed table PDF24/3-2.
//! Declared RTS maneuver: one risk trial at its first actual deceleration.
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
fn fixture(seed: u64) -> (World, EntityId) {
    let sc:Scenario=serde_json::from_value(json!({"id":"crash-back","name":"Source crash back mechanism fixture","seed":seed,"duration_s":20000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},"navigation_2026":{"source":"scenario_assumption: explicit D size nonCPP source mechanism, not a Ford propulsion claim","units":{"SHIP":{"source":"scenario_assumption: source D fixture","size":"D","merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,"undamaged_max_kts":30,"propulsion":"NonSteam","readiness":"Underway"}}}},"units":[{"name":"SHIP","side":"Blue","class":"us_ford","pos":[0,0],"course_deg":0,"speed_kts":20,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(
        sc,
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap(),
    )
    .unwrap();
    let id = *w.entities.keys().next().unwrap();
    let e = w.entity_mut(id).unwrap();
    e.sensors.clear();
    e.weapons.clear();
    (w, id)
}
fn order(id: EntityId, enabled: bool) -> (Side, Command) {
    (
        Side::Blue,
        serde_json::from_value(
            json!({"cmd":"navigation","unit":id,"action":{"type":"crash_back","enabled":enabled}}),
        )
        .expect("source Crash Back must be a real manually available command"),
    )
}
fn trials(w: &World) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "3.1.2.crash_back_trial" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn seed_for(roll: u8) -> u64 {
    (1..100000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(0, 1, "3.1.2.crash_back:0"));
            1 + (r.next_f64() * 100.) as u8 == roll
        })
        .unwrap()
}
#[test]
fn crash_back_doubles_source_deceleration_before_the_first_real_movement() {
    let (mut w, id) = fixture(seed_for(6));
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    let e = w.entity(id).unwrap();
    assert!((e.speed_kts - (20. - 30. / 180.)).abs() < 1e-9);
    assert!((e.pos.y - e.speed_kts / 3600.).abs() < 1e-9);
    assert_eq!(e.course_deg, 0.);
    assert_eq!(e.ordered_speed_kts, 0.);
    for _ in 1..60 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!((w.entity(id).unwrap().speed_kts - 10.).abs() < 1e-9);
}
#[test]
fn raw_five_causes_one_real_engineering_critical_and_raw_six_does_not() {
    for roll in [5, 6] {
        let (mut w, id) = fixture(seed_for(roll));
        let hp = w.entity(id).unwrap().hp;
        harpoon_core::sim::tick(&mut w, &[order(id, true)]);
        let t = trials(&w);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0]["d100"], roll);
        assert_eq!(t[0]["chance_percent"], 5);
        assert_eq!(
            w.entity(id)
                .unwrap()
                .ext
                .get("damage.engine_levels")
                .copied()
                .unwrap_or(0.),
            if roll == 5 { 1. } else { 0. }
        );
        assert_eq!(w.entity(id).unwrap().hp, hp);
        assert!(
            !w.events
                .iter()
                .any(|e| matches!(&e.kind, EventKind::Impact { .. })),
            "a maneuver critical is not ordinary hit DP"
        );
    }
}
#[test]
fn engineering_from_crash_back_excludes_only_the_fire_part() {
    let (mut w, id) = fixture(seed_for(5));
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    let e = w.entity(id).unwrap();
    assert_eq!(e.ext.get("damage.engine_hits"), Some(&1.));
    assert!(!w.events.iter().any(
        |e| matches!(&e.kind,EventKind::RulesEvent{detail,..}if detail.contains("fire_severity"))
    ));
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{detail,..}if detail.contains("engineering_electrical"))),"engineering's electrical part must still execute");
    assert!(!w
        .rules
        .casualties
        .get(&id)
        .is_some_and(|c| c.iter().any(|c| !c.flood)));
}
#[test]
fn repeated_orders_and_crossing_a_turn_do_not_reroll_the_same_maneuver() {
    let (mut w, id) = fixture(seed_for(6));
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    for _ in 0..250 {
        harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    }
    assert_eq!(trials(&w).len(), 1);
    assert_eq!(w.entity(id).unwrap().speed_kts, 0.);
    assert!(w.entity(id).unwrap().pos.y > 0.);
}
#[test]
fn a_stationary_request_does_not_roll_or_start_reverse_movement() {
    let (mut w, id) = fixture(seed_for(5));
    {
        let e = w.entity_mut(id).unwrap();
        e.speed_kts = 0.;
        e.ordered_speed_kts = 0.;
    }
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    assert!(trials(&w).is_empty());
    assert_eq!(w.entity(id).unwrap().pos, harpoon_core::Vec2::zero());
}
#[test]
fn all_crash_back_ships_cavitate_and_cancel_clears_the_maneuver_noise() {
    let (mut w, id) = fixture(seed_for(6));
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    assert_eq!(
        w.entity(id)
            .unwrap()
            .ext
            .get("navigation.crash_back_cavitation"),
        Some(&1.)
    );
    harpoon_core::sim::tick(&mut w, &[order(id, false)]);
    assert!(!w
        .entity(id)
        .unwrap()
        .ext
        .contains_key("navigation.crash_back_cavitation"));
}
#[test]
fn mid_maneuver_world_and_original_trial_restore_without_red_disclosure() {
    let (mut w, id) = fixture(seed_for(6));
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    let mut copy: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    copy.rules.validate_tables().unwrap();
    for _ in 0..50 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut copy, &[]);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&copy).unwrap()
    );
    assert_eq!(trials(&w).len(), 1);
    let red = harpoon_core::observe(&w, Side::Red, 0, 1000);
    assert!(red.units.is_empty());
    assert!(!red
        .events
        .iter()
        .any(|e| matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule.starts_with("3.1.2"))));
}

#[test]
fn crash_back_cavitation_is_consumed_by_a_real_passive_sonar_scan_even_for_a_pumpjet_ship() {
    use harpoon_core::rules::detection2026 as d;
    let (mut w, id) = fixture(seed_for(6));
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    let observer = EntityId(50);
    let mut e = w.entity(id).unwrap().clone();
    e.id = observer;
    e.side = Side::Red;
    e.pos = harpoon_core::Vec2::new(0., -2.);
    e.speed_kts = 0.;
    e.ordered_speed_kts = 0.;
    e.ext.clear();
    let mut sensor = w.db.sensors.values().next().unwrap().clone();
    sensor.id = "scenario_assumption_passive".into();
    sensor.kind = harpoon_core::db::SensorKind::SonarPassive;
    e.sensors = vec![harpoon_core::world::SensorInstance {
        sensor: sensor.id.clone(),
        active: true,
    }];
    w.db.sensors.insert(sensor.id.clone(), sensor.clone());
    w.entities.insert(observer, e);
    let mut s = d::State::default();
    s.config.environment = Some(d::Environment {
        source: "scenario_assumption: quiet direct acoustic path".into(),
        sea_state: 0,
        precipitation: 0,
        visibility_percent: 100,
        shipping: 0,
        biologics: 0,
        water_depth_m: 3000.,
        layer: None,
        ice: 0,
        cloud: None,
    });
    s.config.bindings.insert(
        format!("us_ford:{}", sensor.id),
        d::Receiver::Sonar {
            source: "scenario_assumption: source mechanism receiver, not actual Ford fit".into(),
            generation: 3,
            passive_range_nm: Some(10.),
            active_range_nm: None,
            frequency: d::Frequency::Mf,
            mount: d::SonarMount::Keel,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
    s.config.targets.insert(
        "us_ford".into(),
        d::TargetProfile {
            source: "scenario_assumption: controlled extremely quiet pumpjet hull".into(),
            size: 3,
            noise: Some(4),
            anechoic: false,
            titanium: false,
            pumpjet: true,
            stealthy_ir: false,
            radar_signature: None,
        },
    );
    let noisy = d::evaluate_world(&w, &mut s.clone(), observer, &sensor.id, id).unwrap();
    w.entity_mut(id)
        .unwrap()
        .ext
        .remove("navigation.crash_back_cavitation");
    let normal = d::evaluate_world(&w, &mut s, observer, &sensor.id, id).unwrap();
    assert!(
        noisy.trace["inputs"].get("target_modifier").is_some(),
        "source sonar path was not exercised: {}",
        noisy.trace
    );
    assert!(
        normal.trace["inputs"].get("target_modifier").is_some(),
        "baseline source sonar path was not exercised: {}",
        normal.trace
    );
    assert_eq!(
        noisy.trace["inputs"]["target_modifier"].as_i64().unwrap()
            - normal.trace["inputs"]["target_modifier"].as_i64().unwrap(),
        4,
        "3.1.2 all ships cavitate; source5.4.6.5 +4 must reach the actual scan"
    );
}

#[test]
fn cancelling_then_starting_a_distinct_maneuver_gets_a_new_saved_trial() {
    let (mut w, id) = fixture(seed_for(6));
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    harpoon_core::sim::tick(&mut w, &[order(id, false)]);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: id,
                speed_kts: 20.,
            },
        )],
    );
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    let t = trials(&w);
    assert_eq!(t.len(), 2);
    assert_ne!(t[0]["identity"], t[1]["identity"]);
}

#[test]
fn unresolved_coasting_combination_is_rejected_without_trial_or_changed_speed_order() {
    let (mut w, id) = fixture(seed_for(5));
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Navigation {
                unit: id,
                action: harpoon_core::rules::navigation2026::Action::Coasting { enabled: true },
            },
        )],
    );
    let before = w.entity(id).unwrap().ordered_speed_kts;
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    assert!(trials(&w).is_empty());
    assert_eq!(w.entity(id).unwrap().ordered_speed_kts, before);
}

#[test]
fn cancel_before_movement_and_cancel_mid_maneuver_preserve_exact_braking_semantics() {
    let (mut w, id) = fixture(seed_for(6));
    harpoon_core::sim::tick(&mut w, &[order(id, true), order(id, false)]);
    assert!(trials(&w).is_empty());
    assert!((w.entity(id).unwrap().speed_kts - (20. - 15. / 180.)).abs() < 1e-9);
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    let before = w.entity(id).unwrap().speed_kts;
    harpoon_core::sim::tick(&mut w, &[order(id, false)]);
    assert!((w.entity(id).unwrap().speed_kts - (before - 15. / 180.)).abs() < 1e-9);
    assert_eq!(trials(&w).len(), 1);
}

#[test]
fn a_crash_back_is_not_an_astern_order_or_a_towed_cable_trial() {
    let (mut w, id) = fixture(seed_for(6));
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Navigation {
                unit: id,
                action: harpoon_core::rules::navigation2026::Action::Astern { enabled: true },
            },
        )],
    );
    assert!(w.rules.navigation.as_ref().unwrap().units[&id]
        .astern
        .is_none());
    assert!(w.entity(id).unwrap().pos.y > 0.);
    assert!(!w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="3.1.3.towed_loss"||rule=="3.1.3.cable_foul")));
}

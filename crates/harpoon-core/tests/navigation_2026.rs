use harpoon_core::{
    rules::damage_runtime::RuleProfile, rules::navigation2026::*, Command, Database, EntityId,
    Scenario, Side, World,
};
use serde_json::json;

fn fit(size: char, max: f64) -> Fit {
    Fit {
        source: "scenario_assumption: controlled source navigation fixture".into(),
        size,
        merchant: false,
        passenger_liner: false,
        cpp: false,
        waterjet: false,
        undamaged_max_kts: max,
        propulsion: Propulsion::NonSteam,
        readiness: Readiness::Underway,
        submarine: None,
    }
}
fn fixture(f: Fit, sea: u8, sub: bool) -> (World, EntityId) {
    let class = if sub { "us_virginia" } else { "us_ford" };
    let sc:Scenario=serde_json::from_value(json!({"id":"nav-source","name":"Source navigation test","duration_s":20000,"seed":71,"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":sea,"precipitation":"None"},"flight_environment":{"wind_from_deg":90,"wind_kts":20},"navigation_2026":{"source":"scenario_assumption: source acceleration/readiness test","units":{"Ship":f}}},"weather":{"visibility_nm":40,"jam":[0,0]},"units":[{"side":"Blue","class":class,"name":"Ship","pos":[0,0],"speed_kts":0,"course_deg":90,"depth_m":if sub {50}else{0}}]})).unwrap();
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let id = w.entities.values().find(|e| e.name == "Ship").unwrap().id;
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
#[test]
fn printed_acceleration_rows_and_thresholds() {
    let rows = [
        ('A', 24., false, [4., 2., 6.]),
        ('A', 25., false, [6., 3., 9.]),
        ('B', 30., false, [10., 5., 12.]),
        ('B', 30., true, [15., 8., 18.]),
        ('C', 30., false, [12., 6., 15.]),
        ('E', 30., true, [18., 9., 18.]),
        ('F', 24., false, [15., 8., 18.]),
        ('G', 25., false, [25., 12., 30.]),
    ];
    for (size, max, cpp, expected) in rows {
        let mut f = fit(size, max);
        f.cpp = cpp;
        assert_eq!(acceleration(&f, 0.).unwrap(), expected);
    }
    for (size, expected) in [
        ('A', [4., 3., 8.]),
        ('B', [6., 3., 10.]),
        ('D', [8., 4., 12.]),
        ('G', [12., 6., 15.]),
    ] {
        let mut f = fit(size, 30.);
        f.merchant = true;
        assert_eq!(acceleration(&f, 0.).unwrap(), expected);
        f.passenger_liner = true;
        assert_eq!(
            acceleration(&f, 0.).unwrap(),
            acceleration(&fit(size, 30.), 0.).unwrap()
        );
    }
    let mut f = fit('G', 20.);
    f.waterjet = true;
    assert_eq!(acceleration(&f, 0.).unwrap(), [25., 12., 30.]);
    let (mut w, id) = fixture(fit('A', 32.), 0, false);
    w.entity_mut(id).unwrap().speed_kts = 24.;
    assert!((motion(&mut w, id).unwrap().unwrap().acceleration_per_s - 6. / 180.).abs() < 1e-12);
    w.entity_mut(id).unwrap().speed_kts = 24.001;
    assert!((motion(&mut w, id).unwrap().unwrap().acceleration_per_s - 3. / 180.).abs() < 1e-12);
}
#[test]
fn all_source_sea_state_cells_and_surfaced_sub_column() {
    let expected = [
        [Some(1.), Some(1.), Some(1.), Some(1.), Some(1.)],
        [Some(1.), Some(1.), Some(1.), Some(1.), Some(1.)],
        [Some(1.), Some(1.), Some(1.), Some(1.), Some(1.)],
        [Some(1.), Some(1.), Some(1.), Some(1.), Some(0.75)],
        [Some(1.), Some(1.), Some(1.), Some(0.75), Some(0.5)],
        [Some(1.), Some(1.), Some(0.75), Some(0.5), Some(0.25)],
        [Some(0.75), Some(0.75), Some(0.5), Some(0.5), None],
        [Some(0.5), Some(0.5), Some(0.5), Some(0.25), None],
        [Some(0.25), Some(0.25), Some(0.25), None, None],
        [None, None, None, None, None],
    ];
    for (sea, row) in expected.iter().enumerate() {
        for (col, size) in ['A', 'B', 'C', 'D', 'F'].into_iter().enumerate() {
            assert_eq!(
                weather_fraction(size, sea as u8, false, 0.).unwrap(),
                row[col]
            );
        }
    }
    assert_eq!(weather_fraction('C', 5, true, 0.).unwrap(), Some(1.));
    assert_eq!(weather_fraction('F', 9, true, 1.).unwrap(), Some(1.));
    assert!(weather_fraction('H', 0, false, 0.).is_err());
    assert!(weather_fraction('A', 10, false, 0.).is_err());
}
#[test]
fn underway_original_dice_fractional_turns_nuclear_and_inport() {
    for (size, die, mult) in [('A', 7, 90), ('C', 7, 60), ('E', 7, 45), ('F', 5, 60)] {
        let mut f = fit(size, 20.);
        f.readiness = Readiness::ColdIron;
        assert_eq!(underway_seconds(&f, &[die]).unwrap(), die as i64 * mult);
    }
    for (size, mult) in [('A', 600), ('C', 300), ('E', 200)] {
        let mut f = fit(size, 30.);
        f.propulsion = Propulsion::Steam;
        f.readiness = Readiness::ColdIron;
        assert_eq!(underway_seconds(&f, &[2, 5]).unwrap(), 7 * mult);
        f.propulsion = Propulsion::Nuclear;
        assert_eq!(underway_seconds(&f, &[2, 5]).unwrap(), 14 * mult);
        f.readiness = Readiness::InportSteam;
        assert_eq!(underway_seconds(&f, &[2, 5]).unwrap(), 7 * mult);
    }
    let mut f = fit('F', 20.);
    f.propulsion = Propulsion::Steam;
    assert!(underway_seconds(&f, &[2, 5]).is_err());
    assert!(underway_seconds(&fit('A', 30.), &[0]).is_err());
}
#[test]
fn actual_movement_uses_source_acceleration_turn_coasting_and_sea_cap() {
    let (mut w, id) = fixture(fit('A', 32.), 0, false);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: id,
                speed_kts: 32.,
            },
        )],
    );
    tick(&mut w, 179);
    assert!((w.entity(id).unwrap().speed_kts - 6.).abs() < 1e-9);
    harpoon_core::sim::tick(
        &mut w,
        &[
            (
                Side::Blue,
                Command::Navigation {
                    unit: id,
                    action: Action::Coasting { enabled: true },
                },
            ),
            (
                Side::Blue,
                Command::SetSpeed {
                    unit: id,
                    speed_kts: 0.,
                },
            ),
        ],
    );
    tick(&mut w, 179);
    assert!((w.entity(id).unwrap().speed_kts - 1.5).abs() < 1e-9);
    let (mut x, id) = fixture(fit('A', 32.), 8, false);
    x.entity_mut(id).unwrap().speed_kts = 8.;
    x.entity_mut(id).unwrap().ordered_speed_kts = 32.;
    tick(&mut x, 180);
    assert!((x.entity(id).unwrap().speed_kts - 8.).abs() < 1e-9);
    let (mut x, id) = fixture(fit('A', 32.), 0, false);
    x.entity_mut(id).unwrap().ordered_course_deg = 180.;
    x.entity_mut(id).unwrap().ordered_speed_kts = 32.;
    tick(&mut x, 90);
    assert!((x.entity(id).unwrap().speed_kts - 1.5).abs() < 1e-9);
}
#[test]
fn underway_command_real_clock_no_reroll_save_and_own_observation() {
    let mut f = fit('A', 32.);
    f.readiness = Readiness::ColdIron;
    let (mut w, id) = fixture(f, 0, false);
    harpoon_core::sim::tick(
        &mut w,
        &[
            (
                Side::Blue,
                Command::SetSpeed {
                    unit: id,
                    speed_kts: 32.,
                },
            ),
            (
                Side::Blue,
                Command::Navigation {
                    unit: id,
                    action: Action::Underway,
                },
            ),
        ],
    );
    let u = w.rules.navigation.as_ref().unwrap().units[&id].clone();
    let due = u.ready_at.unwrap();
    assert!((91..=901).contains(&due));
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Navigation {
                unit: id,
                action: Action::Underway,
            },
        )],
    );
    assert_eq!(
        w.rules.navigation.as_ref().unwrap().units[&id].ready_at,
        Some(due)
    );
    assert_eq!(
        w.rules.navigation.as_ref().unwrap().units[&id].underway_dice,
        u.underway_dice
    );
    tick(&mut w, 20);
    assert_eq!(w.entity(id).unwrap().speed_kts, 0.);
    let mut copy: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    copy.rules.validate_tables().unwrap();
    let remaining = (due - 1 - w.tick) as usize;
    tick(&mut w, remaining);
    tick(&mut copy, remaining);
    assert_eq!(w.entity(id).unwrap().speed_kts, 0.);
    tick(&mut w, 1);
    tick(&mut copy, 1);
    assert!(w.entity(id).unwrap().speed_kts > 0.);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&copy).unwrap()
    );
    let blue = harpoon_core::observe(&w, Side::Blue, 0, 100);
    assert!(blue
        .units
        .iter()
        .find(|u| u.id == id)
        .unwrap()
        .navigation
        .as_ref()
        .unwrap()
        .source_rules
        .is_some());
    let red = harpoon_core::observe(&w, Side::Red, 0, 100);
    assert!(red.units.is_empty());
}
#[test]
fn heave_to_unknown_sinking_time_blocks_and_quiet_changes_actual_acceleration() {
    let (mut w, id) = fixture(fit('G', 20.), 9, false);
    w.entity_mut(id).unwrap().ordered_speed_kts = 20.;
    tick(&mut w, 1);
    assert_eq!(w.entity(id).unwrap().speed_kts, 0.);
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..}if rule=="3.movement_blocked")));
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Navigation {
                unit: id,
                action: Action::HeaveTo { enabled: true },
            },
        )],
    );
    tick(&mut w, 179);
    assert!((w.entity(id).unwrap().speed_kts - 5.).abs() < 1e-9);
    assert_eq!(w.entity(id).unwrap().course_deg, 90.);
    let (mut w, id) = fixture(fit('C', 30.), 9, true);
    w.entity_mut(id).unwrap().ordered_speed_kts = 20.;
    tick(&mut w, 180);
    assert!((w.entity(id).unwrap().speed_kts - 6.).abs() < 1e-9);
    assert!(!w
        .entity(id)
        .unwrap()
        .ext
        .contains_key("navigation.cavitation"));
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Navigation {
                unit: id,
                action: Action::Quiet { enabled: false },
            },
        )],
    );
    assert_eq!(
        w.entity(id).unwrap().ext.get("navigation.cavitation"),
        Some(&1.)
    );
}
#[test]
fn navigation_profile_and_table_fingerprint_are_versioned() {
    let (w, _) = fixture(fit('A', 32.), 0, false);
    assert_eq!(w.rules.profile, RuleProfile::Rts2026Full);
    w.rules.validate_tables().unwrap();
    let mut x = w.clone();
    x.rules.navigation = None;
    assert!(x.rules.validate_tables().is_err());
    let mut x = w.clone();
    x.rules.table_fingerprint = Some("stale".into());
    assert!(x.rules.validate_tables().is_err());
}

#[test]
fn real_inport_steam_cannot_exceed_quarter_speed_until_original_dice_clock_finishes() {
    let mut f = fit('A', 32.);
    f.propulsion = Propulsion::Steam;
    f.readiness = Readiness::InportSteam;
    let (mut w, id) = fixture(f, 0, false);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: id,
                speed_kts: 32.,
            },
        )],
    );
    tick(&mut w, 359);
    assert!((w.entity(id).unwrap().speed_kts - 8.).abs() < 1e-9);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Navigation {
                unit: id,
                action: Action::Underway,
            },
        )],
    );
    let unit = &w.rules.navigation.as_ref().unwrap().units[&id];
    let due = unit.ready_at.unwrap();
    let dice = unit.underway_dice.clone();
    assert_eq!(
        due - w.tick,
        dice.iter().map(|v| *v as i64).sum::<i64>() * 300
    );
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    let remaining = (due - 1 - w.tick) as usize;
    tick(&mut w, remaining);
    tick(&mut restored, remaining);
    assert!((w.entity(id).unwrap().speed_kts - 8.).abs() < 1e-9);
    tick(&mut w, 1);
    tick(&mut restored, 1);
    assert!(w.entity(id).unwrap().speed_kts > 8.);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

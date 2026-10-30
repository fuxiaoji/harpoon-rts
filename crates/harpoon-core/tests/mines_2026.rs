use harpoon_core::rules::mines2026 as m;
#[test]
fn mine_table_uses_actual_source_columns_and_shared_eg_row() {
    assert_eq!(m::probability(false, 0, 50).unwrap(), 0.60);
    assert_eq!(m::probability(false, 1, 75).unwrap(), 0.32);
    assert_eq!(m::probability(true, 2, 75).unwrap(), 0.35);
    for size in 4..=6 {
        assert_eq!(m::probability(true, size, 600).unwrap(), 0.01);
    }
    assert!(m::probability(false, 0, 600).is_err());
    assert!(m::probability(true, 0, 50).is_err());
    assert!(m::probability(true, 7, 75).is_err());
}
#[test]
fn mine_count_conserves_inventory_despite_source_example_typo() {
    assert_eq!(m::layout(5000, 125, 150, 300).unwrap(), (41, 3, 1200));
    assert!(m::layout(5001, 125, 150, 300).is_err());
    assert!(m::layout(5000, 125, 150, 199).is_err());
}
#[test]
fn sweep_is_local_reduction_with_cap_ne_and_two_helicopter_passes() {
    assert_eq!(m::sweep_remaining(false, 2, 8).unwrap(), 0.40);
    assert_eq!(m::sweep_remaining(false, 1, 1).unwrap(), 1.0);
    assert_eq!(m::sweep_remaining(false, 1, 2).unwrap(), 0.70);
    assert_eq!(m::sweep_remaining(false, 1, 16).unwrap(), 0.25);
    assert_eq!(m::sweep_remaining(true, 1, 10).unwrap(), 0.25);
    assert!(m::sweep_remaining(true, 4, 2).is_err());
}
use harpoon_core::{Database, EntityId, EventKind, Scenario, Vec2, Visibility, World};
use serde_json::json;
fn field() -> m::Field {
    serde_json::from_value(json!({"id":"secret","owner":"Red","mine":"mdm_1","area":{"start":{"x":0,"y":0},"end":{"x":5000.0/2025.3718285,"y":0},"width_yd":1200},"rows":3,"spacing_yd":125,"row_spacing_yd":300,"case_band":1,"water_band":1,"armed_at":0,"source":"scenario_assumption: source example geometry"})).unwrap()
}
fn target() -> m::Target {
    m::Target {
        size: 1,
        speed: 10.,
        warship: true,
        dwt: None,
        depth_band: 0,
        submarine: false,
        double_hull: false,
        year: 2000,
    }
}
#[test]
fn modifiers_are_source_steps_and_never_continuous_noise_estimates() {
    let mut f = field();
    f.mine = "chen_2".into();
    f.spacing_yd = 75;
    let mut t = target();
    assert_eq!(m::attack_chance(&f, &t, 0).unwrap(), 0.35);
    t.speed = 15.;
    assert_eq!(m::attack_chance(&f, &t, 0).unwrap(), 0.60);
    t.warship = false;
    assert_eq!(m::attack_chance(&f, &t, 0).unwrap(), 0.75);
    t.dwt = Some(160000.);
    assert_eq!(m::attack_chance(&f, &t, 0).unwrap(), 0.90);
    f.mine = "mao_3".into();
    assert_eq!(m::attack_chance(&f, &target(), 8).unwrap(), 0.13);
}
#[test]
fn original_attack_die_selects_optional_tier_and_submarine_hull_chances() {
    let mut f = field();
    f.optional_damage = true;
    f.case_band = 0;
    for (roll, tier_dp) in [
        (14, 235.),
        (15, 141.),
        (24, 141.),
        (25, 71.),
        (31, 71.),
        (32, 35.),
        (35, 35.),
    ] {
        assert_eq!(
            m::damage_solution(&f, &target(), roll, 0.35).unwrap().0,
            tier_dp
        );
    }
    assert!(m::damage_solution(&f, &target(), 36, 0.35).is_err());
    let mut t = target();
    t.submarine = true;
    t.depth_band = 1;
    f.optional_damage = false;
    assert_eq!(
        m::damage_solution(&f, &t, 1, 0.35)
            .unwrap()
            .1
            .pressure_percent,
        Some(50.)
    );
    t.size = 2;
    assert_eq!(
        m::damage_solution(&f, &t, 1, 0.35)
            .unwrap()
            .1
            .pressure_percent,
        Some(80.)
    );
    f.mine = "mao_1".into();
    assert_eq!(
        m::damage_solution(&f, &t, 1, 0.35)
            .unwrap()
            .1
            .pressure_percent,
        Some(100.)
    );
    t.double_hull = true;
    assert_eq!(
        m::damage_solution(&f, &t, 1, 0.35)
            .unwrap()
            .1
            .pressure_percent,
        Some(90.)
    );
}
#[test]
fn unsupported_mine_combinations_do_not_invent_damage_or_service_year() {
    let mut f = field();
    f.optional_damage = true;
    assert!(
        m::damage_solution(&f, &target(), 1, 0.35).is_err(),
        "Shallow surface Major restriction cannot silently become Full damage"
    );
    f.optional_damage = false;
    f.mine = "mao_1".into();
    let mut t = target();
    t.submarine = true;
    t.depth_band = 1;
    t.year = 0;
    assert!(
        m::damage_solution(&f, &t, 1, 0.35).is_err(),
        "unknown in-service date is not pre1955"
    );
    t.size = 7;
    assert!(
        m::attack_chance(&f, &t, 0).is_err(),
        "unknown size is not clamped intoG"
    );
}
#[test]
fn exact_danger_segments_safe_channels_and_attack_positions() {
    let mut f = field();
    let a = Vec2::new(0.1, -1000. / 2025.3718285);
    let b = Vec2::new(0.1, 1000. / 2025.3718285);
    let (p, carry) = m::attack_positions(&f, a, b, 0., 400.);
    assert_eq!(p.len(), 3);
    assert!(carry.abs() < 1e-6);
    assert!((p[0].y * 2025.3718285 + 200.).abs() < 1e-6);
    f.safe_channels.push(m::Rect {
        start: Vec2::new(0., 0.),
        end: Vec2::new(5000. / 2025.3718285, 0.),
        width_yd: 400.,
    });
    assert!((m::field_distance(&f, a, b) - 800.).abs() < 1e-6);
    let (p, _) = m::attack_positions(&f, a, b, 0., 400.);
    assert_eq!(p.len(), 2);
    assert!(p.iter().all(|p| p.y.abs() * 2025.3718285 >= 200. - 1e-6));
    assert_eq!(m::arming_seconds(1, None).unwrap(), 360);
    assert_eq!(m::arming_seconds(2, None).unwrap(), 540);
    assert!(m::arming_seconds(0, None).is_err());
}
fn fixture() -> World {
    let mut f = field();
    f.owner = harpoon_core::Side::Blue;
    let sc:Scenario=serde_json::from_value(json!({"id":"mines","name":"源水雷验收","duration_s":1000,"seed":20261023,"context":{"rules_profile":"hv2026-rts-v4","mines_2026":{"fields":[f],"source":"source example fixture"}},"weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"us_ford","name":"Crossing","pos":[0.1,-0.4],"speed_kts":30,"course_deg":0}]})).unwrap();
    harpoon_core::load_scenario(sc, Database::built_in()).unwrap()
}
#[test]
fn actual_movement_secret_attacks_friend_or_foe_and_save_resume() {
    let mut w = fixture();
    for _ in 0..60 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let attacks=w.events.iter().filter(|e|matches!(&e.kind,EventKind::RulesEvent{domain,rule,..}if domain=="mines2026"&&rule=="13.5.attack")).collect::<Vec<_>>();
    assert!(
        attacks.len() >= 2,
        "actual movement must execute mine attacks even against field owner's ship"
    );
    assert!(attacks.iter().all(|e| e.vis == Visibility::Referee));
    let obs = harpoon_core::observe::observe(&w, harpoon_core::Side::Blue, 0, 5000);
    let public = serde_json::to_string(&obs).unwrap();
    assert!(!public.contains("\"field\":\"secret\""));
    assert!(!public.contains("mdm_1"));
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..30 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
    assert!(w.entity(EntityId(1)).unwrap().hp < w.entity(EntityId(1)).unwrap().hp_max);
}
fn sweeper() -> World {
    let mut w = fixture();
    w.duration_s = 20000;
    w.rules.mines.as_mut().unwrap().fields.clear();
    let e = w.entity_mut(EntityId(1)).unwrap();
    e.speed_kts = 0.;
    e.ordered_speed_kts = 0.;
    w.context.mines_2026.as_mut().unwrap().capabilities.insert("Crossing".into(),m::Capability{source:"scenario_assumption: generic equipped ship fixture; not a claim about Ford equipment".into(),gear:["oropesa".into(),"magnetic".into(),"acoustic".into()].into_iter().collect(),combined:false,helicopter_stream_s:None});
    w
}
#[test]
fn real_sweep_deployment_tow_cap_short_stay_and_restream_survive_load() {
    use harpoon_core::{Command, Side};
    let mut w = sweeper();
    let id = EntityId(1);
    harpoon_core::sim::tick(
        &mut w,
        &[
            (
                Side::Blue,
                Command::Mines {
                    unit: id,
                    action: m::Action::Stream {
                        gear: ["oropesa".into()].into_iter().collect(),
                        partner: None,
                    },
                },
            ),
            (
                Side::Blue,
                Command::SetSpeed {
                    unit: id,
                    speed_kts: 30.,
                },
            ),
        ],
    );
    assert_eq!(w.rules.mines.as_ref().unwrap().gear[&id].due, 3601);
    for _ in 0..1799 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.entity(id).unwrap().speed_kts <= 5. + 1e-8);
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    while w.tick < 3601 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert_eq!(w.rules.mines.as_ref().unwrap().gear[&id].phase, "active");
    for _ in 0..200 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.entity(id).unwrap().speed_kts <= 15. + 1e-8);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetCourse {
                unit: id,
                course_deg: 90.,
            },
        )],
    );
    assert!(
        w.entity(id).unwrap().course_deg.abs() < 1e-6,
        "streamed Oropesa cannot skip short stay"
    );
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Mines {
                unit: id,
                action: m::Action::Turn { course_deg: 45. },
            },
        )],
    );
    let due = w.rules.mines.as_ref().unwrap().gear[&id].due;
    while w.tick < due {
        assert!(w.entity(id).unwrap().course_deg.abs() < 1e-6);
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let mut n = 0;
    while w.rules.mines.as_ref().unwrap().gear[&id].phase == "turning" {
        harpoon_core::sim::tick(&mut w, &[]);
        n += 1;
        assert!(n < 1000);
    }
    let g = &w.rules.mines.as_ref().unwrap().gear[&id];
    assert_eq!(g.phase, "restream");
    assert_eq!(g.due - w.tick, 720);
    assert_eq!(w.rules.mines.as_ref().unwrap().swept.len(), 1);
    assert_eq!(w.rules.mines.as_ref().unwrap().swept[0].area.width_yd, 500.);
}
#[test]
fn simultaneous_multifuze_sweeps_need_declared_fit_and_stationary_is_no_pass() {
    let mut w = sweeper();
    let id = EntityId(1);
    let action = m::Action::Stream {
        gear: ["magnetic".into(), "acoustic".into()].into_iter().collect(),
        partner: None,
    };
    assert!(m::apply(&mut w, id, &action).is_err());
    w.context
        .mines_2026
        .as_mut()
        .unwrap()
        .capabilities
        .get_mut("Crossing")
        .unwrap()
        .combined = true;
    m::apply(&mut w, id, &action).unwrap();
    for _ in 0..1800 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(w.rules.mines.as_ref().unwrap().gear[&id].phase, "active");
    assert_eq!(m::navigation_limits(&w, id).0, Some(7.));
    assert!(m::apply(&mut w, id, &m::Action::FinishPass).is_err());
}
#[test]
fn unsupported_mobile_and_drifting_fields_never_use_regular_row_attacks() {
    for mine in ["te_5", "piao_1"] {
        let mut f = field();
        f.mine = mine.into();
        if mine == "piao_1" {
            f.case_band = 0;
            f.water_band = 0;
        }
        let sc:Scenario=serde_json::from_value(json!({"id":"unsupported-mine","name":"source rejection","duration_s":100,"seed":1,"context":{"rules_profile":"hv2026-rts-v4","mines_2026":{"fields":[f],"source":"scenario_assumption"}},"weather":{"visibility_nm":0,"jam":[0,0]},"units":[]})).unwrap();
        let result = harpoon_core::load_scenario(sc, Database::built_in());
        assert!(result.is_err(),"{mine} needs its own source movement/one-attack procedure; regular row field is not valid");
    }
}

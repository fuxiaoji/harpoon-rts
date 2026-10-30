use harpoon_core::rules::aircombat2026::*;
use harpoon_core::{Command, ContactId, Database, EntityId, Rng, Scenario, Side, World};
use serde_json::json;
fn duel(blue_class: &str) -> (World, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc:Scenario=serde_json::from_value(json!({"id":"aircombat-source","name":"Source air combat","duration_s":5000,"seed":1,"context":{"rules_profile":"hv2026-rts-v4","aviation_2026":{"source":"scenario_assumption: source-mechanism fixture","loads":{"cn_j15":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"},"us_f35c":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"}},"parking":{},"initial_fuel_nm":{"BLUE":700,"RED":700}}},"weather":{"visibility_nm":40,"jam":[0,0]},"units":[{"side":"Blue","class":blue_class,"name":"BLUE","pos":[0,0],"course_deg":90,"speed_kts":485,"altitude_m":4000,"fire_on":false},{"side":"Red","class":"us_f35c","name":"RED","pos":[2,0],"course_deg":270,"speed_kts":520,"altitude_m":4000,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let a = w
        .entities
        .values()
        .find(|e| e.side == Side::Blue)
        .unwrap()
        .id;
    let b = w
        .entities
        .values()
        .find(|e| e.side == Side::Red)
        .unwrap()
        .id;
    for id in [a, b] {
        let e = w.entity_mut(id).unwrap();
        e.weapons.clear();
        e.sensors.clear();
        e.mission = None;
    }
    for (side, enemy) in [(Side::Blue, b), (Side::Red, a)] {
        let target = w.entity(enemy).unwrap();
        let pos = target.pos;
        w.sides[side.index()].contacts.insert(
            ContactId(1),
            harpoon_core::world::Contact {
                id: ContactId(1),
                about: enemy,
                category: harpoon_core::types::PlatformKind::Aircraft,
                class_name: None,
                estimate: pos,
                est_course_deg: 0.,
                est_speed_kts: 0.,
                bearing_only: false,
                quality: 1.,
                last_update_tick: 0,
                first_tick: 0,
                stale: false,
                sensors: vec![],
                offset: Default::default(),
                announced: false,
            },
        );
    }
    (w, a, b)
}
#[test]
fn source_gun_table_exact_cells_and_unprinted_rating_blocking() {
    assert_eq!(gun_probability(2.5, 25.).unwrap(), 0.30);
    assert_eq!(gun_probability(0.05, 100.).unwrap(), 0.01);
    assert_eq!(gun_probability(6., 4.).unwrap(), 0.9);
    assert!(gun_probability(4.2, 29.).is_err());
}
#[test]
fn source_position_modifiers_and_exact_half_rounding() {
    let p = position_result(83, 2.5, 3., 920., 911., 0).unwrap();
    assert_eq!(p.gun, 80);
    assert_eq!(p.aam, 80);
    let p = position_result(50, 4., 3., 900., 750., 3).unwrap();
    assert_eq!(p.gun, 65);
    assert_eq!(p.aam, 95);
    assert_eq!(radius_for_fastest(200.).unwrap(), 1.);
    assert_eq!(radius_for_fastest(400.).unwrap(), 3.);
    assert_eq!(radius_for_fastest(401.).unwrap(), 6.);
}
#[test]
fn every_printed_gun_cell_and_published_air_ground_examples_are_consumed() {
    for row in tables()["gun_attack"].as_array().unwrap() {
        for (col, gun) in tables()["gun_ratings"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            assert_eq!(
                gun_probability(gun.as_f64().unwrap(), row[0].as_f64().unwrap()).unwrap(),
                row[col + 1].as_f64().unwrap()
            );
        }
    }
    assert_eq!(air_attack_hits(0.3, 3, 3).unwrap(), 3);
    assert_eq!(air_attack_hits(0.3, 3, 4).unwrap(), 2);
    assert_eq!(air_attack_hits(0.3, 3, 23).unwrap(), 1);
    assert_eq!(air_attack_hits(0.3, 3, 67).unwrap(), 0);
    let i = UnguidedInput {
        target_size: 'B',
        delivery: Delivery::Glide,
        altitude_m: 1000.,
        target_speed_kts: 30.,
        bombsight: Bombsight::Computing,
        heavy_aa: true,
        no_aa: false,
        rudder: false,
        land: false,
        inertial_lob: false,
    };
    assert_eq!(unguided_probability(&i).unwrap(), 0.26);
    let hits: [u8; 4] = [30, 12, 75, 1].map(|d| unguided_attack(&i, 4, d).unwrap());
    assert_eq!(hits, [1, 2, 0, 3]);
    assert_eq!(
        pgm_probability(PgmGuidance::Salh, 2, false, 2, true).unwrap(),
        0.56 / 4.
    );
    assert_eq!(rocket_hits(32, [6, 6], 'C').unwrap(), 5);
    assert!(rocket_hits(32, [6, 6], 'A').is_err());
    assert!(!satnav_release_allowed(
        1,
        7500.,
        Bombsight::Advanced,
        false,
        false
    ));
    assert!(satnav_release_allowed(
        1,
        7501.,
        Bombsight::Advanced,
        false,
        false
    ));
}
#[test]
fn real_sim_source_minute_round_kill_and_world_resume() {
    let (mut w, a, b) = duel("cn_j15");
    let seed = (0..10000)
        .find(|seed| {
            let master = Rng::new(*seed);
            let mut p = master.derive(Rng::hash_tag(61, a.0, "9.4.1.position.1.1"));
            let mut g = master.derive(Rng::hash_tag(61, a.0, "9.4.2.gun.1.1"));
            p.next_f64() >= 0.8 && g.next_f64() < 0.3
        })
        .unwrap();
    w.rng = Rng::new(seed);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::AirCombat {
                unit: a,
                action: Action::Declare {
                    contact: ContactId(1),
                },
            },
        )],
    );
    assert!(in_dogfight(&w, a));
    let fuel = w.rules.aviation.as_ref().unwrap().flights[&a].fuel_nm;
    for _ in 0..29 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    while w.tick < 60 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    assert!(w.entity(b).unwrap().alive);
    assert!(w.rules.aviation.as_ref().unwrap().flights[&a].fuel_nm < fuel - 20.);
    harpoon_core::sim::tick(&mut w, &[]);
    harpoon_core::sim::tick(&mut saved, &[]);
    assert!(
        !w.entity(b).unwrap().alive,
        "source gun roll is an immediate kill, not 34%HP"
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    assert!(w
        .events
        .iter()
        .any(|e| e.tick == 61 && matches!(e.kind, harpoon_core::EventKind::AirShotDown { .. })));
}
#[test]
fn default_aam_evasion_uses_real_maneuver_until_explicitly_declined() {
    let (mut w, a, _) = duel("cn_j15");
    assert!(maneuvering(&w, a));
    assert_eq!(target_maneuver(&w, a).unwrap(), 4.5);
    apply(&mut w, a, &Action::NonManeuvering).unwrap();
    assert!(!maneuvering(&w, a));
    assert_eq!(target_maneuver(&w, a).unwrap(), 0.);
    apply(&mut w, a, &Action::Attack).unwrap();
    assert_eq!(target_maneuver(&w, a).unwrap(), 4.5);
}
#[test]
fn real_sim_ninth_round_ends_and_departed_planes_cannot_redeclare() {
    let (mut w, a, _) = duel("us_f35c");
    let seed = (0..100000)
        .find(|seed| {
            let r = Rng::new(*seed);
            [(181, 2), (361, 4)].iter().all(|(tick, limit)| {
                ["9.4.6.end_blue", "9.4.6.end_red"].iter().all(|purpose| {
                    let mut c = r.derive(Rng::hash_tag(*tick, 1, purpose));
                    (c.next_f64() * 6.).floor() as u8 + 1 > *limit
                })
            })
        })
        .unwrap();
    w.rng = Rng::new(seed);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::AirCombat {
                unit: a,
                action: Action::Declare {
                    contact: ContactId(1),
                },
            },
        )],
    );
    while w.tick < 540 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(in_dogfight(&w, a));
    harpoon_core::sim::tick(&mut w, &[]);
    assert!(!in_dogfight(&w, a));
    assert!(w.rules.aircombat.as_ref().unwrap().must_rtb.contains(&a));
    assert!(apply(
        &mut w,
        a,
        &Action::Declare {
            contact: ContactId(1)
        }
    )
    .is_err());
}
#[test]
fn real_sim_escape_uses_d10_and_defensive_rating() {
    let (mut w, a, _) = duel("us_f35c");
    let seed = (0..10000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(61, a.0, "9.4.5.escape.1.1"));
            let mut alt =
                Rng::new(*seed).derive(Rng::hash_tag(61, a.0, "9.4.6.escape_altitude.1.1"));
            r.next_f64() >= 0.5 && alt.next_f64() < 0.2
        })
        .unwrap();
    w.rng = Rng::new(seed);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::AirCombat {
                unit: a,
                action: Action::Declare {
                    contact: ContactId(1),
                },
            },
        )],
    );
    apply(&mut w, a, &Action::Escape).unwrap();
    assert_eq!(target_maneuver(&w, a).unwrap(), 5.);
    while w.tick < 61 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(!in_dogfight(&w, a));
    assert!(w.rules.aircombat.as_ref().unwrap().must_rtb.contains(&a));
    assert_eq!(target_maneuver(&w, a).unwrap(), 4.5);
    assert_eq!(w.entity(a).unwrap().altitude_m, 1000.);
    assert!(
        apply(
            &mut w,
            EntityId(2),
            &Action::Declare {
                contact: ContactId(1)
            }
        )
        .is_err(),
        "9.4.5 successful escape cannot be pursued by the same combatant"
    );
}
#[test]
fn airborne_torpedo_source_envelope_cadence_and_two_weapon_interference() {
    let (mut w, a, b) = duel("cn_j15");
    w.entity_mut(b).unwrap().kind = harpoon_core::types::PlatformKind::Sub;
    w.entity_mut(b).unwrap().depth_m = 50.;
    w.sides[0].contacts.get_mut(&ContactId(1)).unwrap().category =
        harpoon_core::types::PlatformKind::Sub;
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert(format!("0:{}", b.0), harpoon_core::types::PlatformKind::Sub);
    assert!(air_torpedo_gate(&w, a, ContactId(1)).is_err());
    let e = w.entity_mut(a).unwrap();
    e.altitude_m = 2000.;
    e.speed_kts = 300.;
    assert!(air_torpedo_gate(&w, a, ContactId(1)).is_ok());
    w.entity_mut(a).unwrap().speed_kts = 300.1;
    assert!(air_torpedo_gate(&w, a, ContactId(1)).is_err());
    w.entity_mut(a).unwrap().speed_kts = 300.;
    air_torpedo_dropped(&mut w, a, a);
    assert!(!w
        .rules
        .aircombat
        .as_ref()
        .unwrap()
        .air_torpedo_last
        .contains_key(&a));
    let mut torpedo = w.entity(a).unwrap().clone();
    let id = w.allocate_id();
    torpedo.id = id;
    torpedo.kind = harpoon_core::types::PlatformKind::Torpedo;
    torpedo.projectile = Some(harpoon_core::world::ProjectileData {
        targeted_sensor: None,
        weapon: "test_source_fixture".into(),
        target: Some(b),
        aim: w.entity(b).unwrap().pos,
        launched_by: a,
        launched_by_side: Side::Blue,
        live_s: 0.,
        enabled: true,
        decoyed: false,
        decoy_point: None,
        chaff_checked: false,
        ciws_checked: false,
        passes: 0,
    });
    w.entities.insert(id, torpedo.clone());
    air_torpedo_dropped(&mut w, a, id);
    assert!(air_torpedo_gate(&w, a, ContactId(1)).is_err());
    assert!(!air_torpedo_interfered(&w, id));
    w.tick = 180;
    assert!(air_torpedo_gate(&w, a, ContactId(1)).is_ok());
    let second = w.allocate_id();
    torpedo.id = second;
    w.entities.insert(second, torpedo);
    air_torpedo_dropped(&mut w, a, second);
    assert!(air_torpedo_interfered(&w, id));
    assert!(air_torpedo_interfered(&w, second));
    let restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert!(air_torpedo_interfered(&restored, id));
}

use harpoon_core::rules::aviation2026::*;
use harpoon_core::{Database, Scenario, World};
use serde_json::json;
fn fixture(airborne: bool) -> World {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut units =
        vec![json!({"side":"Blue","class":"us_ford","name":"CV","pos":[0,0],"speed_kts":32})];
    if airborne {
        units.push(json!({"side":"Blue","class":"us_f18e","name":"air","pos":[0,0],"speed_kts":460,"altitude_m":10000}));
    }
    let sc:Scenario=serde_json::from_value(json!({"id":"av4","name":"Aviation source","duration_s":200000,"seed":23,"context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":3,"precipitation":"None"},"air_wings":{"CV":[{"class":"us_f18e","count":8}]},"aviation_2026":{"source":"scenario_assumption: acceptance load, not verified combat fit","loads":{"us_f18e":{"payload_kg":1000,"tanks":[],"source":"scenario_assumption: source mechanism fixture"}},"parking":{"CV":12}}},"weather":{"visibility_nm":0,"jam":[0,0]},"units":units})).unwrap();
    harpoon_core::load_scenario(sc, db).unwrap()
}
#[test]
fn missing_initial_airborne_fuel_is_not_silently_full() {
    let mut w = fixture(true);
    initialize(&mut w).unwrap();
    assert!(
        w.rules.aviation.as_ref().unwrap().flights.is_empty(),
        "hot-start spent range must be explicit"
    );
}

#[test]
fn real_sim_asw_hover_is_stationary_and_burns_full_power_then_resumes_cruise() {
    let mut w = fixture(true);
    let id = w
        .entities
        .values()
        .find(|e| e.class == "us_f18e")
        .unwrap()
        .id;
    let config = w.context.aviation_2026.as_mut().unwrap();
    config.loads.insert(
        "us_mh60r".into(),
        Loadout {
            payload_kg: 0.,
            tanks: vec![],
            source: "scenario_assumption unarmed ASW hover fixture".into(),
            buddy: false,
            external_weapon_rounds: Default::default(),
            internal_weapon_rounds: Default::default(),
        },
    );
    config.initial_fuel_nm.insert("air".into(), 200.);
    let e = w.entity_mut(id).unwrap();
    e.class = "us_mh60r".into();
    e.altitude_m = 30.;
    e.ordered_altitude_m = 30.;
    e.speed_kts = 0.;
    e.ordered_speed_kts = 0.;
    e.mission = Some(harpoon_core::world::MissionState::Asw {
        area: harpoon_core::Vec2::zero(),
        radius_nm: 1.,
    });
    initialize(&mut w).unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    let pos = w.entity(id).unwrap().pos;
    let fuel = w.rules.aviation.as_ref().unwrap().flights[&id].fuel_nm;
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..60 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        w.entity(id).unwrap().pos,
        pos,
        "4.3 hovering helicopter must remain stationary"
    );
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&id].throttle,
        Throttle::Hover
    );
    let spent = fuel - w.rules.aviation.as_ref().unwrap().flights[&id].fuel_nm;
    assert!(
        (spent - 2.5).abs() < 1e-9,
        "4.2/4.3 hover uses full power; 100kts cruise * 1.5 * 60/3600"
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    w.entity_mut(id).unwrap().mission = Some(harpoon_core::world::MissionState::Recon {
        point: harpoon_core::Vec2::new(5., 0.),
    });
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&id].throttle,
        Throttle::Cruise
    );
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(w.entity(id).unwrap().speed_kts, 100.);
}

#[test]
fn real_sim_external_jettison_changes_maneuver_without_spawning_an_attack() {
    let mut w = fixture(true);
    let id = w
        .entities
        .values()
        .find(|e| e.class == "us_f18e")
        .unwrap()
        .id;
    let cv = w
        .entities
        .values()
        .find(|e| e.class == "us_ford")
        .unwrap()
        .id;
    let config = w.context.aviation_2026.as_mut().unwrap();
    config.initial_fuel_nm.insert("air".into(), 700.);
    let load = config.loads.get_mut("us_f18e").unwrap();
    load.payload_kg = 4000.;
    load.external_weapon_rounds.insert("us_aim120d".into(), 2);
    initialize(&mut w).unwrap();
    assert_eq!(
        harpoon_core::rules::aircombat2026::target_maneuver(&w, id).unwrap(),
        1.5
    );
    let def = w.db.weapons["us_aim120d"].clone();
    let incoming = harpoon_core::weapons::launch(&mut w, cv, &def, Some(id), None).unwrap();
    harpoon_core::rules::aircombat2026::begin_aam_evasion(&mut w, id, incoming);
    assert!(harpoon_core::rules::aircombat2026::release_allowed(&w, id).is_err());
    let launches = w.stats.projectiles_launched;
    let action = Action::Jettison {
        weapon: "us_aim120d".into(),
        rounds: 1,
    };
    let command = harpoon_core::Command::Aviation { unit: id, action };
    harpoon_core::sim::tick(&mut w, &[(harpoon_core::Side::Blue, command)]);
    let flight = &w.rules.aviation.as_ref().unwrap().flights[&id];
    assert!((flight.payload_fraction - 3838. / 7950.).abs() < 1e-12);
    assert_eq!(flight.external_weapon_rounds["us_aim120d"], 1);
    assert_eq!(
        harpoon_core::rules::aircombat2026::target_maneuver(&w, id).unwrap(),
        4.5
    );
    assert_eq!(
        w.entity(id)
            .unwrap()
            .weapons
            .iter()
            .filter(|m| m.weapon == "us_aim120d")
            .map(|m| m.ammo)
            .sum::<u16>(),
        1
    );
    assert_eq!(w.stats.projectiles_launched, launches);
    assert!(
        apply(
            &mut w,
            id,
            &Action::Jettison {
                weapon: "us_aim9x2".into(),
                rounds: 1
            }
        )
        .is_err(),
        "undeclared internal/external stores must not be guessed"
    );
    assert!(apply(
        &mut w,
        id,
        &Action::Jettison {
            weapon: "us_aim120d".into(),
            rounds: 2
        }
    )
    .is_err());
    assert!(w.events.iter().any(
        |e| matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..} if rule=="9.5.1.jettison")
    ));
    let restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn actual_release_deducts_only_explicit_source_external_store_mass() {
    let mut w = fixture(true);
    let id = w
        .entities
        .values()
        .find(|e| e.class == "us_f18e")
        .unwrap()
        .id;
    let config = w.context.aviation_2026.as_mut().unwrap();
    config.initial_fuel_nm.insert("air".into(), 700.);
    let load = config.loads.get_mut("us_f18e").unwrap();
    load.payload_kg = 4000.;
    load.external_weapon_rounds.insert("us_aim120d".into(), 2);
    initialize(&mut w).unwrap();
    let before = w.rules.aviation.as_ref().unwrap().flights[&id].payload_fraction;
    let undeclared = w.db.weapons["us_aim9x2"].clone();
    harpoon_core::weapons::launch(
        &mut w,
        id,
        &undeclared,
        None,
        Some(harpoon_core::Vec2::new(0., 5.)),
    )
    .unwrap();
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&id].payload_fraction,
        before
    );
    let declared = w.db.weapons["us_aim120d"].clone();
    harpoon_core::weapons::launch(
        &mut w,
        id,
        &declared,
        None,
        Some(harpoon_core::Vec2::new(0., 5.)),
    )
    .unwrap();
    let flight = &w.rules.aviation.as_ref().unwrap().flights[&id];
    assert!((flight.payload_fraction - 3838. / 7950.).abs() < 1e-12);
    assert_eq!(flight.external_weapon_rounds["us_aim120d"], 1);
}
#[test]
fn source_range_example_and_all_throttle_engine_branches() {
    let c = |e, t, d, a| range_cost(e, t, d, 60., 400., a, 0.).unwrap();
    assert_eq!(
        c(Engine::TF, Throttle::Cruise, 150., 10000.)
            + c(Engine::TF, Throttle::Cruise, 25., 1000.)
            + c(Engine::TF, Throttle::Fmp, 25., 1000.)
            + c(Engine::TF, Throttle::Cruise, 200., 10000.),
        550.
    );
    assert_eq!(c(Engine::TP, Throttle::Fmp, 100., 1000.), 150.);
    assert_eq!(c(Engine::TF, Throttle::Reheat, 100., 10000.), 1000.);
    assert_eq!(
        range_cost(Engine::TF, Throttle::Loiter, 0., 3600., 400., 10000., 0.).unwrap(),
        320.
    );
    assert!(range_cost(Engine::IP, Throttle::Cruise, 1., 1., 400., 8000., 0.).is_err());
    assert_eq!(
        range_cost(Engine::TF, Throttle::Cruise, 75., 1., 400., 10000., 0.5).unwrap(),
        100.
    );
}
#[test]
fn source_refuel_and_respot_boundaries() {
    assert_eq!(refuel_seconds(3, RefuelMethod::Boom).unwrap(), 1440);
    assert_eq!(refuel_seconds(1, RefuelMethod::WesternProbe).unwrap(), 540);
    assert_eq!(refuel_seconds(2, RefuelMethod::BuddyWestern).unwrap(), 720);
    assert!(refuel_seconds(3, RefuelMethod::BuddyWestern).is_err());
    assert_eq!(respot_seconds(24, &[1, 6]).unwrap(), 2100);
    assert_eq!(respot_seconds(0, &[]).unwrap(), 0);
    assert!(respot_seconds(13, &[1, 2]).is_err());
    assert_eq!(cap_time_hours(800., 200., 400.).unwrap(), 1.25);
}
#[test]
fn source_all_refuel_cells_and_equipment() {
    for size in [1, 2] {
        assert_eq!(refuel_seconds(size, RefuelMethod::Boom).unwrap(), 360);
        assert_eq!(
            refuel_seconds(size, RefuelMethod::SovietProbe).unwrap(),
            540
        );
        assert_eq!(
            refuel_seconds(size, RefuelMethod::BuddySoviet).unwrap(),
            720
        );
    }
    assert_eq!(refuel_seconds(3, RefuelMethod::WesternProbe).unwrap(), 1800);
    assert!(refuel_seconds(0, RefuelMethod::Boom).is_err());
    assert!(!compatible(
        RefuelMethod::WesternProbe,
        RefuelMethod::SovietProbe
    ));
    assert!(!compatible(RefuelMethod::Boom, RefuelMethod::WesternProbe));
    assert!(compatible(
        RefuelMethod::BuddyWestern,
        RefuelMethod::WesternProbe
    ));
    assert!(compatible(RefuelMethod::ZA, RefuelMethod::ZA));
    assert_eq!(offload_factor("ka6d", "f14a").unwrap(), 1.2);
    assert_eq!(offload_factor("us_f18e", "us_f18e").unwrap(), 1.);
    assert_eq!(offload_factor("us_f18e", "us_f35c").unwrap(), 5.2 / 6.1);
    assert!(offload_factor("cn_j15", "us_f18e").is_err());
    assert_eq!(
        transfer_limits(2000. - 300., 300., 500., 1.2).unwrap(),
        (500. / 1.2, 500.)
    );
}
#[test]
fn source_altitude_table_and_heavy_climb() {
    assert_eq!(
        altitude_rate(Engine::TF, false, 4.5, 4000., false, true)
            .unwrap()
            .0
            * 180.,
        13500.
    );
    assert_eq!(
        altitude_rate(Engine::TF, false, 4.5, 4000., false, false)
            .unwrap()
            .0
            * 180.,
        20250.
    );
    assert_eq!(
        altitude_rate(Engine::TP, false, 2., 1000., true, true)
            .unwrap()
            .0
            * 180.,
        1800.
    );
    assert_eq!(
        altitude_rate(Engine::TS, true, 2., 4000., false, true)
            .unwrap()
            .0
            * 180.,
        400.
    );
    assert_eq!(
        altitude_rate(Engine::RT, false, 1., 8000., false, false)
            .unwrap()
            .0
            * 180.,
        3750.
    );
    assert!(altitude_rate(Engine::TF, false, 2.2, 4000., false, true).is_err());
}
#[test]
fn source_low_flight_and_water_risks() {
    assert_eq!(low_crash_chance(150., 10, 0., false).unwrap(), 0.);
    assert_eq!(low_crash_chance(250., 1, 31., true).unwrap(), 0.06);
    assert_eq!(low_crash_chance(650., 10, 0., false).unwrap(), 0.13);
    assert!(low_crash_chance(651., 1, 0., false).is_err());
    assert!((swamp_chance(6, false, 0).unwrap() - 0.3).abs() < 1e-12);
    assert_eq!(swamp_chance(6, true, 0).unwrap(), 0.);
    assert_eq!(swamp_chance(6, false, 2).unwrap(), 0.1);
    assert_eq!(source_speed("cn_j15", 15., Throttle::Cruise).unwrap(), 430.);
    assert_eq!(source_speed("us_f18e", 15., Throttle::Fmp).unwrap(), 450.);
    assert_eq!(source_speed("us_f35c", 50., Throttle::Fmp).unwrap(), 590.);
    assert!(source_speed("us_mh60r", 1000., Throttle::Reheat).is_err());
    assert!(source_speed("us_f18e", 16000., Throttle::Cruise).is_err());
}
#[test]
fn actual_world_respot_roll_saved_until_completion_and_private_trace() {
    let mut w = fixture(false);
    initialize(&mut w).unwrap();
    let cv = *w.entities.keys().next().unwrap();
    assert!(launch_gate(&w, cv, "us_f18e", 1).is_err());
    apply(&mut w, cv, &Action::Respot).unwrap();
    let r = w.rules.aviation.as_ref().unwrap().respots[&cv].clone();
    assert_eq!(r.dice.len(), 1);
    assert!((900..=1200).contains(&r.due));
    assert!(apply(&mut w, cv, &Action::Respot).is_err());
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    w.tick = r.due - 1;
    step(&mut w);
    assert!(recovery_gate(&w, cv).is_err());
    w.tick = r.due;
    saved.tick = r.due;
    step(&mut w);
    step(&mut saved);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    assert!(launch_gate(&w, cv, "us_f18e", 1).is_ok());
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..}if rule=="4.6.respot_started")&&matches!(e.vis,harpoon_core::Visibility::Side(_))));
}
#[test]
fn actual_world_alert_expires_and_requires_source_preflight_again() {
    let mut w = fixture(false);
    initialize(&mut w).unwrap();
    let cv = *w.entities.keys().next().unwrap();
    w.rules
        .aviation
        .as_mut()
        .unwrap()
        .deck_parking
        .insert(cv, 0);
    w.tick = 3600;
    assert!(launch_gate(&w, cv, "us_f18e", 1).is_ok());
    w.tick = 3601;
    assert!(launch_gate(&w, cv, "us_f18e", 1).is_err());
    apply(
        &mut w,
        cv,
        &Action::Alert {
            class: "us_f18e".into(),
            count: 1,
        },
    )
    .unwrap();
    w.tick = 6000;
    step(&mut w);
    assert!(launch_gate(&w, cv, "us_f18e", 1).is_err());
    w.tick = 6001;
    step(&mut w);
    assert!(launch_gate(&w, cv, "us_f18e", 1).is_ok());
    w.rules
        .aviation
        .as_mut()
        .unwrap()
        .airframes
        .values_mut()
        .for_each(|a| {
            a.ready_since = 6001;
            a.sorties = vec![(100, Some(1000)), (2000, Some(3000))];
        });
    assert!(launch_gate(&w, cv, "us_f18e", 1).is_err());
    assert!(launch_gate(&w, cv, "us_f18e", 0).is_err());
}
#[test]
fn actual_world_fuel_distance_heavy_boundary_and_no_double_tick() {
    let mut w = fixture(true);
    initialize(&mut w).unwrap();
    let cv = w
        .entities
        .values()
        .find(|e| e.class == "us_ford")
        .unwrap()
        .id;
    let id = w
        .entities
        .values()
        .find(|e| e.class == "us_f18e")
        .unwrap()
        .id;
    launch_completed(&mut w, cv, "us_f18e", id).unwrap();
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&id].fuel_nm,
        895.
    );
    w.tick = 1;
    w.entity_mut(id).unwrap().pos.x = 25.;
    w.entity_mut(id).unwrap().altitude_m = 1000.;
    fuel_step(&mut w, id).unwrap();
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&id].fuel_nm,
        845.
    );
    let value = fuel_step(&mut w, id).unwrap();
    assert_eq!(fuel_step(&mut w, id).unwrap(), value);
    w.rules
        .aviation
        .as_mut()
        .unwrap()
        .flights
        .get_mut(&id)
        .unwrap()
        .payload_fraction = 0.5;
    w.tick = 2;
    w.entity_mut(id).unwrap().pos.x += 75.;
    w.entity_mut(id).unwrap().altitude_m = 10000.;
    fuel_step(&mut w, id).unwrap();
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&id].fuel_nm,
        745.
    );
}
#[test]
fn unknown_load_mass_or_guessed_tank_blocks_launch() {
    let mut w = fixture(false);
    initialize(&mut w).unwrap();
    let cv = *w.entities.keys().next().unwrap();
    w.rules
        .aviation
        .as_mut()
        .unwrap()
        .deck_parking
        .insert(cv, 0);
    w.context
        .aviation_2026
        .as_mut()
        .unwrap()
        .loads
        .get_mut("us_f18e")
        .unwrap()
        .tanks
        .push(Tank {
            range_add_nm: 9999.,
            fuel_weight_kg: 1.,
            source: "unverified".into(),
        });
    assert!(launch_gate(&w, cv, "us_f18e", 1).is_err());
    w.context.aviation_2026.as_mut().unwrap().loads.clear();
    assert!(launch_gate(&w, cv, "us_f18e", 1).is_err());
}
#[test]
fn buddy_store_cannot_claim_dedicated_tanker_pumping_rate() {
    let mut w = fixture(true);
    let first = w
        .entities
        .values()
        .find(|e| e.class == "us_f18e")
        .unwrap()
        .clone();
    let mut second = first.clone();
    second.id = harpoon_core::EntityId(999);
    second.name = "receiver".into();
    w.entities.insert(second.id, second);
    let c = w.context.aviation_2026.as_mut().unwrap();
    c.initial_fuel_nm.insert("air".into(), 700.);
    c.initial_fuel_nm.insert("receiver".into(), 400.);
    c.loads.get_mut("us_f18e").unwrap().buddy = true;
    c.loads.get_mut("us_f18e").unwrap().tanks.push(Tank {
        range_add_nm: 180.,
        fuel_weight_kg: 920.,
        source: "America Aircraft PDF49/B-44".into(),
    });
    initialize(&mut w).unwrap();
    assert!(
        apply(
            &mut w,
            first.id,
            &Action::Refuel {
                receiver: harpoon_core::EntityId(999),
                method: RefuelMethod::WesternProbe
            }
        )
        .is_err(),
        "a buddy store is not a dedicated probe tanker"
    );
}
#[test]
fn real_sim_buddy_refuel_preserves_mass_and_tanker_return_budget() {
    let mut w = fixture(true);
    let first = w
        .entities
        .values()
        .find(|e| e.class == "us_f18e")
        .unwrap()
        .clone();
    let id = first.id;
    let mut second = first.clone();
    second.id = w.allocate_id();
    second.name = "receiver".into();
    let receiver = second.id;
    w.entities.insert(receiver, second);
    for e in w.entities.values_mut().filter(|e| e.class == "us_f18e") {
        e.altitude_m = 10000.;
        e.ordered_altitude_m = 10000.;
        e.speed_kts = 460.;
        e.ordered_speed_kts = 460.;
        e.course_deg = 0.;
        e.ordered_course_deg = 0.;
        e.mission = None;
        e.fire_on = false;
    }
    let c = w.context.aviation_2026.as_mut().unwrap();
    c.initial_fuel_nm.insert("air".into(), 700.);
    c.initial_fuel_nm.insert("receiver".into(), 400.);
    let l = c.loads.get_mut("us_f18e").unwrap();
    l.buddy = true;
    l.tanks.push(Tank {
        range_add_nm: 180.,
        fuel_weight_kg: 920.,
        source: "America Aircraft PDF49/B-44".into(),
    });
    initialize(&mut w).unwrap();
    let cmd = harpoon_core::Command::Aviation {
        unit: id,
        action: Action::Refuel {
            receiver,
            method: RefuelMethod::BuddyWestern,
        },
    };
    harpoon_core::sim::tick(&mut w, &[(harpoon_core::Side::Blue, cmd)]);
    let due = w.rules.aviation.as_ref().unwrap().refuels[0].due;
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    while w.tick < due {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    let s = w.rules.aviation.as_ref().unwrap();
    let tanker = &s.flights[&id];
    let cv = w.entities.values().find(|e| e.class == "us_ford").unwrap();
    let distance = w.entity(id).unwrap().pos.range_to(cv.pos);
    assert!(
        tanker.fuel_nm + 1e-8 >= distance,
        "refuel must reserve current return range after both planes burned fuel"
    );
    assert!(
        (tanker.fuel_nm + s.flights[&receiver].fuel_nm - (1100. - 2. * 460. * due as f64 / 3600.))
            .abs()
            < 1e-7
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..}if rule=="4.8.refuel_completed")));
}
#[test]
fn actual_sim_respot_launch_birth_fuel_and_source_climb_survive_save() {
    let mut w = fixture(false);
    let cv = *w.entities.keys().next().unwrap();
    harpoon_core::sim::tick(
        &mut w,
        &[(
            harpoon_core::Side::Blue,
            harpoon_core::Command::Aviation {
                unit: cv,
                action: Action::Respot,
            },
        )],
    );
    let due = w.rules.aviation.as_ref().unwrap().respots[&cv].due;
    while w.tick < due {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let launch = harpoon_core::Command::LaunchAircraft {
        unit: cv,
        class: "us_f18e".into(),
        count: 1,
        mission: harpoon_core::AirMission::Recon {
            point: harpoon_core::Vec2::new(50., 0.),
        },
        profile: None,
    };
    harpoon_core::sim::tick(&mut w, &[(harpoon_core::Side::Blue, launch)]);
    let launch_due = w.rules.operations.as_ref().unwrap().launches[0].due;
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    while w.tick < launch_due {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    let e = w
        .entities
        .values()
        .find(|e| e.kind == harpoon_core::types::PlatformKind::Aircraft && e.alive)
        .unwrap();
    let id = e.id;
    assert_eq!(e.altitude_m, 61.);
    assert_eq!(e.speed_kts, 585. * 0.25);
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&id].fuel_nm,
        895.
    );
    let before = e.altitude_m;
    harpoon_core::sim::tick(&mut w, &[]);
    harpoon_core::sim::tick(&mut saved, &[]);
    assert!(
        (w.entity(id).unwrap().altitude_m - before - 150.).abs() < 1e-9,
        "Low doubles 3000*4.5/180 source climb"
    );
    assert!(w.rules.aviation.as_ref().unwrap().flights[&id].fuel_nm < 895.);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}
#[test]
fn actual_sskim_roll_is_independent_saved_and_under_fire_added_once() {
    let mut w = fixture(true);
    let id = w
        .entities
        .values()
        .find(|e| e.class == "us_f18e")
        .unwrap()
        .id;
    w.context
        .aviation_2026
        .as_mut()
        .unwrap()
        .initial_fuel_nm
        .insert("air".into(), 700.);
    w.context.aviation_2026.as_mut().unwrap().visibility_percent = Some(100.);
    let e = w.entity_mut(id).unwrap();
    e.altitude_m = 15.;
    e.ordered_altitude_m = 15.;
    e.speed_kts = 450.;
    e.ordered_speed_kts = 450.;
    e.mission = None;
    initialize(&mut w).unwrap();
    let seed = (0..10000)
        .find(|seed| {
            let mut r = harpoon_core::Rng::new(*seed).derive(harpoon_core::Rng::hash_tag(
                1,
                id.0,
                "4.5.2.low_crash",
            ));
            r.next_f64() > 0.2
        })
        .unwrap();
    w.rng = harpoon_core::Rng::new(seed);
    harpoon_core::sim::tick(&mut w, &[]);
    assert!(w.entity(id).unwrap().alive);
    mark_under_fire(&mut w, id);
    mark_under_fire(&mut w, id);
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..180 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    let traces: Vec<_> = w
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent { rule, detail, .. }
                if rule == "4.5.2.low_crash" =>
            {
                Some(serde_json::from_str::<serde_json::Value>(detail).unwrap())
            }
            _ => None,
        })
        .collect();
    assert_eq!(traces.len(), 2);
    assert!((traces[1]["chance"].as_f64().unwrap() - 0.07).abs() < 1e-12);
    assert_eq!(traces[1]["attacked"], true);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}
#[test]
fn mission_fuel_gate_refuses_suicide_without_using_enemy_truth() {
    let mut w = fixture(false);
    let cv = *w.entities.keys().next().unwrap();
    w.rules
        .aviation
        .as_mut()
        .unwrap()
        .deck_parking
        .insert(cv, 0);
    let mission = harpoon_core::AirMission::Recon {
        point: harpoon_core::Vec2::new(1000., 0.),
    };
    assert!(harpoon_core::air::launch_aircraft(&mut w, cv, "us_f18e", 1, &mission).is_err());
    assert_eq!(w.rules.operations.as_ref().unwrap().launches.len(), 0);
    assert_eq!(w.entity(cv).unwrap().deck.as_ref().unwrap().ready[0].1, 8);
}
#[test]
fn real_sim_recovery_retains_the_individual_sortie_identity() {
    let mut w = fixture(true);
    let cv = w
        .entities
        .values()
        .find(|e| e.class == "us_ford")
        .unwrap()
        .id;
    let id = w
        .entities
        .values()
        .find(|e| e.class == "us_f18e")
        .unwrap()
        .id;
    initialize(&mut w).unwrap();
    w.rules
        .aviation
        .as_mut()
        .unwrap()
        .deck_parking
        .insert(cv, 0);
    launch_completed(&mut w, cv, "us_f18e", id).unwrap();
    let pos = w.entity(cv).unwrap().pos;
    let heading = w.entity(cv).unwrap().course_deg;
    let e = w.entity_mut(id).unwrap();
    e.pos = pos;
    e.altitude_m = 1000.;
    e.ordered_altitude_m = 1000.;
    e.course_deg = heading;
    e.ordered_course_deg = heading;
    e.speed_kts = 585. * 0.25;
    e.ordered_speed_kts = e.speed_kts;
    e.mission = Some(harpoon_core::world::MissionState::Rtb);
    w.rules
        .aviation
        .as_mut()
        .unwrap()
        .flights
        .get_mut(&id)
        .unwrap()
        .last_position = pos;
    harpoon_core::sim::tick(&mut w, &[]);
    let s = w.rules.aviation.as_ref().unwrap();
    assert!(!s.flights.contains_key(&id));
    let a = s.airframes.values().find(|a| a.sorties.len() == 1).unwrap();
    assert_eq!(a.aircraft, None);
    assert_eq!(a.sorties[0], (0, Some(1)));
    assert_eq!(a.ready_at, Some(5401));
    assert!(w
        .events
        .iter()
        .any(|e| matches!(&e.kind, harpoon_core::EventKind::AirRecovered { .. })));
}
#[test]
fn real_sim_helicopter_hifr_fills_only_after_fifteen_minutes() {
    let mut w = fixture(true);
    let id = w
        .entities
        .values()
        .find(|e| e.class == "us_f18e")
        .unwrap()
        .id;
    let ship = w
        .entities
        .values()
        .find(|e| e.class == "us_ford")
        .unwrap()
        .id;
    let c = w.context.aviation_2026.as_mut().unwrap();
    c.loads.insert(
        "us_mh60r".into(),
        Loadout {
            payload_kg: 0.,
            tanks: vec![],
            source: "scenario_assumption unarmed ferry".into(),
            buddy: false,
            external_weapon_rounds: Default::default(),
            internal_weapon_rounds: Default::default(),
        },
    );
    c.initial_fuel_nm.insert("air".into(), 140.);
    let e = w.entity_mut(id).unwrap();
    e.class = "us_mh60r".into();
    e.altitude_m = 15.;
    e.ordered_altitude_m = 15.;
    e.speed_kts = 32.;
    e.ordered_speed_kts = 32.;
    e.course_deg = 0.;
    e.ordered_course_deg = 0.;
    e.mission = None;
    e.assigned = None;
    initialize(&mut w).unwrap();
    harpoon_core::sim::tick(
        &mut w,
        &[(
            harpoon_core::Side::Blue,
            harpoon_core::Command::Aviation {
                unit: id,
                action: Action::Hifr { ship },
            },
        )],
    );
    let due = w.rules.aviation.as_ref().unwrap().hifr[&id].1;
    assert_eq!(due, 901);
    while w.tick < due - 1 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.rules.aviation.as_ref().unwrap().flights[&id].fuel_nm < 140.);
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&id].fuel_nm,
        265.
    );
    assert!(apply(&mut w, id, &Action::Hifr { ship }).is_err());
}

// Source B44/B83 same
// F/A-18E airframe, receiver fuel rate5.2. Heavy payload changes fuel burn;
// do not reuse the old equal-unloaded-burn equation.
#[test]
fn real_mk82_variant_receives_source_buddy_fuel_without_changing_stores_and_replays() {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let scenario:Scenario = serde_json::from_value(json!({
        "id":"mk82-source-refuel", "name":"Same F18 airframe receiver alias", "duration_s":2000,"seed":23,
        "context":{"rules_profile":"hv2026-rts-v4", "air_wings":{"CV":[{"class":"us_f18e","count":1},{"class":"us_f18e_mk82","count":1}]}, "aviation_2026":{
            "source":"scenario_assumption: controlled source refueling rendezvous",
            "loads":{
                "us_f18e":{"payload_kg":920,"tanks":[{"range_add_nm":180,"fuel_weight_kg":920,"source":"US Aircraft2025 PDF49/B44 300USG buddy"}],"buddy":true,"source":"scenario_assumption: only buddy-store fixture mass"},
                "us_f18e_mk82":{"payload_kg":6395,"tanks":[{"range_add_nm":290,"fuel_weight_kg":1485,"source":"B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"B44"}],"external_weapon_rounds":{"us_mk82":6},"source":"US Aircraft2025 B44/H1-3/H4-2 3DT+2AIM120D+2AIM9X2+6Mk82"}},
            "parking":{},"initial_fuel_nm":{"TANKER":700,"RECEIVER":400}}},
        "weather":{"visibility_nm":0,"jam":[0,0]}, "units":[
            {"side":"Blue","class":"us_ford","name":"CV","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":"us_f18e","name":"TANKER","pos":[0,0],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
            {"side":"Blue","class":"us_f18e_mk82","name":"RECEIVER","pos":[0,0],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let tanker = w.entities.values().find(|e| e.name == "TANKER").unwrap().id;
    let receiver = w
        .entities
        .values()
        .find(|e| e.name == "RECEIVER")
        .unwrap()
        .id;
    let stores = w.entity(receiver).unwrap().weapons.clone();
    let payload = w.rules.aviation.as_ref().unwrap().flights[&receiver].payload_fraction;
    assert_eq!(offload_factor("us_f18e", "us_f18e_mk82").unwrap(), 1.);
    // The first fixture omitted a compatible explicit wing: the consolidated
    // Ford has generic fighter/striker placeholders, so return reserve must
    // reject it. Keep that real source gate and explicitly supply this fixture's wing.
    let mut without_recovery_wing = w.clone();
    without_recovery_wing.context.air_wings.clear();
    let reason = apply(
        &mut without_recovery_wing,
        tanker,
        &Action::Refuel {
            receiver,
            method: RefuelMethod::BuddyWestern,
        },
    )
    .unwrap_err();
    assert!(
        reason.contains("no compatible friendly recovery base"),
        "actual precondition failure: {reason}"
    );
    harpoon_core::sim::tick(
        &mut w,
        &[(
            harpoon_core::Side::Blue,
            harpoon_core::Command::Aviation {
                unit: tanker,
                action: Action::Refuel {
                    receiver,
                    method: RefuelMethod::BuddyWestern,
                },
            },
        )],
    );
    let rejected: Vec<_> = w
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent {
                domain,
                rule,
                detail,
                ..
            } if domain == "command" && rule == "rejected" => Some(detail),
            _ => None,
        })
        .collect();
    assert!(
        !w.rules.aviation.as_ref().unwrap().refuels.is_empty(),
        "source buddy action must create a job; actual rejection: {rejected:?}"
    );
    let due = w
        .rules
        .aviation
        .as_ref()
        .unwrap()
        .refuels
        .first()
        .expect("same source F18 airframe must use the5.2 receiver rate")
        .due;
    assert_eq!(due - w.tick, 720);
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    while w.tick < due {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    let s = w.rules.aviation.as_ref().unwrap();
    let t = &s.flights[&tanker];
    let r = &s.flights[&receiver];
    assert_eq!(r.payload_fraction, payload);
    assert_eq!(
        serde_json::to_value(&w.entity(receiver).unwrap().weapons).unwrap(),
        serde_json::to_value(&stores).unwrap()
    );
    assert_eq!(r.external_weapon_rounds["us_mk82"], 6);
    assert!(
        r.fuel_nm > 400.,
        "actual completion must transfer usable receiver range"
    );
    let cv = w.entities.values().find(|e| e.name == "CV").unwrap();
    assert!(t.fuel_nm + 1e-8 >= w.entity(tanker).unwrap().pos.range_to(cv.pos));
    let expected = 1100. - 460. * (1. + 1. / 0.75) * due as f64 / 3600.;
    assert!(
        (t.fuel_nm + r.fuel_nm - expected).abs() < 1e-6,
        "factor1 preserves unloaded-cruise fuel budget while receiver burns at heavy rate"
    );
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent {rule,..} if rule=="4.8.refuel_completed")));
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(saved).unwrap()
    );
}

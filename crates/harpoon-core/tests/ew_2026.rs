use harpoon_core::db::SensorKind;
use harpoon_core::rules::{detection2026 as d, ew2026 as ew};
use harpoon_core::types::PlatformKind;
use harpoon_core::{Command, Database, EntityId, Scenario, Side, Vec2, World};
use serde_json::json;

fn world() -> World {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc:Scenario=serde_json::from_value(json!({"id":"ew-source-fixture","name":"EW","duration_s":20000,"seed":23,"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"}},"weather":{"visibility_nm":12.,"jam":[0.,0.]},"units":[{"side":"Blue","class":"us_ford","name":"observer","pos":[0.,0.],"speed_kts":0.},{"side":"Red","class":"cn_001","name":"launcher","pos":[2.,0.],"speed_kts":0.}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    for e in w.entities.values_mut() {
        e.weapons.clear();
        e.sensors.clear();
        e.fire_on = false;
        e.ordered_speed_kts = 0.;
        e.speed_kts = 0.;
    }
    let mut state = ew::State::default();
    state.config.environment = Some(ew::Environment {
        source: "explicit fixture wind/sea; no actual platform fit claim".into(),
        sea_state: 0,
        wind_from_deg: 270.,
        wind_speed_kts: 10.,
    });
    w.rules.ew = Some(state);
    w.rules.detection.as_mut().unwrap().config.environment = Some(d::Environment {
        source: "source mechanism fixture environment".into(),
        sea_state: 0,
        precipitation: 0,
        visibility_percent: 60,
        shipping: 0,
        biologics: 0,
        water_depth_m: 3000.,
        layer: None,
        ice: 0,
        cloud: None,
    });
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .targets
        .insert("cn_001".into(), profile());
    // Keep probabilistic rules intact while selecting reproducible successful
    // detection fixtures; no individual production roll is replaced.
    while [2, 3, 4].iter().any(|target| {
        [0, 180, 360].iter().any(|at| {
            d::trial(
                &mut d::State::default(),
                w.seed,
                *at,
                &format!("5.4.6:1:source_ew_sonar:{target}"),
                180,
            )
            .0 > 80
        })
    }) {
        w.seed += 1;
    }
    w
}
fn profile() -> d::TargetProfile {
    d::TargetProfile {
        source: "mechanism fixture A-size Quiet boat, no attachment claim".into(),
        size: 0,
        noise: Some(2),
        anechoic: false,
        titanium: false,
        pumpjet: false,
        stealthy_ir: false,
        radar_signature: Some(0),
    }
}
fn fit(kind: ew::ObjectKind) -> ew::Fit {
    ew::Fit {
        source: "English2026 ch12 generic mechanism fixture, not actual fitted hardware".into(),
        kind,
        generation: Some(3),
        launcher: Some(ew::Launcher::Internal),
        endurance_s: None,
        deployment_s: None,
        radar_height_m: Some(1.),
    }
}
fn install(w: &mut World, name: &str, fit: ew::Fit, count: u16) {
    let s = w.rules.ew.as_mut().unwrap();
    s.config.fits.insert(name.into(), fit);
    s.config
        .loadouts
        .entry("cn_001".into())
        .or_default()
        .insert(name.into(), count);
}
fn deploy(name: &str) -> ew::Action {
    ew::Action::Deploy {
        fit: name.into(),
        at: None,
        radar_signature: None,
        course_deg: None,
        speed_kts: None,
        depth_m: None,
    }
}
fn ids(w: &World) -> (EntityId, EntityId) {
    let mut ids = w.entities.keys();
    (*ids.next().unwrap(), *ids.next().unwrap())
}
fn sonar(w: &mut World, observer: EntityId, active: bool) {
    let name = "source_ew_sonar".to_string();
    let mut sensor = w.db.sensors.values().next().unwrap().clone();
    sensor.id = name.clone();
    sensor.kind = if active {
        SensorKind::SonarActive
    } else {
        SensorKind::SonarPassive
    };
    sensor.sub_range_nm = 0.;
    sensor.surface_range_nm = 0.;
    w.db.sensors.insert(name.clone(), sensor);
    let e = w.entity_mut(observer).unwrap();
    e.course_deg = 90.;
    e.ordered_course_deg = 90.;
    e.sensors = vec![harpoon_core::world::SensorInstance {
        sensor: name.clone(),
        active: true,
    }];
    let class = e.class.clone();
    w.rules.detection.as_mut().unwrap().config.bindings.insert(
        format!("{class}:{name}"),
        d::Receiver::Sonar {
            source: "5.4/12.6 mechanism fixture".into(),
            generation: 5,
            passive_range_nm: Some(10.),
            active_range_nm: Some(10.),
            frequency: d::Frequency::Lf,
            mount: d::SonarMount::Bow,
            active_torpedo_detection: false,
            high_speed_tow: false,
        },
    );
}
fn radar(w: &mut World, observer: EntityId) {
    w.entity_mut(observer).unwrap().sensors = vec![harpoon_core::world::SensorInstance {
        sensor: "cn_ljq364".into(),
        active: true,
    }];
}
fn object_id(w: &World) -> EntityId {
    *w.rules.ew.as_ref().unwrap().objects.keys().next().unwrap()
}

#[test]
fn source_tables_cover_exact_generations_and_distinct_countermeasure_domains() {
    assert_eq!(
        ew::tables()["mobile_submarine_decoys"]["generations"][0]["endurance_s"],
        3600
    );
    assert_eq!(
        ew::tables()["mobile_submarine_decoys"]["generations"][3]["max_speed_kts"],
        15
    );
    assert_eq!(
        ew::tables()["acoustic_countermeasures"]["internal_stationary_endurance_s"],
        360
    );
    assert_eq!(
        ew::tables()["acoustic_countermeasures"]["sonar_jammer_affects_torpedo_homing"],
        false
    );
    assert_eq!(
        ew::tables()["acoustic_countermeasures"]["torpedo_acm_affects_passive_search_sonar"],
        false
    );
}
#[test]
fn unknown_fits_inventory_and_large_external_life_block_without_side_effects() {
    let mut w = world();
    let (_, launcher) = ids(&w);
    let before = w.next_entity;
    assert!(ew::apply(&mut w, launcher, &deploy("unknown")).is_err());
    let mut f = fit(ew::ObjectKind::TorpedoAcm);
    f.launcher = Some(ew::Launcher::ExternalLarge);
    install(&mut w, "large", f, 1);
    assert!(ew::apply(&mut w, launcher, &deploy("large"))
        .unwrap_err()
        .contains("exact source"));
    assert_eq!(w.next_entity, before);
    assert!(w.rules.ew.as_ref().unwrap().stocks.is_empty());
    w.rules
        .ew
        .as_mut()
        .unwrap()
        .config
        .fits
        .get_mut("large")
        .unwrap()
        .endurance_s = Some(1261);
    assert!(ew::apply(&mut w, launcher, &deploy("large")).is_err());
    w.rules
        .ew
        .as_mut()
        .unwrap()
        .config
        .fits
        .get_mut("large")
        .unwrap()
        .endurance_s = Some(1260);
    ew::apply(&mut w, launcher, &deploy("large")).unwrap();
    assert!(ew::apply(&mut w, launcher, &deploy("large")).is_err());
    assert_eq!(ew::active_acms(&w, launcher)[0].generation, 3);
}
#[test]
fn actual_long_range_chaff_command_creates_one_false_echo_without_combat_ship_or_reroll() {
    let mut w = world();
    let (observer, launcher) = ids(&w);
    radar(&mut w, observer);
    install(&mut w, "chaff", fit(ew::ObjectKind::LongRangeChaff), 2);
    let action = ew::Action::Deploy {
        fit: "chaff".into(),
        at: Some(Vec2::new(7., 0.)),
        radar_signature: None,
        course_deg: None,
        speed_kts: None,
        depth_m: None,
    };
    let mut replay: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let command = [(
        Side::Red,
        Command::ElectronicWarfare {
            unit: launcher,
            action,
        },
    )];
    harpoon_core::sim::tick(&mut w, &command);
    harpoon_core::sim::tick(&mut replay, &command);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&replay).unwrap()
    );
    let object = object_id(&w);
    assert!(!w.entities.contains_key(&object));
    assert_eq!(w.entities.len(), 2);
    assert!(w.sides[0]
        .contacts
        .values()
        .any(|c| c.about == object && !c.bearing_only && c.class_name.is_none()));
    assert_eq!(harpoon_core::observe(&w, Side::Red, 0, 100).units.len(), 1);
    let signature = w.rules.ew.as_ref().unwrap().objects[&object].radar_signature;
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for item in [&mut w, &mut restored] {
        item.tick = 900;
        harpoon_core::sim::tick(item, &[]);
        assert_eq!(
            item.rules.ew.as_ref().unwrap().objects[&object].status,
            ew::Status::Expired
        );
        assert_eq!(
            item.rules.ew.as_ref().unwrap().objects[&object].radar_signature,
            signature
        );
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
#[test]
fn long_range_chaff_range_boundary_and_unknown_reflector_height_are_not_guessed() {
    let mut w = world();
    let (_, launcher) = ids(&w);
    let mut f = fit(ew::ObjectKind::LongRangeChaff);
    f.radar_height_m = None;
    install(&mut w, "chaff", f, 1);
    assert!(ew::apply(&mut w, launcher, &deploy("chaff"))
        .unwrap_err()
        .contains("height"));
    w.rules
        .ew
        .as_mut()
        .unwrap()
        .config
        .fits
        .get_mut("chaff")
        .unwrap()
        .radar_height_m = Some(1.);
    let action = ew::Action::Deploy {
        fit: "chaff".into(),
        at: Some(Vec2::new(7.00001, 0.)),
        radar_signature: None,
        course_deg: None,
        speed_kts: None,
        depth_m: None,
    };
    assert!(ew::apply(&mut w, launcher, &action).is_err());
    assert!(w.rules.ew.as_ref().unwrap().stocks.is_empty());
}
#[test]
fn floating_decoy_deployment_wind_sea_loss_and_early_sink_are_physical_state() {
    let mut w = world();
    let (observer, launcher) = ids(&w);
    radar(&mut w, observer);
    install(&mut w, "raft", fit(ew::ObjectKind::FloatingAsm), 2);
    let action = ew::Action::Deploy {
        fit: "raft".into(),
        at: None,
        radar_signature: Some(2),
        course_deg: None,
        speed_kts: None,
        depth_m: None,
    };
    ew::apply(&mut w, launcher, &action).unwrap();
    let id = object_id(&w);
    w.tick = 59;
    ew::step(&mut w);
    harpoon_core::sensors::update_sensors(&mut w);
    assert!(!w.sides[0].contacts.values().any(|c| c.about == id));
    w.tick = 60;
    ew::step(&mut w);
    harpoon_core::sensors::update_sensors(&mut w);
    assert!(w.sides[0].contacts.values().any(|c| c.about == id));
    w.tick = 3660;
    ew::step(&mut w);
    assert!((w.rules.ew.as_ref().unwrap().objects[&id].pos.x - 7.).abs() < 1e-9);
    assert!(ew::apply(&mut w, observer, &ew::Action::Sink { object: id }).is_err());
    ew::apply(&mut w, launcher, &ew::Action::Sink { object: id }).unwrap();
    assert_eq!(
        w.rules.ew.as_ref().unwrap().objects[&id].status,
        ew::Status::Sunk
    );
    w.rules
        .ew
        .as_mut()
        .unwrap()
        .config
        .environment
        .as_mut()
        .unwrap()
        .sea_state = 5;
    ew::apply(&mut w, launcher, &action).unwrap();
    assert!(w
        .rules
        .ew
        .as_ref()
        .unwrap()
        .objects
        .values()
        .any(|o| o.status == ew::Status::Swamped));
    assert_eq!(w.rules.ew.as_ref().unwrap().stocks[&launcher]["raft"], 0);
}
#[test]
fn sonar_jammer_cone_executes_in_actual_scan_and_never_becomes_torpedo_acm() {
    let mut w = world();
    let (observer, target) = ids(&w);
    sonar(&mut w, observer, false);
    let mut launcher = w.entity(target).unwrap().clone();
    launcher.id = w.allocate_id();
    launcher.pos = Vec2::new(10., 0.);
    let launcher_id = launcher.id;
    w.entities.insert(launcher_id, launcher);
    install(&mut w, "jam", fit(ew::ObjectKind::SonarJammer), 1);
    harpoon_core::sim::tick(&mut w, &[]);
    let first = w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == target)
        .unwrap()
        .last_update_tick;
    let action = deploy("jam");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Red,
            Command::ElectronicWarfare {
                unit: launcher_id,
                action,
            },
        )],
    );
    assert_eq!(
        w.sides[0]
            .contacts
            .values()
            .find(|c| c.about == target)
            .unwrap()
            .last_update_tick,
        first
    );
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{domain,detail,..} if domain=="detection2026"&&detail.contains("\"sonar_jammed\":true"))));
    assert!(ew::active_acms(&w, launcher_id).is_empty());
    let object = object_id(&w);
    assert!(w.sides[0]
        .contacts
        .values()
        .any(|c| c.about == object && c.bearing_only && c.class_name.is_none()));
    assert!(!w.entities.contains_key(&object));
    w.entity_mut(target).unwrap().pos = Vec2::zero().advanced(95.01, 2.);
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(
        w.sides[0]
            .contacts
            .values()
            .find(|c| c.about == target)
            .unwrap()
            .last_update_tick,
        w.tick
    );
}
#[test]
fn torpedo_acm_does_not_suppress_passive_search_and_expires_at_exact_two_turns() {
    let mut w = world();
    let (observer, target) = ids(&w);
    sonar(&mut w, observer, false);
    install(&mut w, "acm", fit(ew::ObjectKind::TorpedoAcm), 1);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Red,
            Command::ElectronicWarfare {
                unit: target,
                action: deploy("acm"),
            },
        )],
    );
    assert!(w.sides[0]
        .contacts
        .values()
        .any(|c| c.about == target && c.last_update_tick == w.tick));
    assert_eq!(ew::active_acms(&w, target).len(), 1);
    let object = object_id(&w);
    assert!(!w.sides[0].contacts.values().any(|c| c.about == object));
    w.tick = 360;
    ew::step(&mut w);
    assert_eq!(ew::active_acms(&w, target).len(), 1);
    w.tick = 361;
    ew::step(&mut w);
    assert!(ew::active_acms(&w, target).is_empty());
}
#[test]
fn towed_acm_requires_source_deployment_and_is_destroyed_after_successful_seduction() {
    let mut w = world();
    let (_, launcher) = ids(&w);
    let mut f = fit(ew::ObjectKind::TowedTorpedoDecoy);
    install(&mut w, "tow", f.clone(), 1);
    assert!(ew::apply(&mut w, launcher, &deploy("tow"))
        .unwrap_err()
        .contains("deployment time"));
    f.deployment_s = Some(30);
    install(&mut w, "tow", f, 1);
    ew::apply(&mut w, launcher, &deploy("tow")).unwrap();
    assert!(ew::active_acms(&w, launcher).is_empty());
    w.tick = 30;
    ew::step(&mut w);
    let defense = ew::active_acms(&w, launcher);
    assert_eq!(defense.len(), 1);
    assert!(defense[0].towed);
    ew::consume_towed(&mut w, defense[0].id);
    assert!(ew::active_acms(&w, launcher).is_empty());
    let seq = w.event_seq;
    ew::consume_towed(&mut w, defense[0].id);
    assert_eq!(seq, w.event_seq);
}
fn mobile_action(generation: u8) -> ew::Action {
    let speed = [7., 10., 12., 15.][generation as usize - 1];
    ew::Action::Deploy {
        fit: "mobile".into(),
        at: None,
        radar_signature: None,
        course_deg: Some(90.),
        speed_kts: Some(speed),
        depth_m: Some(100.),
    }
}
#[test]
fn mobile_all_generations_follow_source_speed_life_and_course_budgets() {
    for generation in 1..=4 {
        let mut w = world();
        let (_, launcher) = ids(&w);
        w.entity_mut(launcher).unwrap().kind = PlatformKind::Sub;
        let mut f = fit(ew::ObjectKind::MobileDecoy);
        f.generation = Some(generation);
        install(&mut w, "mobile", f, 1);
        ew::apply(&mut w, launcher, &mobile_action(generation)).unwrap();
        let id = object_id(&w);
        let maximum = if generation <= 2 { 0 } else { 2 };
        for j in 0..maximum {
            ew::apply(
                &mut w,
                launcher,
                &ew::Action::MobileCourse {
                    object: id,
                    course_deg: 120. + j as f64 * 30.,
                },
            )
            .unwrap();
        }
        assert!(ew::apply(
            &mut w,
            launcher,
            &ew::Action::MobileCourse {
                object: id,
                course_deg: 270.
            }
        )
        .is_err());
        let lifespan = [3600, 5400, 7200, 7200][generation as usize - 1];
        w.tick = lifespan;
        ew::step(&mut w);
        let o = &w.rules.ew.as_ref().unwrap().objects[&id];
        assert_eq!(o.status, ew::Status::Expired);
        assert!(
            (Vec2::new(2., 0.).range_to(o.pos) - o.speed_kts * lifespan as f64 / 3600.).abs()
                < 1e-9
        );
    }
}
#[test]
fn unknown_mobile_low_speed_energy_and_cross_side_state_are_blocked() {
    let mut w = world();
    let (observer, launcher) = ids(&w);
    w.entity_mut(launcher).unwrap().kind = PlatformKind::Sub;
    install(&mut w, "mobile", fit(ew::ObjectKind::MobileDecoy), 1);
    let mut action = mobile_action(3);
    if let ew::Action::Deploy { speed_kts, .. } = &mut action {
        *speed_kts = Some(6.);
    }
    assert!(ew::apply(&mut w, launcher, &action)
        .unwrap_err()
        .contains("energy"));
    assert!(w.rules.ew.as_ref().unwrap().objects.is_empty());
    assert!(ew::own_view(&w, Side::Blue, launcher).is_none());
    assert!(ew::own_view(&w, Side::Red, launcher).is_some());
    assert!(ew::own_view(&w, Side::Red, observer).is_none());
}
#[test]
fn mobile_active_echo_is_generation_dependent_in_real_sensor_pass() {
    for generation in [2, 3] {
        let mut w = world();
        let (observer, launcher) = ids(&w);
        sonar(&mut w, observer, true);
        w.entity_mut(launcher).unwrap().kind = PlatformKind::Sub;
        let mut f = fit(ew::ObjectKind::MobileDecoy);
        f.generation = Some(generation);
        install(&mut w, "mobile", f, 1);
        ew::apply(&mut w, launcher, &mobile_action(generation)).unwrap();
        let id = object_id(&w);
        harpoon_core::sensors::update_sensors(&mut w);
        assert_eq!(
            w.sides[0].contacts.values().any(|c| c.about == id),
            generation == 3
        );
        assert!(!w.entities.contains_key(&id));
    }
}
#[test]
fn mobile_mad_return_matches_source_third_and_fourth_generations_and_restore_keeps_motion() {
    for generation in [3, 4] {
        let mut w = world();
        let (observer, launcher) = ids(&w);
        w.entity_mut(launcher).unwrap().kind = PlatformKind::Sub;
        let mut f = fit(ew::ObjectKind::MobileDecoy);
        f.generation = Some(generation);
        install(&mut w, "mobile", f, 1);
        ew::apply(&mut w, launcher, &mobile_action(generation)).unwrap();
        let id = object_id(&w);
        let name = "source_ew_mad".to_string();
        let mut def = w.db.sensors.values().next().unwrap().clone();
        def.id = name.clone();
        def.kind = SensorKind::Magnetic;
        w.db.sensors.insert(name.clone(), def);
        {
            let o = w.entity_mut(observer).unwrap();
            o.kind = PlatformKind::Aircraft;
            o.altitude_m = 1000.;
            o.ordered_altitude_m = 1000.;
            o.sensors = vec![harpoon_core::world::SensorInstance {
                sensor: name.clone(),
                active: true,
            }];
        }
        w.rules.detection.as_mut().unwrap().config.bindings.insert(
            format!("us_ford:{name}"),
            d::Receiver::Mad {
                source: "5.7.1/12.6.2 generic mechanism fixture".into(),
                range_nm: 5.,
                western: false,
            },
        );
        while d::trial(
            &mut d::State::default(),
            w.seed,
            0,
            &format!("5.7.1:{}:{name}:{}", observer.0, id.0),
            180,
        )
        .0 > 10
        {
            w.seed += 1;
        }
        harpoon_core::sensors::update_sensors(&mut w);
        assert_eq!(
            w.sides[0].contacts.values().any(|c| c.about == id),
            matches!(generation,3|4)
        );
        let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        w.tick = 100;
        restored.tick = 100;
        ew::step(&mut w);
        ew::step(&mut restored);
        harpoon_core::sensors::update_sensors(&mut w);
        harpoon_core::sensors::update_sensors(&mut restored);
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            serde_json::to_value(&restored).unwrap()
        );
        assert!(!w.entities.contains_key(&id));
    }
}
#[test]
fn source_active_sonar_does_not_require_unrelated_passive_noise_metadata() {
    let mut w = world();
    let (observer, target) = ids(&w);
    sonar(&mut w, observer, true);
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .targets
        .get_mut("cn_001")
        .unwrap()
        .noise = None;
    harpoon_core::sensors::update_sensors(&mut w);
    assert!(
        w.sides[0].contacts.values().any(|c| c.about == target),
        "5.4.6.1 uses target strength, not passive noise rating"
    );
}

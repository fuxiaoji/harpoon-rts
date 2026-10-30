//! Real source ASCM regressions registered before production execution.
//! Explicit author knowledge/FCS isolates exact IC and Ford source mechanics.
use harpoon_core::rules::surface2026::antiship::{self as ascm, Phase};
use harpoon_core::{Command, ContactId, Database, EntityId, Rng, Scenario, Side, Vec2, World};
use serde_json::json;

const EXACT: &str = "us_rgm84d_ic";
const ACTOR: &str = "scenario_assumption:ic_test_launcher";

fn fixture(seed: u64) -> (World, EntityId, EntityId) {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    // Explicit authored carrier: the cloned numeric hull is not a real fitted
    // Burke claim. Only the exact IC missile and target Ford are source facts.
    let mut platform = db.platforms["us_burke_2a"].clone();
    platform.id = ACTOR.into();
    platform.name = "scenario_assumption: isolated IC test launcher".into();
    let mut mount = platform.weapons[0].clone();
    mount.weapon = EXACT.into();
    mount.ammo = 4;
    platform.weapons = vec![mount];
    db.platforms.insert(ACTOR.into(), platform);
    let scenario:Scenario=serde_json::from_value(json!({
        "id":"r0841-controlled-source-chain","name":"Explicit controlled IC / Ford source chain",
        "duration_s":2000,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},"radar_land_clutter":[],"antiship_2026":{
            "source":"scenario_assumption: declared flight order, observer knowledge and hardkill unavailability; exact weapon/Ford rows remain original source",
            "launchers":[{"actor_name":"LAUNCHER","weapon":EXACT,"exact_variant":"RGM-84D Harpoon IC surface launch","max_rounds_per_turn":4,"source":"scenario_assumption: explicit exact IC carriage on authored trial launcher"}],
            "author_fcs":[{"actor_name":"LAUNCHER","contact":71,"weapon":EXACT,"quality":"Good","source":"scenario_assumption: controlled own Good solution, not a claimed real FCS device"}],
            "author_awareness":[{"defender_name":"FORD","shooter_name":"LAUNCHER","weapon":EXACT,"from":0,"until":1800,"source":"scenario_assumption: explicit known incoming attack for controlled countermeasure adjudication"}],
            "author_countermeasure_availability":[{"defender_name":"FORD","jamming_available":true,"decoy_available":true,"source":"scenario_assumption: undamaged available source combined J&D; no new model or stock inferred"}]}},
        "units":[
            {"side":"Blue","class":ACTOR,"name":"LAUNCHER","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_ford","name":"FORD","pos":[0,60],"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":"cn_093","name":"scenario_assumption: blue trial reserve","pos":[1000,1000],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"scenario_assumption: red trial reserve","pos":[-1000,-1000],"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut world = harpoon_core::load_scenario(scenario, db).unwrap();
    let actor = world
        .entities
        .values()
        .find(|e| e.name == "LAUNCHER")
        .unwrap()
        .id;
    let target = world
        .entities
        .values()
        .find(|e| e.name == "FORD")
        .unwrap()
        .id;
    for unit in world.entities.values_mut() {
        unit.sensors.clear();
        if unit.id != actor {
            unit.weapons.clear();
        }
    }
    world.sides[0].contacts.insert(
        ContactId(71),
        harpoon_core::world::Contact {
            id: ContactId(71),
            about: target,
            category: harpoon_core::types::PlatformKind::Ship,
            class_name: None,
            estimate: Vec2::new(0., 60.),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec!["scenario_assumption: controlled own located ship report".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    world.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", target.0),
        harpoon_core::types::PlatformKind::Ship,
    );
    // The exact new Def already has its source rule_data; no legacy Bernoulli
    // or generic DP is allowed to account for the expected source hit.
    world.db.weapons.get_mut(EXACT).unwrap().pk = 0.;
    world.db.weapons.get_mut(EXACT).unwrap().damage = 0.;
    (world, actor, target)
}

fn plot_and_fire(w: &mut World, actor: EntityId) -> EntityId {
    let plot: Command = serde_json::from_value(json!({"cmd":"antiship","unit":actor,
        "action":{"type":"plot","contact":71,"weapon":EXACT,"course_deg":0.,
            "speed_kts":561.,"enable_run_nm":40.,"waypoints":[],"popup":false}}))
    .unwrap();
    harpoon_core::sim::tick(w, &[(Side::Blue, plot)]);
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: actor,
                contact: ContactId(71),
                weapon: EXACT.into(),
                rounds: 1,
            },
        )],
    );
    assert_eq!(
        w.stats.projectiles_launched, 1,
        "real Fire must pass own typed Plot/FCS and physically launch once"
    );
    *w.rules
        .aaw
        .as_ref()
        .unwrap()
        .surface
        .antiship
        .flights
        .keys()
        .next()
        .expect("real release must register the complete source flight")
}

// Proposed stable identities are part of next-window review, not injected
// dice. If accepted, the executor must use these same source identities.
fn seed_for(actor: EntityId, projectile: EntityId, d10: u8, d100: u8) -> u64 {
    for seed in 0..5_000_000 {
        let rng = Rng::new(seed);
        let placement = format!("6.3.8.ascm:{}:{EXACT}:0", actor.0);
        let mut p = rng.derive(Rng::hash_tag(0, 0, &placement));
        let pd = 1 + (p.next_f64() * 10.) as u8;
        let attack = format!("8.4.1.ascm:{}:0", projectile.0);
        let mut a = rng.derive(Rng::hash_tag(0, 0, &attack));
        let ad = 1 + (a.next_f64() * 100.) as u8;
        if pd == d10 && ad == d100 {
            return seed;
        }
    }
    panic!("a real original seed for the declared boundary was not found")
}

#[test]
fn real_ic_source_47_lock_reaches_the_actual_damage_queue_and_saves_full_world() {
    let (probe, actor, _) = fixture(1);
    let projectile = EntityId(probe.next_entity + 1);
    let seed = seed_for(actor, projectile, 9, 47);
    let (mut world, actor, target) = fixture(seed);
    let initial_hp = world.entity(target).unwrap().hp;
    let id = plot_and_fire(&mut world, actor);
    assert_eq!(id, projectile);
    while world.rules.aaw.as_ref().unwrap().surface.antiship.flights[&id].actual_run_nm < 29.5 {
        harpoon_core::sim::tick(&mut world, &[]);
        assert!(
            world.rules.aaw.as_ref().unwrap().surface.antiship.flights[&id]
                .placement
                .is_none(),
            "no source placement before actual half distance"
        );
    }
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    for _ in 0..500 {
        harpoon_core::sim::tick(&mut world, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
        if matches!(
            world.rules.aaw.as_ref().unwrap().surface.antiship.flights[&id].phase,
            Phase::ImpactQueued
        ) {
            break;
        }
    }
    let flight = &world.rules.aaw.as_ref().unwrap().surface.antiship.flights[&id];
    assert_eq!(flight.placement.as_ref().unwrap().d10, 9);
    assert_eq!(flight.lock_trials[0].d100, 47);
    assert_eq!(flight.lock_trials[0].probability, 0.47);
    assert!(flight.lock_trials[0].locked);
    assert!(world.entity(target).unwrap().hp<initial_hp,"real source impact must affect the ordinary source damage queue despite legacy pk/damage zero");
    assert_eq!(
        serde_json::to_value(&world).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn real_ic_source_48_miss_retains_its_actual_course_and_has_no_unsupported_cloverleaf() {
    let (probe, actor, _) = fixture(1);
    let seed = seed_for(actor, EntityId(probe.next_entity + 1), 9, 48);
    let (mut world, actor, target) = fixture(seed);
    let hp = world.entity(target).unwrap().hp;
    let id = plot_and_fire(&mut world, actor);
    for _ in 0..500 {
        harpoon_core::sim::tick(&mut world, &[]);
        if matches!(
            world.rules.aaw.as_ref().unwrap().surface.antiship.flights[&id].phase,
            Phase::MissContinue
        ) {
            break;
        }
    }
    let before = world.rules.aaw.as_ref().unwrap().surface.antiship.flights[&id].clone();
    assert_eq!(before.lock_trials[0].d100, 48);
    assert!(!before.lock_trials[0].locked);
    assert_eq!(world.entity(target).unwrap().hp, hp);
    let original_position = world.entity(id).unwrap().pos;
    for _ in 0..20 {
        harpoon_core::sim::tick(&mut world, &[]);
    }
    let after = &world.rules.aaw.as_ref().unwrap().surface.antiship.flights[&id];
    assert_eq!(
        after.reattacks, 0,
        "IC remarksL/W3 contain no M reattack capability"
    );
    assert_eq!(
        after.lock_trials.len(),
        1,
        "the same miss cannot roll again each second"
    );
    assert_eq!(after.actual_course_deg, before.actual_course_deg);
    assert!(
        after.actual_run_nm > before.actual_run_nm
            && after.remaining_range_nm < before.remaining_range_nm
    );
    assert_ne!(world.entity(id).unwrap().pos, original_position);
    assert_eq!(world.entity(target).unwrap().hp, hp);
}

#[test]
fn failed_source_placement_uses_original_deviation_and_same_turn_salvo_dice() {
    let (probe, actor, _) = fixture(1);
    let seed = seed_for(actor, EntityId(probe.next_entity + 1), 10, 48);
    let (mut world, actor, target) = fixture(seed);
    let plot:Command=serde_json::from_value(json!({"cmd":"antiship","unit":actor,"action":{"type":"plot","contact":71,"weapon":EXACT,"course_deg":0.,"speed_kts":561.,"enable_run_nm":40.,"waypoints":[],"popup":false}})).unwrap();
    harpoon_core::sim::tick(&mut world, &[(Side::Blue, plot)]);
    harpoon_core::sim::tick(
        &mut world,
        &[(
            Side::Blue,
            Command::Fire {
                unit: actor,
                contact: ContactId(71),
                weapon: EXACT.into(),
                rounds: 2,
            },
        )],
    );
    assert_eq!(world.stats.projectiles_launched, 2);
    let ids: Vec<_> = world
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .surface
        .antiship
        .flights
        .keys()
        .copied()
        .collect();
    let hp = world.entity(target).unwrap().hp;
    for _ in 0..300 {
        harpoon_core::sim::tick(&mut world, &[]);
    }
    let s = &world.rules.aaw.as_ref().unwrap().surface.antiship;
    let a = &s.flights[&ids[0]];
    let b = &s.flights[&ids[1]];
    let pa = a.placement.as_ref().unwrap();
    let pb = b.placement.as_ref().unwrap();
    assert_eq!(pa.d10, 10);
    assert!(!pa.placed);
    assert_eq!(
        serde_json::to_value(pa).unwrap(),
        serde_json::to_value(pb).unwrap(),
        "source salvo shares the original raw D10 and both D6, not new projectile rolls"
    );
    let expected =
        (if pa.deviation_d6[0] <= 3 { 1 } else { -1 }) * i32::from(pa.deviation_d6[1]) * 5;
    assert_eq!(pa.course_deviation_deg, expected);
    assert_eq!(
        a.actual_course_deg,
        harpoon_core::geom::normalize_deg(expected as f64)
    );
    assert_eq!(a.actual_course_deg, b.actual_course_deg);
    assert!(
        a.lock_trials.is_empty() && b.lock_trials.is_empty(),
        "failed placement excludes its intended target"
    );
    assert_eq!(world.entity(target).unwrap().hp, hp);
}

#[test]
fn enabled_search_unknown_source_target_keeps_full_pending_and_never_leaks_raw_dice() {
    let (probe, actor, _) = fixture(1);
    let seed = seed_for(actor, EntityId(probe.next_entity + 1), 9, 47);
    let (mut world, actor, target) = fixture(seed);
    world.entity_mut(target).unwrap().class =
        "scenario_assumption: unbound hidden target profile".into();
    let id = plot_and_fire(&mut world, actor);
    for _ in 0..300 {
        harpoon_core::sim::tick(&mut world, &[]);
    }
    let f = &world.rules.aaw.as_ref().unwrap().surface.antiship.flights[&id];
    assert_eq!(f.phase, Phase::Pending);
    assert!(f.pending.is_some() && f.lock_trials.is_empty());
    assert!(world.rules.pending_adjudications.contains_key(&id));
    let original = serde_json::to_value(f).unwrap();
    let view = serde_json::to_value(harpoon_core::observe(&world, Side::Blue, 0, 10000)).unwrap();
    let events = view["events"].to_string();
    assert!(
        !events.contains("source_lock")
            && !events.contains("source_placement")
            && !events.contains("unbound hidden target profile")
    );
    let own = view["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == actor.0)
        .unwrap()["antiship"]
        .to_string();
    assert!(
        own.contains("source_executor")
            && !own.contains("d100")
            && !own.contains("intended_target")
            && !own.contains("locked_target")
    );
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    for _ in 0..20 {
        harpoon_core::sim::tick(&mut world, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_value(&world.rules.aaw.as_ref().unwrap().surface.antiship.flights[&id])
            .unwrap(),
        original
    );
    assert_eq!(
        serde_json::to_value(&world).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn pre_enable_physical_course_does_not_steer_to_a_secret_target_and_generic_chaff_is_not_source_ecm(
) {
    let (probe, actor, _) = fixture(1);
    let seed = seed_for(actor, EntityId(probe.next_entity + 1), 9, 47);
    let (mut left, actor, target) = fixture(seed);
    let id = plot_and_fire(&mut left, actor);
    let mut right = left.clone();
    right.entity_mut(target).unwrap().pos = Vec2::new(40., 60.);
    for _ in 0..160 {
        harpoon_core::sim::tick(&mut left, &[]);
        harpoon_core::sim::tick(&mut right, &[]);
    }
    assert_eq!(left.entity(id).unwrap().pos, right.entity(id).unwrap().pos);
    assert_eq!(
        left.entity(id).unwrap().course_deg,
        right.entity(id).unwrap().course_deg
    );
    assert!(
        left.rules.aaw.as_ref().unwrap().surface.antiship.flights[&id]
            .enabled_at
            .is_none()
    );
    let legacy = left.entity_mut(target).unwrap();
    legacy.auto_chaff = true;
    legacy.chaff = 200;
    legacy.chaff_active_s = 500.;
    for _ in 0..300 {
        harpoon_core::sim::tick(&mut left, &[]);
    }
    let f = &left.rules.aaw.as_ref().unwrap().surface.antiship.flights[&id];
    assert_eq!(f.lock_trials[0].probability, 0.47);
    assert_eq!(f.lock_trials[0].d100, 47);
    assert!(!left
        .entity(target)
        .unwrap()
        .ext
        .contains_key("generic_chaff_seduction"));
}

#[test]
fn source_table_cells_and_player_plot_cannot_inject_referee_quality_or_countermeasures() {
    assert_eq!(
        ascm::table_probability("Large", Some(4), 3, "jamming_decoy").unwrap(),
        0.47
    );
    assert_eq!(
        ascm::table_probability("Small", Some(3), 2, "jamming_decoy").unwrap(),
        0.26
    );
    assert_eq!(
        ascm::table_probability("Large", None, 3, "base").unwrap(),
        0.80
    );
    let bad: Result<Command, _> = serde_json::from_value(
        json!({"cmd":"antiship","unit":1,"action":{"type":"plot","contact":71,"weapon":EXACT,"course_deg":0.,"speed_kts":561.,"enable_run_nm":40.,"waypoints":[],"popup":false,"quality":"Good","ecm_generation":4}}),
    );
    assert!(
        bad.is_err(),
        "player/LLM Plot cannot carry referee quality or enemy ECM"
    );
}

#[test]
fn plot_validation_checks_own_selected_contact_and_orders_before_storing_any_plot() {
    let (mut world, actor, target) = fixture(1);
    let valid = json!({"type":"plot","contact":71,"weapon":EXACT,"course_deg":0.,
        "speed_kts":561.,"enable_run_nm":40.,"waypoints":[],"popup":false});
    let action: ascm::Action = serde_json::from_value(valid.clone()).unwrap();
    assert!(ascm::validate_plot(&world, actor, &action).is_ok());
    // Orders depend on the reported own contact, never the referee target.
    world.entity_mut(target).unwrap().alive = false;
    world.entity_mut(target).unwrap().pos = Vec2::new(500., -500.);
    assert!(ascm::validate_plot(&world, actor, &action).is_ok());
    for (field, value) in [
        ("contact", json!(99)),
        ("speed_kts", json!(1.)),
        ("enable_run_nm", json!(20.)),
        ("popup", json!(true)),
        ("waypoints", json!([{"x":0.,"y":1.}])),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = value;
        let invalid: ascm::Action = serde_json::from_value(invalid).unwrap();
        assert!(ascm::validate_plot(&world, actor, &invalid).is_err());
        assert!(ascm::apply(&mut world, actor, &invalid).is_err());
        assert!(world
            .rules
            .aaw
            .as_ref()
            .unwrap()
            .surface
            .antiship
            .plots
            .is_empty());
    }
    let report = world.sides[0].contacts.remove(&ContactId(71)).unwrap();
    world.sides[1].contacts.insert(ContactId(71), report);
    assert!(ascm::validate_plot(&world, actor, &action).is_err());
    assert!(ascm::apply(&mut world, actor, &action).is_err());
    assert!(world
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .surface
        .antiship
        .plots
        .is_empty());
}

fn incoming_pd_fixture() -> (World, EntityId, EntityId) {
    incoming_pd_fixture_with_seed(1)
}

fn incoming_pd_fixture_with_seed(seed: u64) -> (World, EntityId, EntityId) {
    let (mut world, actor, defender) = fixture(seed);
    let projectile = plot_and_fire(&mut world, actor);
    // scenario_assumption: isolate source point-defense geometry, rather than
    // claiming that the existing full route or automatic detection moved here.
    // The incoming remains a physical Fire-registered exact no-popup IC Flight.
    let incoming = world.entity_mut(projectile).unwrap();
    incoming.pos = Vec2::new(0., 59.25);
    incoming.altitude_m = 5.;
    incoming.course_deg = 0.;
    incoming.speed_kts = 561.;
    let ship = world.entity_mut(defender).unwrap();
    ship.course_deg = 180.;
    ship.weapons = vec![
        harpoon_core::world::WeaponMount {
            weapon: "ciws_30mm".into(),
            mounts: 3,
            ammo: 21,
            cooldown_s: 0.,
        },
        harpoon_core::world::WeaponMount {
            weapon: "us_essm_i".into(),
            mounts: 1,
            ammo: 16,
            cooldown_s: 0.,
        },
    ];
    ship.sensors = ["radar_air_long", "us_spy3_fc", "visual"]
        .into_iter()
        .map(|sensor| harpoon_core::world::SensorInstance {
            sensor: sensor.into(),
            active: true,
        })
        .collect();
    ship.ext.insert("emission.radar".into(), 1.);
    (world, defender, projectile)
}

#[test]
fn registered_no_popup_ic_can_receive_source_ciws_fire_without_inventing_mr() {
    let (mut world, defender, projectile) = incoming_pd_fixture();
    let gun = &world.db.weapons["ciws_30mm"];
    let permit = harpoon_core::rules::aaw2026::world_gun_solution(
        &world,
        world.rules.aaw.as_ref().unwrap(),
        defender,
        projectile,
        gun,
        0,
    )
    .expect("MR is not an input to source AA gun adjudication for a registered no-popup IC");
    assert_eq!(permit.solution.modifiers["speed"], 0.);
    assert_eq!(permit.solution.modifiers["signature"], -1.);
    assert_eq!(permit.solution.modifiers["maneuvers"], 1.);
    assert_eq!(permit.solution.value, 9.5);
    assert_eq!(permit.solution.probability, 0.84);
    let profile = ascm::incoming_aaw_profile(&world, projectile)
        .unwrap()
        .unwrap();
    assert!(profile.get("maneuver_rating").unwrap().is_null());
    assert_eq!(profile["maneuvering"], false);
    assert_eq!(profile["terminal_maneuvers"], false);
    world
        .rules
        .aaw
        .as_mut()
        .unwrap()
        .surface
        .antiship
        .flights
        .get_mut(&projectile)
        .unwrap()
        .plotted
        .popup = true;
    assert!(ascm::incoming_aaw_profile(&world, projectile).is_err());
    world
        .rules
        .aaw
        .as_mut()
        .unwrap()
        .surface
        .antiship
        .flights
        .remove(&projectile);
    assert!(ascm::incoming_aaw_profile(&world, projectile).is_err());
}

#[test]
fn registered_ic_sam_packet_stays_pending_for_unknown_mr_without_consuming_ammunition() {
    let (world, defender, projectile) = incoming_pd_fixture();
    let sam = &world.db.weapons["us_essm_i"];
    let error = harpoon_core::rules::aaw2026::world_prepare(
        &world,
        world.rules.aaw.as_ref().unwrap(),
        defender,
        projectile,
        sam,
        1,
    )
    .err()
    .expect("strict source SAM requires a sourced missile MR; null is not zero");
    assert!(error.contains("MR"), "must block on unresolved MR: {error}");
    assert_eq!(world.entity(defender).unwrap().weapons[1].ammo, 16);
    assert!(world.rules.aaw.as_ref().unwrap().shots.is_empty());
}

#[test]
fn ic_radar_uses_source_rcs_while_visual_still_requires_unknown_physical_size() {
    use harpoon_core::rules::detection2026 as detection;
    let (world, defender, projectile) = incoming_pd_fixture();
    let mut state = world.rules.detection.as_ref().unwrap().clone();
    state.config.environment = Some(detection::Environment {
        source: "scenario_assumption: explicit clear incoming sensor trial environment".into(),
        sea_state: 0,
        precipitation: 0,
        visibility_percent: 100,
        shipping: 0,
        biologics: 0,
        water_depth_m: 1000.,
        layer: None,
        ice: 0,
        cloud: None,
    });
    let radar = &world.db.sensors["radar_air_long"];
    let range = detection::radar_range_world(
        &world,
        &state,
        world.entity(defender).unwrap(),
        radar,
        world.entity(projectile).unwrap(),
    )
    .expect("source Ford SPY4 radar needs IC RCS, not its unknown physical size");
    assert!(range > 0.75);
    let scan =
        detection::evaluate_world(&world, &mut state, defender, "radar_air_long", projectile)
            .unwrap();
    assert_eq!(scan.trace["rule"], "5.2");
    let error = detection::evaluate_world(&world, &mut state, defender, "visual", projectile)
        .err()
        .expect("radar VSmall is not a verified physical size for visual detection");
    assert!(
        error.contains("size"),
        "visual must block for physical size: {error}"
    );
}

#[test]
fn actual_ciws_contact_fire_retires_registered_ic_and_never_queues_later_ship_damage() {
    let (probe, defender, incoming) = incoming_pd_fixture();
    let identity = format!("aaw:gun:{}:ciws_30mm:0:{}:1:1", defender.0, probe.tick);
    // Source 8.1.5 still permits fragments after a successful interception.
    // Select an original fragment-miss seed as well, rather than asserting
    // that interception universally makes source fragments impossible.
    let mut sequence_probe = probe.clone();
    let fragment_identity = harpoon_core::rules::damage_runtime::event_identity(
        &mut sequence_probe,
        incoming,
        "8.1.5.fragment",
    );
    let seed = (0..50_000)
        .find(|seed| {
            let rng = Rng::new(*seed);
            let mut actual = rng.derive(Rng::hash_tag(probe.tick, defender.0, &identity));
            let mut fragments =
                rng.derive(Rng::hash_tag(probe.tick, incoming.0, &fragment_identity));
            1 + (actual.next_f64() * 100.) as u8 == 84
                && 1 + (fragments.next_f64() * 100.) as u8 > 10
        })
        .expect("find original CIWS D100=84 and a source 10% fragment miss, never inject outcomes");
    let (mut world, defender, projectile) = incoming_pd_fixture_with_seed(seed);
    let now = world.tick;
    let position = world.entity(projectile).unwrap().pos;
    world.sides[Side::Red.index()].contacts.insert(
        ContactId(81),
        harpoon_core::world::Contact {
            id: ContactId(81),
            about: projectile,
            category: harpoon_core::types::PlatformKind::Missile,
            class_name: None,
            estimate: position,
            est_course_deg: 0.,
            est_speed_kts: 561.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: now,
            first_tick: now,
            stale: false,
            sensors: vec!["scenario_assumption: controlled own incoming missile report".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    world.rules.detection.as_mut().unwrap().categories.insert(
        format!("{}:{}", Side::Red.index(), projectile.0),
        harpoon_core::types::PlatformKind::Missile,
    );
    let hp = world.entity(defender).unwrap().hp;
    let ammo = world.entity(defender).unwrap().weapons[0].ammo;
    assert!(harpoon_core::weapons::manual_fire(
        &mut world,
        Side::Red,
        defender,
        ContactId(81),
        "ciws_30mm",
        1,
    ));
    let outcome = &world.rules.aaw.as_ref().unwrap().outcomes[&identity];
    assert_eq!(outcome.d100, 84);
    assert_eq!(outcome.probability, 0.84);
    assert!(outcome.hit);
    let fragments = world
        .events
        .iter()
        .find_map(|event| match &event.kind {
            harpoon_core::EventKind::RulesEvent { rule, detail, .. }
                if rule == "8.1.5.fragment_roll" =>
            {
                serde_json::from_str::<serde_json::Value>(detail).ok()
            }
            _ => None,
        })
        .expect("actual interception preserves the independent original source fragment roll");
    assert_eq!(fragments["probability"], 0.1);
    assert!(fragments["d100"].as_u64().unwrap() > 10);
    assert_eq!(fragments["struck"], false);
    assert_eq!(world.entity(defender).unwrap().weapons[0].ammo, ammo - 1);
    assert!(!world.entity(projectile).unwrap().alive);
    assert_eq!(
        world.rules.aaw.as_ref().unwrap().surface.antiship.flights[&projectile].phase,
        Phase::Intercepted,
    );
    assert!(world.contact(Side::Red, ContactId(81)).is_none());
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    for _ in 0..500 {
        harpoon_core::sim::tick(&mut world, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    let flight = &world.rules.aaw.as_ref().unwrap().surface.antiship.flights[&projectile];
    assert_eq!(flight.phase, Phase::Intercepted);
    assert!(!flight.damage_queued);
    assert_eq!(world.entity(defender).unwrap().hp, hp);
    assert_eq!(
        serde_json::to_value(&world).unwrap(),
        serde_json::to_value(&restored).unwrap(),
    );
}

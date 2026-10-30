//! Resume-batch candidates, implemented_unverified until the root combined run.
//! Optional §14.9 casualties below come from actual committed native primaries.
//! Loads and exact component allocations are scenario assumptions, not Annex fits.
use harpoon_core::rules::{
    aam2026 as aam, aircombat2026 as ac, aviation2026 as av, damage_runtime as dr,
    expanded_damage2026 as ex,
};
use harpoon_core::{Database, EntityId, Scenario, World};
use serde_json::json;

fn fixture(seed: u64, payload: f64) -> (World, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let cruise = av::source_speed("us_f18e", 4000., av::Throttle::Cruise).unwrap();
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"expanded-consumer-resume-lab","name":"scenario_assumption: optional damage consumer laboratory",
        "seed":seed,"duration_s":5000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "aviation_2026":{"source":"scenario_assumption: declared external load and remaining range","parking":{},
                "loads":{"us_f18e":{"payload_kg":payload,"tanks":[],"source":"scenario_assumption: explicit load",
                    "external_weapon_rounds":{"us_aim120d":2}}},"initial_fuel_nm":{"AIR":700}},
            "expanded_damage_2026":{"source":"scenario_assumption: explicit optional-rule selection and physical decoy allocation",
                "optional":{"aircraft_damage":"immediate_detailed"},
                "units":{"AIR":{"source":"scenario_assumption: cruise derived from current source altitude row",
                    "cruise_speed_kts":cruise,"crew":2,
                    "components":{"decoy":{"source":"scenario_assumption: declared whole decoy subsystem",
                        "kind":"countermeasure","binding":"decoy"}}}}}},
        "units":[{"side":"Blue","class":"us_f18e","name":"AIR","pos":[0,0],
            "speed_kts":cruise,"course_deg":0,"altitude_m":4000,"fire_on":false}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    av::initialize(&mut w).unwrap();
    let id = w.entities.values().find(|e| e.name == "AIR").unwrap().id;
    (w, id)
}

fn native_hit(w: &mut World, id: EntityId) {
    let mut def = w.db.weapons["us_aim120d"].clone();
    def.id = "scenario_resume_primary".into();
    def.name = "scenario_assumption: adjudicated 20 DP laboratory primary".into();
    def.kind = harpoon_core::db::WeaponKind::Rocket;
    def.damage = 20.;
    def.rule_data = Some(dr::WeaponDamage {
        source: "scenario_assumption: exact committed native primary, not an Annex weapon".into(),
        ship_dp: Some(20.),
        sub_dp: Some(20.),
        penetration_cm: Some(10000.),
        ..Default::default()
    });
    harpoon_core::damage::apply_hit_at(w, id, &def, false, None);
}

fn native_damage(
    payload: f64,
    predicate: impl Fn(&ex::AircraftDamage) -> bool,
) -> (World, EntityId) {
    for seed in 0..2000 {
        let (mut w, id) = fixture(seed, payload);
        native_hit(&mut w, id);
        if w.rules
            .expanded_damage
            .as_ref()
            .unwrap()
            .units
            .get(&id)
            .and_then(|u| u.aircraft.as_ref())
            .is_some_and(&predicate)
        {
            return (w, id);
        }
    }
    panic!("no source-derived native seed satisfied the damage predicate");
}

#[test]
fn native_heavy_damage_changes_actual_motion_altitude_and_source_throttle_gate() {
    let (mut w, id) = native_damage(4000., |d| d.severity == ex::AirSeverity::Heavy);
    let nominal = av::source_speed("us_f18e", 4000., av::Throttle::Cruise).unwrap();
    w.entity_mut(id).unwrap().ordered_speed_kts = 2000.;
    w.entity_mut(id).unwrap().ordered_altitude_m = 5000.;
    let limits = av::movement_limits(&w, id).unwrap();
    let reduced_mr = 1.5 / 2.;
    assert_eq!(ac::target_maneuver(&w, id).unwrap(), reduced_mr);
    assert_eq!(
        limits.climb_mps,
        av::altitude_rate(av::Engine::TF, false, reduced_mr, 4000., true, true)
            .unwrap()
            .0
    );
    assert!(limits.maximum_kts <= nominal);
    let before = serde_json::to_vec(&w).unwrap();
    assert!(av::apply(
        &mut w,
        id,
        &av::Action::Throttle {
            setting: av::Throttle::Fmp
        }
    )
    .is_err());
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        before,
        "rejected throttle must be read-only"
    );
    let altitude = w.entity(id).unwrap().altitude_m;
    harpoon_core::movement::update_movement(&mut w);
    assert!(w.entity(id).unwrap().speed_kts <= nominal);
    assert!((w.entity(id).unwrap().altitude_m - altitude - limits.climb_mps).abs() < 1e-10);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    harpoon_core::movement::update_movement(&mut w);
    harpoon_core::movement::update_movement(&mut restored);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn damage_produced_between_printed_jet_maneuver_rows_remains_source_pending() {
    let (mut w, id) = native_damage(1000., |d| d.severity == ex::AirSeverity::Heavy);
    assert_eq!(ac::target_maneuver(&w, id).unwrap(), 2.25);
    assert!(
        av::movement_limits(&w, id).is_ok(),
        "source-supported level motion remains available"
    );
    w.entity_mut(id).unwrap().ordered_altitude_m = 5000.;
    assert!(av::movement_limits(&w, id)
        .unwrap_err()
        .contains("between printed 2.0 and 2.5 rows"));
    let prior = w.entity(id).unwrap().pos;
    harpoon_core::movement::update_movement(&mut w);
    assert_eq!(
        w.entity(id).unwrap().pos,
        prior,
        "unprinted climb category must not be guessed"
    );
}

#[test]
fn native_critical_load_jettison_changes_zero_maneuver_to_half_and_keeps_all_systems_down() {
    let (mut w, id) = native_damage(4000., |d| d.severity == ex::AirSeverity::Critical);
    assert_eq!(ac::target_maneuver(&w, id).unwrap(), 0.);
    assert_eq!(av::movement_limits(&w, id).unwrap().climb_mps, 0.);
    assert!(ac::release_allowed(&w, id).is_err());
    let sensor = w.entity(id).unwrap().sensors[0].sensor.clone();
    assert!(!ex::sensor_operational(&w, id, &sensor).unwrap());
    let cm = aam::aircraft_countermeasures(&w, id, 5).unwrap();
    assert!(cm.jammer_generation.is_none() && cm.decoy_generation.is_none());
    let launches = w.stats.projectiles_launched;
    av::apply(
        &mut w,
        id,
        &av::Action::Jettison {
            weapon: "us_aim120d".into(),
            rounds: 1,
        },
    )
    .unwrap();
    assert!(w.rules.aviation.as_ref().unwrap().flights[&id].payload_fraction < 0.5);
    assert_eq!(ac::target_maneuver(&w, id).unwrap(), 0.5);
    assert_eq!(w.stats.projectiles_launched, launches);
    w.rules
        .aircombat
        .as_mut()
        .unwrap()
        .orders
        .insert(id, ac::Order::Defend);
    assert_eq!(
        ac::target_maneuver(&w, id).unwrap(),
        0.5,
        "critical fixed source rating has no tactical bonus"
    );
}

#[test]
fn native_heavy_keeps_countermeasures_but_cannot_release_any_offensive_weapon() {
    let (w, id) = native_damage(1000., |d| d.severity == ex::AirSeverity::Heavy);
    let cm = aam::aircraft_countermeasures(&w, id, 5).unwrap();
    assert_eq!(
        cm.jammer_generation,
        aam::aircraft_profile("us_f18e").unwrap()["jammer"]
            .as_u64()
            .map(|v| v as usize)
    );
    assert_eq!(
        cm.decoy_generation,
        aam::aircraft_profile("us_f18e").unwrap()["decoy"]
            .as_u64()
            .map(|v| v as usize)
    );
    assert!(!ex::weapon_operational(&w, id, "us_aim120d").unwrap());
    assert!(ac::release_allowed(&w, id).is_err());
}

#[test]
fn native_pilot_damage_has_two_thirds_maneuver_and_preserves_cruise_unrestricted_speed() {
    let (mut w, id) = native_damage(1000., |d| {
        d.severity == ex::AirSeverity::Light && d.light.contains("pilot")
    });
    assert_eq!(ac::target_maneuver(&w, id).unwrap(), 3.);
    assert!(ac::release_allowed(&w, id).is_err());
    assert_eq!(ex::aircraft_speed_cap(&w, id).unwrap(), None);
    av::apply(
        &mut w,
        id,
        &av::Action::Throttle {
            setting: av::Throttle::Fmp,
        },
    )
    .unwrap();
    w.rules
        .aircombat
        .as_mut()
        .unwrap()
        .orders
        .insert(id, ac::Order::Defend);
    assert_eq!(
        ac::target_maneuver(&w, id).unwrap(),
        3.5,
        "RTS composition applies source damage before tactical bonus"
    );
}

#[test]
fn native_recorded_jettison_failure_rejects_without_reducing_stores_or_rerolling() {
    let (mut w, id) = native_damage(1000., |d| {
        d.severity == ex::AirSeverity::Light
            && d.light.contains("offensive_weapons")
            && d.jettison_failed == Some(true)
    });
    let saved = serde_json::to_vec(&w).unwrap();
    for _ in 0..2 {
        assert!(av::apply(
            &mut w,
            id,
            &av::Action::Jettison {
                weapon: "us_aim120d".into(),
                rounds: 1
            }
        )
        .is_err());
        assert_eq!(serde_json::to_vec(&w).unwrap(), saved);
    }
    assert!(ac::release_allowed(&w, id).is_err());
}

#[test]
fn repaired_or_disabled_optional_aircraft_rule_cannot_leave_old_motion_or_attack_restrictions() {
    let (mut w, id) = native_damage(1000., |d| d.severity == ex::AirSeverity::Heavy);
    w.rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap()
        .aircraft
        .as_mut()
        .unwrap()
        .repaired = true;
    assert_eq!(ac::target_maneuver(&w, id).unwrap(), 4.5);
    assert!(ac::release_allowed(&w, id).is_ok());
    assert_eq!(ex::aircraft_speed_cap(&w, id).unwrap(), None);
    w.rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap()
        .aircraft
        .as_mut()
        .unwrap()
        .repaired = false;
    w.rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .config
        .optional
        .aircraft_damage = ex::AircraftMode::Off;
    assert_eq!(ac::target_maneuver(&w, id).unwrap(), 4.5);
    assert!(ac::release_allowed(&w, id).is_ok());
    assert_eq!(ex::aircraft_speed_cap(&w, id).unwrap(), None);
}

#[test]
fn actual_declared_decoy_loss_and_repair_change_aam_countermeasure_consumer() {
    let (mut w, id) = fixture(9, 1000.);
    // Explicitly declared physical component only; removing native sensors
    // leaves one source eligible object and does not fabricate a random hit.
    w.entity_mut(id).unwrap().sensors.clear();
    ex::source_critical(
        &mut w,
        id,
        "declared-decoy-critical",
        "sensor",
        ex::CriticalContext {
            source:
                "scenario_assumption: already adjudicated critical contract input, not natural hit"
                    .into(),
            dp: 0.,
            penetrated: true,
            underwater: false,
            airburst: false,
            weapon: None,
        },
    )
    .unwrap();
    assert!(aam::aircraft_countermeasures(&w, id, 5)
        .unwrap()
        .decoy_generation
        .is_none());
    ex::restore_component(&mut w, id, "decoy").unwrap();
    assert_eq!(
        aam::aircraft_countermeasures(&w, id, 5)
            .unwrap()
            .decoy_generation,
        aam::aircraft_profile("us_f18e").unwrap()["decoy"]
            .as_u64()
            .map(|v| v as usize)
    );
}

#[test]
fn actual_dogfight_gun_hit_uses_optional_aircraft_damage_instead_of_forced_sink() {
    // F/A-18E's 4.2 gun / 29 damage are between the printed table axes.
    // Keep that source boundary; use the existing J-15 -> F-35C exact cell
    // for this actual round/hit consumer, without rounding either profile.
    assert!(ac::gun_probability(4.2, 29.).is_err());
    assert_eq!(ac::gun_probability(2.5, 25.).unwrap(), 0.30);
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let blue_cruise = av::source_speed("cn_j15", 4000., av::Throttle::Cruise).unwrap();
    let red_cruise = av::source_speed("us_f35c", 4000., av::Throttle::Cruise).unwrap();
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"expanded-consumer-gun-resume-lab",
        "name":"scenario_assumption: optional damage with exact sourced gun cell",
        "seed":0,"duration_s":5000,"weather":{"visibility_nm":40,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "aviation_2026":{"source":"scenario_assumption: declared light loads and remaining fuel",
                "parking":{},"loads":{
                    "cn_j15":{"payload_kg":100,"tanks":[],"source":"scenario_assumption: explicit light load"},
                    "us_f35c":{"payload_kg":100,"tanks":[],"source":"scenario_assumption: explicit light load"}},
                "initial_fuel_nm":{"BLUE":700,"RED":700}},
            "expanded_damage_2026":{"source":"scenario_assumption: explicit optional-rule selection",
                "optional":{"aircraft_damage":"immediate_detailed"},
                "units":{
                    "BLUE":{"source":"scenario_assumption: current source altitude cruise row","cruise_speed_kts":blue_cruise},
                    "RED":{"source":"scenario_assumption: current source altitude cruise row","cruise_speed_kts":red_cruise}}}},
        "units":[
            {"side":"Blue","class":"cn_j15","name":"BLUE","pos":[0,0],
                "speed_kts":blue_cruise,"course_deg":0,"altitude_m":4000,"fire_on":false},
            {"side":"Red","class":"us_f35c","name":"RED","pos":[0,0],
                "speed_kts":red_cruise,"course_deg":0,"altitude_m":4000,"fire_on":false}]
    })).unwrap();
    let mut base = harpoon_core::load_scenario(scenario, db).unwrap();
    av::initialize(&mut base).unwrap();
    let blue = base
        .entities
        .values()
        .find(|e| e.name == "BLUE")
        .unwrap()
        .id;
    let red = base.entities.values().find(|e| e.name == "RED").unwrap().id;
    let center = base.entity(blue).unwrap().pos;
    let combat = base.rules.aircombat.as_mut().unwrap();
    combat.orders.insert(red, ac::Order::NonManeuvering);
    combat.fights.insert(
        1,
        ac::Fight {
            center,
            radius_nm: 6.,
            altitude_m: 4000.,
            members: vec![blue, red],
            next_round: 0,
            rounds: 0,
        },
    );
    for seed in 0..2000 {
        let mut w = base.clone();
        w.seed = seed;
        w.rng = harpoon_core::Rng::new(seed);
        let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        ac::step(&mut w);
        let casualty = w
            .rules
            .expanded_damage
            .as_ref()
            .unwrap()
            .units
            .get(&red)
            .and_then(|u| u.aircraft.as_ref());
        if let Some(d) = casualty {
            if d.severity != ex::AirSeverity::Destroyed {
                assert!(w.entity(red).unwrap().alive && w.entity(red).unwrap().hp > 0.);
                assert_eq!(d.active_hits, 1);
                assert!(d.hits.keys().next().unwrap().contains("attacker:"));
                assert!(w.events.iter().any(|e| matches!(
                    &e.kind,
                    harpoon_core::EventKind::RulesEvent { domain, rule, unit:Some(id), detail, .. }
                    if domain == "aircombat" && rule == "9.4.2.gun_table" && *id == blue
                        && serde_json::from_str::<serde_json::Value>(detail).unwrap()["hit"] == true
                )), "survival must follow an actual source gun-table hit");
                ac::step(&mut restored);
                assert_eq!(
                    serde_json::to_value(&w).unwrap(),
                    serde_json::to_value(&restored).unwrap()
                );
                w.tick = 60;
                restored.tick = 60;
                ac::step(&mut w);
                ac::step(&mut restored);
                assert_eq!(
                    serde_json::to_value(&w).unwrap(),
                    serde_json::to_value(&restored).unwrap()
                );
                return;
            }
        }
    }
    panic!("actual source gun rolls never produced a surviving optional casualty");
}

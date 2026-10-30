//! Batch candidates, not an acceptance claim. Laboratory fittings/roles are
//! explicit scenario assumptions. `source_critical` cases supply an already
//! adjudicated critical contract input, NOT a natural hit or a random table
//! success. Native primary and aircraft cases use the actual damage entry.
use harpoon_core::rules::{damage_runtime as dr, expanded_damage2026 as ex, repairs2026};
use harpoon_core::{Database, EntityId, EventKind, Rng, Scenario, World};
use serde_json::{json, Value};

fn fixture(seed: u64, role: &str, director: bool, single_engine: bool) -> (World, EntityId) {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut class = db.platforms["us_ford"].clone();
    class.id = "scenario_expanded_damage_lab".into();
    class.name = "scenario_assumption: physical-system laboratory hull, not Ford fit".into();
    class.rule_data=Some(dr::PlatformDamage{size:"C".into(),role:role.into(),year:2000,single_engine,source:"scenario_assumption: explicit laboratory role/size/single-engine, no historical fit claim".into(),..Default::default()});
    let sensor = class.sensors[0].clone();
    class.sensors = vec![sensor.clone()];
    let mut mount = class.weapons[0].clone();
    mount.weapon = "us_mk48".into();
    mount.mounts = 2;
    mount.ammo = 4;
    class.weapons = vec![mount];
    db.platforms.insert(class.id.clone(), class);
    let mut components = json!({
        "mount-a":{"source":"scenario_assumption: exact laboratory mount A","kind":"weapon_mount","binding":"us_mk48","mount_index":0,"physical":0,"ammunition":4,"location":"topside","local_control":true},
        "mount-b":{"source":"scenario_assumption: exact laboratory mount B","kind":"weapon_mount","binding":"us_mk48","mount_index":0,"physical":1,"ammunition":4,"location":"topside"},
        "sensor-a":{"source":"scenario_assumption: exact existing sensor binding","kind":"sensor","binding":sensor},
        "radio":{"source":"scenario_assumption: explicit CaS long-range radio","kind":"communications","cas_group":"long_range"}
    });
    if director {
        components["director-a"] = json!({"source":"scenario_assumption: director controls mount A only","kind":"director","controls":["mount-a"]});
    }
    let sc:Scenario=serde_json::from_value(json!({"id":"expanded-source-batch-lab","name":"scenario_assumption: explicit physical fittings","seed":seed,"duration_s":500000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","expanded_damage_2026":{"source":"scenario_assumption: E00-E13 laboratory, not a real ship fit","units":{"LAB":{"source":"scenario_assumption: individual physical equipment and cargo","crew":100,"components":components,"holds":{"general":{"source":"scenario_assumption:100tons measured general cargo","kind":"general","amount":100}},"rescue":{"source":"scenario_assumption: ten-metre station radius; not book hardware data","kind":"ship","capacity":100,"station_radius_m":10}}}}},
        "units":[{"side":"Blue","class":"scenario_expanded_damage_lab","name":"LAB","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false}]})).unwrap();
    let w = harpoon_core::load_scenario(sc, db).unwrap();
    let id = w.entities.values().find(|e| e.name == "LAB").unwrap().id;
    (w, id)
}
fn context(dp: f64) -> ex::CriticalContext {
    ex::CriticalContext{source:"scenario_assumption: explicitly adjudicated source critical contract; no natural attack claim".into(),dp,penetrated:true,underwater:false,airburst:false,weapon:None}
}
fn critical(w: &mut World, id: EntityId, identity: &str, kind: &str) {
    ex::source_critical(w, id, identity, kind, context(0.)).unwrap();
}
fn native_primary(w: &mut World, id: EntityId) {
    let mut d = w.db.weapons["us_mk48"].clone();
    d.id = "scenario_expanded_primary".into();
    d.name = "scenario_assumption: real adjudicated laboratory primary".into();
    d.kind = harpoon_core::db::WeaponKind::Rocket;
    d.damage = 20.;
    d.rule_data = Some(dr::WeaponDamage {
        source: "scenario_assumption: exact20DP lab primary, not hardware".into(),
        ship_dp: Some(20.),
        sub_dp: Some(20.),
        penetration_cm: Some(10000.),
        ..Default::default()
    });
    harpoon_core::damage::apply_hit_at(w, id, &d, false, None);
}
fn no_burst_seed(identity: &str) -> u64 {
    (0..10000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(0, 1, identity));
            r.next_f64();
            1 + (r.next_f64() * 10.) as u8 != 10
        })
        .unwrap()
}

#[test]
fn e00_e13_each_d100_cell_has_one_row_and_actual_source_page() {
    for (column, rows) in ex::tables()["critical_columns"].as_object().unwrap() {
        assert!(ex::tables()["column_pages"][column].as_u64().is_some());
        let mut previous = 0;
        for r in rows.as_array().unwrap() {
            let through = r["through"].as_u64().unwrap();
            assert!(through > previous && through <= 100);
            previous = through;
        }
        assert_eq!(previous, 100);
        for d in 1..=100 {
            assert!(ex::row(column, d).is_some());
        }
    }
}
#[test]
fn optional_cas_does_not_change_modern_or_submarine_columns() {
    let (mut w, id) = fixture(1, "surface", false, false);
    w.rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .config
        .optional
        .cas_pre1955 = true;
    let t = w.entity(id).unwrap();
    assert_eq!(ex::column(&w, t, "surface"), "surface");
    assert_eq!(ex::column(&w, t, "sub_major"), "sub_major");
    assert_eq!(ex::column(&w, t, "pre1955"), "cas_minor");
}
#[test]
fn exact_native_mount_loses_its_own_ammo_once_without_average_allocation() {
    let (mut w, id) = fixture(no_burst_seed("exact-physical"), "surface", false, false);
    let hp = w.entity(id).unwrap().hp;
    critical(&mut w, id, "exact-physical", "weapon");
    assert_eq!(w.entity(id).unwrap().weapons[0].mounts, 1);
    assert_eq!(w.entity(id).unwrap().weapons[0].ammo, 4);
    let first = serde_json::to_vec(&w).unwrap();
    critical(&mut w, id, "exact-physical", "weapon");
    assert_eq!(serde_json::to_vec(&w).unwrap(), first);
    assert_eq!(w.entity(id).unwrap().hp, hp);
    assert_eq!(
        w.rules.expanded_damage.as_ref().unwrap().units[&id]
            .disabled
            .len(),
        1
    );
}
#[test]
fn original_director_roll_can_only_disable_the_selected_mounts_director() {
    let mut found = false;
    for seed in 0..256 {
        let (mut w, id) = fixture(seed, "surface", true, false);
        critical(&mut w, id, "director-case", "weapon");
        let u = &w.rules.expanded_damage.as_ref().unwrap().units[&id];
        if u.disabled.contains("director-a") {
            assert!(!u.disabled.contains("mount-a") && !u.disabled.contains("mount-b"));
            assert_eq!(w.entity(id).unwrap().weapons[0].mounts, 2);
            let dice = &w.rules.expanded_dice[&id];
            assert!(dice
                .iter()
                .any(|d| d["purpose"] == "weapon_director_or_mount"
                    && d["roll"].as_u64().unwrap() <= 2));
            found = true;
            break;
        }
    }
    assert!(
        found,
        "source-controlled seed search must find an actual original director roll"
    );
}
#[test]
fn cic_keeps_explicit_local_control_and_disables_remote_fire_and_sensor() {
    let (mut w, id) = fixture(4, "surface", false, false);
    critical(&mut w, id, "cic-case", "cic");
    assert!(ex::weapon_operational(&w, id, "us_mk48").unwrap());
    let sensor = w.entity(id).unwrap().sensors[0].sensor.clone();
    assert!(!ex::sensor_operational(&w, id, &sensor).unwrap());
    let f = w
        .rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .config
        .units
        .get_mut("LAB")
        .unwrap();
    f.components.get_mut("mount-a").unwrap().local_control = false;
    assert!(!ex::weapon_operational(&w, id, "us_mk48").unwrap());
}
#[test]
fn cargo_consumes_actual_source_dp_and_flooding_an_exact_hold_loses_its_stock() {
    let (mut w, id) = fixture(2, "merchant", false, false);
    ex::source_critical(&mut w, id, "cargo-case", "cargo", context(20.)).unwrap();
    assert_eq!(
        w.rules.expanded_damage.as_ref().unwrap().units[&id].holds["general"].amount,
        90.
    );
    ex::control(&mut w, id, "flood_hold:general")
        .unwrap()
        .unwrap();
    let h = &w.rules.expanded_damage.as_ref().unwrap().units[&id].holds["general"];
    assert_eq!(h.amount, 0.);
    assert!(h.flooded);
}
#[test]
fn source_pending_is_not_reported_as_success_or_rerolled_on_retry() {
    let (mut w, id) = fixture(17, "surface", false, false);
    let input = context(0.);
    assert!(ex::source_critical(&mut w, id, "battery-no-fit", "battery", input.clone()).is_err());
    let saved = serde_json::to_vec(&w).unwrap();
    assert!(ex::source_critical(&mut w, id, "battery-no-fit", "battery", input).is_err());
    assert_eq!(serde_json::to_vec(&w).unwrap(), saved);
    assert!(w
        .entity(id)
        .unwrap()
        .ext
        .get("damage.battery_capacity")
        .is_none());
}
#[test]
fn changed_source_context_under_same_identity_is_read_only_rejected() {
    let (mut w, id) = fixture(4, "surface", false, false);
    critical(&mut w, id, "same-source", "sensor");
    let saved = serde_json::to_vec(&w).unwrap();
    assert!(ex::source_critical(&mut w, id, "same-source", "sensor", context(100.)).is_err());
    assert_eq!(serde_json::to_vec(&w).unwrap(), saved);
}
#[test]
fn opaque_owner_critical_and_repair_keys_keep_original_referee_source() {
    let (mut w, id) = fixture(3, "surface", false, false);
    let hidden = "nuclear:private-enemy-attack:source-2884";
    critical(&mut w, id, hidden, "rudder");
    let owner = ex::own_view(&w, id).unwrap().to_string()
        + &repairs2026::own_view(&w, id).unwrap().to_string();
    assert!(
        !owner.contains(hidden)
            && !owner.contains("private-enemy-attack")
            && !owner.contains("2884")
    );
    let referee = serde_json::to_string(&w.rules).unwrap();
    assert!(referee.contains(hidden));
}
#[test]
fn two_level_engineering_can_remove_one_level_at_each_successful_checkpoint() {
    let seed = (0..100000)
        .find(|s| {
            [6, 12].into_iter().all(|hour| {
                let mut r = Rng::new(*s).derive(Rng::hash_tag(
                    0,
                    1,
                    &format!("14.6.repair:two-level:{hour}"),
                ));
                1 + (r.next_f64() * 100.) as u8 <= 50
            })
        })
        .unwrap();
    let (mut w, id) = fixture(seed, "surface", false, true);
    critical(&mut w, id, "two-level", "engineering_no_fire");
    assert_eq!(w.entity(id).unwrap().ext["damage.engine_levels"], 2.);
    repairs2026::control(&mut w, id, "begin_repairs")
        .unwrap()
        .unwrap();
    w.tick = 6 * 3600;
    repairs2026::control(&mut w, id, "repair_due")
        .unwrap()
        .unwrap();
    assert_eq!(w.entity(id).unwrap().ext["damage.engine_levels"], 1.);
    w.tick = 12 * 3600;
    repairs2026::control(&mut w, id, "repair_due")
        .unwrap()
        .unwrap();
    assert_eq!(w.entity(id).unwrap().ext["damage.engine_levels"], 0.);
}
#[test]
fn physical_component_repair_does_not_regenerate_ammunition_or_hp() {
    let (mut w, id) = fixture(no_burst_seed("mount-repair"), "surface", false, false);
    critical(&mut w, id, "mount-repair", "weapon");
    let component = w.rules.expanded_damage.as_ref().unwrap().units[&id]
        .disabled
        .iter()
        .next()
        .unwrap()
        .clone();
    let hp = w.entity(id).unwrap().hp;
    let ammo = w.entity(id).unwrap().weapons[0].ammo;
    ex::restore_component(&mut w, id, &component).unwrap();
    assert_eq!(w.entity(id).unwrap().weapons[0].mounts, 2);
    assert_eq!(w.entity(id).unwrap().weapons[0].ammo, ammo);
    assert_eq!(w.entity(id).unwrap().hp, hp);
}
#[test]
fn true_primary_and_full_world_restore_keep_private_equipment_state_and_original_dice() {
    let (mut w, id) = fixture(11, "surface", true, false);
    native_primary(&mut w, id);
    critical(&mut w, id, "restore-source", "sensor");
    let saved = serde_json::to_vec(&w).unwrap();
    let recovered: World = serde_json::from_slice(&saved).unwrap();
    assert_eq!(serde_json::to_vec(&recovered).unwrap(), saved);
    assert!(w
        .events
        .iter()
        .any(|e| matches!(&e.kind,EventKind::Impact{damage,..}if *damage>0.)));
    assert!(w.rules.expanded_dice.contains_key(&id));
    assert_eq!(ex::own_view(&recovered, id), ex::own_view(&w, id));
}
#[test]
fn optional_serviceability_is_opt_in_and_daily_checks_are_idempotent() {
    let (mut w, id) = fixture(55, "surface", false, false);
    assert!(ex::serviceability_check(&mut w, id).is_err());
    {
        let s = w.rules.expanded_damage.as_mut().unwrap();
        s.config.optional.serviceability = true;
        s.config.units.get_mut("LAB").unwrap().rank = Some(ex::Rank::First);
    }
    ex::serviceability_check(&mut w, id).unwrap();
    let saved = serde_json::to_vec(&w).unwrap();
    ex::serviceability_check(&mut w, id).unwrap();
    assert_eq!(serde_json::to_vec(&w).unwrap(), saved);
    assert!(
        w.rules.expanded_damage.as_ref().unwrap().units[&id]
            .service_checks
            .len()
            >= 3
    );
}
#[test]
fn survivor_loss_is_private_until_an_explicit_successful_sighting_snapshot() {
    let (mut w, id) = fixture(1, "surface", false, false);
    w.rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .config
        .optional
        .search_and_rescue = true;
    ex::record_loss(&mut w, id, false);
    assert!(ex::survivor_view(&w, id).is_empty());
    let key = w
        .rules
        .expanded_damage
        .as_ref()
        .unwrap()
        .survivors
        .keys()
        .next()
        .unwrap()
        .clone();
    ex::record_survivor_sighting(&mut w, id, &key).unwrap();
    let own = ex::survivor_view(&w, id);
    assert_eq!(own.len(), 1);
    assert_eq!(own[0]["report_is_live_track"], false);
    {
        let s = w.rules.expanded_damage.as_mut().unwrap();
        s.survivors.get_mut(&key).unwrap().position.x += 20.;
    }
    assert_eq!(
        ex::survivor_view(&w, id),
        own,
        "referee noninterference mutation: old source report is a fixed snapshot"
    );
}
#[test]
fn survivor_hidden_group_insert_does_not_renumber_own_labels() {
    let (mut w, id) = fixture(1, "surface", false, false);
    w.rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .config
        .optional
        .search_and_rescue = true;
    ex::record_loss(&mut w, id, false);
    let key = w
        .rules
        .expanded_damage
        .as_ref()
        .unwrap()
        .survivors
        .keys()
        .next()
        .unwrap()
        .clone();
    ex::record_survivor_sighting(&mut w, id, &key).unwrap();
    let before = ex::survivor_view(&w, id);
    let mut hidden = w.rules.expanded_damage.as_ref().unwrap().survivors[&key].clone();
    hidden.observed_by.clear();
    hidden.sightings.clear();
    w.rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .survivors
        .insert("aaa-referee-hidden-contract-group".into(), hidden);
    assert_eq!(ex::survivor_view(&w, id), before);
}
#[test]
fn optional_aircraft_profile_does_not_change_an_undamaged_platform() {
    let (w, id) = fixture(44, "surface", false, false);
    let p = ex::aircraft_profile(&w, id, 3., false).unwrap();
    assert_eq!(p.maneuver_rating, 3.);
    assert!(!p.cruise_only);
    assert!(p.offensive && p.sensors && p.countermeasures && p.jettison);
}

fn aircraft_fixture(seed: u64, mode: &str) -> (World, EntityId) {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut class = db
        .platforms
        .values()
        .find(|c| c.kind == harpoon_core::types::PlatformKind::Aircraft)
        .unwrap()
        .clone();
    class.id = "scenario_expanded_aircraft".into();
    class.name = "scenario_assumption: abstract aircraft damage laboratory".into();
    class.weapons.clear();
    class.sensors.clear();
    db.platforms.insert(class.id.clone(), class);
    let sc:Scenario=serde_json::from_value(json!({"id":"expanded-aircraft-native-primary","name":"scenario_assumption: exact optional-mode and600kt cruise, no real fit claim","seed":seed,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","expanded_damage_2026":{"source":"scenario_assumption: optional aircraft damage laboratory","optional":{"aircraft_damage":mode},"units":{"AIR":{"source":"scenario_assumption: explicit cruise fit","cruise_speed_kts":600,"crew":2}}}},"units":[{"side":"Blue","class":"scenario_expanded_aircraft","name":"AIR","pos":[0,0],"course_deg":0,"speed_kts":600,"altitude_m":1000,"fire_on":false}]})).unwrap();
    let w = harpoon_core::load_scenario(sc, db).unwrap();
    let id = w.entities.values().find(|e| e.name == "AIR").unwrap().id;
    (w, id)
}
#[test]
fn native_primary_immediate_aircraft_critical_consumes_original_d10_and_real_profile() {
    let mut found = false;
    for seed in 0..100 {
        let (mut w, id) = aircraft_fixture(seed, "immediate_detailed");
        native_primary(&mut w, id);
        if w.rules.expanded_damage.as_ref().unwrap().units[&id]
            .aircraft
            .as_ref()
            .unwrap()
            .severity
            == ex::AirSeverity::Critical
        {
            let a = &w.rules.expanded_damage.as_ref().unwrap().units[&id]
                .aircraft
                .as_ref()
                .unwrap();
            assert_eq!(a.hits.values().next().unwrap().d10, 6);
            let p = ex::aircraft_profile(&w, id, 3., false).unwrap();
            assert_eq!(p.maneuver_rating, 0.5);
            assert_eq!(
                ex::aircraft_profile(&w, id, 3., true)
                    .unwrap()
                    .maneuver_rating,
                0.
            );
            assert!(p.cruise_only && !p.offensive && !p.sensors && !p.countermeasures);
            assert_eq!(ex::aircraft_speed_cap(&w, id).unwrap(), Some(600.));
            assert!(w.entity(id).unwrap().hp > 0.);
            found = true;
            break;
        }
    }
    assert!(found);
}
#[test]
fn native_second_hit_on_a_critical_aircraft_is_actual_destroyed_state() {
    for seed in 0..100 {
        let (mut w, id) = aircraft_fixture(seed, "immediate_detailed");
        native_primary(&mut w, id);
        if w.rules.expanded_damage.as_ref().unwrap().units[&id]
            .aircraft
            .as_ref()
            .unwrap()
            .severity
            == ex::AirSeverity::Critical
        {
            native_primary(&mut w, id);
            assert_eq!(
                w.rules.expanded_damage.as_ref().unwrap().units[&id]
                    .aircraft
                    .as_ref()
                    .unwrap()
                    .severity,
                ex::AirSeverity::Destroyed
            );
            assert!(w.entity(id).is_none_or(|e| !e.alive || e.hp <= 0.));
            return;
        }
    }
    panic!("no original critical seed found");
}
#[test]
fn native_two_different_light_aircraft_results_compose_heavy_and_survive_save() {
    for seed in 0..500 {
        let (mut w, id) = aircraft_fixture(seed, "immediate_detailed");
        native_primary(&mut w, id);
        if w.rules.expanded_damage.as_ref().unwrap().units[&id]
            .aircraft
            .as_ref()
            .unwrap()
            .severity
            != ex::AirSeverity::Light
        {
            continue;
        }
        native_primary(&mut w, id);
        let a = w.rules.expanded_damage.as_ref().unwrap().units[&id]
            .aircraft
            .as_ref()
            .unwrap();
        if a.severity == ex::AirSeverity::Heavy && a.hits.values().all(|h| h.d10 >= 9) {
            assert_eq!(a.light.len(), 2);
            let saved = serde_json::to_vec(&w).unwrap();
            let recovered: World = serde_json::from_slice(&saved).unwrap();
            assert_eq!(serde_json::to_vec(&recovered).unwrap(), saved);
            return;
        }
    }
    panic!("no original two-light-different seed found");
}
#[test]
fn native_immediate_fifty_fifty_damage_is_heavy_and_second_hit_kills() {
    for seed in 0..100 {
        let (mut w, id) = aircraft_fixture(seed, "immediate_fifty_fifty");
        native_primary(&mut w, id);
        if w.rules.expanded_damage.as_ref().unwrap().units[&id]
            .aircraft
            .as_ref()
            .unwrap()
            .severity
            == ex::AirSeverity::Heavy
        {
            native_primary(&mut w, id);
            assert_eq!(
                w.rules.expanded_damage.as_ref().unwrap().units[&id]
                    .aircraft
                    .as_ref()
                    .unwrap()
                    .severity,
                ex::AirSeverity::Destroyed
            );
            return;
        }
    }
    panic!("no original50/50 damaged seed found");
}

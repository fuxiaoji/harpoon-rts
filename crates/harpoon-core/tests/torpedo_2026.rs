use harpoon_core::{
    rules::torpedo2026::*, Command, ContactId, Database, EntityId, Scenario, Side, Vec2, World,
};
use serde_json::json;
#[test]
fn official_homing_example_and_every_countermeasure_generation() {
    assert_eq!(homing_probability(4, Some(3), true).unwrap(), 0.51);
    assert_eq!(homing_probability(3, Some(3), true).unwrap(), 0.31);
    for acm in 1..=4 {
        for gen in 1..=4 {
            for evasive in [false, true] {
                let p = homing_probability(gen, Some(acm), evasive).unwrap();
                assert_eq!(
                    p,
                    tables()["homing"][acm - 1][gen - 1][if evasive { 3 } else { 1 }]
                        .as_f64()
                        .unwrap()
                );
            }
        }
    }
    assert!(homing_probability(5, None, false).is_err());
    assert!(homing_probability(4, Some(0), true).is_err());
}
#[test]
fn official_straight_runner_example_uses_one_salvo_die_and_closest_run_row() {
    assert_eq!(straight_hits("Medium", 4, 1.25, false, 22).unwrap(), 1);
    assert_eq!(straight_hits("Medium", 4, 1.25, false, 3).unwrap(), 3);
    assert_eq!(straight_hits("Medium", 4, 1.25, false, 60).unwrap(), 0);
    // Tie at1.5 selects2.0; stationary target shifts back up to1.0.
    assert_eq!(straight_hits("Medium", 4, 1.5, false, 40).unwrap(), 0);
    assert_eq!(straight_hits("Medium", 4, 1.5, true, 40).unwrap(), 1);
    assert_eq!(effective_size(1, 90.).unwrap(), "Medium");
    assert_eq!(effective_size(0, 0.).unwrap(), "Small");
    assert!(effective_size(5, 90.).is_err());
    let cells: usize = tables()["straight"]
        .as_object()
        .unwrap()
        .values()
        .map(|s| {
            s.as_object()
                .unwrap()
                .values()
                .map(|v| {
                    v.as_array()
                        .unwrap()
                        .iter()
                        .map(|r| r.as_array().unwrap().len() - 1)
                        .sum::<usize>()
                })
                .sum::<usize>()
        })
        .sum();
    assert_eq!(cells, 672);
}
#[test]
fn source_twenty_nine_percent_includes_die29_and_excludes_die30() {
    // PDF111/10-2: Large,3 weapons,4nm has29% for one hit.
    assert_eq!(straight_hits("Large", 3, 4., false, 29).unwrap(), 1);
    assert_eq!(straight_hits("Large", 3, 4., false, 30).unwrap(), 0);
}
#[test]
fn source_acquisition_and_wire_boundaries() {
    let a = acquisition_nm(4, true, 2, false, false, false).unwrap();
    assert!((a - 1.9749460043196543).abs() < 1e-12);
    assert_eq!(
        acquisition_nm(4, true, 2, true, true, false).unwrap(),
        a * 0.5 * 0.75
    );
    assert!(
        (acquisition_nm(1, false, 4, false, true, false).unwrap() - 0.03703023758099352).abs()
            < 1e-12
    );
    assert_eq!(wire_probability(10., false).unwrap(), 0.);
    assert_eq!(wire_probability(10., true).unwrap(), 0.15);
    assert_eq!(wire_probability(11., false).unwrap(), 0.25);
    assert_eq!(wire_probability(20., true).unwrap(), 0.80);
    assert_eq!(wire_probability(21., false).unwrap(), 1.);
    assert_eq!(hardkill_probability("RBU", false).unwrap(), 0.05);
    assert_eq!(hardkill_probability("RBU", true).unwrap(), 0.03);
    assert_eq!(hardkill_probability("ATT2", false).unwrap(), 0.35);
    assert_eq!(hardkill_probability("ATT2", true).unwrap(), 0.15);
    assert!(hardkill_probability("unverified-system", true).is_err());
    assert_eq!(depth_band(1200.).unwrap(), 11);
    assert_eq!(depth_band(1201.).unwrap(), 12);
}
#[test]
fn source_four_thousand_yards_is_under_two_nautical_miles() {
    // Independent physical anchor: 4000 yards = 3657.6 metres.
    // Do not repeat the production conversion expression or use its data.
    let actual = acquisition_nm(4, true, 2, false, false, false).unwrap();
    assert!(
        (actual - 1.9749460043196543).abs() < 1e-12,
        "4000 yd must be about 1.975 nm, got {actual}"
    );
}
#[test]
fn source_one_thousand_yards_is_under_half_a_nautical_mile() {
    // Gen2 active source range is1000 yd = 914.4 metres.
    let actual = acquisition_nm(2, true, 2, false, false, false).unwrap();
    assert!(
        (actual - 0.49373650107991357).abs() < 1e-12,
        "1000 yd must be about 0.494 nm, got {actual}"
    );
}
#[test]
fn source_class_d_is_small_for_straight_runner_broad_aspect() {
    // §2.1 PDF17/2-1: both physical C and D are Small. §10.2.3
    // Target Aspects keeps Small on a broad shot, then VSmall on a quarter.
    assert_eq!(effective_size(3, 90.).unwrap(), "Small");
    assert_eq!(effective_size(3, 45.).unwrap(), "VSmall");
    assert_eq!(effective_size(3, 0.).unwrap(), "VSmall");
}
fn fixture() -> (World, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc:Scenario=serde_json::from_value(json!({"id":"source-torpedo","name":"Source torpedo","duration_s":1000,"seed":20261023,
        "context":{"rules_profile":"hv2026-rts-v4","torpedo_2026":{"source":"scenario_assumption: test source mechanisms",
        "environment":{"source":"scenario_assumption: open water no layer","layer_m":null,"reverberant":false},
        "targets":{"cn_093":{"source":"scenario_assumption: controlled noise/coating/size","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
        "platforms":{"us_virginia":{"source":"scenario_assumption: one loaded test tube; US non-Seawolf source wire cap2","wire_capacity":2,"reload_parallel":2,"tubes":[{"bearing_deg":0,"loaded":"us_mk48"},{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}},
        "orders":{"SHOOTER:us_mk48":{"source":"scenario_assumption: declared test order","speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":null,"cut_wire":true,"spread_deg":[],"solution":{"quality":"Good","source":"scenario_assumption: fixture known solution"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,0.5],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let s = w
        .entities
        .values()
        .find(|e| e.name == "SHOOTER")
        .unwrap()
        .id;
    let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    w.entity_mut(t).unwrap().weapons.clear();
    w.db.weapons.get_mut("us_mk48").unwrap().pk = 0.;
    w.sides[0].contacts.insert(
        ContactId(99),
        harpoon_core::world::Contact {
            id: ContactId(99),
            about: t,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 0.5),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: 0,
            stale: false,
            sensors: vec![],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert(format!("0:{}", t.0), harpoon_core::types::PlatformKind::Sub);
    (w, s, t)
}
fn ammo(w: &World, s: EntityId) -> u16 {
    w.entity(s)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_mk48")
        .unwrap()
        .ammo
}
fn fire(s: EntityId) -> (Side, Command) {
    (
        Side::Blue,
        Command::Fire {
            unit: s,
            contact: ContactId(99),
            weapon: "us_mk48".into(),
            rounds: 1,
        },
    )
}

fn own_observation(w: &World, side: Side) -> serde_json::Value {
    serde_json::to_value(harpoon_core::observe::observe(w, side, 0, 10_000)).unwrap()
}
fn observed_tubes(w: &World, s: EntityId) -> serde_json::Value {
    own_observation(w, Side::Blue)["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == json!(s))
        .unwrap()["torpedoes"]
        .clone()
}
#[test]
fn cut_wire_real_release_does_not_project_true_underwater_body_or_homing_turn() {
    // §7.5.4: after cutting the wire the weapon reverts to autonomous guidance;
    // a launch declaration is not a report of its true underwater position.
    let (mut w, s, _) = fixture();
    harpoon_core::sim::tick(&mut w, &[fire(s)]);
    let id = *w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    assert!(!w.rules.torpedoes.as_ref().unwrap().flights[&id].wire);
    let baseline = own_observation(&w, Side::Blue);
    assert!(
        baseline["units"]
            .as_array()
            .unwrap()
            .iter()
            .all(|u| u["id"] != json!(id)),
        "an unreported underwater body must not be an own true-state UnitObs"
    );
    let mut hidden: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    let p = hidden.entity_mut(id).unwrap();
    p.pos = Vec2::new(13., -7.);
    p.course_deg = 287.;
    p.speed_kts = 19.;
    p.depth_m = 400.;
    hidden
        .rules
        .torpedoes
        .as_mut()
        .unwrap()
        .flights
        .get_mut(&id)
        .unwrap()
        .phase = Phase::Locked;
    assert_eq!(
        baseline,
        own_observation(&hidden, Side::Blue),
        "hidden homing changes cannot become shooter telemetry"
    );
}
#[test]
fn single_wire_real_release_has_control_without_unlicensed_status_telemetry() {
    let (mut w, s, _) = fixture();
    let state = w.rules.torpedoes.as_mut().unwrap();
    let weapon = state.config.weapons.get_mut("us_mk48").unwrap();
    weapon.guidance = Guidance::Wire;
    weapon.source = "scenario_assumption: test single-wire fit, no status channel".into();
    state
        .config
        .orders
        .get_mut("SHOOTER:us_mk48")
        .unwrap()
        .cut_wire = false;
    harpoon_core::sim::tick(&mut w, &[fire(s)]);
    let id = *w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    let baseline = own_observation(&w, Side::Blue);
    assert!(
        baseline["units"]
            .as_array()
            .unwrap()
            .iter()
            .all(|u| u["id"] != json!(id)),
        "§7.5.4 grants single-wire commands, but grants receive-status to dual-wire only"
    );
    w.entity_mut(id).unwrap().pos = Vec2::new(17., -8.);
    w.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .flights
        .get_mut(&id)
        .unwrap()
        .phase = Phase::Past { remaining_nm: 0.2 };
    assert_eq!(baseline, own_observation(&w, Side::Blue));
}
#[test]
fn secret_torpedo_retirement_keeps_contact_count_explicitly_unknown() {
    let (mut w, s, _) = fixture();
    harpoon_core::sim::tick(&mut w, &[fire(s)]);
    let id = *w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    let before = own_observation(&w, Side::Blue);
    let contact = before["contacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == 99)
        .unwrap();
    assert_eq!(
        contact["own_in_flight"],
        serde_json::Value::Null,
        "unknown physical status must be null, not a true-alive count or a fake active count"
    );
    w.entities.remove(&id);
    assert_eq!(
        before,
        own_observation(&w, Side::Blue),
        "unreported retirement cannot change shooter observation"
    );
}
#[test]
fn source_own_tubes_reserves_reload_clock_survive_real_command_and_save() {
    let (mut w, s, _) = fixture();
    harpoon_core::sim::tick(&mut w, &[fire(s)]);
    let v = observed_tubes(&w, s);
    assert_eq!(
        v["verified"], true,
        "§10.1 requires the player's actual loaded-tube log"
    );
    assert!(v["tubes"][0]["loaded"].is_null());
    assert_eq!(v["tubes"][1]["loaded"], "us_mk48");
    assert_eq!(v["reserves"]["us_mk48"], 2);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Torpedo {
                unit: s,
                action: Action::Reload {
                    tube: 0,
                    weapon: "us_mk48".into(),
                },
            },
        )],
    );
    let v = observed_tubes(&w, s);
    assert_eq!(v["reserves"]["us_mk48"], 1);
    assert_eq!(v["reloads"][0]["tube"], 0);
    assert_eq!(v["reloads"][0]["remaining_s"], 180);
    let due = v["reloads"][0]["due"].as_i64().unwrap();
    let restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    assert_eq!(v, observed_tubes(&restored, s));
    // Isolate the known servicing clock from combat ending the scenario.
    w.tick = due;
    step(&mut w);
    let done = observed_tubes(&w, s);
    assert_eq!(done["tubes"][0]["loaded"], "us_mk48");
    assert_eq!(done["reloads"], json!([]));
    assert!(own_observation(&w, Side::Red)["units"]
        .as_array()
        .unwrap()
        .iter()
        .all(|u| u["id"] != json!(s)));
}
#[test]
fn missing_source_tube_fit_is_unknown_instead_of_an_empty_inventory() {
    let (w, _, _) = fixture();
    let red = own_observation(&w, Side::Red);
    let target = red["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["name"] == "TARGET")
        .unwrap();
    assert_eq!(target["torpedoes"]["verified"], false);
    assert_eq!(
        target["torpedoes"]["tubes"],
        serde_json::Value::Null,
        "an unbound platform is not a verified zero-tube submarine"
    );
}
#[test]
fn generic_real_torpedo_launch_keeps_an_unknown_declaration_without_a_source_flight() {
    let (mut w, s, t) = fixture();
    let def = w.db.weapons["us_mk48"].clone();
    let id = harpoon_core::weapons::launch(&mut w, s, &def, Some(t), None).unwrap();
    assert!(!w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .contains_key(&id));
    let before = own_observation(&w, Side::Blue);
    let c = before["contacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == 99)
        .unwrap();
    assert_eq!(
        c["own_in_flight"],
        serde_json::Value::Null,
        "the common real launch entry must not report a false known zero"
    );
    assert_eq!(
        c["own_torpedo_releases"]["released"][0]["weapon"],
        "us_mk48"
    );
    w.entities.remove(&id);
    assert_eq!(before, own_observation(&w, Side::Blue));
}
#[test]
fn unreported_retirement_does_not_secretly_free_a_guidance_channel_for_a_second_fire() {
    let (mut w, s, t) = fixture();
    // Fixed seed20261023 gives launch-wire D10021 for projectile3: intact.
    let state = w.rules.torpedoes.as_mut().unwrap();
    state.tubes.get_mut(&s).unwrap().wire_capacity = 1;
    state
        .config
        .orders
        .get_mut("SHOOTER:us_mk48")
        .unwrap()
        .cut_wire = false;
    harpoon_core::sim::tick(&mut w, &[fire(s)]);
    let id = *w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    assert!(w.rules.torpedoes.as_ref().unwrap().flights[&id].wire);
    w.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .config
        .orders
        .get_mut("SHOOTER:us_mk48")
        .unwrap()
        .bearing_deg = Some(20.);
    let mut hidden: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    let mount = w
        .entity(s)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_mk48")
        .unwrap();
    let before = ammo(&w, s);
    assert!(fire_at(&mut w, s, t, mount, 1).is_err());
    hidden.entity_mut(id).unwrap().alive = false;
    assert!(
        fire_at(&mut hidden, s, t, mount, 1).is_err(),
        "an unreported death must not release a channel that still has an uncut wire"
    );
    assert_eq!(ammo(&hidden, s), before);
}
#[test]
fn wire_order_receipt_and_known_order_do_not_probe_secret_projectile_cleanup() {
    let (mut live, s, _) = fixture();
    live.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .config
        .orders
        .get_mut("SHOOTER:us_mk48")
        .unwrap()
        .cut_wire = false;
    harpoon_core::sim::tick(&mut live, &[fire(s)]);
    let id = *live
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    assert!(
        live.rules.torpedoes.as_ref().unwrap().flights[&id].wire,
        "fixed seed20261023 gives intact launch wire D10021"
    );
    let mut hidden: World = serde_json::from_value(serde_json::to_value(&live).unwrap()).unwrap();
    hidden.entities.remove(&id);
    let command = (
        Side::Blue,
        Command::Torpedo {
            unit: s,
            action: Action::WireOrder {
                torpedo: id,
                speed_index: 1,
                depth_m: 70.,
                bearing_deg: 30.,
                enable_run_nm: 0.2,
            },
        },
    );
    harpoon_core::sim::tick(&mut live, &[command.clone()]);
    harpoon_core::sim::tick(&mut hidden, &[command]);
    assert_eq!(own_observation(&live,Side::Blue),own_observation(&hidden,Side::Blue),
        "acceptance and the saved own declaration must depend on the known wire/order, not an unreported missing body");
    let declared = observed_tubes(&hidden, s)["releases"][0]["declared_order"].clone();
    assert_eq!(declared["speed_index"], 1);
    assert_eq!(declared["depth_m"], 70.);
    assert_eq!(declared["bearing_deg"], 30.);
    assert_eq!(declared["enable_run_nm"], 0.2);
    let restored: World = serde_json::from_value(serde_json::to_value(&hidden).unwrap()).unwrap();
    assert_eq!(observed_tubes(&hidden, s), observed_tubes(&restored, s));
}
#[test]
fn a_new_contact_does_not_inherit_an_old_release_through_referee_target_identity() {
    let (mut w, s, _) = fixture();
    harpoon_core::sim::tick(&mut w, &[fire(s)]);
    let mut replacement = w.sides[0].contacts.remove(&ContactId(99)).unwrap();
    replacement.id = ContactId(109);
    // The referee knows it is the same body. The player has a new contact ID
    // and no source-backed old/new identity association in its observations.
    w.sides[0].contacts.insert(replacement.id, replacement);
    let obs = own_observation(&w, Side::Blue);
    let c = obs["contacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == 109)
        .unwrap();
    assert_eq!(
        c["own_in_flight"], 0,
        "a new ID must not silently acquire C99's launch commitment"
    );
    assert!(
        c["own_torpedo_releases"].is_null(),
        "the new contact cannot borrow the referee's true EntityId link"
    );
    let old = observed_tubes(&w, s)["releases"][0].clone();
    assert_eq!(
        old["contact"], 99,
        "the shooter still knows its original release declaration"
    );
    assert_eq!(old["status"], "unknown");
    // Historical v4 saves may lack the original declaration. The old Flight
    // is evidence of release only, not a license to guess a current track ID.
    w.rules.torpedoes.as_mut().unwrap().launches.clear();
    let historical = own_observation(&w, Side::Blue);
    let c = historical["contacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == 109)
        .unwrap();
    assert!(c["own_torpedo_releases"].is_null());
    assert!(observed_tubes(&w, s)["releases"][0]["contact"].is_null());
}
fn source_mine_tubes(w: &mut World, s: EntityId, reserve: u16) {
    let fit = w
        .rules
        .torpedoes
        .as_mut()
        .unwrap()
        .tubes
        .get_mut(&s)
        .unwrap();
    fit.source = "scenario_assumption: exact shared PORT/STARBOARD physical tube mapping".into();
    fit.tube_ids.insert("PORT".into(), 0);
    fit.tube_ids.insert("STARBOARD".into(), 1);
    for tube in &mut fit.tubes {
        tube.loaded = Some("scenario_mine".into());
    }
    fit.reserves.insert("scenario_mine".into(), reserve);
}
#[test]
fn shared_physical_mine_tube_transaction_is_atomic_and_never_guesses_an_id() {
    let (mut w, s, _) = fixture();
    source_mine_tubes(&mut w, s, 1);
    let before = serde_json::to_value(&w.rules.torpedoes).unwrap();
    assert!(occupy_mine_tubes(
        &mut w,
        s,
        &["PORT".into(), "UNKNOWN".into()],
        "scenario_mine",
        360
    )
    .is_err());
    assert_eq!(before, serde_json::to_value(&w.rules.torpedoes).unwrap());
    assert!(occupy_mine_tubes(
        &mut w,
        s,
        &["PORT".into(), "PORT".into()],
        "scenario_mine",
        360
    )
    .is_err());
    assert_eq!(before, serde_json::to_value(&w.rules.torpedoes).unwrap());
    assert!(occupy_mine_tubes(&mut w, s, &["PORT".into()], "wrong_load", 360).is_err());
    assert_eq!(before, serde_json::to_value(&w.rules.torpedoes).unwrap());
}
#[test]
fn shared_mine_clock_blocks_torpedo_reload_and_reserves_only_a_real_existing_item() {
    let (mut w, s, _) = fixture();
    source_mine_tubes(&mut w, s, 1);
    assert_eq!(mine_inventory(&w, s, "scenario_mine").unwrap(), 3);
    occupy_mine_tubes(&mut w, s, &["PORT".into()], "scenario_mine", 360).unwrap();
    assert_eq!(mine_inventory(&w, s, "scenario_mine").unwrap(), 2);
    assert!(tube_ready(&w, s, "PORT").is_err());
    assert!(apply(
        &mut w,
        s,
        &Action::Reload {
            tube: 0,
            weapon: "us_mk48".into()
        }
    )
    .is_err());
    let v = observed_tubes(&w, s);
    assert_eq!(v["tubes"][0]["physical_id"], "PORT");
    assert!(v["tubes"][0]["loaded"].is_null());
    assert_eq!(v["reloads"][0]["remaining_s"], 360);
    assert_eq!(v["reserves"]["scenario_mine"], 0);
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    restored.tick = 359;
    step(&mut restored);
    assert!(tube_ready(&restored, s, "PORT").is_err());
    restored.tick = 360;
    step(&mut restored);
    assert_eq!(tube_ready(&restored, s, "PORT").unwrap(), 0);
    assert_eq!(
        observed_tubes(&restored, s)["tubes"][0]["loaded"],
        "scenario_mine"
    );
    assert_eq!(mine_inventory(&restored, s, "scenario_mine").unwrap(), 2);
}
#[test]
fn exhausted_physical_mine_reserve_leaves_an_empty_tube_without_a_synthetic_reload() {
    let (mut w, s, _) = fixture();
    source_mine_tubes(&mut w, s, 0);
    occupy_mine_tubes(
        &mut w,
        s,
        &["PORT".into(), "STARBOARD".into()],
        "scenario_mine",
        360,
    )
    .unwrap();
    assert_eq!(mine_inventory(&w, s, "scenario_mine").unwrap(), 0);
    assert!(w.rules.torpedoes.as_ref().unwrap().reloads.is_empty());
    w.tick = 360;
    step(&mut w);
    assert!(tube_ready(&w, s, "PORT").is_err());
    assert!(observed_tubes(&w, s)["tubes"][0]["loaded"].is_null());
}
#[test]
fn real_fire_consumes_loaded_tube_and_source_placement_then_source_homing_pk() {
    let (mut w, s, t) = fixture();
    let before = ammo(&w, s);
    harpoon_core::sim::tick(&mut w, &[fire(s)]);
    assert_eq!(ammo(&w, s), before - 1);
    assert_eq!(
        w.rules.torpedoes.as_ref().unwrap().tubes[&s]
            .tubes
            .iter()
            .filter(|t| t.loaded.is_none())
            .count(),
        1
    );
    let id = *w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    for _ in 0..250 {
        if w.rules.torpedoes.as_ref().unwrap().flights[&id]
            .last_d100
            .is_some()
            || w.rules.pending_adjudications.contains_key(&id)
        {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let f = &w.rules.torpedoes.as_ref().unwrap().flights[&id];
    assert!(
        f.placement.as_ref().unwrap().placed,
        "the fixed source fixture must actually acquire and attack"
    );
    assert_eq!(
        f.last_probability,
        Some(0.80),
        "def.pk0 must not override the source seeker generation"
    );
    assert!(f.last_d100.is_some());
    assert!(
        !w.rules.pending_adjudications.contains_key(&id),
        "a fully bound acoustic example must reach the source executor"
    );
    assert!(w.entity(t).unwrap().hp <= w.entity(t).unwrap().hp_max);
    let restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
#[test]
fn source_reload_requires_real_180_seconds_and_preserves_inventory() {
    let (mut w, s, _) = fixture();
    harpoon_core::sim::tick(&mut w, &[fire(s)]);
    // Isolate the crew clock: terminal behavior has its own real-fire case.
    // Otherwise the one enemy can die before180s and stop the game clock.
    let flights: Vec<_> = w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .copied()
        .collect();
    for id in flights {
        w.entities.remove(&id);
        w.rules.torpedoes.as_mut().unwrap().flights.remove(&id);
    }
    apply(
        &mut w,
        s,
        &Action::Reload {
            tube: 0,
            weapon: "us_mk48".into(),
        },
    )
    .unwrap();
    let due = w.tick + 180;
    for _ in 0..179 {
        assert!(!w.over, "reload clock fixture must remain live");
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(w.tick, due - 1);
    assert!(w.rules.torpedoes.as_ref().unwrap().tubes[&s].tubes[0]
        .loaded
        .is_none());
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(
        w.rules.torpedoes.as_ref().unwrap().tubes[&s].tubes[0]
            .loaded
            .as_deref(),
        Some("us_mk48")
    );
    assert_eq!(
        w.rules.torpedoes.as_ref().unwrap().tubes[&s].reserves["us_mk48"],
        1
    );
}
#[test]
fn no_source_solution_never_promotes_contact_quality_to_good_or_spends_ammo() {
    let (mut w, s, _) = fixture();
    let before = ammo(&w, s);
    w.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .config
        .orders
        .get_mut("SHOOTER:us_mk48")
        .unwrap()
        .solution = None;
    harpoon_core::sim::tick(&mut w, &[fire(s)]);
    assert_eq!(ammo(&w, s), before);
    assert!(w.rules.torpedoes.as_ref().unwrap().flights.is_empty());
}
#[test]
fn evasion_requires_two_completed_changes_after_detection() {
    let (mut w, _, t) = fixture();
    let threat = Threat {
        course: 0.,
        speed: 10.,
        depth_band: 1,
    };
    let e = w.entity_mut(t).unwrap();
    e.speed_kts = 20.;
    e.course_deg = 44.;
    assert!(!evasion(&threat, e, false));
    e.course_deg = 45.;
    assert!(evasion(&threat, e, false));
    e.speed_kts = 19.;
    assert!(!evasion(&threat, e, false));
    e.depth_m = 100.;
    assert!(evasion(&threat, e, false));
    e.speed_kts = 25.;
    e.course_deg = 90.;
    assert!(evasion(&threat, e, true));
}

#[test]
fn cut_wire_shooter_does_not_receive_referee_placement_or_enemy_defense_truth() {
    let (mut w, s, _) = fixture();
    harpoon_core::sim::tick(&mut w, &[fire(s)]);
    let id = *w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    for _ in 0..100 {
        if w.rules.torpedoes.as_ref().unwrap().flights[&id]
            .last_d100
            .is_some()
        {
            break;
        }
        assert!(!w.over);
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.rules.torpedoes.as_ref().unwrap().flights[&id]
        .last_d100
        .is_some());
    assert!(w.events.iter().any(|event|matches!(&event.kind,harpoon_core::EventKind::RulesEvent{rule,..} if rule=="10.2.5.source_terminal")));
    let view = harpoon_core::observe::observe(&w, Side::Blue, 0, 1000);
    assert!(!view.events.iter().any(|event|matches!(&event.kind,harpoon_core::EventKind::RulesEvent{rule,..} if rule=="10.2.4.placement"||rule=="10.2.5.source_terminal")),"unobserved coating/ACM/evasion and exact terminal outcome belong to referee state");
}

#[test]
fn v4_launch_without_any_enemy_sensor_never_creates_a_global_counter_detection() {
    let (mut w, s, t) = fixture();
    w.entity_mut(s).unwrap().speed_kts = 5.;
    w.entity_mut(t).unwrap().pos = Vec2::new(0., 1000.);
    for e in w.entities.values_mut() {
        e.sensors.clear();
    }
    w.sides[0].contacts.clear();
    w.sides[1].contacts.clear();
    // Referee launch fixture isolates the physical emission/observation hook.
    // No terminal or target-domain adjudication is requested by this case.
    let missile = w.db.weapons["us_tomahawk"].clone();
    assert!(
        harpoon_core::weapons::launch(&mut w, s, &missile, None, Some(Vec2::new(0., 10.)))
            .is_some()
    );
    assert!(!w.sides[Side::Red.index()].contacts.values().any(|c|c.about==s),"a launch transient must be observed by an eligible enemy sensor, not broadcast with true shooter position");
}

#[test]
fn source_submerged_launch_depth_and_speed_edges() {
    for depth in [0.01, 25., 26., 50.] {
        for speed in [3., 10.] {
            assert!(submerged_launch_legal(depth, speed));
        }
    }
    for (depth, speed) in [
        (0., 5.),
        (-1., 5.),
        (50.01, 5.),
        (25., 2.99),
        (25., 10.01),
        (f64::NAN, 5.),
        (25., f64::INFINITY),
    ] {
        assert!(
            !submerged_launch_legal(depth, speed),
            "source envelope must reject {depth}/{speed}"
        );
    }
}

#[test]
fn real_submerged_standoff_manual_and_auto_reject_without_spending_rounds() {
    // Explicit controlled mount: this isolates §10.3 and does not certify a
    // Virginia ASW-standoff installation, source range, or loaded VLS fit.
    for auto in [false, true] {
        for (depth, speed) in [(51., 5.), (50., 2.99), (25., 10.01)] {
            let (mut w, s, t) = fixture();
            w.entity_mut(t).unwrap().pos = Vec2::new(0., 5.);
            w.sides[0]
                .contacts
                .get_mut(&ContactId(99))
                .unwrap()
                .estimate = Vec2::new(0., 5.);
            let mut mount = w.entity(s).unwrap().weapons[0].clone();
            mount.weapon = "us_vlasroc".into();
            mount.ammo = 3;
            mount.cooldown_s = 0.;
            let e = w.entity_mut(s).unwrap();
            e.weapons = vec![mount];
            e.depth_m = depth;
            e.speed_kts = speed;
            e.assigned = Some(ContactId(99));
            if auto {
                harpoon_core::weapons::fire_control_scan(&mut w);
            } else {
                assert!(!harpoon_core::weapons::manual_fire(
                    &mut w,
                    Side::Blue,
                    s,
                    ContactId(99),
                    "us_vlasroc",
                    1
                ));
            }
            assert_eq!(w.entity(s).unwrap().weapons[0].ammo, 3);
            assert_eq!(w.entity(s).unwrap().weapons[0].cooldown_s, 0.);
            assert_eq!(w.stats.projectiles_launched, 0);
            assert!(w.events.iter().any(|event|matches!(&event.kind,harpoon_core::EventKind::RulesEvent{rule,..} if rule=="10.3.launch_blocked")));
        }
    }
}

#[test]
fn low_level_submerged_missile_launch_cannot_bypass_source_envelope() {
    for weapon in ["us_harpoon", "us_tomahawk", "us_vlasroc"] {
        let (mut w, s, _) = fixture();
        let def = w.db.weapons[weapon].clone();
        w.entity_mut(s).unwrap().speed_kts = 5.;
        w.entity_mut(s).unwrap().depth_m = 51.;
        assert!(harpoon_core::rules::torpedo2026::submerged_launch_gate(&w, s, &def).is_err());
        assert!(
            harpoon_core::weapons::launch(&mut w, s, &def, None, Some(Vec2::new(0., 10.)))
                .is_none()
        );
        assert_eq!(w.stats.projectiles_launched, 0);
        w.entity_mut(s).unwrap().depth_m = 50.;
        w.entity_mut(s).unwrap().speed_kts = 3.;
        assert!(harpoon_core::rules::torpedo2026::submerged_launch_gate(&w, s, &def).is_ok());
        if weapon == "us_harpoon" || weapon == "us_vlasroc" {
            // A legal depth/speed does not supply the separately required exact
            // ASCM/ASW source fit, plot/FCS and release permit. Do not bypass that gate
            // to keep a positive submarine Harpoon launch fixture.
            assert!(harpoon_core::weapons::launch(
                &mut w, s, &def, None, Some(Vec2::new(0., 10.))
            ).is_none());
            assert_eq!(w.stats.projectiles_launched, 0);
            continue;
        }
        assert!(
            harpoon_core::weapons::launch(&mut w, s, &def, None, Some(Vec2::new(0., 10.)))
                .is_some()
        );
        assert_eq!(w.stats.projectiles_launched, 1);
    }
}

#[test]
fn pending_torpedo_preserves_referee_evidence_without_publishing_hidden_reason() {
    let (mut w, s, t) = fixture();
    harpoon_core::sim::tick(&mut w, &[fire(s)]);
    let id = *w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    let original = w.entity(id).unwrap().clone();
    defer(
        &mut w,
        id,
        t,
        "source conflict after hidden ACM generation4",
    );
    let pending = &w.rules.pending_adjudications[&id];
    assert_eq!(
        serde_json::to_value(&pending.projectile).unwrap(),
        serde_json::to_value(&original).unwrap()
    );
    assert!(!pending.evidence.is_empty());
    let count = w.events.len();
    defer(
        &mut w,
        id,
        t,
        "source conflict after hidden ACM generation4",
    );
    assert_eq!(w.events.len(), count);
    assert!(w.events.iter().any(|event|matches!(&event.kind,harpoon_core::EventKind::RulesEvent{rule,..} if rule=="10.pending")&&event.vis==harpoon_core::Visibility::Referee));
    for side in [Side::Blue, Side::Red] {
        let view = harpoon_core::observe::observe(&w, side, 0, 1000);
        assert!(!view.events.iter().any(|event|matches!(&event.kind,harpoon_core::EventKind::RulesEvent{rule,detail,..} if rule=="10.pending"||detail.contains("hidden ACM"))));
    }
    let restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&w.rules.pending_adjudications).unwrap(),
        serde_json::to_value(&restored.rules.pending_adjudications).unwrap()
    );
}

fn straight_fixture(seed: u64) -> (World, EntityId, EntityId) {
    let (mut w, s, t) = fixture();
    w.seed = seed;
    w.rng = harpoon_core::Rng::new(seed);
    let id = "source_straight_fixture";
    let mut def = w.db.weapons["us_mk48"].clone();
    def.id = id.into();
    def.name = "Controlled straight-runner fixture".into();
    def.pk = 0.;
    def.salvo = 4;
    def.speed_kts = 50.;
    def.range_nm = 10.;
    def.targets.surface = true;
    def.targets.sub = true;
    def.rule_data = Some(harpoon_core::rules::damage_runtime::WeaponDamage {
        ship_dp: Some(1.),
        sub_dp: Some(1.),
        penetration_cm: Some(1.),
        fuze: "contact".into(),
        source: "scenario_assumption: controlled 1DP contact warhead; not a historical annex fit"
            .into(),
        ..Default::default()
    });
    w.db.weapons.insert(id.into(), def);
    let state = w.rules.torpedoes.as_mut().unwrap();
    state.config.weapons.insert(id.into(),Weapon{source:"scenario_assumption: controlled Gyro2 speed/range, not Mk48 or53-39 annex statistics".into(),guidance:Guidance::Gyro2,generation:0,settings:vec![Setting{speed_kts:50.,range_nm:10.}],max_depth_m:50.,shallow_water_capable:false,surface:true,submarine:true,launch_offset:None,max_sea_state:None});
    state.config.targets.get_mut("cn_093").unwrap().size = 1;
    state.config.targets.get_mut("cn_093").unwrap().anechoic = false;
    let mut order = state.config.orders["SHOOTER:us_mk48"].clone();
    order.depth_m = 0.;
    order.bearing_deg = Some(0.);
    order.spread_deg = vec![0.; 4];
    order.source =
        "scenario_assumption: declared corrected intercept bearing for controlled spread".into();
    state.config.orders.insert(format!("SHOOTER:{id}"), order);
    state.tubes.get_mut(&s).unwrap().tubes = vec![
        Tube {
            bearing_deg: 0.,
            loaded: Some(id.into())
        };
        4
    ];
    let mut mount = w.entity(s).unwrap().weapons[0].clone();
    mount.weapon = id.into();
    mount.mounts = 4;
    mount.ammo = 4;
    mount.cooldown_s = 0.;
    w.entity_mut(s).unwrap().weapons = vec![mount];
    let e = w.entity_mut(t).unwrap();
    e.kind = harpoon_core::types::PlatformKind::Ship;
    e.pos = Vec2::new(-0.25, 1.25);
    e.depth_m = 0.;
    e.ordered_depth_m = 0.;
    e.course_deg = 90.;
    e.ordered_course_deg = 90.;
    e.speed_kts = 10.;
    e.ordered_speed_kts = 10.;
    e.hp = 100000.;
    e.hp_max = 100000.;
    e.rule_data=Some(harpoon_core::rules::damage_runtime::PlatformDamage{size:"B".into(),role:"surface".into(),source:"scenario_assumption: controlled unarmored medium ship, high HP isolates one1DP source hit".into(),..Default::default()});
    w.sides[0]
        .contacts
        .get_mut(&ContactId(99))
        .unwrap()
        .estimate = Vec2::new(-0.25, 1.25);
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", t.0),
        harpoon_core::types::PlatformKind::Ship,
    );
    (w, s, t)
}
fn straight_fire(s: EntityId) -> (Side, Command) {
    (
        Side::Blue,
        Command::Fire {
            unit: s,
            contact: ContactId(99),
            weapon: "source_straight_fixture".into(),
            rounds: 4,
        },
    )
}
fn straight_events(w: &World) -> Vec<serde_json::Value> {
    w.events
        .iter()
        .filter_map(|event| match &event.kind {
            harpoon_core::EventKind::RulesEvent { rule, detail, .. }
                if rule == "10.2.3.source_salvo" =>
            {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn await_straight(w: &mut World, n: usize, max: usize) {
    for _ in 0..max {
        if straight_events(w).len() >= n || !w.rules.pending_adjudications.is_empty() {
            break;
        }
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}

#[test]
fn real_straight_four_round_salvo_uses_one_seeded_table_roll_and_source_damage() {
    let (mut w, s, t) = straight_fixture(20261023);
    harpoon_core::sim::tick(&mut w, &[straight_fire(s)]);
    assert_eq!(w.entity(s).unwrap().weapons[0].ammo, 0);
    await_straight(&mut w, 1, 200);
    assert!(
        w.rules.pending_adjudications.is_empty(),
        "bound source Gyro2 spread must leave the former !acoustic pending path"
    );
    let events = straight_events(&w);
    assert_eq!(
        events.len(),
        1,
        "one D100 for the complete spread, not four per-projectile attacks"
    );
    assert_eq!(events[0]["target"], json!(t));
    assert_eq!(events[0]["members"].as_array().unwrap().len(), 4);
    assert_eq!(events[0]["d100"], 37);
    assert_eq!(events[0]["hits"], 1); // fixed seed, source Medium4/run≈1.25:16–59 means1
    assert_eq!(events[0]["effective_size"], "Medium");
    let ordinary = w
        .events
        .iter()
        .find_map(|event| match &event.kind {
            harpoon_core::EventKind::Impact {
                side: Side::Red,
                unit,
                weapon,
                damage,
                ..
            } if *unit == t && weapon == "Controlled straight-runner fixture" => Some(*damage),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        ordinary, 1.,
        "source one-hit count must reach the actual1DP damage queue even though legacyPk is0"
    );
    assert!(
        w.entity(t).unwrap().hp <= 99999.,
        "source secondary flood/critical effects must remain intact"
    );
}

#[test]
fn real_straight_spread_can_hit_an_intervening_friendly_not_the_designated_target() {
    let (mut w, s, t) = straight_fixture(3);
    w.entity_mut(t).unwrap().pos = Vec2::new(0., 5.);
    w.sides[0]
        .contacts
        .get_mut(&ContactId(99))
        .unwrap()
        .estimate = Vec2::new(0., 5.);
    let friend = w.allocate_id();
    let mut e = w.entity(t).unwrap().clone();
    e.id = friend;
    e.side = Side::Blue;
    e.name = "INTERVENING_FRIEND".into();
    e.pos = Vec2::new(0., 1.25);
    e.speed_kts = 0.;
    e.ordered_speed_kts = 0.;
    w.entities.insert(friend, e);
    harpoon_core::sim::tick(&mut w, &[straight_fire(s)]);
    await_straight(&mut w, 1, 200);
    assert!(w.rules.pending_adjudications.is_empty());
    let events = straight_events(&w);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["target"], json!(friend));
    assert_eq!(events[0]["d100"], 76);
    assert_eq!(events[0]["hits"], 1); // stationary ship shifts Medium row up to0.5nm
    let ordinary = w
        .events
        .iter()
        .find_map(|event| match &event.kind {
            harpoon_core::EventKind::Impact {
                side: Side::Blue,
                unit,
                weapon,
                damage,
                ..
            } if *unit == friend && weapon == "Controlled straight-runner fixture" => Some(*damage),
            _ => None,
        })
        .unwrap();
    assert_eq!(ordinary, 1.);
    assert!(w.entity(friend).unwrap().hp <= 99999.);
    assert_eq!(w.entity(t).unwrap().hp, 100000.);
}

#[test]
fn real_straight_partial_hit_splits_survivors_but_total_miss_preserves_the_spread() {
    for (seed, first_die, first_hits, second_attacks, second_members) in
        [(3, 45, 1, 3, 1), (2, 79, 0, 1, 4)]
    {
        let (mut w, s, t) = straight_fixture(seed);
        let second = w.allocate_id();
        let mut e = w.entity(t).unwrap().clone();
        e.id = second;
        e.name = "SECOND_SHIP".into();
        e.pos = Vec2::new(0., 2.5);
        e.speed_kts = 0.;
        e.ordered_speed_kts = 0.;
        w.entities.insert(second, e);
        harpoon_core::sim::tick(&mut w, &[straight_fire(s)]);
        await_straight(&mut w, 1 + second_attacks, 400);
        assert!(w.rules.pending_adjudications.is_empty());
        let events = straight_events(&w);
        assert_eq!(events[0]["target"], json!(t));
        assert_eq!(events[0]["d100"], first_die);
        assert_eq!(events[0]["hits"], first_hits);
        let later: Vec<_> = events
            .iter()
            .filter(|event| event["target"] == json!(second))
            .collect();
        assert_eq!(
            later.len(),
            second_attacks,
            "partial hit leaves single weapons; an entire miss retains one full spread"
        );
        assert!(later
            .iter()
            .all(|event| event["members"].as_array().unwrap().len() == second_members));
    }
}

#[test]
fn real_straight_in_flight_save_restore_preserves_salvo_and_referee_dice() {
    let (mut w, s, _) = straight_fixture(20261023);
    harpoon_core::sim::tick(&mut w, &[straight_fire(s)]);
    for _ in 0..10 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.rules.pending_adjudications.is_empty());
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..250 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert!(!straight_events(&w).is_empty());
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn real_straight_collision_uses_relative_swept_paths_when_both_endpoints_miss() {
    let (mut w, s, t) = straight_fixture(20261023);
    // Referee geometry stress fixture: the prescribed segment crosses the
    // torpedo track within one increment; no historical ship speed is claimed.
    w.entity_mut(t).unwrap().pos = Vec2::new(-0.3, 0.01);
    w.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .previous_positions
        .insert(t, Vec2::new(-0.3, 0.01));
    assert!(harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        s,
        ContactId(99),
        "source_straight_fixture",
        4
    ));
    w.entity_mut(t).unwrap().pos = Vec2::new(0.3, 0.01);
    w.tick += 1;
    step(&mut w);
    harpoon_core::weapons::advance_projectiles(&mut w);
    let events = straight_events(&w);
    assert_eq!(events.len(),1,"proportional relative motion must detect the crossing even when end-position distance exceeds0.25nm");
    assert_eq!(events[0]["target"], json!(t));
    let surviving = w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .filter_map(|id| w.entity(*id).filter(|e| e.alive))
        .next()
        .unwrap();
    assert!(surviving.pos.range_to(w.entity(t).unwrap().pos) > 0.25);
}

#[test]
fn straight_range_bounds_running_weapon_without_adding_a_target_center_range_gate() {
    // §10.2 measures range to the running weapon; §10.2.2 independently
    // attacks an eligible vessel inside the printed0.25nm danger radius.
    for (distance, attacked) in [(1.20, true), (1.26, false)] {
        let (mut w, s, t) = straight_fixture(20261023);
        w.rules
            .torpedoes
            .as_mut()
            .unwrap()
            .config
            .weapons
            .get_mut("source_straight_fixture")
            .unwrap()
            .settings[0]
            .range_nm = 1.;
        let e = w.entity_mut(t).unwrap();
        e.pos = Vec2::new(0., distance);
        e.speed_kts = 0.;
        e.ordered_speed_kts = 0.;
        w.sides[0]
            .contacts
            .get_mut(&ContactId(99))
            .unwrap()
            .estimate = Vec2::new(0., distance);
        harpoon_core::sim::tick(&mut w, &[straight_fire(s)]);
        for _ in 0..100 {
            assert!(!w.over);
            harpoon_core::sim::tick(&mut w, &[]);
        }
        assert_eq!(!straight_events(&w).is_empty(), attacked);
        let state = w.rules.torpedoes.as_ref().unwrap();
        for (id, f) in &state.flights {
            assert!(f.run_nm<=1.+1e-10,"a danger zone never permits the running torpedo to travel beyond its source endurance");
            assert!(w.entity(*id).is_none_or(|e|!e.alive),"the remaining spread must retire at source range; cleanup may remove the dead entity");
        }
    }
}

#[test]
fn unknown_intervening_size_freezes_every_still_live_salvo_member_and_preserves_evidence() {
    let (mut w, s, t) = straight_fixture(20261023);
    w.entity_mut(t).unwrap().pos = Vec2::new(0., 5.);
    w.sides[0]
        .contacts
        .get_mut(&ContactId(99))
        .unwrap()
        .estimate = Vec2::new(0., 5.);
    let unknown = w.allocate_id();
    let mut e = w.entity(t).unwrap().clone();
    e.id = unknown;
    e.name = "UNKNOWN_INTERVENING".into();
    e.class = "unbound_intervening_class".into();
    e.rule_data = None;
    e.pos = Vec2::new(0., 1.25);
    e.speed_kts = 0.;
    e.ordered_speed_kts = 0.;
    w.entities.insert(unknown, e);
    harpoon_core::sim::tick(&mut w, &[straight_fire(s)]);
    await_straight(&mut w, 1, 200);
    assert!(straight_events(&w).is_empty());
    assert_eq!(
        w.rules.pending_adjudications.len(),
        4,
        "the whole source spread must stop when an intervening target cannot be adjudicated"
    );
    for (id, pending) in &w.rules.pending_adjudications {
        assert_eq!(pending.target, unknown);
        assert_eq!(pending.projectile.id, *id);
        assert!(!pending.evidence.is_empty());
    }
    let saved = serde_json::to_value(&w.rules.pending_adjudications).unwrap();
    for _ in 0..5 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(
        saved,
        serde_json::to_value(&w.rules.pending_adjudications).unwrap()
    );
    let restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(
        saved,
        serde_json::to_value(&restored.rules.pending_adjudications).unwrap()
    );
}

#[test]
fn unobserved_enemy_depth_does_not_probe_release_admission_but_still_blocks_physical_acquisition() {
    let (mut w, s, t) = fixture();
    w.entity_mut(t).unwrap().depth_m = 1000.;
    let hp = w.entity(t).unwrap().hp;
    let before = ammo(&w, s);
    assert!(
        harpoon_core::weapons::manual_fire(&mut w, Side::Blue, s, ContactId(99), "us_mk48", 1),
        "unknown enemy true depth must not determine command admission"
    );
    assert_eq!(ammo(&w, s), before - 1);
    let id = *w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    for _ in 0..200 {
        w.tick += 1;
        harpoon_core::weapons::advance_projectiles(&mut w);
        harpoon_core::rules::damage_runtime::finish(&mut w);
    }
    assert!(w.rules.torpedoes.as_ref().unwrap().flights[&id]
        .last_d100
        .is_none());
    assert_eq!(
        w.entity(t).unwrap().hp,
        hp,
        "source900m maximum must still prevent real acquisition/hit on1000m target"
    );
}

#[test]
fn unobserved_foil_borne_state_does_not_probe_release_admission_but_remains_immune() {
    let (mut w, s, t) = fixture();
    w.entity_mut(t).unwrap().kind = harpoon_core::types::PlatformKind::Ship;
    w.entity_mut(t).unwrap().depth_m = 0.;
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", t.0),
        harpoon_core::types::PlatformKind::Ship,
    );
    w.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .config
        .targets
        .get_mut("cn_093")
        .unwrap()
        .foil_borne = true;
    let hp = w.entity(t).unwrap().hp;
    let before = ammo(&w, s);
    assert!(
        harpoon_core::weapons::manual_fire(&mut w, Side::Blue, s, ContactId(99), "us_mk48", 1),
        "unobserved foil state must not be exposed through a rejected order"
    );
    assert_eq!(ammo(&w, s), before - 1);
    for _ in 0..200 {
        w.tick += 1;
        harpoon_core::weapons::advance_projectiles(&mut w);
        harpoon_core::rules::damage_runtime::finish(&mut w);
    }
    assert_eq!(
        w.entity(t).unwrap().hp,
        hp,
        "foil-borne craft remain physically immune to torpedoes"
    );
}

fn real_acoustic_percent_edge(seed: u64, wanted: u8, hit: bool) {
    use harpoon_core::rules::ew2026 as ew;
    let (mut w, s, t) = fixture();
    w.seed = seed;
    w.rng = harpoon_core::Rng::new(seed);
    w.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .config
        .targets
        .get_mut("cn_093")
        .unwrap()
        .anechoic = false;
    w.entity_mut(t).unwrap().speed_kts = 10.;
    w.entity_mut(t).unwrap().course_deg = 0.;
    assert!(harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        s,
        ContactId(99),
        "us_mk48",
        1
    ));
    let id = *w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    let mut detection = w.sides[0].contacts[&ContactId(99)].clone();
    detection.id = ContactId(100);
    detection.about = id;
    detection.category = harpoon_core::types::PlatformKind::Torpedo;
    detection.estimate = w.entity(id).unwrap().pos;
    w.sides[1].contacts.insert(detection.id, detection);
    w.tick += 1;
    harpoon_core::weapons::advance_projectiles(&mut w);
    assert!(w.rules.torpedoes.as_ref().unwrap().flights[&id]
        .threat
        .is_some());
    w.entity_mut(t).unwrap().speed_kts = 20.;
    w.entity_mut(t).unwrap().course_deg = 45.;
    let state = w.rules.ew.as_mut().unwrap();
    state.config.fits.insert("source_gen2".into(),ew::Fit{source:"scenario_assumption: controlled gen2 ACM installation for source percentage boundary".into(),kind:ew::ObjectKind::TorpedoAcm,generation:Some(2),launcher:Some(ew::Launcher::Internal),endurance_s:None,deployment_s:None,radar_height_m:None});
    state
        .stocks
        .entry(t)
        .or_default()
        .insert("source_gen2".into(), 1);
    ew::apply(
        &mut w,
        t,
        &ew::Action::Deploy {
            fit: "source_gen2".into(),
            at: None,
            radar_signature: None,
            course_deg: None,
            speed_kts: None,
            depth_m: None,
        },
    )
    .unwrap();
    for _ in 0..100 {
        if w.rules.torpedoes.as_ref().unwrap().flights[&id]
            .last_d100
            .is_some()
        {
            break;
        }
        w.tick += 1;
        harpoon_core::weapons::advance_projectiles(&mut w);
    }
    let terminal = w
        .events
        .iter()
        .find_map(|event| match &event.kind {
            harpoon_core::EventKind::RulesEvent { rule, detail, .. }
                if rule == "10.2.5.source_terminal" =>
            {
                Some(serde_json::from_str::<serde_json::Value>(detail).unwrap())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(terminal["probability"], 0.58);
    assert_eq!(terminal["d100"], wanted);
    assert_eq!(terminal["hit"], hit);
}
#[test]
fn real_source_fifty_eight_percent_includes_seeded_die58() {
    real_acoustic_percent_edge(138, 58, true);
}
#[test]
fn real_source_fifty_eight_percent_excludes_seeded_die59() {
    real_acoustic_percent_edge(89, 59, false);
}

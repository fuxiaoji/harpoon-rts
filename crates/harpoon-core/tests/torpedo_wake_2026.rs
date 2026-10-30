//! §7.5.5/§10.2.6 real source launch, physical wake crossing and movement.
//! Width, lifetime and prehistory are explicit test-author conditions, not
//! Harpoon constants.65-76A values: Russia2025 PDF90/F-1,91/F-2 remarkE.
use harpoon_core::{Command, ContactId, Database, EntityId, Scenario, Side, Vec2, World};
use serde_json::{json, Value};

fn fixture(history: bool, offset: bool, geometry: bool) -> (World, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut weapon = json!({"source":"Russia2025 PDF90/F-1 exact65-76A Kit;F-2 remarkE; historical1998 fixture",
        "guidance":"Wake","generation":3,"settings":[{"speed_kts":50,"range_nm":27}],
        "max_depth_m":25,"shallow_water_capable":false,"surface":true,"submarine":false,"max_sea_state":4});
    if offset {
        weapon["launch_offset"] = json!({"source":"scenario_assumption: controlled650mm tube permits stated bearing only; not a book Wake default","degrees":0});
    }
    let mut cfg = json!({"source":"scenario_assumption: controlled historical1998 Wake mechanism acceptance",
        "weapons":{"ru_65k":weapon},
        "environment":{"source":"scenario_assumption: deep open water no thermocline or bottom reverberation","layer_m":null,"reverberant":false},
        "targets":{"cn_001":{"source":"scenario_assumption: controlled carrier size/noise/coating","size":0,"noise":2,"anechoic":false,"foil_borne":false}},
        "platforms":{"ru_oscar2":{"source":"scenario_assumption: exercise enables exactly one initial650mm test tube; full Oscar reload/refit not claimed",
            "wire_capacity":0,"reload_parallel":1,"tubes":[{"bearing_deg":90,"loaded":"ru_65k"}],"tube_ids":{"TEST650":0},"reserves":{}}},
        "orders":{"SHOOTER:ru_65k":{"source":"scenario_assumption: declared bearing crosses astern of known northbound target",
            "speed_index":0,"depth_m":5,"enable_run_nm":0,"bearing_deg":90,"spread_deg":[],"cut_wire":true,
            "solution":{"quality":"Good","source":"scenario_assumption: known source firing solution"}}}});
    if geometry {
        cfg["wake"] = json!({"source":"scenario_assumption: declared geometric footprint, not a source constant",
        "half_width_nm":0.02,"lifetime_s":600,"history_policy":"explicit_segments_only",
        "initial_segments":if history{json!([{"target_unit":"TARGET","from":{"x":0,"y":-1},"to":{"x":0,"y":1},"created_tick":-30}])}else{json!([])}});
    }
    let sc:Scenario=serde_json::from_value(json!({"id":"wake-source-1998","name":"Source Wake physical crossing",
        "duration_s":1500,"seed":20261023,"context":{"rules_profile":"hv2026-rts-v4",
        "radar_environment":{"sea_state":4,"precipitation":"None"},"torpedo_2026":cfg},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"ru_oscar2","name":"SHOOTER","pos":[-1,-0.5],"course_deg":0,"speed_kts":0,"depth_m":25,"fire_on":false},
            {"side":"Red","class":"cn_001","name":"TARGET","pos":[0,1],"course_deg":0,"speed_kts":0,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let s = w
        .entities
        .values()
        .find(|e| e.name == "SHOOTER")
        .unwrap()
        .id;
    let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    w.entity_mut(s).unwrap().sensors.clear();
    w.entity_mut(t).unwrap().sensors.clear();
    w.entity_mut(t).unwrap().weapons.clear();
    w.entity_mut(t).unwrap().hp = 10_000.;
    w.entity_mut(t).unwrap().hp_max = 10_000.;
    w.sides[0].contacts.insert(
        ContactId(91),
        harpoon_core::world::Contact {
            id: ContactId(91),
            about: t,
            category: harpoon_core::types::PlatformKind::Ship,
            class_name: None,
            estimate: Vec2::new(0., 1.),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec![],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", t.0),
        harpoon_core::types::PlatformKind::Ship,
    );
    (w, s, t)
}
fn ammo(w: &World, s: EntityId) -> u16 {
    w.entity(s)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "ru_65k")
        .unwrap()
        .ammo
}
fn fire(s: EntityId) -> (Side, Command) {
    (
        Side::Blue,
        Command::Fire {
            unit: s,
            contact: ContactId(91),
            weapon: "ru_65k".into(),
            rounds: 1,
        },
    )
}
fn events(w: &World, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent {
                rule: r, detail, ..
            } if r == rule => Some(serde_json::from_str(detail).unwrap()),
            _ => None,
        })
        .collect()
}
fn launch(w: &mut World, s: EntityId) -> EntityId {
    let before = ammo(w, s);
    harpoon_core::sim::tick(w, &[fire(s)]);
    assert_eq!(
        ammo(w, s),
        before - 1,
        "a fully bound source Wake example must enter the real release pipeline"
    );
    *w.rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .expect("source Flight for the real released torpedo")
}
#[test]
fn wake_source_fit_missing_offset_or_geometry_never_borrows_acoustic_defaults() {
    for (offset, geometry) in [(false, true), (true, false)] {
        let (mut w, s, _) = fixture(true, offset, geometry);
        let before = ammo(&w, s);
        harpoon_core::sim::tick(&mut w, &[fire(s)]);
        assert_eq!(ammo(&w, s), before);
        assert!(w.rules.torpedoes.as_ref().unwrap().flights.is_empty());
    }
}
#[test]
fn a_real_wake_launch_near_a_ship_without_crossing_a_wake_does_not_home() {
    let (mut w, s, t) = fixture(false, true, true);
    let id = launch(&mut w, s);
    for _ in 0..100 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(events(&w, "10.2.6.wake_crossing").is_empty());
    assert!(matches!(
        w.rules.torpedoes.as_ref().unwrap().flights[&id].phase,
        harpoon_core::rules::torpedo2026::Phase::Run
    ));
    assert_eq!(w.entity(t).unwrap().hp, 10_000.);
}
#[test]
fn real_swept_wake_crossing_changes_course_only_after_crossing_and_closes_at_three_quarters() {
    let (mut w, s, _) = fixture(true, true, true);
    let id = launch(&mut w, s);
    for _ in 0..90 {
        if !events(&w, "10.2.6.wake_crossing").is_empty() {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(
        events(&w, "10.2.6.wake_crossing").len(),
        1,
        "the real eastbound run must physically cross the authored north/south wake"
    );
    assert!(
        events(&w, "10.2.6.wake_placement")[0]["placement"]["placed"]
            .as_bool()
            .unwrap(),
        "fixed seed source solution must actually acquire"
    );
    let before = w.entity(id).unwrap().pos;
    for _ in 0..60 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let after = w.entity(id).unwrap().pos;
    assert!(
        (before.range_to(after) - 0.625).abs() < 1e-10,
        "50kt source speed has37.5kt made-good:60s closes0.625nm"
    );
    assert_eq!(
        events(&w, "10.2.6.wake_crossing").len(),
        1,
        "one physical encounter is not rerolled on every tick"
    );
}
#[test]
fn in_flight_source_wake_crossing_and_path_survive_full_world_save_restore() {
    let (mut w, s, _) = fixture(true, true, true);
    let _ = launch(&mut w, s);
    for _ in 0..90 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(events(&w, "10.2.6.wake_crossing").len(), 1);
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    for _ in 0..45 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

fn until(w: &mut World, rule: &str) {
    for _ in 0..1000 {
        if !events(w, rule).is_empty() {
            return;
        }
        assert!(
            !w.over,
            "controlled nonlethal fixture ended before source adjudication"
        );
        harpoon_core::sim::tick(w, &[]);
    }
    panic!("real source pipeline never emitted {rule}");
}
fn deploy(
    w: &mut World,
    target: EntityId,
    name: &str,
    kind: harpoon_core::rules::ew2026::ObjectKind,
    generation: u8,
) -> EntityId {
    use harpoon_core::rules::ew2026 as ew;
    let state = w.rules.ew.as_mut().unwrap();
    state.config.fits.insert(name.into(),ew::Fit{source:"scenario_assumption: exact test countermeasure generation,30s towed deployment/720s external endurance".into(),
        kind,generation:Some(generation),launcher:Some(ew::Launcher::ExternalLarge),endurance_s:Some(720),deployment_s:Some(30),radar_height_m:None});
    state
        .config
        .loadouts
        .entry("cn_001".into())
        .or_default()
        .insert(name.into(), 1);
    // Once the first deployment materializes runtime stock, a second authored
    // fit must also be stocked in that same inventory, not only its template.
    state
        .stocks
        .entry(target)
        .or_default()
        .insert(name.into(), 1);
    ew::apply(
        w,
        target,
        &ew::Action::Deploy {
            fit: name.into(),
            at: None,
            radar_signature: None,
            course_deg: None,
            speed_kts: None,
            depth_m: None,
        },
    )
    .unwrap();
    *w.rules
        .ew
        .as_ref()
        .unwrap()
        .objects
        .keys()
        .next_back()
        .unwrap()
}
#[test]
fn real_source_wake_terminal_uses_gen3_base70_and_actual_damage_queue() {
    let (mut w, s, t) = fixture(true, true, true);
    let _ = launch(&mut w, s);
    until(&mut w, "10.2.6.wake_terminal");
    let r = events(&w, "10.2.6.wake_terminal")[0].clone();
    assert_eq!(r["probability"], 0.70);
    assert_eq!(
        r["d100"], 34,
        "fixed seed20261023/id3 raw source attack die"
    );
    assert_eq!(r["hit"], true);
    assert!(
        w.entity(t).unwrap().hp < 10_000.,
        "a real source hit must reach actual damage, not a table-only result"
    );
}
#[test]
fn a_real_gen3_towed_seduction_destroys_both_objects_before_any_damage() {
    use harpoon_core::rules::ew2026 as ew;
    let (mut w, s, t) = fixture(true, true, true);
    let id = launch(&mut w, s);
    let decoy = deploy(&mut w, t, "TOW3", ew::ObjectKind::TowedTorpedoDecoy, 3);
    until(&mut w, "10.2.6.wake_towed");
    let r = events(&w, "10.2.6.wake_towed")[0].clone();
    assert_eq!(r["probability"], 0.39);
    assert_eq!(r["d100"], 42);
    assert_eq!(r["ignored"], false);
    assert!(w.entity(id).is_none_or(|e| !e.alive));
    assert_eq!(
        w.rules.ew.as_ref().unwrap().objects[&decoy].status,
        ew::Status::Destroyed
    );
    assert_eq!(w.entity(t).unwrap().hp, 10_000.);
    assert!(
        events(&w, "10.2.6.wake_terminal").is_empty(),
        "a seduced weapon has no second attack roll"
    );
}
#[test]
fn towed_resistance_then_independent_miss_has_a_real500yard_right_reverse_and_saved_dice() {
    use harpoon_core::rules::ew2026 as ew;
    let (mut w, s, t) = fixture(true, true, true);
    w.rng = harpoon_core::Rng::new(26);
    let id = launch(&mut w, s);
    let decoy = deploy(&mut w, t, "TOW3", ew::ObjectKind::TowedTorpedoDecoy, 3);
    until(&mut w, "10.2.6.wake_terminal");
    let first = events(&w, "10.2.6.wake_towed")[0].clone();
    let second = events(&w, "10.2.6.wake_terminal")[0].clone();
    assert_eq!(first["d100"], 16);
    assert_eq!(first["probability"], 0.39);
    assert_eq!(first["ignored"], true);
    assert_eq!(second["d100"], 92);
    assert_eq!(second["probability"], 0.70);
    assert_eq!(second["hit"], false);
    assert_eq!(
        w.rules.ew.as_ref().unwrap().objects[&decoy].status,
        ew::Status::Deployed
    );
    let attack_pos = w.entity(id).unwrap().pos;
    let incoming = w.entity(id).unwrap().course_deg;
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    for _ in 0..24 {
        if !events(&w, "10.2.7.wake_reverse").is_empty() {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(events(&w, "10.2.7.wake_reverse").len(), 1);
    assert!(
        (attack_pos.range_to(w.entity(id).unwrap().pos) - 0.2468682505399568).abs() < 1e-10,
        "source Wake overshoot is500yd, not acoustic1000yd"
    );
    assert!(
        harpoon_core::geom::angle_diff(w.entity(id).unwrap().course_deg, incoming + 180.).abs()
            < 1e-10
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert_eq!(w.entity(t).unwrap().hp, 10_000.);
}
#[test]
fn early_towed_nontowed_acm_and_sonar_jammer_do_not_enter_the_wake_towed_roll() {
    use harpoon_core::rules::ew2026 as ew;
    for (kind, generation) in [
        (ew::ObjectKind::TowedTorpedoDecoy, 1),
        (ew::ObjectKind::TowedTorpedoDecoy, 2),
        (ew::ObjectKind::TorpedoAcm, 4),
        (ew::ObjectKind::SonarJammer, 4),
    ] {
        let (mut w, s, t) = fixture(true, true, true);
        let _ = launch(&mut w, s);
        let _ = deploy(&mut w, t, "DEFENSE", kind, generation);
        until(&mut w, "10.2.6.wake_terminal");
        assert!(events(&w, "10.2.6.wake_towed").is_empty());
        assert_eq!(
            events(&w, "10.2.6.wake_terminal")[0]["probability"],
            0.70,
            "Wake never uses combined acousticACM columns"
        );
    }
}
#[test]
fn multiple_eligible_towed_objects_preserve_complete_pending_without_a_guessed_priority() {
    use harpoon_core::rules::ew2026 as ew;
    let (mut w, s, t) = fixture(true, true, true);
    let id = launch(&mut w, s);
    let one = deploy(&mut w, t, "TOW3", ew::ObjectKind::TowedTorpedoDecoy, 3);
    let two = deploy(&mut w, t, "TOW4", ew::ObjectKind::TowedTorpedoDecoy, 4);
    for _ in 0..500 {
        if w.rules.pending_adjudications.contains_key(&id) {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let pending = w
        .rules
        .pending_adjudications
        .get(&id)
        .expect("unresolved multiple towed selection must save a real pending projectile");
    assert!(
        pending.rule.contains("multiple"),
        "a generic unimplemented Wake error is not the source ambiguity guard"
    );
    assert_eq!(pending.projectile.id, id);
    assert!(!pending.evidence.is_empty());
    assert!(events(&w, "10.2.6.wake_towed").is_empty());
    assert_eq!(
        w.rules.ew.as_ref().unwrap().objects[&one].status,
        ew::Status::Deployed
    );
    assert_eq!(
        w.rules.ew.as_ref().unwrap().objects[&two].status,
        ew::Status::Deployed
    );
}
fn mark_detected_torpedo(w: &mut World, target: EntityId, torpedo: EntityId) {
    let side = w.entity(target).unwrap().side;
    let pos = w.entity(torpedo).unwrap().pos;
    w.sides[side.index()].contacts.insert(
        ContactId(199),
        harpoon_core::world::Contact {
            id: ContactId(199),
            about: torpedo,
            category: harpoon_core::types::PlatformKind::Torpedo,
            class_name: None,
            estimate: pos,
            est_course_deg: 90.,
            est_speed_kts: 50.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: w.tick,
            first_tick: w.tick,
            stale: false,
            sensors: vec!["scenario_detected_torpedo".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("{}:{}", side.index(), torpedo.0),
        harpoon_core::types::PlatformKind::Torpedo,
    );
}
#[test]
fn real_detected_wake_threat_requires25knots_and90degree_turn_together() {
    for (speed, turn, detected, expected) in [
        (25., 90., true, 0.61),
        (24.9, 90., true, 0.70),
        (25., 89.9, true, 0.70),
        (25., 90., false, 0.70),
    ] {
        let (mut w, s, t) = fixture(true, true, true);
        let id = launch(&mut w, s);
        if detected {
            mark_detected_torpedo(&mut w, t, id);
        }
        harpoon_core::sim::tick(&mut w, &[]);
        // Explicit author-controlled actuator fixture: actual SetCourse and
        // SetSpeed complete before impact. No manual Threat/Locked state.
        let actuator = w.db.platforms.get_mut("cn_001").unwrap();
        actuator.turn_rate_deg_s = 360.;
        actuator.accel_kts_per_s = 100.;
        harpoon_core::sim::tick(
            &mut w,
            &[
                (
                    Side::Red,
                    Command::SetCourse {
                        unit: t,
                        course_deg: turn,
                    },
                ),
                (
                    Side::Red,
                    Command::SetSpeed {
                        unit: t,
                        speed_kts: speed,
                    },
                ),
            ],
        );
        until(&mut w, "10.2.6.wake_terminal");
        assert_eq!(events(&w,"10.2.6.wake_terminal")[0]["probability"],expected,"speed{speed}/turn{turn}/detected{detected}: Wake source requires both maneuvers after detection");
    }
}
#[test]
fn exact65_76a_own_sea_state_and_run_depth_limits_do_not_borrow_model_defaults() {
    for invalid_sea in [false, true] {
        let (mut w, s, _) = fixture(true, true, true);
        let before = ammo(&w, s);
        if invalid_sea {
            w.context.radar_environment.as_mut().unwrap().sea_state = 5;
        } else {
            w.rules
                .torpedoes
                .as_mut()
                .unwrap()
                .config
                .orders
                .get_mut("SHOOTER:ru_65k")
                .unwrap()
                .depth_m = 25.1;
        }
        harpoon_core::sim::tick(&mut w, &[fire(s)]);
        assert_eq!(ammo(&w, s), before);
        assert!(w.rules.torpedoes.as_ref().unwrap().flights.is_empty());
    }
}

#[test]
fn another_ship_really_crossing_the_followed_wake_cannot_silently_keep_the_original_terminal() {
    let (mut w, s, t) = fixture(true, true, true);
    let crossing = w.allocate_id();
    let mut ship = w.entity(t).unwrap().clone();
    ship.id = crossing;
    ship.name = "ACTUAL_CROSSING_SHIP".into();
    ship.pos = Vec2::new(-0.05, 0.);
    ship.course_deg = 90.;
    ship.ordered_course_deg = 90.;
    ship.speed_kts = 20.;
    ship.ordered_speed_kts = 20.;
    w.entities.insert(crossing, ship);
    let id = launch(&mut w, s);
    for _ in 0..300 {
        if w.rules.pending_adjudications.contains_key(&id) {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let state = w.rules.torpedoes.as_ref().unwrap();
    assert!(
        state
            .wakes
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .any(|segment| {
                segment.target == crossing && segment.from.x <= 0. && segment.to.x >= 0.
            }),
        "a real moving ship must have physically crossed the original target wake"
    );
    let pending = w.rules.pending_adjudications.get(&id).expect(
        "an encountered real wake junction needs the source crossing-vessel ACM roll or an explicit pending guard",
    );
    assert!(pending.rule.contains("crossing") || pending.rule.contains("junction"));
    assert_eq!(pending.projectile.id, id);
    assert!(!pending.evidence.is_empty());
    let junction = events(&w, "7.5.5.wake_junction_pending");
    assert_eq!(junction.len(), 1);
    assert_eq!(
        junction[0]["junctions"][0]["crossing_ship"],
        json!(crossing)
    );
    assert!(w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .wakes
        .as_ref()
        .unwrap()
        .pending_junctions
        .contains_key(&id));
    assert!(
        events(&w, "10.2.6.wake_terminal").is_empty(),
        "the source cannot ignore the physical branch and roll .70 against the original ship"
    );
    assert_eq!(w.entity(t).unwrap().hp, 10_000.);
    assert_eq!(w.entity(crossing).unwrap().hp, 10_000.);
    let restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn a_ship_nearby_or_a_crossing_behind_the_weapon_is_not_a_global_junction_guard() {
    for (position, course, speed) in [
        (Vec2::new(0.15, 0.), 0., 20.), // Parallel and outside the explicit wake footprint.
        (Vec2::new(-0.9, -0.8), 90., 20.), // Crosses the old tail only after the weapon passed it.
    ] {
        let (mut w, s, t) = fixture(true, true, true);
        let crossing = w.allocate_id();
        let mut ship = w.entity(t).unwrap().clone();
        ship.id = crossing;
        ship.name = "NON_ENCOUNTERED_SHIP".into();
        ship.pos = position;
        ship.course_deg = course;
        ship.ordered_course_deg = course;
        ship.speed_kts = speed;
        ship.ordered_speed_kts = speed;
        w.entities.insert(crossing, ship);
        let _ = launch(&mut w, s);
        until(&mut w, "10.2.6.wake_terminal");
        assert!(events(&w, "7.5.5.wake_junction_pending").is_empty());
        assert!(w.rules.pending_adjudications.is_empty());
    }
}

#[test]
fn a_real_junction_ahead_waits_until_the_weapon_physically_reaches_it() {
    let (mut w, s, t) = fixture(true, true, true);
    let crossing = w.allocate_id();
    let mut ship = w.entity(t).unwrap().clone();
    ship.id = crossing;
    ship.name = "FUTURE_PATH_JUNCTION".into();
    ship.pos = Vec2::new(-0.05, 0.5);
    ship.course_deg = 90.;
    ship.ordered_course_deg = 90.;
    ship.speed_kts = 20.;
    ship.ordered_speed_kts = 20.;
    w.entities.insert(crossing, ship);
    let id = launch(&mut w, s);
    for _ in 0..60 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(
        w.rules.pending_adjudications.is_empty(),
        "another ship has crossed the tail, but this weapon has not reached that physical branch"
    );
    for _ in 0..200 {
        if w.rules.pending_adjudications.contains_key(&id) {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.rules.pending_adjudications.contains_key(&id));
    assert!(events(&w, "10.2.6.wake_terminal").is_empty());
}

#[test]
fn an_early_real_wake_crossing_cannot_roll_placement_or_home_before_half_distance() {
    let (mut w, s, _) = fixture(true, true, true);
    // Explicitly retain the old early geometry as the source regression case;
    // normal mechanism fixtures now cross only after the half-distance gate.
    w.entity_mut(s).unwrap().pos = Vec2::new(-0.4, -0.5);
    let id = launch(&mut w, s);
    for _ in 0..35 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(
        !events(&w, "10.2.6.wake_crossing").is_empty(),
        "the early encounter must be a real launch and geometric wake crossing"
    );
    assert!(
        events(&w, "10.2.6.wake_placement").is_empty(),
        "6.3.8/10.2.4 half-distance placement cannot be replaced by this earlier wake crossing"
    );
    let pending = w.rules.pending_adjudications.get(&id).expect(
        "the unresolved early-wake/placement combination must preserve the complete source flight",
    );
    assert!(pending.rule.contains("half") || pending.rule.contains("placement"));
    assert_eq!(pending.projectile.id, id);
    assert!(!pending.evidence.is_empty());
    assert!(events(&w, "10.2.6.wake_terminal").is_empty());
}

#[test]
fn late_wake_crossing_gets_placement_at_half_distance_before_it_reaches_the_wake() {
    let (mut w, s, _) = fixture(true, true, true);
    w.entity_mut(s).unwrap().pos = Vec2::new(-1., -0.5);
    let id = launch(&mut w, s);
    // Declared target range sqrt(1^2+1.5^2): half is0.9013878188659973nm.
    // A50kt run covers0.8888888888888888nm in64s and0.9027777777777778 in65s.
    while w.tick < 64 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(events(&w, "10.2.6.wake_placement").is_empty());
    assert!(events(&w, "10.2.6.wake_crossing").is_empty());
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(w.tick, 65);
    assert_eq!(events(&w, "10.2.6.wake_placement").len(), 1,
        "6.3.8/10.2.4 first continuous tick after half-distance rolls placement before a later wake crossing");
    assert!(events(&w, "10.2.6.wake_crossing").is_empty());
    assert!(
        matches!(
            w.rules.torpedoes.as_ref().unwrap().flights[&id].phase,
            harpoon_core::rules::torpedo2026::Phase::Run
        ),
        "a successful placement alone never grants direct homing without a physical wake encounter"
    );
}

#[test]
fn newly_detected_wake_threat_keeps_precommand_baseline_for_immediate_real_evasion() {
    let (mut w, s, t) = fixture(true, true, true);
    w.entity_mut(s).unwrap().pos = Vec2::new(-1., -0.5);
    let id = launch(&mut w, s);
    // This legal contact stands for the previous tick's completed detection.
    // The very next tick must allow immediate evasive commands, without an
    // artificial empty tick that lets the weapon photograph the old posture.
    mark_detected_torpedo(&mut w, t, id);
    let actuator = w.db.platforms.get_mut("cn_001").unwrap();
    actuator.turn_rate_deg_s = 360.;
    actuator.accel_kts_per_s = 100.;
    harpoon_core::sim::tick(
        &mut w,
        &[
            (
                Side::Red,
                Command::SetCourse {
                    unit: t,
                    course_deg: 90.,
                },
            ),
            (
                Side::Red,
                Command::SetSpeed {
                    unit: t,
                    speed_kts: 25.,
                },
            ),
        ],
    );
    let baseline = w.rules.torpedoes.as_ref().unwrap().flights[&id]
        .threat
        .as_ref()
        .expect("known detected torpedo must establish a defender's own precommand posture");
    assert_eq!(
        baseline.course, 0.,
        "the first reaction's new90deg course is not its detection baseline"
    );
    assert_eq!(baseline.speed, 0.);
    until(&mut w, "10.2.6.wake_terminal");
    assert_eq!(events(&w, "10.2.6.wake_terminal")[0]["probability"], 0.61);
}

#[test]
fn real_two_tube_wake_salvo_keeps_grouping_pending_without_secret_survival_feedback() {
    use harpoon_core::rules::torpedo2026::Tube;
    let (mut w, s, _) = fixture(true, true, true);
    let state = w.rules.torpedoes.as_mut().unwrap();
    let mut fit = state.tubes[&s].clone();
    fit.source = "scenario_assumption: exactly two declared650mm loaded tubes for placement grouping; no full Oscar fit claimed".into();
    fit.tubes.push(Tube {
        bearing_deg: 90.,
        loaded: Some("ru_65k".into()),
    });
    fit.tube_ids.insert("TEST650_SECOND".into(), 1);
    state
        .config
        .platforms
        .insert("ru_oscar2".into(), fit.clone());
    state.tubes.insert(s, fit);
    state
        .config
        .orders
        .get_mut("SHOOTER:ru_65k")
        .unwrap()
        .spread_deg = vec![0., 0.];
    let before = ammo(&w, s);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: s,
                contact: ContactId(91),
                weapon: "ru_65k".into(),
                rounds: 2,
            },
        )],
    );
    assert_eq!(ammo(&w, s), before - 2);
    let ids: Vec<_> = w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .copied()
        .collect();
    assert_eq!(
        ids.len(),
        2,
        "this must be a real two-tube release, not a manually fabricated flight"
    );
    assert!(w.rules.torpedoes.as_ref().unwrap().tubes[&s]
        .tubes
        .iter()
        .all(|tube| tube.loaded.is_none()));
    let mut hidden_loss = w.clone();
    hidden_loss.entity_mut(ids[0]).unwrap().alive = false;
    hidden_loss.entity_mut(ids[0]).unwrap().hp = 0.;
    while w.tick < 64 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut hidden_loss, &[]);
    }
    assert!(
        w.rules.pending_adjudications.is_empty(),
        "hold at the actual placement gate, not at release"
    );
    harpoon_core::sim::tick(&mut w, &[]);
    harpoon_core::sim::tick(&mut hidden_loss, &[]);
    for id in &ids {
        let pending = w.rules.pending_adjudications.get(id).expect(
            "unreconciled Wake same-unit/type/Tactical-Turn grouping must hold before guessing separate placement dice");
        assert!(pending.rule.contains("group") || pending.rule.contains("salvo"));
        assert_eq!(pending.projectile.id, *id);
    }
    assert!(
        hidden_loss
            .rules
            .pending_adjudications
            .contains_key(&ids[1]),
        "a secret retirement cannot make the surviving launch a different known placement cohort"
    );
    assert!(events(&w, "10.2.6.wake_placement").is_empty());
    assert!(events(&hidden_loss, "10.2.6.wake_placement").is_empty());
    assert_eq!(w.rules.torpedoes.as_ref().unwrap().launches.len(), 2);
    assert_eq!(
        hidden_loss.rules.torpedoes.as_ref().unwrap().launches.len(),
        2
    );
    assert_eq!(
        serde_json::to_value(harpoon_core::observe::observe(&w, Side::Blue, 0, 10_000)).unwrap(),
        serde_json::to_value(harpoon_core::observe::observe(&hidden_loss, Side::Blue, 0, 10_000)).unwrap(),
        "source release receipts and own knowledge cannot disclose hidden Wake survival or grouping results"
    );
}

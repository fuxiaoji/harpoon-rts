//! Real source Mk82 command path, English2026 PDF105-107, US Aircraft2025
//! B44/H1-3. Geometry/disabled AA are explicit controlled fixture conditions.
use harpoon_core::rules::aircombat2026::{self as ac, Action, Delivery};
use harpoon_core::{Command, ContactId, Database, EntityId, Scenario, Side, Vec2, World};
use serde_json::json;

fn fixture() -> (World, EntityId, EntityId) {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    ac::configure_database(&mut db);
    let sc:Scenario=serde_json::from_value(json!({"id":"mk82-source","name":"Mk82 source delivery acceptance","duration_s":1000,"seed":2,
        "context":{"rules_profile":"hv2026-rts-v4","aviation_2026":{"source":"scenario_assumption: controlled source strike","no_aa_surface_attack_source":"scenario_assumption: controlled unopposed bombing range; no defensive AA","surface_aimpoints":{"range":{"source":"scenario_assumption: exact known fixed source bombing range target coordinate","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}},"loads":{"us_f18e_mk82":{"payload_kg":6395,"tanks":[{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"}],"external_weapon_rounds":{"us_mk82":6},"source":"US2025 B44/H4-2/H1-3: 3DT+2AIM120D+2AIM9X2+6Mk82"}},"parking":{},"initial_fuel_nm":{"STRIKE":1000}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_f18e_mk82","name":"STRIKE","pos":[0,0],"course_deg":0,"speed_kts":460,"altitude_m":1000,"fire_on":false},
            {"side":"Red","class":"cn_001","name":"TARGET","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false}
        ]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let a = w.entities.values().find(|e| e.name == "STRIKE").unwrap().id;
    let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    w.entity_mut(t).unwrap().weapons.clear();
    w.sides[0].contacts.insert(
        ContactId(81),
        harpoon_core::world::Contact {
            id: ContactId(81),
            about: t,
            category: harpoon_core::types::PlatformKind::Ship,
            class_name: None,
            estimate: Vec2::zero(),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: 0,
            stale: false,
            sensors: vec!["visual".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", t.0),
        harpoon_core::types::PlatformKind::Ship,
    );
    (w, a, t)
}
fn attack(a: EntityId, delivery: Delivery) -> (Side, Command) {
    (
        Side::Blue,
        Command::AirCombat {
            unit: a,
            action: Action::SurfaceAttack {
                contact: ContactId(81),
                weapon: "us_mk82".into(),
                rounds: 6,
                delivery,
                aimpoint: "range".into(),
            },
        },
    )
}
fn release_detail(w: &World) -> serde_json::Value {
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
        rejected.is_empty(),
        "source bombing command rejected: {rejected:?}"
    );
    w.events
        .iter()
        .find_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent { rule, detail, .. }
                if rule == "9.6.source_bomb_attack" =>
            {
                Some(serde_json::from_str::<serde_json::Value>(detail).unwrap())
            }
            _ => None,
        })
        .expect("actual source bombing release event")
}

#[test]
fn real_glide_attack_splits_six_bombs_into_source_four_and_two_and_replays() {
    let (mut w, a, t) = fixture();
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for world in [&mut w, &mut saved] {
        harpoon_core::sim::tick(world, &[attack(a, Delivery::Glide)]);
    }
    let detail = release_detail(&w);
    assert_eq!(
        w.entity(a)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "us_mk82")
            .unwrap()
            .ammo,
        0
    );
    assert_eq!(detail["groups"][0]["salvo"], 4);
    assert_eq!(detail["groups"][1]["salvo"], 2);
    // Source A glide35%, stationary+2, Advanced+5, declared NoAA+1 ->75%.
    // Fixed source dice18 and36 hit all four and both bombs respectively.
    assert_eq!(detail["probability"], 0.75);
    assert_eq!(detail["groups"][0]["d100"], 18);
    assert_eq!(detail["groups"][1]["d100"], 36);
    assert_eq!(detail["total_hits"], 6);
    assert_eq!(detail["impact_penetration_cm"], 4.);
    let damage: Vec<_> = w
        .events
        .iter()
        .filter(|e| e.vis == harpoon_core::Visibility::Side(Side::Red))
        .filter_map(|e| match &e.kind {
            harpoon_core::EventKind::Impact {
                unit,
                weapon,
                damage,
                ..
            } if *unit == t && weapon == "Mk82 500lb LDGP" => Some(*damage),
            _ => None,
        })
        .collect();
    assert_eq!(
        damage,
        vec![35.; 6],
        "every actual source hit must enter normal damage resolution"
    );
    assert!(w.entity(t).unwrap().hp <= w.entity(t).unwrap().hp_max - 210.);
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&a].external_weapon_rounds["us_mk82"],
        0
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}
#[test]
fn real_level_stick_is_single_aircraft_salvo_one_and_temporarily_mr_zero() {
    let (mut w, a, _) = fixture();
    harpoon_core::sim::tick(&mut w, &[attack(a, Delivery::Level)]);
    let detail = release_detail(&w);
    assert_eq!(detail["groups"].as_array().unwrap().len(), 1);
    assert_eq!(detail["groups"][0]["salvo"], 1);
    assert_eq!(ac::target_maneuver(&w, a).unwrap(), 0.);
    assert!(detail["total_hits"].as_u64().unwrap() <= 1);
    w.tick = 31;
    assert_eq!(ac::target_maneuver(&w, a).unwrap(), 1.5);
}
#[test]
fn unknown_fit_wrong_geometry_undeclared_aa_and_vlow_do_not_consume_bombs() {
    for case in [0, 1, 2, 3, 5, 6, 7, 8, 9, 10] {
        let (mut w, a, _) = fixture();
        match case {
            0 => {
                w.entity_mut(a).unwrap().class = "us_f18e".into();
            }
            1 => {
                w.entity_mut(a).unwrap().pos = Vec2::new(0., 1.);
            }
            2 => {
                w.entity_mut(a).unwrap().altitude_m = 50.;
            }
            3 => {
                w.context
                    .aviation_2026
                    .as_mut()
                    .unwrap()
                    .no_aa_surface_attack_source = None;
            }
            5 => {
                w.context
                    .aviation_2026
                    .as_mut()
                    .unwrap()
                    .surface_aimpoints
                    .clear();
            }
            6 => {
                w.entity_mut(a).unwrap().pos.x = f64::NAN;
            }
            7 => {
                w.rules.aviation.as_mut().unwrap().flights.remove(&a);
            }
            8 => {
                w.rules
                    .aviation
                    .as_mut()
                    .unwrap()
                    .flights
                    .get_mut(&a)
                    .unwrap()
                    .external_weapon_rounds
                    .clear();
            }
            9 => {
                w.context
                    .aviation_2026
                    .as_mut()
                    .unwrap()
                    .surface_aimpoints
                    .get_mut("range")
                    .unwrap()
                    .controlled_range = false;
            }
            _ => {
                w.context
                    .aviation_2026
                    .as_mut()
                    .unwrap()
                    .surface_aimpoints
                    .get_mut("range")
                    .unwrap()
                    .fixed_target = false;
            }
        }
        let before = w
            .entity(a)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "us_mk82")
            .unwrap()
            .ammo;
        assert!(ac::surface_attack(
            &mut w,
            a,
            ContactId(81),
            "us_mk82",
            6,
            Delivery::Glide,
            "range"
        )
        .is_err());
        assert_eq!(
            w.entity(a)
                .unwrap()
                .weapons
                .iter()
                .find(|m| m.weapon == "us_mk82")
                .unwrap()
                .ammo,
            before
        );
    }
}
#[test]
fn missing_no_aa_declaration_rejects_independently_of_hidden_weapon_inventory() {
    let (mut empty, a, t) = fixture();
    empty
        .context
        .aviation_2026
        .as_mut()
        .unwrap()
        .no_aa_surface_attack_source = None;
    let mut armed = empty.clone();
    armed
        .entity_mut(t)
        .unwrap()
        .weapons
        .push(harpoon_core::world::WeaponMount {
            weapon: "unknown-aa".into(),
            mounts: 1,
            ammo: 10,
            cooldown_s: 0.,
        });
    let first = ac::surface_attack(
        &mut empty,
        a,
        ContactId(81),
        "us_mk82",
        6,
        Delivery::Glide,
        "range",
    )
    .unwrap_err();
    let second = ac::surface_attack(
        &mut armed,
        a,
        ContactId(81),
        "us_mk82",
        6,
        Delivery::Glide,
        "range",
    )
    .unwrap_err();
    assert_eq!(first, second);
    assert!(first.contains("scenario NoAA declaration"));
}
#[test]
fn hidden_position_does_not_change_release_receipts_or_disclose_referee_adjudication() {
    let (mut known, a, t) = fixture();
    let mut displaced = known.clone();
    displaced.entity_mut(t).unwrap().pos = Vec2::new(20., 0.);
    for world in [&mut known, &mut displaced] {
        harpoon_core::sim::tick(world, &[attack(a, Delivery::Glide)]);
    }
    let receipts = |w: &World| {
        w.events
            .iter()
            .filter_map(|e| match &e.kind {
                harpoon_core::EventKind::RulesEvent { domain, rule, .. }
                    if domain == "command" || rule == "9.6.bombs_released" =>
                {
                    Some(serde_json::to_value(&e.kind).unwrap())
                }
                harpoon_core::EventKind::WeaponsRelease { side, .. } if *side == Side::Blue => {
                    Some(serde_json::to_value(&e.kind).unwrap())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(receipts(&known), receipts(&displaced));
    assert_eq!(
        displaced
            .entity(a)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "us_mk82")
            .unwrap()
            .ammo,
        0
    );
    assert_eq!(
        displaced.entity(t).unwrap().hp,
        displaced.entity(t).unwrap().hp_max,
        "unknown placement outside the declared source range must not invent damage"
    );
    assert_eq!(
        displaced
            .rules
            .aircombat
            .as_ref()
            .unwrap()
            .pending_surface_attacks
            .len(),
        1
    );
    for world in [&known, &displaced] {
        let raw=world.events.iter().find(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent {rule,..} if rule=="9.6.source_bomb_attack")).unwrap();
        assert_eq!(raw.vis, harpoon_core::Visibility::Referee);
        for side in [Side::Blue, Side::Red] {
            let obs = harpoon_core::observe(world, side, 0, 1000);
            assert!(!obs.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent {rule,..} if rule=="9.6.source_bomb_attack")));
            for e in obs.events {
                if let harpoon_core::EventKind::RulesEvent { rule, detail, .. } = e.kind {
                    if rule == "9.6.bombs_released" {
                        let value: serde_json::Value = serde_json::from_str(&detail).unwrap();
                        for hidden in [
                            "probability",
                            "groups",
                            "d100",
                            "total_hits",
                            "target_size",
                            "target_horizontal_armor_cm",
                            "impact_penetration_cm",
                            "target_position",
                        ] {
                            assert!(value.get(hidden).is_none());
                        }
                    }
                }
            }
        }
    }
    let mut restored: World =
        serde_json::from_slice(&serde_json::to_vec(&displaced).unwrap()).unwrap();
    for _ in 0..2 {
        harpoon_core::sim::tick(&mut displaced, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_value(&displaced).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert_eq!(
        restored
            .rules
            .aircombat
            .as_ref()
            .unwrap()
            .pending_surface_attacks
            .len(),
        1
    );
}
#[test]
fn new_variant_operations_and_radar_data_are_visible_only_in_v4() {
    let (mut w, a, _) = fixture();
    use harpoon_core::rules::{aviation2026 as av, damage_runtime::RuleProfile, operations};
    assert!(operations::tables()["aviation"]["aircraft"]
        .get("us_f18e_mk82")
        .is_none());
    assert!(av::operations_aircraft(&w, "us_f18e_mk82").is_some());
    let sensor = w.db.sensors.get("radar_air_med").unwrap();
    assert!(av::operations_radar_profile(&w, w.entity(a).unwrap(), sensor).is_some());
    w.rules.profile = RuleProfile::Rts2026Operations;
    assert!(av::operations_aircraft(&w, "us_f18e_mk82").is_none());
    assert_eq!(
        av::operations_radar_profile(&w, w.entity(a).unwrap(), sensor),
        operations::radar_profile(w.entity(a).unwrap(), sensor)
    );
}
#[test]
fn real_variant_launch_recovery_and_unguided_rearming_keep_one_airframe_and_resume() {
    let (template, _, _) = fixture();
    let mut config = template.context.aviation_2026.clone().unwrap();
    config.parking.insert("CV".into(), 0);
    let sc:Scenario=serde_json::from_value(json!({"id":"mk82-deck-source","name":"Mk82 exact fit deck lifecycle","duration_s":20000,"seed":2,
        "context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":3,"precipitation":"None"},"air_wings":{"CV":[{"class":"us_f18e_mk82","count":1}]},"aviation_2026":config},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"us_ford","name":"CV","pos":[0,0],"speed_kts":32,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, template.db).unwrap();
    let cv = w.entities.values().find(|e| e.name == "CV").unwrap().id;
    let launch = Command::LaunchAircraft {
        unit: cv,
        class: "us_f18e_mk82".into(),
        count: 1,
        mission: harpoon_core::AirMission::Recon {
            point: Vec2::new(0., 20.),
        },
        profile: None,
    };
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, launch.clone())]);
    let queues = &w.rules.operations.as_ref().unwrap().launches;
    assert_eq!(
        queues.len(),
        1,
        "verified v4 unguided fit must enter actual launch queue; events={:?}",
        w.events
    );
    let due = queues[0].due;
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for world in [&mut w, &mut saved] {
        world.tick = due - 1;
        harpoon_core::sim::tick(world, &[]);
    }
    let plane = w
        .entities
        .values()
        .find(|e| e.alive && e.kind == harpoon_core::types::PlatformKind::Aircraft)
        .unwrap()
        .id;
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&plane].external_weapon_rounds["us_mk82"],
        6
    );
    for world in [&mut w, &mut saved] {
        // Controlled landing envelope; the real simulator owns reservation,
        // recovery completion, individual identity and source servicing clock.
        let pos = world.entity(cv).unwrap().pos;
        let course = world.entity(cv).unwrap().course_deg;
        let e = world.entity_mut(plane).unwrap();
        e.pos = pos;
        e.course_deg = course;
        e.ordered_course_deg = course;
        e.speed_kts = 585. * 0.25;
        e.ordered_speed_kts = e.speed_kts;
        e.altitude_m = 1000.;
        e.ordered_altitude_m = 1000.;
        e.mission = Some(harpoon_core::world::MissionState::Rtb);
        world
            .rules
            .aviation
            .as_mut()
            .unwrap()
            .flights
            .get_mut(&plane)
            .unwrap()
            .last_position = pos;
        harpoon_core::sim::tick(world, &[]);
    }
    assert!(!w
        .rules
        .aviation
        .as_ref()
        .unwrap()
        .flights
        .contains_key(&plane));
    let frame = w
        .rules
        .aviation
        .as_ref()
        .unwrap()
        .airframes
        .values()
        .find(|a| a.class == "us_f18e_mk82")
        .unwrap();
    assert_eq!(frame.sorties.len(), 1);
    assert_eq!(frame.sorties[0].1, Some(w.tick));
    // Source Small: unguided arm30min exceeds fuel20min, plus alert40min.
    let ready = frame.ready_at.unwrap();
    assert_eq!(ready, w.tick + 4200);
    for world in [&mut w, &mut saved] {
        world.tick = ready - 1;
        harpoon_core::sim::tick(world, &[]);
    }
    assert_eq!(
        w.entity(cv)
            .unwrap()
            .deck
            .as_ref()
            .unwrap()
            .ready
            .iter()
            .find(|(c, _)| c == "us_f18e_mk82")
            .unwrap()
            .1,
        1
    );
    assert!(harpoon_core::rules::aviation2026::launch_gate(&w, cv, "us_f18e_mk82", 1).is_ok());
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}

// Own command admission and Side-allocation source regressions.
fn declare_range_sides(w: &mut World, sides: &[&str]) {
    // Exercise the saved scenario-author Side allocation schema.
    let mut c = serde_json::to_value(w.context.aviation_2026.as_ref().unwrap()).unwrap();
    c["surface_aimpoints"]["range"]["allowed_sides"] = json!(sides);
    w.context.aviation_2026 = Some(serde_json::from_value(c).unwrap());
}

fn own_release_receipts(w: &World) -> Vec<serde_json::Value> {
    w.events
        .iter()
        .filter(|e| e.vis == harpoon_core::Visibility::Side(Side::Blue))
        .filter_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent { domain, rule, .. }
                if domain == "command" || rule == "9.6.bombs_released" =>
            {
                Some(serde_json::to_value(&e.kind).unwrap())
            }
            harpoon_core::EventKind::WeaponsRelease {
                side: Side::Blue, ..
            } => Some(serde_json::to_value(&e.kind).unwrap()),
            _ => None,
        })
        .collect()
}

fn bomb_ammo(w: &World, a: EntityId) -> u16 {
    w.entity(a)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_mk82")
        .unwrap()
        .ammo
}

#[test]
fn hidden_missing_target_size_does_not_change_valid_own_release_or_command_feedback() {
    let (mut supported, a, target) = fixture();
    declare_range_sides(&mut supported, &["Blue"]);
    let mut unsupported = supported.clone();
    let t = unsupported.entity_mut(target).unwrap();
    t.class = "cn_052d".into();
    t.rule_data = None;
    // Same Blue contact has no known class in both worlds; only referee
    // metadata differs. No guessed size may be applied to this world.
    for world in [&mut supported, &mut unsupported] {
        harpoon_core::sim::tick(world, &[attack(a, Delivery::Glide)]);
    }
    assert_eq!(
        own_release_receipts(&supported),
        own_release_receipts(&unsupported),
        "hidden source coverage must not be an action-approval oracle"
    );
    assert_eq!(bomb_ammo(&supported, a), 0);
    assert_eq!(
        bomb_ammo(&unsupported, a),
        0,
        "valid releases consume stores before unresolved referee adjudication"
    );
    assert_eq!(
        unsupported.entity(target).unwrap().hp,
        unsupported.entity(target).unwrap().hp_max
    );
    let pending = unsupported
        .rules
        .aircombat
        .as_ref()
        .unwrap()
        .pending_surface_attacks
        .values()
        .next()
        .unwrap();
    assert_eq!(pending["resolution"], "pending_source_target_properties");
    for unknown in ["probability", "groups", "total_hits"] {
        assert!(
            pending.get(unknown).is_none_or(serde_json::Value::is_null),
            "must not invent {unknown}"
        );
    }
    let resumed: World =
        serde_json::from_slice(&serde_json::to_vec(&unsupported).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&unsupported).unwrap(),
        serde_json::to_value(&resumed).unwrap()
    );
}

fn ground_property_fixture(w: &mut World, target: EntityId, armor: f64) {
    use harpoon_core::types::PlatformKind;
    w.entity_mut(target).unwrap().kind = PlatformKind::Base;
    w.sides[0]
        .contacts
        .get_mut(&ContactId(81))
        .unwrap()
        .category = PlatformKind::Base;
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert(format!("0:{}", target.0), PlatformKind::Base);
    w.context.ground_2026 = Some(serde_json::from_value(json!({
        "source": "scenario_assumption: two hidden facility-property worlds for disclosure boundary only",
        "facilities": {"TARGET": {"template": null,
            "custom": {"size":"A", "damage_points":1010, "vertical_armor":0,
                "horizontal_armor":armor, "effect":"controlled_boundary_fixture",
                "source":"scenario_assumption: hidden referee facility properties, not player knowledge"},
            "source":"scenario_assumption: controlled boundary fixture", "parent":null}}
    })).unwrap());
    harpoon_core::rules::ground2026::initialize(w).unwrap();
}

#[test]
fn hidden_protected_facility_does_not_change_release_but_stays_pending_without_guessed_damage() {
    let (mut unarmored, a, target) = fixture();
    declare_range_sides(&mut unarmored, &["Blue"]);
    ground_property_fixture(&mut unarmored, target, 0.);
    let mut protected = unarmored.clone();
    // The source-bound target data is hidden. Blue still knows only category
    // Base at the same authorized coordinate in both worlds.
    protected
        .rules
        .ground
        .as_mut()
        .unwrap()
        .facilities
        .get_mut(&target)
        .unwrap()
        .target
        .horizontal_armor = 10.;
    for world in [&mut unarmored, &mut protected] {
        harpoon_core::sim::tick(world, &[attack(a, Delivery::Glide)]);
    }
    assert_eq!(
        own_release_receipts(&unarmored),
        own_release_receipts(&protected)
    );
    assert_eq!(bomb_ammo(&unarmored, a), 0);
    assert_eq!(bomb_ammo(&protected, a), 0);
    assert_eq!(
        protected.entity(target).unwrap().hp,
        protected.entity(target).unwrap().hp_max
    );
    assert_eq!(
        protected
            .rules
            .aircombat
            .as_ref()
            .unwrap()
            .pending_surface_attacks
            .len(),
        1
    );
    assert!(
        protected.events.iter().all(
            |e| !matches!(e.kind, harpoon_core::EventKind::Impact { unit, .. } if unit == target)
        ),
        "unverified protected critical mapping must not manufacture damage"
    );
    let before = serde_json::to_value(&protected).unwrap();
    let resumed: World = serde_json::from_value(before.clone()).unwrap();
    assert_eq!(before, serde_json::to_value(resumed).unwrap());
}

#[test]
fn guessed_aimpoint_key_is_rejected_unless_explicitly_authorized_for_actor_side() {
    let (mut w, a, _) = fixture();
    declare_range_sides(&mut w, &["Red"]);
    harpoon_core::sim::tick(&mut w, &[attack(a, Delivery::Glide)]);
    assert_eq!(
        bomb_ammo(&w, a),
        6,
        "an opponent-only source point is not this pilot's authorization"
    );
    assert_eq!(w.stats.projectiles_launched, 0);
    assert!(w.events.iter().any(|e| matches!(&e.kind,
        harpoon_core::EventKind::RulesEvent { domain, rule, .. } if domain == "command" && rule == "rejected")));
    assert!(!w.events.iter().any(|e| matches!(&e.kind,
        harpoon_core::EventKind::RulesEvent { rule, .. } if rule == "9.6.bombs_released")));
}

#[test]
fn unauthorized_or_unknown_aimpoint_does_not_disclose_global_noaa_declaration() {
    let (mut template, a, _) = fixture();
    declare_range_sides(&mut template, &["Red"]);
    let mut feedback = vec![];
    for key in ["range", "unknown-key"] {
        for has_noaa in [true, false] {
            let mut w = template.clone();
            if !has_noaa {
                w.context
                    .aviation_2026
                    .as_mut()
                    .unwrap()
                    .no_aa_surface_attack_source = None;
            }
            let obs = harpoon_core::observe(&w, Side::Blue, 0, 1000);
            let points = &obs
                .units
                .iter()
                .find(|u| u.id == a)
                .unwrap()
                .aviation
                .as_ref()
                .unwrap()["surface_attack_authorizations"];
            assert!(
                points.as_array().unwrap().is_empty(),
                "Blue cannot discover a Red-only point or its NoAA source"
            );
            harpoon_core::sim::tick(
                &mut w,
                &[(
                    Side::Blue,
                    Command::AirCombat {
                        unit: a,
                        action: Action::SurfaceAttack {
                            contact: ContactId(81),
                            weapon: "us_mk82".into(),
                            rounds: 6,
                            delivery: Delivery::Glide,
                            aimpoint: key.into(),
                        },
                    },
                )],
            );
            assert_eq!(bomb_ammo(&w, a), 6);
            feedback.push(own_release_receipts(&w));
        }
    }
    assert!(
        feedback.windows(2).all(|pair| pair[0] == pair[1]),
        "unallocated point requests must be rejected before consulting globally declared NoAA"
    );
}

#[test]
fn own_observation_discovers_only_side_authorized_source_aimpoints_and_resumes() {
    let (mut w, a, _) = fixture();
    declare_range_sides(&mut w, &["Blue"]);
    let mut red = w.entity(a).unwrap().clone();
    red.id = w.allocate_id();
    red.name = "RED-PLANE".into();
    red.side = Side::Red;
    let red_id = red.id;
    w.entities.insert(red_id, red);
    // This additional point is Red's predeclared source information, not a
    // hidden enemy entity. Neither side may discover the other's point key.
    let mut cfg = serde_json::to_value(w.context.aviation_2026.as_ref().unwrap()).unwrap();
    cfg["surface_aimpoints"]["red-only"] = json!({
        "source":"scenario_assumption: Red-only fixed range coordinate", "point":{"x":90,"y":90},
        "controlled_range":true, "fixed_target":true, "allowed_sides":["Red"]
    });
    w.context.aviation_2026 = Some(serde_json::from_value(cfg).unwrap());
    let resumed: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for world in [&w, &resumed] {
        for (side, actor, expected, hidden) in [
            (Side::Blue, a, "range", "red-only"),
            (Side::Red, red_id, "red-only", "range"),
        ] {
            let obs = harpoon_core::observe(world, side, 0, 1000);
            let aviation = obs
                .units
                .iter()
                .find(|u| u.id == actor)
                .unwrap()
                .aviation
                .as_ref()
                .unwrap();
            let points = aviation["surface_attack_authorizations"]
                .as_array()
                .expect("own observation must advertise legal source action parameters");
            assert!(points.iter().any(|p| p["aimpoint"] == expected));
            assert!(!points.iter().any(|p| p["aimpoint"] == hidden));
            for p in points {
                for truth in [
                    "target",
                    "target_size",
                    "target_horizontal_armor_cm",
                    "target_position",
                ] {
                    assert!(
                        p.get(truth).is_none(),
                        "authorization cannot contain referee field {truth}"
                    );
                }
            }
        }
    }
}

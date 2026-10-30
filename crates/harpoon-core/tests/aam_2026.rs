use harpoon_core::rules::aam2026::*;
use harpoon_core::{ContactId, Database, EntityId, Scenario, Side, Vec2, World};
#[test]
fn source_aam_range_stack() {
    assert_eq!(
        range_nm(40.0, 900.0, 10000.0, 1200.0, 0.0, true, false).unwrap(),
        120.0
    );
    assert_eq!(
        range_nm(40.0, 300.0, 1000.0, 400.0, 180.0, false, false).unwrap(),
        5.0
    );
    assert_eq!(
        range_nm(10., 100., 0., 400., 180., true, true).unwrap(),
        10.
    );
}
#[test]
fn official_f4_mig23_example_has_no_sam_range_or_bearing_modifiers() {
    let a = Attack {
        ata: 3.,
        mr: 3.,
        maneuvering: true,
        speed: 710.,
        signature: 2,
        radar_generation: 3,
        snap_penalty: false,
        dogfight_unrated: false,
        countermeasures: harpoon_core::rules::aaw2026::Countermeasures {
            seeker_generation: 3,
            jammer_generation: None,
            decoy_generation: Some(2),
        },
        hoj: false,
    };
    assert_eq!(solution(&a, 1).unwrap().value, 0.5);
    assert_eq!(solution(&a, 1).unwrap().probability, 0.35);
    assert_eq!(solution(&a, 2).unwrap().probability, 0.58);
    let mut a = a;
    a.hoj = true;
    assert_eq!(solution(&a, 1).unwrap().value, -0.5);
}
fn fixture() -> (World, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc:Scenario=serde_json::from_value(serde_json::json!({"id":"aam-source","name":"AAM source","duration_s":1000,"seed":20261023,"context":{"rules_profile":"hv2026-rts-v4","aviation_2026":{"source":"scenario_assumption: AAM acceptance controlled geometry","loads":{"cn_j15":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"},"us_f18e":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"}},"parking":{},"initial_fuel_nm":{"BLUE":700,"RED":700}}},"weather":{"visibility_nm":40,"jam":[0,0]},"units":[{"side":"Blue","class":"us_f18e","name":"BLUE","pos":[0,0],"course_deg":0,"speed_kts":600,"altitude_m":4000,"fire_on":false},{"side":"Red","class":"cn_j15","name":"RED","pos":[0,8],"course_deg":180,"speed_kts":600,"altitude_m":4000,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    w.rules.detection.as_mut().unwrap().config.environment =
        Some(harpoon_core::rules::detection2026::Environment {
            source: "scenario_assumption: clear AAM fixture, no clouds or precipitation".into(),
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
    let b = w
        .entities
        .values()
        .find(|e| e.side == Side::Blue)
        .unwrap()
        .id;
    let r = w
        .entities
        .values()
        .find(|e| e.side == Side::Red)
        .unwrap()
        .id;
    for (side, t) in [(Side::Blue, r), (Side::Red, b)] {
        let pos = w.entity(t).unwrap().pos;
        w.sides[side.index()].contacts.insert(
            ContactId(501),
            harpoon_core::world::Contact {
                id: ContactId(501),
                about: t,
                category: harpoon_core::types::PlatformKind::Aircraft,
                class_name: None,
                estimate: pos,
                est_course_deg: 0.,
                est_speed_kts: 600.,
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
        w.rules.detection.as_mut().unwrap().categories.insert(
            format!("{}:{}", side.index(), t.0),
            harpoon_core::types::PlatformKind::Aircraft,
        );
    }
    (w, b, r)
}
#[test]
fn actual_pair_uses_one_d100_exact_ammo_and_serializes() {
    let (mut w, b, r) = fixture();
    let mi = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_aim120d")
        .unwrap();
    let before = w.entity(b).unwrap().weapons[mi].ammo;
    assert_eq!(fire_at(&mut w, b, r, mi, 2).unwrap(), 2);
    let launch_detail = w
        .events
        .iter()
        .find_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent { rule, detail, .. }
                if rule == "9.5.source_launch" =>
            {
                Some(serde_json::from_str::<serde_json::Value>(detail).unwrap())
            }
            _ => None,
        })
        .unwrap();
    let public_identity = launch_detail["identity"].as_str().unwrap();
    assert_eq!(public_identity.len(), 20);
    assert!(
        public_identity.starts_with("aam:")
            && public_identity[4..].chars().all(|c| c.is_ascii_hexdigit())
    );
    assert!(launch_detail.get("target").is_none());
    assert_eq!(w.entity(b).unwrap().weapons[mi].ammo, before - 2);
    assert!(fire_at(&mut w, b, r, mi, 2).unwrap_err().contains("empty"));
    let other_mount = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_aim9x2")
        .unwrap();
    assert!(fire_at(&mut w, b, r, other_mount, 2)
        .unwrap_err()
        .contains("one-minute"));
    let ids: Vec<_> = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .aam
        .packets
        .keys()
        .copied()
        .collect();
    assert_eq!(ids.len(), 2);
    let mut st = w.rules.aaw.take().unwrap();
    let one = terminal(&w, &mut st.aam, ids[0]).unwrap();
    let two = terminal(&w, &mut st.aam, ids[1]).unwrap();
    assert_eq!(one.d100, two.d100);
    assert_eq!(st.aam.outcomes.len(), 1);
    assert_eq!(one.solution.modifiers["maneuver_rating"], -4.5);
    assert_eq!(one.solution.modifiers["air_intercept_radar"], 3.);
    assert_eq!(one.solution.modifiers["speed"], 0.);
    assert_eq!(one.solution.modifiers["signature"], 0.);
    assert_eq!(one.solution.modifiers["countermeasures"], 0.);
    // H4 AIM-120D4.0 - lightly-loaded J15 MR4.5 + sixth-gen AI3.0.
    assert_eq!(one.solution.value, 2.5);
    assert_eq!(one.probability, 0.80);
    w.rules.aaw = Some(st);
    let save = serde_json::to_string(&w).unwrap();
    let restored: World = serde_json::from_str(&save).unwrap();
    assert_eq!(save, serde_json::to_string(&restored).unwrap());
}
#[test]
fn mutual_same_tick_packets_precede_evasion_then_prevent_new_release() {
    let (mut w, b, r) = fixture();
    let bm = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_aim120d")
        .unwrap();
    let rm = w
        .entity(r)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "cn_pl12")
        .unwrap();
    assert_eq!(fire_at(&mut w, b, r, bm, 2).unwrap(), 2);
    assert_eq!(fire_at(&mut w, r, b, rm, 2).unwrap(), 2);
    after_fire_scan(&mut w);
    assert!(harpoon_core::rules::aircombat2026::controlling_missiles_stupid(&w, b));
    assert!(harpoon_core::rules::aircombat2026::controlling_missiles_stupid(&w, r));
    w.tick += 60;
    assert!(fire_at(&mut w, b, r, bm, 2)
        .unwrap_err()
        .contains("prohibited"));
    let ids: Vec<_> = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .aam
        .packets
        .keys()
        .copied()
        .collect();
    let mut st = w.rules.aaw.take().unwrap();
    assert!(terminal(&w, &mut st.aam, ids[0]).unwrap().probability > 0.);
    w.rules.aaw = Some(st);
}
#[test]
fn actual_sim_tick_arrives_one_packet_and_replays_identically() {
    let (mut w, b, r) = fixture();
    let mi = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_aim120d")
        .unwrap();
    fire_at(&mut w, b, r, mi, 2).unwrap();
    let mut restored: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    for _ in 0..20 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_string(&w).unwrap(),
        serde_json::to_string(&restored).unwrap()
    );
    let events:Vec<_>=w.events.iter().filter(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..}if rule=="9.5.packet_terminal")).collect();
    assert_eq!(events.len(), 1);
    assert_eq!(w.rules.aaw.as_ref().unwrap().aam.outcomes.len(), 1);
    assert!(w.rules.pending_adjudications.is_empty());
    assert_eq!(
        w.entity(r).unwrap().chaff,
        restored.entity(r).unwrap().chaff
    );
}

#[test]
fn source_non_tarh_controller_evasion_is_permanent_after_threat_passes() {
    let (mut w, b, r) = fixture();
    let mi = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_aim9x2")
        .unwrap();
    fire_at(&mut w, b, r, mi, 1).unwrap();
    let id = *w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .aam
        .packets
        .keys()
        .next()
        .unwrap();
    // An actual inbound missile engages the controller before its own IR missile arrives.
    let def = w.db.weapons["cn_pl12"].clone();
    let inbound = harpoon_core::weapons::launch(&mut w, r, &def, Some(b), None).unwrap();
    harpoon_core::rules::aircombat2026::begin_aam_evasion(&mut w, b, inbound);
    w.entity_mut(inbound).unwrap().alive = false;
    assert!(!harpoon_core::rules::aircombat2026::controlling_missiles_stupid(&w, b));
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for state in [&mut w, &mut restored] {
        let mut aaw = state.rules.aaw.take().unwrap();
        let result = terminal(state, &mut aaw.aam, id).unwrap();
        assert_eq!(
            result.probability, 0.,
            "9.5.1 says will miss; evasion cannot restore guidance later"
        );
        state.rules.aaw = Some(aaw);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn source_dogfight_aam_uses_cruise_speed_and_does_not_trigger_outside_evasion() {
    let (mut w, b, r) = fixture();
    w.entity_mut(r).unwrap().pos = Vec2::new(0., 2.);
    harpoon_core::rules::aircombat2026::apply(
        &mut w,
        b,
        &harpoon_core::rules::aircombat2026::Action::Declare {
            contact: ContactId(501),
        },
    )
    .unwrap();
    w.rules.aircombat.as_mut().unwrap().positions.insert(
        b,
        harpoon_core::rules::aircombat2026::Position {
            gun: 90,
            aam: 90,
            round: 1,
        },
    );
    let mi = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_aim120d")
        .unwrap();
    fire_at(&mut w, b, r, mi, 1).unwrap();
    let attack = &w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .aam
        .packets
        .values()
        .next()
        .unwrap()
        .attack;
    assert_eq!(
        attack.speed, 485.,
        "9.4 uses target cruise speed, even though exact speed changes inside the fight"
    );
    after_fire_scan(&mut w);
    assert!(
        !harpoon_core::rules::aircombat2026::controlling_missiles_stupid(&w, r),
        "9.5.1 outside-dogfight restrictions must not prohibit the opponent's normal dogfight turn"
    );
}

fn night_duel() -> (World, EntityId, EntityId) {
    let (mut w, b, r) = fixture();
    w.day_night = harpoon_core::types::DayNight::Night;
    w.entity_mut(r).unwrap().pos = Vec2::new(0., 2.);
    harpoon_core::rules::aircombat2026::apply(
        &mut w,
        b,
        &harpoon_core::rules::aircombat2026::Action::Declare {
            contact: ContactId(501),
        },
    )
    .unwrap();
    w.rules.aircombat.as_mut().unwrap().positions.insert(
        b,
        harpoon_core::rules::aircombat2026::Position {
            gun: 90,
            aam: 90,
            round: 1,
        },
    );
    (w, b, r)
}
fn night_fire(w: &mut World, b: EntityId) {
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            harpoon_core::Command::Fire {
                unit: b,
                contact: ContactId(501),
                weapon: "us_aim9x2".into(),
                rounds: 1,
            },
        )],
    );
}
fn ir_ammo(w: &World, b: EntityId) -> u16 {
    w.entity(b)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_aim9x2")
        .unwrap()
        .ammo
}
fn local_track(w: &mut World, observer: EntityId, sensor: &str, target: EntityId) {
    let last = w.tick;
    w.rules.aaw.as_mut().unwrap().firecontrol.tracks.insert(
        format!("{}:{sensor}:{}", observer.0, target.0),
        harpoon_core::rules::firecontrol2026::Track {
            observer,
            target,
            sensor: sensor.into(),
            first: last,
            last,
            turn_course: 0.,
            turn_speed: 0.,
            last_turn: last.div_euclid(180),
            zig_until: 0,
            report: None,
            motion: None,
        },
    );
}
fn install_test_ir(w: &mut World, b: EntityId, mode: harpoon_core::rules::detection2026::IrMode) {
    let mut def =
        w.db.sensors
            .values()
            .find(|d| d.kind == harpoon_core::db::SensorKind::Ir)
            .unwrap()
            .clone();
    def.id = "night_test_ir".into();
    w.db.sensors.insert(def.id.clone(), def);
    w.entity_mut(b)
        .unwrap()
        .sensors
        .push(harpoon_core::world::SensorInstance {
            sensor: "night_test_ir".into(),
            active: false,
        });
    let class = w.entity(b).unwrap().class.clone();
    w.rules.detection.as_mut().unwrap().config.bindings.insert(format!("{class}:night_test_ir"),harpoon_core::rules::detection2026::Receiver::Ir {source:"scenario_assumption: source night-rule receiver qualification fixture, not an asserted F/A18 equipment fit".into(),generation:3,mode,arc_deg:360.});
}
#[test]
fn real_night_dogfight_rejects_shared_visual_and_flir_evidence_without_spending_ammo() {
    for case in 0..4 {
        let (mut w, b, r) = night_duel();
        match case {
            0 => {} // Only the shared classified contact book exists.
            1 => {
                local_track(&mut w, EntityId(999), "radar_air_med", r);
            }
            2 => {
                local_track(&mut w, b, "visual", r);
            }
            _ => {
                install_test_ir(&mut w, b, harpoon_core::rules::detection2026::IrMode::Flir);
                local_track(&mut w, b, "night_test_ir", r);
            }
        }
        let before = ir_ammo(&w, b);
        night_fire(&mut w, b);
        assert_eq!(
            w.stats.projectiles_launched, 0,
            "9.4.8 MR>0.5 at night requires the shooter's own radar/IRST detection, case{case}"
        );
        assert_eq!(ir_ammo(&w, b), before);
    }
}
#[test]
fn real_night_dogfight_uses_actual_onboard_radar_and_rejects_stale_or_inoperative_receiver() {
    let (mut w, b, r) = night_duel();
    w.context.radar_environment = Some(harpoon_core::rules::electronic_warfare::RadarEnvironment {
        sea_state: 3,
        precipitation: "None".into(),
    });
    harpoon_core::sensors::update_sensors(&mut w);
    assert!(w.rules.aaw.as_ref().unwrap().firecontrol.tracks.values().any(|t|t.observer==b&&t.target==r&&t.sensor=="radar_air_med"),"actual successful scan must retain this observer's provenance, even when fused contact already existed");
    for case in 0..4 {
        let mut state = w.clone();
        match case {
            0 => {}
            1 => {
                state.tick += 2;
            }
            2 => {
                state.entity_mut(b).unwrap().systems.radar = 0.;
            }
            _ => {
                state
                    .entity_mut(b)
                    .unwrap()
                    .ext
                    .insert("emission.radar".into(), 0.);
            }
        }
        let before = ir_ammo(&state, b);
        let mut resumed: World =
            serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        night_fire(&mut state, b);
        night_fire(&mut resumed, b);
        assert_eq!(
            state.stats.projectiles_launched,
            u64::from(case == 0),
            "only the current operational onboard scan qualifies, case{case}"
        );
        assert_eq!(ir_ammo(&state, b), before - u16::from(case == 0));
        assert_eq!(
            serde_json::to_value(&state).unwrap(),
            serde_json::to_value(&resumed).unwrap(),
            "saved observer evidence must preserve both accepted and rejected night-fire decisions"
        );
    }
}
#[test]
fn real_night_irst_alternative_half_mr_day_and_outside_dogfight_exemptions() {
    for case in 0..4 {
        let (mut w, b, r) = night_duel();
        match case {
            0 => {
                install_test_ir(&mut w, b, harpoon_core::rules::detection2026::IrMode::Irst);
                local_track(&mut w, b, "night_test_ir", r);
                w.entity_mut(b)
                    .unwrap()
                    .ext
                    .insert("emission.radar".into(), 0.);
            }
            1 => {
                w.entity_mut(b).unwrap().altitude_m = 50.;
                assert_eq!(
                    harpoon_core::rules::aircombat2026::target_maneuver(&w, b).unwrap(),
                    0.5
                );
                w.rules.aircombat.as_mut().unwrap().positions.insert(
                    r,
                    harpoon_core::rules::aircombat2026::Position {
                        gun: 90,
                        aam: 90,
                        round: 1,
                    },
                );
            }
            2 => {
                w.day_night = harpoon_core::types::DayNight::Day;
            }
            _ => {
                w.rules.aircombat.as_mut().unwrap().fights.clear();
            }
        }
        if case == 1 {
            // J15/PL8B is an actual fitted source pair. F18/AIM9X2 versus
            // MR0.5 exceeds the last printed index after speed/signature/AI
            // shifts, so it cannot isolate this night-rule boundary.
            harpoon_core::sim::tick(
                &mut w,
                &[(
                    Side::Red,
                    harpoon_core::Command::Fire {
                        unit: r,
                        contact: ContactId(501),
                        weapon: "cn_pl8b".into(),
                        rounds: 1,
                    },
                )],
            );
        } else {
            night_fire(&mut w, b);
        }
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
                harpoon_core::EventKind::RulesEvent { rule, detail, .. }
                    if rule.contains("blocked") =>
                {
                    Some(detail)
                }
                _ => None,
            })
            .collect();
        assert_eq!(w.stats.projectiles_launched,1,"source night condition must preserve its IRST/half-MR/day/outside exceptions, case{case}; rejected={rejected:?}");
    }
}
#[test]
fn hoj_prelaunch_variant_and_emitting_condition_are_enforced() {
    let (mut w, b, r) = fixture();
    assert!(set_hoj(&mut w, Side::Blue, b, "us_aim9x2", true).is_err());
    set_hoj(&mut w, Side::Blue, b, "us_aim120d", true).unwrap();
    let mi = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_aim120d")
        .unwrap();
    assert!(fire_at(&mut w, b, r, mi, 2)
        .unwrap_err()
        .contains("radiating"));
    w.entity_mut(r).unwrap().ext.insert("ew.mode".into(), 1.);
    fire_at(&mut w, b, r, mi, 2).unwrap();
    let p = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .aam
        .packets
        .values()
        .next()
        .unwrap();
    assert!(p.hoj);
    assert_eq!(
        solution(&p.attack, 2).unwrap().modifiers["countermeasures"],
        0.
    );
}
#[test]
fn unknown_variant_and_category_cannot_fall_back() {
    let (mut w, b, r) = fixture();
    w.rules.detection.as_mut().unwrap().categories.clear();
    let mi = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_aim120d")
        .unwrap();
    assert!(fire_at(&mut w, b, r, mi, 2).is_err());
    assert!(weapon_profile("us_aim120").is_err());
    assert_eq!(w.stats.projectiles_launched, 0);
}

use harpoon_core::rules::aaw2026::*;
#[test]
fn source_aa_strength_table_is_not_range_fraction_pk() {
    assert_eq!(gun_probability(1.3).unwrap(), 0.27);
    assert_eq!(gun_probability(3.4).unwrap(), 0.47);
}
#[test]
fn source_sam_missile_index_example() {
    assert_eq!(missile_probability(0.5, 1).unwrap(), 0.35);
    assert_eq!(missile_probability(0.5, 2).unwrap(), 0.58);
}

#[test]
fn every_printed_table_cell_is_structured_and_callable() {
    let mut cells = 0;
    for group in tables()["intercept"].as_object().unwrap().values() {
        for row in group.as_object().unwrap().values() {
            assert_eq!(row.as_array().unwrap().len(), 6);
            for cs in row.as_array().unwrap() {
                for cell in cs.as_array().unwrap() {
                    cells += 1;
                    assert!(
                        cell.is_null()
                            || cell
                                .as_str()
                                .unwrap()
                                .chars()
                                .all(|c| "0123456789PLMVES-".contains(c))
                    );
                }
            }
        }
    }
    assert_eq!(cells, 498);
    for row in tables()["missile_attack"].as_array().unwrap() {
        assert_eq!(
            missile_probability(row[0].as_f64().unwrap(), 1).unwrap(),
            row[1].as_f64().unwrap() / 100.0
        );
        assert_eq!(
            missile_probability(row[0].as_f64().unwrap(), 2).unwrap(),
            row[2].as_f64().unwrap() / 100.0
        );
    }
    for row in tables()["gun_attack"].as_array().unwrap() {
        assert_eq!(
            gun_probability(row[0].as_f64().unwrap()).unwrap(),
            row[1].as_f64().unwrap()
        );
    }
}
#[test]
fn official_intercept_cells_and_impossible_descent() {
    let b = intercept_budget(4, false, 16000.0, 2003.0, 68.8, 1650.0, 3.5, false, None).unwrap();
    assert_eq!(b.source_cell, "L-2M-S");
    assert_eq!(b.count, [0, 1, 2, 1, 0, 0]);
    let b = intercept_budget(6, true, 50.0, 594.0, 27.0, 2250.0, 0.5, false, None).unwrap();
    assert_eq!(b.source_cell, "M-3S-P");
    let b = intercept_budget(4, false, 50.0, 594.0, 27.0, 2250.0, 0.5, false, None).unwrap();
    assert_eq!(b.source_cell, "2S-P");
    assert_eq!(b.table_band, Band::Short);
    assert!(intercept_budget(2, false, 50.0, 2000.0, 10.0, 2000.0, 1.0, false, None).is_err());
}
#[test]
fn source_receding_example_is_exact_other_ambiguous_cases_blocked() {
    let b = intercept_budget(4, false, 1000.0, 790.0, 30.0, 2000.0, 1.0, true, None).unwrap();
    assert_eq!(b.count, [0, 1, 1, 0, 0, 0]);
    assert!(intercept_budget(6, true, 50.0, 594.0, 15.0, 2000.0, 1.0, true, None).is_err());
}
#[test]
fn source_fast_sam_extra_requires_explicit_choice_and_valid_band() {
    let b = intercept_budget(
        4,
        false,
        16000.0,
        2003.0,
        68.8,
        3438.0,
        3.8,
        false,
        Some(Band::Long1),
    )
    .unwrap();
    assert_eq!(b.count[3], 2);
    assert_eq!(b.count[0], 0);
    assert!(intercept_budget(
        4,
        false,
        16000.0,
        2003.0,
        68.8,
        2999.0,
        1.0,
        false,
        Some(Band::Long1)
    )
    .is_err());
}
#[test]
fn bearing_rate_speed_rows_shift_towards_shorter_rows() {
    assert_eq!(bearing_rate(30.0, 750.0, 19.0).unwrap(), Bearing::Closing);
    assert_eq!(bearing_rate(30.0, 751.0, 19.0).unwrap(), Bearing::Divergent);
    assert_eq!(bearing_rate(30.0, 1526.0, 19.0).unwrap(), Bearing::Crossing);
    assert_eq!(bearing_rate(1.0, 500.0, 100.0).unwrap(), Bearing::Divergent);
}
#[test]
fn source_range_speed_and_fpole_boundaries() {
    assert_eq!(band(4.0).unwrap(), Band::Point);
    assert_eq!(band(4.00001).unwrap(), Band::Short);
    assert_eq!(band(90.00001).unwrap(), Band::VeryLong);
    assert_eq!(f_pole(40.0, 500.0, true, false).unwrap(), 40.0);
    assert_eq!(f_pole(40.0, 501.0, true, false).unwrap(), 60.0);
    assert_eq!(f_pole(40.0, 2001.0, true, false).unwrap(), 80.0);
    assert_eq!(f_pole(40.0, 2001.0, false, false).unwrap(), 40.0);
    assert_eq!(f_pole(40.0, 2001.0, true, true).unwrap(), 40.0);
}
fn official_attack() -> MissileAttack {
    // PDF83/8-3 supplies the net MR contribution for this Kh-22N example.
    // This algorithm input does not define any real missile family's MR.
    MissileAttack {
        ata: 2.0,
        maneuver_rating: 0.0,
        maneuvering: false,
        speed_kts: 2003.0,
        signature: 2,
        combat_generation: 4,
        automatic: false,
        target_is_missile: true,
        range_nm: 30.0,
        bearing: Bearing::Closing,
        terminal_maneuvers: false,
        sea_skimming: false,
        sea_capability: 2,
        noe: false,
        manpads_or_salh: false,
        high_diving: false,
        land_clutter: false,
        radar_guided: true,
        countermeasures: None,
        hoj: false,
    }
}
#[test]
fn official_ata_example_and_additive_modifiers() {
    let mut a = official_attack();
    let s = missile_solution(&a, 2).unwrap();
    assert_eq!(s.value, 0.5);
    assert_eq!(s.probability, 0.58);
    assert_eq!(s.modifiers["maneuver_rating"], 0.0);
    a.range_nm = 45.0;
    assert_eq!(missile_solution(&a, 1).unwrap().value, 0.0);
    a.signature = 3;
    assert_eq!(missile_solution(&a, 1).unwrap().value, -1.0);
    a.terminal_maneuvers = true;
    assert_eq!(missile_solution(&a, 1).unwrap().value, -2.0);
    a.land_clutter = true;
    assert_eq!(missile_solution(&a, 1).unwrap().value, -4.0);
}
#[test]
fn quarter_rows_and_unprinted_extrapolation_are_not_guessed() {
    assert!(missile_probability(0.25, 1).is_err());
    assert!(gun_probability(1.33).is_err());
    assert!(missile_probability(6.0, 1).is_err());
    assert_eq!(gun_probability(-10.0).unwrap(), 0.02);
    assert_eq!(gun_probability(100.0).unwrap(), 0.90);
}
#[test]
fn all_countermeasure_cells_and_nonmaneuver_half() {
    for seeker in 1..=5 {
        for mode in 0..3 {
            for generation in 1..=4 {
                let c = Countermeasures {
                    seeker_generation: seeker,
                    jammer_generation: if mode != 1 { Some(generation) } else { None },
                    decoy_generation: if mode != 0 { Some(generation) } else { None },
                };
                let expected = tables()["countermeasures"][seeker - 1][mode][generation - 1]
                    .as_f64()
                    .unwrap();
                assert_eq!(countermeasure_modifier(&c, true).unwrap(), expected);
                assert_eq!(countermeasure_modifier(&c, false).unwrap(), expected / 2.0);
            }
        }
    }
}
#[test]
fn independent_mount_two_bursts_per_30_seconds_not_per_target() {
    let mut s = State::default();
    assert_eq!(consume_burst(&mut s, "ford:ciws:0", 29, 2).unwrap(), 1);
    assert_eq!(consume_burst(&mut s, "ford:ciws:0", 30, 2).unwrap(), 2);
    assert!(consume_burst(&mut s, "ford:ciws:0", 58, 2).is_err());
    assert!(consume_burst(&mut s, "ford:ciws:1", 58, 2).is_ok());
    assert_eq!(consume_burst(&mut s, "ford:ciws:0", 59, 2).unwrap(), 1);
    let copy: State = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
    assert!(!can_burst(&copy, "ford:ciws:0", 58, 1));
}
#[test]
fn visual_sam_small_arms_and_lds_conditions() {
    assert!(!visual_sam_legal(3.0, false, false, false));
    assert!(!visual_sam_legal(3.0, true, false, true));
    assert!(visual_sam_legal(3.0, true, true, true));
    assert!(small_arms_legal(0.49, 2000.0, true));
    assert!(!small_arms_legal(0.50, 2000.0, true));
    assert!(dazzler_solution(true, true, false, true, false, false, 50.0).is_err());
    assert_eq!(
        dazzler_solution(true, false, false, true, false, false, 50.0).unwrap(),
        (0.60, 2, DazzleEffect::AbortAndReturn { climb_low: true })
    );
}
#[test]
fn source_arc_overlaps_do_not_duplicate_mounts() {
    assert!(arc_allows("SW", 0.0).unwrap());
    assert!(!arc_allows("SW", 180.0).unwrap());
    assert!(arc_allows("PQ&SQ", 180.0).unwrap());
    assert!(!arc_allows("PQ&SQ", 0.0).unwrap());
    assert!(arc_allows("F", 300.0).unwrap());
    assert!(!arc_allows("F", 180.0).unwrap());
}

use harpoon_core::{Database, EntityId, EventKind, Scenario, Side, Vec2, World};
fn fixture() -> (World, EntityId, EntityId, harpoon_core::ContactId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc:Scenario=serde_json::from_value(serde_json::json!({"id":"aaw-source","name":"AAW source","duration_s":1000,"seed":20261023,"context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":0,"precipitation":"None"}},"weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"us_ford","pos":[0,0],"speed_kts":0,"fire_on":false},{"side":"Red","class":"cn_052d","pos":[0,50],"speed_kts":0,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let blue = w
        .entities
        .values()
        .find(|e| e.side == Side::Blue)
        .unwrap()
        .id;
    let red = w
        .entities
        .values()
        .find(|e| e.side == Side::Red)
        .unwrap()
        .id;
    // A controlled incoming object isolates defense from source ASCM launch.
    // Production's no-bare-ASCM-launch gate remains in force.
    let mut e = w.entity(red).unwrap().clone();
    e.id = w.allocate_id();
    let incoming = e.id;
    e.class = "cn_yj83".into();
    e.name = "controlled incoming defense target".into();
    e.kind = harpoon_core::types::PlatformKind::Missile;
    e.rule_data = None;
    e.pos = Vec2::new(0.0, 10.0);
    e.course_deg = 180.0;
    e.ordered_course_deg = 180.0;
    e.speed_kts = 594.0;
    e.ordered_speed_kts = 594.0;
    e.altitude_m = 50.0;
    e.ordered_altitude_m = 50.0;
    e.hp = 1.0;
    e.hp_max = 1.0;
    e.weapons.clear();
    e.sensors.clear();
    e.ext.clear();
    e.deck = None;
    e.mission = None;
    e.station = None;
    e.projectile = Some(harpoon_core::world::ProjectileData {
        targeted_sensor: None,
        weapon: "cn_yj83".into(),
        target: Some(blue),
        aim: Vec2::zero(),
        launched_by: red,
        launched_by_side: Side::Red,
        live_s: 0.0,
        enabled: true,
        decoyed: false,
        decoy_point: None,
        chaff_checked: false,
        ciws_checked: false,
        passes: 0,
    });
    w.entities.insert(incoming, e);
    let cid = harpoon_core::ContactId(501);
    w.sides[Side::Blue.index()].contacts.insert(
        cid,
        harpoon_core::world::Contact {
            id: cid,
            about: incoming,
            category: harpoon_core::types::PlatformKind::Missile,
            class_name: None,
            estimate: Vec2::new(0.0, 10.0),
            est_course_deg: 180.0,
            est_speed_kts: 594.0,
            bearing_only: false,
            quality: 1.0,
            last_update_tick: 0,
            first_tick: 0,
            stale: false,
            sensors: vec!["radar_air_long".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.operations.as_mut().unwrap().tracks.insert(
        format!("{}:radar_air_long:{}", blue.0, incoming.0),
        harpoon_core::rules::operations::RadarTrack {
            seen: 0,
            recognized: 0,
            engage: 0,
            last_eligible: 0,
            roll: 7,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("{}:{}", Side::Blue.index(), incoming.0),
        harpoon_core::types::PlatformKind::Missile,
    );
    (w, blue, incoming, cid)
}

fn authored_sam_fixture() -> (World, EntityId, EntityId, harpoon_core::ContactId) {
    let (mut w, b, t, c) = fixture();
    let key = "scenario_assumption_aaw_missile";
    // Explicit fictional MR=0 is an authored algorithm input, never YJ-83 data.
    let profile: IncomingProfile = serde_json::from_value(serde_json::json!({
        "signature":3,"maneuver_rating":0.0,"maneuvering":false,
        "terminal_maneuvers":false,"high_diving":false,"sprint_vehicle":false,
        "source":"scenario_assumption"
    }))
    .unwrap();
    let config = Configuration {
        incoming_profiles: std::collections::BTreeMap::from([(key.into(), profile)]),
    };
    validate_configuration(&config).unwrap();
    w.context.aaw_2026 = Some(config);
    let mut def = w.db.weapons["cn_yj83"].clone();
    def.id = key.into();
    def.name = "Scenario assumption AAW algorithm target".into();
    def.rule_data = None;
    w.db.weapons.insert(key.into(), def);
    let incoming = w.entity_mut(t).unwrap();
    incoming.class = key.into();
    incoming.name = "Scenario assumption AAW algorithm target".into();
    incoming.projectile.as_mut().unwrap().weapon = key.into();
    (w, b, t, c)
}
#[test]
fn v4_ford_gets_independent_essm_i_and_old_profile_keeps_ii() {
    let (w, b, _, _) = fixture();
    let m = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_essm_i")
        .unwrap();
    assert_eq!(m.ammo, 16);
    assert_eq!(w.db.weapons["us_essm_i"].range_nm, 18.0);
    assert_eq!(w.db.weapons["us_essm"].range_nm, 27.0);
    assert_eq!(weapon_profile("us_ford", "us_essm_i").unwrap()["ata"], 3.5);
}
#[test]
fn authored_incoming_manual_packet_deducts_two_and_saves_one_outcome() {
    let (mut w, b, t, c) = authored_sam_fixture();
    let ammo = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_essm_i")
        .unwrap()
        .ammo;
    assert!(harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "us_essm_i",
        2
    ));
    assert_eq!(
        w.entity(b)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "us_essm_i")
            .unwrap()
            .ammo,
        ammo - 2
    );
    let ids: Vec<_> = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .shots
        .keys()
        .copied()
        .collect();
    assert_eq!(ids.len(), 2);
    let mut state = w.rules.aaw.take().unwrap();
    let a = terminal(&w, &mut state, ids[0]).unwrap();
    let b = terminal(&w, &mut state, ids[1]).unwrap();
    assert_eq!(a.d100, b.d100);
    assert_eq!(a.probability, 0.91);
    assert_eq!(state.outcomes.len(), 1);
    assert_eq!(a.consumed_projectiles.len(), 2);
    w.rules.aaw = Some(state);
    let mut copy: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let mut state = copy.rules.aaw.take().unwrap();
    let saved = terminal(&copy, &mut state, ids[0]).unwrap();
    assert_eq!(saved.d100, a.d100);
    assert!(copy.entity(t).unwrap().alive);
}
#[test]
fn authored_incoming_tick_resolves_one_packet_and_retires_both_rounds() {
    let (mut w, b, _, c) = authored_sam_fixture();
    assert!(harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "us_essm_i",
        2
    ));
    let ids: Vec<_> = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .shots
        .keys()
        .copied()
        .collect();
    let mut copy = w.clone();
    for _ in 0..30 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut copy, &[]);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&copy).unwrap()
    );
    assert_eq!(w.rules.aaw.as_ref().unwrap().outcomes.len(), 1);
    assert!(ids.iter().all(|id| w.entity(*id).is_none_or(|e| !e.alive)));
    assert_eq!(w.events.iter().filter(|e|matches!(&e.kind,EventKind::RulesEvent{domain,rule,..} if domain=="aaw2026"&&rule=="8.1.1.packet_terminal")).count(),1);
}
#[test]
fn unknown_air_target_category_or_metadata_cannot_fall_back_to_legacy() {
    let (mut w, b, t, c) = fixture();
    w.rules.detection.as_mut().unwrap().categories.clear();
    assert!(!harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "us_essm_i",
        2
    ));
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("{}:{}", Side::Blue.index(), t.0),
        harpoon_core::types::PlatformKind::Missile,
    );
    w.entity_mut(t).unwrap().projectile.as_mut().unwrap().weapon = "unverified_asm".into();
    assert!(!harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "us_essm_i",
        2
    ));
    assert!(w.rules.aaw.as_ref().unwrap().shots.is_empty());
}
#[test]
fn every_non_null_intercept_cell_reaches_the_right_source_row() {
    let alts = [
        ("high", 16000.0),
        ("medium", 5000.0),
        ("low", 1000.0),
        ("vlow", 50.0),
    ];
    let cs = [
        (2, false),
        (3, false),
        (4, false),
        (5, false),
        (5, true),
        (6, true),
    ];
    let speeds = [500.0, 1000.0, 2000.0, 2500.0, 3500.0];
    let ranges = [0.0, 10.0, 20.0, 40.0, 50.0, 75.0, 100.0, 150.0];
    let mut verified = 0;
    for (group, alt) in alts {
        for (b, rows) in tables()["intercept"][group].as_object().unwrap() {
            let b = b.parse::<usize>().unwrap();
            for (row, (gen, auto)) in cs.iter().enumerate() {
                for (col, speed) in speeds
                    .iter()
                    .enumerate()
                    .take(rows[row].as_array().unwrap().len())
                {
                    if let Some(cell) = rows[row][col].as_str() {
                        let r = intercept_budget(
                            *gen, *auto, alt, *speed, ranges[b], 2000.0, 1.0, false, None,
                        )
                        .unwrap();
                        assert_eq!(r.source_cell, cell, "{group}/{b}/{row}/{col}");
                        verified += 1;
                    }
                }
            }
        }
    }
    assert_eq!(verified, 375);
}
#[test]
fn actual_ciws_fires_source_strength_with_physical_mount_and_private_trace() {
    let (mut w, b, t, c) = fixture();
    assert!(world_target(&w, t).unwrap().0["maneuver_rating"].is_null());
    w.entity_mut(t).unwrap().pos = Vec2::new(0.0, 0.8);
    w.sides[Side::Blue.index()]
        .contacts
        .get_mut(&c)
        .unwrap()
        .estimate = Vec2::new(0.0, 0.8);
    let ammo = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "ciws_30mm")
        .unwrap()
        .ammo;
    assert_eq!(ammo, 21);
    assert!(harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "ciws_30mm",
        1
    ));
    assert_eq!(
        w.entity(b)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "ciws_30mm")
            .unwrap()
            .ammo,
        ammo - 1
    );
    let state = w.rules.aaw.as_ref().unwrap();
    assert_eq!(state.gun_total.values().sum::<u32>(), 1);
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{domain,rule,..}if domain=="aaw2026"&&rule=="8.1.3.source_gun")&&matches!(e.vis,harpoon_core::Visibility::Referee)));
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{domain,rule,..}if domain=="aaw2026"&&rule=="8.1.3.gun_release")&&matches!(e.vis,harpoon_core::Visibility::Side(Side::Blue))));
}
#[test]
fn adjudicated_gun_emplacement_loss_blocks_autonomous_defense_without_spending() {
    let (mut w, b, t, c) = fixture();
    // Laboratory ground body borrows the existing source CIWS algorithm fit;
    // this is an authored component assignment, not a real land installation.
    w.entity_mut(b).unwrap().name = "authored AA emplacement".into();
    w.entity_mut(b).unwrap().kind = harpoon_core::types::PlatformKind::Base;
    w.context.ground_2026 = Some(serde_json::from_value(serde_json::json!({
        "source":"scenario_assumption: controlled component-to-mount assignment",
        "facilities":{"authored AA emplacement":{
            "template":"aa_emplacement","custom":null,
            "source":"scenario_assumption: source table target with laboratory CIWS fit"
        }}
    })).unwrap());
    harpoon_core::rules::ground2026::initialize(&mut w).unwrap();
    w.entity_mut(t).unwrap().pos = Vec2::new(0.,0.8);
    w.sides[Side::Blue.index()].contacts.get_mut(&c).unwrap().estimate=Vec2::new(0.,0.8);
    let mut healthy=w.clone();
    assert!(harpoon_core::weapons::manual_fire(&mut healthy,Side::Blue,b,c,"ciws_30mm",1));
    // Isolate the functional aftermath of one already adjudicated source
    // critical. Incoming object/contact are the explicit defense premise above.
    harpoon_core::rules::ground2026::apply_damage(&mut w,b,0.,1,"adjudicated-gun-loss").unwrap();
    assert!(!harpoon_core::rules::ground2026::operational(&w,b));
    let ammo=w.entity(b).unwrap().weapons.iter().find(|m|m.weapon=="ciws_30mm").unwrap().ammo;
    assert!(!harpoon_core::weapons::manual_fire(&mut w,Side::Blue,b,c,"ciws_30mm",1));
    assert_eq!(w.entity(b).unwrap().weapons.iter().find(|m|m.weapon=="ciws_30mm").unwrap().ammo,ammo);
    assert!(w.rules.aaw.as_ref().unwrap().gun_total.is_empty());
}
#[test]
fn authored_incoming_occupies_four_source_director_channels_but_not_a_fifth() {
    let (mut w, b, _, c) = authored_sam_fixture();
    for _ in 0..4 {
        assert!(harpoon_core::weapons::manual_fire(
            &mut w,
            Side::Blue,
            b,
            c,
            "us_essm_i",
            2
        ));
    }
    assert!(!harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "us_essm_i",
        2
    ));
    let state = w.rules.aaw.as_ref().unwrap();
    assert_eq!(state.shots.len(), 8);
    assert_eq!(
        state
            .shots
            .values()
            .map(|p| p.permit.channel)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
}
#[test]
fn another_friendly_ship_last_increment_blocks_without_spending() {
    let (mut w, b, t, c) = fixture();
    let mut friend = w.entity(b).unwrap().clone();
    friend.id = w.allocate_id();
    friend.pos = Vec2::new(0.0, 10.1);
    let f = friend.id;
    w.entities.insert(f, friend);
    w.entity_mut(t).unwrap().projectile.as_mut().unwrap().target = Some(f);
    assert!(friendly_restriction(&w, b, t));
    let before = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_essm_i")
        .unwrap()
        .ammo;
    assert!(!harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "us_essm_i",
        2
    ));
    assert_eq!(
        w.entity(b)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "us_essm_i")
            .unwrap()
            .ammo,
        before
    );
}
#[test]
fn source_sam_requires_live_director_and_unknown_terminal_stays_pending() {
    let (mut w, b, t, c) = authored_sam_fixture();
    w.entity_mut(b).unwrap().systems.radar = 0.0;
    assert!(!harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "us_essm_i",
        2
    ));
    let mut d = w.db.weapons["us_essm_i"].clone();
    d.range_nm = 100.0;
    let id = harpoon_core::weapons::launch(&mut w, b, &d, Some(t), None).unwrap();
    w.entity_mut(id).unwrap().pos = w.entity(t).unwrap().pos;
    harpoon_core::weapons::advance_projectiles(&mut w);
    assert!(w.rules.pending_adjudications.contains_key(&id));
    let save = serde_json::to_vec(&w).unwrap();
    let mut restored: World = serde_json::from_slice(&save).unwrap();
    let pos = restored.entity(id).unwrap().pos;
    harpoon_core::weapons::advance_projectiles(&mut restored);
    assert_eq!(restored.entity(id).unwrap().pos, pos);
    assert!(restored.rules.aaw.as_ref().unwrap().outcomes.is_empty());
}
#[test]
fn source_regular_aa_uses_new_vlow_zero_modifier_and_one_transit_shot() {
    let (mut w, b, t, c) = fixture();
    assert!(world_target(&w, t).unwrap().0["maneuver_rating"].is_null());
    let gun = w.db.weapons["us_mk45_mod4"].clone();
    w.entity_mut(b).unwrap().class = "us_burke_2a".into();
    w.entity_mut(b).unwrap().weapons = vec![harpoon_core::world::WeaponMount {
        weapon: gun.id.clone(),
        mounts: 1,
        ammo: 20,
        cooldown_s: 0.0,
    }];
    w.entity_mut(t).unwrap().pos = Vec2::new(0.0, 3.0);
    w.entity_mut(t).unwrap().altitude_m = 50.0;
    let permit = world_gun_solution(&w, w.rules.aaw.as_ref().unwrap(), b, t, &gun, 0).unwrap();
    assert_eq!(permit.solution.value, 0.8);
    assert_eq!(permit.solution.probability, 0.21);
    assert_eq!(permit.solution.modifiers["altitude"], 0.0);
    assert!(harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        &gun.id,
        1
    ));
    assert!(!harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        &gun.id,
        1
    ));
    assert_eq!(w.entity(b).unwrap().weapons[0].ammo, 19);
}
#[test]
fn all_source_sam_salvo_rows_and_missing_combinations() {
    for row in tables()["sam_salvo_summary"].as_array().unwrap() {
        for g in row["guidance"].as_array().unwrap() {
            let auto = row["control"] == "automatic";
            let found = sam_channels(
                row["gen"].as_u64().unwrap() as usize,
                auto,
                g.as_str().unwrap(),
            )
            .unwrap();
            assert_eq!(
                found,
                (
                    row["channels"].as_u64().unwrap() as u8,
                    row["missiles"].as_u64().unwrap() as u8
                )
            );
        }
    }
    assert!(sam_channels(6, true, "TVM").is_err());
    assert_eq!(sam_channels(3, false, "TVM").unwrap(), (6, 2));
}

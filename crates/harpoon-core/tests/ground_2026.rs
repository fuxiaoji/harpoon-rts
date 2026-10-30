use harpoon_core::rules::ground2026::*;
use harpoon_core::{Database, EntityId, Scenario, World};
use serde_json::json;
fn fixture() -> (World, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json"))
        .unwrap()
        .merged(
            &Database::from_json(include_str!(
                "../../../database/volumes/scenario_exercise_assets.json"
            ))
            .unwrap(),
        );
    let sc:Scenario=serde_json::from_value(json!({"id":"ground-source","name":"Ground source fixture","duration_s":20000,"seed":1,"context":{"rules_profile":"hv2026-rts-v4","ground_2026":{"source":"scenario_assumption: test facility layout","facilities":{"Runway":{"template":"runway","custom":null,"source":"scenario_assumption","parent":"Base","runway":{"length_m":2000,"baseline":"Large","repair":{"seconds_per_cut":600,"teams":1,"source":"scenario_assumption: test crew"}}},"Tower":{"template":"control_tower","custom":null,"source":"scenario_assumption","parent":"Base"}},"complexes":{"Base":{"kind":"Airbase","source":"scenario_assumption","components":{"runways":["Runway"],"buildings":["Tower"]}}}}},"weather":{"visibility_nm":40,"jam":[0,0]},"units":[{"side":"Blue","class":"exercise_airbase","name":"Base","pos":[0,0]},{"side":"Blue","class":"exercise_airbase","name":"Runway","pos":[0,0]},{"side":"Blue","class":"exercise_airbase","name":"Tower","pos":[0,0]}]})).unwrap();
    let w = harpoon_core::load_scenario(sc, db).unwrap();
    let runway = w.entities.values().find(|e| e.name == "Runway").unwrap().id;
    let base = w.entities.values().find(|e| e.name == "Base").unwrap().id;
    (w, runway, base)
}
#[test]
fn all_128_source_missile_cells_and_satnav_rows() {
    for (family, key) in [
        (MissileFamily::Ballistic, "ballistic"),
        (MissileFamily::Lacm, "lacm"),
    ] {
        for (row, name) in tables()["guidance_rows"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            for (col, size) in "ABCDEFGH".chars().enumerate() {
                let s = MissileSpec {
                    family,
                    guidance: name.as_str().unwrap().into(),
                    satellite: false,
                    nuclear: false,
                    source: "English2026 PDF118".into(),
                };
                assert_eq!(
                    missile_probability(&s, size).unwrap(),
                    tables()[key][row][col].as_f64().unwrap()
                );
            }
        }
    }
    let mut s = MissileSpec {
        family: MissileFamily::Ballistic,
        guidance: "D2".into(),
        satellite: true,
        nuclear: false,
        source: "source".into(),
    };
    assert_eq!(missile_probability(&s, 'A').unwrap(), 0.40);
    s.guidance = "D2+".into();
    assert_eq!(missile_probability(&s, 'A').unwrap(), 0.40);
    s.nuclear = true;
    assert_eq!(missile_probability(&s, 'H').unwrap(), 1.);
}
#[test]
fn all_runway_and_reverse_hit_boundaries() {
    let source_rows = [
        [1, 2, 3, 3, 3, 3],
        [1, 2, 2, 2, 2, 3],
        [0, 1, 2, 2, 2, 2],
        [0, 1, 1, 2, 2, 2],
        [-1, 0, 1, 1, 1, 1],
        [-1, 0, 0, 1, 1, 1],
        [-1, -1, 0, 0, 0, 0],
        [-1, -1, -1, 0, 0, 0],
        [-1, -1, -1, -1, 0, 0],
        [-1, -1, -1, -1, -1, 0],
    ];
    for (r, row) in source_rows.iter().enumerate() {
        for (c, expected) in row.iter().enumerate() {
            let classes = [
                RunwayClass::STOL,
                RunwayClass::Small,
                RunwayClass::Medium,
                RunwayClass::Large,
            ];
            let expected = if *expected < 0 {
                None
            } else {
                Some(classes[*expected as usize])
            };
            assert_eq!(
                runway_capacity(r as u32 + 1, [500, 1000, 2000, 3000, 4000, 5000][c]).unwrap(),
                expected,
                "PDF117 source row {}, column {}",
                r + 1,
                c + 1
            );
        }
    }

    for cuts in 1..=10 {
        for length in [500, 1000, 2000, 3000, 4000, 5000] {
            let capacity = runway_capacity(cuts, length).unwrap();
            if cuts == 1 && length == 2000 {
                assert_eq!(capacity, Some(RunwayClass::Large));
            }
            if cuts == 10 && length == 500 {
                assert_eq!(capacity, None);
            }
        }
    }
    assert!(runway_capacity(1, 2700).is_err());
    assert!(runway_capacity(11, 5000).is_err());
    assert_eq!(
        reverse_component(ComplexKind::Airbase, 20)
            .unwrap()
            .as_deref(),
        Some("runways")
    );
    assert_eq!(
        reverse_component(ComplexKind::Airbase, 21)
            .unwrap()
            .as_deref(),
        Some("aircraft_storage")
    );
    assert_eq!(reverse_component(ComplexKind::Airbase, 61).unwrap(), None);
    assert_eq!(
        reverse_component(ComplexKind::Port, 90).unwrap().as_deref(),
        Some("defenses")
    );
    assert_eq!(reverse_component(ComplexKind::Port, 91).unwrap(), None);
    assert_eq!(runway_salvo(15, true, true, 5001., false).unwrap(), 3);
    assert_eq!(runway_salvo(3, true, true, 5001., false).unwrap(), 1);
    assert_eq!(runway_salvo(15, true, true, 5000., false).unwrap(), 15);
    assert!(runway_salvo(1, false, false, 200., true).is_err());
}
#[test]
fn source_facility_damage_disables_at_half_and_persists_cuts() {
    let (mut w, runway, base) = fixture();
    assert_eq!(source_target(&w, runway).unwrap().damage_points, Some(100.));
    assert!(source_target(&w, base).is_err());
    apply_damage(&mut w, runway, 49., 0, "first").unwrap();
    assert!(operational(&w, runway));
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&runway].cuts, 1);
    apply_damage(&mut w, runway, 1., 0, "second").unwrap();
    assert!(!operational(&w, runway));
    assert_eq!(w.entity(runway).unwrap().hp, 50.);
    assert!(w.entity(runway).unwrap().alive);
    apply_damage(&mut w, runway, 1., 0, "second").unwrap();
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&runway].cuts, 2);
    let tower = w.entities.values().find(|e| e.name == "Tower").unwrap().id;
    apply_damage(&mut w, tower, 1., 1, "critical").unwrap();
    assert!(!operational(&w, tower));
    assert_eq!(flight_rate_factor(&w, base, false).unwrap(), 0.5);
    let restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
#[test]
fn real_sim_runway_repair_uses_explicit_resources_and_cannot_repair_rubble() {
    let (mut w, runway, base) = fixture();
    apply_damage(&mut w, runway, 10., 0, "cut").unwrap();
    apply(&mut w, base, &Action::RepairRunway { runway }).unwrap();
    for _ in 0..599 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&runway].cuts, 1);
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&runway].cuts, 0);
    apply_damage(&mut w, runway, 100., 0, "rubble").unwrap();
    assert!(apply(&mut w, base, &Action::RepairRunway { runway }).is_err());
    assert_eq!(w.entity(runway).unwrap().hp, 0.);
}
fn airfield() -> (World, EntityId, EntityId) {
    let (mut w, runway, base) = fixture();
    w.context.aviation_2026=Some(serde_json::from_value(json!({"source":"scenario_assumption","loads":{"us_f18e":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"}},"parking":{},"initial_fuel_nm":{}})).unwrap());
    let deck = w.entity_mut(base).unwrap().deck.as_mut().unwrap();
    deck.ready = vec![("us_f18e".into(), 2)];
    deck.cycling.clear();
    w.context.air_wings.insert(
        "Base".into(),
        vec![harpoon_core::db::WingFit {
            class: "us_f18e".into(),
            count: 2,
        }],
    );
    w.context.ground_2026.as_mut().unwrap().air_services.insert("Base".into(),serde_json::from_value(json!({"source":"scenario_assumption: RTS airfield envelopes only","launch_lanes":1,"landing_lanes":1,"launch_interval_s":60,"landing_interval_s":60,"heading_deg":0,"initial_speed_kts":{"us_f18e":200},"landing_speed_kts":{"us_f18e":200},"turnaround_s":{"us_f18e":600},"landing_min_altitude_m":61,"landing_max_altitude_m":2000,"landing_speed_tolerance_kts":1,"landing_heading_tolerance_deg":1})).unwrap());
    harpoon_core::rules::aviation2026::initialize(&mut w).unwrap();
    initialize(&mut w).unwrap();
    (w, runway, base)
}
#[test]
fn real_sim_explicit_land_air_service_launches_and_cuts_gate_actual_aircraft() {
    let (mut w, runway, base) = airfield();
    assert!(runway_gate(&w, base, "us_f18e").is_ok());
    harpoon_core::sim::tick(
        &mut w,
        &[(
            harpoon_core::Side::Blue,
            harpoon_core::Command::LaunchAircraft {
                unit: base,
                class: "us_f18e".into(),
                count: 1,
                mission: harpoon_core::commands::AirMission::Recon {
                    point: harpoon_core::Vec2 { x: 20., y: 0. },
                },
                profile: None,
            },
        )],
    );
    while w.tick < 300 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.rules.aviation.as_ref().unwrap().flights.is_empty());
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(w.rules.aviation.as_ref().unwrap().flights.len(), 1);
    for i in 0..4 {
        apply_damage(&mut w, runway, 1., 0, &format!("cut{i}")).unwrap();
    }
    assert!(
        runway_gate(&w, base, "us_f18e").is_ok(),
        "four cuts on 2000m permit the source Small F/A-18E"
    );
    for i in 4..8 {
        apply_damage(&mut w, runway, 1., 0, &format!("cut{i}")).unwrap();
    }
    assert!(
        runway_gate(&w, base, "us_f18e").is_err(),
        "eight cuts on 2000m are X: no aircraft can use the runway"
    );
    let before = w.next_entity;
    assert!(harpoon_core::air::launch_aircraft(
        &mut w,
        base,
        "us_f18e",
        1,
        &harpoon_core::commands::AirMission::Recon {
            point: Default::default()
        }
    )
    .is_err());
    assert_eq!(w.next_entity, before);
}

#[test]
fn blocked_same_runway_launches_resume_with_actual_lane_spacing_and_save() {
    let (mut w, runway, base) = airfield();
    // A declared accelerated repair fixture; these are author timings, not
    // Harpoon universal repair values. Actual repair commands clear three cuts.
    let repair = w
        .rules
        .ground
        .as_mut()
        .unwrap()
        .facilities
        .get_mut(&runway)
        .unwrap()
        .config
        .runway
        .as_mut()
        .unwrap()
        .repair
        .as_mut()
        .unwrap();
    repair.seconds_per_cut = 1;
    repair.teams = 3;
    repair.source = "scenario_assumption: source lane-spacing regression".into();
    harpoon_core::air::launch_aircraft(
        &mut w,
        base,
        "us_f18e",
        2,
        &harpoon_core::AirMission::Recon {
            point: harpoon_core::Vec2::new(20., 0.),
        },
    )
    .unwrap();
    for i in 0..8 {
        apply_damage(&mut w, runway, 1., 0, &format!("blocked-cut-{i}")).unwrap();
    }
    while w.tick < 400 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.rules.aviation.as_ref().unwrap().flights.is_empty());
    for _ in 0..3 {
        apply(&mut w, base, &Action::RepairRunway { runway }).unwrap();
    }
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&runway].cuts, 5);
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights.len(),
        1,
        "two delayed jobs must not launch together when a shared runway reopens"
    );
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    while w.tick < 460 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights.len(),
        1,
        "declared60-second lane interval is measured from actual first takeoff401"
    );
    harpoon_core::sim::tick(&mut w, &[]);
    harpoon_core::sim::tick(&mut saved, &[]);
    assert_eq!(w.rules.aviation.as_ref().unwrap().flights.len(), 2);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}

#[test]
fn land_ratio_uses_remaining_dp_for_f_size_building_without_ship_sinking_side_effect() {
    let (mut w, _, _) = fixture();
    let tower = w.entities.values().find(|e| e.name == "Tower").unwrap().id;
    let before = w.entity(tower).unwrap().hp;
    let count = harpoon_core::rules::damage_runtime::land_ratio_criticals(
        &mut w,
        tower,
        before * 0.75,
        "land-ratio",
    );
    assert!(
        count >= 10,
        "ratio3 adds ten critical hits beyond the 1.0 source row"
    );
    assert_eq!(
        w.entity(tower).unwrap().hp,
        before,
        "11.1 structures cannot sink like ships"
    );
    let event = w
        .events
        .iter()
        .find_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent { rule, detail, .. }
                if rule == "critical_ratio" =>
            {
                Some(serde_json::from_str::<serde_json::Value>(detail).unwrap())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(event["remaining"], json!(before * 0.25));
    let events = w.events.len();
    assert_eq!(
        harpoon_core::rules::damage_runtime::land_ratio_criticals(
            &mut w,
            tower,
            before,
            "destroyed"
        ),
        0
    );
    assert_eq!(
        harpoon_core::rules::damage_runtime::land_ratio_criticals(
            &mut w,
            tower,
            before * 0.009,
            "below-one-percent"
        ),
        0
    );
    assert_eq!(
        w.events.len(),
        events,
        "direct destruction and insignificant hits do not roll extra dice"
    );
}
#[test]
fn facility_runtime_fingerprint_and_observation_are_own_side_only() {
    let (mut w, runway, _) = fixture();
    assert!(w.rules.validate_tables().is_ok());
    apply_damage(&mut w, runway, 50., 0, "own-damage").unwrap();
    let blue = harpoon_core::observe::observe(&w, harpoon_core::Side::Blue, 0, 1000);
    let red = harpoon_core::observe::observe(&w, harpoon_core::Side::Red, 0, 1000);
    assert!(blue
        .units
        .iter()
        .find(|e| e.id == runway)
        .unwrap()
        .ground
        .as_ref()
        .unwrap()["facility"]["disabled"]
        .as_bool()
        .unwrap());
    assert!(!serde_json::to_string(&red).unwrap().contains("own-damage"));
    assert!(red.units.is_empty());
    w.rules.ground = None;
    assert!(
        w.rules.validate_tables().is_err(),
        "v4 cannot resume without its facility state"
    );
}

//! A pending acoustic branch cannot stop an independent size/foil/wake rule.
//! Source Wake performance is unchanged. Gyro2 is an explicitly authored
//! laboratory weapon, following the accepted straight-runner fixture; no real
//! hardware fit or native FCS claim. No HP/Flight/phase/fuel/die is injected.
use harpoon_core::rules::damage_runtime as damage;
use harpoon_core::{
    Command, ContactId, Database, EntityId, EventKind, Scenario, Side, Vec2, World,
};
use serde_json::{json, Value};

fn details(w: &World, wanted: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == wanted => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}

fn fixture(wake: bool) -> (World, EntityId, EntityId, String, u8) {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let (class, weapon, rounds, bearing, depth) = if wake {
        ("ru_oscar2", "ru_65k", 1_u8, 90., 5.)
    } else {
        ("us_virginia", "scenario_signature_gyro2", 4_u8, 0., 0.)
    };
    if !wake {
        let mut p = db.weapons["us_mk48"].clone();
        p.id = weapon.into();
        p.name = "scenario_assumption: controlled Gyro2 one-DP laboratory warhead".into();
        p.salvo = 4;
        p.speed_kts = 50.;
        p.range_nm = 10.;
        p.targets.surface = true;
        p.targets.sub = false;
        p.rule_data = Some(damage::WeaponDamage {
            ship_dp: Some(1.),
            sub_dp: Some(1.),
            penetration_cm: Some(10000.),
            fuze: "influence".into(),
            source: "scenario_assumption: one-DP test warhead; not Mk48/Gyro hardware data".into(),
            ..Default::default()
        });
        db.weapons.insert(weapon.into(), p);
    }
    for key in [class, "us_burke_2a"] {
        db.platforms.get_mut(key).unwrap().sensors.clear();
    }
    db.platforms.get_mut("us_burke_2a").unwrap().weapons.clear();
    let launcher = db.platforms.get_mut(class).unwrap();
    let mut mount = launcher.weapons[0].clone();
    mount.weapon = weapon.into();
    mount.ammo = rounds as u16;
    mount.mounts = rounds;
    launcher.weapons = vec![mount];
    let performance = if wake {
        json!({"source":"Russia2025 PDF90/F-1 exact65-76A Kit; F-2 remarkE; historical fixture",
            "guidance":"Wake","generation":3,"settings":[{"speed_kts":50,"range_nm":27}],
            "max_depth_m":25,"shallow_water_capable":false,"surface":true,"submarine":false,"max_sea_state":4,
            "launch_offset":{"source":"scenario_assumption: controlled single650mm tube, not a Wake default","degrees":0}})
    } else {
        json!({"source":"scenario_assumption: accepted Gyro2 laboratory speed/range, not historical hardware data",
            "guidance":"Gyro2","generation":0,"settings":[{"speed_kts":50,"range_nm":10}],
            "max_depth_m":50,"shallow_water_capable":false,"surface":true,"submarine":false})
    };
    let tubes: Vec<Value> = (0..rounds)
        .map(|_| json!({"bearing_deg":bearing,"loaded":weapon}))
        .collect();
    let mut cfg = json!({"source":"scenario_assumption: independent nonacoustic consumer laboratory",
        "weapons":{weapon:performance},
        "environment":{"source":"scenario_assumption: no layer or reverberation","layer_m":null,"reverberant":false},
        "targets":{"us_burke_2a":{"source":"scenario_assumption: independently sourced coated fit intentionally conflicts with detection coating",
            "size":1,"noise":2,"anechoic":true,"foil_borne":false}},
        "platforms":{class:{"source":"scenario_assumption: controlled loaded laboratory tubes, not complete platform fit",
            "wire_capacity":0,"reload_parallel":1,"tubes":tubes,"reserves":{}}},
        "orders":{format!("SHOOTER:{weapon}"):{"source":"scenario_assumption: known own report and authored Good; not native FCS",
            "speed_index":0,"depth_m":depth,"enable_run_nm":0,"bearing_deg":bearing,"spread_deg":if wake {vec![]} else {vec![0.;4]},
            "cut_wire":true,"solution":{"quality":"Good","source":"scenario_assumption: authored isolated source solution"}}}});
    if wake {
        cfg["wake"] = json!({"source":"scenario_assumption: explicit width/lifetime/prehistory, not book constants",
            "half_width_nm":0.02,"lifetime_s":600,"history_policy":"explicit_segments_only",
            "initial_segments":[{"target_unit":"TARGET","from":{"x":0,"y":-1},"to":{"x":0,"y":1},"created_tick":-30}]});
    }
    let target_pos = if wake { [0., 1.] } else { [0., 1.25] };
    let sc: Scenario = serde_json::from_value(json!({
        "id":if wake {"signature-pending-native-wake"} else {"signature-pending-native-gyro"},
        "name":"scenario_assumption: actual source release/motion with independent acoustic conflict",
        "duration_s":1500,"seed":20261023,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":4,"precipitation":"None"},
            "torpedo_2026":cfg,"detection_2026":{"source":"scenario_assumption: explicit separate acoustic source",
                "targets":{"us_burke_2a":{"source":"scenario_assumption: uncoated source deliberately conflicts only with torpedo coating",
                    "size":1,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
        "units":[{"side":"Blue","class":class,"name":"SHOOTER","pos":if wake {[-1.,-0.5]} else {[0.,0.]},
                "course_deg":0,"speed_kts":0,"depth_m":if wake {25.} else {50.},"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"TARGET","pos":target_pos,
                "course_deg":if wake {0.} else {90.},"speed_kts":0,"fire_on":false}]
    }))
    .unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let find = |name| w.entities.values().find(|e| e.name == name).unwrap().id;
    let shooter = find("SHOOTER");
    let target = find("TARGET");
    assert_eq!(w.entity(target).unwrap().hp_max, 341.);
    w.sides[0].contacts.insert(
        ContactId(91),
        harpoon_core::world::Contact {
            id: ContactId(91),
            about: target,
            category: harpoon_core::types::PlatformKind::Ship,
            class_name: None,
            estimate: Vec2::new(target_pos[0], target_pos[1]),
            est_course_deg: if wake { 0. } else { 90. },
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec![
                "scenario_assumption: authored own located report, not native FCS".into(),
            ],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", target.0),
        harpoon_core::types::PlatformKind::Ship,
    );
    (w, shooter, target, weapon.into(), rounds)
}

fn primary_creates_real_acoustic_pending(w: &mut World, target: EntityId) {
    let original = w.entity(target).unwrap().hp_max;
    let prior = w.entity(target).unwrap().hp;
    let mut lab = w.db.weapons["us_mk48"].clone();
    lab.id = "scenario_signature_nonacoustic_primary".into();
    lab.name = "scenario_assumption: adjudicated eleven-percent laboratory primary".into();
    lab.damage = original * 0.11;
    lab.rule_data = Some(damage::WeaponDamage {
        ship_dp: Some(original * 0.11),
        sub_dp: Some(original * 0.11),
        penetration_cm: Some(10000.),
        fuze: "influence".into(),
        source: "scenario_assumption: exact adjudicated primary, not a real weapon release".into(),
        ..Default::default()
    });
    harpoon_core::damage::apply_hit_at(w, target, &lab, true, Some(90.));
    assert!(w.entity(target).is_some_and(|e| e.alive && e.hp < prior));
    assert!(details(w, "hit_resolved").iter().any(|v| {
        v["weapon"] == lab.id
            && v["dp"]
                .as_f64()
                .is_some_and(|dp| (dp - original * 0.11).abs() < 1e-8)
    }));
    assert!(details(w, "14.2.signature_pending").iter().any(|v| {
        v["branches"]["acoustic"]
            .as_str()
            .is_some_and(|s| s.contains("conflict"))
    }));
    assert!(details(w, "14.2.3.acoustic").is_empty());
}

fn real_case(wake: bool) {
    let (mut w, shooter, target, weapon, rounds) = fixture(wake);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: shooter,
                contact: ContactId(91),
                weapon,
                rounds,
            },
        )],
    );
    let members: Vec<_> = w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .copied()
        .collect();
    assert_eq!(
        members.len(),
        rounds as usize,
        "actual legal source Fire must release real bodies"
    );
    let id = members[0];
    let at_release = w.entity(id).unwrap().pos;
    for _ in 0..5 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(
        w.entity(id).unwrap().pos.range_to(at_release) > 0.05,
        "native flight must move before damage"
    );
    assert!(w.rules.pending_adjudications.is_empty());
    primary_creates_real_acoustic_pending(&mut w, target);
    let after_damage = w.entity(id).unwrap().pos;
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    restored.rules.validate_tables().unwrap();
    for _ in 0..90 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
    assert!(w.rules.pending_adjudications.is_empty(),
        "acoustic source conflict must stay in its branch: pure Wake/Gyro cannot be frozen by unused noise/coating");
    if wake {
        assert!(w.entity(id).unwrap().pos.range_to(after_damage) > 0.5);
        assert_eq!(
            details(&w, "10.2.6.wake_crossing").len(),
            1,
            "actual authored wake must be physically crossed"
        );
        assert_eq!(
            details(&w, "10.2.6.wake_placement").len(),
            1,
            "actual source Wake placement still executes once"
        );
    } else {
        let salvos = details(&w, "10.2.3.source_salvo");
        assert_eq!(
            salvos.len(),
            1,
            "actual swept straight spread must reach its source table"
        );
        assert_eq!(salvos[0]["target"], target.0);
        assert_eq!(salvos[0]["members"].as_array().unwrap().len(), 4);
        assert_eq!(salvos[0]["effective_size"], "Medium");
    }
}

#[test]
fn native_wake_motion_and_crossing_ignore_unrelated_persistent_acoustic_pending() {
    real_case(true);
}

#[test]
fn native_gyro_spread_actual_encounter_ignores_unrelated_persistent_acoustic_pending() {
    real_case(false);
}

#[test]
fn public_torpedo_advance_outside_batch_keeps_full_torpedo_source_at_actual_hit() {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let launcher = db.platforms.get_mut("us_virginia").unwrap();
    // Same explicitly authored Mk54 tube as the already accepted C12 native
    // hit. Keep original Mk54 performance and original target145DP unchanged.
    launcher.weapons[0].weapon = "us_mk54".into();
    launcher.weapons[0].ammo = 1;
    launcher.weapons[0].mounts = 1;
    launcher.sensors.clear();
    db.platforms.get_mut("cn_093").unwrap().weapons.clear();
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    let seed =
        (1..20_000_u64)
            .find(|seed| {
                let mut placement = harpoon_core::Rng::new(*seed)
                    .derive(harpoon_core::Rng::hash_tag(0, 0, "10.placement:3:0"));
                let mut attack = harpoon_core::Rng::new(*seed).derive(harpoon_core::Rng::hash_tag(
                    0,
                    3,
                    "10.2.5.attack:0",
                ));
                let d10 = 1 + (placement.next_f64() * 10.) as u8;
                let d100 = 1 + (attack.next_f64() * 100.) as u8;
                d10 <= 7 && d100 <= 10
            })
            .unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"signature-public-advance-native-hit",
        "name":"scenario_assumption: authored tube/report/Good; actual source Mk54 outside sim damage batch",
        "seed":seed,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "detection_2026":{"source":"scenario_assumption: known active target size/coating; passive noise cell is genuinely absent",
                "targets":{"cn_093":{"source":"scenario_assumption: partial detection profile, complete independent torpedo source below",
                    "size":2,"noise":null,"anechoic":true,"titanium":false,"pumpjet":false,
                    "stealthy_ir":false,"radar_signature":2}}},
            "torpedo_2026":{"source":"scenario_assumption: public source-entry lifecycle laboratory",
                "environment":{"source":"scenario_assumption: no layer/reverberation","layer_m":null,"reverberant":false},
                "targets":{"cn_093":{"source":"scenario_assumption: complete independently verified Quiet/coated source",
                    "size":2,"noise":2,"anechoic":true,"foil_borne":false}},
                "platforms":{"us_virginia":{"source":"scenario_assumption: isolated loaded Mk54 tube, not a real Virginia fit",
                    "wire_capacity":2,"reload_parallel":2,"tubes":[{"bearing_deg":0,"loaded":"us_mk54"}],"reserves":{}}},
                "orders":{"SHOOTER:us_mk54":{"source":"scenario_assumption: explicit source release and authored Good, not native FCS",
                    "speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":0,"cut_wire":true,"spread_deg":[],
                    "solution":{"quality":"Good","source":"scenario_assumption: controlled own located report"}}}}},
        "units":[{"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"depth_m":50,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,0.5],"depth_m":50,"course_deg":0,"speed_kts":0,"fire_on":false}]
    }))
    .unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let find = |name| w.entities.values().find(|e| e.name == name).unwrap().id;
    let shooter = find("SHOOTER");
    let target = find("TARGET");
    let original_hp = w.entity(target).unwrap().hp;
    assert_eq!(w.entity(target).unwrap().hp_max, 145.);
    w.sides[0].contacts.insert(
        ContactId(91),
        harpoon_core::world::Contact {
            id: ContactId(91),
            about: target,
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
            sensors: vec!["scenario_assumption: authored own report, not native sonar FCS".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", target.0),
        harpoon_core::types::PlatformKind::Sub,
    );
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: shooter,
                contact: ContactId(91),
                weapon: "us_mk54".into(),
                rounds: 1,
            },
        )],
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
    assert_eq!(
        id,
        EntityId(3),
        "natural seed uses actual third allocated body"
    );
    assert!(
        details(&w, "hit_resolved").is_empty(),
        "first source launch tick cannot already hit this distant target"
    );
    let origin = w.entity(id).unwrap().pos;
    let def = w.db.weapons["us_mk54"].clone();
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    restored.rules.validate_tables().unwrap();
    let mut moved = false;
    for _ in 0..150 {
        if !details(&w, "hit_resolved").is_empty() {
            break;
        }
        for state in [&mut w, &mut restored] {
            assert!(
                !state.rules.in_batch,
                "public entry has no surrounding damage batch"
            );
            state.tick += 1;
            harpoon_core::rules::torpedo2026::begin(state);
            harpoon_core::rules::torpedo2026::step(state);
            harpoon_core::rules::torpedo2026::advance(state, id, &def)
                .expect("actual public source advance must complete its supported tick");
            assert!(
                !state.rules.in_batch,
                "public advance must preserve caller batch ownership"
            );
        }
        moved |= w.entity(id).unwrap().pos.range_to(origin) > 0.001;
    }
    assert!(moved, "native real flight must precede terminal damage");
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
    assert!(details(&w, "10.2.5.source_terminal")
        .iter()
        .any(|v| v["hit"] == true));
    assert!(
        details(&w, "hit_resolved")
            .iter()
            .any(|v| { v["weapon"] == "us_mk54" && v["dp"].as_f64().is_some_and(|dp| dp >= 14.5) }),
        "natural Mk54 source attack must really cross original145DP's threshold"
    );
    assert!(w.entity(target).unwrap().hp < original_hp);
    assert!(
        w.rules.torpedoes.is_some(),
        "public entry must restore original source runtime"
    );
    assert!(
        w.rules.pending.is_empty(),
        "actual hit commitment cannot be delayed indefinitely"
    );
    assert!(details(&w, "14.2.signature_pending").is_empty(),
        "temporarily taking torpedo runtime cannot erase its complete valid source while immediate damage commits");
    assert_eq!(
        details(&w, "14.2.3.acoustic").len(),
        1,
        "same actual source hit must commit acoustic damage even outside the main sim batch"
    );
}

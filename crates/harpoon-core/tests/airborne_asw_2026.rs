//! §9.7.1 real command/weapon-loop regressions. Source: English Jan2026
//! PDF109/9-8; exact Mk54 profile is supplied by the chapter10 source executor.
use harpoon_core::rules::{aircombat2026, aviation2026};
use harpoon_core::{Command, ContactId, Database, EntityId, Scenario, Side, Vec2, World};
use serde_json::json;

fn fixture(two: bool) -> (World, EntityId, EntityId, Option<EntityId>) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut units = vec![
        json!({"side":"Blue","class":"us_mh60r","name":"HELO1","pos":[0,0],"speed_kts":0,"altitude_m":1000,"fire_on":false}),
        json!({"side":"Red","class":"cn_093","name":"SUB","pos":[0,1],"speed_kts":0,"depth_m":50,"fire_on":false}),
    ];
    if two {
        units.push(json!({"side":"Blue","class":"us_mh60r","name":"HELO2","pos":[0,0],"speed_kts":0,"altitude_m":1000,"fire_on":false}));
    }
    let sc:Scenario=serde_json::from_value(json!({
        "id":"airborne-asw-source","name":"Airborne ASW source acceptance","duration_s":1000,"seed":20261023,
        "context":{"rules_profile":"hv2026-rts-v4","torpedo_2026":{
            "source":"scenario_assumption: ASW acceptance source mechanisms",
            "environment":{"source":"scenario_assumption: no layer or bottom reverberation","layer_m":null,"reverberant":false},
            "targets":{"cn_093":{"source":"scenario_assumption: controlled target attributes","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
            "orders":if two {json!({"HELO1:us_mk54":{"source":"scenario_assumption","speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":null,"spread_deg":[],"cut_wire":true,"solution":{"quality":"Good","source":"scenario_assumption: fixture fire-control quality"}},"HELO2:us_mk54":{"source":"scenario_assumption","speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":null,"spread_deg":[],"cut_wire":true,"solution":{"quality":"Good","source":"scenario_assumption: fixture fire-control quality"}}})}else{json!({"HELO1:us_mk54":{"source":"scenario_assumption","speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":null,"spread_deg":[],"cut_wire":true,"solution":{"quality":"Good","source":"scenario_assumption: fixture fire-control quality"}}})}
        },"aviation_2026":{
            "source":"scenario_assumption: controlled unarmed-extra-payload and fuel for ASW source regression",
            "loads":{"us_mh60r":{"payload_kg":0,"tanks":[],"source":"scenario_assumption: mass-neutral acceptance fixture"}},
            "parking":{},"initial_fuel_nm":{"HELO1":200,"HELO2":200}
        }},"weather":{"visibility_nm":0,"jam":[0,0]},"units":units
    })).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let h = w.entities.values().find(|e| e.name == "HELO1").unwrap().id;
    let t = w.entities.values().find(|e| e.name == "SUB").unwrap().id;
    let h2 = w
        .entities
        .values()
        .find(|e| e.name == "HELO2")
        .map(|e| e.id);
    w.entity_mut(t).unwrap().weapons.clear();
    let pos = w.entity(t).unwrap().pos;
    w.sides[0].contacts.insert(
        ContactId(71),
        harpoon_core::world::Contact {
            id: ContactId(71),
            about: t,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: pos,
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec!["sonar_dipping".into()],
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
    w.rules.operations.as_mut().unwrap().motion.insert(
        format!("0:{}", t.0),
        harpoon_core::rules::operations::MotionFix {
            start: -1800,
            last: 0,
            first: pos,
            known: true,
        },
    );
    for id in std::iter::once(h).chain(h2) {
        aviation2026::apply(
            &mut w,
            id,
            &aviation2026::Action::Throttle {
                setting: aviation2026::Throttle::Hover,
            },
        )
        .unwrap();
        w.rules.aaw.as_mut().unwrap().firecontrol.tracks.insert(
            format!("{}:sonar_dipping:{}", id.0, t.0),
            harpoon_core::rules::firecontrol2026::Track {
                observer: id,
                target: t,
                sensor: "sonar_dipping".into(),
                first: -1800,
                last: 0,
                turn_course: 0.,
                turn_speed: 0.,
                last_turn: 0,
                zig_until: 0,
                report: None,
                motion: None,
            },
        );
    }
    (w, h, t, h2)
}
fn fire(h: EntityId, rounds: u8) -> (Side, Command) {
    (
        Side::Blue,
        Command::Fire {
            unit: h,
            contact: ContactId(71),
            weapon: "us_mk54".into(),
            rounds,
        },
    )
}
fn ammo(w: &World, h: EntityId) -> u16 {
    w.entity(h)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_mk54")
        .unwrap()
        .ammo
}

#[test]
fn real_fire_rejects_airborne_torpedo_high_altitude_and_excess_speed_without_ammo_loss() {
    for high in [true, false] {
        let (mut w, h, _, _) = fixture(false);
        if high {
            let e = w.entity_mut(h).unwrap();
            e.altitude_m = 2500.;
            e.ordered_altitude_m = 2500.;
        } else {
            aviation2026::apply(
                &mut w,
                h,
                &aviation2026::Action::Throttle {
                    setting: aviation2026::Throttle::Fmp,
                },
            )
            .unwrap();
            w.entity_mut(h).unwrap().speed_kts = 140.;
        }
        let before = ammo(&w, h);
        harpoon_core::sim::tick(&mut w, &[fire(h, 1)]);
        assert_eq!(
            w.stats.projectiles_launched, 0,
            "9.7.1 release envelope must reach real Fire, not only helper tests"
        );
        assert_eq!(ammo(&w, h), before);
    }
}

#[test]
fn real_manual_and_automatic_airborne_drop_share_one_per_turn_clock() {
    let (mut w, h, _, _) = fixture(false);
    harpoon_core::sim::tick(&mut w, &[fire(h, 2)]);
    assert_eq!(
        w.stats.projectiles_launched, 1,
        "a two-round Fire request cannot bypass one aerial torpedo per turn"
    );
    assert_eq!(ammo(&w, h), 1);
    assert_eq!(
        w.rules.aircombat.as_ref().unwrap().air_torpedo_last.get(&h),
        Some(&1)
    );
    w.entity_mut(h).unwrap().assigned = Some(ContactId(71));
    for _ in 0..30 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(
        w.stats.projectiles_launched, 1,
        "automatic scan must obey the same clock after mount cooldown20s"
    );
    assert_eq!(ammo(&w, h), 1);
}

#[test]
fn real_same_target_air_drops_interfere_before_damage_and_survive_world_resume() {
    let (mut w, h, t, h2) = fixture(true);
    let before = w.entity(t).unwrap().hp;
    harpoon_core::sim::tick(&mut w, &[fire(h, 1), fire(h2.unwrap(), 1)]);
    assert_eq!(w.stats.projectiles_launched, 2);
    assert_eq!(
        w.rules
            .aircombat
            .as_ref()
            .unwrap()
            .interfered_torpedoes
            .len(),
        2,
        "7.5.3/9.7.1 both entering-water torpedoes must be marked"
    );
    assert_eq!(
        w.entity(t).unwrap().hp,
        before,
        "interference must be checked before either immediate attack can damage target"
    );
    assert!(w
        .rules
        .aircombat
        .as_ref()
        .unwrap()
        .interfered_torpedoes
        .iter()
        .all(|id| aircombat2026::air_torpedo_interfered(&w, *id)));
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..3 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    assert_eq!(w.entity(t).unwrap().hp, before);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}

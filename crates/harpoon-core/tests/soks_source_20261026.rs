//! Source-only SOKS candidates against existing public API. No new enum required.
//! Author SOKS fit is a controlled physical retrofit, not a real Annex fit claim.
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, Vec2, World};
use serde_json::{json, Value};
fn seed() -> u64 {
    (0..100_000)
        .find(|s| {
            (0..3).all(|i| {
                let key = format!("5.7.3:1:mnk100_kolos:2:{i}");
                let mut r = Rng::new(*s).derive(Rng::hash_tag(0, 0, &key));
                1 + (r.next_f64() * 100.) as u8 <= 25
            })
        })
        .unwrap()
}
fn input(depth: f64) -> Value {
    json!({"id":"soks-source","name":"scenario_assumption: exact physical MNK100 author retrofit","seed":seed(),"duration_s":2500,
 "weather":{"visibility_nm":40,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4",
 "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: sea3 and deep water","sea_state":3,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
 "soks":{"source":"English2026 PDF61 section5.7.3 plus explicit physical author retrofit; no consolidated sensor fit claim",
 "fits":{"LOOK":{"source":"scenario_assumption: actual hull-mounted MNK100 installed retrofit on this exact body; not original Akula Annex proof","system":"mnk100_kolos","installed":true,"enabled":true,"mount":"hull"}},
 "geometry":{"source":"scenario_assumption: exact centerline crossings and independent0.1nm points; no guessed wake width","crossing_policy":"exact_centerline_intersection","distinct_fix_separation_nm":0.1,"history_policy":"actual_runtime_only","trial_policy":"one_per_discrete_crossing","course_policy":"historical_local_tangent_after_three"}}}},
 "units":[{"side":"Blue","class":"ru_akula1","name":"LOOK","pos":[0.1,-0.2],"course_deg":0,"speed_kts":8,"depth_m":depth,"fire_on":false},
 {"side":"Red","class":"us_virginia","name":"SUB","pos":[0,0],"course_deg":90,"speed_kts":8,"depth_m":150,"fire_on":false}]})
}
fn world(depth: f64) -> World {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for class in ["ru_akula1", "us_virginia"] {
        let p = db.platforms.get_mut(class).unwrap();
        p.sensors.clear();
        p.weapons.clear();
    }
    harpoon_core::load_scenario(
        serde_json::from_value::<Scenario>(input(depth)).unwrap(),
        db,
    )
    .unwrap()
}
fn own_events(w: &World, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter(|e| matches!(e.vis, harpoon_core::events::Visibility::Side(Side::Blue)))
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule: r,
                side: Some(Side::Blue),
                unit: Some(EntityId(1)),
                detail,
                ..
            } if r == rule => Some(serde_json::from_str(detail).unwrap()),
            _ => None,
        })
        .collect()
}
#[test]
fn soks01_actual_old_submarine_wake_crossing_reports_recent_presence_without_current_sub_contact() {
    let mut w = world(150.);
    for _ in 0..210 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(
        w.entity(EntityId(2)).unwrap().pos.x > 0.4,
        "normal movement must create an actual past trail"
    );
    let detections = own_events(&w, "5.7.3.wake_detected");
    assert!(!detections.is_empty(),"source-valid same-band physical cut must use native SOKS, not leave only wake_probability helper");
    assert!(
        own_events(&w, "5.7.3.wake_course").is_empty(),
        "one cut does not grant direction"
    );
    assert!(
        w.find_contact(Side::Blue, EntityId(2)).is_none(),
        "old wake presence is not current submarine position/FCS"
    );
    for d in detections {
        assert!(d.get("wake_alias").is_some());
        assert!(d.get("truth_target").is_none());
        assert!(d.get("target").is_none());
        assert!(d.get("course_deg").is_none());
    }
}
#[test]
fn soks02_three_distinct_real_cuts_grant_rough_wake_course_but_wrong_depth_does_not() {
    for depth in [150., 250.] {
        let mut w = world(depth);
        let cmd = (
            Side::Blue,
            Command::Goto {
                unit: EntityId(1),
                waypoints: vec![
                    Vec2 { x: 0.1, y: 0.2 },
                    Vec2 { x: 0.5, y: 0.2 },
                    Vec2 { x: 0.5, y: -0.2 },
                    Vec2 { x: 0.9, y: -0.2 },
                    Vec2 { x: 0.9, y: 0.2 },
                ],
            },
        );
        harpoon_core::sim::tick(&mut w, &[cmd]);
        for _ in 1..550 {
            harpoon_core::sim::tick(&mut w, &[]);
        }
        assert!(
            own_events(&w, "5.7.3.wake_course").is_empty(),
            "two cuts cannot acquire course"
        );
        let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        for _ in 550..1100 {
            harpoon_core::sim::tick(&mut w, &[]);
            harpoon_core::sim::tick(&mut saved, &[]);
        }
        if depth == 150. {
            assert!(
                own_events(&w, "5.7.3.wake_detected").len() >= 3,
                "three actual separated cuts need three native successes"
            );
            let c = own_events(&w, "5.7.3.wake_course");
            assert!(
                !c.is_empty(),
                "third distinct success must earn historical rough course"
            );
            assert!((c[0]["course_deg"].as_f64().unwrap() - 90.).abs() < 1e-6);
        } else {
            assert!(own_events(&w, "5.7.3.wake_detected").is_empty());
            assert!(own_events(&w, "5.7.3.wake_course").is_empty());
        }
        assert!(w.find_contact(Side::Blue, EntityId(2)).is_none());
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            serde_json::to_value(&saved).unwrap()
        );
    }
}

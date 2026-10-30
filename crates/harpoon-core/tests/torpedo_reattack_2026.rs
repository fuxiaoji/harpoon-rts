//! Source PDF114/10-5 §10.2.7: complete homing-torpedo reattack chain —
//! 1000yd Past, right circle of the generation diameter, second Weapon
//! Placement reacquisition, and the next 10.2.5 attack. No ACM/evasion is
//! authored here, so this fixture does not prove quality degradation.
use harpoon_core::{
    load_scenario, Command, ContactId, Database, EventKind, Scenario, Side, Vec2, World,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

const TORPEDO_ID: u32 = 3;

fn roll(seed: u64, entity: u32, purpose: &str, sides: u8) -> u8 {
    let mut r =
        harpoon_core::Rng::new(seed).derive(harpoon_core::Rng::hash_tag(0, entity, purpose));
    1 + (r.next_f64() * sides as f64) as u8
}
fn d100(seed: u64, entity: u32, purpose: &str) -> u8 {
    roll(seed, entity, purpose, 100)
}
fn d10(seed: u64, purpose: &str) -> u8 {
    roll(seed, 0, purpose, 10)
}
fn attack_purpose(pass: u32) -> String {
    format!("10.2.5.attack:{pass}")
}
fn placement_purpose(pass: u32) -> String {
    format!("10.placement:{TORPEDO_ID}:{pass}")
}
fn seed_for(test: impl Fn(u64) -> bool) -> u64 {
    (1..=2_000_000)
        .find(|seed| test(*seed))
        .expect("pre-World native seed exists")
}

fn fixture(seed: u64) -> (World, harpoon_core::EntityId, harpoon_core::EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"torpedo-reattack","name":"10.2.7 reattack chain","duration_s":3000,"seed":seed,
        "context":{"rules_profile":"hv2026-rts-v4","torpedo_2026":{"source":"scenario_assumption: reattack chain mechanisms",
        "environment":{"source":"scenario_assumption: open water no layer","layer_m":null,"reverberant":false},
        "targets":{"cn_093":{"source":"scenario_assumption: controlled target","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
        "platforms":{"us_virginia":{"source":"scenario_assumption: loaded tubes","wire_capacity":2,"reload_parallel":2,
            "tubes":[{"bearing_deg":0,"loaded":"us_mk48"},{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}},
        "orders":{"SHOOTER:us_mk48":{"source":"scenario_assumption: declared order","speed_index":0,"depth_m":50,
            "enable_run_nm":0,"bearing_deg":null,"cut_wire":true,"spread_deg":[],
            "solution":{"quality":"Good","source":"scenario_assumption: fixture solution"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,5.0],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut w = load_scenario(sc, db).unwrap();
    let s = w
        .entities
        .values()
        .find(|e| e.name == "SHOOTER")
        .unwrap()
        .id;
    let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    w.entity_mut(t).unwrap().weapons.clear();
    assert_eq!(s.0, 1);
    assert_eq!(t.0, 2);
    w.sides[0].contacts.insert(
        ContactId(99),
        harpoon_core::world::Contact {
            id: ContactId(99),
            about: t,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 5.0),
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
        .insert("0:2".into(), harpoon_core::types::PlatformKind::Sub);
    (w, s, t)
}

fn launch(w: &mut World, s: harpoon_core::EntityId) -> harpoon_core::EntityId {
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: s,
                contact: ContactId(99),
                weapon: "us_mk48".into(),
                rounds: 1,
            },
        )],
    );
    let id = w
        .entities
        .values()
        .find(|e| e.kind == harpoon_core::types::PlatformKind::Torpedo)
        .expect("torpedo released")
        .id;
    assert_eq!(id.0, TORPEDO_ID, "fixture geometry fixes the torpedo id");
    id
}

fn details(w: &World, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RulesEvent {
                rule: r, detail, ..
            } if r == rule => Some(serde_json::from_str(detail).unwrap()),
            _ => None,
        })
        .collect()
}
fn attacks(w: &World) -> Vec<Value> {
    details(w, "10.2.5.source_terminal")
}
fn placements(w: &World) -> Vec<Value> {
    details(w, "10.2.4.placement")
}
fn flight<'a>(w: &'a World) -> &'a harpoon_core::rules::torpedo2026::Flight {
    w.rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .get(&harpoon_core::EntityId(TORPEDO_ID))
        .unwrap()
}
fn restored(w: &World) -> World {
    let copy: World = serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap();
    copy.rules.validate_tables().unwrap();
    copy
}

/// R1: first attack misses natively; the 1000yd Past runs without a placement
/// roll or heading change; the right circle sweeps back; the second Weapon
/// Placement die fires exactly once in the toward-target window; a placed
/// reacquisition goes straight to Locked and attacks again with the same
/// identity family.
#[test]
fn r1_first_miss_runs_past_then_circle_then_reacquires_and_attacks_again() {
    // Good solution vs no ACM/no evasion: base Ph .80; anechoic -2 keeps the
    // reacquisition chance at 9-2=7 (D10<=7 places).
    let seed = seed_for(|seed| {
        d10(seed, &placement_purpose(0)) <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(0)) > 80
            && d10(seed, &placement_purpose(1)) <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(1)) <= 80
    });
    let (mut w, s, _t) = fixture(seed);
    let torp = launch(&mut w, s);
    let mut placements_during_past: Vec<_> = vec![];
    let mut past_started = None;
    let mut past_heading = None;
    for _ in 0..3000 {
        if w.over {
            break;
        }
        let past = matches!(
            &flight(&w).phase,
            harpoon_core::rules::torpedo2026::Phase::Past { .. }
        );
        if past {
            if past_started.is_none() {
                past_started = Some(w.tick);
                past_heading = Some(w.entity(torp).unwrap().course_deg);
                placements_during_past = placements(&w)
                    .into_iter()
                    .filter(|p| p["pass"] == 1)
                    .collect();
            }
            // RTS-1: no heading change and no new placement roll during Past.
            assert!(
                (w.entity(torp).unwrap().course_deg - past_heading.unwrap()).abs() < 1e-9,
                "Past must run straight on the original course"
            );
        }
        harpoon_core::sim::tick(&mut w, &[]);
        if !w
            .rules
            .torpedoes
            .as_ref()
            .unwrap()
            .flights
            .contains_key(&torp)
        {
            break;
        }
    }
    assert!(
        past_started.is_some(),
        "first attack must miss natively and enter Past"
    );
    assert!(
        placements_during_past.is_empty(),
        "no reacquisition die may fire before the 1000yd Past completes"
    );
    let p = placements(&w);
    let reacq = p
        .iter()
        .find(|p| p["pass"] == 1)
        .expect("second Weapon Placement die must fire on the circle");
    assert_eq!(reacq["placement"]["placed"], true, "seed placed pass=1");
    assert_eq!(
        reacq["placement"]["d10"].as_u64().unwrap(),
        d10(seed, &placement_purpose(1)) as u64,
        "reacquisition keeps its original die"
    );
    let a = attacks(&w);
    assert_eq!(a.len(), 2, "initial attack plus reattack");
    assert_eq!(a[0]["pass"], 0);
    assert_eq!(a[0]["hit"], false);
    assert_eq!(a[1]["pass"], 1);
    assert_eq!(
        a[1]["d100"].as_u64().unwrap(),
        d100(seed, TORPEDO_ID, &attack_purpose(1)) as u64,
        "reattack keeps its original die identity"
    );
    assert_eq!(a[1]["hit"], true, "seed placed the second attack hit");
}

/// R2: a failed reacquisition keeps circling without deviation or a second
/// roll for the same pass, while the initial-placement-failure branch keeps
/// its existing course deviation behavior.
#[test]
fn r2_failed_reacquisition_keeps_circling_without_deviation_or_reroll() {
    let seed = seed_for(|seed| {
        d10(seed, &placement_purpose(0)) <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(0)) > 80
            && d10(seed, &placement_purpose(1)) > 7
    });
    let (mut w, s, _t) = fixture(seed);
    let torp = launch(&mut w, s);
    let mut reacq_seen = false;
    for _ in 0..2600 {
        if w.over {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
        if !reacq_seen && placements(&w).iter().any(|p| p["pass"] == 1) {
            reacq_seen = true;
        }
        if reacq_seen {
            // RTS-5: no course deviation may be applied on a failed
            // reacquisition; the circle continues on its own geometry.
            let f = flight(&w);
            match &f.phase {
                harpoon_core::rules::torpedo2026::Phase::Search { .. } => {}
                other => panic!("failed reacquisition must stay on the circle, got {other:?}"),
            }
            if f.fuel >= 1. {
                break;
            }
        }
    }
    assert!(reacq_seen, "reacquisition die must fire once on the circle");
    let p = placements(&w);
    assert_eq!(
        p.iter().filter(|p| p["pass"] == 1).count(),
        1,
        "identity cache prevents a second roll for the same pass"
    );
    let f = flight(&w);
    assert!(
        f.placement.as_ref().is_some_and(|p| !p.placed),
        "failed reacquisition is recorded with its original die"
    );
    assert!(
        f.fuel >= 1. || w.over,
        "the weapon keeps circling until fuel runs out"
    );
    let a = attacks(&w);
    assert_eq!(a.len(), 1, "no second attack without reacquisition");
}

/// R3/R4: without ACM/evasion the original Good quality remains unchanged;
/// anechoic -2 applies, and a second miss repeats with fresh identities.
/// ACM/evasion degradation and the conflicting Poor floor remain outside this case.
#[test]
fn r3_r4_no_acm_quality_stays_good_and_chain_repeats_with_fresh_identities() {
    // pass0 miss, pass1 reacquire then miss, pass2 reacquire then hit.
    let seed = seed_for(|seed| {
        d10(seed, &placement_purpose(0)) <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(0)) > 80
            && d10(seed, &placement_purpose(1)) <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(1)) > 80
            && d10(seed, &placement_purpose(2)) <= 6
            && d100(seed, TORPEDO_ID, &attack_purpose(2)) <= 80
    });
    let (mut w, s, _t) = fixture(seed);
    let torp = launch(&mut w, s);
    for _ in 0..3000 {
        if w.over {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
        if !w
            .rules
            .torpedoes
            .as_ref()
            .unwrap()
            .flights
            .contains_key(&torp)
        {
            break;
        }
    }
    let p = placements(&w);
    let q1 = p.iter().find(|p| p["pass"] == 1).expect("pass1 placement");
    let q2 = p.iter().find(|p| p["pass"] == 2).expect("pass2 placement");
    // No ACM and no evasion on this fixture: the solution stays Good (9-2=7).
    assert_eq!(q1["placement"]["chance"], 7);
    assert_eq!(q2["placement"]["chance"], 7, "no ACM/evasion keeps Good");
    let a = attacks(&w);
    assert_eq!(a.len(), 3, "attack + reattack + second reattack");
    assert_eq!(a[2]["pass"], 2);
    assert_eq!(a[2]["hit"], true);
    let identities: BTreeSet<String> = p
        .iter()
        .map(|p| p["placement"]["identity"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        identities.len(),
        3,
        "fresh placement identity per pass including the initial one"
    );
}

/// R5: placement and terminal dice stay Referee-only; the own observation
/// never carries the reacquisition roll, chance or circle geometry dice.
#[test]
fn r5_reattack_dice_remain_referee_private() {
    let seed = seed_for(|seed| {
        d10(seed, &placement_purpose(0)) <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(0)) > 80
            && d10(seed, &placement_purpose(1)) <= 7
    });
    let (mut w, s, _t) = fixture(seed);
    let torp = launch(&mut w, s);
    for _ in 0..900 {
        if w.over {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
        if placements(&w).iter().any(|p| p["pass"] == 1) {
            break;
        }
    }
    let _ = torp;
    for rule in [
        "10.2.4.placement",
        "10.2.5.source_terminal",
        "10.2.7.circle",
    ] {
        let events: Vec<_> = w
            .events
            .iter()
            .filter(|e| matches!(&e.kind, EventKind::RulesEvent { rule: r, .. } if r == rule))
            .collect();
        if rule != "10.2.7.circle" {
            assert!(!events.is_empty(), "privacy must inspect actual {rule} events");
        }
        for event in events {
            assert!(
                matches!(event.vis, harpoon_core::Visibility::Referee),
                "{rule} must stay Referee"
            );
        }
        assert!(
            !serde_json::to_string(&harpoon_core::observe(&w, Side::Blue, 0, 10_000))
                .unwrap()
                .contains(rule),
            "{rule} must not appear in the own observation"
        );
    }
}

/// R6: complete-World restore during Past and on the circle reproduces the
/// continuation byte-for-byte with original dice retained.
#[test]
fn r6_restore_during_past_and_circle_keeps_physics_and_dice() {
    let seed = seed_for(|seed| {
        d10(seed, &placement_purpose(0)) <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(0)) > 80
            && d10(seed, &placement_purpose(1)) <= 7
    });
    let (mut w, s, _t) = fixture(seed);
    let torp = launch(&mut w, s);
    while !matches!(
        &flight(&w).phase,
        harpoon_core::rules::torpedo2026::Phase::Past { .. }
    ) {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let base = restored(&w);
    let mut on_circle = restored(&base);
    while !matches!(
        &flight(&on_circle).phase,
        harpoon_core::rules::torpedo2026::Phase::Search { .. }
    ) {
        harpoon_core::sim::tick(&mut on_circle, &[]);
    }
    for _ in 0..12 {
        let mut copy = restored(&on_circle);
        harpoon_core::sim::tick(&mut on_circle, &[]);
        harpoon_core::sim::tick(&mut copy, &[]);
        assert_eq!(
            serde_json::to_value(&on_circle).unwrap(),
            serde_json::to_value(&copy).unwrap(),
            "circle continuation must be deterministic"
        );
    }
    let mut reference = restored(&base);
    while reference
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .contains_key(&torp)
        && !reference.over
    {
        harpoon_core::sim::tick(&mut reference, &[]);
    }
    let mut branch = restored(&on_circle);
    while branch
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .contains_key(&torp)
        && !branch.over
    {
        harpoon_core::sim::tick(&mut branch, &[]);
    }
    assert_eq!(
        serde_json::to_value(&branch).unwrap(),
        serde_json::to_value(&reference).unwrap(),
        "restore on the circle must reach the same terminal state"
    );
    let a = attacks(&branch);
    assert!(!a.is_empty());
}

//! Independent regression candidates. NOT REGISTERED OR EXECUTED.
//! Author Good and one located original contact isolate post-release physics;
//! no contact/FCS/acquisition success is claimed from native sonar.
//! Native RNG, Fire, Past, Search, Placement, pursuit, damage, and observation
//! paths remain untouched. No Flight phase, die, opportunity, or jam is set.
use harpoon_core::{
    load_scenario, Command, ContactId, Database, EventKind, Scenario, Side, Vec2, World,
};
use serde_json::{json, Value};

const TORPEDO_ID: u32 = 5;

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
    let db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
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
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,5.0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"OTHER_OPPORTUNITY","pos":[0.8,5.5],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Blue","class":"us_burke_2a","name":"FAR_BLUE_RETAINER","pos":[300,300],"speed_kts":0,"fire_on":false}]})).unwrap();
    let mut w = load_scenario(sc, db).unwrap();
    let s = w
        .entities
        .values()
        .find(|e| e.name == "SHOOTER")
        .unwrap()
        .id;
    let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    // These author fixtures isolate post-release mechanics. The only injected
    // located contact is the original TARGET; it is not native sonar/FCS.
    // OTHER remains absent from Blue contacts and never creates its own ACM.
    for e in w.entities.values_mut() {
        e.sensors.clear();
        if e.side == Side::Red {
            e.weapons.clear();
        }
    }
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

fn candidate_events(w: &World, rule: &str) -> Vec<Value> {
    details(w, rule)
}

fn own_unknown(w: &World, shooter: harpoon_core::EntityId, other: harpoon_core::EntityId) {
    assert!(
        w.sides[Side::Blue.index()]
            .contacts
            .values()
            .all(|c| c.about != other),
        "source opportunity capture does not mint a Blue contact"
    );
    let view = harpoon_core::rules::torpedo2026::own_view(w, Side::Blue, shooter)
        .expect("live own launch record remains visible");
    let releases = view["releases"].as_array().expect("own release array");
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0]["status"], "unknown");
    assert!(releases[0]["physical_alive"].is_null());
    assert!(releases[0]["reported_status"].is_null());
    for private in [
        "opportunity_acquired",
        "opportunity_placement",
        "source_terminal",
        "OTHER_OPPORTUNITY",
        "10.placement:5:1:op:3",
    ] {
        assert!(
            !view.to_string().contains(private),
            "own release leaks {private}"
        );
    }
}

/// The old O2 label claimed an entity but its fixture actually created two
/// MOSS objects. This case has one ordinary real entity and no EW objects.
/// It must acquire, move after capture, and hit through production damage.
#[test]
fn independent_entity_opportunity_uses_native_entity_pursuit_and_damage() {
    let seed = seed_for(|seed| {
        d10(seed, &placement_purpose(0)) <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(0)) > 80
            && d10(seed, &placement_purpose(1)) > 7
            && d10(seed, "10.placement:5:1:op:3") <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(1)) <= 80
    });
    let (mut w, shooter, original) = fixture(seed);
    let other = w
        .entities
        .values()
        .find(|e| e.name == "OTHER_OPPORTUNITY")
        .unwrap()
        .id;
    assert_eq!(other.0, 3);
    assert!(w.rules.ew.as_ref().unwrap().objects.is_empty());
    let original_hp = w.entity(original).unwrap().hp;
    let other_hp = w.entity(other).unwrap().hp;
    let torpedo = launch(&mut w, shooter);
    let mut saw_past = false;
    let mut captured_at = None;
    for _ in 0..1600 {
        if matches!(
            flight(&w).phase,
            harpoon_core::rules::torpedo2026::Phase::Past { .. }
        ) {
            saw_past = true;
        }
        if !candidate_events(&w, "10.2.7.opportunity_acquired").is_empty() {
            captured_at = Some(w.tick);
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
        if !w.entity(torpedo).is_some_and(|e| e.alive) {
            break;
        }
    }
    assert!(saw_past, "native initial miss enters an actual Past leg");
    assert_eq!(
        attacks(&w).len(),
        1,
        "first real attack only before opportunity capture"
    );
    assert_eq!(attacks(&w)[0]["hit"], false);
    assert!(
        captured_at.is_some(),
        "unique ordinary entity must be captured: {:?}",
        w.rules.pending_adjudications
    );
    let ops = candidate_events(&w, "10.2.7.opportunity_acquired");
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0]["kind"], "entity");
    assert_eq!(ops[0]["candidate"], 3);
    own_unknown(&w, shooter, other);
    let at_capture = w.entity(torpedo).unwrap().pos;
    // Exercise persistent takeover through the public command, never an ext write.
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetUnitAutomation {
                unit: shooter,
                enabled: false,
            },
        )],
    );
    assert!(
        !w.rules.pending_adjudications.contains_key(&torpedo),
        "ordinary entity capture must not enter the MOSS generation error"
    );
    assert_ne!(
        w.entity(torpedo).unwrap().pos,
        at_capture,
        "captured entity pursuit must move on the very next tick"
    );
    assert_eq!(
        w.entity(shooter).unwrap().ext.get("control.manual"),
        Some(&1.)
    );
    // Branch with the launcher physically removed: a cut-wire torpedo's
    // private owner/release ledger must suffice for autonomous pursuit.
    let mut no_launcher = restored(&w);
    no_launcher.entities.remove(&shooter);
    for _ in 0..1200 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut no_launcher, &[]);
        if !w.entity(torpedo).is_some_and(|e| e.alive)
            && !no_launcher.entity(torpedo).is_some_and(|e| e.alive)
        {
            break;
        }
    }
    for branch in [&w, &no_launcher] {
        assert!(
            !branch.rules.pending_adjudications.contains_key(&torpedo),
            "a qualified autonomous entity opportunity must execute"
        );
        let terminals = attacks(branch);
        assert_eq!(
            terminals.len(),
            2,
            "first miss plus one native opportunity attack"
        );
        assert_eq!(terminals[1]["pass"], 1);
        assert_eq!(terminals[1]["hit"], true);
        assert_eq!(
            terminals[1]["target"], other.0,
            "the native attack is against the captured entity"
        );
        let damage: f64 = branch
            .events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::Impact { unit, damage, .. } if *unit == other => Some(*damage),
                _ => None,
            })
            .sum();
        assert!(
            damage > 0.,
            "actual production damage must be nonempty even if the sunk entity was cleaned up"
        );
        if let Some(body) = branch.entity(other) {
            assert!(body.hp < other_hp);
        } else {
            assert!(
                branch
                    .events
                    .iter()
                    .any(|e| matches!(&e.kind,EventKind::Sunk{unit,..}if *unit==other)),
                "missing body needs a real native sunk event"
            );
        }
        assert_eq!(
            branch.entity(original).unwrap().hp,
            original_hp,
            "original target never hit"
        );
        for event in &branch.events {
            if matches!(&event.kind, EventKind::RulesEvent {rule,..}
                if rule == "10.2.4.opportunity_placement" || rule == "10.2.7.opportunity_acquired"
                || rule == "10.2.5.source_terminal")
            {
                assert!(matches!(event.vis, harpoon_core::Visibility::Referee));
            }
        }
    }
    own_unknown(&w, shooter, other);
    assert_eq!(
        w.entity(other).map(|e| e.hp),
        no_launcher.entity(other).map(|e| e.hp),
        "launcher removal must not change source attack or damage"
    );
}

/// The old initial_window also admitted pass>0 and bypassed the declared
/// RTS-3 <=60deg window. Check every native pre-tick state after real Past.
#[test]
fn independent_reacquisition_waits_for_declared_sixty_degree_circle_window() {
    let seed = seed_for(|seed| {
        d10(seed, &placement_purpose(0)) <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(0)) > 80
            && d10(seed, &placement_purpose(1)) <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(1)) <= 80
    });
    let (mut w, shooter, original) = fixture(seed);
    let torpedo = launch(&mut w, shooter);
    let mut past_distance = 0.;
    let mut saw_past = false;
    for _ in 0..1600 {
        if matches!(
            flight(&w).phase,
            harpoon_core::rules::torpedo2026::Phase::Past { .. }
        ) {
            saw_past = true;
            let before = w.entity(torpedo).unwrap().pos;
            let heading = w.entity(torpedo).unwrap().course_deg;
            let dice_before = placements(&w).iter().filter(|p| p["pass"] == 1).count();
            harpoon_core::sim::tick(&mut w, &[]);
            past_distance += before.range_to(w.entity(torpedo).unwrap().pos);
            assert_eq!(w.entity(torpedo).unwrap().course_deg, heading);
            assert_eq!(
                placements(&w).iter().filter(|p| p["pass"] == 1).count(),
                dice_before
            );
            if matches!(
                flight(&w).phase,
                harpoon_core::rules::torpedo2026::Phase::Search { .. }
            ) {
                break;
            }
        } else {
            harpoon_core::sim::tick(&mut w, &[]);
        }
    }
    assert!(saw_past);
    assert!(
        past_distance + 1e-9 >= 1000. * 0.000493737,
        "Past must physically travel source 1000 yards before Search: {past_distance}"
    );
    let mut saw_outside = false;
    let mut rolled_in_window = false;
    for _ in 0..800 {
        let body = w.entity(torpedo).unwrap();
        let target = w.entity(original).unwrap();
        let off_axis =
            harpoon_core::geom::angle_diff(body.course_deg, body.pos.bearing_to(target.pos)).abs();
        let before = placements(&w).iter().filter(|p| p["pass"] == 1).count();
        harpoon_core::sim::tick(&mut w, &[]);
        let after = placements(&w).iter().filter(|p| p["pass"] == 1).count();
        if off_axis > 60. + 1e-9 {
            saw_outside = true;
            assert_eq!(
                after, before,
                "pass>0 must not reuse initial_window while off-axis is {off_axis}"
            );
            assert!(
                matches!(
                    flight(&w).phase,
                    harpoon_core::rules::torpedo2026::Phase::Search { .. }
                ),
                "unqualified circle position must not become Locked"
            );
        } else if after == before + 1 {
            rolled_in_window = true;
            break;
        }
    }
    assert!(saw_outside, "fixture exercises the away-facing arc");
    assert!(
        rolled_in_window,
        "one native placement fires only in the <=60deg window"
    );
    assert_eq!(placements(&w).iter().filter(|p| p["pass"] == 1).count(), 1);
}

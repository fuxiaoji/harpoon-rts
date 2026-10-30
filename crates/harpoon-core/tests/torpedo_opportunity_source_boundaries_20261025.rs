//! Independent regression candidates. NOT REGISTERED OR EXECUTED.
//! Author Good and one located original contact isolate post-release physics;
//! no contact/FCS/acquisition success is claimed from native sonar.
//! Native RNG, Fire, Past, Search, Placement, pursuit, damage, and observation
//! paths remain untouched. No Flight phase, die, opportunity, or jam is set.
use harpoon_core::{
    load_scenario, Command, ContactId, Database, EventKind, Scenario, Side, Vec2, World,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

const TORPEDO_ID: u32 = 4;

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
        "targets":{"cn_093":{"source":"scenario_assumption: controlled original target","size":2,"noise":2,"anechoic":true,"foil_borne":false},"cn_094":{"source":"scenario_assumption: independent opportunity profile","size":2,"noise":2,"anechoic":false,"foil_borne":false}},
        "platforms":{"us_virginia":{"source":"scenario_assumption: loaded tubes","wire_capacity":2,"reload_parallel":2,
            "tubes":[{"bearing_deg":0,"loaded":"us_mk48"},{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}},
        "orders":{"SHOOTER:us_mk48":{"source":"scenario_assumption: declared order","speed_index":0,"depth_m":50,
            "enable_run_nm":0,"bearing_deg":null,"cut_wire":true,"spread_deg":[],
            "solution":{"quality":"Good","source":"scenario_assumption: fixture solution"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,5.0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_094","name":"OTHER_OPPORTUNITY","pos":[0.8,5.5],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
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

fn first_opportunity_seed() -> u64 {
    seed_for(|seed| {
        d10(seed, &placement_purpose(0)) <= 7
            && d100(seed, TORPEDO_ID, &attack_purpose(0)) > 80
            && d10(seed, &placement_purpose(1)) > 7
            && d10(seed, "10.placement:4:1:op:3") == 9
            && d100(seed, TORPEDO_ID, &attack_purpose(1)) <= 80
    })
}

fn other_id(w: &World) -> harpoon_core::EntityId {
    w.entities
        .values()
        .find(|e| e.name == "OTHER_OPPORTUNITY")
        .unwrap()
        .id
}

fn continue_until_capture_or_pending(w: &mut World, torpedo: harpoon_core::EntityId) {
    for _ in 0..2000 {
        harpoon_core::sim::tick(w, &[]);
        if !details(w, "10.2.7.opportunity_acquired").is_empty()
            || w.rules.pending_adjudications.contains_key(&torpedo)
            || !w.entity(torpedo).is_some_and(|e| e.alive)
        {
            break;
        }
    }
}

#[test]
fn opportunity_uses_candidate_coating_not_original_coating() {
    let (mut w, shooter, original) = fixture(first_opportunity_seed());
    let other = other_id(&w);
    let before = w.entity(other).unwrap().hp;
    let original_hp = w.entity(original).unwrap().hp;
    let torpedo = launch(&mut w, shooter);
    continue_until_capture_or_pending(&mut w, torpedo);
    let ops = details(&w, "10.2.4.opportunity_placement");
    assert_eq!(
        ops.len(),
        1,
        "unique ordinary candidate must get its own Placement once"
    );
    assert_eq!(ops[0]["placement"]["d10"], 9);
    assert_eq!(
        ops[0]["placement"]["chance"], 9,
        "original coated -2 must not leak into the uncoated opportunity"
    );
    assert_eq!(ops[0]["placement"]["placed"], true);
    assert_eq!(ops[0]["inputs"]["profile"]["anechoic"], false);
    assert_eq!(
        flight(&w).target,
        original,
        "historical release target is immutable"
    );
    assert_eq!(
        flight(&w).placement.as_ref().unwrap().chance,
        7,
        "original failed source Placement remains unchanged"
    );
    let capture = flight(&w)
        .opportunity
        .as_ref()
        .unwrap()
        .capture
        .as_ref()
        .unwrap();
    let capture_tick = w.tick;
    assert_eq!(capture.captured_at, capture_tick);
    assert_eq!(capture.pursuit_started_at, None);
    assert_eq!(flight(&w).speed_index, capture.search_speed_index);
    let mut observed_before_attack = false;
    for _ in 0..800 {
        if !w.entity(torpedo).is_some_and(|e| e.alive) {
            break;
        }
        let target = w.entity(other).unwrap();
        let body = w.entity(torpedo).unwrap();
        let flight = flight(&w);
        let setting = &flight.weapon.settings[flight
            .opportunity
            .as_ref()
            .unwrap()
            .capture
            .as_ref()
            .unwrap()
            .attack_speed_index];
        let affordable = (setting.speed_kts / 3600.).min((1. - flight.fuel) * setting.range_nm);
        let distance = body.pos.range_to(target.pos);
        let old = attacks(&w).len();
        harpoon_core::sim::tick(&mut w, &[]);
        if distance > affordable + 1e-9 {
            observed_before_attack = true;
            assert_eq!(
                attacks(&w).len(),
                old,
                "cannot hit from farther than the whole tick's affordable travel"
            );
        }
    }
    assert!(observed_before_attack);
    assert_eq!(
        attacks(&w).len(),
        2,
        "first miss and exactly one source entity hit"
    );
    assert_eq!(attacks(&w)[1]["hit"], true);
    assert!(w.entity(other).unwrap().hp < before);
    assert_eq!(w.entity(original).unwrap().hp, original_hp);
}

#[test]
fn unknown_candidate_source_blocks_without_candidate_die() {
    let (mut w, shooter, _) = fixture(first_opportunity_seed());
    // Author fixture variation occurs before launch; no success state is set.
    w.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .config
        .targets
        .remove("cn_094");
    let torpedo = launch(&mut w, shooter);
    continue_until_capture_or_pending(&mut w, torpedo);
    assert!(
        w.rules.pending_adjudications.contains_key(&torpedo),
        "unknown candidate in the source cone/range must not inherit original profile"
    );
    assert!(
        details(&w, "10.2.4.opportunity_placement").is_empty(),
        "no die until all competition is qualified"
    );
    assert!(details(&w, "10.2.7.opportunity_acquired").is_empty());
}

#[test]
fn friendly_ordinary_candidate_is_not_filtered_by_hidden_iff() {
    let (mut w, shooter, _) = fixture(first_opportunity_seed());
    let other = other_id(&w);
    // An explicit author-side variation before any source launch.
    w.entity_mut(other).unwrap().side = Side::Blue;
    let torpedo = launch(&mut w, shooter);
    continue_until_capture_or_pending(&mut w, torpedo);
    let acquired = details(&w, "10.2.7.opportunity_acquired");
    assert_eq!(
        acquired.len(),
        1,
        "physical seeker has no inferred friend/foe immunity"
    );
    assert_eq!(acquired[0]["candidate"], other.0);
    assert_eq!(acquired[0]["kind"], "entity");
    assert!(!w.rules.pending_adjudications.contains_key(&torpedo));
}

#[test]
fn foil_borne_candidate_is_source_excluded_not_captured() {
    let (mut w, shooter, _) = fixture(first_opportunity_seed());
    w.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .config
        .targets
        .get_mut("cn_094")
        .unwrap()
        .foil_borne = true;
    let torpedo = launch(&mut w, shooter);
    for _ in 0..2000 {
        harpoon_core::sim::tick(&mut w, &[]);
        if !w.entity(torpedo).is_some_and(|e| e.alive) {
            break;
        }
    }
    assert!(details(&w, "10.2.4.opportunity_placement").is_empty());
    assert!(details(&w, "10.2.7.opportunity_acquired").is_empty());
    assert!(
        !details(&w, "10.2.source_runout").is_empty(),
        "failed original reacquisition keeps searching to real runout"
    );
}

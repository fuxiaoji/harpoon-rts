//! §3.3/3.4 first source candidates: real motion and common damage only.
//! Author-labelled navigation/shore/armor fits, no private contact/HP/dice/ground flag writes.
use harpoon_core::rules::navigation2026 as n;
use harpoon_core::{Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
fn fit(size: &str) -> Value {
    json!({"source":"scenario_assumption: explicit controlled navigation fit; no real CPP/readiness inference",
    "size":size,"merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,
    "undamaged_max_kts":30,"propulsion":"NonSteam","readiness":"Underway"})
}
fn hull() -> Value {
    json!({"source":"scenario_assumption: explicit no-armor/no-reinforced-bow/no-civilian controlled hull",
    "civilian":false,"armor_points":0,"reinforced_bow":false})
}
fn collision_seed() -> u64 {
    (0..100000)
        .find(|s| {
            let mut r = Rng::new(*s).derive(Rng::hash_tag(1, 1, "3.3.contact.1:2.0"));
            let a = (r.next_f64() * 10.).floor() as u8 + 1;
            let b = (r.next_f64() * 10.).floor() as u8 + 1;
            a + b >= 16
        })
        .unwrap()
}
fn grounding_seed() -> u64 {
    (0..100000)
        .find(|s| {
            let mut r = Rng::new(*s).derive(Rng::hash_tag(180, 1, "3.4.grounding.0"));
            (r.next_f64() * 100.).floor() as u8 + 1 <= 30
        })
        .unwrap()
}
fn value(collision: bool) -> Value {
    let mut v = json!({"id":"wide-collision-grounding","name":"scenario_assumption: source physical movement hazard experiment",
      "seed":if collision{collision_seed()}else{grounding_seed()},"duration_s":40000,"weather":{"visibility_nm":40,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
        "navigation_2026":{"source":"scenario_assumption: no guessed real hull equipment",
          "units":{"A":fit("C")},"hazards":{"source":"English2026 PDF30-32 §3.3/3.4; explicit experiment geometry and armor",
            "collisions":collision,"visibility_percent":100,"units":{"A":hull()},
            "grounding":if collision{Value::Null}else{json!({"source":"scenario_assumption: straight declared shore; no bathymetry or draft inference",
              "shorelines":[[[-0.05,-5],[-0.05,5]]],"safe_areas":[]})}}}},
      "units":[{"side":"Blue","class":"us_burke_2a","name":"A","pos":[0,0],"course_deg":0,"speed_kts":5,"fire_on":false},
        {"side":"Red","class":"cn_039a","name":"DISTANT","pos":[50,50],"course_deg":0,"speed_kts":0,"depth_m":100,"fire_on":false}]});
    if collision {
        v["units"][0]["pos"] = json!([0, -0.0625]);
        v["units"][0]["speed_kts"] = json!(10);
        v["units"].as_array_mut().unwrap().insert(1,json!({"side":"Blue","class":"us_burke_2a","name":"B","pos":[0,0.0625],"course_deg":180,"speed_kts":10,"fire_on":false}));
        v["context"]["navigation_2026"]["units"]["B"] = fit("C");
        v["context"]["navigation_2026"]["hazards"]["units"]["B"] = hull();
    }
    v
}
fn load(v: Value) -> World {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    for class in ["us_burke_2a", "cn_039a"] {
        let p = db.platforms.get_mut(class).unwrap();
        p.weapons.clear();
        p.sensors.clear();
    }
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(v).unwrap(), db).unwrap()
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn run(w: &mut World, to: i64) {
    while w.tick < to {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn events(w: &World, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule: r, detail, ..
            } if r == rule => Some(serde_json::from_str(detail).unwrap()),
            _ => None,
        })
        .collect()
}
// New following boundary candidates; the original four remain unchanged.
fn bow_value() -> Value {
    let mut v = value(true);
    v["units"][1]["course_deg"] = json!(90);
    v["units"][1]["speed_kts"] = json!(0);
    v["context"]["navigation_2026"]["hazards"]["joined_speed_policy"] = json!("faster");
    v
}
#[test]
fn h01_actual_bow_on_contact_commits_both_damage_and_persists_joint_motion_without_enemy_truth() {
    let mut w = load(bow_value());
    let (a, b) = (id(&w, "A"), id(&w, "B"));
    let hp = [w.entity(a).unwrap().hp, w.entity(b).unwrap().hp];
    run(&mut w, 1);
    let hit = events(&w, "3.3.collision");
    assert_eq!(hit.len(), 1);
    assert_eq!(hit[0]["collision_type"], "bow_on");
    assert!(w.entity(a).unwrap().hp < hp[0] && w.entity(b).unwrap().hp < hp[1]);
    let own = n::own_view(&w, a).unwrap();
    assert_eq!(own["hazards"]["bow_maximum_fraction"], 0.75);
    let joined = &own["hazards"]["joined_contact"];
    assert_eq!(joined["motion_pending"], false);
    assert!(joined.get("other").is_none());
    assert!(joined.get("class").is_none());
    assert!(joined.get("armor_points").is_none());
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    run(&mut w, 3);
    run(&mut restored, 3);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert_eq!(events(&w, "3.3.collision").len(), 1);
}
#[test]
fn h02_unknown_recipient_armor_keeps_damage_pending_but_applies_source_known_bow_constraint() {
    let mut v = bow_value();
    v["context"]["navigation_2026"]["hazards"]["units"]["B"]["armor_points"] = Value::Null;
    let mut w = load(v);
    let (a, b) = (id(&w, "A"), id(&w, "B"));
    let hp = [w.entity(a).unwrap().hp, w.entity(b).unwrap().hp];
    run(&mut w, 1);
    assert_eq!(
        [w.entity(a).unwrap().hp, w.entity(b).unwrap().hp],
        hp,
        "unknown armor is not treated as zero or fabricated damage"
    );
    let own = n::own_view(&w, a).unwrap();
    assert_eq!(own["hazards"]["bow_maximum_fraction"], 0.75);
    assert_eq!(own["hazards"]["joined_contact"]["motion_pending"], true);
    assert_eq!(events(&w, "3.3.damage_rolls").len(), 1);
    assert!(events(&w, "3.hazard_pending")
        .iter()
        .any(|v| v["reason"].as_str().unwrap().contains("armor")));
}
#[test]
fn h03_source_small_hulls_use_actual_100_yard_envelope_instead_of_250() {
    let mut v = value(true);
    v["context"]["navigation_2026"]["units"]["A"] = fit("F");
    v["context"]["navigation_2026"]["units"]["B"] = fit("F");
    let mut w = load(v);
    run(&mut w, 1);
    assert!(events(&w, "3.3.encounter").is_empty());
    run(&mut w, 20);
    let trials = events(&w, "3.3.encounter");
    assert_eq!(trials.len(), 1);
    assert!(trials[0]["minimum_distance_yards"].as_f64().unwrap() <= 100.);
}
#[test]
fn h04_unknown_visibility_does_not_create_a_guessed_accidental_hit() {
    let mut v = value(true);
    v["context"]["navigation_2026"]["hazards"]["visibility_percent"] = Value::Null;
    let mut w = load(v);
    let a = id(&w, "A");
    let hp = w.entity(a).unwrap().hp;
    run(&mut w, 1);
    assert_eq!(w.entity(a).unwrap().hp, hp);
    assert!(events(&w, "3.3.encounter").is_empty());
    assert!(events(&w, "3.hazard_pending")
        .iter()
        .any(|v| v["reason"].as_str().unwrap().contains("visibility")));
}

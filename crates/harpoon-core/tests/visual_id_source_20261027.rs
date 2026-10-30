//! Artifact-only5.8.9 optional graded visual ID source consumer.
//! Normal native Visual scans and source-created contact; no Contact/dice writes.
use harpoon_core::rules::detection2026 as d;
use harpoon_core::{Database, EntityId, Rng, Scenario, Side, World};
use serde_json::{json, Value};
fn original_dice(seed: u64) -> (u8, u8) {
    let mut detect = Rng::new(seed).derive(Rng::hash_tag(0, 0, "5.8.visual:1:visual"));
    let mut identify = Rng::new(seed).derive(Rng::hash_tag(0, 0, "5.8.9.visual:1:visual:2"));
    (
        1 + (detect.next_f64() * 100.) as u8,
        1 + (identify.next_f64() * 10.) as u8,
    )
}
fn master_seed() -> u64 {
    (0..1_000_000)
        .find(|s| {
            let (d100, d10) = original_dice(*s);
            (51..=75).contains(&d100) && d10 <= 2
        })
        .unwrap()
}
fn scenario(optional: bool) -> Value {
    json!({"id":"source-optional-visual-identification","name":"English2026 5.8.9 actual Visual identification source boundary","seed":master_seed(),"duration_s":600,
      "weather":{"visibility_nm":40,"jam":[0,0]},"day_night":"Day",
      "context":{"rules_profile":"hv2026-rts-v4","detection_2026":{
        "visual_identification":{"source":"English2026 PDF67/5-28 and68/5-29 optional5.8.9; explicitly enabled author trial","enabled":optional},
        "bindings":{"us_burke_2a:visual":{"type":"visual","source":"scenario_assumption: actual sole fitted clear daylight lookout"}},
        "targets":{
          "us_burke_2a":{"source":"scenario_assumption: C observer signature for source horizon experiment, not a reclassified real Burke hull","size":2,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2},
          "cn_001":{"source":"China2025 source A ship; isolated native daylight Visual target","size":0,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":0}},
        "environment":{"source":"scenario_assumption: clear daylight range,100percentvisibility, no cloud/precipitation; no close-contact bonus at4nm","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
      "units":[{"side":"Blue","class":"us_burke_2a","name":"LOOK","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false},
         {"side":"Red","class":"cn_001","name":"TARGET","pos":[0,4],"course_deg":0,"speed_kts":0,"fire_on":false}]})
}
fn world(optional: bool) -> World {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    for class in ["us_burke_2a", "cn_001"] {
        let p = db.platforms.get_mut(class).unwrap();
        p.weapons.clear();
        p.sensors.clear();
        p.deck = None;
    }
    db.platforms.get_mut("us_burke_2a").unwrap().sensors = vec!["visual".into()];
    harpoon_core::load_scenario(
        serde_json::from_value::<Scenario>(scenario(optional)).unwrap(),
        db,
    )
    .unwrap()
}
fn native_prerequisite(w: &World) {
    let (detected, _) = original_dice(w.seed);
    let state = w.rules.detection.as_ref().unwrap();
    assert_eq!(
        state.trials["5.8.visual:1:visual"].d100, detected,
        "actual normal lookout detection uses the preselected original source die"
    );
    assert!(w.rules.aaw.as_ref().unwrap().firecontrol.tracks.contains_key("1:visual:2"),"this observer must have a real native measured Visual report; side icon alone is insufficient");
    let cid = w.find_contact(Side::Blue, EntityId(2)).expect(
        "normal Visual must create an actual native side contact before classification is tested",
    );
    let c = &w.sides[Side::Blue.index()].contacts[&cid];
    assert!(!c.stale && c.sensors.iter().any(|s| s == "visual"));
    assert!(c.estimate.range_to(w.entity(EntityId(2)).unwrap().pos) < 1e-9);
}
#[test]
fn visual_id01_optional_ship_only_result_must_withhold_class_while_basic_rule_still_identifies() {
    let mut basic = world(false);
    harpoon_core::sim::tick(&mut basic, &[]);
    native_prerequisite(&basic);
    let cid = basic.find_contact(Side::Blue, EntityId(2)).unwrap();
    assert!(
        basic.sides[Side::Blue.index()].contacts[&cid]
            .class_name
            .is_some(),
        "Basic5.8.9 retains ordinary immediate Visual class identification"
    );
    let mut graded = world(true);
    harpoon_core::sim::tick(&mut graded, &[]);
    native_prerequisite(&graded);
    let (d100, d10) = original_dice(graded.seed);
    assert!((51..=75).contains(&d100));
    assert!(d10 <= 2);
    let t = d::tables();
    assert_eq!(t["visual"]["identification_detection_modifiers"][3], 0);
    assert_eq!(
        t["visual"]["classification_hull_up"][usize::from(d10 - 1)],
        0,
        "the visually verified HullUp row1/2 grants Ship only, not type/class/nationality"
    );
    //4nm is beyond both strict close thresholds (<4nm / <8kyd), within the
    // source C-observer100percent17kyd horizon, and clear daylight adds no
    // environmental penalty. There is no source reason to alter the low D10.
    let cid = graded.find_contact(Side::Blue, EntityId(2)).unwrap();
    assert!(graded.sides[Side::Blue.index()].contacts[&cid].class_name.is_none(),
       "explicit optional5.8.9 Ship-only result must not inherit Basic automatic class truth from a successful native Visual scan");
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&graded).unwrap()).unwrap();
    for _ in 0..4 {
        harpoon_core::sim::tick(&mut graded, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
        let c = graded.find_contact(Side::Blue, EntityId(2)).unwrap();
        assert!(graded.sides[Side::Blue.index()].contacts[&c].class_name.is_none(),"repeated normal scan within one declared RTS identification epoch must not grant free new D10/class");
    }
    assert_eq!(
        serde_json::to_value(&graded).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}

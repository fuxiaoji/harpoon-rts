//! English2026 §§13.5/13.6 -> actual primary DP ->14.2.2/14.2.3.
//! The initially laid field, target acoustic fit and route are explicit author
//! scenario inputs. Source MDM-1 attack/damage cells are unchanged. These cases
//! prove real movement/trigger/damage, not a newly deployed native minelayer.
//! No mine hit, HP, Flight, signature state or die is injected.
use harpoon_core::rules::{
    damage_runtime as damage, mines2026 as mines, signature_damage2026 as signature,
};
use harpoon_core::{Database, EntityId, EventKind, Rng, Scenario, Side, Visibility, World};
use serde_json::{json, Value};
use std::sync::OnceLock;

fn database() -> Database {
    static DB: OnceLock<Database> = OnceLock::new();
    DB.get_or_init(|| {
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
    })
    .clone()
}

fn native_seed() -> u64 {
    // Real first field crossing identity/target, selected before creating World.
    // Size B magnetic+1/acoustic at30kt-1 -> B; spacing125 source Ph=.36.
    (1..10_000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(0, 2, "mine:signature-field:2:1"));
            1 + (r.next_f64() * 100.).floor() as u8 <= 36
        })
        .expect("a natural successful first source mine attack seed exists")
}

fn fixture(noise: Option<usize>, armed_at: i64) -> (World, EntityId) {
    let mut db = database();
    for class in ["us_virginia", "us_burke_2a"] {
        let p = db.platforms.get_mut(class).unwrap();
        p.weapons.clear();
        p.sensors.clear();
        p.rule_data = Some(damage::PlatformDamage {
            size: if class == "us_burke_2a" { "B" } else { "C" }.into(),
            role: "surface".into(),
            source: "scenario_assumption: explicit laboratory Size and warship/zero-armor fit; original loaded hull DP unchanged".into(),
            ..Default::default()
        });
    }
    let field: mines::Field=serde_json::from_value(json!({
        "id":"signature-field","owner":"Red","mine":"mdm_1",
        "area":{"start":{"x":0,"y":0},"end":{"x":5000.0/2025.3718285,"y":0},"width_yd":1200},
        "rows":3,"spacing_yd":125,"row_spacing_yd":300,"case_band":1,"water_band":1,
        "armed_at":armed_at,"optional_damage":false,
        "source":"scenario_assumption: initially laid source MDM-1 field, Shallow case/ground and declared complete geometry"
    })).unwrap();
    let sc: Scenario=serde_json::from_value(json!({
        "id":"signature-mine-native-crossing","name":"scenario_assumption: native source mine/signature laboratory",
        "seed":native_seed(),"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "mines_2026":{"fields":[field],"source":"scenario_assumption: initial minefield and crossing route"},
            "detection_2026":{"source":"scenario_assumption: controlled original target acoustics; not actual complete Burke fit",
                "targets":{"us_burke_2a":{"source":"scenario_assumption: source mechanism target profile only",
                    "size":1,"noise":noise,"anechoic":true,"titanium":false,"pumpjet":false,
                    "stealthy_ir":false,"radar_signature":2}}}},
        "units":[{"side":"Blue","class":"us_virginia","name":"FAR_OBSERVER","pos":[-10,-10],
                "depth_m":50,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"CROSSING","pos":[0.1,-0.4],
                "course_deg":0,"speed_kts":30,"fire_on":false}]
    })).unwrap();
    let w = harpoon_core::load_scenario(sc, db).unwrap();
    let t = w
        .entities
        .values()
        .find(|e| e.name == "CROSSING")
        .unwrap()
        .id;
    assert_eq!(
        t,
        EntityId(2),
        "natural die purpose uses actual allocated target"
    );
    assert_eq!(
        w.entity(t).unwrap().hp_max,
        341.,
        "original source hull DP is never authored or injected"
    );
    (w, t)
}

fn events(w: &World, target: EntityId, domain: &str, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                domain: d,
                rule: r,
                unit,
                detail,
                ..
            } if d == domain && r == rule && *unit == Some(target) => {
                if d == "signature_damage2026" || d == "mines2026" {
                    assert!(
                        matches!(e.vis, Visibility::Referee),
                        "hidden physical source trace remains Referee"
                    );
                }
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}

fn cross_to_first_real_hit(w: &mut World, t: EntityId) -> Value {
    let origin = w.entity(t).unwrap().pos;
    for _ in 0..100 {
        if let Some(e) = events(w, t, "damage", "hit_resolved").first() {
            assert!(
                w.entity(t).unwrap().pos.range_to(origin) > 0.2,
                "real hull movement must precede natural mine damage"
            );
            let attack = events(w, t, "mines2026", "13.5.attack");
            assert_eq!(
                attack.len(),
                1,
                "fixture intentionally stops immediately after the first real mine attack"
            );
            assert_eq!(attack[0]["identity"], "mine:signature-field:2:1");
            assert_eq!(attack[0]["chance"], 0.36);
            assert!(
                attack[0]["roll_d100"].as_u64().unwrap() <= 36,
                "actual original natural die succeeds"
            );
            //13.6 Shallow bottom case against a surface ship is Major71,
            // not Full235 or a synthetic threshold-only damage value.
            assert_eq!(e["weapon"], "unidentified_underwater_explosion");
            assert_eq!(e["dp"], 71.);
            assert!(w.entity(t).unwrap().alive && w.entity(t).unwrap().hp < 341.);
            return e.clone();
        }
        harpoon_core::sim::tick(w, &[]);
    }
    panic!("actual movement must produce a source mine attack and committed DP; no queue_mine fallback");
}

#[test]
fn m01_real_minefield_crossing_commits_major_source_dp_and_changes_both_signatures() {
    let (mut w, t) = fixture(Some(2), 0);
    let fitted = serde_json::to_value(&w.rules.detection.as_ref().unwrap().config.targets).unwrap();
    assert_eq!(
        signature::project_acoustic(&w, t, Some(2), true).unwrap(),
        (Some(2), true)
    );
    cross_to_first_real_hit(&mut w, t);
    let changed = events(&w, t, "signature_damage2026", "14.2.3.acoustic");
    assert_eq!(
        changed.len(),
        1,
        "actual source mine impact is a primary underwater attack"
    );
    assert_eq!(changed[0]["underwater_class"], "source_mine_13.6");
    assert_eq!(changed[0]["resolved_dp"], 71.);
    assert_eq!(changed[0]["original_dp"], 341.);
    assert_eq!(changed[0]["threshold_dp"], 341. * 0.1);
    assert_eq!(
        signature::project_acoustic(&w, t, Some(2), true).unwrap(),
        (Some(1), false)
    );
    assert_eq!(signature::project_radar(&w, t, 2).unwrap(), 1);
    assert_eq!(
        events(&w, t, "signature_damage2026", "14.2.2.signature").len(),
        1
    );
    assert!(events(&w, t, "signature_damage2026", "14.2.signature_pending").is_empty());
    assert_eq!(
        serde_json::to_value(&w.rules.detection.as_ref().unwrap().config.targets).unwrap(),
        fitted
    );
    let copy: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(
        signature::project_acoustic(&copy, t, Some(2), true).unwrap(),
        (Some(1), false)
    );
    assert_eq!(
        events(&copy, t, "signature_damage2026", "14.2.3.acoustic").len(),
        1
    );
    let enemy = serde_json::to_string(&harpoon_core::observe(&w, Side::Blue, 0, 1000)).unwrap();
    assert!(
        !enemy.contains("signature-field") && !enemy.contains("mdm_1"),
        "opponent gains no source field report from hidden trigger"
    );
}

#[test]
fn m02_same_natural_mine_hit_with_missing_acoustic_fit_saves_only_that_branch_pending() {
    let (mut w, t) = fixture(None, 0);
    cross_to_first_real_hit(&mut w, t);
    let pending = events(&w, t, "signature_damage2026", "14.2.signature_pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["underwater_class"], "source_mine_13.6");
    assert!(pending[0]["branches"]["acoustic"]
        .as_str()
        .is_some_and(|s| s.contains("source")));
    assert!(signature::project_acoustic(&w, t, None, true).is_err());
    assert_eq!(signature::project_radar(&w, t, 2).unwrap(), 1);
    assert!(events(&w, t, "signature_damage2026", "14.2.3.acoustic").is_empty());
}

#[test]
fn m03_real_crossing_of_not_yet_armed_field_causes_no_attack_dp_or_signature_change() {
    let (mut w, t) = fixture(Some(2), 1000);
    let origin = w.entity(t).unwrap().pos;
    for _ in 0..100 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.entity(t).unwrap().pos.range_to(origin) > 0.5);
    assert_eq!(w.entity(t).unwrap().hp, 341.);
    assert!(events(&w, t, "mines2026", "13.5.attack").is_empty());
    assert!(events(&w, t, "damage", "hit_resolved").is_empty());
    assert!(signature::own_view(&w, t).is_none());
    assert_eq!(
        signature::project_acoustic(&w, t, Some(2), true).unwrap(),
        (Some(2), true)
    );
    assert!(serde_json::to_value(&w).unwrap()["rules"]
        .get("signature_damage")
        .is_none());
}

#[test]
fn m04_verified_size_a_to_g_cap_mapping_is_data_contract_not_an_extra_native_attack_claim() {
    // Existing native primary and mine tests exercise commit/consumers; this
    // independent seven-cell check certifies exactly the source mapping only.
    for (size, cap) in [
        ("A", 0),
        ("B", 1),
        ("C", 2),
        ("D", 2),
        ("E", 3),
        ("F", 3),
        ("G", 4),
    ] {
        assert_eq!(signature::tables()["radar_size_cap"][size], cap);
    }
    assert!(
        signature::tables()["radar_size_cap"]["H"].is_null(),
        "no unsupported physical-H extension"
    );
}

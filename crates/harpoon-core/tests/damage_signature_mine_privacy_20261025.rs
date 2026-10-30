//! Native adversarial mine-source privacy: Blue owns the hidden source field,
//! Red really crosses it and receives original MDM-1 Major71 damage. Initial
//! geometry/acoustic fit are explicit author conditions. No hit, HP, damage
//! state, die, or mine-status injection; no native minelayer deployment claim.
use harpoon_core::rules::{damage_runtime as damage, mines2026 as mines};
use harpoon_core::{Database, EntityId, EventKind, Rng, Scenario, Side, Visibility, World};
use serde_json::{json, Value};

const FIELD: &str = "signature-field";
const IDENTITY: &str = "mine:signature-field:2:1";

fn fixture() -> (World, EntityId) {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for class in ["us_virginia", "us_burke_2a"] {
        let p = db.platforms.get_mut(class).unwrap();
        p.weapons.clear();
        p.sensors.clear();
        p.rule_data = Some(damage::PlatformDamage {
            size: if class == "us_burke_2a" { "B" } else { "C" }.into(),
            role: "surface".into(),
            source: "scenario_assumption: controlled source Size and zero armor; original source hullDP unchanged".into(),
            ..Default::default()
        });
    }
    let seed = (1..10_000_u64)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(0, 2, IDENTITY));
            1 + (r.next_f64() * 100.).floor() as u8 <= 36
        })
        .expect("a natural successful first real MDM-1 crossing seed exists");
    let field: mines::Field = serde_json::from_value(json!({
        "id":FIELD,"owner":"Blue","mine":"mdm_1",
        "area":{"start":{"x":0,"y":0},"end":{"x":5000.0/2025.3718285,"y":0},"width_yd":1200},
        "rows":3,"spacing_yd":125,"row_spacing_yd":300,"case_band":1,"water_band":1,
        "armed_at":0,"optional_damage":false,
        "source":"scenario_assumption: initially laid Blue field; source MDM-1 Shallow case/ground and complete authored geometry"
    })).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"signature-mine-adversarial-privacy","name":"scenario_assumption: real hostile hidden mine crossing privacy",
        "seed":seed,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "mines_2026":{"fields":[field],"source":"scenario_assumption: Blue minefield and Red crossing route"},
            "detection_2026":{"source":"scenario_assumption: controlled target acoustics, not complete real Burke fit",
                "targets":{"us_burke_2a":{"source":"scenario_assumption: explicit source mechanism fit",
                    "size":1,"noise":2,"anechoic":true,"titanium":false,"pumpjet":false,
                    "stealthy_ir":false,"radar_signature":2}}}},
        "units":[{"side":"Blue","class":"us_virginia","name":"FAR_OBSERVER","pos":[-10,-10],
                "depth_m":50,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"CROSSING","pos":[0.1,-0.4],
                "course_deg":0,"speed_kts":30,"fire_on":false}]
    })).unwrap();
    let w = harpoon_core::load_scenario(sc, db).unwrap();
    let target = w
        .entities
        .values()
        .find(|e| e.name == "CROSSING")
        .unwrap()
        .id;
    assert_eq!(target, EntityId(2));
    assert_eq!(w.entity(target).unwrap().hp_max, 341.);
    (w, target)
}

fn records(w: &World, wanted_domain: &str, wanted_rule: &str) -> Vec<(Visibility, Value)> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                domain,
                rule,
                detail,
                ..
            } if domain == wanted_domain && rule == wanted_rule => {
                Some((e.vis, serde_json::from_str(detail).unwrap()))
            }
            _ => None,
        })
        .collect()
}

fn to_first_real_hit(w: &mut World, target: EntityId) {
    let origin = w.entity(target).unwrap().pos;
    for _ in 0..100 {
        if !records(w, "mines2026", "13.5.attack").is_empty() {
            break;
        }
        harpoon_core::sim::tick(w, &[]);
    }
    let attacks = records(w, "mines2026", "13.5.attack");
    assert_eq!(attacks.len(), 1, "stop at one actual source mine attack");
    assert_eq!(attacks[0].0, Visibility::Referee);
    assert_eq!(attacks[0].1["identity"], IDENTITY);
    assert_eq!(attacks[0].1["chance"], 0.36);
    assert!(attacks[0].1["roll_d100"].as_u64().unwrap() <= 36);
    assert!(
        w.entity(target).unwrap().pos.range_to(origin) > 0.1,
        "real Red hull movement must precede the mine trigger"
    );
    assert!(w.entity(target).is_some_and(|e| e.alive && e.hp < 341.));
    assert!(
        w.events.iter().any(|e| matches!(&e.kind,
        EventKind::Impact{side:Side::Red,unit,weapon,damage,..}
        if *unit==target && *damage==71. && weapon=="水下爆炸（来源未明）")),
        "actual source Shallow MDM-1 Major71 must commit, not a synthetic threshold hit"
    );
}

fn assert_views_and_full_referee(w: &World, target: EntityId) {
    let red = harpoon_core::observe(w, Side::Red, 0, 10_000);
    let own = red
        .units
        .iter()
        .find(|e| e.id == target)
        .expect("victim retains own hull observation");
    assert_eq!(
        own.hp,
        w.entity(target).unwrap().hp,
        "own actual remainingHP remains visible"
    );
    assert!(
        own.signature_damage.is_some(),
        "victim can see its own actual signature casualty"
    );
    assert!(
        red.events.iter().any(|e| matches!(&e.kind,
        EventKind::Impact{unit,damage,..} if *unit==target && *damage==71.)),
        "privacy repair must retain Red's real damage consequence"
    );
    let red_json = serde_json::to_string(&red).unwrap().to_lowercase();
    for secret in [FIELD, "mdm_1", "mdm-1", IDENTITY, "mine:"] {
        assert!(!red_json.contains(secret),
            "victim observation must not expose hidden field/model/attack-source identity: {secret}");
    }

    let blue = harpoon_core::observe(w, Side::Blue, 0, 10_000);
    assert!(blue.units.iter().all(|u| u.id != target));
    assert!(
        blue.contacts.is_empty(),
        "field ownership cannot create a detection report on the victim"
    );
    assert!(
        !blue.events.iter().any(|e| matches!(&e.kind,
        EventKind::Impact{unit,..}|EventKind::CriticalHit{unit,..}|EventKind::Sunk{unit,..}
            if *unit==target)),
        "mine owner receives no unobserved victim impact/critical/sunk BDA"
    );
    assert!(!blue.events.iter().any(|e| matches!(&e.kind,
        EventKind::RulesEvent{domain,unit,..} if *unit==Some(target) && (domain=="damage" || domain=="signature_damage2026"))),
        "mine owner receives no victim damage/signature truth");

    let attacks = records(w, "mines2026", "13.5.attack");
    assert_eq!(attacks.len(), 1);
    assert_eq!(attacks[0].0, Visibility::Referee);
    assert_eq!(attacks[0].1["identity"], IDENTITY);
    assert_eq!(attacks[0].1["chance"], 0.36);
    assert!(
        records(w, "damage", "hit_resolved")
            .iter()
            .any(|(vis, v)| *vis == Visibility::Referee && v["hit"] == IDENTITY && v["dp"] == 71.),
        "complete original source identity/DP must survive in Referee evidence"
    );
    let signatures = records(w, "signature_damage2026", "14.2.3.acoustic");
    assert_eq!(signatures.len(), 1);
    assert_eq!(signatures[0].0, Visibility::Referee);
    assert_eq!(signatures[0].1["hit"], IDENTITY);
    assert_eq!(signatures[0].1["underwater_class"], "source_mine_13.6");
    assert_eq!(signatures[0].1["resolved_dp"], 71.);
    assert_eq!(signatures[0].1["original_dp"], 341.);
    assert_eq!(signatures[0].1["threshold_dp"], 341. * 0.1);
}

#[test]
fn native_enemy_mine_hit_keeps_victim_effects_but_hides_blue_field_model_and_attack_identity() {
    let (mut w, target) = fixture();
    to_first_real_hit(&mut w, target);
    assert_views_and_full_referee(&w, target);
}

#[test]
fn native_enemy_mine_source_privacy_and_full_evidence_survive_world_restore() {
    let (mut w, target) = fixture();
    for _ in 0..10 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(records(&w, "mines2026", "13.5.attack").is_empty());
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    restored.rules.validate_tables().unwrap();
    to_first_real_hit(&mut w, target);
    to_first_real_hit(&mut restored, target);
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
    for _ in 0..5 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
    assert_views_and_full_referee(&w, target);
    let copy: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    copy.rules.validate_tables().unwrap();
    assert_views_and_full_referee(&copy, target);
}

// The source identity bounding algorithm is independently mirrored only to
// select a natural seed and to describe the explicitly adjudicated downstream
// contract inputs below. No event, die, HP or production ledger is preloaded.
fn bounded_source_identity(value: &str) -> String {
    if value.len() <= 96 {
        return value.into();
    }
    let h = value.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    format!("{}#{h:016x}", &value[..64])
}

fn long_mine_engineering_fixture() -> (World, EntityId, String, u8) {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for class in ["us_virginia", "us_burke_2a"] {
        let p = db.platforms.get_mut(class).unwrap();
        p.weapons.clear();
        p.sensors.clear();
        p.rule_data = Some(damage::PlatformDamage {
            size: if class == "us_burke_2a" { "B" } else { "C" }.into(),
            role: "surface".into(),
            source: "scenario_assumption: controlled Size/zero armor, original hullDP unchanged"
                .into(),
            ..Default::default()
        });
    }
    let field_id = format!("{FIELD}-{}", "long-private-source-".repeat(8));
    let attack_identity = format!("mine:{field_id}:2:1");
    let original_hit = bounded_source_identity(&attack_identity);
    let natural_cause = format!("{original_hit}:ratio:0");
    let (seed, expected_critical_die) = (1..100_000_u64)
        .find_map(|seed| {
            let master = Rng::new(seed);
            let mut mine = master.derive(Rng::hash_tag(0, 2, &attack_identity));
            let mut ratio = master.derive(Rng::hash_tag(37, 2, &format!("ratio:{original_hit}")));
            let mut critical = master.derive(Rng::hash_tag(37, 2, &natural_cause));
            let mine_die = 1 + (mine.next_f64() * 100.).floor() as u8;
            let ratio_die = 1 + (ratio.next_f64() * 6.) as u8;
            let critical_die = 1 + (critical.next_f64() * 100.) as u8;
            // Actual71/270 source ratio row gives exactlyone critical atD6=4.
            // Actual underwater column31..50 is Engineering, before its own
            // electrical/fire rolls. Selection occurs before creatingWorld.
            (mine_die <= 36 && ratio_die == 4 && (31..=50).contains(&critical_die))
                .then_some((seed, critical_die))
        })
        .expect("natural first source mine hit with Engineering critical exists");
    let field: mines::Field = serde_json::from_value(json!({
        "id":field_id,"owner":"Blue","mine":"mdm_1",
        "area":{"start":{"x":0,"y":0},"end":{"x":5000.0/2025.3718285,"y":0},"width_yd":1200},
        "rows":3,"spacing_yd":125,"row_spacing_yd":300,"case_band":1,"water_band":1,
        "armed_at":0,"optional_damage":false,
        "source":"scenario_assumption: source MDM-1 first-hit geometry with deliberately long private field identifier"
    })).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"signature-mine-engineering-private-causes",
        "name":"scenario_assumption: natural mine Engineering plus explicit adjudicated cause-projection contract",
        "seed":seed,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "mines_2026":{"fields":[field],"source":"scenario_assumption: Blue initially laid source field"},
            "detection_2026":{"source":"scenario_assumption: controlled acoustic source mechanism fit",
                "targets":{"us_burke_2a":{"source":"scenario_assumption: explicitly fitted Quiet/coated target",
                    "size":1,"noise":2,"anechoic":true,"titanium":false,"pumpjet":false,
                    "stealthy_ir":false,"radar_signature":2}}}},
        "units":[{"side":"Blue","class":"us_virginia","name":"FAR_OBSERVER","pos":[-10,-10],
                "depth_m":50,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"CROSSING","pos":[0.1,-0.4],
                "course_deg":0,"speed_kts":30,"fire_on":false}]
    })).unwrap();
    let w = harpoon_core::load_scenario(sc, db).unwrap();
    assert_eq!(w.entity(EntityId(2)).unwrap().hp_max, 341.);
    (w, EntityId(2), natural_cause, expected_critical_die)
}

#[test]
fn real_mine_engineering_and_adjudicated_compound_causes_keep_distinct_private_repair_entries() {
    use harpoon_core::rules::repairs2026 as repairs;
    let (mut w, target, natural_cause, expected_die) = long_mine_engineering_fixture();
    for _ in 0..100 {
        if !records(&w, "mines2026", "13.5.attack").is_empty() {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(
        w.tick, 37,
        "actual unchanged crossing geometry anchors the natural seed"
    );
    assert!(w.entity(target).is_some_and(|e| e.alive && e.hp < 341.));
    let real_criticals = &w.rules.repairs.as_ref().unwrap().units[&target].criticals;
    assert_eq!(
        real_criticals.len(),
        1,
        "one actual natural Engineering casualty must be committed first"
    );
    assert!(real_criticals.contains_key(&natural_cause));
    assert!(
        records(&w, "damage", "damage_roll")
            .iter()
            .any(|(vis, v)| *vis == Visibility::Referee
                && v["hit"] == natural_cause
                && v["purpose"] == "critical_type"
                && v["roll"] == expected_die),
        "original actual source critical identity and natural die stay Referee"
    );

    // Explicitly authorized public contract input: these TWO additional
    // independently adjudicated one-level Engineering records model bounded
    // secondary/cascade identities from the same long hidden mine source.
    // They are not claimed to be two more naturally generated mine criticals,
    // and do not alter HP, aggregate engine levels, source events or RNG.
    let parent = natural_cause.strip_suffix(":ratio:0").unwrap();
    let first = format!(
        "{}:ratio:0",
        bounded_source_identity(&format!("{parent}:ratio:0:initial_secondary"))
    );
    let second = format!(
        "{}:ratio:0",
        bounded_source_identity(&format!("{parent}:ratio:1:initial_secondary"))
    );
    assert_ne!(first, second);
    let rng_before = serde_json::to_vec(&w.rng).unwrap();
    let real_events_before = serde_json::to_vec(&w.events).unwrap();
    repairs::record_engineering(&mut w, target, &first, 1);
    repairs::record_engineering(&mut w, target, &second, 1);
    assert_eq!(serde_json::to_vec(&w.rng).unwrap(), rng_before);
    assert_eq!(
        serde_json::to_vec(&w.events).unwrap(),
        real_events_before,
        "recording adjudicated identities cannot rewrite the actual original source rolls"
    );
    let raw = &w.rules.repairs.as_ref().unwrap().units[&target].criticals;
    assert_eq!(raw.len(), 3);
    for source in [&natural_cause, &first, &second] {
        assert_eq!(
            &raw[source].identity, source,
            "Referee immutable physical identity remains original"
        );
    }
    let state_before_view = serde_json::to_vec(&w).unwrap();
    let own = repairs::own_view(&w, target).unwrap();
    let public = own["criticals"].as_object().unwrap();
    assert_eq!(
        public.len(),
        3,
        "opaque projection must not merge distinct compound mine causes into one generic JSON key"
    );
    let mut identities = std::collections::BTreeSet::new();
    for (key, value) in public {
        assert!(!key.contains("mine:") && !key.contains(FIELD));
        let identity = value["identity"].as_str().unwrap();
        assert!(!identity.contains("mine:") && !identity.contains(FIELD));
        assert!(identities.insert(identity));
    }
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        state_before_view,
        "own projection is read-only"
    );
    let copy: World = serde_json::from_slice(&state_before_view).unwrap();
    copy.rules.validate_tables().unwrap();
    assert_eq!(repairs::own_view(&copy, target).unwrap(), own);
    assert_eq!(serde_json::to_vec(&copy).unwrap(), state_before_view);
}

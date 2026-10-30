//! English2026 PDF132/14-6 §§14.2.2/3; source/interpretation contract at
//! artifacts/rules-resume-20261025/damage-signature-design/ (read-only).
//! Adopted RTS policy: each committed primary Hit independently >=10% of
//! original DP, no integer rounding; one RCS step per hit capped by Size;
//! already-Noisy/Loud are exempt from the whole acoustic paragraph.
//! This is an explicit RTS interpretation, not the source's only reading.
//! Laboratory signatures/armor/weapons/contacts/Good solution are explicitly
//! authored. Original loaded hull HP is unchanged. No Flight/RNG/HP/signature
//! state is injected. C11 alone supplies an allowed referee secondary Hit.
use harpoon_core::db::{Guidance, WeaponDef, WeaponKind};
use harpoon_core::rules::{damage_runtime as damage, detection2026 as detection};
use harpoon_core::{
    Command, ContactId, Database, EntityId, EventKind, Rng, Scenario, Side, Vec2, Visibility, World,
};
use serde_json::{json, Value};
use std::sync::OnceLock;

fn database() -> Database {
    static DB: OnceLock<Database> = OnceLock::new();
    DB.get_or_init(|| {
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
    })
    .clone()
}

fn profile(size: usize, noise: usize, signature: usize) -> Value {
    json!({"source":"scenario_assumption: explicit signature laboratory, not an actual complete platform fit",
        "size":size,"noise":noise,"anechoic":true,"titanium":false,
        "pumpjet":false,"stealthy_ir":false,"radar_signature":signature})
}

fn fixture(
    signature: usize,
    noise: usize,
    armor: f64,
    submarine: bool,
) -> (World, EntityId, EntityId) {
    let mut db = database();
    let class = if submarine {
        "us_virginia"
    } else {
        "us_burke_2a"
    };
    let target = db.platforms.get_mut(class).unwrap();
    // Source HP stays 341 for the real IIA entry /168 for Virginia. Only the
    // isolated laboratory fit is authored, before the scenario is loaded.
    target.weapons.clear();
    target.sensors.clear();
    target.rule_data = Some(damage::PlatformDamage {
        size: "B".into(),
        role: "surface".into(),
        belt_cm: armor,
        deck_cm: armor,
        source:
            "scenario_assumption: physical Size B and controlled armor, no real armor-fit claim"
                .into(),
        ..Default::default()
    });
    let mut targets = json!({});
    targets[class] = profile(1, noise, signature);
    let sc: Scenario = serde_json::from_value(json!({
        "id":"source-damage-signatures-20261025","name":"scenario_assumption: isolated signature damage laboratory",
        "seed":23,"duration_s":5000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "detection_2026":{"source":"scenario_assumption: explicit signature mechanism inputs","targets":targets}},
        "units":[
            {"side":"Blue","class":"us_f18e","name":"OBSERVER","pos":[0,0],"course_deg":90,
                "speed_kts":0,"altitude_m":3000,"fire_on":false},
            {"side":"Red","class":class,"name":"TARGET","pos":[100,0],"course_deg":0,
                "speed_kts":0,"depth_m":if submarine {50}else{0},"fire_on":false}]
    })).unwrap();
    let w = harpoon_core::load_scenario(sc, db).unwrap();
    let find = |name: &str| w.entities.values().find(|e| e.name == name).unwrap().id;
    let observer = find("OBSERVER");
    let target = find("TARGET");
    assert_eq!(
        w.entity(target).unwrap().hp_max,
        if submarine { 168. } else { 341. }
    );
    (w, observer, target)
}

fn lab_weapon(dp: f64, underwater: bool, contact: bool) -> WeaponDef {
    let mut weapon = database().weapons["us_mk48"].clone();
    weapon.id = "scenario_assumption_signature_laboratory_warhead".into();
    weapon.name = "scenario_assumption: referee adjudicated laboratory hit".into();
    weapon.kind = if underwater {
        WeaponKind::Torpedo
    } else {
        WeaponKind::Gun
    };
    weapon.guidance = if underwater {
        Guidance::TorpedoActive
    } else {
        Guidance::BallisticGun
    };
    weapon.damage = dp;
    weapon.rule_data = Some(damage::WeaponDamage {
        ship_dp: Some(dp),
        sub_dp: Some(dp),
        penetration_cm: Some(0.),
        fuze: if contact { "contact" } else { "influence" }.into(),
        source: "scenario_assumption: explicit laboratory DP, not modified real Mk48 performance"
            .into(),
        ..Default::default()
    });
    weapon
}

fn queue(w: &mut World, target: EntityId, dp: f64, water: bool, identity: &str) {
    let weapon = lab_weapon(dp, water, false);
    damage::queue_with_parameters(
        w,
        target,
        &weapon,
        water,
        Some(90.),
        Some(Side::Blue),
        Some(identity.into()),
        false,
        None,
        None,
    );
}

fn signature_events(w: &World, target: EntityId, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                domain,
                rule: name,
                unit,
                detail,
                ..
            } if domain == "signature_damage2026" && name == rule && *unit == Some(target) => {
                assert!(
                    matches!(e.vis, Visibility::Referee),
                    "signature threshold truth must remain referee-only"
                );
                Some(serde_json::from_str(detail).expect("structured signature event"))
            }
            _ => None,
        })
        .collect()
}

fn resolved(w: &World, target: EntityId) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                domain,
                rule,
                unit,
                detail,
                ..
            } if domain == "damage" && rule == "hit_resolved" && *unit == Some(target) => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}

fn radar(w: &World, observer: EntityId, target: EntityId) -> f64 {
    detection::radar_range_world(
        w,
        w.rules.detection.as_ref().unwrap(),
        w.entity(observer).unwrap(),
        &w.db.sensors["radar_air_med"],
        w.entity(target).unwrap(),
    )
    .expect("source APG79 plus explicit laboratory surface signature")
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn c01_actual_committed_hit_increases_small_to_medium_before_annex_radar_lookup() {
    let (mut w, observer, target) = fixture(2, 2, 0., false);
    close(radar(&w, observer, target), 76.);
    queue(&mut w, target, 40., false, "c01-primary");
    assert!(resolved(&w, target).iter().any(|h| h["dp"] == 40.));
    assert_eq!(
        signature_events(&w, target, "14.2.2.signature").len(),
        1,
        "40 committed DP exceeds original341*10%; source effect is missing"
    );
    close(radar(&w, observer, target), 133.);
    assert!(!serde_json::to_value(&w).unwrap()["rules"]["signature_damage"].is_null());
}

#[test]
fn c02_unrounded_original_dp_threshold_accepts_exact_34_point_1_and_rejects_below() {
    for (dp, wanted) in [(34.099_999, false), (34.1, true), (34.100_001, true)] {
        let (mut w, observer, target) = fixture(2, 2, 0., false);
        queue(&mut w, target, dp, false, "c02-boundary");
        close(resolved(&w, target)[0]["dp"].as_f64().unwrap(), dp);
        assert_eq!(
            signature_events(&w, target, "14.2.2.signature").len(),
            usize::from(wanted),
            "adopted real-number per-hit threshold, dp={dp}"
        );
        close(radar(&w, observer, target), if wanted { 133. } else { 76. });
    }
}

#[test]
fn c03_one_oversized_primary_hit_is_only_one_signature_level() {
    let (mut w, observer, target) = fixture(4, 2, 0., false);
    close(radar(&w, observer, target), 24.);
    queue(&mut w, target, 80., false, "c03-one-large-hit");
    assert_eq!(signature_events(&w, target, "14.2.2.signature").len(), 1);
    close(radar(&w, observer, target), 43.);
}

#[test]
fn c04_independent_qualifying_hits_repeat_once_each_until_physical_size_cap() {
    let (mut w, observer, target) = fixture(3, 2, 0., false);
    for (identity, expected_range, transitions) in [
        ("c04-first", 76., 1),
        ("c04-second", 133., 2),
        ("c04-at-cap", 133., 2),
    ] {
        queue(&mut w, target, 40., false, identity);
        assert_eq!(
            signature_events(&w, target, "14.2.2.signature").len(),
            transitions
        );
        close(radar(&w, observer, target), expected_range);
    }
    assert_eq!(
        resolved(&w, target).len(),
        3,
        "the cap must not discard actual hull damage"
    );
}

#[test]
fn c05_same_hit_identity_is_idempotent_even_across_finish_and_world_restore() {
    let (mut w, observer, target) = fixture(3, 2, 0., false);
    damage::begin(&mut w);
    queue(&mut w, target, 40., false, "c05-same-hit");
    queue(&mut w, target, 40., false, "c05-same-hit");
    damage::finish(&mut w);
    let mut w: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    queue(&mut w, target, 40., false, "c05-same-hit");
    damage::finish(&mut w);
    assert_eq!(resolved(&w, target).len(), 1);
    assert_eq!(signature_events(&w, target, "14.2.2.signature").len(), 1);
    close(radar(&w, observer, target), 76.);
}

#[test]
fn c06_two_six_percent_hits_do_not_merge_in_one_batch_or_across_batches() {
    for batched in [false, true] {
        let (mut w, observer, target) = fixture(2, 2, 0., false);
        if batched {
            damage::begin(&mut w);
        }
        queue(&mut w, target, 20.46, false, "c06-six-percent-a");
        queue(&mut w, target, 20.46, false, "c06-six-percent-b");
        damage::finish(&mut w);
        assert_eq!(resolved(&w, target).len(), 2);
        assert!(signature_events(&w, target, "14.2.2.signature").is_empty());
        close(radar(&w, observer, target), 76.);
        queue(&mut w, target, 40., false, "c06-later-qualified");
        assert_eq!(signature_events(&w, target, "14.2.2.signature").len(), 1);
        close(radar(&w, observer, target), 133.);
    }
}

#[test]
fn c07_threshold_uses_after_armor_committed_dp_instead_of_nominal_warhead() {
    let (mut w, observer, target) = fixture(2, 2, 10., false);
    queue(&mut w, target, 50., false, "c07-nominal50-actual25");
    close(resolved(&w, target)[0]["dp"].as_f64().unwrap(), 25.);
    assert!(signature_events(&w, target, "14.2.2.signature").is_empty());
    close(radar(&w, observer, target), 76.);
    queue(&mut w, target, 80., false, "c07-nominal80-actual40");
    assert!(resolved(&w, target).iter().any(|h| h["dp"] == 40.));
    assert_eq!(signature_events(&w, target, "14.2.2.signature").len(), 1);
    close(radar(&w, observer, target), 133.);
}

#[test]
fn c08_same_primary_dp_changes_acoustics_only_for_actual_underwater_attack() {
    for underwater in [false, true] {
        let (mut w, _, target) = fixture(2, 2, 0., false);
        let original =
            serde_json::to_value(&w.rules.detection.as_ref().unwrap().config.targets).unwrap();
        queue(&mut w, target, 40., underwater, "c08-primary-domain");
        assert_eq!(
            signature_events(&w, target, "14.2.3.acoustic").len(),
            usize::from(underwater)
        );
        assert_eq!(signature_events(&w, target, "14.2.2.signature").len(), 1);
        assert_eq!(
            original,
            serde_json::to_value(&w.rules.detection.as_ref().unwrap().config.targets).unwrap(),
            "damage is persistent runtime state, not a mutation of immutable fitted profile"
        );
    }
}

#[test]
fn c09_already_noisy_or_loud_coated_vessels_have_no_acoustic_transition() {
    for noise in [1, 0] {
        let (mut w, observer, target) = fixture(2, noise, 0., false);
        queue(&mut w, target, 40., true, "c09-acoustic-exempt");
        assert!(
            signature_events(&w, target, "14.2.3.acoustic").is_empty(),
            "whole-paragraph exemption must not make Loud quieter or strip already-Noisy coating"
        );
        assert_eq!(
            signature_events(&w, target, "14.2.2.signature").len(),
            1,
            "acoustic exemption does not cancel the independent radar rule"
        );
        close(radar(&w, observer, target), 133.);
    }
}

#[test]
fn c10_submarine_gets_underwater_acoustic_damage_without_surface_ship_rcs_rule() {
    let (mut w, _, target) = fixture(2, 2, 0., true);
    queue(&mut w, target, 20., true, "c10-submarine-primary");
    assert!(resolved(&w, target).iter().any(|h| h["dp"] == 20.));
    assert_eq!(signature_events(&w, target, "14.2.3.acoustic").len(), 1);
    assert!(signature_events(&w, target, "14.2.2.signature").is_empty());
}

#[test]
fn c11_referee_secondary_underwater_hit_commits_dp_but_cannot_trigger_signature_rules() {
    let (mut w, observer, target) = fixture(2, 2, 0., false);
    // Explicit referee queue-contract input permitted by root: this tests the
    // secondary classification gate. A natural D6 flooding record is <=6%,
    // so this authored40DP secondary is NOT claimed as naturally generated.
    damage::begin(&mut w);
    w.rules.pending.push(damage::Hit {
        target,
        weapon: lab_weapon(40., true, false),
        underwater: true,
        bearing: Some(90.),
        shooter: None,
        identity: "c11-author-referee-secondary".into(),
        tick: w.tick,
        secondary: true,
        torpedo_deep: false,
        impact_area: None,
        targeted_sensor: None,
        mine: None,
        navigation: None,
        projector: None,
            asw: None,
    });
    damage::finish(&mut w);
    assert!(resolved(&w, target).iter().any(|h| h["dp"] == 40.));
    assert!(signature_events(&w, target, "14.2.3.acoustic").is_empty());
    assert!(signature_events(&w, target, "14.2.2.signature").is_empty());
    close(radar(&w, observer, target), 76.);
}

fn native_hit_seed() -> u64 {
    (1..20_000)
        .find(|seed| {
            let mut placement = Rng::new(*seed).derive(Rng::hash_tag(0, 0, "10.placement:3:0"));
            let d10 = 1 + (placement.next_f64() * 10.) as u8;
            let mut attack = Rng::new(*seed).derive(Rng::hash_tag(0, 3, "10.2.5.attack:0"));
            let d100 = 1 + (attack.next_f64() * 100.) as u8;
            d10 <= 7 && d100 <= 10
        })
        .expect("a natural Good-minus-coating placed/hit seed exists")
}

#[test]
fn c12_native_fire_real_motion_and_natural_mk54_terminal_hit_trigger_acoustic_damage() {
    let mut db = database();
    // Deliberately authored isolated Mk54 tube on Virginia; no real fit claim.
    let launcher = db.platforms.get_mut("us_virginia").unwrap();
    launcher.weapons[0].weapon = "us_mk54".into();
    launcher.weapons[0].ammo = 1;
    launcher.weapons[0].mounts = 1;
    launcher.sensors.clear();
    db.platforms.get_mut("cn_093").unwrap().weapons.clear();
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"signature-native-mk54-20261025","name":"scenario_assumption: author tube/report/Good/signature, actual source Mk54 flight and damage",
        "seed":native_hit_seed(),"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "detection_2026":{"source":"scenario_assumption: controlled Quiet coated target",
                "targets":{"cn_093":profile(2,2,2)}},
            "torpedo_2026":{"source":"scenario_assumption: post-release source mechanism fixture",
                "environment":{"source":"scenario_assumption: no layer/reverb","layer_m":null,"reverberant":false},
                "targets":{"cn_093":{"source":"scenario_assumption: Quiet coated target","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
                "platforms":{"us_virginia":{"source":"scenario_assumption: one loaded laboratory Mk54 tube; not real Virginia fit",
                    "wire_capacity":2,"reload_parallel":2,"tubes":[{"bearing_deg":0,"loaded":"us_mk54"}],"reserves":{}}},
                "orders":{"SHOOTER:us_mk54":{"source":"scenario_assumption: declared source attack; author Good isolates physical signature mechanism",
                    "speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":0,"cut_wire":true,"spread_deg":[],
                    "solution":{"quality":"Good","source":"scenario_assumption: controlled own report, not native FCS claim"}}}}},
        "units":[{"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"depth_m":50,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,0.5],"depth_m":50,"course_deg":0,"speed_kts":0,"fire_on":false}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let shooter = w
        .entities
        .values()
        .find(|e| e.name == "SHOOTER")
        .unwrap()
        .id;
    let target = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
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
            sensors: vec!["scenario_assumption: own located target report".into()],
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
    let projectile = w
        .entities
        .values()
        .find(|e| e.kind == harpoon_core::types::PlatformKind::Torpedo)
        .expect("legal source Fire must create a real Mk54 body")
        .id;
    assert_eq!(
        projectile,
        EntityId(3),
        "natural seed purpose assumes actual third allocated entity"
    );
    let origin = w.entity(projectile).unwrap().pos;
    let mut moved = false;
    for _ in 0..150 {
        if !resolved(&w, target).is_empty() {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
        moved |= w
            .entity(projectile)
            .is_some_and(|e| e.pos.range_to(origin) > 0.001);
    }
    assert!(moved, "actual flight distance must precede damage");
    assert!(w.events.iter().any(|e| matches!(&e.kind,EventKind::RulesEvent{rule,detail,..}
        if rule=="10.2.5.source_terminal" && serde_json::from_str::<Value>(detail).is_ok_and(|v|v["hit"]==true))),
        "the actual natural source terminal die must hit");
    assert!(
        resolved(&w, target)
            .iter()
            .any(|h| h["weapon"] == "us_mk54" && h["dp"].as_f64().is_some_and(|dp| dp >= 14.5)),
        "real Mk54 after-aspect damage must cross original145's10% threshold"
    );
    assert_eq!(
        signature_events(&w, target, "14.2.3.acoustic").len(),
        1,
        "actual Fire/motion/hit must reach the same signature commit as referee hits"
    );
}

//! Independent candidates, implemented_unverified. English2026 PDF135/14-9
//! permits only manually operated machine guns/autocannon in local control
//! after total electrical loss. These cases exercise the real outer weapon
//! consumers, not merely expanded_damage::weapon_operational.
//!
//! The hull and selected 30mm weapon/ammunition come from the loaded database.
//! The laboratory local/manual physical fittings are explicit scenario
//! assumptions, NOT a claim about Ford or any historical 30mm installation.
//! An adjudicated engineering critical is an explicit source contract input;
//! the original production engineering D6 still decides electrical failure.
//! No Contact, FCS, quality, HP, casualty marker or die is injected.
use harpoon_core::rules::{damage_runtime as dr, expanded_damage2026 as ex};
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::json;

const GUN: &str = "gun_30mm";
const CLASS: &str = "scenario_local_manual_power_lab";
const ENGINEERING: &str = "local-power-source-engineering";

fn fixture(seed: u64, local: bool, manual: bool, fitted: bool) -> (World, EntityId, ContactId) {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut class = db.platforms["us_ford"].clone();
    class.id = CLASS.into();
    class.name = "scenario_assumption: local/manual laboratory hull, not real Ford fit".into();
    class.rule_data = Some(dr::PlatformDamage {
        source: "scenario_assumption: explicit laboratory surface damage classification".into(),
        size: "C".into(),
        role: "surface".into(),
        year: 2000,
        ..Default::default()
    });
    class.deck = None;
    class.sensors.retain(|s| s == "visual");
    class.weapons.retain(|m| m.weapon == GUN);
    assert_eq!(class.weapons.len(), 1);
    let count = class.weapons[0].mounts;
    let ammo = class.weapons[0].ammo;
    db.platforms.insert(CLASS.into(), class);
    // This target has original hull HP but no order to fire back; no target
    // weapon or sensor is needed to establish the laboratory lookout report.
    let mut target = db.platforms["us_burke_2a"].clone();
    target.id = "scenario_local_manual_power_target".into();
    target.name = "scenario_assumption: passive laboratory target hull".into();
    target.sensors.clear();
    target.weapons.clear();
    target.deck = None;
    db.platforms.insert(target.id.clone(), target);
    let mut components = serde_json::Map::new();
    if fitted {
        for physical in 0..count {
            components.insert(format!("local-30mm-{physical}"), json!({
                "source":"scenario_assumption: experimental manually operated local autocannon, not historical installation",
                "kind":"weapon_mount","binding":GUN,"mount_index":0,"physical":physical,
                "ammunition":ammo,"local_control":local,"manually_operated":manual
            }));
        }
    }
    let profile = json!({"source":"scenario_assumption: explicitly measured laboratory visual target", "size":2,
        "noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    let sc: Scenario = serde_json::from_value(json!({
        "id":"local-manual-power-resume","name":"scenario_assumption: explicit physical local/manual source experiment",
        "seed":seed,"duration_s":3600,"weather":{"visibility_nm":40,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "expanded_damage_2026":{"source":"scenario_assumption: exact physical laboratory fits",
                "units":{"LAB":{"source":"scenario_assumption: experimental local/manual mounting","crew":100,"components":components}}},
            "detection_2026":{"environment":{"source":"scenario_assumption: laboratory daylight clear-water conditions",
                "sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
                "water_depth_m":1000,"layer":null,"ice":0,"cloud":null},
                "targets":{CLASS:profile,"scenario_local_manual_power_target":profile}}},
        "units":[{"side":"Blue","class":CLASS,"name":"LAB","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"scenario_local_manual_power_target","name":"TARGET","pos":[0,0.2],"speed_kts":0,"fire_on":false}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let shooter = w.entities.values().find(|e| e.name == "LAB").unwrap().id;
    let target = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    for _ in 0..10 {
        harpoon_core::sim::tick(&mut w, &[]);
        if let Some(c) = w.sides[Side::Blue.index()].contacts.values().find(|c| {
            c.about == target
                && !c.stale
                && !c.bearing_only
                && c.quality >= 0.3
                && c.sensors.iter().any(|s| s == "visual")
        }) {
            assert_eq!(
                harpoon_core::rules::detection2026::known_category(&w, Side::Blue, target),
                Some(harpoon_core::types::PlatformKind::Ship)
            );
            return (w.clone(), shooter, c.id);
        }
    }
    panic!("native production lookout must establish a usable source contact");
}

fn with_power_loss(local: bool, manual: bool, fitted: bool) -> (World, World, EntityId, ContactId) {
    for seed in 0..128 {
        let (mut damaged, shooter, contact) = fixture(seed, local, manual, fitted);
        let baseline = damaged.clone();
        let hp = damaged.entity(shooter).unwrap().hp;
        ex::source_critical(&mut damaged, shooter, ENGINEERING, "engineering_no_fire", ex::CriticalContext {
            source: "scenario_assumption: already adjudicated engineering critical input, not natural attack success".into(),
            dp: 0., penetrated: true, underwater: false, airburst: false, weapon: None,
        }).unwrap();
        if damaged.entity(shooter).unwrap().ext.get("damage.power_lost").copied().unwrap_or(0.) > damaged.tick as f64 {
            assert_eq!(damaged.entity(shooter).unwrap().hp, hp);
            assert!(damaged.events.iter().any(|event| match &event.kind {
                EventKind::RulesEvent { rule, detail, .. } if rule == "damage_roll" => {
                    let d: serde_json::Value = serde_json::from_str(detail).unwrap();
                    d["purpose"] == "engineering_electrical" && d["roll"] == 1
                }
                _ => false,
            }));
            return (damaged, baseline, shooter, contact);
        }
    }
    panic!("source seed search must encounter the original electrical D6=1");
}
fn ammo(w: &World, shooter: EntityId) -> u16 {
    w.entity(shooter).unwrap().weapons[0].ammo
}
fn gun_actions(w: &World, shooter: EntityId) -> usize {
    w.events
        .iter()
        .filter(|e| matches!(&e.kind, EventKind::GunAction {shooter:id,..} if *id == shooter))
        .count()
}

#[test]
fn local_power01_manual_fire_consumes_the_fitted_local_autocannon_after_real_electrical_roll() {
    let (mut damaged, mut baseline, shooter, contact) = with_power_loss(true, true, true);
    assert!(ex::weapon_operational(&damaged, shooter, GUN).unwrap());
    let before = ammo(&damaged, shooter);
    let event_before = gun_actions(&damaged, shooter);
    assert!(
        harpoon_core::weapons::manual_fire(&mut baseline, Side::Blue, shooter, contact, GUN, 1),
        "same original native contact/weapon must fire before electrical loss"
    );
    assert!(harpoon_core::weapons::manual_fire(&mut damaged, Side::Blue, shooter, contact, GUN, 1),
        "Full exact local/manual fit must reach the actual surface gun executor despite total electrical loss");
    assert_eq!(ammo(&damaged, shooter), before - 1);
    assert_eq!(gun_actions(&damaged, shooter), event_before + 1);
}

#[test]
fn local_power02_assigned_target_scan_reaches_the_same_legal_local_autocannon() {
    let (mut damaged, mut baseline, shooter, contact) = with_power_loss(true, true, true);
    assert!(ex::weapon_operational(&damaged, shooter, GUN).unwrap());
    let before = ammo(&damaged, shooter);
    let event_before = gun_actions(&damaged, shooter);
    let cmd = Command::AssignTarget {
        unit: shooter,
        contact,
    };
    harpoon_core::sim::tick(&mut baseline, &[(Side::Blue, cmd.clone())]);
    assert!(
        ammo(&baseline, shooter) < before,
        "actual assigned-target source control must fire without damage"
    );
    harpoon_core::sim::tick(&mut damaged, &[(Side::Blue, cmd)]);
    assert!(
        ammo(&damaged, shooter) < before,
        "the aggregate shooter filter must not veto a legal exact local/manual physical weapon"
    );
    assert_eq!(gun_actions(&damaged, shooter), event_before + 1);
}

#[test]
fn local_power03_local_and_manual_flags_and_actual_physical_binding_are_all_required() {
    for (local, manual, fitted) in [
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        let (mut w, _, shooter, contact) = with_power_loss(local, manual, fitted);
        assert!(!ex::weapon_operational(&w, shooter, GUN).unwrap());
        let before = ammo(&w, shooter);
        assert!(!harpoon_core::weapons::manual_fire(
            &mut w,
            Side::Blue,
            shooter,
            contact,
            GUN,
            1
        ));
        assert_eq!(ammo(&w, shooter), before);
        assert_eq!(gun_actions(&w, shooter), 0);
    }
}

#[test]
fn local_power04_actual_damage_control_crew_order_still_stops_even_legal_local_gun() {
    let (mut w, _, shooter, contact) = with_power_loss(true, true, true);
    assert!(ex::weapon_operational(&w, shooter, GUN).unwrap());
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::DamageControl {
                unit: shooter,
                action: "reinforce_dc".into(),
            },
        )],
    );
    assert_eq!(
        w.entity(shooter).unwrap().ext.get("damage.dc_weapons_crew").copied().unwrap_or(0.),
        1.
    );
    assert!(!ex::weapon_operational(&w, shooter, GUN).unwrap());
    let before = ammo(&w, shooter);
    assert!(!harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        shooter,
        contact,
        GUN,
        1
    ));
    assert_eq!(ammo(&w, shooter), before);
    assert_eq!(gun_actions(&w, shooter), 0);
}

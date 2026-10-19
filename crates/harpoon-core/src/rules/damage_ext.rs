//! Damage closure: damage-control repair over time, armor, extended critical
//! rows (bridge / rudder / CIC).
//!
//! Hook: `step(world)` advances repair progress stored on `entity.ext`
//! ("dc_progress", "dc_last_hit_tick", "dc_next_repair_tick") and applies
//! partial system restoration; armor is consulted via `armor_factor`.
//!
//! Rule sources:
//! - §14.1.6 Effects of Armor (PDF `14-2.HpnV_Rules_1_Jan_26.pdf` p.130,
//!   rule page 14-4): belt/deck armor, partial armor, non-penetrating hits
//!   halve damage. The engine has no armour-thickness or penetration fields,
//!   so the transcribed multipliers live in
//!   `rules::damage_tables::ZoneArmor` as an estimated per-location band
//!   (capital 0.90 bow / 1.00 side / 0.80 stern; escort 0.95 / 0.90 / 0.85;
//!   small craft 1.00). `armor_factor` now takes the incoming bearing
//!   relative to the target's bow; `None` = broadside (legacy behaviour).
//! - §14.1.7 Armor and Underwater Attack (PDF pp.130-131, 14-4/14-5):
//!   torpedo protection systems, the `2 - plate` underwater approximation,
//!   and the printed rule that extreme bow/stern torpedo hits halve damage.
//! - §14.4 Fire & Flooding (PDF pp.133-134, 14-7/14-8): fires and flooding
//!   are fought by damage-control parties over successive turns. The numerical
//!   adaptation (0.8 progress chance per second, 60/120 s of effort) is the
//!   real-time conversion specified for this workstream.
//! - §14.6 Repairs (PDF p.136, 14-10): systems can be repaired after the
//!   battle; `300 s quiet then +0.2 per 60 s` is the RTS adaptation of the
//!   6/12/24/48-hour repair rolls specified for this workstream.
//! - Expanded critical tables 3-2 (PDF p.8): underwater column shares for
//!   CIC/bridge/rudder (5% each) are applied in `crate::damage`.
//!
//! NOTE: this workstream also owns `crate::damage` (table-driven
//! `roll_critical`) and `rules::damage_tables` (3-2 rows and armor bands);
//! keep the surface of this module minimal.

use crate::db::WeaponDef;
use crate::events::{EventKind, Visibility};
use crate::rules::damage_tables;
use crate::types::{EntityId, Side};
use crate::world::{Entity, World};

/// Seconds of crew effort needed to put out a fire (60 "progress" ticks).
pub const DC_FIRE_PROGRESS: f64 = 60.0;
/// Seconds of crew effort needed to contain flooding (120 "progress" ticks).
pub const DC_FLOODING_PROGRESS: f64 = 120.0;
/// Per-second chance that a damage-control party advances by one point.
pub const DC_PROGRESS_CHANCE: f64 = 0.8;
/// Quiet time after the last hit before systems can be repaired (§14.6
/// adaptation; a fresh hit restarts the clock).
pub const DC_REPAIR_DELAY_S: f64 = 300.0;
/// Interval between system-repair steps once repairs have started.
pub const DC_REPAIR_INTERVAL_S: f64 = 60.0;
/// Fraction of a damaged system restored per repair step.
pub const DC_REPAIR_STEP: f64 = 0.2;

/// Extra DB (armor values, DC crew ratings).
///
/// Armor lives in `rules::damage_tables::ZoneArmor` instead of the database:
/// neither `PlatformClass` nor `SensorDef` carries armor, and the brief
/// allows `db()` to stay empty for this workstream.
pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

/// Armor mitigation multiplier for a hit by `weapon` on `target`, at an
/// incoming bearing measured relative to the target's bow (`None` = unknown
/// / broadside).
///
/// The per-class, per-zone multipliers are in
/// `rules::damage_tables::armor_factor` together with their §14.1.6/§14.1.7
/// citations and the `estimated` marker for the numeric bands. Torpedoes and
/// other underwater weapons use `2 - plate`, multiplied by the class's
/// torpedo-protection factor (e.g. capital 0.75) and halved on an extreme
/// bow/stern hit (§14.1.7, PDF p.130). Surface weapons use the zone plate
/// directly.
pub fn armor_factor(
    target: &Entity,
    weapon: &WeaponDef,
    hit_bearing_rel_deg: Option<f64>,
) -> f64 {
    damage_tables::armor_factor(&target.class, weapon.kind, hit_bearing_rel_deg)
}

/// Per-tick damage-control behavior.
///
/// 1. Fire/flooding: while a casualty burns/floods, each second has an 80%
///    chance to advance `ext["dc_progress"]` by one. At 60 (fire) / 120
///    (flooding) the casualty is put out/contained and a `RulesEvent`
///    (`fire_out` / `flooding_contained`) is emitted. Flooding is fought
///    first when both are present because it drains HP faster.
/// 2. System repair: 300 s after the last hit (`ext["dc_last_hit_tick"]`,
///    set by `damage::apply_hit`), every 60 s restores 0.2 to each damaged
///    mobility/radar/sonar/fire_control value, capped at 1.0. No system repair
///    while flooding is active.
pub fn step(world: &mut World) {
    let tick = world.tick;
    let ids: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.alive && !e.kind.is_projectile())
        .map(|e| e.id)
        .collect();

    for id in ids {
        let mut rng = world.scope_rng(id, "damage_control");
        let mut pending: Vec<(Side, &'static str, String)> = Vec::new();

        {
            let Some(t) = world.entities.get_mut(&id) else { continue };

            // --- 1. Fire and flooding -------------------------------------------------
            if t.burn_s > 0.0 || t.flooding_s > 0.0 {
                if rng.chance(DC_PROGRESS_CHANCE) {
                    *t.ext.entry("dc_progress".to_string()).or_insert(0.0) += 1.0;
                }
                let progress = t.ext.get("dc_progress").copied().unwrap_or(0.0);
                // Fight flooding first: it is the faster hull killer.
                let flooding_first = t.flooding_s > 0.0;
                let threshold = if flooding_first {
                    DC_FLOODING_PROGRESS
                } else {
                    DC_FIRE_PROGRESS
                };
                if progress >= threshold {
                    if flooding_first {
                        t.flooding_s = 0.0;
                        pending.push((
                            t.side,
                            "flooding_contained",
                            format!(
                                "damage control contained flooding on {} (dc_progress {progress:.0})",
                                t.name
                            ),
                        ));
                    } else {
                        t.burn_s = 0.0;
                        pending.push((
                            t.side,
                            "fire_out",
                            format!(
                                "damage control extinguished the fire on {} (dc_progress {progress:.0})",
                                t.name
                            ),
                        ));
                    }
                    t.ext.insert("dc_progress".to_string(), 0.0);
                }
            } else if t.ext.get("dc_progress").copied().unwrap_or(0.0) != 0.0 {
                // The casualty ended on its own (burned out / flooded out) or
                // was just put out: discard residual effort so a later casualty
                // starts from zero.
                t.ext.insert("dc_progress".to_string(), 0.0);
            }

            // --- 2. System restoration (§14.6 adaptation) -----------------------------
            // Flooding keeps repair parties fully occupied ("进水期不修").
            if t.flooding_s <= 0.0 {
                let damaged = t.systems.mobility < 1.0
                    || t.systems.radar < 1.0
                    || t.systems.sonar < 1.0
                    || t.systems.fire_control < 1.0;
                if damaged {
                    let last_hit = t.ext.get("dc_last_hit_tick").copied().unwrap_or(0.0);
                    let earliest = last_hit + DC_REPAIR_DELAY_S;
                    let due = t
                        .ext
                        .get("dc_next_repair_tick")
                        .copied()
                        .unwrap_or(earliest)
                        .max(earliest);
                    if tick as f64 >= due {
                        let mut restored: Vec<&'static str> = Vec::new();
                        if t.systems.mobility < 1.0 {
                            t.systems.mobility = (t.systems.mobility + DC_REPAIR_STEP).min(1.0);
                            restored.push("mobility");
                        }
                        if t.systems.radar < 1.0 {
                            t.systems.radar = (t.systems.radar + DC_REPAIR_STEP).min(1.0);
                            restored.push("radar");
                        }
                        if t.systems.sonar < 1.0 {
                            t.systems.sonar = (t.systems.sonar + DC_REPAIR_STEP).min(1.0);
                            restored.push("sonar");
                        }
                        if t.systems.fire_control < 1.0 {
                            t.systems.fire_control =
                                (t.systems.fire_control + DC_REPAIR_STEP).min(1.0);
                            restored.push("fire_control");
                        }
                        t.ext.insert(
                            "dc_next_repair_tick".to_string(),
                            tick as f64 + DC_REPAIR_INTERVAL_S,
                        );
                        pending.push((
                            t.side,
                            "system_repair",
                            format!(
                                "damage control restored {} on {}; systems now mobility {:.2}, radar {:.2}, sonar {:.2}, fire_control {:.2}",
                                restored.join("/"),
                                t.name,
                                t.systems.mobility,
                                t.systems.radar,
                                t.systems.sonar,
                                t.systems.fire_control
                            ),
                        ));
                    } else {
                        t.ext.insert("dc_next_repair_tick".to_string(), due);
                    }
                } else {
                    // Fully healthy: clear any stale repair schedule.
                    let _ = t.ext.remove("dc_next_repair_tick");
                }
            }
        }

        for (side, rule, detail) in pending {
            world.emit(
                EventKind::RulesEvent {
                    domain: "damage".to_string(),
                    rule: rule.to_string(),
                    side: Some(side),
                    unit: Some(id),
                    contact: None,
                    detail,
                },
                Visibility::Side(side),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Guidance, Targets, WeaponKind};
    use crate::geom::Vec2;
    use crate::scenario::{Scenario, UnitSpawn};
    use crate::types::DayNight;
    use crate::world::Weather;

    fn world_with(class: &str) -> World {
        let sc = Scenario {
            id: "damage-ext-test".into(),
            name: "Damage Ext Test".into(),
            description: String::new(),
            context: Default::default(),
            duration_s: 100_000,
            weather: Weather { visibility_nm: 20.0, jam: [0.0, 0.0], overcast: 0.0, rain: 0.0 },
            day_night: DayNight::Day,
            seed: 20251018,
            victory: Default::default(),
            units: vec![UnitSpawn {
                side: Side::Blue,
                class: class.into(),
                name: Some("TEST SHIP".into()),
                pos: Vec2::new(0.0, 0.0),
                course_deg: 0.0,
                speed_kts: 0.0,
                depth_m: None,
                altitude_m: None,
                waypoints: None,
                follow: None,
                sonar_active: false,
                fire_on: false,
            }],
        };
        crate::scenario::load_scenario(sc, crate::db::Database::built_in()).unwrap()
    }

    fn weapon(kind: WeaponKind, damage: f64) -> WeaponDef {
        WeaponDef {
            id: "test_weapon".into(),
            name: "Test Weapon".into(),
            kind,
            targets: Targets { surface: true, ..Default::default() },
            range_nm: 10.0,
            min_range_nm: 0.0,
            speed_kts: 1000.0,
            enable_range_nm: 0.0,
            pk: 1.0,
            salvo: 1,
            reload_s: 0.0,
            damage,
            guidance: Guidance::BallisticGun,
            ir: false,
            directors: 0,
        }
    }

    fn has_rule(world: &World, rule: &str) -> bool {
        world.events.iter().any(|ev| {
            matches!(
                &ev.kind,
                EventKind::RulesEvent { domain, rule: r, .. }
                    if domain == "damage" && r == rule
            )
        })
    }

    #[test]
    fn armor_plate_tiers_and_torpedo_bonus() {
        let gun = weapon(WeaponKind::Gun, 10.0);
        let torp = weapon(WeaponKind::Torpedo, 10.0);

        // Estimated work-order bands for the capital ship: bow 0.90 / side
        // 1.00 / stern 0.80, torpedo protection 0.75. `None` (unknown
        // bearing) is a broadside, preserving the legacy behaviour.
        let carrier = world_with("cvn");
        let target = carrier.entity(EntityId(1)).unwrap();
        assert!((armor_factor(target, &gun, Some(0.0)) - 0.90).abs() < 1e-12);
        assert!((armor_factor(target, &gun, Some(90.0)) - 1.00).abs() < 1e-12);
        assert!((armor_factor(target, &gun, Some(180.0)) - 0.80).abs() < 1e-12);
        assert!((armor_factor(target, &gun, None) - 1.00).abs() < 1e-12);
        assert!((armor_factor(target, &torp, Some(90.0)) - 0.75).abs() < 1e-12);
        // §14.1.7: extreme bow/stern underwater hits are halved:
        // (2 - 0.90) * 0.75 * 0.5 = 0.4125.
        assert!((armor_factor(target, &torp, Some(0.0)) - 0.4125).abs() < 1e-12);

        // Escort: legacy side 0.90 preserved; no torpedo-bulge data (1.10).
        let ddg = world_with("ddg_aegis");
        let target = ddg.entity(EntityId(1)).unwrap();
        assert!((armor_factor(target, &gun, Some(90.0)) - 0.90).abs() < 1e-12);
        assert!((armor_factor(target, &torp, Some(90.0)) - 1.10).abs() < 1e-12);

        let boat = world_with("mgb");
        let target = boat.entity(EntityId(1)).unwrap();
        assert_eq!(armor_factor(target, &gun, Some(0.0)), 1.0);
        assert_eq!(armor_factor(target, &torp, Some(90.0)), 1.0);
        // Extreme bow/stern torpedo hits halve even an unarmored hull.
        assert_eq!(armor_factor(target, &torp, Some(0.0)), 0.5);

        // ffg has no band entry: "no data" falls back to 1.0.
        let no_data = world_with("ffg");
        let target = no_data.entity(EntityId(1)).unwrap();
        assert_eq!(armor_factor(target, &gun, Some(45.0)), 1.0);
    }

    #[test]
    fn fire_is_extinguished_by_damage_control() {
        let mut w = world_with("ddg_aegis");
        let id = EntityId(1);
        {
            let e = w.entity_mut(id).unwrap();
            // Long fire so the test exercises DC closure, not natural burn-out.
            e.burn_s = 600.0;
        }
        let mut out = false;
        for _ in 0..400 {
            crate::sim::tick(&mut w, &[]);
            if has_rule(&w, "fire_out") {
                out = true;
                break;
            }
        }
        assert!(out, "expected damage-rule fire_out event");
        let e = w.entity(id).expect("ship should still be alive");
        assert_eq!(e.burn_s, 0.0);
        assert_eq!(e.ext.get("dc_progress").copied().unwrap_or(0.0), 0.0);
    }

    #[test]
    fn flooding_is_contained_by_damage_control() {
        let mut w = world_with("ddg_aegis");
        let id = EntityId(1);
        {
            let e = w.entity_mut(id).unwrap();
            e.flooding_s = 600.0;
        }
        let mut out = false;
        for _ in 0..400 {
            crate::sim::tick(&mut w, &[]);
            if has_rule(&w, "flooding_contained") {
                out = true;
                break;
            }
        }
        assert!(out, "expected damage-rule flooding_contained event");
        let e = w.entity(id).expect("ship should still be alive");
        assert_eq!(e.flooding_s, 0.0);
        assert_eq!(e.ext.get("dc_progress").copied().unwrap_or(0.0), 0.0);
    }

    #[test]
    fn systems_repair_after_300s_quiet_then_every_60s() {
        let mut w = world_with("ddg_aegis");
        let id = EntityId(1);
        {
            let e = w.entity_mut(id).unwrap();
            e.systems.mobility = 0.4;
            e.systems.radar = 0.3;
            e.systems.sonar = 0.8;
            e.systems.fire_control = 0.5;
            e.ext.insert("dc_last_hit_tick".into(), 0.0);
        }

        for _ in 0..299 {
            crate::sim::tick(&mut w, &[]);
        }
        let e = w.entity(id).unwrap();
        assert_eq!(e.systems.mobility, 0.4, "repaired before 300 s");
        assert_eq!(e.systems.radar, 0.3, "repaired before 300 s");
        assert_eq!(e.systems.fire_control, 0.5, "repaired before 300 s");

        crate::sim::tick(&mut w, &[]); // tick 300
        let e = w.entity(id).unwrap();
        assert!((e.systems.mobility - 0.6).abs() < 1e-9);
        assert!((e.systems.radar - 0.5).abs() < 1e-9);
        assert!((e.systems.sonar - 1.0).abs() < 1e-9);
        assert!((e.systems.fire_control - 0.7).abs() < 1e-9);
        assert!(has_rule(&w, "system_repair"));

        for _ in 0..60 {
            crate::sim::tick(&mut w, &[]); // tick 360
        }
        let e = w.entity(id).unwrap();
        assert!((e.systems.mobility - 0.8).abs() < 1e-9);
        assert!((e.systems.radar - 0.7).abs() < 1e-9);
        assert!((e.systems.fire_control - 0.9).abs() < 1e-9);
    }

    #[test]
    fn system_repair_is_suspended_while_flooding() {
        let mut w = world_with("ddg_aegis");
        let id = EntityId(1);
        w.tick = 300; // repair is already due (last hit at t=0)
        {
            let e = w.entity_mut(id).unwrap();
            e.systems.mobility = 0.5;
            e.flooding_s = 6000.0;
            e.ext.insert("dc_last_hit_tick".into(), 0.0);
        }

        // One rules pass at the due tick: flooding blocks system repair.
        step(&mut w);
        assert_eq!(
            w.entity(id).unwrap().systems.mobility,
            0.5,
            "system repaired while flooding"
        );

        // Stop the flooding; the overdue repair schedule resumes next pass.
        w.tick = 301;
        w.entity_mut(id).unwrap().flooding_s = 0.0;
        step(&mut w);
        assert!((w.entity(id).unwrap().systems.mobility - 0.7).abs() < 1e-9);
    }
}

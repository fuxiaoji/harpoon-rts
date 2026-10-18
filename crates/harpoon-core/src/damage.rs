//! Damage adjudication: hit application, critical-hit subsystem rolls, fires,
//! sinking. Implemented as referee rolls seeded from per-entity derived RNG
//! streams — replaces the tabletop's damage/critical hit tables while keeping
//! their structure (damage points + subsystem crits).

use crate::db::WeaponDef;
use crate::events::{EventKind, Visibility};
use crate::types::{EntityId, PlatformKind};
use crate::world::World;

/// Roll damage + criticals for a successful weapon hit on `target`.
/// `underwater` selects the torpedo/rocket critical column (flooding-heavy).
pub fn apply_hit(world: &mut World, target_id: EntityId, weapon: &WeaponDef, underwater: bool) {
    let Some(t) = world.entities.get(&target_id).cloned() else { return };
    if !t.alive {
        return;
    }
    let mut rng = world.scope_rng(target_id, "damage");
    let variance = rng.range(0.85, 1.15);
    // Armor: guns use the plate multiplier directly; torpedoes use
    // 2 - plate to approximate the underwater-attack armor rules.
    // §14.1.6 (PDF 14-2 p.130, rule p.14-4): non-penetrating hits do less
    // damage; §14.1.7 (PDF p.131, 14-5): underwater hits are far less
    // attenuated by belt/deck armor. The per-class plate table and the
    // continuous multiplier are the damage-extension adaptation; see
    // `rules::damage_ext::armor_factor`.
    let armor = crate::rules::damage_ext::armor_factor(&t, weapon);
    let dmg = weapon.damage * variance * armor;
    let hp_frac = (t.hp / t.hp_max).clamp(0.0, 1.0);
    let new_hp = t.hp - dmg;
    let crit_p = 0.10 + 0.30 * (1.0 - hp_frac);
    let do_crit = new_hp > 0.0 && rng.chance(crit_p);

    let now = world.tick as f64;
    if let Some(t) = world.entities.get_mut(&target_id) {
        t.hp = new_hp;
        // §14.6 (PDF p.136, 14-10): fresh damage interrupts repair work and
        // restarts the system-repair clock. DC progress toward the current
        // fire/flooding casualty is lost.
        t.ext.insert("dc_progress".to_string(), 0.0);
        t.ext.insert("dc_last_hit_tick".to_string(), now);
        let _ = t.ext.remove("dc_next_repair_tick");
    }

    let (side, name, kind) = (t.side, t.name.clone(), t.kind);
    world.emit(
        EventKind::Impact {
            side,
            unit: target_id,
            unit_name: name.clone(),
            weapon: weapon.name.clone(),
            damage: dmg,
            hp_left: new_hp.max(0.0),
        },
        Visibility::Side(side),
    );
    if kind == PlatformKind::Base {
        world.emit(
            EventKind::BaseDamaged {
                side,
                base: target_id,
                base_name: name,
                damage: dmg,
                hp_left: new_hp.max(0.0),
            },
            Visibility::Side(side),
        );
    }

    // Bomb-damage assessment for the shooter if they hold a track.
    bda_to_shooter(world, target_id, weapon, dmg, new_hp.max(0.0));

    if do_crit {
        roll_critical(world, target_id, &mut rng, underwater);
    }
    if new_hp <= 0.0 {
        sink(world, target_id);
    }
}

/// Broadcast battle-damage assessment to the shooter's side when they track
/// the victim.
fn bda_to_shooter(
    world: &mut World,
    target_id: EntityId,
    weapon: &WeaponDef,
    dmg: f64,
    hp_left: f64,
) {
    let Some(t) = world.entities.get(&target_id) else { return };
    let shooter_side = t.side.other();
    let tracked = world
        .sides[shooter_side.index()]
        .contacts
        .values()
        .any(|c| c.about == target_id && !c.stale && c.quality > 0.25);
    if !tracked {
        return;
    }
    if t.kind == PlatformKind::Base {
        world.emit(
            EventKind::BaseDamaged {
                side: shooter_side,
                base: t.id,
                base_name: t.name.clone(),
                damage: dmg,
                hp_left,
            },
            Visibility::Side(shooter_side),
        );
    } else {
        world.emit(
            EventKind::Impact {
                side: shooter_side,
                unit: t.id,
                unit_name: t.name.clone(),
                weapon: weapon.name.clone(),
                damage: dmg,
                hp_left,
            },
            Visibility::Side(shooter_side),
        );
    }
}

fn roll_critical(world: &mut World, target_id: EntityId, rng: &mut crate::rng::Rng, underwater: bool) {
    let system;
    let side;
    {
        let Some(t) = world.entities.get_mut(&target_id) else { return };
        // Surface column vs underwater column (torpedo/rocket hits are
        // flooding/hull-pen dominated, mirroring the official tables).
        //
        // Extended critical hit tables (3-2, Chinese ed., PDF p.4 =
        // 1955+ surface combatants, PDF p.8 = underwater attacks):
        //   surface:   CIC 36-45 (10%), bridge 91-95 (5%), rudder 96-00 (5%)
        //   underwater: CIC 26-30 (5%), bridge 91-95 (5%), rudder 96-00 (5%)
        // The base-game Critical Hit Table (14-2, rule p.14-2 / PDF p.128,
        // §14.1.2) carries the same rows as D20 6-7 / 18-19 / 20. Official
        // expanded surface total is 20%; the engine compresses CIC to 5% so
        // the three new rows total 15% in both columns, inside the 12-18%
        // adaptation band for this workstream, and absorbs the difference by
        // shaving 1-2 points from the legacy rows. Total remains exactly 1.0.
        let roll = rng.next_f64();
        let label = if underwater {
            if roll < 0.30 {
                // 3-2 p.8: flooding 51-80 dominates the underwater column.
                t.flooding_s = (t.flooding_s + 120.0).min(300.0);
                t.systems.mobility = (t.systems.mobility - 0.2).max(0.0);
                "flooding"
            } else if roll < 0.50 {
                t.systems.mobility = (t.systems.mobility - rng.range(0.4, 1.0)).max(0.0);
                t.speed_kts = t.speed_kts.min(t.speed_kts * t.systems.mobility);
                "propulsion"
            } else if roll < 0.65 {
                // Hull penetration: immediate structural damage.
                t.hp -= t.hp_max * 0.10;
                "hull_penetration"
            } else if roll < 0.75 {
                t.systems.sonar = 0.0;
                "sonar"
            } else if roll < 0.80 {
                t.systems.fire_control = (t.systems.fire_control - rng.range(0.5, 1.0)).max(0.0);
                "fire_control"
            } else if roll < 0.85 {
                // CIC: 3-2 p.8 row 26-30. Command space destroyed; no local
                // fire-control mode. Engine: fire control zero, radar -0.5.
                t.systems.fire_control = 0.0;
                t.systems.radar = (t.systems.radar - 0.5).max(0.0);
                "cic"
            } else if roll < 0.90 {
                // Bridge: 3-2 p.8 row 91-95. Conning station destroyed.
                t.systems.fire_control = (t.systems.fire_control - 0.3).max(0.0);
                t.systems.mobility = (t.systems.mobility - 0.3).max(0.0);
                "bridge"
            } else if roll < 0.95 {
                // Rudder: 3-2 p.8 row 96-00. The task also asks for an ext
                // marker so movement can model jammed/disabled steering later.
                t.ext.insert("rudder_hit".to_string(), 1.0);
                t.systems.mobility = (t.systems.mobility - 0.4).max(0.0);
                "rudder"
            } else {
                t.flooding_s = (t.flooding_s + 90.0).min(300.0);
                "flooding"
            }
        } else if roll < 0.17 {
            t.systems.mobility = (t.systems.mobility - rng.range(0.4, 1.0)).max(0.0);
            t.speed_kts = t.speed_kts.min(t.speed_kts * t.systems.mobility);
            "propulsion"
        } else if roll < 0.31 {
            t.systems.radar = 0.0;
            "radar"
        } else if roll < 0.41 {
            t.systems.sonar = 0.0;
            "sonar"
        } else if roll < 0.51 {
            t.systems.fire_control = (t.systems.fire_control - rng.range(0.5, 1.0)).max(0.0);
            "fire_control"
        } else if roll < 0.61 {
            // Knock out a weapon mount.
            let victim = t
                .weapons
                .iter_mut()
                .filter(|m| m.ammo > 0)
                .min_by_key(|m| m.mounts);
            match victim {
                Some(m) => {
                    m.ammo = 0;
                    "weapon_mount"
                }
                None => "superstructure",
            }
        } else if roll < 0.66 {
            // Bridge: 3-2 p.4 row 91-95. Conning station destroyed; the task
            // models it as simultaneous fire-control and mobility loss
            // (navigation radar loss is folded into the generic systems).
            t.systems.fire_control = (t.systems.fire_control - 0.3).max(0.0);
            t.systems.mobility = (t.systems.mobility - 0.3).max(0.0);
            "bridge"
        } else if roll < 0.71 {
            // Rudder: 3-2 p.4 row 96-00.
            t.ext.insert("rudder_hit".to_string(), 1.0);
            t.systems.mobility = (t.systems.mobility - 0.4).max(0.0);
            "rudder"
        } else if roll < 0.76 {
            // CIC: 3-2 p.4 row 36-45. Fire control zero, radar -0.5.
            t.systems.fire_control = 0.0;
            t.systems.radar = (t.systems.radar - 0.5).max(0.0);
            "cic"
        } else if roll < 0.90 {
            // Flooding: HP drain over time plus a mobility knock.
            t.flooding_s = (t.flooding_s + 120.0).min(300.0);
            t.systems.mobility = (t.systems.mobility - 0.2).max(0.0);
            "flooding"
        } else if roll < 0.98 {
            t.burn_s = (t.burn_s + 60.0).min(180.0);
            "fire"
        } else {
            // Magazine detonation — catastrophic.
            t.hp = 0.0;
            "magazine_detonation"
        };
        system = label.to_string();
        side = Some(t.side);
    }
    if let Some(side) = side {
        world.emit(
            EventKind::CriticalHit { side, unit: target_id, system },
            Visibility::Side(side),
        );
    }
}

/// Kill a unit: notify owner, update enemy contacts, schedule removal.
pub fn sink(world: &mut World, target_id: EntityId) {
    let Some(t) = world.entities.get(&target_id).cloned() else { return };
    if !t.alive {
        return;
    }
    {
        let Some(t) = world.entities.get_mut(&target_id) else { return };
        t.alive = false;
        t.hp = 0.0;
        t.last_emit_tick = world.tick; // death timestamp for cleanup linger
    }

    // Aircraft ditch / shoot-down reporting is handled by air.rs for its own
    // flows; this path covers ships, subs, bases, and kinetic kills.
    match t.kind {
        PlatformKind::Aircraft => {
            world.emit(
                EventKind::AirShotDown {
                    side: t.side,
                    unit: t.id,
                    unit_name: t.name.clone(),
                    by_side: t.side.other(),
                },
                Visibility::Side(t.side),
            );
        }
        PlatformKind::Base => {
            world.emit(
                EventKind::BaseDestroyed {
                    side: t.side,
                    base: t.id,
                    base_name: t.name.clone(),
                },
                Visibility::All,
            );
        }
        _ => {
            world.emit(
                EventKind::Sunk {
                    side: t.side,
                    unit: t.id,
                    unit_name: t.name.clone(),
                },
                Visibility::Side(t.side),
            );
        }
    }

    // Enemy side: mark any live contact about this unit as sunk (a kill
    // assessment), then remove the contact shortly.
    let enemy = t.side.other();
    let mut sank_contacts = Vec::new();
    {
        let state = &mut world.sides[enemy.index()];
        for c in state.contacts.values_mut() {
            if c.about == target_id && !c.stale {
                c.quality = 0.0;
                c.stale = true;
                sank_contacts.push(c.id);
            }
        }
    }
    for cid in sank_contacts {
        world.emit(
            EventKind::ContactSank { side: enemy, contact: cid },
            Visibility::Side(enemy),
        );
    }
}

/// Fires burn and flooding drains HP over time.
pub fn update_fires(world: &mut World) {
    let ids: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.alive && (e.burn_s > 0.0 || e.flooding_s > 0.0))
        .map(|e| e.id)
        .collect();
    for id in ids {
        let Some(t) = world.entities.get_mut(&id) else { continue };
        if t.burn_s > 0.0 {
            t.burn_s -= crate::world::TICK_SECONDS;
            t.hp -= 0.05;
        }
        if t.flooding_s > 0.0 {
            t.flooding_s -= crate::world::TICK_SECONDS;
            t.hp -= 0.15;
        }
        if t.hp <= 0.0 {
            sink(world, id);
        }
    }
}

/// Remove dead entities after a short lingering period (wrecks, splashes).
pub fn cleanup_dead(world: &mut World) {
    // Projectiles die immediately; platforms linger 120 s for UI fade-out.
    let dead: Vec<(EntityId, i64)> = world
        .entities
        .values()
        .filter(|e| !e.alive)
        .map(|e| (e.id, e.last_emit_tick))
        .collect();
    let now = world.tick;
    for (id, _) in dead {
        let kind = world.entities.get(&id).map(|e| e.kind);
        let linger = match kind {
            Some(k) if k.is_projectile() => 0,
            _ => 120,
        };
        // Use hp<=0 marker time — approximate with tick since death via
        // `last_emit_tick` set to death tick in sink().
        if now - world.entities.get(&id).map(|e| e.last_emit_tick).unwrap_or(now) >= linger {
            world.entities.remove(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Guidance, Targets, WeaponKind};
    use crate::geom::Vec2;
    use crate::scenario::{Scenario, UnitSpawn};
    use crate::types::{DayNight, Side};
    use crate::world::{Weather, World};

    fn test_world(class: &str) -> World {
        let sc = Scenario {
            id: "damage-test".into(),
            name: "Damage Test".into(),
            description: String::new(),
            duration_s: 100_000,
            weather: Weather { visibility_nm: 20.0, jam: [0.0, 0.0] },
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

    fn test_weapon(kind: WeaponKind, damage: f64) -> WeaponDef {
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

    fn last_impact_damage(world: &World) -> f64 {
        world
            .events
            .iter()
            .rev()
            .find_map(|ev| match &ev.kind {
                EventKind::Impact { damage, .. } => Some(*damage),
                _ => None,
            })
            .expect("impact event")
    }

    fn last_crit_label(world: &World) -> Option<String> {
        world.events.iter().rev().find_map(|ev| match &ev.kind {
            EventKind::CriticalHit { system, .. } => Some(system.clone()),
            _ => None,
        })
    }

    #[test]
    fn armor_reduces_gun_damage_and_torpedoes_bypass_it() {
        let gun = test_weapon(WeaponKind::Gun, 100.0);
        let torp = test_weapon(WeaponKind::Torpedo, 100.0);

        let mut w_gun = test_world("ddg_aegis");
        apply_hit(&mut w_gun, EntityId(1), &gun, false);
        let gun_dmg = last_impact_damage(&w_gun);

        let mut w_torp = test_world("ddg_aegis");
        apply_hit(&mut w_torp, EntityId(1), &torp, true);
        let torp_dmg = last_impact_damage(&w_torp);

        // ddg_aegis plate = 0.90: gun ×0.90, torpedo ×(2-0.90)=1.10.
        // Both worlds share seed/tick/entity, so the ±15% damage variance is
        // identical and the ratio isolates the armor factor.
        assert!(
            gun_dmg >= 100.0 * 0.85 * 0.90 - 1e-9
                && gun_dmg <= 100.0 * 1.15 * 0.90 + 1e-9,
            "gun damage {gun_dmg} outside armored band"
        );
        let ratio = torp_dmg / gun_dmg;
        assert!(
            (ratio - 1.10 / 0.90).abs() < 1e-9,
            "torpedo/gun ratio {ratio} should be 1.10/0.90"
        );

        // Patrol boat plate = 1.0: no mitigation.
        let mut w_boat = test_world("pb");
        apply_hit(&mut w_boat, EntityId(1), &gun, false);
        let boat_dmg = last_impact_damage(&w_boat);
        assert!(
            boat_dmg >= 100.0 * 0.85 - 1e-9 && boat_dmg <= 100.0 * 1.15 + 1e-9,
            "unarmored boat damage {boat_dmg} should be unmitigated"
        );
    }

    #[test]
    fn bridge_rudder_cic_rows_appear_in_both_columns() {
        let base = test_world("ddg_aegis");
        let id = EntityId(1);

        for underwater in [false, true] {
            let mut seen = std::collections::BTreeSet::new();
            for seed in 0..2000u64 {
                let mut w = base.clone();
                let mut rng = crate::rng::Rng::new(seed);
                roll_critical(&mut w, id, &mut rng, underwater);
                let label = last_crit_label(&w).expect("critical hit event");
                match label.as_str() {
                    "bridge" => {
                        let e = w.entity(id).unwrap();
                        assert!(
                            (e.systems.fire_control - 0.7).abs() < 1e-9,
                            "bridge fire_control {}",
                            e.systems.fire_control
                        );
                        assert!(
                            (e.systems.mobility - 0.7).abs() < 1e-9,
                            "bridge mobility {}",
                            e.systems.mobility
                        );
                    }
                    "rudder" => {
                        let e = w.entity(id).unwrap();
                        assert_eq!(e.ext.get("rudder_hit").copied(), Some(1.0));
                        assert!(
                            (e.systems.mobility - 0.6).abs() < 1e-9,
                            "rudder mobility {}",
                            e.systems.mobility
                        );
                    }
                    "cic" => {
                        let e = w.entity(id).unwrap();
                        assert_eq!(e.systems.fire_control, 0.0, "CIC must zero fire control");
                        assert!(
                            (e.systems.radar - 0.5).abs() < 1e-9,
                            "CIC radar {}",
                            e.systems.radar
                        );
                    }
                    _ => {}
                }
                seen.insert(label);
                if seen.contains("bridge") && seen.contains("rudder") && seen.contains("cic") {
                    break;
                }
            }
            for want in ["bridge", "rudder", "cic"] {
                assert!(
                    seen.contains(want),
                    "underwater={underwater}: new critical row {want} never rolled; saw {seen:?}"
                );
            }
            assert!(
                seen.len() > 3,
                "underwater={underwater}: legacy rows disappeared; saw {seen:?}"
            );
        }
    }

    #[test]
    fn new_hit_clears_dc_progress_and_restarts_repair_clock() {
        let mut w = test_world("ddg_aegis");
        let id = EntityId(1);
        w.tick = 42;
        {
            let e = w.entity_mut(id).unwrap();
            e.ext.insert("dc_progress".into(), 55.0);
            e.ext.insert("dc_last_hit_tick".into(), 0.0);
            e.ext.insert("dc_next_repair_tick".into(), 300.0);
        }

        apply_hit(&mut w, id, &test_weapon(WeaponKind::Gun, 1.0), false);

        let e = w.entity(id).unwrap();
        assert_eq!(e.ext.get("dc_progress").copied(), Some(0.0));
        assert_eq!(e.ext.get("dc_last_hit_tick").copied(), Some(42.0));
        assert!(
            e.ext.get("dc_next_repair_tick").is_none(),
            "new hit must drop the pending repair schedule"
        );
    }
}

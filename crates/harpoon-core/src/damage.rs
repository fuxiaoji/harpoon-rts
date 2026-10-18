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
    let dmg = weapon.damage * variance;
    let hp_frac = (t.hp / t.hp_max).clamp(0.0, 1.0);
    let new_hp = t.hp - dmg;
    let crit_p = 0.10 + 0.30 * (1.0 - hp_frac);
    let do_crit = new_hp > 0.0 && rng.chance(crit_p);

    if let Some(t) = world.entities.get_mut(&target_id) {
        t.hp = new_hp;
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
        let roll = rng.next_f64();
        let label = if underwater {
            if roll < 0.30 {
                t.flooding_s = (t.flooding_s + 120.0).min(300.0);
                t.systems.mobility = (t.systems.mobility - 0.2).max(0.0);
                "flooding"
            } else if roll < 0.55 {
                t.systems.mobility = (t.systems.mobility - rng.range(0.4, 1.0)).max(0.0);
                t.speed_kts = t.speed_kts.min(t.speed_kts * t.systems.mobility);
                "propulsion"
            } else if roll < 0.75 {
                // Hull penetration: immediate structural damage.
                t.hp -= t.hp_max * 0.10;
                "hull_penetration"
            } else if roll < 0.85 {
                t.systems.sonar = 0.0;
                "sonar"
            } else if roll < 0.95 {
                t.systems.fire_control = (t.systems.fire_control - rng.range(0.5, 1.0)).max(0.0);
                "fire_control"
            } else {
                t.flooding_s = (t.flooding_s + 90.0).min(300.0);
                "flooding"
            }
        } else if roll < 0.20 {
            t.systems.mobility = (t.systems.mobility - rng.range(0.4, 1.0)).max(0.0);
            t.speed_kts = t.speed_kts.min(t.speed_kts * t.systems.mobility);
            "propulsion"
        } else if roll < 0.36 {
            t.systems.radar = 0.0;
            "radar"
        } else if roll < 0.48 {
            t.systems.sonar = 0.0;
            "sonar"
        } else if roll < 0.60 {
            t.systems.fire_control = (t.systems.fire_control - rng.range(0.5, 1.0)).max(0.0);
            "fire_control"
        } else if roll < 0.72 {
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
        } else if roll < 0.88 {
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
            drop(t);
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

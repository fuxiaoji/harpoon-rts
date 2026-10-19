//! Damage adjudication: hit application, critical-hit subsystem rolls, fires,
//! sinking. Implemented as referee rolls seeded from per-entity derived RNG
//! streams — replaces the tabletop's damage/critical hit tables while keeping
//! their structure (damage points + subsystem crits).
//!
//! Critical rows are table-driven from `rules::damage_tables`, which
//! transcribes the four 3-2 columns requested by this workstream:
//! gunfire (3-2 p.4), guided missiles (p.5), underwater attack (p.8) and
//! bombs/airbursts (p.9). See that module for the printed D100 bands and
//! citations. Effect magnitudes and the real-time critical probability remain
//! engine adaptations, so critical events keep the same shape
//! (`EventKind::CriticalHit { system }`) as before.

use crate::db::{WeaponDef, WeaponKind};
use crate::events::{EventKind, Visibility};
use crate::rng::Rng;
use crate::rules::damage_tables::{self, CriticalColumn, EffectSpec, HitZone};
use crate::types::{EntityId, PlatformKind, Side};
use crate::world::{Entity, World};

/// Roll damage + criticals for a successful weapon hit on `target`.
/// `underwater` selects the underwater critical column (flooding-heavy).
///
/// The incoming bearing is recovered from the projectile entity when one is
/// still present (all guided/underwater hits; see `incoming_bearing_rel`).
/// Gun-line hits are adjudicated instantly without a shooter id, so the
/// nearest hostile mount of the same weapon is used as an estimate; if no
/// shooter can be recovered, `None` means broadside (legacy behaviour).
pub fn apply_hit(world: &mut World, target_id: EntityId, weapon: &WeaponDef, underwater: bool) {
    let hit_bearing_rel = incoming_bearing_rel(world, target_id, weapon);
    apply_hit_at(world, target_id, weapon, underwater, hit_bearing_rel);
}

/// Production hit with the actual firing side. BDA is credited only to that
/// side; inferring "the enemy of the victim" would misattribute a RAM or gun
/// misfire against a friendly aircraft.
pub fn apply_hit_from(
    world: &mut World,
    target_id: EntityId,
    weapon: &WeaponDef,
    underwater: bool,
    shooter_side: Side,
) {
    let hit_bearing_rel = incoming_bearing_rel(world, target_id, weapon);
    apply_hit_inner(
        world,
        target_id,
        weapon,
        underwater,
        hit_bearing_rel,
        Some(shooter_side),
    );
}

/// `apply_hit` with an explicit hit bearing relative to the target's bow
/// (0° = from ahead, 180° = from astern). Split out so tests and future
/// geometry-aware call sites can bypass the shooter-recovery estimate.
pub fn apply_hit_at(
    world: &mut World,
    target_id: EntityId,
    weapon: &WeaponDef,
    underwater: bool,
    hit_bearing_rel: Option<f64>,
) {
    apply_hit_inner(world, target_id, weapon, underwater, hit_bearing_rel, None);
}

fn apply_hit_inner(
    world: &mut World,
    target_id: EntityId,
    weapon: &WeaponDef,
    underwater: bool,
    hit_bearing_rel: Option<f64>,
    shooter_side: Option<Side>,
) {
    let Some(t) = world.entities.get(&target_id).cloned() else { return };
    if !t.alive {
        return;
    }
    let mut rng = world.scope_rng(target_id, "damage");
    let variance = rng.range(0.85, 1.15);
    // Armor: surface weapons use the per-location plate; torpedoes use
    // 2 - plate times the torpedo-protection factor and are halved on an
    // extreme bow/stern hit.
    // §14.1.6 (PDF 14-2 p.130, rule p.14-4): armor reduces damage; partial /
    // non-penetrating hits halve it. §14.1.7 (PDF pp.130-131, 14-4/14-5):
    // underwater hits are far less attenuated by belt/deck armor, torpedo
    // protection absorbs part of a deep-running hit, and an extreme bow/stern
    // hit spends its energy pushing water. The per-class, per-zone band and
    // the 0.5 extreme-hit factor live in `rules::damage_tables`.
    let armor = crate::rules::damage_ext::armor_factor(&t, weapon, hit_bearing_rel);
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
    bda_to_shooter(world, target_id, weapon, dmg, new_hp.max(0.0), shooter_side);

    if do_crit {
        let column = critical_column_for(weapon, underwater);
        roll_critical(world, target_id, &mut rng, column);
    }

    // Location critical: a hit in the bow or stern third has an independent
    // 10% chance of a light "bow_collapse" / "rudder" critical (work-order
    // rule; the manual's rudder row is 3-2 p.4/p.8 D100 96-00, and §14.1.7
    // notes that extreme bow/stern hits spend their energy on the water).
    // This is additional to the main column critical.
    let still_afloat = world
        .entities
        .get(&target_id)
        .map(|e| e.alive && e.hp > 0.0)
        .unwrap_or(false);
    if still_afloat {
        maybe_location_light_crit(world, target_id, &mut rng, weapon, hit_bearing_rel);
    }

    // A critical (magazine detonation) may have zeroed HP after the impact
    // amount was computed; sink on the *current* HP, not `new_hp`.
    let hp_now = world.entities.get(&target_id).map(|e| e.hp).unwrap_or(0.0);
    if hp_now <= 0.0 {
        sink(world, target_id);
    }
}

/// Pick the 3-2 critical column for a weapon:
/// - underwater flag (torpedo / ASW rocket hit) → p.8 underwater column;
/// - Bomb / Rocket / AswRocket → p.9 airburst & fragments column;
/// - Ascm / Slcm / Sam / Aam → p.5 guided-weapon column;
/// - guns and CIWS → p.4 gunfire column.
///
/// The manual has no column literally named "bomb"; the p.9 `空爆和弹片命中`
/// (airburst & fragments) column is the only bomb-related column, so it is
/// used as the work order's bomb column. Contact-fused bombs scoring direct
/// hits are an engine adaptation of that column (estimated).
fn critical_column_for(weapon: &WeaponDef, underwater: bool) -> CriticalColumn {
    if underwater {
        return CriticalColumn::Torpedo;
    }
    match weapon.kind {
        WeaponKind::Torpedo | WeaponKind::Lwt => CriticalColumn::Torpedo,
        WeaponKind::Bomb | WeaponKind::Rocket | WeaponKind::AswRocket => CriticalColumn::Bomb,
        WeaponKind::Ascm | WeaponKind::Slcm | WeaponKind::Sam | WeaponKind::Aam => {
            CriticalColumn::Missile
        }
        WeaponKind::Gun | WeaponKind::Ciws => CriticalColumn::Gun,
    }
}

/// Recover the bearing the hit came from, relative to the target's bow.
///
/// At impact time the weapon projectile still exists in the world (the shared
/// weapons loop removes it right after `apply_hit`), so its position/course
/// gives the true incoming bearing. Instant gun-line hits have no projectile
/// and the shared call site does not pass a shooter id, so the nearest live
/// hostile platform carrying the same weapon and in range is used as an
/// estimate. `None` (no projectile, no plausible shooter) = broadside.
fn incoming_bearing_rel(
    world: &World,
    target_id: EntityId,
    weapon: &WeaponDef,
) -> Option<f64> {
    let t = world.entities.get(&target_id)?;
    if !t.alive {
        return None;
    }
    let projectile = world
        .entities
        .values()
        .filter(|e| e.alive && e.kind.is_projectile())
        .filter_map(|e| {
            let data = e.projectile.as_ref()?;
            if data.target == Some(target_id) && data.weapon == weapon.id {
                Some((e, t.pos.range_to(e.pos)))
            } else {
                None
            }
        })
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    if let Some((p, range)) = projectile {
        // If the projectile is exactly on top of the target, its course
        // (pointing at the target) is more stable than a zero-length bearing.
        let from = if range > 0.005 {
            t.pos.bearing_to(p.pos)
        } else {
            crate::geom::normalize_deg(p.course_deg + 180.0)
        };
        return Some(crate::geom::angle_diff(t.course_deg, from).abs());
    }

    let shooter = world
        .entities
        .values()
        .filter(|e| e.alive && e.side != t.side && !e.kind.is_projectile())
        .filter(|e| e.weapons.iter().any(|m| m.weapon == weapon.id && m.mounts > 0))
        .filter(|e| t.pos.range_to(e.pos) <= weapon.range_nm.max(0.1) * 1.05)
        .min_by(|a, b| {
            t.pos
                .range_to(a.pos)
                .partial_cmp(&t.pos.range_to(b.pos))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    let from = t.pos.bearing_to(shooter.pos);
    Some(crate::geom::angle_diff(t.course_deg, from).abs())
}

/// Broadcast battle-damage assessment to the shooter's side when they track
/// the victim.
fn bda_to_shooter(
    world: &mut World,
    target_id: EntityId,
    weapon: &WeaponDef,
    dmg: f64,
    hp_left: f64,
    shooter_side: Option<Side>,
) {
    let Some(shooter_side) = shooter_side else { return };
    let Some(t) = world.entities.get(&target_id) else { return };
    if t.side == shooter_side {
        return;
    }
    let tracked = world
        .sides[shooter_side.index()]
        .contacts
        .values()
        .any(|c| c.about == target_id && !c.stale && c.quality > 0.25);
    if !tracked {
        return;
    }
    let record = world.sides[shooter_side.index()]
        .bda
        .entry(target_id)
        .or_default();
    record.hits += 1;
    record.damage += dmg;
    if t.hp_max > 0.0 {
        record.hp_fraction = Some((hp_left / t.hp_max).clamp(0.0, 1.0));
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

/// Draw one D100 (`0..=99`, printed `01..=00`) and apply the matching row of
/// `column`. This is the production critical entry; `apply_hit_at` selects the
/// column from the weapon kind and the underwater flag.
fn roll_critical(
    world: &mut World,
    target_id: EntityId,
    rng: &mut Rng,
    column: CriticalColumn,
) -> Option<String> {
    let roll = (rng.next_f64() * 100.0) as u8;
    apply_critical_roll(world, target_id, rng, column, roll)
}

/// Apply a specific roll of a specific column. Private, but callable from the
/// module tests to pin every band without relying on RNG luck.
fn apply_critical_roll(
    world: &mut World,
    target_id: EntityId,
    rng: &mut Rng,
    column: CriticalColumn,
    roll: u8,
) -> Option<String> {
    let row = *damage_tables::row_for_roll(column, roll)?;
    let t = world.entities.get(&target_id)?;
    if !t.alive {
        return None;
    }
    let (side, class) = (t.side, t.class.clone());
    // 3-2 p.7: aviation ships are carriers, helicopter carriers, LHA/LPH or
    // any ship with at least half its main deck given to aircraft; the
    // database marks these with a `deck`.
    let is_aviation = world
        .db
        .platforms
        .get(&class)
        .and_then(|p| p.deck.as_ref())
        .is_some();

    let system = {
        let Some(t) = world.entities.get_mut(&target_id) else { return None };
        apply_effect(t, rng, row.effect, column, is_aviation)
    };

    match system {
        Some(system) => {
            world.emit(
                EventKind::CriticalHit {
                    side,
                    unit: target_id,
                    system: system.to_string(),
                },
                Visibility::Side(side),
            );
            Some(system.to_string())
        }
        None => {
            // 3-2 p.2 general rule: if the rolled system is not fitted,
            // ignore the critical. Record the ignored row for rules audit.
            world.emit(
                EventKind::RulesEvent {
                    domain: "damage".to_string(),
                    rule: "critical_ignored".to_string(),
                    side: Some(side),
                    unit: Some(target_id),
                    contact: None,
                    detail: format!(
                        "{} column row `{}` (roll {roll}) ignored: system not fitted",
                        column_name(column),
                        row.label
                    ),
                },
                Visibility::Side(side),
            );
            None
        }
    }
}

/// Apply one table row. Returns the subsystem label emitted in
/// `EventKind::CriticalHit`, or `None` when the row does not apply to the
/// target (3-2 p.2: ignore systems the ship does not carry).
fn apply_effect(
    t: &mut Entity,
    rng: &mut Rng,
    effect: EffectSpec,
    column: CriticalColumn,
    is_aviation: bool,
) -> Option<&'static str> {
    match effect {
        // 3-2 p.4-p.9 `武器`: a random mount (the engine has no separate
        // director entries, so a director is treated as a mount). If the
        // mount has explosive ammunition, D10 = 0 explodes its magazine.
        EffectSpec::Weapon => apply_weapon_effect(t, rng, column),
        // 3-2 p.9 D100 61-90: flight deck on aviation ships, weapon on all
        // other ships.
        EffectSpec::WeaponOrFlightDeck => {
            if is_aviation {
                apply_flight_deck_effect(t)
            } else {
                apply_weapon_effect(t, rng, column)
            }
        }
        // `探测器`: one sensor is knocked out; countermeasures are separate
        // and not modelled as sensors here. The station-keeping weighting is
        // an engine estimate (surface 65% radar, submarine 75% sonar), and
        // the other sensor is used when the preferred one is already dead.
        EffectSpec::Detector => {
            let prefer_radar = if t.kind == PlatformKind::Sub { 0.25 } else { 0.65 };
            let want_radar = rng.chance(prefer_radar);
            let radar_ok = t.systems.radar > 0.0;
            let sonar_ok = t.systems.sonar > 0.0;
            let hit_radar = radar_ok && (want_radar || !sonar_ok);
            let label = if hit_radar {
                t.systems.radar = 0.0;
                "radar"
            } else if sonar_ok {
                t.systems.sonar = 0.0;
                "sonar"
            } else {
                // No sensor left to destroy: the row still hits the sensor
                // compartment but has no game effect.
                "detector"
            };
            Some(label)
        }
        // `作战情报中心`: command space destroyed. Weapons without a local
        // fire-control mode fail; the engine folds that into fire control = 0
        // and knocks the command radar down. The navigation radar exception
        // is not separately modelled.
        EffectSpec::Cic => {
            t.systems.fire_control = 0.0;
            t.systems.radar = (t.systems.radar - 0.5).max(0.0);
            Some("cic")
        }
        // `轮机*`: speed drops to the next damage/speed level and a fire
        // starts. The real-time adaptation is a large mobility loss plus the
        // legacy speed clip; electric-power loss is not separately modelled.
        EffectSpec::Propulsion => {
            t.systems.mobility = (t.systems.mobility - rng.range(0.4, 1.0)).max(0.0);
            t.speed_kts = t.speed_kts.min(t.speed_kts * t.systems.mobility);
            Some("propulsion")
        }
        // `进水*`: severe flooding; the engine's continuous HP drain (see
        // `update_fires`) plus a mobility knock.
        EffectSpec::Flooding => {
            t.flooding_s = (t.flooding_s + 120.0).min(300.0);
            t.systems.mobility = (t.systems.mobility - 0.2).max(0.0);
            Some("flooding")
        }
        // `火灾`: fire severity is a D6 severity table; the engine starts a
        // 60-second burn instead.
        EffectSpec::Fire => {
            t.burn_s = (t.burn_s + 60.0).min(180.0);
            Some("fire")
        }
        // `舰桥`: conning station destroyed; the engine models the loss of
        // local control as fire-control and mobility damage.
        EffectSpec::Bridge => {
            t.systems.fire_control = (t.systems.fire_control - 0.3).max(0.0);
            t.systems.mobility = (t.systems.mobility - 0.3).max(0.0);
            Some("bridge")
        }
        // `舵`: the ext marker lets movement model jammed steering later
        // (movement.rs is a shared file); mobility drops immediately.
        EffectSpec::Rudder => {
            t.ext.insert("rudder_hit".to_string(), 1.0);
            t.systems.mobility = (t.systems.mobility - 0.4).max(0.0);
            Some("rudder")
        }
        // `飞行甲板*`: only aviation ships; stops launches/recovery and
        // starts a fire in the engine's abstraction.
        EffectSpec::FlightDeck => {
            if is_aviation {
                apply_flight_deck_effect(t)
            } else {
                None
            }
        }
    }
}

/// 3-2 p.4-p.9 `武器`: knock out one mount. The D10 magazine check is skipped
/// for the bomb column because 3-2 p.2 formal change 1 says an airburst
/// weapon critical never causes a magazine explosion.
fn apply_weapon_effect(
    t: &mut Entity,
    rng: &mut Rng,
    column: CriticalColumn,
) -> Option<&'static str> {
    let victim = t
        .weapons
        .iter_mut()
        .filter(|m| m.ammo > 0)
        .min_by_key(|m| m.mounts);
    match victim {
        Some(m) => {
            let magazine = column != CriticalColumn::Bomb && (rng.next_f64() * 10.0) as u8 == 0;
            if magazine {
                // "甲板下导弹或炮弹库的爆炸将摧毁该舰船" (3-2 p.4).
                t.hp = 0.0;
                Some("magazine_detonation")
            } else {
                m.ammo = 0;
                Some("weapon_mount")
            }
        }
        // Unarmed or all ammunition spent: the hit lands on the
        // superstructure (the manual's unarmed-merchant rule maps here).
        None => Some("superstructure"),
    }
}

/// `飞行甲板*` / carrier bomb rows: launch and recovery disabled, plus a fire.
fn apply_flight_deck_effect(t: &mut Entity) -> Option<&'static str> {
    t.ext.insert("flight_deck_hit".to_string(), 1.0);
    t.burn_s = (t.burn_s + 60.0).min(180.0);
    Some("flight_deck")
}

/// Independent 10% bow/stern light critical (work-order rule, estimated
/// magnitudes). Bow → `bow_collapse`; stern → `rudder`; broadside → none.
fn maybe_location_light_crit(
    world: &mut World,
    target_id: EntityId,
    rng: &mut Rng,
    weapon: &WeaponDef,
    hit_bearing_rel: Option<f64>,
) -> Option<&'static str> {
    if !rng.chance(0.10) {
        return None;
    }
    let zone = damage_tables::hit_zone(hit_bearing_rel);
    let label = match zone {
        HitZone::Bow => "bow_collapse",
        HitZone::Stern => "rudder",
        HitZone::Side => return None,
    };
    let Some((side, kind)) = world.entities.get(&target_id).map(|t| (t.side, t.kind)) else {
        return None;
    };
    if !matches!(kind, PlatformKind::Ship | PlatformKind::Sub) {
        return None;
    }
    {
        let Some(t) = world.entities.get_mut(&target_id) else { return None };
        if !t.alive {
            return None;
        }
        // Keep the stern marker consistent with the main rudder row so
        // movement/steering code has one key to look for.
        let ext_key = if zone == HitZone::Bow { "bow_collapse" } else { "rudder_hit" };
        t.ext.insert(ext_key.to_string(), 1.0);
        t.systems.mobility = (t.systems.mobility - 0.2).max(0.0);
    }
    world.emit(
        EventKind::CriticalHit {
            side,
            unit: target_id,
            system: label.to_string(),
        },
        Visibility::Side(side),
    );
    world.emit(
        EventKind::RulesEvent {
            domain: "damage".to_string(),
            rule: "location_light_crit".to_string(),
            side: Some(side),
            unit: Some(target_id),
            contact: None,
            detail: format!(
                "{} hit by {}: {label} light critical (10% bow/stern location effect)",
                zone_name(zone),
                weapon.name
            ),
        },
        Visibility::Side(side),
    );
    Some(label)
}

fn column_name(column: CriticalColumn) -> &'static str {
    match column {
        CriticalColumn::Gun => "gun",
        CriticalColumn::Missile => "missile",
        CriticalColumn::Torpedo => "torpedo",
        CriticalColumn::Bomb => "bomb",
    }
}

fn zone_name(zone: HitZone) -> &'static str {
    match zone {
        HitZone::Bow => "bow",
        HitZone::Side => "broadside",
        HitZone::Stern => "stern",
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
    // Sonobuoys are expendable sensors (1 DP), not scored platforms: their
    // loss must never count toward kill counts or weighted scores.
    let is_buoy = t.class == crate::rules::asw::SONOBUOY_CLASS;
    if !t.kind.is_projectile() && !is_buoy {
        world.losses[t.side.index()] += 1;
        if let Some(kind_key) = World::kill_kind(t.kind) {
            world
                .losses_by_kind
                .entry(kind_key.to_string())
                .or_default()[t.side.index()] += 1;
        }
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
    let had_confirmed_contact = !sank_contacts.is_empty();
    for cid in sank_contacts {
        world.emit(
            EventKind::ContactSank { side: enemy, contact: cid },
            Visibility::Side(enemy),
        );
    }
    // One confirmed kill per destroyed platform, even if several tracks
    // resolved onto it.
    if had_confirmed_contact && !t.kind.is_projectile() && !is_buoy {
        world.sides[enemy.index()].confirmed_kills += 1;
        if let Some(kind_key) = World::kill_kind(t.kind) {
            *world.sides[enemy.index()]
                .confirmed_kills_by_kind
                .entry(kind_key.to_string())
                .or_insert(0) += 1;
        }
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

    /// Target (Blue, cvn) at `target_pos`, shooter (Red, ddg_aegis) at
    /// `shooter_pos`; both face north. Used to exercise hit-bearing recovery.
    fn pair_world(target_pos: Vec2, shooter_pos: Vec2) -> World {
        let sc = Scenario {
            id: "damage-pair-test".into(),
            name: "Damage Pair Test".into(),
            description: String::new(),
            context: Default::default(),
            duration_s: 100_000,
            weather: Weather { visibility_nm: 20.0, jam: [0.0, 0.0], overcast: 0.0, rain: 0.0 },
            day_night: DayNight::Day,
            seed: 20251018,
            victory: Default::default(),
            units: vec![
                UnitSpawn {
                    side: Side::Blue,
                    class: "cvn".into(),
                    name: Some("TARGET".into()),
                    pos: target_pos,
                    course_deg: 0.0,
                    speed_kts: 0.0,
                    depth_m: None,
                    altitude_m: None,
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: false,
                },
                UnitSpawn {
                    side: Side::Red,
                    class: "ddg_aegis".into(),
                    name: Some("SHOOTER".into()),
                    pos: shooter_pos,
                    course_deg: 0.0,
                    speed_kts: 0.0,
                    depth_m: None,
                    altitude_m: None,
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: false,
                },
            ],
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
            let column =
                if underwater { CriticalColumn::Torpedo } else { CriticalColumn::Gun };
            let mut seen = std::collections::BTreeSet::new();
            for seed in 0..2000u64 {
                let mut w = base.clone();
                let mut rng = crate::rng::Rng::new(seed);
                roll_critical(&mut w, id, &mut rng, column);
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
    fn critical_columns_follow_the_weapon_kind() {
        // 3-2 p.4 gunfire, p.5 guided weapons, p.8 underwater, p.9 airburst.
        assert_eq!(
            critical_column_for(&test_weapon(WeaponKind::Gun, 10.0), false),
            CriticalColumn::Gun
        );
        assert_eq!(
            critical_column_for(&test_weapon(WeaponKind::Ciws, 10.0), false),
            CriticalColumn::Gun
        );
        assert_eq!(
            critical_column_for(&test_weapon(WeaponKind::Ascm, 40.0), false),
            CriticalColumn::Missile
        );
        assert_eq!(
            critical_column_for(&test_weapon(WeaponKind::Slcm, 40.0), false),
            CriticalColumn::Missile
        );
        assert_eq!(
            critical_column_for(&test_weapon(WeaponKind::Aam, 10.0), false),
            CriticalColumn::Missile
        );
        assert_eq!(
            critical_column_for(&test_weapon(WeaponKind::Torpedo, 40.0), false),
            CriticalColumn::Torpedo
        );
        assert_eq!(
            critical_column_for(&test_weapon(WeaponKind::Torpedo, 40.0), true),
            CriticalColumn::Torpedo
        );
        assert_eq!(
            critical_column_for(&test_weapon(WeaponKind::AswRocket, 10.0), true),
            CriticalColumn::Torpedo
        );
        assert_eq!(
            critical_column_for(&test_weapon(WeaponKind::Bomb, 40.0), false),
            CriticalColumn::Bomb
        );
        assert_eq!(
            critical_column_for(&test_weapon(WeaponKind::Rocket, 10.0), false),
            CriticalColumn::Bomb
        );
    }

    #[test]
    fn table_rolls_apply_the_printed_column_effects() {
        let id = EntityId(1);

        // Gun column D100 36-45 → CIC (3-2 p.4).
        let mut w = test_world("ddg_aegis");
        let mut rng = Rng::new(1);
        assert_eq!(
            apply_critical_roll(&mut w, id, &mut rng, CriticalColumn::Gun, 40).as_deref(),
            Some("cic")
        );
        assert_eq!(w.entity(id).unwrap().systems.fire_control, 0.0);
        assert!((w.entity(id).unwrap().systems.radar - 0.5).abs() < 1e-9);

        // Missile column D100 96-00 → bridge (3-2 p.5).
        let mut w = test_world("ddg_aegis");
        let mut rng = Rng::new(2);
        assert_eq!(
            apply_critical_roll(&mut w, id, &mut rng, CriticalColumn::Missile, 96).as_deref(),
            Some("bridge")
        );
        assert!((w.entity(id).unwrap().systems.fire_control - 0.7).abs() < 1e-9);
        assert!((w.entity(id).unwrap().systems.mobility - 0.7).abs() < 1e-9);

        // Torpedo column D100 51-80 → flooding (3-2 p.8).
        let mut w = test_world("ddg_aegis");
        let mut rng = Rng::new(3);
        assert_eq!(
            apply_critical_roll(&mut w, id, &mut rng, CriticalColumn::Torpedo, 55).as_deref(),
            Some("flooding")
        );
        assert!(w.entity(id).unwrap().flooding_s >= 120.0);

        // Bomb column D100 91-95 → flight deck on an aviation ship
        // (3-2 p.9); the carrier database entry has a deck.
        let mut w = test_world("cvn");
        let mut rng = Rng::new(4);
        assert_eq!(
            apply_critical_roll(&mut w, id, &mut rng, CriticalColumn::Bomb, 92).as_deref(),
            Some("flight_deck")
        );
        assert_eq!(w.entity(id).unwrap().ext.get("flight_deck_hit").copied(), Some(1.0));

        // Bomb column D100 61-90 is a weapon hit on a non-aviation ship.
        let mut w = test_world("ddg_aegis");
        let mut rng = Rng::new(5);
        assert_eq!(
            apply_critical_roll(&mut w, id, &mut rng, CriticalColumn::Bomb, 65).as_deref(),
            Some("weapon_mount")
        );

        // Bomb column D100 91-95 is flight deck only: on a non-aviation ship
        // the p.2 general rule ignores it and records a rules event.
        let mut w = test_world("ddg_aegis");
        let mut rng = Rng::new(6);
        assert_eq!(apply_critical_roll(&mut w, id, &mut rng, CriticalColumn::Bomb, 92), None);
        assert!(w.events.iter().any(|ev| matches!(
            &ev.kind,
            EventKind::RulesEvent { domain, rule, .. }
                if domain == "damage" && rule == "critical_ignored"
        )));

        // 3-2 p.2 formal change 1: airburst weapon criticals never explode a
        // magazine. Sweep seeds through the bomb weapon/flight-deck row.
        let bomb_base = test_world("ddg_aegis");
        for seed in 0..500u64 {
            let mut w = bomb_base.clone();
            let mut rng = Rng::new(seed);
            let label = apply_critical_roll(&mut w, id, &mut rng, CriticalColumn::Bomb, 70);
            assert_ne!(
                label.as_deref(),
                Some("magazine_detonation"),
                "bomb column must not cause magazine explosions"
            );
        }
    }

    #[test]
    fn incoming_bearing_is_recovered_from_the_projectile() {
        let mut missile = test_weapon(WeaponKind::Ascm, 100.0);
        missile.guidance = Guidance::SeaSkimmer;

        // Target faces north at the origin; the bearing the hit came from is
        // the reciprocal of the projectile's position relative to the target.
        for (shooter_pos, want) in [
            (Vec2::new(0.0, 10.0), 0.0),   // shooter ahead → bow hit
            (Vec2::new(10.0, 0.0), 90.0),  // shooter to starboard → side hit
            (Vec2::new(0.0, -10.0), 180.0), // shooter astern → stern hit
        ] {
            let mut w = pair_world(Vec2::new(0.0, 0.0), shooter_pos);
            let target = EntityId(1);
            let shooter = EntityId(2);
            assert_eq!(w.entity(shooter).unwrap().side, Side::Red);
            crate::weapons::launch(&mut w, shooter, &missile, Some(target), None)
                .expect("missile launch");
            let bearing = incoming_bearing_rel(&w, target, &missile).expect("incoming bearing");
            assert!(
                (bearing - want).abs() < 1e-9,
                "shooter at {shooter_pos:?}: bearing {bearing}, want {want}"
            );
        }
    }

    #[test]
    fn zone_armor_changes_damage_by_hit_bearing() {
        let gun = test_weapon(WeaponKind::Gun, 100.0);
        let torp = test_weapon(WeaponKind::Torpedo, 100.0);
        let id = EntityId(1);

        let hit = |weapon: &WeaponDef, underwater: bool, bearing: f64| {
            let mut w = test_world("cvn");
            apply_hit_at(&mut w, id, weapon, underwater, Some(bearing));
            last_impact_damage(&w)
        };
        let side = hit(&gun, false, 90.0);
        let bow = hit(&gun, false, 0.0);
        let stern = hit(&gun, false, 180.0);
        // Estimated capital bands: 0.90 bow / 1.00 side / 0.80 stern. The
        // ±15% variance is identical across these fresh worlds, so the ratios
        // isolate the location factor.
        assert!((bow / side - 0.90).abs() < 1e-9, "bow/side {}", bow / side);
        assert!((stern / side - 0.80).abs() < 1e-9, "stern/side {}", stern / side);

        // Torpedo: 0.75 broadside ((2 - 1.00) * 0.75); extreme bow is halved
        // by §14.1.7 → 0.4125, i.e. 0.55 of the broadside factor.
        let torp_side = hit(&torp, true, 90.0);
        let torp_bow = hit(&torp, true, 0.0);
        assert!((torp_side / side - 0.75).abs() < 1e-9);
        assert!((torp_bow / torp_side - 0.55).abs() < 1e-9);

        // Unknown bearing = broadside (legacy behaviour): the ddg keeps its
        // 0.90 plate, so the two calls agree exactly.
        let mut legacy = test_world("ddg_aegis");
        apply_hit(&mut legacy, id, &gun, false);
        let legacy_dmg = last_impact_damage(&legacy);
        let mut explicit = test_world("ddg_aegis");
        apply_hit_at(&mut explicit, id, &gun, false, None);
        let explicit_dmg = last_impact_damage(&explicit);
        assert!((legacy_dmg - explicit_dmg).abs() < 1e-12);
    }

    #[test]
    fn bow_and_stern_hits_add_a_ten_percent_light_critical() {
        let gun = test_weapon(WeaponKind::Gun, 1.0);
        let id = EntityId(1);
        let base = test_world("ddg_aegis");
        let mut bow = 0u32;
        let mut stern = 0u32;
        let mut side = 0u32;

        for seed in 0..5000u64 {
            let mut w = base.clone();
            let mut rng = Rng::new(seed);
            match maybe_location_light_crit(&mut w, id, &mut rng, &gun, Some(0.0)) {
                Some("bow_collapse") => {
                    bow += 1;
                    let e = w.entity(id).unwrap();
                    assert_eq!(e.ext.get("bow_collapse").copied(), Some(1.0));
                    assert!((e.systems.mobility - 0.8).abs() < 1e-9);
                }
                Some(other) => panic!("bow hit produced {other}"),
                None => {}
            }

            let mut w = base.clone();
            let mut rng = Rng::new(seed);
            match maybe_location_light_crit(&mut w, id, &mut rng, &gun, Some(180.0)) {
                Some("rudder") => {
                    stern += 1;
                    let e = w.entity(id).unwrap();
                    assert_eq!(e.ext.get("rudder_hit").copied(), Some(1.0));
                    assert!((e.systems.mobility - 0.8).abs() < 1e-9);
                }
                Some(other) => panic!("stern hit produced {other}"),
                None => {}
            }

            let mut w = base.clone();
            let mut rng = Rng::new(seed);
            if maybe_location_light_crit(&mut w, id, &mut rng, &gun, Some(90.0)).is_some() {
                side += 1;
            }
        }

        // 10% per eligible hit; 5000 trials make a 5-15% band unambiguous.
        assert!((250..=750).contains(&bow), "bow light crits {bow}/5000");
        assert!((250..=750).contains(&stern), "stern light crits {stern}/5000");
        assert_eq!(side, 0, "broadside hits must not roll a location critical");
    }

    #[test]
    fn bomb_hits_integrate_through_the_bomb_column() {
        let bomb = test_weapon(WeaponKind::Bomb, 1.0);
        let base = test_world("cvn");
        let id = EntityId(1);
        let mut labels = std::collections::BTreeSet::new();
        let mut flight_deck = 0u32;

        for tick in 0..2000i64 {
            let mut w = base.clone();
            w.tick = tick;
            apply_hit_at(&mut w, id, &bomb, false, None);
            for ev in &w.events {
                if let EventKind::CriticalHit { system, .. } = &ev.kind {
                    labels.insert(system.clone());
                    if system == "flight_deck" {
                        flight_deck += 1;
                    }
                }
            }
        }

        assert!(flight_deck > 0, "bomb column never produced a flight-deck critical");
        assert!(
            !labels.contains("magazine_detonation"),
            "airburst weapon criticals must not explode magazines; saw {labels:?}"
        );
        // Only rows present in the 3-2 p.9 column may appear.
        let allowed = [
            "weapon_mount",
            "superstructure",
            "radar",
            "sonar",
            "detector",
            "propulsion",
            "flight_deck",
            "bridge",
        ];
        for label in &labels {
            assert!(
                allowed.contains(&label.as_str()),
                "bomb column produced a non-bomb-row system {label}; saw {labels:?}"
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

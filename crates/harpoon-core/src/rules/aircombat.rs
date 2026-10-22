//! Air combat depth: merge/dogfight adjudication, maneuver levels, air-to-air
//! gunnery.
//!
//! Hook: `step(world)` runs the merge engagement model each tick — opposing
//! aircraft within merge range exchange gun passes with maneuver-modified
//! Pk until one side disengages (passes exhausted, fuel threshold, or death).
//!
//! Rules basis (Harpoon V, `14-2.HpnV_Rules_1_Jan_26.pdf`):
//! - §9.1 (PDF p.102 / printed 9-1): aircraft have two Maneuver Ratings by
//!   load state. The RTS adaptation here uses `ext["maneuver"]` 0/1/2 =
//!   straight / standard / violent (defensive) — an abstraction of the
//!   load-dependent rating pair, not a claim that 0/1/2 are the official
//!   numeric ratings.
//! - §9.4 (PDF p.102 / printed 9-1): dogfight is declared within 6 nmi when
//!   the fastest aircraft's max speed is 401+ kt; dogfight rounds are one
//!   minute each, three to a Tactical Turn. This engine compresses a round
//!   to a 15-second pass (work order; declared RTS adaptation, not tabletop).
//! - §9.4.1 (PDF p.102 / printed 9-1): D100 position roll, modified by the
//!   Maneuver Rating difference (5% per point) and by ±150 kt speed
//!   differences (10%). The scalar gun Pk below (base 0.12, speed >150 kt
//!   ×0.7, altitude >1500 m ×0.8) is the work-order abstraction of the
//!   position roll + gun attack chain — **estimated** against the official
//!   tables.
//! - §9.4.2 (PDF p.103 / printed 9-2): a successful gun position is resolved
//!   on the Aircraft Gun Attack Table (printed p.9-3) as a kill chance based
//!   on gun rating and target Damage Rating. Our hp pool (db_data.rs, DV
//!   magnitudes) replaces the single kill roll: a hit removes
//!   `GUN_HIT_FRACTION` of the target's hp (estimated: about three cannon
//!   hits ≈ one table kill), and hp ≤ 0 reuses `damage::sink`.
//! - §9.4.4 (PDF p.103 / printed 9-2): defensive maneuvering improves the
//!   Maneuver Rating by 0.5 and halves defensive guns. Here "violent" level 2
//!   is the RTS defensive-maneuver state: +25% survival against AAMs but
//!   ×1.2 incoming gun hit chance (it gives up offensive position).
//! - §9.4.5/9.4.6 (PDF p.103 / printed 9-2): escaping and ending a dogfight
//!   (tabletop: automatic end after nine one-minute rounds). This engine ends
//!   after seven 15-second passes (work-order `pass > 6`), earlier on death or
//!   fuel < 15%.
//! - §9.5.1 (PDF p.105 / printed 9-4): evasive maneuvers + countermeasures
//!   reduce an AAM's chance to hit. The +25% violent-maneuver survival factor
//!   is exposed as `aam_evasion_multiplier`; the AAM terminal roll itself
//!   stays in `weapons.rs` (another workstream owns that file).
//!
//! State keys owned in `Entity.ext`:
//! - `"maneuver"` — 0 straight, 1 standard, 2 violent.
//! - `"merge_with"` — opponent EntityId.0 while merged.
//! - `"merge_passes"` — completed 15 s passes in this merge.
//! - `"merge_timer_s"` — seconds accumulated toward the next pass.
//! - `"merge_cooldown_s"` — brief no-re-merge window after disengaging
//!   (estimated RTS value; lets movement separate the aircraft).

use crate::damage;
use crate::events::{EventKind, Visibility};
use crate::types::{EntityId, PlatformKind, Side};
use crate::world::{Entity, World, TICK_SECONDS};

/// §9.4 (printed 9-1): dogfight radius is 6 nmi for aircraft whose max speed
/// is 401+ kt. This engine uses the top band for all aircraft.
pub const MERGE_RADIUS_NM: f64 = 6.0;
/// Work order: one dogfight pass every 15 s. Tabletop uses one-minute rounds
/// (§9.4); this is a declared RTS time compression.
pub const PASS_INTERVAL_S: f64 = 15.0;
/// Work order: end a merge when the pass count is greater than 6.
pub const PASS_LIMIT: u32 = 6;
/// Work-order base gun hit probability per pass (§9.4.2 abstraction,
/// estimated against the Aircraft Gun Attack Table).
pub const GUN_BASE_PK: f64 = 0.12;
/// A gun hit removes this fraction of the target's maximum hp (estimated:
/// roughly three cannon hits ≈ one Aircraft Gun Attack Table kill).
pub const GUN_HIT_FRACTION: f64 = 0.34;
/// HP fraction below which the aircraft automatically selects violent
/// maneuvering (work order; mirrors §9.4.4 defensive maneuvering).
pub const AUTO_EVADE_HP_FRAC: f64 = 0.30;
/// Fuel remaining below this fraction forces disengagement (work order).
pub const DISENGAGE_FUEL_FRAC: f64 = 0.15;
/// Incoming gun Pk multiplier against a violently maneuvering defender
/// (work order; it sacrifices offensive position).
pub const EVASIVE_INCOMING_GUN_MULT: f64 = 1.2;
/// AAM hit-probability multiplier for a violently maneuvering target:
/// +25% survival (work order, §9.5.1).
pub const EVASIVE_AAM_SURVIVAL_MULT: f64 = 0.75;
/// Post-disengage no-re-merge window. Estimated RTS value: without it, two
/// aircraft still inside 6 nmi would immediately re-merge every tick.
const MERGE_COOLDOWN_S: f64 = 60.0;

const KEY_MANEUVER: &str = "maneuver";
const KEY_MERGE_WITH: &str = "merge_with";
const KEY_MERGE_PASSES: &str = "merge_passes";
const KEY_MERGE_TIMER: &str = "merge_timer_s";
const KEY_MERGE_COOLDOWN: &str = "merge_cooldown_s";

/// Extra DB (gun Pk vs air, merge parameters).
///
/// No rows are needed: the work order fixes the dogfight gun Pk (0.12 base)
/// and the merge geometry is in this module. AAMs remain ordinary
/// `WeaponDef`s resolved by `weapons.rs`.
pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

/// Public hook for the AAM terminal step (§9.5.1): violent maneuvering gives
/// the target +25% survival, i.e. returns 0.75. `weapons.rs` belongs to
/// another workstream and does not call this yet; it is exported so the
/// missile loop can adopt it without duplicating air-combat state here.
pub fn aam_evasion_multiplier(target: &Entity) -> f64 {
    if maneuver_of(target) >= 2.0 {
        EVASIVE_AAM_SURVIVAL_MULT
    } else {
        1.0
    }
}

/// Per-tick dogfight adjudication.
pub fn step(world: &mut World) {
    auto_maneuver(world);
    tick_merge_cooldowns(world);
    process_merges(world);
    start_merges(world);
}

/// Derive the maneuver level: default standard (1), forced violent (2) when
/// the aircraft is below 30% hp (work order). Existing manually set levels
/// are preserved otherwise and clamped to 0..=2.
fn auto_maneuver(world: &mut World) {
    for e in world.entities.values_mut() {
        if !e.alive || e.kind != PlatformKind::Aircraft {
            continue;
        }
        let hp_frac = if e.hp_max > 0.0 {
            (e.hp / e.hp_max).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let current = e.ext.get(KEY_MANEUVER).copied().unwrap_or(1.0).clamp(0.0, 2.0);
        let next = if hp_frac < AUTO_EVADE_HP_FRAC { 2.0 } else { current };
        e.ext.insert(KEY_MANEUVER.to_string(), next);
    }
}

fn maneuver_of(e: &Entity) -> f64 {
    e.ext.get(KEY_MANEUVER).copied().unwrap_or(1.0).clamp(0.0, 2.0)
}

fn tick_merge_cooldowns(world: &mut World) {
    for e in world.entities.values_mut() {
        if let Some(cd) = e.ext.get_mut(KEY_MERGE_COOLDOWN) {
            *cd = (*cd - TICK_SECONDS).max(0.0);
        }
    }
}

/// Immutable per-pass snapshot. Both sides fire simultaneously (§9.4.2 /
/// §9.4.3: effects take place at the same time), so all inputs are captured
/// before any damage is applied.
#[derive(Clone, Debug)]
struct PlaneSnap {
    id: EntityId,
    side: Side,
    name: String,
    speed_kts: f64,
    altitude_m: f64,
    maneuver: f64,
    hp: f64,
    hp_max: f64,
}

impl PlaneSnap {
    fn of(e: &Entity) -> Self {
        Self {
            id: e.id,
            side: e.side,
            name: e.name.clone(),
            speed_kts: e.speed_kts,
            altitude_m: e.altitude_m,
            maneuver: maneuver_of(e),
            hp: e.hp,
            hp_max: e.hp_max,
        }
    }
}

/// Gun Pk for `attacker` shooting `defender` in one pass.
///
/// Work order: base 0.12; speed difference >150 kt ×0.7; altitude difference
/// >1500 m ×0.8; violent defender ×1.2. The tabletop equivalent is the
/// position roll (§9.4.1) followed by the Aircraft Gun Attack Table
/// (§9.4.2); the scalar multipliers are an RTS abstraction (estimated).
fn gun_pk(attacker: &PlaneSnap, defender: &PlaneSnap) -> f64 {
    let mut pk = GUN_BASE_PK;
    if (attacker.speed_kts - defender.speed_kts).abs() > 150.0 {
        pk *= 0.7;
    }
    if (attacker.altitude_m - defender.altitude_m).abs() > 1500.0 {
        pk *= 0.8;
    }
    if defender.maneuver >= 2.0 {
        pk *= EVASIVE_INCOMING_GUN_MULT;
    }
    pk.clamp(0.0, 1.0)
}

/// Fuel remaining as a fraction of the class endurance. `None` means the
/// aircraft has no endurance data and therefore no fuel gate.
fn fuel_fraction(world: &World, e: &Entity) -> Option<f64> {
    let left = e.endurance_s?;
    let endurance = e.class_def(&world.db).map(|c| c.endurance_s).unwrap_or(0.0);
    if endurance <= 0.0 {
        return None;
    }
    Some((left / endurance).clamp(0.0, 1.0))
}

fn is_merged(e: &Entity) -> bool {
    e.ext.contains_key(KEY_MERGE_WITH)
}

fn merge_partner(e: &Entity) -> Option<EntityId> {
    e.ext.get(KEY_MERGE_WITH).map(|v| EntityId(*v as u32))
}

/// Advance all active merges: end checks, 15 s timer, pass adjudication.
fn process_merges(world: &mut World) {
    // Include dead aircraft: if the lower-id partner of a pair was killed by
    // something outside this module (e.g. an AAM), its stale link must still
    // be cleared. Dead entities linger in the world for 120 s (damage.rs).
    let ids: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.kind == PlatformKind::Aircraft)
        .map(|e| e.id)
        .collect();

    for id in ids {
        let Some(partner) = world.entity(id).and_then(merge_partner) else { continue };
        let passes = world
            .entity(id)
            .and_then(|e| e.ext.get(KEY_MERGE_PASSES).copied())
            .unwrap_or(0.0) as u32;

        if partner == id {
            end_merge(world, id, partner, passes, "stale_merge");
            continue;
        }
        // Validate the link before the id-order gate: the partner may have
        // been removed entirely (cleanup_dead), leaving only this side.
        let Some(other) = world.entity(partner) else {
            end_merge(world, id, partner, passes, "partner_lost");
            continue;
        };
        let own_side = world.entity(id).map(|e| e.side);
        if other.kind != PlatformKind::Aircraft
            || own_side != Some(other.side.other())
            || merge_partner(other) != Some(id)
        {
            end_merge(world, id, partner, passes, "stale_merge");
            continue;
        }
        // Each valid pair is adjudicated from the lower entity id only.
        if partner.0 <= id.0 {
            continue;
        }

        let dead = !other.alive
            || other.hp <= 0.0
            || world.entity(id).map(|e| !e.alive || e.hp <= 0.0).unwrap_or(true);
        if dead {
            end_merge(world, id, partner, passes, "aircraft_destroyed");
            continue;
        }
        if other.altitude_m<=60.0 || world.entity(id).is_some_and(|e|e.altitude_m<=60.0) {
            end_merge(world,id,partner,passes,"vlow_no_dogfight");continue;
        }

        let fuel_a = world.entity(id).and_then(|e| fuel_fraction(world, e));
        let fuel_b = fuel_fraction(world, other);
        let low_fuel = fuel_a.map(|f| f < DISENGAGE_FUEL_FRAC).unwrap_or(false)
            || fuel_b.map(|f| f < DISENGAGE_FUEL_FRAC).unwrap_or(false);
        if low_fuel {
            end_merge(world, id, partner, passes, "fuel_low");
            continue;
        }
        if passes > PASS_LIMIT {
            end_merge(world, id, partner, passes, "pass_limit");
            continue;
        }

        // Timer is authoritative on the lower id; mirror it to the partner.
        let timer = world
            .entity(id)
            .and_then(|e| e.ext.get(KEY_MERGE_TIMER).copied())
            .unwrap_or(0.0)
            + TICK_SECONDS;

        if timer < PASS_INTERVAL_S {
            set_merge_timer(world, id, partner, timer, passes);
            continue;
        }

        let pass_no = passes + 1;
        let next_timer = timer - PASS_INTERVAL_S;
        set_merge_timer(world, id, partner, next_timer, pass_no);

        resolve_pass(world, id, partner, pass_no);

        // End-of-pass checks, work order order: death first (sink already
        // called), then fuel, then the pass limit.
        let after_dead = world
            .entities
            .get(&id)
            .map(|e| !e.alive || e.hp <= 0.0)
            .unwrap_or(true)
            || world
                .entities
                .get(&partner)
                .map(|e| !e.alive || e.hp <= 0.0)
                .unwrap_or(true);
        if after_dead {
            end_merge(world, id, partner, pass_no, "aircraft_destroyed");
            continue;
        }
        let after_fuel_low = world
            .entity(id)
            .and_then(|e| fuel_fraction(world, e))
            .map(|f| f < DISENGAGE_FUEL_FRAC)
            .unwrap_or(false)
            || world
                .entity(partner)
                .and_then(|e| fuel_fraction(world, e))
                .map(|f| f < DISENGAGE_FUEL_FRAC)
                .unwrap_or(false);
        if after_fuel_low {
            end_merge(world, id, partner, pass_no, "fuel_low");
            continue;
        }
        if pass_no > PASS_LIMIT {
            end_merge(world, id, partner, pass_no, "pass_limit");
        }
    }
}

fn set_merge_timer(world: &mut World, a: EntityId, b: EntityId, timer: f64, passes: u32) {
    for id in [a, b] {
        if let Some(e) = world.entity_mut(id) {
            e.ext.insert(KEY_MERGE_TIMER.to_string(), timer);
            e.ext.insert(KEY_MERGE_PASSES.to_string(), passes as f64);
        }
    }
}

/// One 15 s pass: both sides roll one gun attack, simultaneously.
fn resolve_pass(world: &mut World, a_id: EntityId, b_id: EntityId, pass_no: u32) {
    let (Some(a), Some(b)) = (
        world.entity(a_id).map(PlaneSnap::of),
        world.entity(b_id).map(PlaneSnap::of),
    ) else {
        return;
    };

    let pk_a = gun_pk(&a, &b);
    let pk_b = gun_pk(&b, &a);
    let hit_a = world.scope_rng(a.id, "dogfight_gun").chance(pk_a);
    let hit_b = world.scope_rng(b.id, "dogfight_gun").chance(pk_b);

    // Simultaneous damage: resolve both shots before applying either (§9.4.2).
    let dmg_to_b = if hit_a { (b.hp_max * GUN_HIT_FRACTION).min(b.hp.max(0.0)) } else { 0.0 };
    let dmg_to_a = if hit_b { (a.hp_max * GUN_HIT_FRACTION).min(a.hp.max(0.0)) } else { 0.0 };

    if hit_a {
        if let Some(t) = world.entity_mut(b_id) {
            t.hp -= dmg_to_b;
        }
    }
    if hit_b {
        if let Some(t) = world.entity_mut(a_id) {
            t.hp -= dmg_to_a;
        }
    }

    let b_hp_left = world.entity(b_id).map(|e| e.hp.max(0.0)).unwrap_or(0.0);
    let a_hp_left = world.entity(a_id).map(|e| e.hp.max(0.0)).unwrap_or(0.0);

    emit_pass(world, &a, &b, pass_no, pk_a, hit_a, b_hp_left);
    emit_pass(world, &b, &a, pass_no, pk_b, hit_b, a_hp_left);

    if hit_a {
        emit_hit(world, &a, &b, pass_no, dmg_to_b, b_hp_left);
    }
    if hit_b {
        emit_hit(world, &b, &a, pass_no, dmg_to_a, a_hp_left);
    }

    // Shootdowns go through the shared damage path (§14 / work order).
    for (target, hp_left) in [(b_id, b_hp_left), (a_id, a_hp_left)] {
        if hp_left <= 0.0 {
            damage::sink(world, target);
        }
    }
}

fn emit_pass(
    world: &mut World,
    shooter: &PlaneSnap,
    target: &PlaneSnap,
    pass_no: u32,
    pk: f64,
    hit: bool,
    target_hp_left: f64,
) {
    let contact = world.find_contact(shooter.side, target.id);
    let detail = format!(
        "pass {pass_no}: {} (maneuver {:.0}, {:.0} kt, {:.0} m) vs {} \
         (maneuver {:.0}, {:.0} kt, {:.0} m); gun pk {:.3}; hit={}; target hp {:.1}/{:.1}",
        shooter.name,
        shooter.maneuver,
        shooter.speed_kts,
        shooter.altitude_m,
        target.name,
        target.maneuver,
        target.speed_kts,
        target.altitude_m,
        pk,
        hit,
        target_hp_left,
        target.hp_max
    );
    world.emit(
        EventKind::RulesEvent {
            domain: "aircombat".to_string(),
            rule: "dogfight_pass".to_string(),
            side: Some(shooter.side),
            unit: Some(shooter.id),
            contact,
            detail,
        },
        Visibility::Side(shooter.side),
    );
}

fn emit_hit(
    world: &mut World,
    shooter: &PlaneSnap,
    target: &PlaneSnap,
    pass_no: u32,
    damage: f64,
    hp_left: f64,
) {
    let contact = world.find_contact(shooter.side, target.id);
    world.emit(
        EventKind::RulesEvent {
            domain: "aircombat".to_string(),
            rule: "dogfight_hit".to_string(),
            side: Some(shooter.side),
            unit: Some(target.id),
            contact,
            detail: format!(
                "pass {pass_no}: {} hits {} for {:.1} damage; target hp {:.1}/{:.1}",
                shooter.name, target.name, damage, hp_left, target.hp_max
            ),
        },
        Visibility::Side(shooter.side),
    );
    // The victim's side also knows it is taking hits; mirror the generic
    // damage event used by weapons/damage so existing consumers see it.
    world.emit(
        EventKind::Impact {
            side: target.side,
            unit: target.id,
            unit_name: target.name.clone(),
            weapon: "aircraft cannon".to_string(),
            damage,
            hp_left,
        },
        Visibility::Side(target.side),
    );
}

/// Clear the merge pair, start the re-merge cooldown, and emit one
/// `dogfight_end` per side (own unit id + enemy contact id).
fn end_merge(world: &mut World, a_id: EntityId, b_id: EntityId, passes: u32, reason: &str) {
    for (id, other) in [(a_id, b_id), (b_id, a_id)] {
        let Some((side, name)) = world.entity(id).map(|e| (e.side, e.name.clone())) else {
            continue;
        };
        let contact = world.find_contact(side, other);
        if let Some(e) = world.entity_mut(id) {
            e.ext.remove(KEY_MERGE_WITH);
            e.ext.remove(KEY_MERGE_PASSES);
            e.ext.remove(KEY_MERGE_TIMER);
            e.ext.insert(KEY_MERGE_COOLDOWN.to_string(), MERGE_COOLDOWN_S);
        }
        world.emit(
            EventKind::RulesEvent {
                domain: "aircombat".to_string(),
                rule: "dogfight_end".to_string(),
                side: Some(side),
                unit: Some(id),
                contact,
                detail: format!("{name} disengages after {passes} passes ({reason})"),
            },
            Visibility::Side(side),
        );
    }
}

/// Pair opposing aircraft that are within merge range. Deterministic:
/// candidates in EntityId order; nearest unmerged opponent, ties by id.
fn start_merges(world: &mut World) {
    let candidates: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| {
            e.alive
                && e.kind == PlatformKind::Aircraft
                && e.fire_on
                  && e.altitude_m > 60.0
                && !is_merged(e)
                && e.ext.get(KEY_MERGE_COOLDOWN).copied().unwrap_or(0.0) <= 0.0
        })
        .map(|e| e.id)
        .collect();

    for id in candidates {
        let eligible = world
            .entity(id)
            .map(|e| {
                e.alive
                    && e.fire_on
                    && e.kind == PlatformKind::Aircraft
                    && !is_merged(e)
                    && e.ext.get(KEY_MERGE_COOLDOWN).copied().unwrap_or(0.0) <= 0.0
            })
            .unwrap_or(false);
        if !eligible {
            continue;
        }
        let Some((side, pos)) = world.entity(id).map(|e| (e.side, e.pos)) else { continue };

        let mut best: Option<(f64, EntityId)> = None;
        for other in world.entities.values() {
            if other.id == id
                || !other.alive
                || other.kind != PlatformKind::Aircraft
                || other.side == side
                || !other.fire_on
                  || other.altitude_m <= 60.0
                || is_merged(other)
                || other.ext.get(KEY_MERGE_COOLDOWN).copied().unwrap_or(0.0) > 0.0
            {
                continue;
            }
            let dist = pos.range_to(other.pos);
            if dist >= MERGE_RADIUS_NM {
                continue;
            }
            let better = match best {
                None => true,
                Some((best_dist, best_id)) => {
                    dist < best_dist || (dist == best_dist && other.id < best_id)
                }
            };
            if better {
                best = Some((dist, other.id));
            }
        }

        let Some((dist, partner)) = best else { continue };
        for (unit, other) in [(id, partner), (partner, id)] {
            if let Some(e) = world.entity_mut(unit) {
                e.ext.insert(KEY_MERGE_WITH.to_string(), other.0 as f64);
                e.ext.insert(KEY_MERGE_PASSES.to_string(), 0.0);
                e.ext.insert(KEY_MERGE_TIMER.to_string(), 0.0);
            }
        }
        emit_merge(world, id, partner, dist);
        emit_merge(world, partner, id, dist);
    }
}

fn emit_merge(world: &mut World, id: EntityId, partner: EntityId, dist_nm: f64) {
    let Some((side, name)) = world.entity(id).map(|e| (e.side, e.name.clone())) else {
        return;
    };
    let contact = world.find_contact(side, partner);
    world.emit(
        EventKind::RulesEvent {
            domain: "aircombat".to_string(),
            rule: "dogfight_merge".to_string(),
            side: Some(side),
            unit: Some(id),
            contact,
            detail: format!("{name} merges with #{} at {:.1} nmi", partner.0, dist_nm),
        },
        Visibility::Side(side),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, PlatformClass};
    use crate::geom::Vec2;
    use crate::scenario::{load_scenario, Scenario, UnitSpawn};
    use crate::types::{DayNight, RcsClass};
    use crate::world::Weather;

    /// Self-contained test database: one fighter platform, no weapons or
    /// sensors. Keeps these tests independent of other rules workstreams'
    /// `db()` layers (the merge/pass model only needs hp/endurance/kinematics).
    fn fighter_db() -> Database {
        let mut db = Database::default();
        db.platforms.insert(
            "fighter".into(),
            PlatformClass {
                rule_data:None,
                id: "fighter".into(),
                name: "Test Fighter".into(),
                kind: PlatformKind::Aircraft,
                hp: 30.0,
                max_speed_kts: 1100.0,
                cruise_speed_kts: 450.0,
                turn_rate_deg_s: 12.0,
                accel_kts_per_s: 0.4,
                rcs: RcsClass::Small,
                noise_db: 50.0,
                visual_factor: 1.0,
                vp_value: 12.0,
                sensors: vec![],
                weapons: vec![],
                chaff: 0,
                decoys: 0,
                endurance_s: 5400.0,
                max_depth_m: 0.0,
                dipping_sonar: false,
                is_base: false,
                deck: None,
            },
        );
        db
    }

    fn duel(seed: u64, separation_nm: f64, altitude_diff_m: f64) -> World {
        let scenario = Scenario {
            id: "aircombat-duel".into(),
            name: "Aircombat Duel".into(),
            description: String::new(),
            context: Default::default(),
            duration_s: 3600,
            weather: Weather { visibility_nm: 30.0, jam: [0.0, 0.0], overcast: 0.0, rain: 0.0 },
            day_night: DayNight::Day,
            seed,
            victory: Default::default(),
            units: vec![
                UnitSpawn {
                    side: Side::Blue,
                    class: "fighter".into(),
                    name: Some("BLUE FIGHTER".into()),
                    pos: Vec2::new(0.0, 0.0),
                    course_deg: 90.0,
                    speed_kts: 500.0,
                    depth_m: None,
                    altitude_m: Some(6000.0),
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: true,
                },
                UnitSpawn {
                    side: Side::Red,
                    class: "fighter".into(),
                    name: Some("RED FIGHTER".into()),
                    pos: Vec2::new(separation_nm, 0.0),
                    course_deg: 270.0,
                    speed_kts: 500.0,
                    depth_m: None,
                    altitude_m: Some(6000.0 + altitude_diff_m),
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: true,
                },
            ],
        };
        load_scenario(scenario, fighter_db()).expect("test scenario must load")
    }

    fn advance(world: &mut World, ticks: usize) {
        for _ in 0..ticks {
            world.tick += 1;
            step(world);
        }
    }

    fn ids(world: &World) -> (EntityId, EntityId) {
        let mut ids: Vec<EntityId> = world.entities.values().map(|e| e.id).collect();
        ids.sort();
        (ids[0], ids[1])
    }

    fn rule_events(world: &World, rule: &str) -> usize {
        world
            .events
            .iter()
            .filter(|e| {
                matches!(
                    &e.kind,
                    EventKind::RulesEvent { domain, rule: r, .. }
                        if domain == "aircombat" && r == rule
                )
            })
            .count()
    }

    fn end_details(world: &World) -> Vec<String> {
        world
            .events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::RulesEvent { domain, rule, detail, .. }
                    if domain == "aircombat" && rule == "dogfight_end" =>
                {
                    Some(detail.clone())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn opposing_aircraft_within_six_nm_merge() {
        let mut world = duel(11, 2.0, 0.0);
        advance(&mut world, 1);
        let (a, b) = ids(&world);
        for (id, other) in [(a, b), (b, a)] {
            let e = world.entity(id).unwrap();
            assert_eq!(
                e.ext.get(KEY_MERGE_WITH).copied(),
                Some(other.0 as f64),
                "merge_with must record the opponent"
            );
            assert_eq!(e.ext.get(KEY_MERGE_PASSES).copied(), Some(0.0));
            assert_eq!(e.ext.get(KEY_MANEUVER).copied(), Some(1.0));
        }
        assert!(rule_events(&world, "dogfight_merge") >= 2);

        // Outside the 6 nmi merge radius no merge is declared.
        let mut far = duel(12, 7.0, 0.0);
        advance(&mut far, 1);
        assert!(far.entities.values().all(|e| !is_merged(e)));
    }

    #[test]
    fn vlow_blocks_merge_and_ends_an_existing_dogfight() {
        let mut w=duel(12,3.0,0.0);
        for e in w.entities.values_mut(){e.altitude_m=50.0;}
        advance(&mut w,1);assert!(w.entities.values().all(|e|!is_merged(e)));
        for e in w.entities.values_mut(){e.altitude_m=3000.0;}
        advance(&mut w,1);assert!(w.entities.values().all(is_merged));
        w.entities.values_mut().next().unwrap().altitude_m=60.0;
        advance(&mut w,1);assert!(w.entities.values().all(|e|!is_merged(e)));
    }

    #[test]
    fn dogfight_produces_passes_and_observable_events() {
        let mut world = duel(7, 2.0, 0.0);
        advance(&mut world, 20);
        let (a, _) = ids(&world);
        let passes = world
            .entity(a)
            .unwrap()
            .ext
            .get(KEY_MERGE_PASSES)
            .copied()
            .unwrap_or(0.0);
        assert!(passes >= 1.0, "expected at least one 15 s pass, got {passes}");
        assert!(
            rule_events(&world, "dogfight_pass") >= 2,
            "each side should see one dogfight_pass event per pass"
        );
        // The event is visible to the owning side through the normal filter.
        let blue_sees_pass = world.events_for(Side::Blue, 0).iter().any(|e| {
            matches!(
                &e.kind,
                EventKind::RulesEvent { domain, rule, .. }
                    if domain == "aircombat" && rule == "dogfight_pass"
            )
        });
        assert!(blue_sees_pass);
    }

    #[test]
    fn maneuver_defaults_and_auto_escalates_when_damaged() {
        let mut world = duel(9, 20.0, 0.0);
        advance(&mut world, 1);
        for e in world.entities.values() {
            assert_eq!(e.ext.get(KEY_MANEUVER).copied(), Some(1.0));
            assert_eq!(aam_evasion_multiplier(e), 1.0);
        }
        for e in world.entities.values_mut() {
            e.hp = e.hp_max * 0.29;
        }
        advance(&mut world, 1);
        for e in world.entities.values() {
            assert_eq!(
                e.ext.get(KEY_MANEUVER).copied(),
                Some(2.0),
                "hp below 30% must select violent maneuvering"
            );
            assert_eq!(aam_evasion_multiplier(e), EVASIVE_AAM_SURVIVAL_MULT);
        }
    }

    #[test]
    fn gun_pk_applies_speed_altitude_and_evasive_modifiers() {
        let mut world = duel(3, 20.0, 0.0);
        advance(&mut world, 1);
        let (a_id, b_id) = ids(&world);
        let a = PlaneSnap::of(world.entity(a_id).unwrap());
        let mut b = PlaneSnap::of(world.entity(b_id).unwrap());

        assert!((gun_pk(&a, &b) - GUN_BASE_PK).abs() < 1e-12);

        b.speed_kts = a.speed_kts + 151.0;
        assert!((gun_pk(&a, &b) - 0.084).abs() < 1e-12, "speed >150 kt must be ×0.7");

        b.speed_kts = a.speed_kts;
        b.altitude_m = a.altitude_m + 1501.0;
        assert!((gun_pk(&a, &b) - 0.096).abs() < 1e-12, "altitude >1500 m must be ×0.8");

        b.altitude_m = a.altitude_m;
        b.maneuver = 2.0;
        assert!(
            (gun_pk(&a, &b) - GUN_BASE_PK * EVASIVE_INCOMING_GUN_MULT).abs() < 1e-12,
            "violent defender is easier to hit"
        );

        b.speed_kts = a.speed_kts + 200.0;
        b.altitude_m = a.altitude_m + 2000.0;
        let expected = GUN_BASE_PK * 0.7 * 0.8 * EVASIVE_INCOMING_GUN_MULT;
        assert!((gun_pk(&a, &b) - expected).abs() < 1e-12);
    }

    #[test]
    fn dogfight_ends_after_pass_limit() {
        let mut world = duel(21, 2.0, 0.0);
        // Keep both aircraft alive through seven passes so the pass cap,
        // not a shootdown, is what ends the merge.
        for e in world.entities.values_mut() {
            e.hp = e.hp_max * 10.0;
        }
        advance(&mut world, 1 + 15 * 7 + 2);
        for e in world.entities.values() {
            assert!(!is_merged(e), "merge state must be cleared on disengage");
        }
        let details = end_details(&world);
        assert!(!details.is_empty(), "dogfight_end must be emitted");
        assert!(
            details.iter().any(|d| d.contains("pass_limit") && d.contains("7 passes")),
            "expected pass-limit end, got {details:?}"
        );
    }

    #[test]
    fn low_fuel_breaks_the_merge() {
        let mut world = duel(5, 2.0, 0.0);
        advance(&mut world, 1);
        let endurance = world.db.platforms.get("fighter").unwrap().endurance_s;
        for e in world.entities.values_mut() {
            e.endurance_s = Some(endurance * 0.14);
        }
        advance(&mut world, 1);
        for e in world.entities.values() {
            assert!(!is_merged(e));
        }
        assert!(
            end_details(&world).iter().any(|d| d.contains("fuel_low")),
            "expected fuel_low end, got {:?}",
            end_details(&world)
        );
    }

    #[test]
    fn externally_killed_partner_clears_the_merge() {
        let mut world = duel(13, 2.0, 0.0);
        advance(&mut world, 1);
        let (a, b) = ids(&world);
        // An AAM (weapons.rs) may kill one side between our ticks. The
        // surviving side must still drop its stale merge link.
        crate::damage::sink(&mut world, a);
        advance(&mut world, 1);
        assert!(!is_merged(world.entity(a).unwrap()));
        assert!(!is_merged(world.entity(b).unwrap()));
        assert!(
            end_details(&world)
                .iter()
                .any(|d| d.contains("aircraft_destroyed")),
            "stale merge must end after an external kill, got {:?}",
            end_details(&world)
        );
    }

    #[test]
    fn gun_hit_damages_and_shootdown_reuses_sink() {
        // Deterministic seed search: with both aircraft at 5 hp (<30% -> both
        // violent, pk 0.144) a first-pass hit occurs within a few seeds. A
        // hit removes 34% of hp_max = 10.2, which is lethal at 5 hp.
        for seed in 0..64u64 {
            let mut world = duel(seed, 2.0, 0.0);
            for e in world.entities.values_mut() {
                e.hp = 5.0;
            }
            advance(&mut world, 17);
            if rule_events(&world, "dogfight_hit") == 0 {
                continue;
            }
            assert!(
                world.events.iter().any(|e| matches!(e.kind, EventKind::AirShotDown { .. })),
                "shootdown must go through damage::sink"
            );
            assert!(
                end_details(&world)
                    .iter()
                    .any(|d| d.contains("aircraft_destroyed")),
                "merge must end on a shootdown"
            );
            return;
        }
        panic!("no first-pass gun hit across 64 deterministic seeds");
    }
}

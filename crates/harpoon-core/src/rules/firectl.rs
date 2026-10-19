//! Fire control & data links — §6.3 of the Harpoon V rules.
//!
//! Implemented here:
//! - §6.3.3 passive cross-fix (multi-unit passive-sonar bearing convergence);
//! - §6.3.9 tactical data link (TDL) contact sharing, with the §6.3.10
//!   degraded non-TDL relay cadence.
//!
//! Engine note: `World` keeps **one contact book per side**
//! (`SideState.contacts`, docs/architecture/engine.md:34), not one per unit.
//! To model the rules' per-platform books, TDL shadows are kept in
//! `Entity.ext` (`firectl.shadow.*`): each TDL-capable platform holds its own
//! copy of a track, TDL units exchange copies, and a copy is re-injected into
//! the side book (with a fresh id on collision) when the fused book has lost
//! the track. This is an engine adaptation, documented in the result notes.

use crate::db::SensorKind;
use crate::events::{EventKind, Visibility};
use crate::geom::{Vec2, KTS_TO_NM_PER_S};
use crate::types::{ContactId, EntityId, PlatformKind, Side};
use crate::world::{Contact, Entity, World, CONTACT_STALE_S};
use std::collections::BTreeMap;

pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

// §6.3.3: a cross-fix adds one step of bearing quality; passive-sonar fire
// control solutions are capped at 0.85 in the sensor model
// (sensors.rs SonarPassive quality_cap).
const CROSSFIX_BONUS: f64 = 0.1;
const CROSSFIX_CAP: f64 = 0.85;

// §6.3.9: TDL relays preserve most of the source track's quality; the
// non-TDL relay (§6.3.10, delayed voice/teletype picture) loses half.
const TDL_INTERVAL_S: i64 = 60;
const NON_TDL_INTERVAL_S: i64 = 300;
const TDL_QUALITY_FACTOR: f64 = 0.9;
const NON_TDL_QUALITY_FACTOR: f64 = 0.5;

/// Estimated persistence of a unit-level TDL shadow: the side book keeps a
/// stale track for `CONTACT_STALE_S` (900 s); TDL shadows add a short hold
/// beyond that (300 s) so a track can be re-shared after local loss. The
/// rules do not specify a retention time — estimated adaptation.
const SHADOW_TTL_S: i64 = CONTACT_STALE_S as i64 + 300;

const SHADOW_PREFIX: &str = "firectl.shadow.";
const TDL_BUCKET_PREFIX: &str = "firectl.tdl.last_bucket.";

/// Per-tick fire-control/data-link behavior.
pub fn step(world: &mut World) {
    cross_fix(world);
    tdl_sync(world);
}

// ---------------------------------------------------------------------------
// §6.3.3 Passive cross-fix
// ---------------------------------------------------------------------------

fn crossfix_key(side: Side, cid: ContactId, first_tick: i64) -> String {
    format!(
        "firectl.crossfix.{}.{}.{}",
        side.index(),
        cid.0,
        first_tick
    )
}

fn is_passive_sensor(world: &World, sensor: &str) -> bool {
    if sensor.starts_with("sonar_p") {
        return true;
    }
    world
        .db
        .sensors
        .get(sensor)
        .map(|d| d.kind == SensorKind::SonarPassive)
        .unwrap_or(false)
}

/// Can `observer` currently hear `target` on a passive sonar? Range factors
/// mirror sensors.rs:118-136 (base × speed × noise × propagation); the
/// sensor pass itself remains the sole producer of contacts — this check is
/// only used to count distinct contributing units for a cross-fix.
fn passive_hears(world: &World, observer: &Entity, target: &Entity) -> bool {
    if observer.systems.sonar <= 0.05 {
        return false;
    }
    let noise = target
        .class_def(&world.db)
        .map(|c| c.noise_db / 50.0)
        .unwrap_or(1.0);
    let speed_factor = (target.speed_kts / 10.0).clamp(0.3, 2.2);
    let range = observer.pos.range_to(target.pos);
    for inst in &observer.sensors {
        let Some(def) = world.db.sensors.get(&inst.sensor) else { continue };
        if def.kind != SensorKind::SonarPassive || def.sub_range_nm <= 0.0 {
            continue;
        }
        let effective = match target.kind {
            PlatformKind::Torpedo => def.sub_range_nm.min(3.0),
            PlatformKind::Sub => {
                def.sub_range_nm
                    * speed_factor
                    * noise
                    * crate::rules::asw::propagation_modifier(observer, target)
            }
            PlatformKind::Ship => def.sub_range_nm * 0.35 * speed_factor * noise,
            _ => continue,
        };
        if range <= effective {
            return true;
        }
    }
    false
}

fn cross_fix(world: &mut World) {
    struct Candidate {
        cid: ContactId,
        about: EntityId,
        first_tick: i64,
        sensors: Vec<String>,
        offset: Vec2,
        truth: Vec2,
    }

    for side in [Side::Blue, Side::Red] {
        let mut candidates: Vec<Candidate> = Vec::new();
        for c in world.sides[side.index()].contacts.values() {
            if c.stale {
                continue;
            }
            let Some(target) = world.entity(c.about) else { continue };
            if !target.alive {
                continue;
            }
            // Work-order mapping of §6.3.3 to passive sonar: the contact must
            // at least carry a passive sonar sensor observation (sonar_p_*).
            if !c.sensors.iter().any(|s| is_passive_sensor(world, s)) {
                continue;
            }
            // One quality reward per contact (dedupe / "每接触一次去重").
            if target
                .ext
                .contains_key(&crossfix_key(side, c.id, c.first_tick))
            {
                continue;
            }
            candidates.push(Candidate {
                cid: c.id,
                about: c.about,
                first_tick: c.first_tick,
                sensors: c.sensors.clone(),
                offset: c.offset,
                truth: target.pos,
            });
        }

        for cand in candidates {
            let Some(target) = world.entity(cand.about) else { continue };
            // Distinct same-side units contributing passive bearings. The
            // engine's contact book stores sensor *types*, not observer ids;
            // a unit counts when it can hear the target now OR when it mounts
            // a passive sensor id recorded on the contact (book-provenance
            // fallback). Interval separation is the rules' angle criterion;
            // it is not modelled beyond "two distinct units".
            let mut hearers: Vec<(f64, EntityId)> = Vec::new();
            for o in world.entities.values() {
                if !o.alive || o.side != side || o.kind.is_projectile() {
                    continue;
                }
                let mounts_listed_sensor = o.systems.sonar > 0.05
                    && cand.sensors.iter().any(|listed| {
                        is_passive_sensor(world, listed)
                            && o.sensors.iter().any(|inst| &inst.sensor == listed)
                    });
                if !mounts_listed_sensor && !passive_hears(world, o, target) {
                    continue;
                }
                hearers.push((o.pos.range_to(cand.truth), o.id));
            }
            if hearers.len() < 2 {
                continue;
            }
            hearers.sort_by(|a, b| {
                a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)
            });
            let (_, observer) = hearers[0];

            let (q_old, q_new) = {
                let state = &mut world.sides[side.index()];
                let Some(contact) = state.contacts.get_mut(&cand.cid) else { continue };
                let q_old = contact.quality;
                // +0.1 one-shot reward, capped at 0.85 without ever
                // lowering an already-better track.
                let q_new = (q_old + CROSSFIX_BONUS)
                    .min(CROSSFIX_CAP)
                    .max(q_old);
                contact.quality = q_new;
                contact.bearing_only = false;
                // Converge on truth using the same persistent error model as
                // sensors.rs:462-464: estimate = truth + offset·(1-q)².
                contact.estimate = cand
                    .truth
                    .add(cand.offset.scaled((1.0 - q_new) * (1.0 - q_new)));
                contact.last_update_tick = world.tick;
                (q_old, q_new)
            };

            if let Some(t) = world.entity_mut(cand.about) {
                t.ext
                    .insert(crossfix_key(side, cand.cid, cand.first_tick), 1.0);
            }
            world.emit(
                EventKind::RulesEvent {
                    domain: "firectl".into(),
                    rule: "cross_fix".into(),
                    side: Some(side),
                    unit: Some(observer),
                    contact: Some(cand.cid),
                    detail: format!(
                        "passive bearing cross-fix ({}+ units); quality {:.2}->{:.2}, bearing_only cleared",
                        hearers.len(),
                        q_old,
                        q_new
                    ),
                },
                Visibility::Side(side),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// §6.3.9 Tactical data link
// ---------------------------------------------------------------------------

/// Unit-level shadow track (engine adaptation of a per-platform contact book).
#[derive(Clone, Debug)]
struct Shadow {
    cid: ContactId,
    about: EntityId,
    category: PlatformKind,
    quality: f64,
    /// Source estimate at `last_update_tick` (source contact's estimate_at).
    estimate: Vec2,
    course_deg: f64,
    speed_kts: f64,
    bearing_only: bool,
    last_update_tick: i64,
    /// Tick this copy was received (shadow TTL anchor).
    stored_tick: i64,
}

impl Shadow {
    fn estimate_at(&self, tick: i64) -> Vec2 {
        let dt = (tick - self.last_update_tick).max(0) as f64;
        self.estimate.add(
            Vec2::from_bearing(self.course_deg)
                .scaled(self.speed_kts * KTS_TO_NM_PER_S * dt),
        )
    }

    /// Copy a source contact at `tick`, applying the relay quality factor.
    /// `estimate` is the source's `estimate_at(tick)` (§6.3.9).
    fn from_contact(c: &Contact, quality: f64, tick: i64) -> Self {
        Self {
            cid: c.id,
            about: c.about,
            category: c.category,
            quality,
            estimate: c.estimate_at(tick),
            course_deg: c.est_course_deg,
            speed_kts: c.est_speed_kts,
            bearing_only: c.bearing_only,
            last_update_tick: tick,
            stored_tick: tick,
        }
    }
}

fn kind_code(kind: PlatformKind) -> f64 {
    match kind {
        PlatformKind::Ship => 0.0,
        PlatformKind::Sub => 1.0,
        PlatformKind::Aircraft => 2.0,
        PlatformKind::Missile => 3.0,
        PlatformKind::Torpedo => 4.0,
        PlatformKind::Base => 5.0,
    }
}

fn kind_from_code(code: f64) -> PlatformKind {
    match code as i32 {
        1 => PlatformKind::Sub,
        2 => PlatformKind::Aircraft,
        3 => PlatformKind::Missile,
        4 => PlatformKind::Torpedo,
        5 => PlatformKind::Base,
        _ => PlatformKind::Ship,
    }
}

/// Unit books are keyed by the tracked entity, not by the side-book contact
/// id: the same contact id can be reused for a different target after a loss,
/// while a platform's book still holds the old track for the old entity.
fn shadow_prefix(about: EntityId) -> String {
    format!("{SHADOW_PREFIX}{}.", about.0)
}

fn shadow_store(e: &mut Entity, sh: &Shadow) {
    let p = shadow_prefix(sh.about);
    e.ext.insert(format!("{p}cid"), sh.cid.0 as f64);
    e.ext.insert(format!("{p}cat"), kind_code(sh.category));
    e.ext.insert(format!("{p}q"), sh.quality);
    e.ext.insert(format!("{p}x"), sh.estimate.x);
    e.ext.insert(format!("{p}y"), sh.estimate.y);
    e.ext.insert(format!("{p}course"), sh.course_deg);
    e.ext.insert(format!("{p}speed"), sh.speed_kts);
    e.ext
        .insert(format!("{p}bearing"), if sh.bearing_only { 1.0 } else { 0.0 });
    e.ext.insert(format!("{p}tick"), sh.last_update_tick as f64);
    e.ext.insert(format!("{p}stored"), sh.stored_tick as f64);
}

fn shadow_get(e: &Entity, about: EntityId) -> Option<Shadow> {
    let p = shadow_prefix(about);
    let get = |field: &str| e.ext.get(&format!("{p}{field}")).copied();
    Some(Shadow {
        cid: ContactId(get("cid")? as u32),
        about,
        category: kind_from_code(get("cat")?),
        quality: get("q")?,
        estimate: Vec2::new(get("x")?, get("y")?),
        course_deg: get("course")?,
        speed_kts: get("speed")?,
        bearing_only: get("bearing")? > 0.5,
        last_update_tick: get("tick")? as i64,
        stored_tick: get("stored")? as i64,
    })
}

fn shadow_list(e: &Entity) -> Vec<Shadow> {
    let mut out = Vec::new();
    for key in e.ext.keys() {
        let Some(rest) = key.strip_prefix(SHADOW_PREFIX) else { continue };
        let Some((id, field)) = rest.split_once('.') else { continue };
        if field != "cid" {
            continue;
        }
        let Ok(id) = id.parse::<u32>() else { continue };
        if let Some(sh) = shadow_get(e, EntityId(id)) {
            out.push(sh);
        }
    }
    out
}

fn shadow_remove(e: &mut Entity, about: EntityId) {
    let p = shadow_prefix(about);
    let keys: Vec<String> = e
        .ext
        .keys()
        .filter(|k| k.starts_with(&p))
        .cloned()
        .collect();
    for k in keys {
        e.ext.remove(&k);
    }
}

fn shadow_lacks_about(e: &Entity, about: EntityId) -> bool {
    !shadow_list(e).iter().any(|s| s.about == about)
}

fn is_tdl_unit(world: &World, id: EntityId) -> bool {
    world
        .entity(id)
        .and_then(|e| e.class_def(&world.db))
        // §6.3.9 TDL equipment: the platform class carries an air-search
        // radar fit (`radar_air_*`) — the engine's TDL marker.
        .map(|c| c.sensors.iter().any(|s| s.starts_with("radar_air_")))
        .unwrap_or(false)
}

fn prune_shadows(world: &mut World, own: &[EntityId], tick: i64) {
    for uid in own {
        let Some(e) = world.entity(*uid) else { continue };
        let expired: Vec<EntityId> = shadow_list(e)
            .into_iter()
            .filter(|sh| {
                let dead = world
                    .entity(sh.about)
                    .map(|t| !t.alive)
                    .unwrap_or(true);
                dead || tick - sh.stored_tick > SHADOW_TTL_S
            })
            .map(|sh| sh.about)
            .collect();
        if let Some(e) = world.entity_mut(*uid) {
            for about in expired {
                shadow_remove(e, about);
            }
        }
    }
}

/// Re-share a lost track from a TDL unit's shadow book into the side's fused
/// book. If the source contact id is already occupied by a different target,
/// allocate `next_contact` (§6.3.9 id-conflict rule).
fn inject_missing(world: &mut World, side: Side, tdl: &[EntityId]) -> usize {
    let mut best: BTreeMap<EntityId, Shadow> = BTreeMap::new();
    for uid in tdl {
        let Some(e) = world.entity(*uid) else { continue };
        for sh in shadow_list(e) {
            let replace = best
                .get(&sh.about)
                .map(|cur| sh.quality > cur.quality)
                .unwrap_or(true);
            if replace {
                best.insert(sh.about, sh);
            }
        }
    }

    let mut injected = 0usize;
    for (about, sh) in best {
        let missing = !world.sides[side.index()]
            .contacts
            .values()
            .any(|c| c.about == about);
        if !missing {
            continue;
        }
        let alive = world.entity(about).map(|e| e.alive).unwrap_or(false);
        if !alive || world.tick - sh.stored_tick > SHADOW_TTL_S {
            continue;
        }
        let estimate = sh.estimate_at(world.tick);
        let state = &mut world.sides[side.index()];
        let cid = if state.contacts.contains_key(&sh.cid) {
            state.next_contact += 1;
            ContactId(state.next_contact)
        } else {
            state.next_contact = state.next_contact.max(sh.cid.0);
            sh.cid
        };
        state.contacts.insert(
            cid,
            Contact {
                id: cid,
                about,
                category: sh.category,
                class_name: None,
                estimate,
                est_course_deg: sh.course_deg,
                est_speed_kts: sh.speed_kts,
                bearing_only: sh.bearing_only,
                quality: sh.quality,
                last_update_tick: world.tick,
                first_tick: world.tick,
                stale: false,
                sensors: vec!["tdl_shared".into()],
                offset: Vec2::zero(),
                // Not a fresh sensor identification: suppress a duplicate
                // ContactIdentified announcement.
                announced: true,
            },
        );
        injected += 1;

        // One relayed copy of an entity is enough; consume every unit book's
        // copy so other TDL receivers cannot inject the same ghost.
        let owners: Vec<EntityId> = world
            .entities
            .values()
            .filter(|e| e.alive && e.side == side && !e.kind.is_projectile())
            .map(|e| e.id)
            .collect();
        for uid in owners {
            if let Some(e) = world.entity_mut(uid) {
                shadow_remove(e, about);
            }
        }
    }
    injected
}

fn tdl_sync(world: &mut World) {
    let tick = world.tick;
    if tick <= 0 || tick % TDL_INTERVAL_S != 0 {
        return;
    }
    let bucket = tick / TDL_INTERVAL_S;

    for side in [Side::Blue, Side::Red] {
        let own: Vec<EntityId> = world
            .entities
            .values()
            .filter(|e| e.alive && e.side == side && !e.kind.is_projectile())
            .map(|e| e.id)
            .collect();
        if own.is_empty() {
            continue;
        }

        // One batch per (side, tick/60) — "每次批量同步一次，按 (tick/60) 去重".
        let bucket_key = format!("{TDL_BUCKET_PREFIX}{}", side.index());
        let already = own.iter().any(|uid| {
            world
                .entity(*uid)
                .and_then(|e| e.ext.get(&bucket_key))
                .map(|v| *v == bucket as f64)
                .unwrap_or(false)
        });
        if already {
            continue;
        }

        let tdl: Vec<EntityId> = own
            .iter()
            .copied()
            .filter(|id| is_tdl_unit(world, *id))
            .collect();
        let non_tdl: Vec<EntityId> = own
            .iter()
            .copied()
            .filter(|id| !tdl.contains(id))
            .collect();

        // Live, own-sensor tracks only. Relayed copies (`tdl_shared`) are not
        // re-relayed, which bounds ghost circulation in the shadow books.
        let snapshot: Vec<Contact> = world.sides[side.index()]
            .contacts
            .values()
            .filter(|c| !c.stale && c.quality > 0.0)
            .filter(|c| world.entity(c.about).map(|e| e.alive).unwrap_or(false))
            .filter(|c| !(c.sensors.len() == 1 && c.sensors[0] == "tdl_shared"))
            .cloned()
            .collect();

        // 1) Every TDL unit's own book acquires the fused track at ×0.9.
        let mut relayed = 0usize;
        for uid in &tdl {
            for c in &snapshot {
                let lacks = world
                    .entity(*uid)
                    .map(|e| shadow_lacks_about(e, c.about))
                    .unwrap_or(false);
                if lacks {
                    let sh = Shadow::from_contact(c, c.quality * TDL_QUALITY_FACTOR, tick);
                    if let Some(e) = world.entity_mut(*uid) {
                        shadow_store(e, &sh);
                    }
                    relayed += 1;
                }
            }
        }

        // 2) Merge the TDL units' books with each other: a unit that lacks
        //    another unit's track receives a ×0.9 copy.
        let mut union: BTreeMap<EntityId, Shadow> = BTreeMap::new();
        for uid in &tdl {
            let Some(e) = world.entity(*uid) else { continue };
            for sh in shadow_list(e) {
                let replace = union
                    .get(&sh.about)
                    .map(|cur| sh.quality > cur.quality)
                    .unwrap_or(true);
                if replace {
                    union.insert(sh.about, sh);
                }
            }
        }
        for uid in &tdl {
            let missing: Vec<Shadow> = {
                let Some(e) = world.entity(*uid) else { continue };
                union
                    .values()
                    .filter(|sh| shadow_lacks_about(e, sh.about))
                    .cloned()
                    .collect()
            };
            for sh in missing {
                let mut copy = sh.clone();
                copy.quality = sh.quality * TDL_QUALITY_FACTOR;
                copy.estimate = sh.estimate_at(tick);
                copy.last_update_tick = tick;
                copy.stored_tick = tick;
                if let Some(e) = world.entity_mut(*uid) {
                    shadow_store(e, &copy);
                }
                relayed += 1;
            }
        }

        // 3) Non-TDL units get the shared picture every 300 s at ×0.5
        //    (§6.3.10 degraded relay cadence).
        let mut delayed = 0usize;
        if tick % NON_TDL_INTERVAL_S == 0 && !non_tdl.is_empty() {
            let mut sources = union.clone();
            for c in &snapshot {
                sources
                    .entry(c.about)
                    .or_insert_with(|| Shadow::from_contact(c, c.quality, tick));
            }
            for uid in &non_tdl {
                for sh in sources.values() {
                    let lacks = world
                        .entity(*uid)
                        .map(|e| shadow_lacks_about(e, sh.about))
                        .unwrap_or(false);
                    if !lacks {
                        continue;
                    }
                    let mut copy = sh.clone();
                    copy.quality = sh.quality * NON_TDL_QUALITY_FACTOR;
                    copy.estimate = sh.estimate_at(tick);
                    copy.last_update_tick = tick;
                    copy.stored_tick = tick;
                    if let Some(e) = world.entity_mut(*uid) {
                        shadow_store(e, &copy);
                    }
                    delayed += 1;
                }
            }
        }

        // 4) Expire old shadows, then let TDL units re-share tracks the fused
        //    book has lost (id conflict -> next_contact).
        prune_shadows(world, &own, tick);
        let injected = inject_missing(world, side, &tdl);

        let synced = !tdl.is_empty()
            || (tick % NON_TDL_INTERVAL_S == 0 && !non_tdl.is_empty());
        if synced {
            if let Some(anchor) = own.first() {
                if let Some(e) = world.entity_mut(*anchor) {
                    e.ext.insert(bucket_key, bucket as f64);
                }
            }
            world.emit(
                EventKind::RulesEvent {
                    domain: "firectl".into(),
                    rule: "tdl_share".into(),
                    side: Some(side),
                    unit: tdl.first().copied(),
                    contact: None,
                    detail: format!(
                        "TDL net {} unit(s): {} track(s) relayed (q×0.9), {} delayed to non-TDL units (q×0.5), {} track(s) re-shared",
                        tdl.len(),
                        relayed,
                        delayed,
                        injected
                    ),
                },
                Visibility::Side(side),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::scenario::{load_scenario, Scenario, UnitSpawn};
    use crate::types::DayNight;
    use crate::world::Weather;

    fn scenario(units: Vec<UnitSpawn>) -> Scenario {
        Scenario {
            id: "firectl-test".into(),
            name: "Fire control test".into(),
            description: String::new(),
            context: Default::default(),
            duration_s: 40_000,
            weather: Weather {
                visibility_nm: 12.0,
                jam: [0.0, 0.0],
                overcast: 0.0,
                rain: 0.0,
            },
            day_night: DayNight::Day,
            seed: 11,
            victory: Default::default(),
            units,
        }
    }

    fn world(units: Vec<UnitSpawn>) -> World {
        load_scenario(scenario(units), Database::built_in()).unwrap()
    }

    fn unit(side: Side, class: &str, name: &str, x: f64, y: f64, speed: f64) -> UnitSpawn {
        UnitSpawn {
            side,
            class: class.into(),
            name: Some(name.into()),
            pos: Vec2::new(x, y),
            course_deg: 0.0,
            speed_kts: speed,
            depth_m: None,
            altitude_m: None,
            waypoints: None,
            follow: None,
            sonar_active: false,
            fire_on: false,
        }
    }

    fn sub(side: Side, name: &str, x: f64, y: f64, depth_m: f64, speed: f64) -> UnitSpawn {
        UnitSpawn {
            depth_m: Some(depth_m),
            ..unit(side, "ssk", name, x, y, speed)
        }
    }

    fn id_of(w: &World, name: &str) -> EntityId {
        w.entities
            .values()
            .find(|e| e.name == name)
            .map(|e| e.id)
            .unwrap_or_else(|| panic!("no entity named {name}"))
    }

    #[allow(clippy::too_many_arguments)]
    fn seed_contact(
        w: &mut World,
        side: Side,
        about: EntityId,
        category: PlatformKind,
        pos: Vec2,
        quality: f64,
        sensors: Vec<&str>,
        offset: Vec2,
        bearing_only: bool,
        stale: bool,
        last_update_tick: i64,
    ) -> ContactId {
        let state = &mut w.sides[side.index()];
        state.next_contact += 1;
        let cid = ContactId(state.next_contact);
        state.contacts.insert(
            cid,
            Contact {
                id: cid,
                about,
                category,
                class_name: None,
                estimate: pos,
                est_course_deg: 0.0,
                est_speed_kts: 0.0,
                bearing_only,
                quality,
                last_update_tick,
                first_tick: 0,
                stale,
                sensors: sensors.into_iter().map(str::to_string).collect(),
                offset,
                announced: true,
            },
        );
        cid
    }

    fn rules_event_count(w: &World, rule: &str) -> usize {
        w.events
            .iter()
            .filter(|e| {
                matches!(
                    &e.kind,
                    EventKind::RulesEvent { domain, rule: r, .. }
                        if domain == "firectl" && r == rule
                )
            })
            .count()
    }

    #[test]
    fn passive_cross_fix_improves_quality_once() {
        // Two hull passive sonars, each 3 nm from the sub. The sub runs at
        // 10 kt, so the passive range factor is 1.0 (sensors.rs:122).
        let mut w = world(vec![
            unit(Side::Blue, "ddg_aegis", "BLUE A", 2.0, 0.0, 0.0),
            unit(Side::Blue, "ddg_aegis", "BLUE B", 8.0, 0.0, 0.0),
            sub(Side::Red, "RED SUB", 5.0, 0.0, 60.0, 10.0),
        ]);
        let red = id_of(&w, "RED SUB");
        let cid = seed_contact(
            &mut w,
            Side::Blue,
            red,
            PlatformKind::Sub,
            Vec2::new(7.0, 0.0),
            0.30,
            vec!["sonar_p_hull"],
            Vec2::new(2.0, 0.0),
            true,
            false,
            0,
        );
        w.tick = 1;
        step(&mut w);

        let c = w.contact(Side::Blue, cid).unwrap();
        assert!((c.quality - 0.40).abs() < 1e-9, "quality {}", c.quality);
        assert!(!c.bearing_only);
        assert!(
            c.estimate.range_to(Vec2::new(5.0, 0.0)) < 1.0,
            "estimate did not converge: {:?}",
            c.estimate
        );
        assert_eq!(rules_event_count(&w, "cross_fix"), 1);

        // One-shot reward: a second step must not add another +0.1 nor emit.
        step(&mut w);
        let c = w.contact(Side::Blue, cid).unwrap();
        assert!((c.quality - 0.40).abs() < 1e-9);
        assert_eq!(rules_event_count(&w, "cross_fix"), 1);
    }

    #[test]
    fn tdl_relays_at_ninety_percent_and_non_tdl_at_half() {
        let mut w = world(vec![
            unit(Side::Blue, "ddg_aegis", "BLUE DDG", 0.0, 0.0, 0.0),
            unit(Side::Blue, "mgb", "BLUE MGB", 0.0, 10.0, 0.0),
            unit(Side::Red, "mgb", "RED MGB", 30.0, 0.0, 0.0),
        ]);
        let ddg = id_of(&w, "BLUE DDG");
        let blue_mgb = id_of(&w, "BLUE MGB");
        let red = id_of(&w, "RED MGB");
        let _cid = seed_contact(
            &mut w,
            Side::Blue,
            red,
            PlatformKind::Ship,
            Vec2::new(30.0, 0.0),
            0.60,
            vec!["radar_air_long"],
            Vec2::zero(),
            false,
            false,
            0,
        );

        w.tick = 60;
        step(&mut w);
        let sh = shadow_get(w.entity(ddg).unwrap(), red).expect("TDL shadow");
        assert!((sh.quality - 0.54).abs() < 1e-9, "shadow q {}", sh.quality);
        assert_eq!(rules_event_count(&w, "tdl_share"), 1);

        // Same (tick/60) bucket: no double relay, no duplicate event.
        step(&mut w);
        let sh_again = shadow_get(w.entity(ddg).unwrap(), red).expect("TDL shadow");
        assert!((sh_again.quality - 0.54).abs() < 1e-9);
        assert_eq!(rules_event_count(&w, "tdl_share"), 1);

        // 300 s: the non-TDL missile boat gets the relayed track at ×0.5.
        w.tick = 300;
        step(&mut w);
        let delayed = shadow_get(w.entity(blue_mgb).unwrap(), red)
            .expect("non-TDL shadow");
        assert!(
            (delayed.quality - 0.27).abs() < 1e-9,
            "non-TDL q {}",
            delayed.quality
        );
    }

    #[test]
    fn tdl_reinjects_lost_track_with_new_id_on_conflict() {
        let mut w = world(vec![
            unit(Side::Blue, "ddg_aegis", "BLUE DDG", 0.0, 0.0, 0.0),
            unit(Side::Red, "mgb", "RED MGB", 30.0, 0.0, 0.0),
            unit(Side::Red, "pb", "RED PB", 40.0, 0.0, 0.0),
        ]);
        let red_mgb = id_of(&w, "RED MGB");
        let red_pb = id_of(&w, "RED PB");
        let cid = seed_contact(
            &mut w,
            Side::Blue,
            red_mgb,
            PlatformKind::Ship,
            Vec2::new(30.0, 0.0),
            0.60,
            vec!["radar_air_long"],
            Vec2::zero(),
            false,
            false,
            0,
        );

        w.tick = 60;
        step(&mut w);
        assert!(shadow_get(w.entity(id_of(&w, "BLUE DDG")).unwrap(), red_mgb).is_some());

        // Simulate the fused book losing the track, and reuse the id for a
        // different target: re-sharing must allocate a new contact id.
        {
            let state = &mut w.sides[Side::Blue.index()];
            state.contacts.remove(&cid);
            state.contacts.insert(
                cid,
                Contact {
                    id: cid,
                    about: red_pb,
                    category: PlatformKind::Ship,
                    class_name: None,
                    estimate: Vec2::new(40.0, 0.0),
                    est_course_deg: 0.0,
                    est_speed_kts: 0.0,
                    bearing_only: false,
                    quality: 0.70,
                    last_update_tick: 60,
                    first_tick: 60,
                    stale: false,
                    sensors: vec!["radar_air_long".into()],
                    offset: Vec2::zero(),
                    announced: true,
                },
            );
        }

        w.tick = 120;
        step(&mut w);

        let injected = w.sides[Side::Blue.index()]
            .contacts
            .values()
            .find(|c| c.about == red_mgb)
            .expect("lost track re-shared by TDL")
            .clone();
        assert_ne!(injected.id, cid, "id conflict must use next_contact");
        assert_eq!(injected.sensors, vec!["tdl_shared".to_string()]);
        assert!((injected.quality - 0.54).abs() < 1e-9);
        assert!(injected.estimate.range_to(Vec2::new(30.0, 0.0)) < 1e-9);
    }
}

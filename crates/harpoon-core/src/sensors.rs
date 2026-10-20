//! Detection & knowledge: the electronic referee's double-blind layer.
//!
//! Every side maintains its own contact book. Positions carry a persistent
//! per-contact error vector that shrinks as tracking quality rises; ESM gives
//! bearing-only fixes. Enemies never appear in a side's picture unless some
//! sensor sees them.

use crate::db::{Database, SensorDef, SensorKind};
use crate::events::{EventKind, Visibility};
use crate::geom::{horizon_nm, Vec2};
use crate::types::{ContactId, DayNight, EntityId, PlatformKind, Side};
use crate::world::{Contact, Entity, World, CONTACT_STALE_S, SENSOR_INTERVAL_TICKS};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(crate) struct DetObs {
    pub sensor: String,
    pub observer_pos: Vec2,
    pub true_bearing_deg: f64,
    pub bearing_only: bool,
    pub err_base_nm: f64,
    pub quality_cap: f64,
}

fn range_for_target(def: &SensorDef, kind: PlatformKind) -> f64 {
    match kind {
        PlatformKind::Aircraft => def.air_range_nm,
        PlatformKind::Ship => def.surface_range_nm,
        PlatformKind::Sub => def.sub_range_nm,
        PlatformKind::Missile => def.missile_range_nm,
        PlatformKind::Torpedo => def.sub_range_nm * 0.5,
        PlatformKind::Base => def.surface_range_nm,
    }
}

/// Radar-generation / weather-clutter hook owned by rules::sensing (§5.2/§5.5).
fn sensing_radar_modifier(def: &SensorDef, t: &Entity) -> f64 {
    crate::rules::sensing::radar_type_modifier(def, t)
}

/// Visual/EO-IR conditions hook (cloud, rain, dusk) owned by rules::sensing.
fn sensing_visual_modifier(o: &Entity, t: &Entity) -> f64 {
    crate::rules::sensing::visual_modifier(o, t)
}

/// Sonar propagation hook owned by rules::asw (layer physics).
fn propagation_modifier(o: &Entity, t: &Entity) -> f64 {
    crate::rules::asw::propagation_modifier(o, t)
}

fn is_radar_sensor(sensor_id: &str) -> bool {
    sensor_id.starts_with("radar")
}

/// Effective detection range for `def` (mounted on `o`) against `t`.
fn effective_range(
    db: &Database,
    o: &Entity,
    def: &SensorDef,
    t: &Entity,
    weather_jam: f64,
    day_night: DayNight,
) -> Option<f64> {
    let base = range_for_target(def, t.kind);
    if base <= 0.0 {
        return None;
    }
    let target_class = |e: &Entity| db.platforms.get(&e.class);
    let eff = match def.kind {
        SensorKind::Radar => {
            if t.kind == PlatformKind::Sub {
                return None;
            }
            if o.systems.radar <= 0.05 {
                return None;
            }
            // Air-search ranges already describe aircraft-sized targets; the
            // RCS factor applies to surface/subsurface/missile discrimination
            // only (§5.2 adaptation — was flattening BVR detection to 38 nm).
            let rcs = match t.kind {
                PlatformKind::Aircraft => 1.0,
                _ => target_class(t).map(|c| c.rcs.factor()).unwrap_or(1.0),
            };
            let horizon = horizon_nm(def.alt_m, t.current_alt_for_horizon());
            (base * rcs).min(horizon) * (1.0 - weather_jam.clamp(0.0, 0.8))
                * sensing_radar_modifier(def, t)
        }
        SensorKind::Esm => {
            // ESM hears emitters only: radar-carrying platforms and live
            // sea-skimmers with active seekers.
            let emitter = match t.kind {
                PlatformKind::Ship | PlatformKind::Aircraft | PlatformKind::Base => target_class(t)
                    .map(|c| c.sensors.iter().any(|s| is_radar_sensor(s)))
                    .unwrap_or(false),
                PlatformKind::Missile => {
                    t.projectile.as_ref().map(|p| p.enabled).unwrap_or(false)
                }
                _ => false,
            };
            if !emitter {
                return None;
            }
            let h = horizon_nm(def.alt_m, t.current_alt_for_horizon());
            base.min(h * 1.5) * (1.0 - weather_jam.clamp(0.0, 0.8))
        }
        SensorKind::Visual => {
            let vis = match day_night {
                DayNight::Day => 1.0,
                DayNight::Night => 0.5,
            };
            let horizon = horizon_nm(def.alt_m, t.current_alt_for_horizon());
            let visual_factor = target_class(t).map(|c| c.visual_factor).unwrap_or(1.0);
            (base * visual_factor * vis).min(horizon) * sensing_visual_modifier(o, t)
        }
        SensorKind::Ir => {
            // EO/IR: horizon-limited imaging; strong against heat emitters
            // (aircraft at speed), degraded by weather (hook in rules::sensing).
            if t.kind != PlatformKind::Aircraft && t.kind != PlatformKind::Missile {
                return None;
            }
            let horizon = horizon_nm(def.alt_m, t.current_alt_for_horizon());
            base.min(horizon) * sensing_visual_modifier(o, t)
        }
        SensorKind::SonarPassive => {
            if o.systems.sonar <= 0.05 {
                return None;
            }
            let speed_factor = (t.speed_kts / 10.0).clamp(0.3, 2.2);
            // Torpedoes have no platform class; a running torpedo is loud but
            // short-ranged (official "noisy torpedo" band, ~2-3 nm).
            if t.kind == PlatformKind::Torpedo {
                return Some(base.min(3.0));
            }
            let noise = target_class(t).map(|c| c.noise_db / 50.0).unwrap_or(1.0);
            match t.kind {
                PlatformKind::Sub => {
                    base * speed_factor * noise * propagation_modifier(o, t)
                }
                PlatformKind::Ship => base * 0.35 * speed_factor * noise,
                _ => return None,
            }
        }
        SensorKind::SonarActive => {
            if o.systems.sonar <= 0.05 {
                return None;
            }
            match t.kind {
                PlatformKind::Sub => base * 0.75 * propagation_modifier(o, t),
                PlatformKind::Ship => base * 1.2,
                _ => return None,
            }
        }
        SensorKind::Magnetic => {
            // MAD: only near-surface submarines, very short range.
            if t.kind != PlatformKind::Sub || t.depth_m > 60.0 {
                return None;
            }
            base.min(def.sub_range_nm) * 0.5
        }
    };
    if eff <= 0.0 {
        None
    } else {
        Some(eff)
    }
}

/// Active sonar counter-detection radius: a pinger is heard by passive sets
/// far beyond its own search range.
const PING_REVEAL_FACTOR: f64 = 1.6;

struct PendingEvent {
    vis: Visibility,
    kind: EventKind,
}

pub fn update_sensors(world: &mut World) {
    if world.tick % SENSOR_INTERVAL_TICKS != 0 {
        return;
    }

    let mut pending: Vec<PendingEvent> = Vec::new();

    for side in [Side::Blue, Side::Red] {
        let observer_ids: Vec<EntityId> = world
            .entities
            .values()
            .filter(|e| e.alive && e.side == side && !e.kind.is_projectile())
            .map(|e| e.id)
            .collect();

        // Phase A: strongest detection per enemy entity. The target list is
        // built once per side: rebuilding it inside `consider` cloned every
        // enemy entity for every sensor of every observer (O(N^2) clones) and
        // saturated the CPU once a few dozen aircraft were airborne.
        let mut dets: BTreeMap<EntityId, DetObs> = BTreeMap::new();
        let targets: Vec<Entity> = world
            .entities
            .values()
            .filter(|t| t.alive && t.side != side)
            .cloned()
            .collect();
        let jam = world.weather.jam[side.index()];
        let consider = |world: &World,
                            o: &Entity,
                            def: &SensorDef,
                            dets: &mut BTreeMap<EntityId, DetObs>| {
            for t in &targets {
                let dist = o.pos.range_to(t.pos);
                let Some(eff) =
                    effective_range(&world.db, o, def, t, jam, world.day_night)
                else {
                    continue;
                };
                if dist > eff {
                    continue;
                }
                let obs = match def.kind {
                    SensorKind::Esm => DetObs {
                        sensor: def.id.clone(),
                        observer_pos: o.pos,
                        true_bearing_deg: o.pos.bearing_to(t.pos),
                        bearing_only: true,
                        err_base_nm: 22.0,
                        quality_cap: 0.35,
                    },
                    SensorKind::Visual => DetObs {
                        sensor: def.id.clone(),
                        observer_pos: o.pos,
                        true_bearing_deg: o.pos.bearing_to(t.pos),
                        bearing_only: false,
                        err_base_nm: 0.3,
                        quality_cap: 1.0,
                    },
                    SensorKind::Radar => DetObs {
                        sensor: def.id.clone(),
                        observer_pos: o.pos,
                        true_bearing_deg: o.pos.bearing_to(t.pos),
                        bearing_only: false,
                        err_base_nm: 3.0,
                        quality_cap: 1.0,
                    },
                    SensorKind::SonarPassive => DetObs {
                        sensor: def.id.clone(),
                        observer_pos: o.pos,
                        true_bearing_deg: o.pos.bearing_to(t.pos),
                        bearing_only: false,
                        err_base_nm: 6.0,
                        quality_cap: 0.85,
                    },
                    SensorKind::SonarActive => DetObs {
                        sensor: def.id.clone(),
                        observer_pos: o.pos,
                        true_bearing_deg: o.pos.bearing_to(t.pos),
                        bearing_only: false,
                        err_base_nm: 0.8,
                        quality_cap: 1.0,
                    },
                    SensorKind::Magnetic => DetObs {
                        sensor: def.id.clone(),
                        observer_pos: o.pos,
                        true_bearing_deg: o.pos.bearing_to(t.pos),
                        bearing_only: false,
                        err_base_nm: 0.4,
                        quality_cap: 1.0,
                    },
                    SensorKind::Ir => DetObs {
                        sensor: def.id.clone(),
                        observer_pos: o.pos,
                        true_bearing_deg: o.pos.bearing_to(t.pos),
                        bearing_only: false,
                        err_base_nm: 0.5,
                        quality_cap: 0.95,
                    },
                };
                let better = match dets.get(&t.id) {
                    None => true,
                    Some(cur) => {
                        (obs.quality_cap - obs.err_base_nm * 0.01)
                            > (cur.quality_cap - cur.err_base_nm * 0.01)
                    }
                };
                if better {
                    dets.insert(t.id, obs);
                }
            }
        };

        for oid in &observer_ids {
            let sensors: Vec<(bool, SensorDef)> = {
                let o = &world.entities[oid];
                o.sensors
                    .iter()
                    .map(|inst| (inst.active, world.db.sensors.get(&inst.sensor).cloned()))
                    .filter_map(|(a, d)| d.map(|d| (a, d)))
                    .collect()
            };
            for (active_flag, def) in &sensors {
                if def.kind == SensorKind::SonarActive && !active_flag {
                    continue;
                }
                consider(world, &world.entities[oid], def, &mut dets);
            }
        }

        // Phase B: apply to the side's contact book.
        pending.extend(apply_detections(world, side, &dets));
    }

    // Active-sonar counter-detection (rules §5.4): the LISTENER hears the
    // ping — pinging exposes the pinger to every enemy passive set within
    // PING_REVEAL_FACTOR of its own range.
    let pingers: Vec<(EntityId, Vec2, f64, Side)> = world
        .entities
        .values()
        .filter(|e| e.alive && !e.kind.is_projectile())
        .filter_map(|e| {
            let reveal = e
                .sensors
                .iter()
                .filter_map(|s| {
                    let d = world.db.sensors.get(&s.sensor)?;
                    if d.kind == SensorKind::SonarActive && s.active {
                        Some(d.sub_range_nm * PING_REVEAL_FACTOR)
                    } else {
                        None
                    }
                })
                .fold(0.0f64, f64::max);
            if reveal > 0.0 {
                Some((e.id, e.pos, reveal, e.side))
            } else {
                None
            }
        })
        .collect();
    for (pid, ppos, reveal, pside) in pingers {
        let listeners: Vec<Entity> = world
            .entities
            .values()
            .filter(|t| {
                t.alive
                    && t.side != pside
                    && !t.kind.is_projectile()
                    && t.sensors.iter().any(|s| {
                        world
                            .db
                            .sensors
                            .get(&s.sensor)
                            .map(|d| d.kind == SensorKind::SonarPassive)
                            .unwrap_or(false)
                    })
            })
            .cloned()
            .collect();
        for listener in listeners {
            if listener.pos.range_to(ppos) <= reveal {
                let mut m = BTreeMap::new();
                m.insert(
                    pid,
                    DetObs {
                        sensor: "passive sonar hears active ping".to_string(),
                        observer_pos: listener.pos,
                        true_bearing_deg: listener.pos.bearing_to(ppos),
                        bearing_only: false,
                        err_base_nm: 1.0,
                        quality_cap: 0.9,
                    },
                );
                pending.extend(apply_detections(world, listener.side, &m));
            }
        }
    }

    for side in [Side::Blue, Side::Red] {
        pending.extend(decay_contacts(world, side));
    }

    for ev in pending {
        world.emit(ev.kind, ev.vis);
    }
}

/// One-off detection flash (missile launch plume / ESM intercept of a launch).
/// Creates or reinforces a contact for the enemy side about `about`.
pub fn flash_detection(world: &mut World, about: EntityId, sensor_label: &str, err_nm: f64, cap: f64) {
    let Some(owner) = world.entity(about).map(|e| e.side) else { return };
    let enemy = owner.other();
    let pos = world.entity(about).map(|e| e.pos).unwrap_or_default();
    let mut m = BTreeMap::new();
    m.insert(
        about,
        DetObs {
            sensor: sensor_label.to_string(),
            observer_pos: pos,
            true_bearing_deg: 0.0,
            bearing_only: false,
            err_base_nm: err_nm,
            quality_cap: cap,
        },
    );
    for ev in apply_detections(world, enemy, &m) {
        world.emit(ev.kind, ev.vis);
    }
    // Launch counter-detection notice to the shooter's own side.
    world.emit(
        crate::events::EventKind::CounterDetected { side: owner, unit: about, by_side: enemy },
        crate::events::Visibility::Side(owner),
    );
}

fn apply_detections(
    world: &mut World,
    side: Side,
    dets: &BTreeMap<EntityId, DetObs>,
) -> Vec<PendingEvent> {
    let mut out: Vec<PendingEvent> = Vec::new();

    // Precompute per-target truth data (immutable phase).
    struct Update {
        tid: EntityId,
        truth_pos: Vec2,
        truth_course: f64,
        truth_speed: f64,
        class_name: Option<String>,
        is_incoming: bool,
        weapon_speed_kts: f64,
    }
    let mut updates: Vec<(EntityId, &DetObs, Update)> = Vec::new();
    for (tid, obs) in dets {
        let Some(t) = world.entities.get(tid) else { continue };
        let weapon_speed = t
            .projectile
            .as_ref()
            .and_then(|p| world.db.weapons.get(&p.weapon))
            .map(|w| w.speed_kts)
            .unwrap_or(500.0)
            .max(20.0);
        updates.push((
            *tid,
            obs,
            Update {
                tid: *tid,
                truth_pos: t.pos,
                truth_course: t.course_deg,
                truth_speed: t.speed_kts,
                class_name: world.db.platforms.get(&t.class).map(|p| p.name.clone()),
                is_incoming: t.kind == PlatformKind::Missile || t.kind == PlatformKind::Torpedo,
                weapon_speed_kts: weapon_speed,
            },
        ));
    }

    let mut just_identified: Vec<(ContactId, String)> = Vec::new();
    {
        let state = &mut world.sides[side.index()];
        for (tid, obs, upd) in &updates {
            let existing = state
                .contacts
                .values_mut()
                .find(|c| c.about == *tid && !c.stale);

            match existing {
                Some(c) => {
                    c.quality = (c.quality + 0.15).min(obs.quality_cap);
                    c.last_update_tick = world.tick;
                    c.bearing_only = obs.bearing_only;
                    if !c.sensors.contains(&obs.sensor) {
                        c.sensors.push(obs.sensor.clone());
                    }
                    let q = c.quality;
                    let err = c.offset.scaled((1.0 - q) * (1.0 - q));
                    c.estimate = upd.truth_pos.add(err);
                    let course_noise = (1.0 - q) * 40.0;
                    let mut rng = world
                        .rng
                        .derive(crate::rng::Rng::hash_tag(world.tick, upd.tid.0, "est_course"));
                    c.est_course_deg =
                        crate::geom::normalize_deg(upd.truth_course + rng.range(-course_noise, course_noise));
                    c.est_speed_kts = upd.truth_speed * (1.0 - (1.0 - q) * 0.3);
                    let dist_now = upd.truth_pos.range_to(c.estimate);
                    let identified_now =
                        c.class_name.is_none() && (q >= 0.7 || dist_now < 6.0);
                    if identified_now {
                        if let Some(name) = &upd.class_name {
                            c.class_name = Some(name.clone());
                            if !c.announced {
                                c.announced = true;
                                just_identified.push((c.id, name.clone()));
                            }
                        }
                    }
                }
                None => {
                    state.next_contact += 1;
                    let cid = ContactId(state.next_contact);
                    let mut rng = world
                        .rng
                        .derive(crate::rng::Rng::hash_tag(0, tid.0, "contact_offset"));
                    let ang = rng.range(0.0, 360.0);
                    let mag = rng.range(0.2, obs.err_base_nm);
                    let offset = Vec2::from_bearing(ang).scaled(mag);
                    let q = 0.25f64.min(obs.quality_cap);
                    let mut c = Contact {
                        id: cid,
                        about: *tid,
                        category: world.entities[tid].kind,
                        class_name: None,
                        estimate: Vec2::zero(),
                        est_course_deg: upd.truth_course,
                        est_speed_kts: upd.truth_speed,
                        bearing_only: obs.bearing_only,
                        quality: q,
                        last_update_tick: world.tick,
                        first_tick: world.tick,
                        stale: false,
                        sensors: vec![obs.sensor.clone()],
                        offset,
                        announced: false,
                    };
                    if obs.bearing_only {
                        // Bearing-only fix: place the estimate along the true
                        // bearing at an uncertain range.
                        let mut rng2 = world
                            .rng
                            .derive(crate::rng::Rng::hash_tag(1, tid.0, "esm_range"));
                        let guess = rng2.range(12.0, 45.0);
                        c.estimate = obs.observer_pos.advanced(obs.true_bearing_deg, guess);
                        c.quality = q.min(0.3);
                        c.est_speed_kts = 0.0;
                    } else {
                        let err = offset.scaled((1.0 - q) * (1.0 - q));
                        c.estimate = upd.truth_pos.add(err);
                    }
                    state.contacts.insert(cid, c);
                }
            }
        }
    }

    // Emit events (needs immutable world access).
    for (tid, _obs, upd) in &updates {
        let Some(cid) = world
            .sides[side.index()]
            .contacts
            .values()
            .find(|c| c.about == *tid && c.first_tick == world.tick)
            .map(|c| c.id)
        else {
            // Existing contact: maybe identified this pass.
            continue;
        };
        out.push(PendingEvent {
            vis: Visibility::Side(side),
            kind: EventKind::Detection {
                side,
                contact: cid,
                category: world.entities[tid].kind.category().to_string(),
                pos_hint: world.contact(side, cid).map(|c| c.estimate).unwrap_or_default(),
                sensor: world
                    .contact(side, cid)
                    .map(|c| c.sensors.first().cloned().unwrap_or_default())
                    .unwrap_or_default(),
            },
        });
        if upd.is_incoming {
            let dist = upd.truth_pos.range_to(
                world.contact(side, cid).map(|c| c.estimate).unwrap_or_default(),
            );
            let eta = dist / upd.weapon_speed_kts * 3600.0;
            out.push(PendingEvent {
                vis: Visibility::Side(side),
                kind: EventKind::Incoming { side, contact: cid, eta_s: eta },
            });
            if world.entities[tid].kind == PlatformKind::Torpedo {
                out.push(PendingEvent {
                    vis: Visibility::Side(side),
                    kind: EventKind::TorpedoDetected { side, contact: cid },
                });
            }
        }
    }
    // Identification events.
    for (cid, name) in just_identified {
        out.push(PendingEvent {
            vis: Visibility::Side(side),
            kind: EventKind::ContactIdentified { side, contact: cid, class_name: name },
        });
    }
    out
}

fn decay_contacts(world: &mut World, side: Side) -> Vec<PendingEvent> {
    let mut out = Vec::new();
    let mut lost: Vec<ContactId> = Vec::new();
    {
        let state = &mut world.sides[side.index()];
        for c in state.contacts.values_mut() {
            if c.stale {
                continue;
            }
            let scanned_this_pass = (world.tick - c.last_update_tick) < SENSOR_INTERVAL_TICKS * 2;
            if !scanned_this_pass {
                c.quality -= 0.10;
                if c.quality <= 0.0 {
                    c.stale = true;
                    c.quality = 0.0;
                    lost.push(c.id);
                }
            }
        }
    }
    for cid in lost {
        out.push(PendingEvent {
            vis: Visibility::Side(side),
            kind: EventKind::ContactLost { side, contact: cid },
        });
    }
    let stale_cutoff = world.tick - (CONTACT_STALE_S as i64);
    world.sides[side.index()]
        .contacts
        .retain(|_, c| !(c.stale && c.last_update_tick < stale_cutoff));
    out
}

/// Chaff burst: enemy radar-driven contacts on this unit degrade; weak locks
/// break entirely (quality 0 -> stale next decay pass).
pub fn chaff_breaks_lock(world: &mut World, unit: EntityId) {
    let Some(owner) = world.entity(unit).map(|e| e.side) else { return };
    let enemy = owner.other();
    let state = &mut world.sides[enemy.index()];
    for c in state.contacts.values_mut() {
        if c.about != unit || c.stale {
            continue;
        }
        if c.sensors.iter().any(|s| is_radar_sensor(s)) {
            c.quality = (c.quality - 0.35).max(0.0);
        }
    }
}

/// Nominal detection ranges for the UI (day, no jam, nominal target):
/// (vs_air@6000m, vs_surface, vs_sub@60m). `None` = cannot detect that
/// domain with this sensor. Mirrors `effective_range` at standard
/// conditions so the map rings match what the referee actually uses.
pub fn nominal_ranges(db: &Database, o: &crate::world::Entity, inst: &crate::world::SensorInstance) -> (Option<f64>, Option<f64>, Option<f64>) {
    let Some(def) = db.sensors.get(&inst.sensor) else { return (None, None, None) };
    if o.systems.radar <= 0.05 && matches!(def.kind, SensorKind::Radar) {
        return (None, None, None);
    }
    if o.systems.sonar <= 0.05 && matches!(def.kind, SensorKind::SonarPassive | SensorKind::SonarActive) {
        return (None, None, None);
    }
    if def.kind == SensorKind::SonarActive && !inst.active {
        return (None, None, None);
    }
    let horizon_to = |t_alt: f64| horizon_nm(def.alt_m, t_alt);
    let (mut a, mut sf, mut sb) = (None, None, None);
    if def.air_range_nm > 0.0 {
        let v = def.air_range_nm.min(horizon_to(6000.0));
        a = Some(v);
    }
    if def.surface_range_nm > 0.0 {
        let v = def.surface_range_nm.min(horizon_to(0.0)).max(0.0);
        sf = Some(v);
    }
    if def.sub_range_nm > 0.0 {
        let v = match def.kind {
            SensorKind::SonarPassive => def.sub_range_nm * 1.0,   // nominal speed/noise
            SensorKind::SonarActive => def.sub_range_nm * 0.75,
            SensorKind::Magnetic => def.sub_range_nm * 0.5,
            _ => def.sub_range_nm * 0.5,
        };
        sb = Some(v);
    }
    (a, sf, sb)
}

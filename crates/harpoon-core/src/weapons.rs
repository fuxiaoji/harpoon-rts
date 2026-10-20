//! Weapons: fire-control scanning, projectile flight, and terminal
//! adjudication. The tabletop's dice-driven attack tables become deterministic
//! referee rolls here; the defense chain for sea-skimmers is
//! SAM envelope -> chaff -> CIWS -> terminal Pk, mirroring the Harpoon
//! anti-missile sequence in continuous time.

use crate::db::{Guidance, WeaponDef, WeaponKind};
use crate::damage;
use crate::events::{EventKind, Visibility};
use crate::geom::{KTS_TO_NM_PER_S, Vec2};
use crate::sensors;
use crate::types::{ContactId, EntityId, PlatformKind, Side};
use crate::world::{Entity, World, TICK_SECONDS};

pub fn update_weapons(world: &mut World) {
    tick_cooldowns(world);
    fire_control_scan(world);
    advance_projectiles(world);
}

fn tick_cooldowns(world: &mut World) {
    for e in world.entities.values_mut() {
        if !e.alive {
            continue;
        }
        for m in e.weapons.iter_mut() {
            m.cooldown_s = (m.cooldown_s - TICK_SECONDS).max(0.0);
        }
        if e.chaff_active_s > 0.0 {
            e.chaff_active_s = (e.chaff_active_s - TICK_SECONDS).max(0.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Fire control
// ---------------------------------------------------------------------------

struct ShooterCtx {
    id: EntityId,
    side: Side,
    pos: Vec2,
    altitude_m: f64,
    fire_on: bool,
    assigned: Option<ContactId>,
}

pub fn fire_control_scan(world: &mut World) {
    let shooters: Vec<ShooterCtx> = world
        .entities
        .values()
        .filter(|e| {
            e.alive
                && !e.kind.is_projectile()
                && !e.weapons.is_empty()
                && e.systems.fire_control > 0.05
        })
        .map(|e| ShooterCtx {
            id: e.id,
            side: e.side,
            pos: e.pos,
            altitude_m: e.altitude_m,
            fire_on: e.fire_on,
            assigned: e.assigned,
        })
        .collect();

    for shooter in shooters {
        // Resolve the target: assigned contact first; else nearest valid
        // contact when weapons are free.
        let mut target: Option<(ContactId, EntityId)> = None;
        if let Some(cid) = shooter.assigned {
            if let Some(eid) = resolve_contact(world, shooter.side, cid) {
                target = Some((cid, eid));
            }
        }
        if target.is_none() && shooter.fire_on {
            target = pick_free_target(world, &shooter);
        }
        let Some((contact_id, target_id)) = target else { continue };
        let Some(target_kind) = world.entity(target_id).map(|t| t.kind) else { continue };

        // Find the first weapon that can legally engage this target now.
        let mounts: Vec<(usize, WeaponDef, u8, u16, f64)> = {
            let e = world.entity(shooter.id).unwrap();
            e.weapons
                .iter()
                .enumerate()
                .filter_map(|(i, m)| {
                    let def = world.db.weapons.get(&m.weapon)?.clone();
                    Some((i, def, m.mounts, m.ammo, m.cooldown_s))
                })
                .collect()
        };

        for (idx, mut def, mounts_n, ammo, cooldown) in mounts {
            if ammo == 0 || cooldown > 0.0 {
                continue;
            }
            // Weapon-target compatibility gate: a weapon only engages target
            // kinds its `targets` table admits (e.g. SLCM never shoots at
            // ships; SAM never at surface contacts).
            if !def.targets.accepts(target_kind) {
                continue;
            }
            let dist = shooter.pos.range_to(world.entity(target_id).map(|t| t.pos).unwrap_or_default());
            let shooter_alt_m = shooter.altitude_m;
            let engaged = match def.kind {
                WeaponKind::Ciws => false, // CIWS engages inside the terminal chain
                WeaponKind::Gun => {
                    // AA-capable guns may engage aircraft (§8.1.3, PDF p. 83).
                    // The `targets` gate above already rejects aircraft for
                    // guns without `air: true`; missiles remain in the CIWS
                    // terminal chain and subs are not gun targets.
                    dist <= def.range_nm
                        && dist >= def.min_range_nm
                        && !matches!(target_kind, PlatformKind::Missile | PlatformKind::Sub)
                }
                WeaponKind::Sam | WeaponKind::Aam => {
                    matches!(target_kind, PlatformKind::Aircraft | PlatformKind::Missile)
                        && dist <= def.range_nm
                        && dist >= def.min_range_nm
                }
                WeaponKind::Ascm | WeaponKind::Slcm => {
                    matches!(target_kind, PlatformKind::Ship | PlatformKind::Base)
                        && dist <= def.range_nm
                        && dist >= def.min_range_nm
                }
                WeaponKind::Torpedo | WeaponKind::Lwt => {
                    dist <= def.range_nm
                        && shooter.assigned.is_some() // torpedoes are fired deliberately
                }
                WeaponKind::AswRocket => {
                    target_kind == PlatformKind::Sub && dist <= def.range_nm
                }
                WeaponKind::Bomb | WeaponKind::Rocket => {
                    // Unguided air attack (§9.6): release from a low pass.
                    matches!(target_kind, PlatformKind::Ship | PlatformKind::Base)
                        && dist <= def.range_nm
                        && shooter_alt_m < 900.0
                }
            };
            if !engaged {
                continue;
            }

            // Visual fire-control solution (§6.3.5, PDF p.6-5): if the target
            // is held in the contact's visual channel, lookouts can spot and
            // correct the fall of shot — guns get +15% Pk. The contact's
            // `sensors` list is the engine's record of how the track was
            // obtained (sensors.rs:218-225 stores the `visual` sensor id).
            if def.kind == WeaponKind::Gun
                && world
                    .contact(shooter.side, contact_id)
                    .map(|c| c.sensors.iter().any(|s| s == "visual"))
                    .unwrap_or(false)
            {
                def.pk *= 1.15;
            }

            match def.kind {
                WeaponKind::Gun => {
                    let salvo = def.salvo.max(mounts_n.min(2));
                    gun_salvo(world, shooter.id, contact_id, target_id, &def, salvo);
                }
                WeaponKind::Sam | WeaponKind::Aam => {
                    let mut salvo = def.salvo;
                    if def.kind == WeaponKind::Sam && def.directors > 0 {
                        // SAM fire-control channels (§6.1 SAM Salvo Summary;
                        // §8.1.1 step 8, PDF p. 81): each director controls
                        // one engagement of up to `salvo` missiles, so the
                        // ship can keep `directors × salvo` rounds in flight.
                        // Count this mount's live projectiles as the
                        // referee's channel occupancy.
                        let capacity = def.directors as usize * def.salvo.max(1) as usize;
                        let in_flight = in_flight_projectiles(world, shooter.id, &def.id);
                        if in_flight >= capacity {
                            continue; // channels full: try another mount
                        }
                        // Clamp to free channels; the result is <= salvo <= u8.
                        let available = (capacity - in_flight).min(salvo as usize);
                        salvo = available as u8;
                    }
                    for _ in 0..salvo {
                        launch(world, shooter.id, &def, Some(target_id), None);
                    }

                    // §8.1.8 (PDF p.8-10): a weapon fired in the increment a
                    // unit arrives at (or adjacent to) a friendly unit can hit
                    // that friend; the friendly's radar/visual return is much
                    // larger than the missile's. Adapted to this engine's
                    // air-defense geometry: when a friendly aircraft is within
                    // 2.0 nm of the engaged air/missile target, there is a 5%
                    // chance per engagement to damage the nearest friendly
                    // aircraft at half weapon damage. (Estimated adaptation:
                    // the rule text describes friendly ships; the ask scopes
                    // the mechanic to same-side aircraft.)
                    let nearest_friend = world.entity(target_id).map(|t| t.pos).and_then(|tpos| {
                        world
                            .entities
                            .values()
                            .filter(|f| {
                                f.alive
                                    && f.side == shooter.side
                                    && f.kind == PlatformKind::Aircraft
                                    && f.id != shooter.id
                            })
                            .map(|f| (f.pos.range_to(tpos), f.id))
                            .filter(|(d, _)| *d <= 2.0)
                            .min_by(|a, b| {
                                a.0.partial_cmp(&b.0)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                    });
                    if let Some((_dist, friend_id)) = nearest_friend {
                        let mut rng = world.scope_rng(shooter.id, "friendly_fire");
                        if rng.chance(0.05) {
                            let mut half = def.clone();
                            half.damage *= 0.5;
                            damage::apply_hit_from(world, friend_id, &half, false, shooter.side);
                            world.emit(
                                EventKind::RulesEvent {
                                    domain: "firectl".into(),
                                    rule: "friendly_fire".into(),
                                    side: Some(shooter.side),
                                    unit: Some(friend_id),
                                    contact: Some(contact_id),
                                    detail: format!(
                                        "SAM/AAM engagement by #{} within 2.0 nm of friendly aircraft #{}; 5% misidentification, half damage",
                                        shooter.id.0, friend_id.0
                                    ),
                                },
                                Visibility::Side(shooter.side),
                            );
                        }
                    }

                    world.emit(
                        EventKind::SamShot {
                            side: shooter.side,
                            shooter: shooter.id,
                            weapon: def.name.clone(),
                            target_contact: contact_id,
                        },
                        Visibility::Side(shooter.side),
                    );
                }
                WeaponKind::Ascm | WeaponKind::Slcm => {
                    let salvo = def.salvo;
                    for _ in 0..salvo {
                        launch(world, shooter.id, &def, Some(target_id), None);
                    }
                }
                WeaponKind::Torpedo | WeaponKind::Lwt => {
                    launch(world, shooter.id, &def, Some(target_id), None);
                }
                WeaponKind::AswRocket => {
                    let salvo = def.salvo;
                    for _ in 0..salvo {
                        launch(world, shooter.id, &def, Some(target_id), None);
                    }
                }
                WeaponKind::Bomb | WeaponKind::Rocket => {
                    let salvo = def.salvo.max(mounts_n.min(2));
                    gun_salvo(world, shooter.id, contact_id, target_id, &def, salvo);
                }
                WeaponKind::Ciws => unreachable!(),
            }

            // Consume ammo + start reload.
            let Some(e) = world.entity_mut(shooter.id) else { continue };
            let m = &mut e.weapons[idx];
            m.ammo = m.ammo.saturating_sub(salvo_or(&def, 1));
            m.cooldown_s = def.reload_s;
            break; // one engagement per shooter per scan
        }
    }
}

fn salvo_or(def: &WeaponDef, fallback: u16) -> u16 {
    match def.kind {
        WeaponKind::Gun => def.salvo as u16,
        _ => fallback,
    }
}

/// Live projectiles of one weapon type this shooter still has in flight.
/// SAM director channels count these to cap simultaneous engagements
/// (§6.1 / §8.1.1 step 8: one engagement = `salvo` missiles per director).
fn in_flight_projectiles(world: &World, shooter: EntityId, weapon_id: &str) -> usize {
    world
        .entities
        .values()
        .filter(|p| {
            p.alive
                && p.kind == PlatformKind::Missile
                && p.projectile
                    .as_ref()
                    .map(|d| d.launched_by == shooter && d.weapon == weapon_id)
                    .unwrap_or(false)
        })
        .count()
}

pub fn resolve_contact(world: &World, side: Side, cid: ContactId) -> Option<EntityId> {
    let c = world.contact(side, cid)?;
    if c.stale {
        return None;
    }
    let e = world.entity(c.about)?;
    if !e.alive {
        return None;
    }
    Some(e.id)
}

fn pick_free_target(world: &World, shooter: &ShooterCtx) -> Option<(ContactId, EntityId)> {
    let mut candidates: Vec<(f64, ContactId, EntityId)> = world.sides[shooter.side.index()]
        .contacts
        .values()
        .filter(|c| !c.stale && c.quality > 0.25)
        .filter(|c| {
            // A radar fire-control solution is required to shoot at aircraft:
            // bearing-only ESM fixes never authorize BVR shots (§6.3).
            if world.entity(c.about).map(|e| e.kind) == Some(PlatformKind::Aircraft) {
                !c.bearing_only && c.quality > 0.4
            } else {
                true
            }
        })
        .filter_map(|c| {
            let e = world.entity(c.about)?;
            if !e.alive {
                return None;
            }
            // Surface fire control will not shoot at torpedoes; incoming
            // missiles ARE valid targets for air-defense doctrine.
            if e.kind == PlatformKind::Torpedo {
                return None;
            }
            // Sonobuoys are expendable sensors; weapons-free never spends
            // missiles on them even though they resolve as surface contacts.
            if e.class == crate::rules::asw::SONOBUOY_CLASS {
                return None;
            }
            Some((shooter.pos.range_to(e.pos), c.id, e.id))
        })
        .collect();
    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    candidates.first().map(|(_, c, e)| (*c, *e))
}

/// Gun-line: instant adjudication with tracer event. Hit chance follows the
/// rules' firing-arc bands (beam ≈ 100%, quarter ≈ 73%, bow/stern ≈ 33%
/// relative), scaled by range.
fn gun_salvo(
    world: &mut World,
    shooter_id: EntityId,
    contact_id: ContactId,
    target_id: EntityId,
    def: &WeaponDef,
    rounds: u8,
) {
    let shooter_name = world.entity(shooter_id).map(|e| e.name.clone()).unwrap_or_default();
    let side = world.entity(shooter_id).map(|e| e.side).unwrap_or(Side::Blue);
    let (dist, rel_deg) = world
        .entity(shooter_id)
        .zip(world.entity(target_id))
        .map(|(s, t)| {
            let bearing_from_target = t.pos.bearing_to(s.pos);
            let rel = crate::geom::angle_diff(t.course_deg, bearing_from_target).abs();
            (s.pos.range_to(t.pos), rel)
        })
        .unwrap_or((f64::MAX, 180.0));
    let target_is_air = world
        .entity(target_id)
        .map(|t| t.kind == PlatformKind::Aircraft)
        .unwrap_or(false);
    let (range_base, aspect_mult) = if target_is_air {
        // AA gunnery path (§8.1.3, PDF pp. 83-84): dedicated range bands and
        // Crossing Target Table aspect. A crossing aircraft is the hardest
        // shot, the inverse of the surface beam-on bonus below.
        (
            crate::rules::aaw::aa_gun_range_base(dist, def.range_nm),
            crate::rules::aaw::aa_gun_aspect_mult(rel_deg, dist),
        )
    } else {
        let range_frac = dist / def.range_nm.max(1.0);
        let range_base = if range_frac <= 0.25 {
            0.75
        } else if range_frac <= 0.5 {
            0.55
        } else if range_frac <= 0.75 {
            0.25
        } else {
            0.15
        };
        // Official range-band base hit rates (75/55/25/15%) with bearing
        // as an additive penalty, approximated multiplicatively.
        let band = if (45.0..=135.0).contains(&rel_deg) {
            1.00 // beam: no penalty
        } else if (20.0..45.0).contains(&rel_deg) || (135.0..160.0).contains(&rel_deg) {
            0.90 // quarter: ~-1 on D100
        } else {
            0.80 // bow/stern on: ~-2..-4
        };
        (range_base, band)
    };
    let pk = range_base * def.pk * aspect_mult;
    let mut rng = world.scope_rng(shooter_id, "gun");
    let mut hits = 0u8;
    for _ in 0..rounds {
        if rng.chance(pk) {
            hits += 1;
            damage::apply_hit_from(world, target_id, def, false, side);
        }
    }
    world.emit(
        EventKind::GunAction {
            side,
            shooter: shooter_id,
            shooter_name,
            target_contact: contact_id,
            hits,
        },
        Visibility::Side(side),
    );
}

// ---------------------------------------------------------------------------
// Projectile spawning
// ---------------------------------------------------------------------------

pub fn launch(
    world: &mut World,
    shooter_id: EntityId,
    def: &WeaponDef,
    target: Option<EntityId>,
    aim_override: Option<Vec2>,
) -> Option<EntityId> {
    let shooter = world.entity(shooter_id)?;
    let side = shooter.side;
    let start = shooter.pos;
    let aim = aim_override
        .or(target.and_then(|t| world.entity(t)).map(|t| t.pos))
        .unwrap_or(start);
    let eid = world.allocate_id();
    let kind = if matches!(def.kind, WeaponKind::Torpedo | WeaponKind::Lwt) {
        PlatformKind::Torpedo
    } else {
        PlatformKind::Missile
    };
    let course = start.bearing_to(aim);
    let alt = if matches!(def.guidance, Guidance::SeaSkimmer) {
        8.0
    } else if matches!(def.guidance, Guidance::HomingAir) {
        target
            .and_then(|t| world.entity(t))
            .map(|t| t.altitude_m)
            .unwrap_or(100.0)
    } else {
        5.0
    };
    let e = Entity {
        id: eid,
        side,
        class: def.id.clone(),
        name: def.name.clone(),
        kind,
        pos: start.advanced(course, 0.03),
        course_deg: course,
        speed_kts: def.speed_kts,
        ordered_course_deg: course,
        ordered_speed_kts: def.speed_kts,
        waypoints: Default::default(),
        altitude_m: alt,
        ordered_altitude_m: alt,
        depth_m: if kind == PlatformKind::Torpedo { 25.0 } else { 0.0 },
        ordered_depth_m: 25.0,
        hp: 1.0,
        hp_max: 1.0,
        alive: true,
        systems: Default::default(),
        fire_on: false,
        assigned: None,
        weapons: vec![],
        sensors: vec![],
        chaff: 0,
        decoys: 0,
        auto_chaff: false,
        chaff_active_s: 0.0,
        endurance_s: None,
        burn_s: 0.0,
        flooding_s: 0.0,
            ext: Default::default(),
        deck: None,
        mission: None,
        station: None,
        last_emit_tick: world.tick,
        projectile: Some(crate::world::ProjectileData {
            weapon: def.id.clone(),
            target,
            aim,
            launched_by: shooter_id,
            launched_by_side: side,
            live_s: 0.0,
            enabled: false,
            decoyed: false,
            decoy_point: None,
            chaff_checked: false,
            ciws_checked: false,
            passes: 0,
        }),
    };
    world.entities.insert(eid, e);
    world.stats.projectiles_launched += 1;
    world.emit(
        EventKind::WeaponsRelease {
            side,
            shooter: shooter_id,
            shooter_name: world.entity(shooter_id).map(|e| e.name.clone()).unwrap_or_default(),
            weapon: def.name.clone(),
            count: 1,
        },
        Visibility::Side(side),
    );
    // Launch plume: only radar-missile launches expose the shooter
    // (rules §5.3/§5.8 — torpedo and rocket launches are silent).
    let silent = matches!(
        def.kind,
        WeaponKind::Torpedo | WeaponKind::Lwt | WeaponKind::AswRocket
    );
    if !silent {
        let is_big = matches!(def.kind, WeaponKind::Ascm | WeaponKind::Slcm);
        sensors::flash_detection(world, shooter_id, "launch flash", if is_big { 7.0 } else { 4.0 }, 0.45);
    }
    Some(eid)
}

/// Manual Fire command (routed from sim.rs command processing).
pub fn manual_fire(world: &mut World, side: Side, unit: EntityId, contact: ContactId, weapon_id: &str, rounds: u8) -> bool {
    // A contact record is required. A stale record is usable only for a BOL
    // shot (§8.4.2); a normal fire-control solution needs a live target.
    let Some(contact_rec) = world.contact(side, contact).cloned() else { return false };
    let Some(e) = world.entity(unit) else { return false };
    if e.side != side {
        return false;
    }
    let Some(idx) = e.weapons.iter().position(|m| m.weapon == weapon_id) else { return false; };
    let (ammo_ok, cooldown_ok, ammo) = {
        let m = &e.weapons[idx];
        (m.ammo > 0, m.cooldown_s <= 0.0, m.ammo)
    };
    if !ammo_ok || !cooldown_ok {
        return false;
    }
    let Some(target) = world.entity(contact_rec.about) else { return false };
    if !target.alive {
        return false;
    }
    let (eid, target_pos, target_kind) = (target.id, target.pos, target.kind);
    let mut def = match world.db.weapons.get(weapon_id) {
        Some(d) => d.clone(),
        None => return false,
    };

    // Fire-control solution gate (§6.3.1 — a firing solution requires a
    // quality contact). Bearing-only launch (§6.3.6 / §8.4.2) is the
    // deliberate exception: a poor track (quality < 0.3, stale allowed) may
    // still launch air-defense weapons with terminal homing.
    let bol = contact_rec.quality < 0.3;
    if !bol && contact_rec.stale {
        return false;
    }

    let shooter_pos = world.entity(unit).map(|s| s.pos).unwrap_or_default();
    // BOL is planned against the estimated position; a normal shot uses the
    // live target position.
    let firing_range = if bol {
        shooter_pos.range_to(contact_rec.estimate_at(world.tick))
    } else {
        shooter_pos.range_to(target_pos)
    };
    if firing_range > def.range_nm || firing_range < def.min_range_nm {
        return false;
    }

    let n = rounds.min((ammo as u8).min(def.salvo.max(1) * 2));

    if bol {
        // BOL is scoped to air-defense weapons with terminal homing fired at
        // air/missile contacts (§6.3.6 wording; §8.4.2 procedure). Because
        // ProjectileData has no Pk field, the adaptation adjudicates the
        // salvo immediately instead of spawning homing projectiles — the
        // comment "BOL salvo does not guide home" records the choice. Each
        // round's Pk is ×0.25.
        let bol_capable = matches!(def.kind, WeaponKind::Sam | WeaponKind::Aam)
            && matches!(contact_rec.category, PlatformKind::Aircraft | PlatformKind::Missile)
            && matches!(def.guidance, Guidance::HomingAir)
            && def.targets.accepts(target_kind);
        if !bol_capable {
            return false;
        }
        // Include the contact in the scope so two BOL calls by the same unit
        // in one tick do not share a roll stream.
        let bol_scope = format!("bol_terminal.{}", contact.0);
        let mut rng = world.scope_rng(unit, &bol_scope);
        let mut fired = 0u16;
        for _ in 0..n {
            if rng.chance(def.pk * 0.25) {
                damage::apply_hit_from(world, eid, &def, false, side);
            }
            fired += 1;
        }
        if fired > 0 {
            world.emit(
                EventKind::RulesEvent {
                    domain: "firectl".into(),
                    rule: "bol_launch".into(),
                    side: Some(side),
                    unit: Some(unit),
                    contact: Some(contact),
                    detail: format!(
                        "BOL launch: track q={:.2} < 0.30, {} round(s), Pk ×0.25, immediate unguided adjudication (adapted §8.4.2)",
                        contact_rec.quality, fired
                    ),
                },
                Visibility::Side(side),
            );
        }
        let Some(e) = world.entity_mut(unit) else { return false };
        let m = &mut e.weapons[idx];
        m.ammo = m.ammo.saturating_sub(fired);
        m.cooldown_s = def.reload_s;
        return fired > 0;
    }

    // Visual fire-control solution (§6.3.5, PDF p.6-5): lookouts spotting /
    // correcting the fall of shot gives a gun +15% Pk.
    if def.kind == WeaponKind::Gun && contact_rec.sensors.iter().any(|s| s == "visual") {
        def.pk *= 1.15;
    }

    let mut fired = 0u16;
    for _ in 0..n {
        if def.kind.is_gun_like() {
            // Guns adjudicate per salvo; manual fire = one salvo per round count.
            gun_salvo(world, unit, contact, eid, &def, 1);
            fired += 1;
            if fired >= rounds as u16 {
                break;
            }
        } else if launch(world, unit, &def, Some(eid), None).is_some() {
            fired += 1;
        }
    }
    let Some(e) = world.entity_mut(unit) else { return false };
    let m = &mut e.weapons[idx];
    m.ammo = m.ammo.saturating_sub(fired);
    m.cooldown_s = def.reload_s;
    fired > 0
}

// ---------------------------------------------------------------------------
// Projectile flight & terminal adjudication
// ---------------------------------------------------------------------------

pub fn advance_projectiles(world: &mut World) {
    let ids: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.alive && e.kind.is_projectile())
        .map(|e| e.id)
        .collect();

    for id in ids {
        let Some(e) = world.entities.get(&id) else { continue };
        // An earlier projectile in this snapshot may already have intercepted it.
        if !e.alive { continue; }
        let Some(data) = e.projectile.clone() else { continue };
        let Some(def) = world.db.weapons.get(&data.weapon).cloned() else { continue };

        match def.guidance {
            Guidance::SeaSkimmer => advance_sea_skimmer(world, id, def, data),
            Guidance::HomingAir => advance_homing_air(world, id, def, data),
            Guidance::TorpedoActive | Guidance::WakeHome => advance_torpedo(world, id, def, data),
            Guidance::AswBallistic => advance_asw_ballistic(world, id, def, data),
            Guidance::BallisticGun => {}
        }
    }
}

fn fly_toward(e: &mut Entity, aim: Vec2, dt: f64) {
    let brg = e.pos.bearing_to(aim);
    e.course_deg = brg;
    // A one-second step must stop at the aim point. Unbounded steps can jump
    // across the terminal radius indefinitely, reversing course every tick.
    let v = (e.speed_kts * KTS_TO_NM_PER_S * dt).min(e.pos.range_to(aim));
    e.pos = e.pos.advanced(brg, v);
}

fn runout(_world: &World, def: &WeaponDef, data: &crate::world::ProjectileData) -> bool {
    let range_time_s = if def.speed_kts > 0.0 {
        def.range_nm / def.speed_kts * 3600.0
    } else {
        300.0
    };
    data.live_s > range_time_s + 30.0
}

fn remove_projectile(world: &mut World, id: EntityId) {
    let now = world.tick;
    if let Some(e) = world.entity_mut(id) {
        e.alive = false;
        e.hp = 0.0;
        e.last_emit_tick = now - 1000; // immediate cleanup
    }
}

fn advance_sea_skimmer(world: &mut World, id: EntityId, def: WeaponDef, mut data: crate::world::ProjectileData) {
    let dt = TICK_SECONDS;
    data.live_s += dt;
    if !data.enabled && data.live_s > 4.0 {
        data.enabled = true;
    }
    let Some(target_id) = data.target else {
        remove_projectile(world, id);
        return;
    };
    let Some(target) = world.entity(target_id).filter(|t| t.alive).cloned() else {
        // Target died mid-flight: fly to last aim point, then self-destruct.
        {
            let Some(e) = world.entity_mut(id) else { return };
            fly_toward(e, data.aim, dt);
            e.projectile = Some(data.clone());
        }
        let at = world.entity(id).map(|e| e.pos.range_to(data.aim)).unwrap_or(f64::MAX);
        if at < 0.2 || runout(world, &def, &data) {
            remove_projectile(world, id);
        }
        return;
    };

    // Decoyed missiles chase the decoy point.
    let aim = match (data.decoyed, data.decoy_point) {
        (true, Some(p)) => p,
        _ => {
            if data.enabled {
                target.pos
            } else {
                data.aim
            }
        }
    };

    {
        let Some(e) = world.entity_mut(id) else { return };
        fly_toward(e, aim, dt);
        e.projectile = Some(data.clone());
    }

    // RTS adaptation: seduction is terminal at the false aim point. It must
    // neither orbit that point until runout nor hit the original ship en route.
    if data.decoyed && world.entity(id).map(|e| e.pos.range_to(aim) < 0.2).unwrap_or(false) {
        let contact = world.find_contact(target.side, id);
        world.emit(EventKind::RulesEvent {
            domain: "weapons".into(), rule: "decoy_expired".into(), side: Some(target.side),
            unit: Some(target_id), contact,
            detail: "Seduced missile reached its false aim point; original target not hit".into(),
        }, Visibility::Side(target.side));
        remove_projectile(world, id);
        return;
    }

    let dist = world.entity(id).map(|e| e.pos.range_to(target.pos)).unwrap_or(f64::MAX);

    // --- defense chain (only vs live surface/base targets) ---
    let defendable = matches!(target.kind, PlatformKind::Ship | PlatformKind::Base);

    if defendable && !data.chaff_checked && dist < 8.0 {
        data.chaff_checked = true;
        let t = world.entity(target_id).unwrap();
        let should_chaff = t.chaff > 0 && t.auto_chaff && t.systems.radar > 0.05;
        if should_chaff {
            let t = world.entity_mut(target_id).unwrap();
            t.chaff -= 1;
            t.chaff_active_s = 30.0;
            let tside = t.side;
            sensors::chaff_breaks_lock(world, target_id);
            world.emit(
                EventKind::ChaffDeployed { side: tside, unit: target_id },
                Visibility::Side(tside),
            );
        }
    }

    if defendable && !data.ciws_checked && dist < 1.8 {
        data.ciws_checked = true;
        // Auto-CIWS: one burst per incoming leaker.
        let ciws_idx = world.entity(target_id).and_then(|t| {
            t.weapons
                .iter()
                .position(|m| {
                    world
                        .db
                        .weapons
                        .get(&m.weapon)
                        .map(|d| d.kind == WeaponKind::Ciws)
                        .unwrap_or(false)
                })
        });
        if let Some(idx) = ciws_idx {
            let (ammo_ok, cd_ok, ciws_pk) = {
                let t = world.entity(target_id).unwrap();
                let m = &t.weapons[idx];
                let d = &world.db.weapons[&m.weapon];
                (m.ammo > 0, m.cooldown_s <= 0.0, d.pk)
            };
            if ammo_ok && cd_ok {
                {
                    let t = world.entity_mut(target_id).unwrap();
                    let m = &mut t.weapons[idx];
                    m.ammo -= 1;
                    m.cooldown_s = 2.5;
                }
                let mut rng = world.scope_rng(id, "ciws");
                if rng.chance(ciws_pk) {
                    let defender = target.side;
                    let contact = world.find_contact(defender, id);
                    let burst_range = dist;
                    remove_projectile(world, id);
                    // Only the defending side knows this intercept succeeded.
                    world.sides[defender.index()].contacts.retain(|_, c| c.about != id);
                    world.emit(
                        EventKind::Intercept { side: defender, contact, intercepted: true },
                        Visibility::Side(defender),
                    );
                    // Three-second rule: intercept fragments may air-burst
                    // over the defender (rules §8.1.5, AAW workstream).
                    crate::rules::aaw::on_intercept(world, target_id, burst_range);
                    return;
                }
            }
        }
    }

    // Chaff seduction window.
    if defendable && !data.decoyed && world.entity(target_id).map(|t| t.chaff_active_s).unwrap_or(0.0) > 0.0
        && dist < 4.0
    {
        let mut rng = world.scope_rng(id, "chaff_seduction");
        if rng.chance(0.45) {
            data.decoyed = true;
            let tpos = world.entity(target_id).map(|t| t.pos).unwrap_or_default();
            data.decoy_point = Some(tpos.advanced(rng.cone_deg(0.0, 180.0), 1.5));
            let defender = target.side;
            let contact = world.find_contact(defender, id);
            world.emit(
                EventKind::DecoyFooled { side: defender, contact },
                Visibility::Side(defender),
            );
        }
    }

    // Terminal impact.
    if defendable && !data.decoyed && dist < 0.25 {
        let chaff_on = world.entity(target_id).map(|t| t.chaff_active_s).unwrap_or(0.0) > 0.0;
        let mut pk = def.pk;
        if chaff_on && !data.decoyed {
            pk *= 0.45;
        }
        let mut rng = world.scope_rng(id, "terminal");
        if rng.chance(pk) {
            damage::apply_hit_from(world, target_id, &def, false, data.launched_by_side);
        }
        remove_projectile(world, id);
        return;
    }

    if dist < 0.25 && !defendable {
        remove_projectile(world, id);
        return;
    }

    if runout(world, &def, &data) {
        remove_projectile(world, id);
        return;
    }
    if let Some(e) = world.entity_mut(id) {
        e.projectile = Some(data);
    }
}

fn advance_homing_air(world: &mut World, id: EntityId, def: WeaponDef, mut data: crate::world::ProjectileData) {
    let dt = TICK_SECONDS;
    data.live_s += dt;
    let Some(target_id) = data.target else {
        remove_projectile(world, id);
        return;
    };
    let Some(target) = world.entity(target_id).filter(|t| t.alive).cloned() else {
        remove_projectile(world, id);
        return;
    };
    {
        let Some(e) = world.entity_mut(id) else { return };
        fly_toward(e, target.pos, dt);
        e.altitude_m = target.altitude_m;
        e.projectile = Some(data.clone());
    }
    let dist = world.entity(id).map(|e| e.pos.range_to(target.pos)).unwrap_or(f64::MAX);
    // Aircrews self-protect: a radar-guided AAM/SAM closing inside six miles
    // triggers the automatic chaff dispenser, exactly as ships do against
    // sea-skimmers. Without this the aircraft `chaff` field was dead weight
    // and every fighter died without countermeasures.
    if target.kind == PlatformKind::Aircraft
        && !def.ir
        && target.chaff > 0
        && target.auto_chaff
        && target.chaff_active_s <= 0.0
        && dist < 6.0
    {
        let tside = target.side;
        if let Some(t) = world.entity_mut(target_id) {
            t.chaff = t.chaff.saturating_sub(1);
            t.chaff_active_s = 30.0;
        }
        world.emit(
            EventKind::ChaffDeployed { side: tside, unit: target_id },
            Visibility::Side(tside),
        );
    }
    if dist < 0.35 {
        // Terminal roll; heavy salvo saturation (3+ in flight) lowers
        // per-missile Pk — two missiles stacking is the standard shot pair.
        let inbound = world
            .entities
            .values()
            .filter(|p| {
                p.alive
                    && p.kind == PlatformKind::Missile
                    && p.id != id
                    && p.projectile
                        .as_ref()
                        .map(|d| d.target == Some(target_id))
                        .unwrap_or(false)
            })
            .count();
        let mut pk = def.pk;
        if inbound >= 3 {
            pk *= 0.7;
        }
        // Chaff only seduces radar seekers, never IR. Re-read the live value:
        // the auto-dispenser may have fired earlier in this same tick.
        let chaff_on = world
            .entity(target_id)
            .map(|t| t.chaff_active_s > 0.0)
            .unwrap_or(false);
        if !def.ir && chaff_on {
            pk *= 0.85;
        }
        let mut rng = world.scope_rng(id, "aam_terminal");
        let roll = rng.next_f64();
        let hit = roll < pk.clamp(0.0, 1.0);
        let defender = data.launched_by_side;
        let contact = world.find_contact(defender, target_id);
        world.emit(EventKind::RulesEvent {
            domain: "aaw".into(), rule: "air_terminal".into(), side: Some(defender),
            unit: Some(data.launched_by), contact,
            detail: format!("Air intercept: Pk={pk:.6}; roll={roll:.6}; hit={hit}"),
        }, Visibility::Side(defender));
        if hit {
            let burst_range = dist;
            // §8.1.1, printed 8-3 / PDF83: a successful attack shoots down
            // the air target. The fragment hook must see the incoming missile
            // before it is retired, to recover the defended ship and geometry.
            crate::rules::aaw::on_intercept(world, target_id, burst_range);
            if target.kind.is_projectile() {
                remove_projectile(world, target_id);
                // Resolve only the firing side's confirmed target track.
                world.sides[defender.index()].contacts.retain(|_, c| c.about != target_id);
            } else {
                damage::sink(world, target_id);
            }
            remove_projectile(world, id);
            world.emit(
                EventKind::Intercept { side: defender, contact, intercepted: true },
                Visibility::Side(defender),
            );
            return;
        }
        remove_projectile(world, id);
        world.emit(
            EventKind::Intercept { side: defender, contact, intercepted: false },
            Visibility::Side(defender),
        );
        return;
    }
    if runout(world, &def, &data) {
        remove_projectile(world, id);
        return;
    }
    if let Some(e) = world.entity_mut(id) {
        e.projectile = Some(data);
    }
}

fn advance_torpedo(world: &mut World, id: EntityId, def: WeaponDef, mut data: crate::world::ProjectileData) {
    let dt = TICK_SECONDS;
    data.live_s += dt;
    let Some(target_id) = data.target else {
        remove_projectile(world, id);
        return;
    };
    let Some(target) = world.entity(target_id).cloned() else {
        remove_projectile(world, id);
        return;
    };

    // Enable the seeker after running `enable_range_nm`.
    if !data.enabled {
        let run_nm = data.live_s * def.speed_kts * KTS_TO_NM_PER_S;
        if run_nm >= def.enable_range_nm {
            data.enabled = true;
        }
    }

    // Decoyed torpedoes swim to the decoy, then re-search.
    if data.decoyed {
        if let Some(p) = data.decoy_point {
            {
                let Some(e) = world.entity_mut(id) else { return };
                fly_toward(e, p, dt);
                e.projectile = Some(data.clone());
            }
            let at_decoy = world.entity(id).map(|e| e.pos.range_to(p)).unwrap_or(f64::MAX);
            if at_decoy < 0.1 {
                data.decoyed = false;
                data.decoy_point = None;
                data.passes = 0; // re-acquire
            }
            if runout(world, &def, &data) {
                remove_projectile(world, id);
                return;
            }
            if let Some(e) = world.entity_mut(id) {
                e.projectile = Some(data);
            }
            return;
        }
    }

    let dist = world.entity(id).map(|e| e.pos.range_to(target.pos)).unwrap_or(f64::MAX);

    if data.enabled && data.passes == 0 {
        // Acquisition: cone ±40°, seeker range by guidance.
        let cone = 60.0; // rules: ±60° acquisition cone
        let seeker = if matches!(def.guidance, Guidance::WakeHome) { 2.0 } else { 3.5 };
        let brg = world.entity(id).map(|e| e.pos.bearing_to(target.pos)).unwrap_or(0.0);
        let off = crate::geom::angle_diff(world.entity(id).map(|e| e.course_deg).unwrap_or(0.0), brg);
        if dist < seeker && off.abs() < cone {
            data.passes = 1; // locked
        }
    }

    if data.passes == 1 {
        // Evasion: target may drop a decoy once inside 1.2 nm. Seeker
        // generation (rules::torpedoes) modulates the seduction chance.
        if dist < 1.2 && target.decoys > 0 {
            let mut rng = world.scope_rng(id, "torp_decoy");
            let seduce_p = crate::rules::torpedoes::decoy_resistance(&def) * 0.55;
            if rng.chance(seduce_p) {
                let t = world.entity_mut(target_id).unwrap();
                t.decoys -= 1;
                let tside = t.side;
                data.decoyed = true;
                let tpos = target.pos;
                data.decoy_point = Some(tpos.advanced(rng.cone_deg(target.course_deg, 170.0), 1.2));
                let contact = world.find_contact(tside, id);
                world.emit(
                    EventKind::DecoyFooled { side: tside, contact },
                    Visibility::Side(tside),
                );
            }
        }
        {
            let Some(e) = world.entity_mut(id) else { return };
            fly_toward(e, target.pos, dt);
            e.projectile = Some(data.clone());
        }
        let dist2 = world.entity(id).map(|e| e.pos.range_to(target.pos)).unwrap_or(f64::MAX);
        if dist2 < 0.12 {
            let mut pk = def.pk;
            if target.speed_kts > 25.0 {
                pk *= 0.85;
            }
            let mut rng = world.scope_rng(id, "torp_terminal");
            let hit = rng.chance(pk);
            if hit {
                damage::apply_hit_from(world, target_id, &def, true, data.launched_by_side);
            } else {
                data.passes = 0; // miss, re-attack if fuel remains
            }
            if hit {
                remove_projectile(world, id);
                return;
            }
        }
    } else {
        // Run-out toward the aim point while seeker is cold.
        let aim = if data.enabled { target.pos } else { data.aim };
        {
            let Some(e) = world.entity_mut(id) else { return };
            fly_toward(e, aim, dt);
            e.projectile = Some(data.clone());
        }
    }

    if runout(world, &def, &data) {
        remove_projectile(world, id);
        return;
    }
    if let Some(e) = world.entity_mut(id) {
        e.projectile = Some(data);
    }
}

fn advance_asw_ballistic(world: &mut World, id: EntityId, def: WeaponDef, mut data: crate::world::ProjectileData) {
    let dt = TICK_SECONDS;
    data.live_s += dt;
    {
        let Some(e) = world.entity_mut(id) else { return };
        fly_toward(e, data.aim, dt);
        e.projectile = Some(data.clone());
        e.speed_kts = 300.0;
    }
    let at_aim = world.entity(id).map(|e| e.pos.range_to(data.aim)).unwrap_or(f64::MAX);
    if at_aim < 0.15 {
        if let Some(tid) = data.target {
            let still_close = world
                .entity(tid)
                .map(|t| t.pos.range_to(data.aim) < 0.8 && t.alive)
                .unwrap_or(false);
            if still_close {
                let mut rng = world.scope_rng(id, "asw_rocket");
                if rng.chance(def.pk) {
                    damage::apply_hit_from(world, tid, &def, true, data.launched_by_side);
                }
            }
        }
        remove_projectile(world, id);
        return;
    }
    if data.live_s > 120.0 {
        remove_projectile(world, id);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::rng::Rng;
    use crate::scenario::{load_scenario, Scenario, UnitSpawn};
    use crate::types::DayNight;
    use crate::world::Contact;

    fn scenario(units: Vec<UnitSpawn>) -> Scenario {
        Scenario {
            id: "weapons-test".into(),
            name: "Weapons test".into(),
            description: String::new(),
            context: Default::default(),
            duration_s: 40_000,
            weather: crate::world::Weather {
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

    #[allow(clippy::too_many_arguments)]
    fn unit(
        side: Side,
        class: &str,
        name: &str,
        x: f64,
        y: f64,
        course_deg: f64,
        altitude_m: Option<f64>,
        fire_on: bool,
    ) -> UnitSpawn {
        UnitSpawn {
            side,
            class: class.into(),
            name: Some(name.into()),
            pos: Vec2::new(x, y),
            course_deg,
            speed_kts: 0.0,
            depth_m: None,
            altitude_m,
            waypoints: None,
            follow: None,
            sonar_active: false,
            fire_on,
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
        stale: bool,
        sensors: Vec<&str>,
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
                bearing_only: false,
                quality,
                last_update_tick: 0,
                first_tick: 0,
                stale,
                sensors: sensors.into_iter().map(str::to_string).collect(),
                offset: Vec2::zero(),
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

    fn gun_hits(w: &World) -> u32 {
        w.events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::GunAction { hits, .. } => Some(*hits as u32),
                _ => None,
            })
            .sum()
    }

    fn ammo_of(w: &World, unit: EntityId, weapon: &str) -> u16 {
        w.entity(unit)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == weapon)
            .unwrap()
            .ammo
    }

    fn terminal_world() -> (World, EntityId, EntityId) {
        let mut w = world(vec![
            unit(Side::Blue, "ddg_aegis", "DEFENDER", 0.0, 0.0, 0.0, None, false),
            unit(Side::Red, "fighter", "ATTACKER", 10.0, 0.0, 0.0, Some(5000.0), false),
        ]);
        let blue = id_of(&w, "DEFENDER");
        let red = id_of(&w, "ATTACKER");
        // Terminal-roll tests pin the raw Pk; disarm the new automatic aircraft
        // chaff so it does not perturb the deterministic roll (the dedicated
        // auto-chaff test arms it explicitly).
        {
            let attacker = w.entity_mut(red).unwrap();
            attacker.chaff = 0;
            attacker.auto_chaff = false;
        }
        (w, blue, red)
    }

    #[test]
    fn terminal_sam_success_destroys_incoming_and_reports_to_shooter() {
        let (mut w, blue, red) = terminal_world();
        let asm = w.db.weapons.values().find(|d| d.guidance == Guidance::SeaSkimmer).unwrap().clone();
        let incoming = launch(&mut w, red, &asm, Some(blue), None).unwrap();
        w.entity_mut(incoming).unwrap().pos = Vec2::new(0.5, 0.0);
        let cid = seed_contact(&mut w, Side::Blue, incoming, PlatformKind::Missile,
            Vec2::new(0.5, 0.0), 1.0, false, vec!["radar_air_long"]);
        let mut sam = w.db.weapons["sam_long"].clone();
        sam.pk = 1.0;
        sam.speed_kts = 720.0;
        let interceptor = launch(&mut w, blue, &sam, Some(incoming), None).unwrap();
        let data = w.entity(interceptor).unwrap().projectile.clone().unwrap();
        advance_homing_air(&mut w, interceptor, sam, data);
        assert!(!w.entity(incoming).unwrap().alive, "successful SAM must destroy its target");
        assert!(!w.entity(interceptor).unwrap().alive);
        assert!(w.events.iter().any(|e| matches!(e.kind,
            EventKind::Intercept { side: Side::Blue, contact: Some(c), intercepted: true } if c == cid)));
        assert!(!w.sides[Side::Blue.index()].contacts.contains_key(&cid),
            "the firing side's confirmed intercept must resolve its own track");
        assert_eq!(w.entity(blue).unwrap().hp, w.entity(blue).unwrap().hp_max,
            "fragment effects cause no direct DP");
    }

    #[test]
    fn terminal_air_success_shoots_down_aircraft() {
        for weapon in ["sam_long", "aam_ir"] {
            let (mut w, blue, red) = terminal_world();
            let mut def = w.db.weapons[weapon].clone();
            def.pk = 1.0;
            def.speed_kts = 360.0;
            let missile = launch(&mut w, blue, &def, Some(red), None).unwrap();
            w.entity_mut(missile).unwrap().pos = Vec2::new(9.8, 0.0);
            let data = w.entity(missile).unwrap().projectile.clone().unwrap();
            advance_homing_air(&mut w, missile, def, data);
            assert!(!w.entity(red).unwrap().alive, "{weapon} success must shoot down aircraft");
            assert_eq!(w.entity(red).unwrap().hp, 0.0);
            assert_eq!(w.events.iter().filter(|e| matches!(e.kind,
                EventKind::AirShotDown { unit, .. } if unit == red)).count(), 1);
        }
    }

    #[test]
    fn terminal_air_miss_leaves_target_alive() {
        let (mut w, blue, red) = terminal_world();
        let mut def = w.db.weapons["aam_radar"].clone();
        def.pk = 0.0;
        def.speed_kts = 360.0;
        let missile = launch(&mut w, blue, &def, Some(red), None).unwrap();
        w.entity_mut(missile).unwrap().pos = Vec2::new(9.8, 0.0);
        let data = w.entity(missile).unwrap().projectile.clone().unwrap();
        advance_homing_air(&mut w, missile, def, data);
        assert!(w.entity(red).unwrap().alive);
        assert!(!w.entity(missile).unwrap().alive);
        assert!(w.events.iter().any(|e| matches!(e.kind,
            EventKind::Intercept { side: Side::Blue, intercepted: false, .. })));
    }

    #[test]
    fn terminal_fast_missile_cannot_step_over_target_and_oscillate() {
        let (mut w, blue, red) = terminal_world();
        let mut def = w.db.weapons["sam_long"].clone();
        def.pk = 1.0;
        def.speed_kts = 7200.0;
        let missile = launch(&mut w, blue, &def, Some(red), None).unwrap();
        w.entity_mut(missile).unwrap().pos = Vec2::new(9.0, 0.0);
        let data = w.entity(missile).unwrap().projectile.clone().unwrap();
        advance_homing_air(&mut w, missile, def, data);
        assert!(!w.entity(missile).unwrap().alive, "must resolve on the crossing tick");
        assert!(!w.entity(red).unwrap().alive);
    }

    #[test]
    fn terminal_decoy_arrival_expires_without_ship_damage() {
        let (mut w, blue, red) = terminal_world();
        let mut def = w.db.weapons.values().find(|d| d.guidance == Guidance::SeaSkimmer).unwrap().clone();
        def.pk = 1.0;
        let missile = launch(&mut w, red, &def, Some(blue), None).unwrap();
        let e = w.entity_mut(missile).unwrap();
        e.pos = Vec2::new(0.0, 1.55);
        let mut data = e.projectile.clone().unwrap();
        data.decoyed = true;
        data.decoy_point = Some(Vec2::new(0.0, 1.5));
        data.chaff_checked = true;
        data.ciws_checked = true;
        advance_sea_skimmer(&mut w, missile, def, data);
        assert!(!w.entity(missile).unwrap().alive, "must not loiter at a decoy point");
        assert_eq!(w.entity(blue).unwrap().hp, w.entity(blue).unwrap().hp_max);
        assert!(!w.events.iter().any(|e| matches!(e.kind, EventKind::Impact { .. })));
    }

    #[test]
    fn terminal_ciws_success_clears_only_the_defenders_confirmed_track() {
        let (mut w, blue, red) = terminal_world();
        w.db.weapons.get_mut("ciws_30mm").unwrap().pk = 1.0;
        let def = w.db.weapons["asm_air"].clone();
        let missile = launch(&mut w, red, &def, Some(blue), None).unwrap();
        w.entity_mut(missile).unwrap().pos = Vec2::new(0.0, 0.8);
        let a = seed_contact(&mut w, Side::Blue, missile, PlatformKind::Missile,
            Vec2::new(0.0, 0.8), 1.0, false, vec!["visual"]);
        let b = seed_contact(&mut w, Side::Red, missile, PlatformKind::Missile,
            Vec2::new(0.0, 0.8), 1.0, false, vec!["visual"]);
        let mut data = w.entity(missile).unwrap().projectile.clone().unwrap();
        data.enabled = true;
        data.chaff_checked = true;
        advance_sea_skimmer(&mut w, missile, def, data);
        assert!(!w.entity(missile).unwrap().alive);
        assert!(!w.sides[Side::Blue.index()].contacts.contains_key(&a));
        assert!(w.sides[Side::Red.index()].contacts.contains_key(&b));
    }

    #[test]
    fn terminal_surface_hit_and_miss_resolve_once_with_real_damage() {
        for pk in [0.0, 1.0] {
            let (mut w, blue, red) = terminal_world();
            let mut def = w.db.weapons["asm_air"].clone();
            def.pk = pk;
            w.db.weapons.insert(def.id.clone(), def.clone());
            let missile = launch(&mut w, red, &def, Some(blue), None).unwrap();
            let e = w.entity_mut(missile).unwrap();
            e.pos = Vec2::new(0.0, 0.1);
            let mut data = e.projectile.clone().unwrap();
            data.enabled = true;
            data.chaff_checked = true;
            data.ciws_checked = true;
            advance_sea_skimmer(&mut w, missile, def, data);
            advance_projectiles(&mut w);
            assert!(!w.entity(missile).unwrap().alive);
            assert_eq!(w.events.iter().filter(|e| matches!(e.kind,
                EventKind::Impact { unit, .. } if unit == blue)).count(), if pk == 1.0 { 1 } else { 0 });
            assert_eq!(w.entity(blue).unwrap().hp < w.entity(blue).unwrap().hp_max, pk == 1.0);
        }
    }

    #[test]
    fn terminal_dead_projectile_in_iteration_snapshot_cannot_hit() {
        let (mut w, blue, red) = terminal_world();
        let mut def = w.db.weapons.values().find(|d| d.guidance == Guidance::SeaSkimmer).unwrap().clone();
        def.pk = 1.0;
        w.db.weapons.insert(def.id.clone(), def.clone());
        let mut sam = w.db.weapons["sam_long"].clone();
        sam.pk = 1.0;
        sam.speed_kts = 360.0;
        w.db.weapons.insert(sam.id.clone(), sam.clone());
        let a = launch(&mut w, blue, &sam, Some(red), None).unwrap();
        let b = launch(&mut w, red, &def, Some(blue), None).unwrap();
        // Both ids are collected alive; A intercepts B before B's update in that same tick.
        w.entity_mut(a).unwrap().pos = Vec2::new(0.0, 0.35);
        w.entity_mut(a).unwrap().projectile.as_mut().unwrap().target = Some(b);
        w.entity_mut(b).unwrap().pos = Vec2::new(0.0, 0.1);
        let data = w.entity_mut(b).unwrap().projectile.as_mut().unwrap();
        data.chaff_checked = true;
        data.ciws_checked = true;
        advance_projectiles(&mut w);
        assert!(!w.entity(b).unwrap().alive);
        assert!(!w.events.iter().any(|e| matches!(e.kind, EventKind::Impact { .. })));
    }

    #[test]
    fn bol_launch_allows_poor_air_track_for_sam() {
        let mut w = world(vec![
            unit(Side::Blue, "ddg_aegis", "BLUE DDG", 0.0, 0.0, 0.0, None, false),
            unit(Side::Red, "fighter", "RED FIGHTER", 30.0, 0.0, 90.0, Some(5000.0), false),
        ]);
        let ddg = id_of(&w, "BLUE DDG");
        let red = id_of(&w, "RED FIGHTER");
        let cid = seed_contact(
            &mut w,
            Side::Blue,
            red,
            PlatformKind::Aircraft,
            Vec2::new(30.0, 0.0),
            0.20,
            false,
            vec!["radar_air_long"],
        );

        assert!(manual_fire(&mut w, Side::Blue, ddg, cid, "sam_long", 1));
        assert_eq!(rules_event_count(&w, "bol_launch"), 1);
        assert_eq!(ammo_of(&w, ddg, "sam_long"), 47);
    }

    #[test]
    fn stale_air_track_can_still_bol() {
        let mut w = world(vec![
            unit(Side::Blue, "ddg_aegis", "BLUE DDG", 0.0, 0.0, 0.0, None, false),
            unit(Side::Red, "fighter", "RED FIGHTER", 5.0, 0.0, 90.0, Some(5000.0), false),
        ]);
        let ddg = id_of(&w, "BLUE DDG");
        let red = id_of(&w, "RED FIGHTER");
        let cid = seed_contact(
            &mut w,
            Side::Blue,
            red,
            PlatformKind::Aircraft,
            Vec2::new(5.0, 0.0),
            0.10,
            true,
            vec![],
        );

        assert!(manual_fire(&mut w, Side::Blue, ddg, cid, "sam_point", 1));
        assert_eq!(rules_event_count(&w, "bol_launch"), 1);
    }

    #[test]
    fn poor_track_blocks_non_bol_and_surface_targets() {
        let mut w = world(vec![
            unit(Side::Blue, "ddg_aegis", "BLUE DDG", 0.0, 0.0, 0.0, None, false),
            unit(Side::Red, "fighter", "RED FIGHTER", 5.0, 0.0, 90.0, Some(5000.0), false),
            unit(Side::Red, "mgb", "RED MGB", 5.0, 5.0, 90.0, None, false),
        ]);
        let ddg = id_of(&w, "BLUE DDG");
        let fighter = id_of(&w, "RED FIGHTER");
        let mgb = id_of(&w, "RED MGB");
        let fcid = seed_contact(
            &mut w,
            Side::Blue,
            fighter,
            PlatformKind::Aircraft,
            Vec2::new(5.0, 0.0),
            0.20,
            false,
            vec!["radar_air_long"],
        );
        let scid = seed_contact(
            &mut w,
            Side::Blue,
            mgb,
            PlatformKind::Ship,
            Vec2::new(5.0, 5.0),
            0.20,
            false,
            vec!["radar_surf_med"],
        );

        // A poor track does not relax the gate for guns...
        assert!(!manual_fire(&mut w, Side::Blue, ddg, fcid, "gun_5in", 1));
        // ...nor does BOL apply to a surface target.
        assert!(!manual_fire(&mut w, Side::Blue, ddg, scid, "sam_long", 1));
        assert_eq!(rules_event_count(&w, "bol_launch"), 0);
        assert_eq!(ammo_of(&w, ddg, "sam_long"), 48);
        assert_eq!(ammo_of(&w, ddg, "gun_5in"), 40);
    }

    #[test]
    fn sam_fire_near_friendly_aircraft_can_misfire() {
        let mut w = world(vec![
            unit(Side::Blue, "ddg_aegis", "BLUE DDG", 0.0, 0.0, 0.0, None, false),
            unit(Side::Red, "fighter", "RED FIGHTER", 30.0, 0.0, 90.0, Some(5000.0), false),
            unit(Side::Blue, "fighter", "BLUE FIGHTER", 30.5, 0.5, 270.0, Some(5000.0), false),
        ]);
        let ddg = id_of(&w, "BLUE DDG");
        let red = id_of(&w, "RED FIGHTER");
        let friend = id_of(&w, "BLUE FIGHTER");
        let cid = seed_contact(
            &mut w,
            Side::Blue,
            red,
            PlatformKind::Aircraft,
            Vec2::new(30.0, 0.0),
            0.90,
            false,
            vec!["radar_air_long"],
        );
        w.tick = 1;

        // The per-shooter stream is order-independent: find a seed whose 5%
        // roll succeeds, then let fire_control_scan consume that same roll.
        let mut chosen: Option<u64> = None;
        for seed in 0..100_000u64 {
            w.rng = Rng::new(seed);
            if w.scope_rng(ddg, "friendly_fire").chance(0.05) {
                chosen = Some(seed);
                break;
            }
        }
        let seed = chosen.expect("no seed produced a 5% friendly-fire roll");
        w.rng = Rng::new(seed);
        w.entity_mut(ddg).unwrap().assigned = Some(cid);

        fire_control_scan(&mut w);

        let ev = w
            .events
            .iter()
            .find(|e| {
                matches!(
                    &e.kind,
                    EventKind::RulesEvent { domain, rule, .. }
                        if domain == "firectl" && rule == "friendly_fire"
                )
            })
            .expect("friendly-fire event");
        match &ev.kind {
            EventKind::RulesEvent { unit, contact, side, .. } => {
                assert_eq!(*unit, Some(friend));
                assert_eq!(*contact, Some(cid));
                assert_eq!(*side, Some(Side::Blue));
            }
            _ => unreachable!(),
        }
        assert!(
            w.entity(friend).unwrap().hp < w.entity(friend).unwrap().hp_max,
            "nearest friendly aircraft must take half damage"
        );
    }

    #[test]
    fn visual_spotting_improves_gun_pk() {
        // 2 nm, 90° crossing target: base Pk = 0.75 × 0.30 × 0.80 = 0.18;
        // with the visual solution Pk = 0.207. Pick a roll in between.
        let mut probe = world(vec![
            unit(Side::Blue, "ddg_aegis", "BLUE DDG", 0.0, 0.0, 0.0, None, false),
            unit(Side::Red, "mgb", "RED MGB", 2.0, 0.0, 90.0, None, false),
        ]);
        let ddg = id_of(&probe, "BLUE DDG");
        let mut chosen = None;
        for seed in 0..100_000u64 {
            probe.rng = Rng::new(seed);
            probe.tick = 0;
            let roll = probe.scope_rng(ddg, "gun").next_f64();
            if roll >= 0.18 && roll < 0.18 * 1.15 {
                chosen = Some(seed);
                break;
            }
        }
        let seed = chosen.expect("no roll between the two Pk values");

        let run = |sensors: Vec<&str>| {
            let mut w = world(vec![
                unit(Side::Blue, "ddg_aegis", "BLUE DDG", 0.0, 0.0, 0.0, None, false),
                unit(Side::Red, "mgb", "RED MGB", 2.0, 0.0, 90.0, None, false),
            ]);
            let ddg = id_of(&w, "BLUE DDG");
            let red = id_of(&w, "RED MGB");
            let cid = seed_contact(
                &mut w,
                Side::Blue,
                red,
                PlatformKind::Ship,
                Vec2::new(2.0, 0.0),
                0.90,
                false,
                sensors,
            );
            w.rng = Rng::new(seed);
            w.tick = 0;
            assert!(manual_fire(&mut w, Side::Blue, ddg, cid, "gun_5in", 1));
            gun_hits(&w)
        };

        assert_eq!(run(vec!["radar_surf_med"]), 0, "base Pk run must miss");
        assert_eq!(run(vec!["visual"]), 1, "visual solution must hit");
    }
    #[test]
    fn aircraft_auto_chaff_fires_when_a_radar_missile_closes() {
        let (mut w, blue, red) = terminal_world();
        {
            let t = w.entity_mut(red).unwrap();
            t.chaff = 2;
            t.auto_chaff = true;
        }
        let mut def = w.db.weapons["aam_radar"].clone();
        def.pk = 0.0;
        def.speed_kts = 360.0;
        let missile = launch(&mut w, blue, &def, Some(red), None).unwrap();
        w.entity_mut(missile).unwrap().pos = Vec2::new(5.0, 0.0);
        let data = w.entity(missile).unwrap().projectile.clone().unwrap();
        advance_homing_air(&mut w, missile, def, data);
        assert_eq!(w.entity(red).unwrap().chaff, 1, "one dispenser round used");
        assert!(w.events.iter().any(|e| matches!(
            e.kind, EventKind::ChaffDeployed { unit, .. } if unit == red)));
        assert!(w.entity(red).unwrap().alive, "PK 0 must not kill the aircraft");
    }

}

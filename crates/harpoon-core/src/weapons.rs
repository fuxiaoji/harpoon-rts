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

        for (idx, def, mounts_n, ammo, cooldown) in mounts {
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
            };
            if !engaged {
                continue;
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
            damage::apply_hit(world, target_id, def, false);
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
    let Some(eid) = resolve_contact(world, side, contact) else { return false };
    // Fire-control solution gate: manual shots need a usable track
    // (rules §6.3.1 — a firing solution requires a quality contact).
    let solution_quality = world.contact(side, contact).map(|c| c.quality).unwrap_or(0.0);
    if solution_quality < 0.3 {
        return false;
    }
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
    let def = match world.db.weapons.get(weapon_id) {
        Some(d) => d.clone(),
        None => return false,
    };
    let dist = world
        .entity(unit)
        .zip(world.entity(eid))
        .map(|(s, t)| s.pos.range_to(t.pos))
        .unwrap_or(f64::MAX);
    if dist > def.range_nm || dist < def.min_range_nm {
        return false;
    }
    let shooter_pos = world.entity(unit).map(|e| e.pos).unwrap_or_default();
    let _ = shooter_pos;
    let n = rounds.min((ammo as u8).min(def.salvo.max(1) * 2));
    let mut fired = 0;
    for _ in 0..n {
        if def.kind.is_gun_like() {
            // Guns adjudicate per salvo; manual fire = one salvo per round count.
            let cid = contact;
            gun_salvo(world, unit, cid, eid, &def, 1);
            fired += 1;
            if fired >= rounds {
                break;
            }
        } else if launch(world, unit, &def, Some(eid), None).is_some() {
            fired += 1;
        }
    }
    let Some(e) = world.entity_mut(unit) else { return false };
    let m = &mut e.weapons[idx];
    m.ammo = m.ammo.saturating_sub(fired as u16);
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
    let v = e.speed_kts * KTS_TO_NM_PER_S * dt;
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
    let Some(target) = world.entity(target_id).cloned() else {
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

    // Decoysed missiles chase the decoy point.
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
    if defendable && dist < 0.25 {
        let chaff_on = world.entity(target_id).map(|t| t.chaff_active_s).unwrap_or(0.0) > 0.0;
        let mut pk = def.pk;
        if chaff_on && !data.decoyed {
            pk *= 0.45;
        }
        let mut rng = world.scope_rng(id, "terminal");
        if rng.chance(pk) {
            damage::apply_hit(world, target_id, &def, false);
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
    let Some(target) = world.entity(target_id).cloned() else {
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
    if dist < 0.35 {
        // Terminal roll; salvo saturation lowers per-missile Pk.
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
        if inbound >= 2 {
            pk *= 0.7;
        }
        // Chaff only seduces radar seekers, never IR.
        if !def.ir && target.chaff_active_s > 0.0 {
            pk *= 0.85;
        }
        let mut rng = world.scope_rng(id, "aam_terminal");
        let hit = rng.chance(pk);
        let defender = target.side;
        let contact = world.find_contact(defender, id);
        if hit {
            let burst_range = dist;
            remove_projectile(world, id);
            world.emit(
                EventKind::Intercept { side: defender, contact, intercepted: true },
                Visibility::Side(defender),
            );
            crate::rules::aaw::on_intercept(world, target_id, burst_range);
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
        // Evasion: target may drop a decoy once inside 1.2 nm.
        if dist < 1.2 && target.decoys > 0 {
            let mut rng = world.scope_rng(id, "torp_decoy");
            if rng.chance(0.55) {
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
                damage::apply_hit(world, target_id, &def, true);
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
                    damage::apply_hit(world, tid, &def, true);
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

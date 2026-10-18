//! Air operations: deck cycles (launch/recover/rearm), CAP stations, strike
//! releases, ASW dipping, endurance. Deck timelines replace the tabletop's
//! phased air-turn bookkeeping with continuous timers.

use crate::commands::AirMission;
use crate::db::{Database, WeaponKind};
use crate::events::{EventKind, Visibility};
use crate::geom::Vec2;
use crate::types::{ContactId, EntityId, PlatformKind, Side};
use crate::world::{DeckState, Entity, MissionState, World, TICK_SECONDS};

pub fn update_air_ops(world: &mut World) {
    cycle_decks(world);
    fly_missions(world);
}

fn cycle_decks(world: &mut World) {
    let carrier_ids: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.alive && e.deck.is_some())
        .map(|e| e.id)
        .collect();
    for cid in carrier_ids {
        let now = world.tick;
        let mut ready_again: Vec<(String, u8)> = Vec::new();
        {
            let Some(deck) = world.entity_mut(cid).unwrap().deck.as_mut() else { continue };
            deck.cycling.retain(|(class, count, ready_at)| {
                if *ready_at <= now {
                    ready_again.push((class.clone(), *count));
                    false
                } else {
                    true
                }
            });
        }
        let Some(deck) = world.entity_mut(cid).unwrap().deck.as_mut() else { continue };
        for (class, count) in ready_again {
            add_ready(deck, &class, count);
        }
    }
}

fn add_ready(deck: &mut DeckState, class: &str, count: u8) {
    if let Some(slot) = deck.ready.iter_mut().find(|(c, _)| c == class) {
        slot.1 = slot.1.saturating_add(count);
    } else {
        deck.ready.push((class.to_string(), count));
    }
}

fn ready_count(deck: &DeckState, class: &str) -> u8 {
    deck.ready
        .iter()
        .find(|(c, _)| c == class)
        .map(|(_, n)| *n)
        .unwrap_or(0)
}

/// Launch aircraft from a deck. Returns Err(reason) if impossible.
pub fn launch_aircraft(
    world: &mut World,
    carrier_id: EntityId,
    class: &str,
    count: u8,
    mission: &AirMission,
) -> Result<(), String> {
    let Some(carrier) = world.entity(carrier_id).cloned() else {
        return Err("no such carrier".into());
    };
    if !carrier.alive {
        return Err("carrier destroyed".into());
    }
    let Some(deck) = carrier.deck.clone() else {
        return Err("unit has no flight deck".into());
    };
    let available = ready_count(&deck, class);
    if available < count {
        return Err(format!("only {available} {class} ready"));
    }
    let Some(ac_class) = world.db.platforms.get(class).cloned() else {
        return Err(format!("unknown aircraft class {class}"));
    };
    if ac_class.kind != PlatformKind::Aircraft {
        return Err(format!("{class} is not an aircraft"));
    }

    let mut bearing = carrier.course_deg;
    for i in 0..count {
        let eid = world.allocate_id();
        let pos = carrier.pos.advanced(bearing, 0.3 + i as f64 * 0.15);
        bearing += 15.0;
        let e = Entity {
            id: eid,
            side: carrier.side,
            class: class.to_string(),
            name: format!("{} {}", carrier.side.name().to_uppercase(), ac_class.name),
            kind: PlatformKind::Aircraft,
            pos,
            course_deg: carrier.course_deg,
            speed_kts: ac_class.max_speed_kts * 0.5,
            ordered_course_deg: carrier.course_deg,
            ordered_speed_kts: cruise_of(&ac_class),
            waypoints: Default::default(),
            altitude_m: 6000.0,
            ordered_altitude_m: 8000.0,
            depth_m: 0.0,
            ordered_depth_m: 0.0,
            hp: ac_class.hp,
            hp_max: ac_class.hp,
            alive: true,
            systems: Default::default(),
            fire_on: true,
            assigned: None,
            weapons: ac_class
                .weapons
                .iter()
                .map(|w| crate::world::WeaponMount {
                    weapon: w.weapon.clone(),
                    mounts: w.mounts,
                    ammo: w.ammo * w.mounts as u16,
                    cooldown_s: 0.0,
                })
                .collect(),
            sensors: ac_class
                .sensors
                .iter()
                .map(|s| crate::world::SensorInstance { sensor: s.clone(), active: false })
                .collect(),
            chaff: ac_class.chaff,
            decoys: 0,
            auto_chaff: false,
            chaff_active_s: 0.0,
            endurance_s: Some(ac_class.endurance_s),
            burn_s: 0.0,
            deck: None,
            mission: Some(MissionState::from_air_mission(mission)),
            station: None,
            last_emit_tick: world.tick,
            projectile: None,
        };
        world.entities.insert(eid, e);
    }

    let deck_mut = world.entity_mut(carrier_id).unwrap().deck.as_mut().unwrap();
    if let Some(slot) = deck_mut.ready.iter_mut().find(|(c, _)| c == class) {
        slot.1 -= count;
    }
    world.emit(
        EventKind::AirLaunch {
            side: carrier.side,
            carrier: carrier_id,
            class: ac_class.name.clone(),
            count,
            mission: mission.tag().to_string(),
        },
        Visibility::Side(carrier.side),
    );
    Ok(())
}

fn cruise_of(class: &crate::db::PlatformClass) -> f64 {
    if class.cruise_speed_kts > 0.0 {
        class.cruise_speed_kts
    } else {
        class.max_speed_kts * 0.7
    }
}

fn cruise_of_entity(e: &Entity, db: &Database) -> f64 {
    db.platforms.get(&e.class).map(cruise_of).unwrap_or(300.0)
}

fn fly_missions(world: &mut World) {
    let air_ids: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.alive && e.kind == PlatformKind::Aircraft)
        .map(|e| e.id)
        .collect();

    for id in air_ids {
        let Some(e) = world.entities.get(&id) else { continue };
        if !e.alive {
            continue;
        }

        // Endurance & ditching.
        let endurance_left = e.endurance_s.unwrap_or(f64::INFINITY) - TICK_SECONDS;
        let side = e.side;
        let name = e.name.clone();
        let low_fuel = endurance_left < 900.0;
        let out_of_fuel = endurance_left <= 0.0;
        let now = world.tick;
        {
            let Some(e) = world.entity_mut(id) else { continue };
            e.endurance_s = Some(endurance_left);
            if out_of_fuel {
                e.alive = false;
                e.hp = 0.0;
                e.last_emit_tick = now - 1000;
            } else if low_fuel && !matches!(e.mission, Some(MissionState::Rtb)) {
                e.mission = Some(MissionState::Rtb);
            }
        }
        if out_of_fuel {
            world.emit(
                EventKind::Ditched { side, unit: id, unit_name: name },
                Visibility::Side(side),
            );
            continue;
        }

        let mission = world.entities.get(&id).and_then(|e| e.mission.clone());
        match mission {
            Some(MissionState::Cap { station, radius_nm }) => {
                cap_station(world, id, station, radius_nm);
            }
            Some(MissionState::Recon { point }) => {
                cap_station(world, id, point, 5.0);
            }
            Some(MissionState::Asw { area, radius_nm }) => {
                asw_station(world, id, area, radius_nm);
            }
            Some(MissionState::Strike { contact, released }) => {
                strike_run(world, id, contact, released);
            }
            Some(MissionState::Rtb) => {
                try_recover(world, id);
            }
            None => {}
        }
    }
}

/// Orbit a station: fly there, then fly a gentle right-hand circle.
fn cap_station(world: &mut World, id: EntityId, station: Vec2, radius_nm: f64) {
    let (dist, cruise) = {
        let e = world.entity(id).unwrap();
        (e.pos.range_to(station), cruise_of_entity(e, &world.db))
    };
    let Some(e) = world.entity_mut(id) else { return };
    if dist > radius_nm * 0.8 {
        e.ordered_course_deg = e.pos.bearing_to(station);
        e.ordered_speed_kts = cruise;
    } else {
        e.ordered_course_deg = crate::geom::normalize_deg(e.course_deg + 2.5 * TICK_SECONDS);
        e.ordered_speed_kts = cruise * 0.7;
    }
}

/// Helo ASW: transit to area, hover, run the dipping sonar; the weapons loop
/// drops torpedoes on tracked contacts.
fn asw_station(world: &mut World, id: EntityId, area: Vec2, radius_nm: f64) {
    let (dist, cruise, is_dipping_helo) = {
        let e = world.entity(id).unwrap();
        (
            e.pos.range_to(area),
            cruise_of_entity(e, &world.db),
            e.class_def(&world.db).map(|c| c.dipping_sonar).unwrap_or(false),
        )
    };
    let hovering = dist < radius_nm;
    let Some(e) = world.entity_mut(id) else { return };
    if hovering {
        e.ordered_speed_kts = 0.0;
        e.speed_kts = 0.0;
        e.altitude_m = 30.0;
        e.ordered_altitude_m = 30.0;
    } else {
        e.ordered_course_deg = e.pos.bearing_to(area);
        e.ordered_speed_kts = cruise;
        e.ordered_altitude_m = 500.0;
    }
    if is_dipping_helo {
        for s in e.sensors.iter_mut() {
            if s.sensor.starts_with("sonar_dipping") {
                s.active = hovering;
            }
        }
    }
}

/// Strike: fly at the contact, ripple standoff weapons at envelope, RTB.
fn strike_run(world: &mut World, id: EntityId, contact: ContactId, released: bool) {
    let side = world.entity(id).map(|e| e.side).unwrap_or(Side::Blue);
    let Some(target_id) = crate::weapons::resolve_contact(world, side, contact) else {
        let Some(e) = world.entity_mut(id) else { return };
        e.mission = Some(MissionState::Rtb);
        return;
    };
    if released {
        try_recover(world, id);
        return;
    }
    let (apos, tpos, best_range, cruise) = {
        let a = world.entity(id).unwrap();
        let t = world.entity(target_id).unwrap();
        let best = a
            .weapons
            .iter()
            .filter_map(|m| world.db.weapons.get(&m.weapon))
            .filter(|d| matches!(d.kind, WeaponKind::Ascm))
            .map(|d| d.range_nm)
            .fold(0.0f64, f64::max);
        (a.pos, t.pos, best, cruise_of_entity(a, &world.db))
    };
    let dist = apos.range_to(tpos);
    if best_range <= 0.0 || dist > best_range * 0.85 {
        let Some(e) = world.entity_mut(id) else { return };
        e.ordered_course_deg = e.pos.bearing_to(tpos);
        e.ordered_speed_kts = cruise;
        return;
    }
    // Ripple all anti-ship mounts.
    let launch_plan: Vec<(crate::db::WeaponDef, u16)> = world
        .entity(id)
        .map(|e| {
            e.weapons
                .iter()
                .filter(|m| {
                    world
                        .db
                        .weapons
                        .get(&m.weapon)
                        .map(|d| matches!(d.kind, WeaponKind::Ascm))
                        .unwrap_or(false)
                })
                .filter_map(|m| {
                    let d = world.db.weapons.get(&m.weapon)?.clone();
                    Some((d, m.ammo))
                })
                .collect()
        })
        .unwrap_or_default();
    for (def, ammo) in &launch_plan {
        for _ in 0..*ammo {
            crate::weapons::launch(world, id, def, Some(target_id), None);
        }
    }
    let asm_ids: Vec<String> = launch_plan.iter().map(|(d, _)| d.id.clone()).collect();
    let Some(e) = world.entity_mut(id) else { return };
    for m in e.weapons.iter_mut() {
        if asm_ids.contains(&m.weapon) {
            m.ammo = 0;
        }
    }
    e.mission = Some(MissionState::Rtb);
}

/// Head for the nearest friendly deck and land.
fn try_recover(world: &mut World, id: EntityId) {
    let (side, apos) = {
        let Some(e) = world.entity(id) else { return };
        (e.side, e.pos)
    };
    let nearest_deck: Option<(EntityId, f64)> = world
        .entities
        .values()
        .filter(|c| c.alive && c.side == side && c.deck.is_some())
        .map(|c| (c.id, apos.range_to(c.pos)))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let Some((carrier_id, dist)) = nearest_deck else { return };
    if dist > 1.5 {
        let cpos = world.entity(carrier_id).map(|c| c.pos).unwrap_or_default();
        let cruise = world
            .entity(id)
            .map(|e| cruise_of_entity(e, &world.db))
            .unwrap_or(300.0);
        let Some(e) = world.entity_mut(id) else { return };
        e.ordered_course_deg = e.pos.bearing_to(cpos);
        e.ordered_speed_kts = cruise;
        return;
    }
    let class = world.entity(id).map(|e| e.class.clone()).unwrap_or_default();
    let class_name = world
        .db
        .platforms
        .get(&class)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| class.clone());
    let cycle_s = world
        .entity(carrier_id)
        .and_then(|c| c.deck.as_ref())
        .map(|d| d.cycle_s)
        .unwrap_or(120.0);
    let now = world.tick;
    let ready_at = now + (cycle_s as i64);
    let Some(e) = world.entity_mut(id) else { return };
    e.alive = false;
    e.hp = 0.0;
    e.last_emit_tick = now - 1000;
    {
        let c = world.entity_mut(carrier_id).unwrap();
        if let Some(deck) = c.deck.as_mut() {
            deck.cycling.push((class, 1, ready_at));
        }
    }
    world.emit(
        EventKind::AirRecovered { side, carrier: carrier_id, class: class_name, count: 1 },
        Visibility::Side(side),
    );
}

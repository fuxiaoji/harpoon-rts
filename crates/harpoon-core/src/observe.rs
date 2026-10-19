//! Per-side observation projection — the *only* sanctioned way for any client
//! (web UI, LLM staff, RL agent, MCP tool) to read the world. Enemy truth
//! never leaves this module; clients see contacts with estimates.

use crate::events::Event;
use crate::types::{ContactId, DayNight, EntityId, PlatformKind, Side};
use crate::world::{VictoryResult, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitObs {
    pub id: EntityId,
    pub class: String,
    pub name: String,
    pub kind: PlatformKind,
    pub side: Side,
    pub pos: crate::geom::Vec2,
    pub course_deg: f64,
    pub speed_kts: f64,
    pub ordered_course_deg: f64,
    pub ordered_speed_kts: f64,
    pub depth_m: f64,
    pub altitude_m: f64,
    pub hp: f64,
    pub hp_max: f64,
    pub fire_on: bool,
    pub assigned: Option<ContactId>,
    pub weapons: Vec<WeaponObs>,
    pub sensors: Vec<SensorObs>,
    pub chaff: u16,
    pub decoys: u16,
    pub endurance_s: Option<f64>,
    pub deck: Option<DeckObs>,
    pub mission: Option<String>,
    pub systems: crate::world::Systems,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeaponObs {
    pub weapon: String,
    pub name: String,
    pub ammo: u16,
    pub cooldown_s: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorObs {
    pub sensor: String,
    pub name: String,
    pub active: bool,
    /// Nominal detection range vs air/surface/sub targets in nm (UI rings).
    pub vs_air: Option<f64>,
    pub vs_surface: Option<f64>,
    pub vs_sub: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeckObs {
    pub ready: Vec<(String, u8)>,
    pub cycling: Vec<(String, u8, i64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContactObs {
    pub id: ContactId,
    pub category: PlatformKind,
    pub class_name: Option<String>,
    /// Dead-reckoned estimate at the current tick.
    pub estimate: crate::geom::Vec2,
    pub est_course_deg: f64,
    pub est_speed_kts: f64,
    pub quality: f64,
    pub bearing_only: bool,
    pub last_seen_s: f64,
    pub stale: bool,
    pub sensors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub tick: i64,
    pub time_s: f64,
    pub scenario_id: String,
    pub scenario_name: String,
    pub your_side: Side,
    pub over: bool,
    pub result: Option<VictoryResult>,
    pub weather: crate::world::Weather,
    pub day_night: DayNight,
    pub units: Vec<UnitObs>,
    pub contacts: Vec<ContactObs>,
    pub events: Vec<Event>,
}

/// Project the world as seen by `side`. `since_seq` filters the event tail.
pub fn observe(world: &World, side: Side, since_seq: u64, max_events: usize) -> Observation {
    let units = world
        .entities
        .values()
        .filter(|e| e.side == side && e.alive)
        .map(|e| UnitObs {
            id: e.id,
            class: e.class.clone(),
            name: e.name.clone(),
            kind: e.kind,
            side: e.side,
            pos: e.pos,
            course_deg: e.course_deg,
            speed_kts: e.speed_kts,
            ordered_course_deg: e.ordered_course_deg,
            ordered_speed_kts: e.ordered_speed_kts,
            depth_m: e.depth_m,
            altitude_m: e.altitude_m,
            hp: e.hp,
            hp_max: e.hp_max,
            fire_on: e.fire_on,
            assigned: e.assigned,
            weapons: e
                .weapons
                .iter()
                .map(|m| WeaponObs {
                    weapon: m.weapon.clone(),
                    name: world
                        .db
                        .weapons
                        .get(&m.weapon)
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| m.weapon.clone()),
                    ammo: m.ammo,
                    cooldown_s: m.cooldown_s,
                })
                .collect(),
            sensors: e
                .sensors
                .iter()
                .map(|s| {
                    let (a, sf, sb) =
                        crate::sensors::nominal_ranges(&world.db, e, s);
                    SensorObs {
                        sensor: s.sensor.clone(),
                        name: world
                            .db
                            .sensors
                            .get(&s.sensor)
                            .map(|d| d.name.clone())
                            .unwrap_or_else(|| s.sensor.clone()),
                        active: s.active,
                        vs_air: a,
                        vs_surface: sf,
                        vs_sub: sb,
                    }
                })
                .collect(),
            chaff: e.chaff,
            decoys: e.decoys,
            endurance_s: e.endurance_s,
            deck: e.deck.as_ref().map(|d| DeckObs {
                ready: d.ready.clone(),
                cycling: d.cycling.clone(),
            }),
            mission: e.mission.as_ref().map(|m| m.tag().to_string()),
            systems: e.systems.clone(),
        })
        .collect();

    let contacts = world.sides[side.index()]
        .contacts
        .values()
        .map(|c| ContactObs {
            id: c.id,
            category: c.category,
            class_name: c.class_name.clone(),
            estimate: c.estimate_at(world.tick),
            est_course_deg: c.est_course_deg,
            est_speed_kts: c.est_speed_kts,
            quality: c.quality,
            bearing_only: c.bearing_only,
            last_seen_s: (world.tick - c.last_update_tick) as f64 * crate::world::TICK_SECONDS,
            stale: c.stale,
            sensors: c.sensors.clone(),
        })
        .collect();

    let mut events = world.events_for(side, since_seq);
    if events.len() > max_events {
        events = events.split_off(events.len() - max_events);
    }
    let events = events.into_iter().cloned().collect();

    Observation {
        tick: world.tick,
        time_s: world.time_s(),
        scenario_id: world.scenario_id.clone(),
        scenario_name: world.scenario_name.clone(),
        your_side: side,
        over: world.over,
        result: world.result.clone(),
        weather: world.weather.clone(),
        day_night: world.day_night,
        units,
        contacts,
        events,
    }
}

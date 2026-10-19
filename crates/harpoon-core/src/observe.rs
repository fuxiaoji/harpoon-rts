//! Per-side observation projection — the *only* sanctioned way for any client
//! (web UI, LLM staff, RL agent, MCP tool) to read the world. Enemy truth
//! never leaves this module; clients see contacts with estimates.

use crate::events::Event;
use crate::types::{ContactId, DayNight, EntityId, PlatformKind, Side};
use std::collections::BTreeMap;
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
    /// 潜艇是否在跃变层之下（>150m，声纳探测 ×0.5）。
    pub below_layer: Option<bool>,
    /// 柴电潜艇电池余量（0-100），非潜艇为 None。
    pub battery: Option<f64>,
    /// 拖曳阵是否展开（航速 >25kt 自动收起）。
    pub towed_deployed: Option<bool>,
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
    /// Shooter-side battle-damage assessment. Only populated from hits this
    /// side actually observed on a tracked contact.
    #[serde(default)]
    pub bda_hits: u32,
    #[serde(default)]
    pub bda_damage: f64,
    #[serde(default)]
    pub bda_hp_fraction: Option<f64>,
    /// Own ordnance already in flight at this contact. Lets a commander (or
    /// the AI staff) avoid stacking redundant salvos on an already-targeted
    /// contact and keep magazines in reserve.
    #[serde(default)]
    pub own_in_flight: u32,
}

/// Objective display for an exercise. It deliberately exposes only the
/// player's own confirmed kills and losses; the authoritative enemy total is
/// revealed in `result` when the scenario ends.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VictoryObservation {
    /// `area_denial`, `kill_count`, or `vp`.
    pub mode: String,
    pub duration_s: i64,
    pub remaining_s: i64,
    pub kill_count_at_end: bool,
    pub target_vp: f64,
    pub own_confirmed_kills: u32,
    pub own_losses: u32,
    /// Confirmed kills per class ("ship"/"sub"/"aircraft"/"base").
    #[serde(default)]
    pub own_kills_by_kind: BTreeMap<String, u32>,
    /// Own losses per class.
    #[serde(default)]
    pub own_losses_by_kind: BTreeMap<String, u32>,
    /// Scenario kill weights, when configured; ships/subs can outweigh
    /// aircraft. Absent means every platform counts 1.
    #[serde(default)]
    pub kill_weights: Option<crate::scenario::KillWeights>,
    /// Own weighted score under those weights (0.0 without weights).
    #[serde(default)]
    pub own_kill_points: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub tick: i64,
    pub time_s: f64,
    pub scenario_id: String,
    pub scenario_name: String,
    #[serde(default)]
    pub scenario_description: String,
    pub theater: Option<crate::scenario::Theater>,
    pub mission: Option<crate::mission::MissionObservation>,
    #[serde(default)]
    pub victory: Option<VictoryObservation>,
    pub automated_side: Option<Side>,
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
            below_layer: (e.kind == PlatformKind::Sub).then(|| e.depth_m > 150.0),
            battery: e.ext.get("battery").copied(),
            towed_deployed: e
                .ext
                .get("tow_deployed")
                .map(|v| *v > 0.5)
                .filter(|_| e.kind == PlatformKind::Ship),
        })
        .collect();

    let mut own_in_flight: std::collections::BTreeMap<EntityId, u32> = Default::default();
    for e in world.entities.values() {
        if !e.alive || !e.kind.is_projectile() {
            continue;
        }
        if let Some(p) = &e.projectile {
            if p.launched_by_side == side {
                if let Some(t) = p.target {
                    *own_in_flight.entry(t).or_insert(0) += 1;
                }
            }
        }
    }

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
            bda_hits: world.sides[side.index()].bda.get(&c.about).map(|b| b.hits).unwrap_or(0),
            bda_damage: world.sides[side.index()].bda.get(&c.about).map(|b| b.damage).unwrap_or(0.0),
            bda_hp_fraction: world.sides[side.index()].bda.get(&c.about).and_then(|b| b.hp_fraction),
            own_in_flight: own_in_flight.get(&c.about).copied().unwrap_or(0),
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
        scenario_description: world.scenario_description.clone(),
        theater: world.context.theater.clone(),
        mission: crate::mission::observe(world, side),
        victory: Some(VictoryObservation {
            mode: if world.context.area_denial.is_some() {
                "area_denial".into()
            } else if world.scenario_kill_count_at_end {
                "kill_count".into()
            } else {
                "vp".into()
            },
            duration_s: world.duration_s,
            remaining_s: (world.duration_s - world.tick).max(0),
            kill_count_at_end: world.scenario_kill_count_at_end,
            target_vp: world.scenario_target_vp,
            own_confirmed_kills: world.sides[side.index()].confirmed_kills,
            own_losses: world.losses[side.index()],
            own_kills_by_kind: world.sides[side.index()].confirmed_kills_by_kind.clone(),
            own_losses_by_kind: world.losses_by_kind.iter()
                .map(|(kind, n)| (kind.clone(), n[side.index()]))
                .collect(),
            kill_weights: world.scenario_kill_weights.clone(),
            own_kill_points: world.kill_score(side),
        }),
        automated_side: world.context.automated_side,
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

//! Scenario schema + loading. Scenarios are plain JSON; the same schema is
//! used by the server, the MCP tools, the RL environment, and tests.

use crate::db::Database;
use crate::events::{EventKind, Visibility};
use crate::geom::Vec2;
use crate::rng::Rng;
use crate::types::{DayNight, EntityId, PlatformKind, Side};
use crate::world::{DeckState, Entity, SensorInstance, SideState, WeaponMount, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub duration_s: i64,
    pub weather: crate::world::Weather,
    #[serde(default)]
    pub day_night: DayNight,
    pub seed: u64,
    #[serde(default)]
    pub victory: VictoryConfig,
    pub units: Vec<UnitSpawn>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VictoryConfig {
    /// Early win if one side leads by this VP margin (0 = disabled).
    #[serde(default)]
    pub target_vp: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitSpawn {
    pub side: Side,
    pub class: String,
    #[serde(default)]
    pub name: Option<String>,
    pub pos: Vec2,
    #[serde(default)]
    pub course_deg: f64,
    #[serde(default)]
    pub speed_kts: f64,
    #[serde(default)]
    pub depth_m: Option<f64>,
    #[serde(default)]
    pub altitude_m: Option<f64>,
    #[serde(default)]
    pub waypoints: Option<Vec<Vec2>>,
    #[serde(default)]
    pub follow: Option<FollowSpec>,
    #[serde(default)]
    pub sonar_active: bool,
    /// Weapons free at scenario start (auto-engage tracked contacts).
    #[serde(default)]
    pub fire_on: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FollowSpec {
    pub leader_name: String,
    pub bearing_rel_deg: f64,
    pub distance_nm: f64,
}

pub fn load_scenario(sc: Scenario, db: Database) -> Result<World, String> {
    db.validate().map_err(|e| e.to_string())?;
    let seed = sc.seed;
    let mut world = World {
        db,
        tick: 0,
        seed,
        rng: Rng::new(seed),
        entities: Default::default(),
        next_entity: 0,
        sides: [SideState::default(), SideState::default()],
        events: Vec::new(),
        event_seq: 0,
        event_cap: 4096,
        weather: sc.weather.clone(),
        day_night: sc.day_night,
        duration_s: sc.duration_s,
        scenario_id: sc.id.clone(),
        scenario_name: sc.name.clone(),
        scenario_target_vp: sc.victory.target_vp,
        over: false,
        result: None,
        log_commands: true,
        command_log: Vec::new(),
        name_counters: [0, 0],
        sides_present: [
            sc.units.iter().any(|u| u.side == Side::Blue),
            sc.units.iter().any(|u| u.side == Side::Red),
        ],
        stats: Default::default(),
    };

    let mut follow_specs: Vec<(EntityId, FollowSpec)> = Vec::new();
    for (i, spawn) in sc.units.iter().enumerate() {
        let class = world
            .db
            .platforms
            .get(&spawn.class)
            .cloned()
            .ok_or_else(|| format!("unit {i}: unknown class {}", spawn.class))?;
        let eid = world.allocate_id();
        let name = spawn.name.clone().unwrap_or_else(|| {
            // auto_name needs &mut world; emulate deterministically here.
            world.name_counters[spawn.side.index()] += 1;
            let n = world.name_counters[spawn.side.index()];
            format!("{}-{:02} {}", spawn.side.name().to_uppercase(), n, class.name)
        });
        let sensors = class
            .sensors
            .iter()
            .map(|s| {
                let active = spawn.sonar_active && s.starts_with("sonar_a");
                SensorInstance { sensor: s.clone(), active }
            })
            .collect();
        let weapons = class
            .weapons
            .iter()
            .map(|w| WeaponMount {
                weapon: w.weapon.clone(),
                mounts: w.mounts,
                ammo: w.ammo * w.mounts as u16,
                cooldown_s: 0.0,
            })
            .collect();
        let deck = class.deck.as_ref().map(|d| DeckState {
            cycle_s: d.cycle_s,
            capacity: d.capacity,
            ready: d.wing.iter().map(|w| (w.class.clone(), w.count)).collect(),
            cycling: Vec::new(),
        });
        let e = Entity {
            id: eid,
            side: spawn.side,
            class: class.id.clone(),
            name,
            kind: class.kind,
            pos: spawn.pos,
            course_deg: spawn.course_deg,
            speed_kts: spawn.speed_kts.min(class.max_speed_kts),
            ordered_course_deg: spawn.course_deg,
            ordered_speed_kts: spawn.speed_kts.min(class.max_speed_kts),
            waypoints: spawn.waypoints.clone().unwrap_or_default().into(),
            altitude_m: spawn.altitude_m.unwrap_or(match class.kind {
                PlatformKind::Aircraft => 6000.0,
                _ => 0.0,
            }),
            ordered_altitude_m: spawn.altitude_m.unwrap_or(match class.kind {
                PlatformKind::Aircraft => 8000.0,
                _ => 0.0,
            }),
            depth_m: spawn.depth_m.unwrap_or(match class.kind {
                PlatformKind::Sub => 60.0,
                _ => 0.0,
            }),
            ordered_depth_m: spawn.depth_m.unwrap_or(match class.kind {
                PlatformKind::Sub => 60.0,
                _ => 0.0,
            }),
            hp: class.hp,
            hp_max: class.hp,
            alive: true,
            systems: Default::default(),
            fire_on: spawn.fire_on,
            assigned: None,
            weapons,
            sensors,
            chaff: class.chaff,
            decoys: class.decoys,
            auto_chaff: true,
            chaff_active_s: 0.0,
            endurance_s: if class.kind == PlatformKind::Aircraft {
                Some(class.endurance_s)
            } else {
                None
            },
            burn_s: 0.0,
            flooding_s: 0.0,
            deck,
            mission: None,
            station: None,
            last_emit_tick: 0,
            projectile: None,
        };
        world.entities.insert(eid, e);
        if let Some(f) = spawn.follow.clone() {
            follow_specs.push((eid, f));
        }
    }

    // Resolve formation leaders by name.
    for (follower, spec) in follow_specs {
        let leader = world
            .entities
            .values()
            .find(|e| e.name == spec.leader_name && e.side == world.entity(follower).map(|x| x.side).unwrap_or(Side::Blue))
            .map(|e| e.id);
        if let Some((leader_id, fe)) = leader.zip(world.entity_mut(follower)) {
            fe.station = Some(crate::world::StationState {
                leader: leader_id,
                bearing_rel_deg: spec.bearing_rel_deg,
                distance_nm: spec.distance_nm,
            });
        }
    }

    world.emit(
        EventKind::ScenarioStarted { name: sc.name },
        Visibility::All,
    );
    Ok(world)
}

/// Parse a scenario from JSON text.
pub fn scenario_from_json(json: &str) -> Result<Scenario, serde_json::Error> {
    serde_json::from_str(json)
}

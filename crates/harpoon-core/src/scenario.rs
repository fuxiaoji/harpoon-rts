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
    #[serde(default)]
    pub context: ScenarioContext,
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
pub struct ScenarioContext {
    pub theater: Option<Theater>,
    pub area_denial: Option<crate::mission::AreaDenialConfig>,
    #[serde(default)]
    pub air_wings: std::collections::BTreeMap<String, Vec<crate::db::WingFit>>,
    #[serde(default)]
    pub air_missions: std::collections::BTreeMap<String, crate::commands::AirMission>,
    pub automated_side: Option<Side>,
}

/// Local equirectangular plane: x east, y north, distances in nautical miles.
/// lon = west + x/(60*cos(reference_lat)); lat = south + y/60.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Theater {
    pub west: f64,
    pub east: f64,
    pub south: f64,
    pub north: f64,
    pub reference_lat: f64,
    pub image: String,
    pub attribution: String,
    pub source_url: String,
    pub imagery_date: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VictoryConfig {
    /// Early win if one side leads by this VP margin (0 = disabled).
    #[serde(default)]
    pub target_vp: f64,
    /// At the time limit, compare destroyed enemy platforms instead of VP.
    #[serde(default)]
    pub kill_count_at_end: bool,
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
    validate_context(&sc, &db)?;
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
        scenario_description: sc.description.clone(),
        scenario_target_vp: sc.victory.target_vp,
        scenario_kill_count_at_end: sc.victory.kill_count_at_end,
        losses: Default::default(),
        context: sc.context.clone(),
        mission_state: Default::default(),
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
            ready: sc.context.air_wings.get(&name).unwrap_or(&d.wing).iter().map(|w| (w.class.clone(), w.count)).collect(),
            cycling: Vec::new(),
        });
        let e = Entity {
            id: eid,
            side: spawn.side,
            class: class.id.clone(),
            name: name.clone(),
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
            ext: Default::default(),
            deck,
            mission: sc.context.air_missions.get(&name).map(crate::world::MissionState::from_air_mission),
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

fn validate_context(sc: &Scenario, db: &Database) -> Result<(), String> {
    if let Some(t) = &sc.context.theater {
        if ![t.west, t.east, t.south, t.north, t.reference_lat].iter().all(|v| v.is_finite())
            || t.west >= t.east || t.south >= t.north || t.south < -90.0 || t.north > 90.0
            || t.reference_lat.abs() >= 85.0 || !t.image.starts_with("maps/") || t.image.contains("..") {
            return Err("invalid geographic theater".into());
        }
    }
    if let Some(m) = &sc.context.area_denial {
        if !m.radius_nm.is_finite() || m.radius_nm <= 0.0 || !m.center.x.is_finite() || !m.center.y.is_finite()
            || m.hold_s <= 0 || m.hold_s > sc.duration_s || !m.functional_hp_fraction.is_finite()
            || !(0.0..1.0).contains(&m.functional_hp_fraction) || m.protected_names.is_empty() || m.intruder_names.is_empty() {
            return Err("invalid area-denial mission".into());
        }
        for (side, names) in [(m.defender, &m.protected_names), (m.defender.other(), &m.intruder_names)] {
            let unique: std::collections::BTreeSet<_> = names.iter().collect();
            if unique.len() != names.len() { return Err("duplicate mission unit name".into()); }
            for name in names {
                let units: Vec<_> = sc.units.iter().filter(|u| u.name.as_ref() == Some(name)).collect();
                if units.len() != 1 || units[0].side != side || db.platforms.get(&units[0].class).is_none_or(|p| p.kind != PlatformKind::Ship || p.deck.is_none()) {
                    return Err(format!("invalid mission carrier {name}"));
                }
            }
        }
    }
    for (name, wing) in &sc.context.air_wings {
        let unit = sc.units.iter().find(|u| u.name.as_ref() == Some(name)).ok_or_else(|| format!("unknown air-wing carrier {name}"))?;
        let deck = db.platforms.get(&unit.class).and_then(|p| p.deck.as_ref()).ok_or_else(|| format!("{name} has no deck"))?;
        let mut classes = std::collections::BTreeSet::new();
        if wing.iter().map(|w| w.count as u16).sum::<u16>() > deck.capacity as u16 { return Err(format!("{name}: wing exceeds deck capacity")); }
        for w in wing {
            if w.count == 0 || !classes.insert(&w.class) || db.platforms.get(&w.class).is_none_or(|p| p.kind != PlatformKind::Aircraft) {
                return Err(format!("{name}: invalid wing aircraft {}", w.class));
            }
        }
    }
    for (name, mission) in &sc.context.air_missions {
        let unit = sc.units.iter().find(|u| u.name.as_ref() == Some(name)).ok_or_else(|| format!("unknown mission aircraft {name}"))?;
        if db.platforms.get(&unit.class).is_none_or(|p| p.kind != PlatformKind::Aircraft) || unit.waypoints.as_ref().is_some_and(|w| !w.is_empty()) {
            return Err(format!("{name}: initial air mission requires an aircraft without waypoint orders"));
        }
        use crate::commands::AirMission;
        let valid = match mission {
            AirMission::Cap { station, radius_nm } => station.x.is_finite() && station.y.is_finite() && radius_nm.is_finite() && *radius_nm > 0.0,
            AirMission::Recon { point } => point.x.is_finite() && point.y.is_finite(),
            AirMission::Asw { area, radius_nm } => area.x.is_finite() && area.y.is_finite() && radius_nm.is_finite() && *radius_nm > 0.0,
            _ => false,
        };
        if !valid { return Err(format!("{name}: unsupported initial air mission")); }
    }
    Ok(())
}

/// Parse a scenario from JSON text.
pub fn scenario_from_json(json: &str) -> Result<Scenario, serde_json::Error> {
    serde_json::from_str(json)
}

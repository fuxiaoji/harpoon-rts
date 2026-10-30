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
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub beached_2026: Option<crate::rules::beached2026::Configuration>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub surface_gunnery_2026: Option<crate::rules::surface2026::gunnery::Configuration>,
    /// New scenarios default to the repaired RTS rules. Saved World state has
    /// its own profile, so loading old saves never silently changes the rules.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub rules_profile: Option<crate::rules::damage_runtime::RuleProfile>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub flight_environment: Option<crate::rules::operations::FlightEnvironment>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub radar_land_clutter: Option<Vec<crate::rules::operations::LandClutterRegion>>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub aviation_2026: Option<crate::rules::aviation2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub waterborne_2026: Option<crate::rules::aviation2026::waterborne::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub aircombat_2026: Option<crate::rules::aircombat2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub pgm_2026: Option<crate::rules::pgm2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub salh_2026: Option<crate::rules::pgm2026::salh::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub eoir_2026: Option<crate::rules::pgm2026::controlled::eoir1::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub command_sarh_2026: Option<crate::rules::pgm2026::controlled::command_sarh::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub controlled_pgm_los_2026: Option<crate::rules::pgm2026::controlled::LosConfiguration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub rocket_2026: Option<crate::rules::rocket2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub cluster_2026: Option<crate::rules::cluster2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub strafe_2026: Option<crate::rules::strafe2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub detection_2026: Option<crate::rules::detection2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub hf_es_2026: Option<crate::rules::communications2026::hf_es::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub ew_2026: Option<crate::rules::ew2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub torpedo_2026: Option<crate::rules::torpedo2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub navigation_2026: Option<crate::rules::navigation2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub communications_2026: Option<crate::rules::communications2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub unmanned_2026: Option<crate::rules::unmanned2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub battery_2026: Option<crate::rules::battery2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub nuclear_2026: Option<crate::rules::nuclear2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub expanded_damage_2026: Option<crate::rules::expanded_damage2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub asw_weapons_2026:Option<crate::rules::asw_weapons2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub antiship_2026: Option<crate::rules::surface2026::antiship::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub aaw_2026: Option<crate::rules::aaw2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub mines_2026: Option<crate::rules::mines2026::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub ground_2026: Option<crate::rules::ground2026::Configuration>,
    pub theater: Option<Theater>,
    pub area_denial: Option<crate::mission::AreaDenialConfig>,
    #[serde(default)]
    pub air_wings: std::collections::BTreeMap<String, Vec<crate::db::WingFit>>,
    #[serde(default)]
    pub air_missions: std::collections::BTreeMap<String, crate::commands::AirMission>,
    pub automated_side: Option<Side>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub terrain: Option<crate::terrain::Terrain>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub radar_environment: Option<crate::rules::electronic_warfare::RadarEnvironment>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub island_exercise: Option<crate::exercise::IslandExercise>,
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
    /// Optional display-only provider, frozen with the scenario/save.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub tile_layer: Option<String>,
}

/// Per-platform-class kill values for weighted kill-count victory. Without a
/// config every platform is worth 1. Ships and submarines weigh far more than
/// aircraft: an air war won while both carriers survive must not decide a
/// fleet battle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KillWeights {
    #[serde(default = "one_f64")]
    pub ship: f64,
    #[serde(default = "one_f64")]
    pub sub: f64,
    #[serde(default = "one_f64")]
    pub aircraft: f64,
    #[serde(default = "one_f64")]
    pub base: f64,
}

fn one_f64() -> f64 {
    1.0
}

impl Default for KillWeights {
    fn default() -> Self {
        Self { ship: 1.0, sub: 1.0, aircraft: 1.0, base: 1.0 }
    }
}

impl KillWeights {
    pub fn for_kind(&self, kind: &str) -> f64 {
        match kind {
            "ship" => self.ship,
            "sub" => self.sub,
            "aircraft" => self.aircraft,
            _ => self.base,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VictoryConfig {
    /// Early win if one side leads by this VP margin (0 = disabled).
    #[serde(default)]
    pub target_vp: f64,
    /// At the time limit, compare destroyed enemy platforms instead of VP.
    #[serde(default)]
    pub kill_count_at_end: bool,
    /// Optional per-class kill values; present ⇒ the time limit compares
    /// weighted scores and the result carries the per-class breakdown.
    #[serde(default)]
    pub kill_weights: Option<KillWeights>,
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

pub fn load_scenario(sc: Scenario, mut db: Database) -> Result<World, String> {
    if sc.context.flight_environment.as_ref().is_some_and(|e|!e.valid()) {return Err("invalid flight wind conditions".into());}
    if sc.context.radar_land_clutter.as_ref().is_some_and(|rs|rs.iter().any(|r|!r.valid())) {return Err("invalid radar land-clutter region".into());}
    if sc.context.rules_profile!=Some(crate::rules::damage_runtime::RuleProfile::Legacy){crate::rules::damage_runtime::configure_database(&mut db);}
    if sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full){crate::rules::aaw2026::configure_database(&mut db);}
    if sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full){crate::rules::mines2026::configure_database(&mut db);}
    if sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full){crate::rules::surface2026::antiship::configure_database(&mut db);}
    if sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full){crate::rules::pgm2026::configure_database(&mut db);}
    if sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full){crate::rules::rocket2026::configure_database(&mut db);}
    if sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full){crate::rules::cluster2026::configure_database(&mut db);}
    if sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full){crate::rules::strafe2026::configure_database(&mut db);}
    if sc.context.radar_environment.as_ref().is_some_and(|e|!e.valid()) {return Err("invalid radar sea-state/precipitation category".into());}
    db.validate().map_err(|e| e.to_string())?;
    validate_context(&sc, &db)?;
    let seed = sc.seed;
    let mut world = World {
        db,
        rules: crate::rules::damage_runtime::Runtime {
            profile:sc.context.rules_profile.unwrap_or(crate::rules::damage_runtime::RuleProfile::Rts2026),
            table_fingerprint:(sc.context.rules_profile!=Some(crate::rules::damage_runtime::RuleProfile::Legacy)).then(||crate::rules::damage_runtime::fingerprint_for(sc.context.rules_profile.unwrap_or(crate::rules::damage_runtime::RuleProfile::Rts2026))),
            operations:matches!(sc.context.rules_profile,Some(crate::rules::damage_runtime::RuleProfile::Rts2026Operations|crate::rules::damage_runtime::RuleProfile::Rts2026Full)).then(Default::default),
            aviation:(sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full)).then(Default::default),
            aaw:(sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full)).then(Default::default),
            detection:(sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full)).then(||crate::rules::detection2026::State {config:sc.context.detection_2026.clone().unwrap_or_default(),..Default::default()}),
            communications:(sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full)).then(||crate::rules::communications2026::State::new(sc.context.communications_2026.clone().unwrap_or_default())).transpose()?,
            ew:(sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full)).then(||crate::rules::ew2026::State {config:sc.context.ew_2026.clone().unwrap_or_default(),..Default::default()}),
            mines:(sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full)).then(Default::default),
            aircombat:(sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full)).then(Default::default),
            repairs:(sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full)).then(Default::default),
            expanded_damage:(sc.context.rules_profile==Some(crate::rules::damage_runtime::RuleProfile::Rts2026Full)).then(||crate::rules::expanded_damage2026::State::new(sc.context.expanded_damage_2026.clone().unwrap_or_default())).transpose()?,
            ..Default::default()
        },
        tick: 0,
        seed,
        rng: Rng::new(seed),
        entities: Default::default(),
        next_entity: 0,
        sides: [SideState::default(), SideState::default()],
        events: std::collections::VecDeque::new(),
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
        scenario_kill_weights: sc.victory.kill_weights.clone(),
        losses: Default::default(),
        losses_by_kind: Default::default(),
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

    if let Some(t)=&mut world.context.terrain {t.normalize()?;}
    let mut follow_specs: Vec<(EntityId, FollowSpec)> = Vec::new();
    for (i, spawn) in sc.units.iter().enumerate() {
        let class = world
            .db
            .platforms
            .get(&spawn.class)
            .cloned()
            .ok_or_else(|| format!("unit {i}: unknown class {}", spawn.class))?;
        let eid = world.allocate_id();
        if matches!(class.kind,PlatformKind::Ship|PlatformKind::Sub) && world.context.terrain.as_ref().is_some_and(|t|t.contains(spawn.pos)) {
            return Err(format!("unit {i}: ship/submarine spawn is on charted land"));
        }
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
            rule_data:class.rule_data.clone(),
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
            ext: if crate::rules::damage_runtime::active(&world) {
                std::collections::BTreeMap::from([("rules.v2".to_string(),1.0)])
            }else{Default::default()},
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

    crate::exercise::initialize(&mut world)?;
    crate::rules::communications2026::annex::initialize(&world)?;
    crate::rules::aviation2026::initialize(&mut world)?;
    crate::rules::aviation2026::waterborne::initialize(&mut world)?;
    crate::rules::ew2026::initialize(&mut world)?;
    crate::rules::aircombat2026::initialize(&mut world)?;
    crate::rules::mines2026::initialize(&mut world)?;
    crate::rules::ground2026::initialize(&mut world)?;
    crate::rules::torpedo2026::initialize(&mut world)?;
    crate::rules::navigation2026::initialize(&mut world)?;
    crate::rules::battery2026::initialize(&mut world)?;
    if let Some(config)=sc.context.unmanned_2026.clone(){crate::rules::unmanned2026::initialize(&mut world,config)?;}
    crate::rules::nuclear2026::initialize(&mut world)?;
    crate::rules::expanded_damage2026::initialize(&mut world)?;
    crate::rules::detection2026::initialize_emissions(&mut world)?;
    crate::rules::asw_weapons2026::initialize(&mut world)?;
    crate::rules::surface2026::antiship::initialize(&mut world,sc.context.antiship_2026.clone())?;
    crate::rules::surface2026::gunnery::initialize(&mut world)?;
    crate::rules::pgm2026::initialize(&mut world)?;
    crate::rules::rocket2026::initialize(&mut world)?;
    crate::rules::cluster2026::initialize(&mut world)?;
    crate::rules::strafe2026::initialize(&mut world)?;
    crate::rules::unmanned2026::finalize_initial_stow(&mut world)?;
    crate::rules::beached2026::initialize(&mut world)?;
    world.emit(
        EventKind::ScenarioStarted { name: sc.name },
        Visibility::All,
    );
    Ok(world)
}

fn validate_context(sc: &Scenario, db: &Database) -> Result<(), String> {
    if let Some(config)=&sc.context.aaw_2026 {crate::rules::aaw2026::validate_configuration(config)?;}
    crate::exercise::validate(sc,db)?;
    if let Some(t) = &sc.context.theater {
        if ![t.west, t.east, t.south, t.north, t.reference_lat].iter().all(|v| v.is_finite())
            || t.west >= t.east || t.south >= t.north || t.south < -90.0 || t.north > 90.0
            || t.reference_lat.abs() >= 85.0 || !t.image.starts_with("maps/") || t.image.contains("..") {
            return Err("invalid geographic theater".into());
        }
    }
    if let Some(w) = &sc.victory.kill_weights {
        if ![w.ship, w.sub, w.aircraft, w.base].iter().all(|v| v.is_finite() && *v >= 0.0) {
            return Err("invalid kill weights".into());
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
            AirMission::Cap { station, radius_nm, .. } => station.x.is_finite() && station.y.is_finite() && radius_nm.is_finite() && *radius_nm > 0.0,
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

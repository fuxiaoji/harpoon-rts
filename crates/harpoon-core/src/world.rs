//! Authoritative world state. This module owns every mutable fact in the
//! simulation; subsystems (movement, sensors, weapons, air ops) mutate it via
//! `&mut World`. Iteration order is always `BTreeMap` order for determinism.

use crate::commands::AirMission;
use crate::db::{Database, PlatformClass, SensorKind, WeaponDef};
use crate::events::{Event, EventKind, Visibility};
use crate::geom::Vec2;
use crate::rng::Rng;
use crate::types::{ContactId, DayNight, EntityId, PlatformKind, Side};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One simulated second per tick.
pub const TICK_SECONDS: f64 = 1.0;
/// Sensor/knowledge update cadence in ticks.
pub const SENSOR_INTERVAL_TICKS: i64 = 4;
/// Seconds a lost contact is kept (dead-reckoned) before deletion.
pub const CONTACT_STALE_S: f64 = 900.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Weather {
    /// Surface visual range in nm.
    pub visibility_nm: f64,
    /// Radar jamming factor per side: 0 = clear, 0.8 = heavy. Applied as a
    /// multiplier on that side's radar effective ranges.
    pub jam: [f64; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Systems {
    /// 0..1 multipliers; 0 = knocked out. `mobility` caps speed.
    pub mobility: f64,
    pub radar: f64,
    pub sonar: f64,
    pub fire_control: f64,
}

impl Default for Systems {
    fn default() -> Self {
        Self { mobility: 1.0, radar: 1.0, sonar: 1.0, fire_control: 1.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeaponMount {
    pub weapon: String,
    pub mounts: u8,
    pub ammo: u16,
    pub cooldown_s: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorInstance {
    pub sensor: String,
    /// Sonar only: active pinging (reveals own position).
    pub active: bool,
}

impl SensorInstance {
    pub fn is_active_kind(&self, kind: SensorKind) -> bool {
        match kind {
            SensorKind::SonarActive => self.active,
            _ => true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeckState {
    pub cycle_s: f64,
    pub capacity: u8,
    /// (class id, ready count)
    pub ready: Vec<(String, u8)>,
    /// (class id, count, ready_at_tick) — landed/rearming aircraft.
    pub cycling: Vec<(String, u8, i64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MissionState {
    Cap { station: Vec2, radius_nm: f64 },
    Strike { contact: ContactId, released: bool },
    Asw { area: Vec2, radius_nm: f64 },
    Recon { point: Vec2 },
    Rtb,
}

impl MissionState {
    pub fn tag(&self) -> &'static str {
        match self {
            MissionState::Cap { .. } => "cap",
            MissionState::Strike { .. } => "strike",
            MissionState::Asw { .. } => "asw",
            MissionState::Recon { .. } => "recon",
            MissionState::Rtb => "rtb",
        }
    }

    pub fn from_air_mission(m: &AirMission) -> Self {
        match m.clone() {
            AirMission::Cap { station, radius_nm } => MissionState::Cap { station, radius_nm },
            AirMission::Strike { contact } => MissionState::Strike { contact, released: false },
            AirMission::Asw { area, radius_nm } => MissionState::Asw { area, radius_nm },
            AirMission::Recon { point } => MissionState::Recon { point },
            AirMission::Rtb => MissionState::Rtb,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StationState {
    pub leader: EntityId,
    pub bearing_rel_deg: f64,
    pub distance_nm: f64,
}

/// Data attached to Missile/Torpedo entities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectileData {
    pub weapon: String,
    /// Truth target (referee-only).
    pub target: Option<EntityId>,
    /// Fixed aim point for ballistic weapons.
    pub aim: Vec2,
    pub launched_by: EntityId,
    pub launched_by_side: Side,
    pub live_s: f64,
    pub enabled: bool,
    pub decoyed: bool,
    pub decoy_point: Option<Vec2>,
    pub chaff_checked: bool,
    pub ciws_checked: bool,
    pub passes: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub side: Side,
    pub class: String,
    pub name: String,
    pub kind: PlatformKind,
    pub pos: Vec2,
    pub course_deg: f64,
    pub speed_kts: f64,
    pub ordered_course_deg: f64,
    pub ordered_speed_kts: f64,
    #[serde(default)]
    pub waypoints: std::collections::VecDeque<Vec2>,
    /// Aircraft altitude (m, positive up).
    pub altitude_m: f64,
    pub ordered_altitude_m: f64,
    /// Submarine depth (m, positive down).
    pub depth_m: f64,
    pub ordered_depth_m: f64,
    pub hp: f64,
    pub hp_max: f64,
    pub alive: bool,
    pub systems: Systems,
    pub fire_on: bool,
    pub assigned: Option<ContactId>,
    pub weapons: Vec<WeaponMount>,
    pub sensors: Vec<SensorInstance>,
    pub chaff: u16,
    pub decoys: u16,
    pub auto_chaff: bool,
    pub chaff_active_s: f64,
    /// Aircraft: remaining fuel seconds.
    pub endurance_s: Option<f64>,
    /// Seconds of fire damage remaining.
    pub burn_s: f64,
    /// Seconds of flooding damage remaining (drains HP faster than fire).
    #[serde(default)]
    pub flooding_s: f64,
    /// Rules-extension scratch state (damage control, battery, maneuver
    /// levels, ...). Keys are owned by the corresponding rules::* module.
    #[serde(default)]
    pub ext: std::collections::BTreeMap<String, f64>,
    #[serde(default)]
    pub deck: Option<DeckState>,
    #[serde(default)]
    pub mission: Option<MissionState>,
    #[serde(default)]
    pub station: Option<StationState>,
    #[serde(default)]
    pub projectile: Option<ProjectileData>,
    /// Ticks since this entity created a distinct emission signature event.
    #[serde(skip)]
    pub last_emit_tick: i64,
}

impl Entity {
    pub fn class_def<'a>(&self, db: &'a Database) -> Option<&'a PlatformClass> {
        db.platforms.get(&self.class)
    }

    pub fn weapon_def<'a>(&self, id: &str, db: &'a Database) -> Option<&'a WeaponDef> {
        db.weapons.get(id)
    }

    pub fn max_speed(&self, db: &Database) -> f64 {
        let class = match db.platforms.get(&self.class) {
            Some(c) => c,
            None => return 0.0,
        };
        let cap = match self.kind {
            PlatformKind::Sub => {
                // Subs are much slower deep.
                if self.depth_m > 60.0 { class.max_speed_kts * 0.6 } else { class.max_speed_kts }
            }
            _ => class.max_speed_kts,
        };
        cap * self.systems.mobility
    }

    pub fn current_alt_for_horizon(&self) -> f64 {
        match self.kind {
            PlatformKind::Aircraft | PlatformKind::Missile => self.altitude_m,
            _ => 0.0,
        }
    }
}

/// A contact in one side's tactical picture. `about`/`offset` are
/// referee-only; clients see only the estimate fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contact {
    pub id: ContactId,
    pub about: EntityId,
    pub category: PlatformKind,
    pub class_name: Option<String>,
    /// Estimate position at `last_update_tick`.
    pub estimate: Vec2,
    pub est_course_deg: f64,
    pub est_speed_kts: f64,
    pub bearing_only: bool,
    pub quality: f64,
    pub last_update_tick: i64,
    pub first_tick: i64,
    pub stale: bool,
    pub sensors: Vec<String>,
    /// Persistent position error vector; shrinks to 0 as quality -> 1.
    pub offset: Vec2,
    /// ContactIdentified already dispatched (referee bookkeeping).
    #[serde(default)]
    pub announced: bool,
}

impl Contact {
    /// Display estimate advanced by dead reckoning to `tick`.
    pub fn estimate_at(&self, tick: i64) -> Vec2 {
        let dt = (tick - self.last_update_tick).max(0) as f64;
        let quality_shrink = (1.0 - self.quality).max(0.0);
        let err = self.offset.scaled(quality_shrink * quality_shrink);
        let drift = Vec2::from_bearing(self.est_course_deg)
            .scaled(self.est_speed_kts * crate::geom::KTS_TO_NM_PER_S * dt);
        self.estimate.add(drift).add(err)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SideState {
    pub contacts: BTreeMap<ContactId, Contact>,
    pub next_contact: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VictoryResult {
    pub winner: Option<Side>,
    pub reason: String,
    pub vp_blue: f64,
    pub vp_red: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoggedCommand {
    pub tick: i64,
    pub side: Side,
    pub command: crate::commands::Command,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SimStats {
    pub sensor_scans: u64,
    pub projectiles_launched: u64,
    pub adjudications: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub db: Database,
    pub tick: i64,
    pub seed: u64,
    pub rng: Rng,
    pub entities: BTreeMap<EntityId, Entity>,
    pub next_entity: u32,
    pub sides: [SideState; 2],
    pub events: Vec<Event>,
    pub event_seq: u64,
    pub event_cap: usize,
    pub weather: Weather,
    pub day_night: DayNight,
    pub duration_s: i64,
    pub scenario_id: String,
    pub scenario_name: String,
    /// Early-win VP margin from the scenario victory config.
    pub scenario_target_vp: f64,
    pub over: bool,
    pub result: Option<VictoryResult>,
    pub log_commands: bool,
    pub command_log: Vec<LoggedCommand>,
    pub name_counters: [u32; 2],
    /// Which sides fielded units at scenario start (victory applies only
    /// between participating sides).
    pub sides_present: [bool; 2],
    pub stats: SimStats,
}

impl World {
    /// Emit an event to the log (ring buffer).
    pub fn emit(&mut self, kind: EventKind, vis: Visibility) {
        self.event_seq += 1;
        self.events.push(Event { seq: self.event_seq, tick: self.tick, kind, vis });
        if self.events.len() > self.event_cap {
            let drop = self.events.len() - self.event_cap;
            self.events.drain(0..drop);
        }
    }

    pub fn events_for(&self, side: Side, since_seq: u64) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| e.seq > since_seq && e.vis.visible_to(side))
            .collect()
    }

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn entity_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.entities.get_mut(&id)
    }

    pub fn time_s(&self) -> f64 {
        self.tick as f64 * TICK_SECONDS
    }

    /// Deterministic per-scope RNG: derived streams keyed by purpose so scan
    /// order or command timing cannot perturb other subsystems' sequences.
    pub fn scope_rng(&self, entity: EntityId, purpose: &str) -> Rng {
        self.rng.derive(Rng::hash_tag(self.tick, entity.0, purpose))
    }

    pub fn allocate_id(&mut self) -> EntityId {
        self.next_entity += 1;
        EntityId(self.next_entity)
    }

    /// Auto-name: BLUE-01, RED-01, ...
    pub fn auto_name(&mut self, side: Side, class_name: &str) -> String {
        self.name_counters[side.index()] += 1;
        let n = self.name_counters[side.index()];
        format!("{}-{:02} {}", side.name().to_uppercase(), n, class_name)
    }

    pub fn find_contact(&self, side: Side, about: EntityId) -> Option<ContactId> {
        self.sides[side.index()]
            .contacts
            .values()
            .find(|c| c.about == about && !c.stale)
            .map(|c| c.id)
    }

    pub fn contact(&self, side: Side, id: ContactId) -> Option<&Contact> {
        self.sides[side.index()].contacts.get(&id)
    }

    /// Live (non-stale) enemy entities known to `side` at given quality.
    pub fn tracked_entities<'a>(
        &'a self,
        side: Side,
        min_quality: f64,
    ) -> impl Iterator<Item = (&'a Contact, &'a Entity)> + 'a {
        self.sides[side.index()]
            .contacts
            .values()
            .filter(move |c| !c.stale && c.quality >= min_quality)
            .filter_map(|c| self.entities.get(&c.about).map(|e| (c, e)))
            .filter(|(_, e)| e.alive)
    }

    /// VP currently earned by each side (kills + base damage).
    pub fn victory_points(&self) -> (f64, f64) {
        let mut vp = [0.0f64; 2];
        for e in self.entities.values() {
            if e.alive || e.kind.is_projectile() {
                continue;
            }
            if e.kind == PlatformKind::Missile || e.kind == PlatformKind::Torpedo {
                continue;
            }
            let Some(class) = self.db.platforms.get(&e.class) else { continue };
            // VP for killing the *other* side's stuff.
            let killer = e.side.other();
            vp[killer.index()] += class.vp_value;
        }
        // Base damage credit.
        for e in self.entities.values() {
            let Some(class) = self.db.platforms.get(&e.class) else { continue };
            if !class.is_base || !e.alive {
                continue;
            }
            let frac = 1.0 - (e.hp / e.hp_max).clamp(0.0, 1.0);
            let attacker = e.side.other();
            vp[attacker.index()] += class.vp_value * 0.5 * frac;
        }
        (vp[0], vp[1])
    }
}

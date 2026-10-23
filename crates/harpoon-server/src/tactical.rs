//! TacticalStaff — a state-machine naval doctrine staff.
//!
//! Doctrine basis: docs/design/naval-doctrine.md (P1..P8), distilled from the
//! Harpoon V rulebook and modern carrier-group practice.
//!
//! Each friendly combatant is assigned a role (carrier, AEW, CAP, ASW screen,
//! gun screen, striker, submarine, raider) and follows a per-role finite state
//! machine:
//!
//!   ON_STATION -> ENGAGE -> (WITHDRAW | RETREAT)
//!
//! with launch cohorts, plotted VLow ingress, bounded CAP/escort, private
//! intent logs and sensor-only assets excluded from combat command loops.
//! Tunables in `Params` are AI policy, separate from adjudication data.

use crate::ai::{CommandStaff, StaffNote};
#[path = "tactical/aviation.rs"]
mod aviation;
#[path = "tactical/coordination.rs"]
mod coordination;
#[path = "tactical/supervision.rs"]
pub mod supervision;
#[path = "tactical/assessment.rs"]
pub mod assessment;
use harpoon_core::commands::{AirMission, Command};
use harpoon_core::db::{Database, WeaponKind};
use harpoon_core::observe::{Observation, UnitObs};
use harpoon_core::types::{ContactId, EntityId, PlatformKind, Side};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Params {
    pub cap_min_size: u32,
    pub cap_fuel_rtb_s: f64,
    pub aew_standoff_nm: f64,
    pub strike_min_q: f64,
    pub screen_standoff_nm: f64,
    pub retreat_hp_frac: f64,
    pub sub_attack_range_nm: f64,
    pub sub_patrol_speed_kts: f64,
    pub salvo_min_interval_s: i64,
    /// Massed-raid doctrine: ASCM rounds per shooter per salvo, the minimum
    /// spacing between that shooter's salvos, and the in-flight ceiling per
    /// target contact above which shooters hold fire.
    pub surface_salvo_rounds: u16,
    pub surface_salvo_interval_s: i64,
    pub surface_inflight_cap: u32,
    /// Aircraft per anti-ship strike wave.
    pub strike_wave_size: u32,
    pub strike_altitude_m: f64,
    pub strike_assembly_s: i64,
    pub strike_wave_interval_s: i64,
    pub cap_leash_nm: f64,
    pub asw_barrier_nm: f64,
    #[serde(skip_serializing_if = "default_policy_timer")]
    pub strike_retarget_s: i64,
    #[serde(skip_serializing_if = "default_policy_timer")]
    pub strike_relay_hold_s: i64,
}

fn default_policy_timer(value: &i64) -> bool { *value == 180 }

impl Default for Params {
    fn default() -> Self {
        Self {
            cap_min_size: 2,
            cap_fuel_rtb_s: 900.0,
            aew_standoff_nm: 40.0,
            strike_min_q: 0.4,
            screen_standoff_nm: 16.0,
            retreat_hp_frac: 0.3,
            sub_attack_range_nm: 22.0,
            sub_patrol_speed_kts: 4.0,
            salvo_min_interval_s: 20,
            surface_salvo_rounds: 8,
            surface_salvo_interval_s: 150,
            surface_inflight_cap: 24,
            strike_wave_size: 6,
            strike_altitude_m: 50.0,
            strike_assembly_s: 330,
            strike_wave_interval_s: 600,
            cap_leash_nm: 55.0,
            asw_barrier_nm: 12.0,
            strike_retarget_s: 180,
            strike_relay_hold_s: 180,
        }
    }
}

impl Params {
    /// data/ai/tactical_params.json overrides defaults; the self-play tuner
    /// writes improved sets back.
    pub fn load_optional() -> Params {
        std::fs::read_to_string("data/ai/tactical_params.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Carrier,
    Airbase,
    Aew,
    Ew,
    Scout,
    Cap,
    AswScreen,
    GunScreen,
    Striker,
    Submarine,
    Raider,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum State {
    OnStation,
    Withdraw,
    Retreat,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct UnitMind {
    role: Role,
    state: State,
    last_salvo_tick: i64,
}

fn is_aew_class(class: &str) -> bool {
    let c = class.to_ascii_lowercase();
    c.contains("aew")
        || c.contains("e2")
        || c.contains("e-2")
        || c.contains("kj600")
        || c.contains("kj200")
        || c.contains("z18j")
        || c.contains("e3")
}

fn is_aew(u: &UnitObs) -> bool {
    let n = u.name.to_ascii_uppercase();
    n.contains("AEW") || is_aew_class(&u.class)
}

fn is_helo(u: &UnitObs) -> bool {
    let n = u.name.to_ascii_uppercase();
    let c = u.class.to_ascii_lowercase();
    n.contains("MH-")
        || n.contains("SH-")
        || n.contains("Z-")
        || n.contains("卡-")
        || c.contains("helo")
        || c.contains("z18")
        || c.contains("z9")
        || c.contains("merlin")
        || c.contains("ch14")
        || c.contains("ch148")
}

fn contact_pos(
    obs: &Observation,
    id: ContactId,
    fallback: harpoon_core::Vec2,
) -> harpoon_core::Vec2 {
    obs.contacts
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.estimate)
        .unwrap_or(fallback)
}

fn controllable(u: &UnitObs) -> bool {
    !u.manual_control && !u.kind.is_projectile() && u.class != harpoon_core::rules::asw::SONOBUOY_CLASS
}

fn known_sensor_contact(c: &harpoon_core::observe::ContactObs) -> bool {
    c.class_name
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase()
        .contains("buoy")
}

impl TacticalStaff {
    fn role_for(&self, u: &UnitObs) -> Role {
        if u.kind==PlatformKind::Base && u.deck.is_some() {return Role::Airbase;}
        // Many surface combatants carry a helicopter deck; only a real carrier
        // (large hull or CV-class name) runs the carrier state machine.
        let carrier_name = {
            let n = u.name.to_ascii_uppercase();
            let c = u.class.to_ascii_lowercase();
            n.contains("CV")
                || c.contains("cv")
                || c.contains("carrier")
                || c.contains("cn_001")
                || c.contains("cn_002")
                || c.contains("us_ford")
                || c.contains("us_nimitz")
        };
        if u.deck.is_some() && u.kind == PlatformKind::Ship && (u.hp_max >= 700.0 || carrier_name) {
            return Role::Carrier;
        }
        match u.kind {
            PlatformKind::Sub => Role::Submarine,
            PlatformKind::Aircraft => {
                if harpoon_core::rules::electronic_warfare::capability(&u.class).is_some() {Role::Ew}
                else if is_aew(u) {
                    Role::Aew
                } else if is_helo(u) {
                    Role::AswScreen
                } else if u.mission.as_deref()==Some("recon") {
                    Role::Scout
                } else if u.mission.as_deref() == Some("strike") {
                    // Multirole fighters are only strikers while tasked as such.
                    Role::Striker
                } else if self.has_kind(&u.class, WeaponKind::Aam) {
                    Role::Cap
                } else if self.has_kind(&u.class, WeaponKind::Ascm)
                    || self.has_kind(&u.class, WeaponKind::Bomb)
                    || self.has_kind(&u.class, WeaponKind::Rocket)
                {
                    Role::Striker
                } else {
                    Role::Cap
                }
            }
            PlatformKind::Ship => {
                let aa = self.has_kind(&u.class, WeaponKind::Sam);
                let asw = self.has_kind(&u.class, WeaponKind::AswRocket)
                    || self.has_kind(&u.class, WeaponKind::Lwt)
                    || u.sensors.iter().any(|s| s.sensor.contains("sonar_p_tow"));
                let asm = self.has_kind(&u.class, WeaponKind::Ascm)
                    || self.has_kind(&u.class, WeaponKind::Slcm);
                if aa {
                    Role::GunScreen
                } else if asw {
                    Role::AswScreen
                } else if asm {
                    Role::Raider
                } else {
                    Role::Other
                }
            }
            _ => Role::Other,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TacticalStaff {
    #[serde(default,skip_serializing_if="supervision::Supervisor::is_empty")]
    pub supervision:supervision::Supervisor,
    #[allow(dead_code)]
    side: Side,
    params: Params,
    minds: BTreeMap<EntityId, UnitMind>,
    /// 威胁轴：从己方锚点指向最早一次对敌探测的方位（度，0=北，顺时针）。
    threat_axis_deg: f64,
    threat_axis_locked: bool,
    /// 每艘航母上次组织打击波次的 tick（P4 多域协同窗口）。
    last_strike_wave: BTreeMap<EntityId, i64>,
    /// Weapon kinds and ranges; the observation only carries mount ids.
    #[serde(skip)]
    db: Option<Database>,
    air_minds: BTreeMap<EntityId, aviation::AirMind>,
    #[serde(default,skip_serializing_if="BTreeMap::is_empty")]
    packages:BTreeMap<EntityId,aviation::Package>,
    last_launch: BTreeMap<EntityId, i64>,
    wave_targets: BTreeMap<EntityId, ContactId>,
    notes: Vec<StaffNote>,
    note_keys: BTreeMap<EntityId, (String, Option<ContactId>)>,
    identified_sensors: std::collections::BTreeSet<ContactId>,
    #[serde(default)]
    track_memory: BTreeMap<ContactId, (i64, harpoon_core::observe::ContactObs)>,
    #[serde(default)]
    pending_salvos: Vec<coordination::PendingSalvo>,
    #[serde(default)]
    posture: u8,
}

impl TacticalStaff {
    pub fn new(side: Side) -> Self {
        Self::with_db_or_none(side, None)
    }

    pub(crate) fn restore_database(&mut self, db: Database) { self.db = Some(db); }

    pub fn with_db(side: Side, db: Database) -> Self {
        Self::with_db_or_none(side, Some(db))
    }

    fn with_db_or_none(side: Side, db: Option<Database>) -> Self {
        Self {
            supervision:Default::default(),
            side,
            params: Params::load_optional(),
            minds: BTreeMap::new(),
            // Blue looks east, Red west until the first contact locks the axis.
            threat_axis_deg: if side == Side::Blue { 90.0 } else { 270.0 },
            threat_axis_locked: false,
            last_strike_wave: BTreeMap::new(),
            db,
            air_minds: BTreeMap::new(),
            packages:BTreeMap::new(),
            last_launch: BTreeMap::new(),
            wave_targets: BTreeMap::new(),
            notes: Vec::new(),
            note_keys: BTreeMap::new(),
            identified_sensors: Default::default(),
            track_memory: Default::default(),
            pending_salvos: Default::default(),
            posture:0,
        }
    }

    /// Kind of an equipped weapon id; falls back to the id prefix when no
    /// database is attached (tests / legacy staff).
    fn kind_of(&self, id: &str) -> Option<WeaponKind> {
        if let Some(db) = &self.db {
            if let Some(w) = db.weapons.get(id) {
                return Some(w.kind);
            }
        }
        let id = id.to_ascii_lowercase();
        if id.contains("torp") || id.contains("mk48") || id.contains("yu6") || id.contains("yu10") {
            Some(WeaponKind::Torpedo)
        } else if id.contains("harpoon") || id.contains("yj") || id.contains("ascm") {
            Some(WeaponKind::Ascm)
        } else if id.contains("tomahawk") || id.contains("dh10") || id.contains("slcm") {
            Some(WeaponKind::Slcm)
        } else if id.contains("mk54") || id.contains("lwt") {
            Some(WeaponKind::Lwt)
        } else if id.contains("asw") || id.contains("whh") || id.contains("vlasroc") {
            Some(WeaponKind::AswRocket)
        } else {
            None
        }
    }

    fn range_of(&self, id: &str) -> f64 {
        self.db
            .as_ref()
            .and_then(|db| db.weapons.get(id))
            .map(|w| w.range_nm)
            .unwrap_or(60.0)
    }

    fn cruise_of(&self, class:&str)->f64 {
        self.db.as_ref().and_then(|db|db.platforms.get(class)).map(|p|p.cruise_speed_kts.max(60.0)).unwrap_or(300.0)
    }

    /// Pick an equipped, loaded mount whose kind is in `kinds` and whose range
    /// covers `dist`. Returns the weapon id, nominal range and ready ammo.
    fn pick_weapon(
        &self,
        u: &UnitObs,
        kinds: &[WeaponKind],
        dist: f64,
    ) -> Option<(String, f64, u16)> {
        let mut best: Option<(String, f64, u16)> = None;
        for w in &u.weapons {
            if w.ammo == 0 || w.cooldown_s > 0.0 {
                continue;
            }
            let Some(kind) = self.kind_of(&w.weapon) else {
                continue;
            };
            if !kinds.contains(&kind) {
                continue;
            }
            let range = self.range_of(&w.weapon);
            if dist > range * 0.95 {
                continue;
            }
            if best.as_ref().map(|(_, r, _)| range > *r).unwrap_or(true) {
                best = Some((w.weapon.clone(), range, w.ammo));
            }
        }
        best
    }

    fn has_kind(&self, class: &str, kind: WeaponKind) -> bool {
        let Some(db) = &self.db else { return false };
        db.platforms
            .get(class)
            .map(|p| {
                p.weapons.iter().any(|m| {
                    db.weapons
                        .get(&m.weapon)
                        .map(|w| w.kind == kind)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }

    fn can_fly_anti_ship(&self, class: &str) -> bool {
        if self.has_kind(class, WeaponKind::Ascm) {
            return true;
        }
        // Tests/legacy: class-name fallback for the usual strike airframes.
        let c = class.to_ascii_lowercase();
        c.contains("asm") || c.contains("strike") || c.contains("bomber")
    }

    fn can_fly_asw(&self, class: &str) -> bool {
        if let Some(db) = &self.db {
            if let Some(p) = db.platforms.get(class) {
                return p.dipping_sonar
                    || p.weapons.iter().any(|m| {
                        db.weapons
                            .get(&m.weapon)
                            .map(|w| matches!(w.kind, WeaponKind::Lwt | WeaponKind::Torpedo))
                            .unwrap_or(false)
                    });
            }
        }
        let c = class.to_ascii_lowercase();
        c.contains("mh60") || c.contains("z9") || c.contains("z18f") || c.contains("merlin")
    }

    fn ensure_roles(&mut self, obs: &Observation) {
        let ids: std::collections::BTreeSet<_> = obs
            .units
            .iter()
            .filter(|u| controllable(u))
            .map(|u| u.id)
            .collect();
        self.minds.retain(|id, _| ids.contains(id));
        for u in obs.units.iter().filter(|u| controllable(u)) {
            let role = self.role_for(u);
            self.minds
                .entry(u.id)
                .and_modify(|m| m.role = role)
                .or_insert(UnitMind {
                    role,
                    state: State::OnStation,
                    last_salvo_tick: -100000,
                });
        }
    }

    fn sensor_contact(&self, c: &harpoon_core::observe::ContactObs) -> bool {
        known_sensor_contact(c) || self.identified_sensors.contains(&c.id)
    }

    fn lock_threat_axis(&mut self, obs: &Observation) {
        let Some(anchor) = self.anchor_of(obs) else {
            return;
        };
        let Some(c) = obs
            .contacts
            .iter()
            .filter(|c| {
                c.category == PlatformKind::Ship
                    && !self.sensor_contact(c)
                    && !c.stale
                    && !c.bearing_only
                    && c.quality > 0.4
            })
            .min_by(|a, b| {
                anchor
                    .pos
                    .range_to(a.estimate)
                    .total_cmp(&anchor.pos.range_to(b.estimate))
            })
        else {
            return;
        };
        self.threat_axis_deg = anchor.pos.bearing_to(c.estimate);
        self.threat_axis_locked = true;
    }

    /// P8: 岸上目标（SLCM/ASCM 对地）。
    fn best_base_target(&self, obs: &Observation, shooter: &UnitObs) -> Option<(ContactId, f64)> {
        obs.contacts
            .iter()
            .filter(|c| {
                !c.stale
                    && self.supervision.target_allowed(c.id,obs.tick)
                    && c.quality >= self.params.strike_min_q
                    && c.category == PlatformKind::Base
            })
            .map(|c| (c.id, shooter.pos.range_to(c.estimate)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// 潜艇的攻舰目标：优先航母，其次护航舰；只打水面舰艇。
    fn best_ship_target(&self, obs: &Observation, shooter: &UnitObs) -> Option<(ContactId, f64)> {
        let mut best: Option<(ContactId, f64, f64)> = None;
        for c in &obs.contacts {
            if c.stale
                || c.bearing_only
                || c.quality < 0.4
                || c.category != PlatformKind::Ship
                || c.bda_hp_fraction == Some(0.0)
                || self.sensor_contact(c)
                || !self.supervision.target_allowed(c.id,obs.tick)
            {
                continue;
            }
            let value = match c.class_name.as_deref() {
                // Escort-first doctrine: peel the air-defence screen before
                // shooting at the flight decks it protects.
                Some(n)
                    if n.contains("Burke")
                        || n.contains("055")
                        || n.contains("052")
                        || n.contains("Ticonderoga") =>
                {
                    60.0
                }
                Some(n)
                    if n.contains("CV")
                        || n.contains("Carrier")
                        || n.contains("FORD")
                        || n.contains("NIMITZ") =>
                {
                    30.0
                }
                _ => 15.0,
            };
            let dist = shooter.pos.range_to(c.estimate);
            let retries=self.supervision.attacks.get(&c.id).map(|a|a.unconfirmed_batches).unwrap_or(0);
            let score = value - dist * 0.02 - retries as f64*value;
            if best.map(|(_, _, s)| score > s).unwrap_or(true) {
                best = Some((c.id, dist, score));
            }
        }
        best.map(|(id, dist, _)| (id, dist))
    }

    /// 反潜目标：最近的非失效潜艇接触。
    fn best_sub_target(&self, obs: &Observation, shooter: &UnitObs) -> Option<(ContactId, f64)> {
        obs.contacts
            .iter()
            .filter(|c| !c.stale && c.quality > 0.4 && c.category == PlatformKind::Sub)
            .map(|c| (c.id, shooter.pos.range_to(c.estimate)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    fn anchor_of<'a>(&self, obs: &'a Observation) -> Option<&'a UnitObs> {
        obs.units
            .iter()
            .find(|u| self.minds.get(&u.id).map(|m| m.role) == Some(Role::Carrier))
            .or_else(||obs.units.iter().find(|u|self.role_for(u)==Role::Airbase))
            .or_else(|| {
                obs.units
                    .iter()
                    .find(|u| u.kind == PlatformKind::Ship && controllable(u))
            })
    }
}

impl CommandStaff for TacticalStaff {
    fn name(&self) -> &'static str {
        "tactical_fsm"
    }
    fn take_notes(&mut self) -> Vec<StaffNote> {
        std::mem::take(&mut self.notes)
    }

    fn decide(&mut self, obs: &Observation) -> Vec<Command> {
        self.notes.clear();
        if obs.over || obs.your_side != self.side || obs.exercise.as_ref().is_some_and(|e|e.locked) {
            return Vec::new();
        }
        self.notes.clear();
        self.track_memory.retain(|_,(tick,_)|obs.tick-*tick<=1800);
        for c in &obs.contacts {
            let mut remembered=c.clone();
            if remembered.class_name.is_none() {
                remembered.class_name=self.track_memory.get(&c.id).and_then(|(_,old)|old.class_name.clone());
            }
            if !c.stale && (c.quality>=self.params.strike_min_q&&!c.bearing_only
                || self.track_memory.get(&c.id).is_none_or(|(_,old)|old.quality<self.params.strike_min_q)) {self.track_memory.insert(c.id,(obs.tick,remembered));}
        }
        // TDL may restore the same track id with class_name=None. Retain a
        // classification this side actually observed instead of treating its
        // known buoy as an unidentified warship again.
        for c in &obs.contacts {
            if c.class_name.is_some() {
                if known_sensor_contact(c) {
                    self.identified_sensors.insert(c.id);
                } else {
                    self.identified_sensors.remove(&c.id);
                }
            }
        }
        self.ensure_roles(obs);
        for e in &obs.events {if let harpoon_core::EventKind::DeckBusy{carrier,reason,..}=&e.kind {
            if self.last_launch.get(carrier).is_some_and(|tick|e.tick>*tick) {
                let when=self.last_launch.remove(carrier).unwrap();
                if self.last_strike_wave.get(carrier)==Some(&when){self.last_strike_wave.remove(carrier);self.wave_targets.remove(carrier);}
                self.note(*carrier,None,"launch_rejected",format!("实际起飞被核心拒绝：{}；撤销本次出击冷却和未成立波次",reason),format!("Actual launch rejected: {}; roll back launch cooldown and unformed wave",reason));
            }
        }}
        if let Some(db)=&self.db {self.notes.extend(self.supervision.observe(obs,db));self.notes.extend(self.supervision.review_current(obs,db));}
        self.lock_threat_axis(obs);
        // This is an estimate from own confirmed kills and own observed losses,
        // never the referee's hidden opponent score.
        self.posture=if obs.exercise.is_some() {0}else{obs.victory.as_ref().map(|v| {
            let lost=v.own_losses_by_kind.iter().map(|(k,n)|v.kill_weights.as_ref().map(|w|w.for_kind(k)).unwrap_or(1.0)*(*n as f64)).sum::<f64>();
            if v.remaining_s<1800 && v.own_kill_points-lost>5.0 {1}
            else if v.remaining_s<3600 && v.own_kill_points-lost< -5.0 {2}else{0}
        }).unwrap_or(0)};
        let mut cmds=Vec::new();
        self.manage_aviation(obs, &mut cmds);
        for u in obs.units.iter().filter(|u|controllable(u)) {
            cmds.push(Command::SetEngagementPolicy {unit:u.id,air_only:u.kind!=PlatformKind::Sub,max_in_flight:if obs.contacts.iter().any(|c|c.category==PlatformKind::Missile && !c.stale && u.pos.range_to(c.estimate)<8.0){4}else{2}});
            if matches!(self.role_for(u),Role::GunScreen|Role::Carrier) {
                let aew_cover=obs.units.iter().any(|a|self.role_for(a)==Role::Aew && a.sensors.iter().any(|s|s.active && s.vs_air.is_some()));
                let imminent=obs.contacts.iter().any(|c|!c.stale && matches!(c.category,PlatformKind::Missile|PlatformKind::Aircraft) && u.pos.range_to(c.estimate)<45.0);
                let duty=(u.id.0 as i64%2)==(obs.tick/300%2);
                cmds.push(Command::SetRadar {unit:u.id,active:!aew_cover||imminent||duty});
            }
        }
        let carriers: Vec<_> = obs
            .units
            .iter()
            .filter(|u| self.role_for(u) == Role::Carrier)
            .collect();
        for u in obs
            .units
            .iter()
            .filter(|u| u.kind != PlatformKind::Aircraft && controllable(u))
        {
            let Some(mut mind) = self.minds.get(&u.id).cloned() else {
                continue;
            };
            let home = carriers
                .iter()
                .min_by(|a, b| u.pos.range_to(a.pos).total_cmp(&u.pos.range_to(b.pos)))
                .map(|a| a.pos)
                .unwrap_or(u.pos);
            if u.kind!=PlatformKind::Base && (mind.state == State::Retreat
                || u.hp / u.hp_max.max(1.0) <= self.params.retreat_hp_frac)
            {
                mind.state = State::Retreat;
                cmds.push(Command::ClearStation { unit: u.id });
                cmds.push(Command::Goto {
                    unit: u.id,
                    waypoints: vec![],
                });
                cmds.push(Command::SetCourse {
                    unit: u.id,
                    course_deg: home.bearing_to(u.pos),
                });
                cmds.push(Command::SetSpeed {
                    unit: u.id,
                    speed_kts: 18.0,
                });
                cmds.push(Command::ClearTarget { unit: u.id });
                self.note(
                    u.id,
                    None,
                    "retreat",
                    format!("{}：重伤退出阵位并后撤", u.name),
                    format!("{}: damaged; leave screen and withdraw", u.name),
                );
                self.minds.insert(u.id, mind);
                continue;
            }
            match mind.role {
                Role::Airbase => {
                    cmds.push(Command::ClearTarget {unit:u.id});
                    cmds.push(Command::SetWeaponsFree {unit:u.id,free:true});
                }
                Role::Carrier => {
                    if u.ordered_speed_kts < 1.0 {
                        cmds.push(Command::SetSpeed {
                            unit: u.id,
                            speed_kts: 18.0,
                        });
                    }
                    // Keep the fleet at stand-off rather than close a detected
                    // airborne contact as if it were the enemy fleet center.
                    let threatened = obs.contacts.iter().any(|c| {
                        c.category == PlatformKind::Ship
                            && !self.sensor_contact(c)
                            && !c.stale
                            && c.quality > 0.25
                            && u.pos.range_to(c.estimate) < match self.posture {1=>140.0,2=>70.0,_=>90.0}
                    });
                    if threatened {
                        cmds.push(Command::SetCourse {
                            unit: u.id,
                            course_deg: (self.threat_axis_deg + 180.0) % 360.0,
                        });
                        self.note(
                            u.id,
                            None,
                            "carrier_standoff",
                            format!("{}：向威胁轴后方机动，保持航空作战距离", u.name),
                            format!("{}: turn away to preserve air-operation stand-off", u.name),
                        );
                    }
                    cmds.push(Command::ClearTarget { unit: u.id });
                    if !u.fire_on {
                        cmds.push(Command::SetWeaponsFree {
                            unit: u.id,
                            free: true,
                        });
                    }
                }
                Role::Submarine => {
                    // Explicit torpedo salvos only. An assigned standing target
                    // otherwise triggers a second, uncontrolled auto salvo.
                    cmds.push(Command::ClearStation { unit: u.id });
                    cmds.push(Command::ClearTarget { unit: u.id });
                    cmds.push(Command::SetWeaponsFree {
                        unit: u.id,
                        free: false,
                    });
                    let armed=u.weapons.iter().any(|w|w.ammo>0 && matches!(self.kind_of(&w.weapon),Some(WeaponKind::Torpedo|WeaponKind::Ascm|WeaponKind::Slcm)));
                    if !armed {
                        // Exhaustion changes the standing task. Do not keep an
                        // empty submarine charging its last hostile contact.
                        cmds.push(Command::Goto {unit:u.id,waypoints:vec![]});
                        cmds.push(Command::SetRadar {unit:u.id,active:false});
                        cmds.push(Command::SetSonar {unit:u.id,active:false});
                        cmds.push(Command::SetSpeed {unit:u.id,speed_kts:self.params.sub_patrol_speed_kts});
                        if let Some((cid,_))=self.best_ship_target(obs,u) {
                            cmds.push(Command::SetCourse {unit:u.id,course_deg:(u.pos.bearing_to(contact_pos(obs,cid,u.pos))+180.0)%360.0});
                        }
                        mind.state=State::Withdraw;
                        self.note(u.id,None,"sub_reserve","进攻弹药耗尽：解除追击、关闭主动发射并低速撤离".into(),"offensive load empty: cancel pursuit, silence active emitters and withdraw slowly".into());
                    } else if mind.state == State::Withdraw && obs.tick - mind.last_salvo_tick < 120 {
                        cmds.push(Command::SetSpeed {
                            unit: u.id,
                            speed_kts: 18.0,
                        });
                    } else if let Some((contact, dist)) = self.best_ship_target(obs, u) {
                        let inflight = obs
                            .contacts
                            .iter()
                            .find(|c| c.id == contact)
                            .map(|c| c.own_in_flight)
                            .unwrap_or(0);
                        if let Some((weapon, _, ammo)) = self
                            .pick_weapon(u, &[WeaponKind::Torpedo], dist)
                            .filter(|_| inflight < 4 && obs.tick - mind.last_salvo_tick >= 120)
                        {
                            cmds.push(Command::Fire {
                                unit: u.id,
                                contact,
                                weapon,
                                rounds: ammo.min(2) as u8,
                            });
                            cmds.push(Command::SetCourse {
                                unit: u.id,
                                course_deg: (u.course_deg + 110.0) % 360.0,
                            });
                            cmds.push(Command::SetSpeed {
                                unit: u.id,
                                speed_kts: 18.0,
                            });
                            mind.state = State::Withdraw;
                            mind.last_salvo_tick = obs.tick;
                            self.note(
                                u.id,
                                Some(contact),
                                "sub_salvo",
                                format!("{}：向C{}下令鱼雷齐射后脱离", u.name, contact.0),
                                format!(
                                    "{}: order torpedo salvo on C{}, then withdraw",
                                    u.name, contact.0
                                ),
                            );
                        } else {
                            mind.state = State::OnStation;
                            cmds.push(Command::SetCourse {
                                unit: u.id,
                                course_deg: u.pos.bearing_to(contact_pos(obs, contact, u.pos)),
                            });
                            cmds.push(Command::SetSpeed {
                                unit: u.id,
                                // 093A sustains ~18kt submerged. Close at
                                // attack speed until inside the firing
                                // basket; the old 25nm patrol-speed limbo
                                // left subs creeping at 4kt a few miles
                                // short of torpedo release all battle.
                                speed_kts: if dist > self.params.sub_attack_range_nm {
                                    18.0
                                } else {
                                    self.params.sub_patrol_speed_kts
                                },
                            });
                            self.note(
                                u.id,
                                Some(contact),
                                "sub_approach",
                                format!("{}：接近C{}，等待射程与在飞弹药窗口", u.name, contact.0),
                                format!(
                                    "{}: approach C{}; await range and salvo window",
                                    u.name, contact.0
                                ),
                            );
                        }
                    } else {
                        cmds.push(Command::SetSpeed {
                            unit: u.id,
                            speed_kts: self.params.sub_patrol_speed_kts,
                        });
                    }
                }
                Role::GunScreen | Role::AswScreen | Role::Raider | Role::Other => {
                    // Distribute escorts about their nearest carrier; never
                    // abandon that carrier to chase a distant aircraft blip.
                    let group=obs.exercise.as_ref().and_then(|e|e.naval_groups.iter().find(|g|g.members.contains(&u.id)));
                    let group_leader=group.and_then(|g|obs.units.iter().find(|v|v.id==g.leader)
                        .or_else(||obs.units.iter().filter(|v|g.members.contains(&v.id)).min_by_key(|v|v.id)));
                    let leader=if group.is_some() {group_leader}else{carriers.iter().copied().filter(|_|u.kind==PlatformKind::Ship)
                        .min_by(|a,b|u.pos.range_to(a.pos).total_cmp(&u.pos.range_to(b.pos)))};
                    if let Some(leader)=leader.filter(|v|v.id!=u.id) {
                        let offset = if u.id.0 % 2 == 0 { 35.0 } else { -35.0 };
                        cmds.push(Command::Follow {
                            unit: u.id,
                            leader: leader.id,
                            bearing_rel_deg: (self.threat_axis_deg - leader.course_deg + offset)
                                .rem_euclid(360.0),
                            distance_nm: if mind.role == Role::AswScreen {
                                10.0
                            } else {
                                self.params.screen_standoff_nm
                            },
                        });
                    } else if let Some(g)=group {
                        cmds.push(Command::ClearStation {unit:u.id});
                        self.note(u.id,None,"group_independent",format!("{}：保持{}独立航路，不转投其他航母编队",u.name,g.name),format!("{}: retain {} independent route; do not merge with another carrier group",u.name,g.name));
                    }
                    if !u.fire_on {
                        cmds.push(Command::SetWeaponsFree {
                            unit: u.id,
                            free: true,
                        });
                    }
                    // Keep automatic air defence unpinned; deliberate surface
                    // salvos use Fire without overriding the air-defence target.
                    cmds.push(Command::ClearTarget { unit: u.id });
                    let target = self
                        .best_sub_target(obs, u)
                        .and_then(|(c, d)| {
                            self.pick_weapon(u, &[WeaponKind::AswRocket, WeaponKind::Lwt], d)
                                .map(|w| (c, w))
                        })
                        .or_else(|| {
                            self.best_ship_target(obs, u).and_then(|(c, d)| {
                                self.pick_weapon(u, &[WeaponKind::Ascm], d).map(|w| (c, w))
                            })
                        })
                        .or_else(|| {
                            self.best_base_target(obs, u).and_then(|(c, d)| {
                                self.pick_weapon(u, &[WeaponKind::Slcm], d).map(|w| (c, w))
                            })
                        });
                    if let Some((contact, (weapon, _, ammo))) = target {
                        let inflight = obs
                            .contacts
                            .iter()
                            .find(|c| c.id == contact)
                            .map(|c| c.own_in_flight)
                            .unwrap_or(0);
                        if obs.tick - mind.last_salvo_tick
                            >= self.params.surface_salvo_interval_s.max(30)
                            && inflight < self.params.surface_inflight_cap
                        {
                            cmds.push(Command::Fire {
                                unit: u.id,
                                contact,
                                weapon,
                                rounds: ammo.min(self.params.surface_salvo_rounds) as u8,
                            });
                            mind.last_salvo_tick = obs.tick;
                            self.note(
                                u.id,
                                Some(contact),
                                "screen_salvo",
                                format!("{}：保持屏卫，向C{}下令齐射", u.name, contact.0),
                                format!(
                                    "{}: hold screen and order salvo at C{}",
                                    u.name, contact.0
                                ),
                            );
                        }
                    }
                }
                _ => {}
            }
            self.minds.insert(u.id, mind);
        }
        // Reassociation/retasking above may have replaced a lost track. Review
        // the current package assignment, not a stale pre-planning snapshot.
        for command in &cmds {
            match command {
                Command::RetaskAircraft{unit,mission,..}=>if let Some(t)=self.supervision.tasks.get_mut(unit){t.contact=if let AirMission::Strike{contact}=mission{Some(*contact)}else{None};},
                Command::AssignTarget{unit,contact} if !obs.units.iter().any(|u|u.id==*unit&&u.mission.as_deref()==Some("strike"))=>if obs.contacts.iter().any(|c|c.id==*contact&&!c.stale&&!c.bearing_only&&c.quality>=self.params.strike_min_q){if let Some(t)=self.supervision.tasks.get_mut(unit){t.contact=Some(*contact);}},
                _=>{}
            }
        }
        if let Some(db)=&self.db {self.notes.extend(self.supervision.review_current(obs,db));let (maintenance,notes)=self.supervision.maintenance(obs,db);self.notes.extend(notes);cmds.extend(maintenance);}
        let cmds=self.coordinate_salvos(obs,cmds);
        let accepted=if let Some(db)=&self.db {
            let (cmds,notes)=self.supervision.guard(obs,db,cmds);self.notes.extend(notes);cmds
        }else{cmds};
        // Commit launch state only for orders that survived whole-batch policy.
        for command in &accepted {
            if let Command::LaunchAircraft{unit,mission,..}=command {
                self.last_launch.insert(*unit,obs.tick);
                if let AirMission::Strike{contact}=mission {self.last_strike_wave.insert(*unit,obs.tick);self.wave_targets.insert(*unit,*contact);}
            }
        }
        accepted
    }
}

/// Write private intent through the existing generic event carrier. No new
/// combat rule or event visibility exception is introduced.
pub fn record_notes(world: &mut harpoon_core::World, side: Side, notes: Vec<StaffNote>) {
    for note in notes {
        world.emit(
            harpoon_core::events::EventKind::RulesEvent {
                domain: "tactical_ai".into(),
                rule: note.action.clone(),
                side: Some(side),
                unit: Some(note.unit),
                contact: note.contact,
                detail: serde_json::to_string(&note).unwrap(),
            },
            harpoon_core::events::Visibility::Side(side),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harpoon_core::types::ContactId;
    use harpoon_core::world::Contact;
    use harpoon_core::{observe, scenario_from_json, Vec2};

    fn db() -> Database {
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
    }

    fn world(units: &str, wings: &str) -> harpoon_core::World {
        let src = format!(
            r#"{{"id":"tactical-test","name":"Tactical test","duration_s":3600,"seed":7,
               "context":{{"air_wings":{wings}}},
               "weather":{{"visibility_nm":60,"jam":[0,0]}},
               "units":[{units}]}}"#
        );
        harpoon_core::load_scenario(scenario_from_json(&src).unwrap(), db()).unwrap()
    }

    fn id_of(w: &harpoon_core::World, name: &str) -> harpoon_core::EntityId {
        w.entities.values().find(|e| e.name == name).unwrap().id
    }

    fn track(w: &mut harpoon_core::World, side: Side, about: harpoon_core::EntityId) -> ContactId {
        let state = &mut w.sides[side.index()];
        let cid = ContactId(state.next_contact + 1);
        state.next_contact += 1;
        state.contacts.insert(
            cid,
            Contact {
                id: cid,
                about,
                category: PlatformKind::Ship,
                class_name: Some("CVN".into()),
                estimate: w.entities[&about].pos,
                est_course_deg: 0.0,
                est_speed_kts: 0.0,
                bearing_only: false,
                quality: 1.0,
                last_update_tick: w.tick,
                first_tick: w.tick,
                stale: false,
                sensors: vec!["test".into()],
                offset: Vec2::zero(),
                announced: true,
            },
        );
        cid
    }

    fn fired_weapon(cmds: &[Command]) -> Option<&str> {
        cmds.iter().find_map(|c| match c {
            Command::Fire { weapon, .. } => Some(weapon.as_str()),
            _ => None,
        })
    }

    #[test]
    fn an_empty_submarine_cancels_pursuit_and_active_emissions() {
        let mut w=world(r#"{"side":"Blue","class":"cn_093","name":"SUB","pos":[0,0]},
            {"side":"Red","class":"us_ford","name":"TARGET","pos":[20,0]}"#,"{}");
        let target=id_of(&w,"TARGET");track(&mut w,Side::Blue,target);
        let mut o=observe(&w,Side::Blue,0,0);let sub=o.units[0].id;
        for m in &mut o.units[0].weapons {m.ammo=0;}
        let mut staff=TacticalStaff::with_db(Side::Blue,db());let c=staff.decide(&o);
        assert!(!c.iter().any(|c|matches!(c,Command::Fire{..})));
        assert!(c.iter().any(|c|matches!(c,Command::SetRadar{unit,active:false} if *unit==sub)));
        assert!(c.iter().any(|c|matches!(c,Command::SetSonar{unit,active:false} if *unit==sub)));
        assert!(staff.take_notes().iter().any(|n|n.action=="sub_reserve"));
    }
    #[test]fn declared_surface_groups_do_not_collapse_into_the_nearest_carrier() {
        let w=world(r#"{"side":"Blue","class":"cn_001","name":"CV","pos":[0,0]},
            {"side":"Blue","class":"cn_055","name":"LEAD","pos":[100,0]},
            {"side":"Blue","class":"cn_052d","name":"MEMBER","pos":[10,0]}"#,"{}");
        let mut o=observe(&w,Side::Blue,0,0);let leader=id_of(&w,"LEAD");let member=id_of(&w,"MEMBER");
        o.exercise=Some(serde_json::from_value(serde_json::json!({"locked":false,"alerted_at":null,"objectives":[],
            "required_carrier_points":30,"required_infrastructure_points":20,"naval_groups":[
                {"side":"Blue","name":"Independent","leader":leader,"members":[leader,member]}]})).unwrap());
        let mut staff=TacticalStaff::with_db(Side::Blue,db());let c=staff.decide(&o);
        assert!(c.iter().any(|c|matches!(c,Command::Follow{unit,leader:l,..} if *unit==member && *l==leader)));
        assert!(!c.iter().any(|c|matches!(c,Command::Follow{unit,..} if *unit==leader)));
        o.units.retain(|u|u.id!=leader);let c=staff.decide(&o);
        assert!(!c.iter().any(|c|matches!(c,Command::Follow{unit,..} if *unit==member)),"surviving member becomes independent leader instead of merging");
    }

    #[test]
    fn strikers_are_ordered_to_low_level_ingress() {
        let mut w = world(
            r#"{"side":"Blue","class":"cn_001","name":"CV","pos":[0,0]},
               {"side":"Red","class":"us_ford","name":"TARGET","pos":[120,0]}"#,
            r#"{"CV":[{"class":"cn_j15","count":4}]}"#,
        );
        let target = id_of(&w, "TARGET");
        track(&mut w, Side::Blue, target);
        let cv = id_of(&w, "CV");
        // 从甲板放飞一组突击机（引擎会按默认爬到 8000 m）。
        let strike = AirMission::Strike {
            contact: harpoon_core::types::ContactId(1),
        };
        assert!(harpoon_core::air::launch_aircraft(&mut w, cv, "cn_j15", 2, &strike).is_ok());
        let obs = observe(&w, Side::Blue, 0, 0);
        let mut staff = TacticalStaff::with_db(Side::Blue, db());
        let cmds = staff.decide(&obs);
        // §4.5.4 / p4-3: VLow is 21–60m; 50m is this AI's
        // selected flight profile, not a mandatory anti-ship rule.
        assert!(
            cmds.iter().any(|c| matches!(
                c,
                Command::SetAltitude { altitude_m, .. } if *altitude_m == 50.0
            )),
            "strikers must be ordered to low level; got {:?}",
            cmds
        );
    }

    #[test]
    fn strike_waits_for_descent_and_clears_route_on_rtb() {
        let mut w = world(
            r#"{"side":"Blue","class":"cn_001","name":"CV","pos":[0,0]},
               {"side":"Red","class":"us_ford","name":"TARGET","pos":[160,0]}"#,
            r#"{"CV":[{"class":"cn_j15","count":2}]}"#,
        );
        let cv = id_of(&w, "CV");
        let target = id_of(&w, "TARGET");
        let cid = track(&mut w, Side::Blue, target);
        harpoon_core::air::launch_aircraft(
            &mut w,
            cv,
            "cn_j15",
            2,
            &AirMission::Strike { contact: cid },
        )
        .unwrap();
        let ids: Vec<_> = w
            .entities
            .values()
            .filter(|e| e.kind == PlatformKind::Aircraft)
            .map(|e| e.id)
            .collect();
        let mut staff = TacticalStaff::with_db(Side::Blue, db());
        let first = staff.decide(&observe(&w, Side::Blue, 0, 0));
        assert!(ids.iter().all(|id| first.iter().any(
            |c| matches!(c,Command::Goto {unit,waypoints} if unit==id && !waypoints.is_empty())
        )));
        assert!(!first
            .iter()
            .any(|c| matches!(c,Command::Fire {unit,..} if ids.contains(unit))));
        for id in &ids {
            w.entities.get_mut(id).unwrap().mission = Some(harpoon_core::world::MissionState::Rtb);
        }
        let cmds = staff.decide(&observe(&w, Side::Blue, 0, 0));
        assert!(ids.iter().all(|id| cmds.iter().any(
            |c| matches!(c,Command::Goto {unit,waypoints} if unit==id && waypoints.is_empty())
        )));
        assert!(ids.iter().all(|id|cmds.iter().any(|c|matches!(c,Command::SetAltitude {unit,altitude_m} if unit==id && *altitude_m==3000.0))),"RTB phase must select cruise altitude rather than hold the ingress height");
    }

    #[test]
    fn cap_has_leash_and_bearing_only_tracks_do_not_authorize_interception() {
        let mut w = world(
            r#"{"side":"Blue","class":"cn_001","name":"CV","pos":[0,0]},
               {"side":"Blue","class":"cn_j15","name":"CAP","pos":[20,0]},
               {"side":"Red","class":"us_e2d","name":"AEW","pos":[140,0]}"#,
            "{}",
        );
        let target = id_of(&w, "AEW");
        let cap = id_of(&w, "CAP");
        let cid = track(&mut w, Side::Blue, target);
        let c = w.sides[0].contacts.get_mut(&cid).unwrap();
        c.category = PlatformKind::Aircraft;
        let mut staff = TacticalStaff::with_db(Side::Blue, db());
        let cmds = staff.decide(&observe(&w, Side::Blue, 0, 0));
        assert!(!cmds
            .iter()
            .any(|c| matches!(c,Command::AssignTarget {unit,..} if *unit==cap)));
        let c = w.sides[0].contacts.get_mut(&cid).unwrap();
        c.estimate = Vec2::new(30.0, 0.0);
        c.bearing_only = true;
        let cmds = staff.decide(&observe(&w, Side::Blue, 0, 0));
        assert!(!cmds
            .iter()
            .any(|c| matches!(c,Command::AssignTarget {unit,..} if *unit==cap)));
        w.sides[0].contacts.get_mut(&cid).unwrap().bearing_only = false;
        let cmds = staff.decide(&observe(&w, Side::Blue, 0, 0));
        assert!(cmds.iter().any(
            |c| matches!(c,Command::AssignTarget {unit,contact} if *unit==cap && *contact==cid)
        ));
    }

    #[test]
    fn one_launch_per_deck_ready_count_and_defensive_cap_first() {
        let w = world(
            r#"{"side":"Blue","class":"cn_001","name":"CV","pos":[0,0]},
                        {"side":"Red","class":"us_ford","name":"TARGET","pos":[160,0]}"#,
            r#"{"CV":[{"class":"cn_j15","count":1},{"class":"cn_z18f","count":2}]}"#,
        );
        let mut staff = TacticalStaff::with_db(Side::Blue, db());
        let cmds = staff.decide(&observe(&w, Side::Blue, 0, 0));
        let launches: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, Command::LaunchAircraft { .. }))
            .collect();
        assert_eq!(launches.len(), 1);
        assert!(matches!(
            launches[0],
            Command::LaunchAircraft {
                count: 1,
                mission: AirMission::Cap { .. },
                ..
            }
        ));
    }

    #[test]
    fn action_notes_are_private_deduplicated_and_hidden_truth_does_not_affect_orders() {
        let mut w = world(
            r#"{"side":"Blue","class":"cn_001","name":"CV","pos":[0,0]},
                            {"side":"Red","class":"us_ford","name":"HIDDEN","pos":[160,0]}"#,
            r#"{"CV":[{"class":"cn_j15","count":4}]}"#,
        );
        let obs = observe(&w, Side::Blue, 0, 0);
        let mut a = TacticalStaff::with_db(Side::Blue, db());
        let orders = a.decide(&obs);
        let notes = a.take_notes();
        assert!(!notes.is_empty());
        let hidden = id_of(&w, "HIDDEN");
        w.entities.get_mut(&hidden).unwrap().pos = Vec2::new(500.0, 500.0);
        let mut b = TacticalStaff::with_db(Side::Blue, db());
        assert_eq!(
            serde_json::to_value(orders).unwrap(),
            serde_json::to_value(b.decide(&observe(&w, Side::Blue, 0, 0))).unwrap()
        );
        record_notes(&mut w, Side::Blue, notes);
        assert!(observe(&w,Side::Blue,0,usize::MAX).events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent {domain,..} if domain=="tactical_ai")));
        assert!(!observe(&w,Side::Red,0,usize::MAX).events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent {domain,..} if domain=="tactical_ai")));
        a.decide(&obs);
        assert!(
            a.take_notes().is_empty(),
            "unchanged orders must not repeat their explanation"
        );
    }

    #[test]
    fn sensor_buoys_never_receive_movement_or_weapons_free_orders() {
        let w = world(
            r#"{"side":"Blue","class":"cn_001","name":"CV","pos":[0,0]},
                        {"side":"Red","class":"us_ford","name":"TARGET","pos":[160,0]}"#,
            "{}",
        );
        let mut obs = observe(&w, Side::Blue, 0, 0);
        let mut buoy = obs.units[0].clone();
        buoy.id = EntityId(999);
        buoy.class = harpoon_core::rules::asw::SONOBUOY_CLASS.into();
        buoy.deck = None;
        buoy.weapons.clear();
        buoy.fire_on = false;
        obs.units.push(buoy);
        let mut staff = TacticalStaff::with_db(Side::Blue, db());
        for c in staff.decide(&obs) {
            assert_ne!(
                serde_json::to_value(c).unwrap()["unit"],
                serde_json::json!(999)
            );
        }
    }

    #[test]
    fn identified_buoys_never_attract_a_carrier_strike_or_set_the_threat_axis() {
        let mut w = world(
            r#"{"side":"Blue","class":"cn_001","name":"CV","pos":[0,0]},
                            {"side":"Red","class":"us_ford","name":"TARGET","pos":[120,0]}"#,
            r#"{"CV":[{"class":"cn_j15","count":4}]}"#,
        );
        let cv = id_of(&w, "CV");
        let target = id_of(&w, "TARGET");
        let real = track(&mut w, Side::Blue, target);
        w.sides[0].contacts.get_mut(&real).unwrap().class_name = Some("Destroyer".into());
        let mut buoy = w.sides[0].contacts[&real].clone();
        buoy.id = ContactId(99);
        buoy.class_name = Some("Sonobuoy".into());
        buoy.estimate = Vec2::new(0.0, 20.0);
        w.sides[0].contacts.insert(buoy.id, buoy);
        harpoon_core::air::launch_aircraft(
            &mut w,
            cv,
            "cn_j15",
            2,
            &AirMission::Cap {
                station: Vec2::zero(),
                radius_nm: 15.0,
                follow: true,
            },
        )
        .unwrap();
        let mut staff = TacticalStaff::with_db(Side::Blue, db());
        let cmds = staff.decide(&observe(&w, Side::Blue, 0, 0));
        assert!(cmds.iter().any(|c|matches!(c,Command::LaunchAircraft {mission:AirMission::Strike {contact},..} if *contact==real)));
        assert!((staff.threat_axis_deg - 90.0).abs() < 0.01);
        assert!(!cmds
            .iter()
            .any(|c| matches!(c,Command::SetCourse {unit,..} if *unit==cv)));
        w.sides[0].contacts.remove(&real);
        let mut staff = TacticalStaff::with_db(Side::Blue, db());
        let cmds = staff.decide(&observe(&w, Side::Blue, 0, 0));
        assert!(!cmds.iter().any(|c| matches!(
            c,
            Command::LaunchAircraft {
                mission: AirMission::Strike { .. },
                ..
            }
        )));
        // Re-sharing the track through TDL can lose the class label. The
        // commander must remember the identification it already received.
        w.sides[0]
            .contacts
            .get_mut(&ContactId(99))
            .unwrap()
            .class_name = None;
        let cmds = staff.decide(&observe(&w, Side::Blue, 0, 0));
        assert!(!cmds.iter().any(|c| matches!(
            c,
            Command::LaunchAircraft {
                mission: AirMission::Strike { .. },
                ..
            }
        )));
    }

    #[test]
    fn stale_strike_searches_observed_last_position_but_only_fires_after_reacquisition() {
        let mut w = world(
            r#"{"side":"Blue","class":"cn_001","name":"CV","pos":[0,0]},
                            {"side":"Red","class":"us_ford","name":"TARGET","pos":[150,0]}"#,
            r#"{"CV":[{"class":"cn_j15","count":2}]}"#,
        );
        let cv = id_of(&w, "CV");
        let target = id_of(&w, "TARGET");
        let cid = track(&mut w, Side::Blue, target);
        harpoon_core::air::launch_aircraft(
            &mut w,
            cv,
            "cn_j15",
            2,
            &AirMission::Strike { contact: cid },
        )
        .unwrap();
        let ids: Vec<_> = w
            .entities
            .values()
            .filter(|u| u.kind == PlatformKind::Aircraft)
            .map(|u| u.id)
            .collect();
        let mut staff = TacticalStaff::with_db(Side::Blue, db());
        let orders = staff.decide(&observe(&w, Side::Blue, 0, 0));
        let station = orders
            .iter()
            .find_map(|c| {
                if let Command::Goto { unit, waypoints } = c {
                    if ids.contains(unit) {
                        waypoints.first().copied()
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap();
        w.tick = 600;
        for id in &ids {
            let u = w.entities.get_mut(id).unwrap();
            u.pos = station;
            u.altitude_m = 50.0;
        }
        w.sides[0].contacts.get_mut(&cid).unwrap().stale = true;
        let orders = staff.decide(&observe(&w, Side::Blue, 0, 0));
        assert!(!orders
            .iter()
            .any(|c| matches!(c,Command::Fire {unit,..} if ids.contains(unit))));
        assert!(orders.iter().any(|c|matches!(c,Command::Goto {unit,waypoints} if ids.contains(unit) && waypoints.first().is_some_and(|p|p.x>100.0))));
        let c = w.sides[0].contacts.get_mut(&cid).unwrap();
        c.stale = false;
        c.last_update_tick = w.tick;
        for id in &ids {
            let aircraft=w.entities.get_mut(id).unwrap();
            aircraft.pos = Vec2::new(130.0, 0.0);
            aircraft.altitude_m=81.0; // VLow ingress, then a legal Low release.
        }
        let orders = staff.decide(&observe(&w, Side::Blue, 0, 0));
        assert!(!staff.pending_salvos.is_empty()||orders.iter().any(|c|matches!(c,Command::Fire{unit,contact,..}if ids.contains(unit)&&*contact==cid)),"reacquired strike must schedule or submit an actual release");
        w.tick+=90;
        let mut orders=orders;
        orders.extend(staff.decide(&observe(&w,Side::Blue,0,0)));
        assert!(orders.iter().any(
            |c| matches!(c,Command::Fire {unit,contact,..} if ids.contains(unit) && *contact==cid)
        ));
    }

    #[test]
    fn submarine_fires_its_actual_torpedo_mount() {
        let mut w = world(
            r#"{"side":"Blue","class":"cn_093a","name":"SUB","pos":[0,0]},
               {"side":"Red","class":"us_ford","name":"TARGET","pos":[15,0]}"#,
            "{}",
        );
        let target = id_of(&w, "TARGET");
        track(&mut w, Side::Blue, target);
        let obs = observe(&w, Side::Blue, 0, 0);
        let mut staff = TacticalStaff::with_db(Side::Blue, db());
        let cmds = staff.decide(&obs);
        assert_eq!(
            fired_weapon(&cmds),
            Some("cn_yu10"),
            "must use the equipped Yu-10, not a synthetic torp_homing id; got {:?}",
            cmds
        );
    }

    #[test]
    fn surface_ship_fires_its_actual_anti_ship_mount() {
        let mut w = world(
            r#"{"side":"Blue","class":"cn_055","name":"CG","pos":[0,0]},
               {"side":"Red","class":"us_ford","name":"TARGET","pos":[80,0]}"#,
            "{}",
        );
        let target = id_of(&w, "TARGET");
        track(&mut w, Side::Blue, target);
        let obs = observe(&w, Side::Blue, 0, 0);
        let mut staff = TacticalStaff::with_db(Side::Blue, db());
        let mut cmds = staff.decide(&obs);
        for t in [30,60,90,120,150,180] {
            w.tick=t;
            cmds.extend(staff.decide(&observe(&w,Side::Blue,0,0)));
        }
        assert_eq!(
            fired_weapon(&cmds),
            Some("cn_yj18"),
            "must fire the installed YJ-18; got {:?}",
            cmds
        );
    }

    #[test]
    fn carrier_launches_only_anti_ship_capable_strike_aircraft() {
        let mut w = world(
            r#"{"side":"Blue","class":"cn_001","name":"CV","pos":[0,0]},
               {"side":"Red","class":"us_ford","name":"TARGET","pos":[120,0]}"#,
            r#"{"CV":[{"class":"cn_j15","count":4}]}"#,
        );
        let target = id_of(&w, "TARGET");
        track(&mut w, Side::Blue, target);
        let cv = id_of(&w, "CV");
        harpoon_core::air::launch_aircraft(
            &mut w,
            cv,
            "cn_j15",
            2,
            &AirMission::Cap {
                station: Vec2::zero(),
                radius_nm: 15.0,
                follow: true,
            },
        )
        .unwrap();
        let obs = observe(&w, Side::Blue, 0, 0);
        let mut staff = TacticalStaff::with_db(Side::Blue, db());
        let cmds = staff.decide(&obs);
        assert!(
            cmds.iter().any(|c| matches!(
                c,
                Command::LaunchAircraft { class, mission: AirMission::Strike { .. }, .. }
                    if class == "cn_j15"
            )),
            "J-15 carries YJ-83K and should be launched on strike; got {:?}",
            cmds
        );
    }
    #[test]
    #[ignore = "3-hour dual-staff soak (~50 s debug); run explicitly for full coverage"]
    fn dual_staff_runs_three_hour_carrier_duel() {
        let db = db();
        let sc =
            scenario_from_json(include_str!("../../../scenarios/carrier_duel_3h.json")).unwrap();
        let mut world = harpoon_core::load_scenario(sc, db.clone()).unwrap();
        let mut staffs = [
            TacticalStaff::with_db(Side::Blue, db.clone()),
            TacticalStaff::with_db(Side::Red, db.clone()),
        ];
        for t in 0..10_800 {
            let mut cmds = Vec::new();
            if t % 30 == 0 {
                for (i, staff) in staffs.iter_mut().enumerate() {
                    let side = if i == 0 { Side::Blue } else { Side::Red };
                    let obs = observe(&world, side, 0, 0);
                    cmds.extend(staff.decide(&obs).into_iter().map(|c| (side, c)));
                }
            }
            harpoon_core::sim::tick(&mut world, &cmds);
            if world.over {
                break;
            }
        }
        assert!(world.over, "three-hour exercise must reach a decision");
        let mut missions: BTreeMap<String, usize> = Default::default();
        let mut weapons: BTreeMap<String, usize> = Default::default();
        let mut chaff = 0usize;
        for e in &world.events {
            match &e.kind {
                harpoon_core::events::EventKind::AirLaunch { mission, .. } => {
                    *missions.entry(mission.clone()).or_default() += 1;
                }
                harpoon_core::events::EventKind::WeaponsRelease { weapon, .. } => {
                    let key = weapon.to_ascii_lowercase();
                    if key.contains("torp") || key.contains("mk48") || key.contains("yu") {
                        // Count only anti-ship missiles below.
                    } else if key.contains("yj")
                        || key.contains("harpoon")
                        || key.contains("agm84")
                        || key.contains("asm")
                    {
                        *weapons.entry(weapon.clone()).or_default() += 1;
                    }
                }
                harpoon_core::events::EventKind::ChaffDeployed { unit, .. } => {
                    chaff += 1;
                    let _ = unit;
                }
                _ => {}
            }
        }
        assert!(
            missions.keys().any(|m| m == "strike"),
            "carrier strike waves must launch"
        );
        assert!(
            !weapons.is_empty(),
            "surface/air anti-ship launches must occur"
        );
        assert!(chaff > 0, "automatic chaff must fire at least once");
        assert!(
            missions.keys().any(|m| m == "asw"),
            "standing ASW barrier must launch helos even without sub contacts"
        );
    }
}

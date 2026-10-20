//! TacticalStaff — a state-machine naval doctrine staff.
//!
//! Doctrine basis: docs/design/naval-doctrine.md (P1..P8), distilled from the
//! Harpoon V rulebook and modern carrier-group practice.
//!
//! Each friendly unit is assigned a role once (carrier, AEW, CAP, ASW screen,
//! gun screen, striker, submarine, raider) and follows a per-role finite state
//! machine:
//!
//!   ON_STATION -> ENGAGE -> (WITHDRAW | RETREAT)
//!
//! with global coordination rules: CAP rotation, AEW standing patrol, a
//! strike window where every shooter with a solution fires together, and a
//! retreat rule for badly damaged ships. Tunables live in `Params`
//! (data/ai/tactical_params.json overrides defaults; the self-play tuner
//! writes improved sets back).

use crate::ai::CommandStaff;
use harpoon_core::commands::{AirMission, Command};
use harpoon_core::observe::{Observation, UnitObs};
use harpoon_core::types::{ContactId, EntityId, PlatformKind, Side};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
}

impl Default for Params {
    fn default() -> Self {
        Self {
            cap_min_size: 2,
            cap_fuel_rtb_s: 900.0,
            aew_standoff_nm: 40.0,
            strike_min_q: 0.4,
            screen_standoff_nm: 16.0,
            retreat_hp_frac: 0.3,
            sub_attack_range_nm: 9.0,
            sub_patrol_speed_kts: 4.0,
            salvo_min_interval_s: 20,
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
    Aew,
    Cap,
    AswScreen,
    GunScreen,
    Striker,
    Submarine,
    Raider,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    OnStation,
    Withdraw,
    Retreat,
}

#[derive(Clone, Copy)]
struct UnitMind {
    role: Role,
    state: State,
    last_salvo_tick: i64,
}

fn is_aew(u: &UnitObs) -> bool {
    let n = u.name.to_ascii_uppercase();
    let c = u.class.to_ascii_lowercase();
    n.contains("AEW") || c.contains("aew") || c.contains("e2") || c.contains("kj600") || c.contains("e3")
}

fn is_helo(u: &UnitObs) -> bool {
    let n = u.name.to_ascii_uppercase();
    let c = u.class.to_ascii_lowercase();
    n.contains("MH-") || n.contains("SH-") || n.contains("Z-") || n.contains("卡-")
        || c.contains("helo") || c.contains("z18") || c.contains("z9") || c.contains("merlin")
        || c.contains("ch14") || c.contains("ch148")
}

fn contact_pos(obs: &Observation, id: ContactId, fallback: harpoon_core::Vec2) -> harpoon_core::Vec2 {
    obs.contacts
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.estimate)
        .unwrap_or(fallback)
}

fn role_for(u: &UnitObs) -> Role {
    if u.deck.is_some() && u.kind == PlatformKind::Ship {
        return Role::Carrier;
    }
    match u.kind {
        PlatformKind::Sub => Role::Submarine,
        PlatformKind::Aircraft => {
            if is_aew(u) {
                Role::Aew
            } else if is_helo(u) {
                Role::AswScreen
            } else {
                let aam: u16 = u
                    .weapons
                    .iter()
                    .filter(|w| w.weapon.starts_with("aam"))
                    .map(|w| w.ammo)
                    .sum();
                let aship: u16 = u
                    .weapons
                    .iter()
                    .filter(|w| w.weapon.starts_with("asm") || w.weapon.starts_with("bomb"))
                    .map(|w| w.ammo)
                    .sum();
                if aship > aam { Role::Striker } else { Role::Cap }
            }
        }
        PlatformKind::Ship => {
            let aa = u.weapons.iter().any(|w| w.weapon.starts_with("sam"));
            let asw = u.sensors.iter().any(|s| s.sensor.contains("sonar_p_tow"))
                || u.weapons.iter().any(|w| w.weapon.starts_with("asw"));
            let raider_gun = u.weapons.iter().any(|w| w.weapon.contains("ascm"));
            if aa {
                Role::GunScreen
            } else if asw {
                Role::AswScreen
            } else if raider_gun {
                Role::Raider
            } else {
                Role::Other
            }
        }
        _ => Role::Other,
    }
}

pub struct TacticalStaff {
    #[allow(dead_code)]
    side: Side,
    params: Params,
    minds: BTreeMap<EntityId, UnitMind>,
    roles_assigned: bool,
    /// 威胁轴：从己方锚点指向最早一次对敌探测的方位（度，0=北，顺时针）。
    threat_axis_deg: f64,
    threat_axis_locked: bool,
    /// 每艘航母上次组织打击波次的 tick（P4 多域协同窗口）。
    last_strike_wave: BTreeMap<EntityId, i64>,
}

impl TacticalStaff {
    pub fn new(side: Side) -> Self {
        Self {
            side,
            params: Params::load_optional(),
            minds: BTreeMap::new(),
            roles_assigned: false,
            threat_axis_deg: 90.0,
            threat_axis_locked: false,
            last_strike_wave: BTreeMap::new(),
        }
    }

    fn ensure_roles(&mut self, obs: &Observation) {
        if self.roles_assigned {
            return;
        }
        for u in &obs.units {
            self.minds.insert(
                u.id,
                UnitMind { role: role_for(u), state: State::OnStation, last_salvo_tick: -100_000 },
            );
        }
        self.roles_assigned = true;
    }

    /// P1: 威胁轴由最早一次对敌探测决定，之后锁定（避免自旋）。
    fn lock_threat_axis(&mut self, obs: &Observation) {
        if self.threat_axis_locked || obs.contacts.is_empty() {
            return;
        }
        let Some(anchor) = obs.units.first() else { return };
        self.threat_axis_deg = anchor.pos.bearing_to(obs.contacts[0].estimate);
        self.threat_axis_locked = true;
    }

    fn standoff_point(&self, anchor: &harpoon_core::Vec2, distance: f64) -> harpoon_core::Vec2 {
        anchor.advanced(self.threat_axis_deg, distance)
    }

    /// P8: 打击目标选择——按存活价值排序，敌预警机双倍权重；返回 (接触, 距离, 分)。
    fn best_strike_target(&self, obs: &Observation, shooter: &UnitObs) -> Option<(ContactId, f64, f64)> {
        let mut best: Option<(ContactId, f64, f64)> = None;
        for c in &obs.contacts {
            if c.stale || c.quality < self.params.strike_min_q {
                continue;
            }
            let value = match c.class_name.as_deref() {
                Some(n) if n.contains("CV") || n.contains("Carrier") || n.contains("FORD") || n.contains("NIMITZ") => 60.0,
                Some(n) if n.contains("AEW") || n.contains("E-2") || n.contains("KJ") => 50.0,
                Some(n) if n.contains("Burke") || n.contains("055") || n.contains("052") || n.contains("Ticonderoga") => 30.0,
                _ => 15.0,
            };
            let dist = shooter.pos.range_to(c.estimate);
            let score = value - dist * 0.08;
            if best.map(|(_, _, s)| score > s).unwrap_or(true) {
                best = Some((c.id, dist, score));
            }
        }
        best
    }

    /// CAP 的对空火控解目标：最近的、有雷达解的敌机。
    fn best_air_target(&self, obs: &Observation, shooter: &UnitObs) -> Option<ContactId> {
        obs.contacts
            .iter()
            .filter(|c| c.category == PlatformKind::Aircraft && !c.stale && c.quality > 0.4)
            .min_by(|a, b| {
                shooter
                    .pos
                    .range_to(a.estimate)
                    .partial_cmp(&shooter.pos.range_to(b.estimate))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|c| c.id)
    }

    /// 有合格解的水面/岸上目标（供舰射反舰）。
    fn best_surface_target(&self, obs: &Observation, shooter: &UnitObs) -> Option<(ContactId, f64)> {
        obs.contacts
            .iter()
            .filter(|c| !c.stale && c.quality >= self.params.strike_min_q)
            .filter(|c| matches!(c.category, PlatformKind::Ship | PlatformKind::Base))
            .map(|c| (c.id, shooter.pos.range_to(c.estimate)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// P8: 岸上目标（SLCM/ASCM 对地）。
    fn best_base_target(&self, obs: &Observation, shooter: &UnitObs) -> Option<(ContactId, f64)> {
        obs.contacts
            .iter()
            .filter(|c| !c.stale && c.quality >= self.params.strike_min_q
                && c.category == PlatformKind::Base)
            .map(|c| (c.id, shooter.pos.range_to(c.estimate)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// 反潜目标：最近的非失效潜艇接触。
    fn best_sub_target(&self, obs: &Observation, shooter: &UnitObs) -> Option<(ContactId, f64)> {
        obs.contacts
            .iter()
            .filter(|c| !c.stale && c.quality > 0.4 && c.category == PlatformKind::Sub)
            .map(|c| (c.id, shooter.pos.range_to(c.estimate)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    fn anchor_of<'a>(&'a self, obs: &'a Observation) -> Option<&'a UnitObs> {
        obs.units
            .iter()
            .find(|u| self.minds.get(&u.id).map(|m| m.role) == Some(Role::Carrier))
            .or_else(|| obs.units.iter().find(|u| u.kind == PlatformKind::Ship))
    }
}

impl CommandStaff for TacticalStaff {
    fn name(&self) -> &'static str {
        "tactical_fsm"
    }

    fn decide(&mut self, obs: &Observation) -> Vec<Command> {
        if obs.over {
            return Vec::new();
        }
        self.ensure_roles(obs);
        self.lock_threat_axis(obs);

        let p = self.params.clone();
        let mut cmds: Vec<Command> = Vec::new();

        // P3: CAP 轮换——空中 CAP 机少于阈值时命令航母补飞。
        let cap_airborne = obs
            .units
            .iter()
            .filter(|u| u.kind == PlatformKind::Aircraft && role_for(u) == Role::Cap)
            .count() as u32;
        if cap_airborne < p.cap_min_size {
            for carrier in obs.units.iter().filter(|u| {
                u.deck.is_some() && self.minds.get(&u.id).map(|m| m.role) == Some(Role::Carrier)
            }) {
                if let Some(deck) = &carrier.deck {
                    if let Some((class, _)) = deck
                        .ready
                        .iter()
                        .find(|(c, n)| *n > 0 && (c.contains("fighter") || c.contains("f18") || c.contains("f35") || c.contains("j15")))
                    {
                        cmds.push(Command::LaunchAircraft {
                            unit: carrier.id,
                            class: class.clone(),
                            count: 2,
                            mission: AirMission::Cap {
                                station: carrier.pos.advanced(self.threat_axis_deg, 30.0),
                                radius_nm: 15.0,
                            },
                        });
                        break;
                    }
                }
            }
        }

        // 威胁轴前出点（锚点缺省时用编队首个单位）。
        let anchor_pos = self
            .anchor_of(obs)
            .map(|a| a.pos)
            .or_else(|| obs.units.first().map(|u| u.pos));

        for u in &obs.units {
            // P0 战略机动：有接触就水面/潜艇向敌推进。
            let nearest_contact = obs.contacts.iter()
                .filter(|c| !c.stale && c.quality > 0.25)
                .map(|c| (c, u.pos.range_to(c.estimate)))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let is_carrier = self.minds.get(&u.id).map(|m| m.role) == Some(Role::Carrier);
            let advancing = nearest_contact.is_some() && !is_carrier && u.kind != PlatformKind::Aircraft;
            if advancing {
                if let Some((c, dist)) = nearest_contact {
                    let bearing = u.pos.bearing_to(c.estimate);
                    match u.kind {
                        PlatformKind::Sub => {
                            if dist > 12.0 {
                                cmds.push(Command::ClearStation { unit: u.id });
                                cmds.push(Command::SetCourse { unit: u.id, course_deg: bearing });
                                cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 12.0 });
                            }
                        }
                        PlatformKind::Ship => {
                            if dist > 18.0 {
                                // 解除编队锁定（follow 覆盖 SetCourse/SetSpeed）
                                cmds.push(Command::ClearStation { unit: u.id });
                                cmds.push(Command::SetCourse { unit: u.id, course_deg: bearing });
                                cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 28.0 });
                            }
                            let ascm = u.weapons.iter()
                                .find(|w| (w.weapon.contains("yj") || w.weapon.contains("p700")
                                    || w.weapon.contains("p270") || w.weapon.contains("ascm")
                                    || w.weapon.starts_with("asm")) && w.ammo > 0);
                            if let Some(w) = ascm {
                                if dist <= 55.0 {
                                    // 集火：优先打 BDA 显示损伤最重的已接触目标
                                    let focus = obs.contacts.iter()
                                        .filter(|fc| fc.id == c.id
                                            || (fc.category == PlatformKind::Ship && !fc.stale && fc.quality > 0.25
                                                && u.pos.range_to(fc.estimate) <= 55.0))
                                        .min_by(|a, b| a.bda_hp_fraction.unwrap_or(1.0)
                                            .total_cmp(&b.bda_hp_fraction.unwrap_or(1.0)))
                                        .map(|fc| fc.id)
                                        .unwrap_or(c.id);
                                    cmds.push(Command::AssignTarget { unit: u.id, contact: focus });
                                    cmds.push(Command::Fire {
                                        unit: u.id, contact: focus,
                                        weapon: w.weapon.clone(), rounds: 2,
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            // 全局撤退规则：重伤舰退出阵位（P7）。撤退状态是"粘性"的：
            // 一旦进入就不再退出（用 minds 表写回，下一 tick 生效）。
            let retreating = self
                .minds
                .get(&u.id)
                .map(|m| m.state == State::Retreat)
                .unwrap_or(false)
                || (u.kind != PlatformKind::Aircraft
                    && u.hp / u.hp_max.max(1.0) <= p.retreat_hp_frac);
            if retreating {
                self.minds.get_mut(&u.id).map(|m| m.state = State::Retreat);
                let home = anchor_pos.unwrap_or(u.pos);
                cmds.push(Command::SetCourse { unit: u.id, course_deg: u.pos.bearing_to(home) });
                cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 25.0 });
                cmds.push(Command::ClearTarget { unit: u.id });
                continue;
            }
            let Some(mind) = self.minds.get(&u.id).cloned() else { continue };

            // 舰载机一律按引擎任务逻辑飞行（air_missions/弹射时给定），
            // 参谋只负责目标指派与打击波次组织。
            if u.kind == PlatformKind::Aircraft {
                match mind.role {
                    Role::Cap => {
                        if let Some(contact) = self.best_air_target(obs, u) {
                            cmds.push(Command::AssignTarget { unit: u.id, contact });
                        } else if let Some((contact, _, _)) = self.best_strike_target(obs, u) {
                            cmds.push(Command::AssignTarget { unit: u.id, contact });
                        }
                    }
                    Role::AswScreen => {
                        if let Some((contact, dist)) = self.best_sub_target(obs, u) {
                            if dist < 6.0 {
                                cmds.push(Command::AssignTarget { unit: u.id, contact });
                            }
                        }
                    }
                    _ => {}
                }
                continue;
            }

            // 水面/水下单位的状态机。
            let mut mind = mind;
            match mind.role {
                Role::Carrier => {
                    if u.ordered_speed_kts < 1.0 {
                        cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 18.0 });
                    }
                    // P4/P8: 组织打击波次——甲板有就绪打击机且存在合格接触
                    // （质量达标、距离在打击包线内），每 3 分钟一波。
                    let Some(deck) = &u.deck else { continue };
                    let last = self.last_strike_wave.get(&u.id).copied().unwrap_or(-100_000);
                    if obs.tick - last < 180 {
                        continue;
                    }
                    let anchor_here = u.pos;
                    let good = obs.contacts.iter().any(|c| {
                        !c.stale
                            && c.quality >= p.strike_min_q
                            && anchor_here.range_to(c.estimate) <= 200.0
                            && matches!(c.category, PlatformKind::Ship | PlatformKind::Base)
                    });
                    if !good {
                        continue;
                    }
                    let Some(target_contact) = obs
                        .contacts
                        .iter()
                        .filter(|c| {
                            !c.stale
                                && c.quality >= p.strike_min_q
                                && anchor_here.range_to(c.estimate) <= 200.0
                                && matches!(c.category, PlatformKind::Ship | PlatformKind::Base)
                        })
                        .map(|c| c.id)
                        .next()
                    else {
                        continue;
                    };
                    let Some((class, _)) = deck
                        .ready
                        .iter()
                        .find(|(c, n)| {
                            *n > 0 && (c.contains("striker") || c.contains("bomber")
                                || c.contains("j15") || c.contains("f18") || c.contains("f35"))
                        })
                    else {
                        continue;
                    };
                    cmds.push(Command::LaunchAircraft {
                        unit: u.id,
                        class: class.clone(),
                        count: 2,
                        mission: AirMission::Strike { contact: target_contact },
                    });
                    self.last_strike_wave.insert(u.id, obs.tick);
                }

                Role::Aew => {
                    // 预警机按想定 air_missions 前出；若想定未给任务则参谋补一个。
                    if u.mission.is_none() {
                        if let Some(a) = anchor_pos {
                            let standoff = self.standoff_point(&a, p.aew_standoff_nm);
                            cmds.push(Command::SetCourse {
                                unit: u.id,
                                course_deg: u.pos.bearing_to(standoff),
                            });
                            cmds.push(Command::SetSpeed {
                                unit: u.id,
                                speed_kts: unit_obs_ext::cruise(u),
                            });
                        }
                    }
                }

                Role::GunScreen => {
                    let standoff = self.standoff_point(
                        &anchor_pos.unwrap_or(u.pos),
                        p.screen_standoff_nm,
                    );
                    cmds.push(Command::SetCourse { unit: u.id, course_deg: u.pos.bearing_to(standoff) });
                    cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 18.0 });
                    if !u.fire_on {
                        cmds.push(Command::SetWeaponsFree { unit: u.id, free: true });
                    }
                    // P8: 对空 > 水面 > 岸上（SLCM 对地）。
                    if let Some(contact) = self.best_air_target(obs, u) {
                        cmds.push(Command::AssignTarget { unit: u.id, contact });
                    } else if let Some((contact, _)) = self.best_surface_target(obs, u) {
                        cmds.push(Command::AssignTarget { unit: u.id, contact });
                    } else if let Some((base_contact, dist)) = self.best_base_target(obs, u) {
                        cmds.push(Command::AssignTarget { unit: u.id, contact: base_contact });
                        // SLCM 对地主动开火（射程内且弹药未耗尽）。
                        let slcm = u.weapons.iter().find(|w| w.weapon == "slcm");
                        if let Some(m) = slcm {
                            if m.ammo > 0 && m.cooldown_s <= 0.0 && dist <= 160.0 {
                                cmds.push(Command::Fire {
                                    unit: u.id,
                                    contact: base_contact,
                                    weapon: "slcm".into(),
                                    rounds: 1,
                                });
                            }
                        }
                    } else {
                        cmds.push(Command::ClearTarget { unit: u.id });
                    }
                }

                Role::AswScreen => {
                    // P6: 反潜屏卫限速 15 节保拖曳阵；发现潜艇接触则指派给
                    // 舰上的 ASW 武器（火箭/轻鱼雷）自动接战。
                    cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 15.0 });
                    if let Some((contact, _)) = self.best_sub_target(obs, u) {
                        cmds.push(Command::AssignTarget { unit: u.id, contact });
                    }
                }

                Role::Submarine => {
                    if let Some((contact, dist, _)) = self.best_strike_target(obs, u) {
                        if dist <= p.sub_attack_range_nm {
                            cmds.push(Command::AssignTarget { unit: u.id, contact });
                            cmds.push(Command::Fire {
                                unit: u.id,
                                contact,
                                weapon: "torp_homing".into(),
                                rounds: 2,
                            });
                            cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 18.0 });
                            cmds.push(Command::SetCourse {
                                unit: u.id,
                                course_deg: (u.course_deg + 120.0) % 360.0,
                            });
                            self.minds.get_mut(&u.id).map(|m| m.state = State::Withdraw);
                        } else {
                            cmds.push(Command::SetCourse {
                                unit: u.id,
                                course_deg: u.pos.bearing_to(contact_pos(obs, contact, u.pos)),
                            });
                            let approach = if dist > 20.0 { 16.0 } else { p.sub_patrol_speed_kts };
                            cmds.push(Command::SetSpeed { unit: u.id, speed_kts: approach });
                        }
                    } else {
                        cmds.push(Command::SetSpeed {
                            unit: u.id,
                            speed_kts: p.sub_patrol_speed_kts,
                        });
                    }
                }

                Role::Raider => {
                    if let Some((contact, dist, _)) = self.best_strike_target(obs, u) {
                        cmds.push(Command::AssignTarget { unit: u.id, contact });
                        if dist <= 55.0 && obs.tick - mind.last_salvo_tick >= p.salvo_min_interval_s {
                            cmds.push(Command::Fire {
                                unit: u.id,
                                contact,
                                weapon: "ascm_std".into(),
                                rounds: 2,
                            });
                            mind.last_salvo_tick = obs.tick;
                            self.minds.get_mut(&u.id).map(|m| m.state = State::Withdraw);
                        }
                        if mind.state == State::Withdraw {
                            cmds.push(Command::SetCourse {
                                unit: u.id,
                                course_deg: (u.course_deg + 90.0) % 360.0,
                            });
                            cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 30.0 });
                        } else {
                            cmds.push(Command::SetCourse {
                                unit: u.id,
                                course_deg: u.pos.bearing_to(contact_pos(obs, contact, u.pos)),
                            });
                            cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 20.0 });
                        }
                    } else {
                        cmds.push(Command::SetWeaponsFree { unit: u.id, free: true });
                    }
                }

                // 其余角色（无角色舰等）保持武器自由。
                _ => {
                    if !u.fire_on && u.kind == PlatformKind::Ship {
                        cmds.push(Command::SetWeaponsFree { unit: u.id, free: true });
                    }
                }
            }
        }

        // P8: 基地打击——任一 Slcm 舰对已识别岸上目标齐射。
        for u in &obs.units {
            if u.kind != PlatformKind::Ship {
                continue;
            }
            let has_slcm = u.weapons.iter().any(|w| w.weapon == "slcm" && w.ammo > 0);
            if !has_slcm {
                continue;
            }
            let Some(mind) = self.minds.get(&u.id) else { continue };
            if mind.last_salvo_tick > 0 && obs.tick - mind.last_salvo_tick < 180 {
                continue;
            }
            for c in &obs.contacts {
                if c.stale || c.quality < 0.25 || c.category != PlatformKind::Base {
                    continue;
                }
                let dist = u.pos.range_to(c.estimate);
                if dist <= 160.0 {
                    cmds.push(Command::AssignTarget { unit: u.id, contact: c.id });
                    cmds.push(Command::Fire {
                        unit: u.id,
                        contact: c.id,
                        weapon: "slcm".into(),
                        rounds: 1,
                    });
                    if let Some(m) = self.minds.get_mut(&u.id) {
                        m.last_salvo_tick = obs.tick;
                    }
                    break;
                }
            }
        }

        cmds
    }
}

pub mod unit_obs_ext {
    use super::is_helo;
    use harpoon_core::observe::UnitObs;
    use harpoon_core::types::PlatformKind;

    pub fn max_speed_hint(u: &UnitObs) -> f64 {
        match u.kind {
            PlatformKind::Aircraft => {
                if is_helo(u) { 130.0 } else { 500.0 }
            }
            PlatformKind::Sub => 18.0,
            PlatformKind::Ship => {
                if u.hp_max >= 800.0 { 30.0 } else { 28.0 }
            }
            _ => 20.0,
        }
    }

    pub fn cruise(u: &UnitObs) -> f64 {
        let max = max_speed_hint(u);
        if max > 200.0 { max * 0.7 } else { max * 0.75 }
    }

    #[allow(dead_code)]
    pub fn strike_envelope_nm(u: &UnitObs) -> f64 {
        u.weapons
            .iter()
            .filter(|w| w.weapon.starts_with("asm"))
            .map(|_| 38.25) // 45 nm envelope × 0.85 release fraction
            .next()
            .unwrap_or(0.0)
    }
}

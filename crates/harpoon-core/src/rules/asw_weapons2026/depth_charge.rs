//! §8.5.4 actual ship DC patterns. Hardware, stock and DP require exact source
//! bindings; no historical weapon Pk, damage, salvo or reload clock is used.
use super::{active, tables};
use crate::db::{SensorKind, WeaponKind};
use crate::types::PlatformKind;
use crate::{ContactId, EntityId, EventKind, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub mod safety;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    pub source: String,
    pub damage_major: f64,
    pub damage_minor: f64,
    pub max_depth_band: usize,
    pub units: BTreeMap<String, UnitFit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitFit {
    pub source: String,
    pub mount_index: usize,
    pub physical_size: usize,
    pub reserve: u16,
    pub mounts: Vec<MountFit>,
    /// Explicit source-bound dedicated depth/attack sonar. Actual native
    /// sensing is still required; declaring a sensor never creates a report.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub depth_determining_sensors: BTreeSet<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MountKind {
    Rail,
    Projector,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MountFit {
    pub physical: usize,
    pub kind: MountKind,
    pub capacity: u16,
    pub loaded: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allocation {
    pub physical: usize,
    pub charges: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Distribution {
    Straight,
    Dispersed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    ProjectorPairPattern {
        contact: ContactId,
        weapon: String,
        physicals: Vec<usize>,
        phase: super::projector::advanced::FirePhase,
    },
    ProjectorPattern {
        contact: ContactId,
        weapon: String,
        physical: usize,
        phase: super::projector::advanced::FirePhase,
    },
    DepthChargePattern {
        contact: ContactId,
        weapon: String,
        charges: u16,
        depth_band: usize,
        distribution: Distribution,
        allocations: Vec<Allocation>,
    },
    DepthChargeEvasion {
        evading: bool,
    },
    ReloadDepthCharges {
        weapon: String,
        allocations: Vec<Allocation>,
    },
    DepthChargeDepthReport {
        attack: String,
        bands: [usize; 2],
    },
    DepthChargeSelectDepth {
        attack: String,
        depth_band: usize,
    },
    AirDepthChargePattern {
        contact: ContactId,
        weapon: String,
        charges: u16,
        depth_band: Option<usize>,
    },
    AirDepthChargeDepthReport {
        attack: String,
        bands: [usize; 3],
    },
    AirDepthChargeSelectDepth {
        attack: String,
        depth_band: usize,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mount {
    pub fit: MountFit,
    pub loaded: u16,
    pub epoch: Option<i64>,
    pub used: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    pub name: String,
    pub class: String,
    pub owner: Side,
    pub fit: UnitFit,
    pub reserve: u16,
    pub mounts: BTreeMap<usize, Mount>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Defense {
    pub evading: bool,
    pub at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reload {
    pub weapon: String,
    pub allocations: Vec<Allocation>,
    pub started: i64,
    pub due: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub actor: EntityId,
    pub action: Action,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub units: BTreeMap<EntityId, BTreeMap<String, Unit>>,
    pub defense: BTreeMap<EntityId, Defense>,
    pub patterns: BTreeMap<String, Pattern>,
    pub reloads: BTreeMap<EntityId, Reload>,
    pub next_pattern: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requests: Vec<Request>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub phase_origins: BTreeMap<EntityId, Vec2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_tick: Option<i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub incoming_sequences: BTreeMap<EntityId, u64>,
    #[serde(default, skip_serializing_if = "safety::State::is_empty")]
    pub safety: safety::State,
}
impl State {
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
            && self.defense.is_empty()
            && self.patterns.is_empty()
            && self.reloads.is_empty()
            && self.requests.is_empty()
            && self.next_pattern == 0
            && self.safety.is_empty()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Physical {
    pub at: i64,
    pub pass_yards: f64,
    pub alive: bool,
    pub submarine: bool,
    pub depth_m: f64,
    pub depth_band: usize,
    pub speed_kts: f64,
    pub cannot_evade: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    pub id: String,
    pub actor: EntityId,
    pub owner: Side,
    pub target: EntityId,
    pub contact: ContactId,
    pub weapon: String,
    pub charges: u16,
    pub declared_depth: usize,
    pub distribution: Distribution,
    pub allocations: Vec<Allocation>,
    pub origin: Vec2,
    pub target_origin: Option<Vec2>,
    pub aim: Vec2,
    pub tick: i64,
    pub accurate_depth: bool,
    pub depth_evidence: Value,
    pub sea_state: u8,
    pub damage_major: f64,
    pub damage_minor: f64,
    pub max_depth_band: usize,
    pub physical: Option<Physical>,
    pub evading: Option<bool>,
    pub depth_report: Option<[usize; 2]>,
    pub depth_selected: bool,
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defender_alias: Option<String>,
}
fn field(name: &str) -> f64 {
    tables()["depth_charge"][name]
        .as_f64()
        .expect("verified DC rule field")
}
fn state(w: &World) -> Result<&State, String> {
    w.rules
        .asw_weapons
        .as_ref()
        .map(|s| &s.depth_charges)
        .ok_or("8.5.4 source runtime absent".into())
}
fn entity(w: &World, id: EntityId) -> Result<&crate::world::Entity, String> {
    w.entity(id)
        .filter(|e| e.alive)
        .ok_or("8.5.4 own live unit unavailable".into())
}
fn source_fit<'a>(w: &'a World, weapon: &str) -> Result<&'a Fit, String> {
    w.rules
        .asw_weapons
        .as_ref()
        .and_then(|s| s.config.depth_charges.get(weapon))
        .ok_or("8.5.4 exact DC source/DP fit absent".into())
}
fn named(w: &World, name: &str) -> Result<EntityId, String> {
    let mut es = w.entities.values().filter(|e| e.name == name);
    let e = es.next().ok_or("8.5.4 exact unit absent")?;
    if es.next().is_some() {
        return Err("8.5.4 unit name must be unique".into());
    }
    Ok(e.id)
}
fn sea(w: &World) -> Result<u8, String> {
    w.context
        .radar_environment
        .as_ref()
        .map(|e| e.sea_state)
        .filter(|s| *s < 7)
        .ok_or("8.5.4 rails/projectors require known sea state below7".into())
}
fn counts(u: &Unit, live: &BTreeSet<usize>) -> Result<u16, String> {
    u.mounts
        .iter()
        .filter(|(id, _)| live.contains(id))
        .try_fold(u.reserve, |n, (_, m)| {
            n.checked_add(m.loaded)
                .ok_or("8.5.4 physical stock overflow".into())
        })
}
fn live_mounts(
    w: &World,
    actor: EntityId,
    weapon: &str,
    u: &Unit,
) -> Result<BTreeSet<usize>, String> {
    let e = entity(w, actor)?;
    if e.name != u.name || e.class != u.class || e.side != u.owner || e.kind != PlatformKind::Ship {
        return Err("8.5.4 actual source owner/class identity changed".into());
    }
    let m = e
        .weapons
        .get(u.fit.mount_index)
        .filter(|m| m.weapon == weapon)
        .ok_or("8.5.4 exact aggregate weapon binding changed")?;
    let live: BTreeSet<_> =
        super::super::expanded_damage2026::physical_weapon_mounts(w, actor, u.fit.mount_index)?
            .into_iter()
            .collect();
    if live.len() != usize::from(m.mounts) || live.iter().any(|id| !u.mounts.contains_key(id)) {
        return Err(
            "8.5.4 physical equipment unavailable or lacks exact surviving identity".into(),
        );
    }
    if counts(u, &live)? != m.ammo {
        return Err(
            "8.5.4 aggregate/loaded/reserve stock mismatch; no inferred redistribution".into(),
        );
    }
    for id in &live {
        if super::super::expanded_damage2026::physical_weapon_stock(
            w,
            actor,
            weapon,
            u.fit.mount_index,
            *id,
        )?
        .is_some_and(|n| n != u.mounts[id].loaded)
        {
            return Err("8.5.4 physical component stock must equal that rail/projector loaded charges; central reserve is separate".into());
        }
    }
    Ok(live)
}
pub(super) fn initialize(w: &World, config: &super::Configuration) -> Result<State, String> {
    let mut s = State::default();
    for (weapon, f) in &config.depth_charges {
        if f.source.trim().is_empty()
            || f.units.is_empty()
            || !f.damage_major.is_finite()
            || !f.damage_minor.is_finite()
            || f.damage_major <= 0.
            || f.damage_minor <= 0.
            || f.damage_minor > f.damage_major
            || !(1..=7).contains(&f.max_depth_band)
            || config.standoffs.contains_key(weapon)
            || config.projectors.contains_key(weapon)
            || !w.db.weapons.get(weapon).is_some_and(|d| {
                d.targets.sub && matches!(d.kind, WeaponKind::Bomb | WeaponKind::AswRocket)
            })
        {
            return Err("8.5.4 independently sourced positive Major/MinorDP and exact depth-charge weapon fit required".into());
        }
        for (name, c) in &f.units {
            let id = named(w, name)?;
            let e = entity(w, id)?;
            let aggregate = e
                .weapons
                .get(c.mount_index)
                .filter(|m| m.weapon == *weapon)
                .ok_or("8.5.4 exact actual weapon mount absent")?;
            if c.source.trim().is_empty()
                || c.physical_size > 6
                || c.mounts.is_empty()
                || e.kind != PlatformKind::Ship
                || e.weapons.iter().filter(|m| m.weapon == *weapon).count() != 1
                || c.mounts.len() != usize::from(aggregate.mounts)
                || c.depth_determining_sensors
                    .iter()
                    .any(|sensor| !e.sensors.iter().any(|s| s.sensor == *sensor))
            {
                return Err(
                    "8.5.4 exact ship size/physical allocations/sensor identities required".into(),
                );
            }
            let mut mounts = BTreeMap::new();
            for m in &c.mounts {
                if m.capacity == 0
                    || m.loaded > m.capacity
                    || m.physical >= usize::from(aggregate.mounts)
                    || mounts
                        .insert(
                            m.physical,
                            Mount {
                                fit: m.clone(),
                                loaded: m.loaded,
                                epoch: None,
                                used: 0,
                            },
                        )
                        .is_some()
                {
                    return Err("8.5.4 physical capacity/loaded stock invalid or duplicate".into());
                }
            }
            let u = Unit {
                name: name.clone(),
                class: e.class.clone(),
                owner: e.side,
                fit: c.clone(),
                reserve: c.reserve,
                mounts,
            };
            let live = (0..usize::from(aggregate.mounts)).collect();
            if counts(&u, &live)? != aggregate.ammo {
                return Err("8.5.4 initial loaded plus central reserve must exactly equal real aggregate ammo".into());
            }
            s.units.entry(id).or_default().insert(weapon.clone(), u);
        }
    }
    Ok(s)
}
pub fn is_depth_charge_configured(w: &World, actor: EntityId, weapon: &str) -> bool {
    active(w)
        && (state(w).is_ok_and(|s| s.units.get(&actor).is_some_and(|u| u.contains_key(weapon)))
            || super::air_depth_charge::configured(w, actor, weapon))
}
fn unit<'a>(s: &'a State, actor: EntityId, weapon: &str) -> Result<&'a Unit, String> {
    s.units
        .get(&actor)
        .and_then(|u| u.get(weapon))
        .ok_or("8.5.4 selected unit has no exact physical DC fit".into())
}
fn allocations(u: &Unit, live: &BTreeSet<usize>, as_: &[Allocation]) -> Result<u16, String> {
    let mut seen = BTreeSet::new();
    let mut total = 0u16;
    for a in as_ {
        if a.charges == 0
            || !live.contains(&a.physical)
            || !u.mounts.contains_key(&a.physical)
            || !seen.insert(a.physical)
        {
            return Err(
                "8.5.4 distinct actual physical allocations and positive charges required".into(),
            );
        }
        total = total
            .checked_add(a.charges)
            .ok_or("8.5.4 allocation sum overflow")?;
    }
    if total == 0 {
        return Err("8.5.4 empty pattern/reload".into());
    }
    Ok(total)
}
fn pattern_preflight(
    w: &World,
    s: &State,
    actor: EntityId,
    contact: ContactId,
    weapon: &str,
    charges: u16,
    depth_band: usize,
    distribution: Distribution,
    as_: &[Allocation],
    phase_origin: bool,
) -> Result<(Vec2, EntityId), String> {
    let e = entity(w, actor)?;
    if !super::super::unmanned2026::can_attack(w, actor) {
        return Err(
            "2.5 actual unmanned control/payload state does not permit weapon release".into(),
        );
    }
    let u = unit(s, actor, weapon)?;
    let f = source_fit(w, weapon)?;
    let live = live_mounts(w, actor, weapon, u)?;
    sea(w)?;
    if depth_band > f.max_depth_band || depth_band == 0 {
        return Err("8.5.4 selected fuze band outside verified source depth domain".into());
    }
    if allocations(u, &live, as_) != Ok(charges) {
        return Err("8.5.4 declared pattern count must exactly equal physical allocations".into());
    }
    let mut rail = 0u16;
    let mut projected = 0u16;
    for a in as_ {
        let m = &u.mounts[&a.physical];
        let limit = field(if m.fit.kind == MountKind::Rail {
            "rail_per_tactical_turn"
        } else {
            "projector_per_tactical_turn"
        }) as u16;
        let used = if m.epoch == Some(w.tick.div_euclid(180)) {
            m.used
        } else {
            0
        };
        if a.charges > m.loaded || u32::from(used) + u32::from(a.charges) > u32::from(limit) {
            return Err("8.5.4 actual loaded stock or physical per-turn capacity exceeded".into());
        }
        match m.fit.kind {
            MountKind::Rail => rail += a.charges,
            MountKind::Projector => projected += a.charges,
        }
    }
    if match distribution {
        Distribution::Straight => projected != 0,
        Distribution::Dispersed => charges < 2 || rail != projected,
    } {
        return Err(
            "8.5.4 straight pattern uses rails; dispersed requires half rails and half projectors"
                .into(),
        );
    }
    let c = w.sides[e.side.index()]
        .contacts
        .get(&contact)
        .filter(|c| !c.stale && !c.bearing_only)
        .ok_or("8.5.4 own current measured ContactId required")?;
    let approach = if phase_origin && s.phase_tick == Some(w.tick) {
        s.phase_origins.get(&actor).copied().unwrap_or(e.pos)
    } else {
        e.pos
    };
    let aim=located_aim(w,actor,c.about)?.ok_or("8.5.4 no own native localization or genuinely received friendly direction; fused side CID is not communication")?;
    // §8.5.4.2 requires a measured own/friendly datum, not an independently
    // completed passive-sonar classification. A real active search can locate
    // an echo without producing the passive classification ledger. Physical
    // submarine applicability is sampled only after the complete stock debit.
    if !aim.x.is_finite()
        || !aim.y.is_finite()
        || approach.range_to(aim) * field("yards_per_nm") > field("pass_yards")
    {
        return Err("8.5.4 own measured datum and measured250yd approach required".into());
    }
    Ok((aim, c.about))
}
pub(super) fn located_aim(
    w: &World,
    actor: EntityId,
    target: EntityId,
) -> Result<Option<Vec2>, String> {
    let e = entity(w, actor)?;
    if super::super::detection2026::wide_mad::only_mad_waiting(w, actor, target) {
        return Err(
            "5.7.1 wide-area MAD-only localization must wait until the next Tactical Turn".into(),
        );
    }
    let cid = w.find_contact(e.side, target);
    let fresh = super::super::firecontrol2026::tables()["execution_revision"]["own_motion"]
        ["report_freshness_s"]
        .as_i64()
        .unwrap();
    let own = w
        .rules
        .aaw
        .as_ref()
        .into_iter()
        .flat_map(|a| a.firecontrol.tracks.values())
        .filter_map(|t| {
            let r = t.report.as_ref()?;
            (t.observer == actor
                && t.target == target
                && r.at <= w.tick
                && w.tick - r.at <= fresh
                && cid.is_some_and(|cid| {
                    super::super::communications2026::native_report_view(w, actor, cid, &t.sensor)
                        .is_ok()
                }))
            .then(|| r.measured_position.map(|p| ((r.at, t.sensor.clone()), p)))
            .flatten()
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, p)| p);
    if own.is_some() {
        return Ok(own);
    }
    if let Some(aim)=super::super::detection2026::sonobuoys::attack_solution(w,actor,target).and_then(|fix|fix.reported_aim) {
        return Ok(Some(aim));
    }
    if let Some(report) =
        super::super::communications2026::received_located_report(w, actor, target)?
    {
        return Ok(report.measured_position);
    }
    let remote = super::super::communications2026::received_solution(w, actor, target)?;
    Ok(remote
        .filter(|r| r.domain == super::super::communications2026::FcsDomain::Asw)
        .and_then(|r| r.aim))
}
fn reload_preflight(
    w: &World,
    s: &State,
    actor: EntityId,
    weapon: &str,
    as_: &[Allocation],
) -> Result<u16, String> {
    let u = unit(s, actor, weapon)?;
    let live = live_mounts(w, actor, weapon, u)?;
    if s.reloads.contains_key(&actor) {
        return Err("8.5.4 one ship-wide reload detail is already working".into());
    }
    let n = allocations(u, &live, as_)?;
    let limit = field(if u.fit.physical_size <= 2 {
        "large_ship_reload_charges"
    } else {
        "small_ship_reload_charges"
    }) as u16;
    if n > limit || n > u.reserve {
        return Err("8.5.4 source reload detail or actual central reserve exceeded".into());
    }
    for a in as_ {
        let m = &u.mounts[&a.physical];
        if u32::from(m.loaded) + u32::from(a.charges) > u32::from(m.fit.capacity) {
            return Err("8.5.4 reload exceeds selected physical capacity".into());
        }
    }
    Ok(n)
}
fn validate(
    w: &World,
    s: &State,
    actor: EntityId,
    a: &Action,
    phase_origin: bool,
) -> Result<(), String> {
    if !active(w) {
        return Err("8.5.4 source Full profile required".into());
    }
    let e = entity(w, actor)?;
    if w.over || crate::exercise::locked(w, e.side) {
        return Err("8.5.4 active exercise required".into());
    }
    match a {
        Action::DepthChargePattern {
            contact,
            weapon,
            charges,
            depth_band,
            distribution,
            allocations,
        } => {
            pattern_preflight(
                w,
                s,
                actor,
                *contact,
                weapon,
                *charges,
                *depth_band,
                *distribution,
                allocations,
                phase_origin,
            )?;
        }
        Action::ReloadDepthCharges {
            weapon,
            allocations,
        } => {
            reload_preflight(w, s, actor, weapon, allocations)?;
        }
        Action::DepthChargeEvasion { .. } => {
            if e.kind != PlatformKind::Sub {
                return Err("8.5.4 evasion declaration belongs to own submarine".into());
            }
        }
        Action::DepthChargeDepthReport { attack, bands } => {
            let p = s
                .patterns
                .values()
                .find(|p| {
                    p.defender_alias.as_deref() == Some(attack.as_str())
                        && p.target == actor
                        && p.result.is_none()
                        && !p.accurate_depth
                })
                .ok_or("8.5.4 own pending attack depth report required")?;
            let b = p
                .physical
                .as_ref()
                .ok_or("8.5.4 original pass depth not captured")?
                .depth_band;
            if bands[0].checked_add(1) != Some(bands[1])
                || bands[1] > 7
                || !bands.contains(&b)
                || p.depth_report.is_some()
            {
                return Err("8.5.4 report must contain original true depth in two adjacent bands exactly once".into());
            }
        }
        Action::DepthChargeSelectDepth { attack, depth_band } => {
            let p = s
                .patterns
                .get(attack)
                .filter(|p| {
                    p.actor == actor
                        && p.owner == e.side
                        && p.result.is_none()
                        && p.depth_report.is_some()
                        && !p.depth_selected
                })
                .ok_or(
                    "8.5.4 original shooter must receive a depth report before one final selection",
                )?;
            if *depth_band == 0 || *depth_band > source_fit(w, &p.weapon)?.max_depth_band {
                return Err("8.5.4 chosen depth outside sourced fuze envelope".into());
            }
        }
        Action::AirDepthChargePattern { .. }
        | Action::AirDepthChargeDepthReport { .. }
        | Action::AirDepthChargeSelectDepth { .. } => {
            return Err("8.5.4.4 aircraft action requires its independent physical executor".into())
        }
        Action::ProjectorPattern { .. } | Action::ProjectorPairPattern { .. } => {
            return Err("8.5.3 projector action requires its independent physical executor".into())
        }
    }
    Ok(())
}
pub fn validate_action(w: &World, actor: EntityId, a: &Action) -> Result<(), String> {
    if super::projector::paired::is_action(a) {
        return super::projector::paired::validate_action(w, actor, a);
    }
    if super::projector::advanced::is_action(a) {
        return super::projector::advanced::validate_action(w, actor, a);
    }
    if super::air_depth_charge::is_action(a) {
        return super::air_depth_charge::validate_action(w, actor, a);
    }
    validate(w, state(w)?, actor, a, false)
}
pub fn control(w: &mut World, actor: EntityId, mut action: Action) -> Result<(), String> {
    if super::projector::paired::is_action(&action) {
        return super::projector::paired::control(w, actor, action);
    }
    if super::projector::advanced::is_action(&action) {
        return super::projector::advanced::control(w, actor, action);
    }
    if super::air_depth_charge::is_action(&action) {
        return super::air_depth_charge::control(w, actor, action);
    }
    match &mut action {
        Action::DepthChargePattern { allocations, .. }
        | Action::ReloadDepthCharges { allocations, .. } => allocations.sort_by_key(|a| a.physical),
        _ => {}
    }
    validate_action(w, actor, &action)?;
    w.rules
        .asw_weapons
        .as_mut()
        .unwrap()
        .depth_charges
        .requests
        .push(Request { actor, action });
    Ok(())
}
fn depth_source(w: &World, actor: EntityId, target: EntityId, u: &Unit) -> (bool, Value) {
    let fresh = super::super::firecontrol2026::tables()["execution_revision"]["own_motion"]
        ["report_freshness_s"]
        .as_i64()
        .unwrap();
    let cid = w.find_contact(u.owner, target);
    let own=w.rules.aaw.as_ref().into_iter().flat_map(|a|a.firecontrol.tracks.values()).find(|t|{
        if t.observer!=actor||t.target!=target{return false;}
        let Some(r)=&t.report else{return false;};
        r.at<=w.tick&&w.tick-r.at<=fresh&&r.sensor_kind==SensorKind::SonarActive&&r.measured_position.is_some()
            &&w.entity(actor).is_some_and(|e|e.systems.sonar>0. && e.sensors.iter().any(|i|i.sensor==t.sensor&&i.active)&&!crate::sensors::sensor_damaged(e,&t.sensor))
            &&super::super::expanded_damage2026::sensor_operational(w,actor,&t.sensor).unwrap_or(false)
            &&cid.is_some_and(|cid|super::super::communications2026::native_report_view(w,actor,cid,&t.sensor).is_ok())
            &&(matches!(&r.receiver,Some(super::super::detection2026::Receiver::Sonar{generation:4..=7,mount,..}) if *mount!=super::super::detection2026::SonarMount::Towed)
                ||u.fit.depth_determining_sensors.contains(&t.sensor))
    });
    own.map_or((false,json!({"source":"two adjacent true bands must be declared by defending player"})),|t|(true,json!({"source":"actual held depth-determining native sonar","observer":actor,"sensor":t.sensor,"at":t.last})))
}
fn private_trace(w: &mut World, actor: EntityId, owner: Side, rule: &str, detail: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "asw_weapons2026".into(),
            rule: rule.into(),
            side: Some(owner),
            unit: Some(actor),
            contact: None,
            detail: detail.to_string(),
        },
        Visibility::Side(owner),
    );
}
fn commit(w: &mut World, s: &mut State, actor: EntityId, a: &Action) -> Result<(), String> {
    let Action::DepthChargePattern {
        contact,
        weapon,
        charges,
        depth_band,
        distribution,
        allocations,
    } = a
    else {
        unreachable!()
    };
    let (aim, target) = pattern_preflight(
        w,
        s,
        actor,
        *contact,
        weapon,
        *charges,
        *depth_band,
        *distribution,
        allocations,
        true,
    )?;
    let u = unit(s, actor, weapon)?.clone();
    let f = source_fit(w, weapon)?.clone();
    let next = s
        .next_pattern
        .checked_add(1)
        .ok_or("8.5.4 attack sequence exhausted")?;
    let attack = format!("8.5.4:{}:{next}", actor.0);
    let (accurate_depth, depth_evidence) = depth_source(w, actor, target, &u);
    for allocation in allocations {
        super::super::expanded_damage2026::spend_physical_weapon_stock(
            w,
            actor,
            weapon,
            u.fit.mount_index,
            allocation.physical,
            allocation.charges,
        )
        .expect("all selected component stocks preflighted before any debit");
    }
    w.entity_mut(actor).unwrap().weapons[u.fit.mount_index].ammo -= *charges;
    let own = s.units.get_mut(&actor).unwrap().get_mut(weapon).unwrap();
    for allocation in allocations {
        let m = own.mounts.get_mut(&allocation.physical).unwrap();
        if m.epoch != Some(w.tick.div_euclid(180)) {
            m.epoch = Some(w.tick.div_euclid(180));
            m.used = 0;
        }
        m.loaded -= allocation.charges;
        m.used += allocation.charges;
    }
    // Referee target physics is consulted only after the complete physical
    // ammunition debit. A missing/changed target can never refund the attack.
    let target_origin = if s.phase_tick == Some(w.tick) {
        s.phase_origins.get(&target).copied()
    } else {
        w.entity(target).map(|e| e.pos)
    };
    let origin = if s.phase_tick == Some(w.tick) {
        s.phase_origins
            .get(&actor)
            .copied()
            .unwrap_or(w.entity(actor).unwrap().pos)
    } else {
        w.entity(actor).unwrap().pos
    };
    let p = Pattern {
        id: attack.clone(),
        actor,
        owner: u.owner,
        target,
        contact: *contact,
        weapon: weapon.clone(),
        charges: *charges,
        declared_depth: *depth_band,
        distribution: *distribution,
        allocations: allocations.clone(),
        origin,
        target_origin,
        aim,
        tick: w.tick,
        accurate_depth,
        depth_evidence,
        sea_state: sea(w)?,
        damage_major: f.damage_major,
        damage_minor: f.damage_minor,
        max_depth_band: f.max_depth_band,
        physical: None,
        evading: s.defense.get(&target).map(|d| d.evading),
        depth_report: None,
        depth_selected: accurate_depth,
        result: None,
        defender_alias: None,
    };
    s.next_pattern = next;
    safety::record_drop(w, &mut s.safety, &p);
    s.patterns.insert(attack.clone(), p);
    private_trace(
        w,
        actor,
        u.owner,
        "8.5.4.pattern_committed",
        json!({"attack":attack,"contact":contact,"weapon":weapon,"charges":charges,"declared_depth":depth_band,"distribution":distribution,"allocations":allocations,"phase":"post_movement_common_damage","physical_stock_debited":true}),
    );
    let e = w.entity(actor).unwrap();
    let name = e.name.clone();
    let weapon_name = w.db.weapons[weapon].name.clone();
    let mut left = *charges;
    while left > 0 {
        let count = left.min(u16::from(u8::MAX)) as u8;
        w.emit(
            EventKind::WeaponsRelease {
                side: u.owner,
                shooter: actor,
                shooter_name: name.clone(),
                weapon: weapon_name.clone(),
                count,
            },
            Visibility::Side(u.owner),
        );
        left -= u16::from(count);
    }
    Ok(())
}
fn closest(a: Vec2, b: Vec2) -> f64 {
    let d = b.sub(a);
    let square = d.x * d.x + d.y * d.y;
    let t = if square == 0. {
        0.
    } else {
        (-(a.x * d.x + a.y * d.y) / square).clamp(0., 1.)
    };
    a.add(d.scaled(t)).norm()
}
fn capture(w: &World, p: &mut Pattern) {
    if p.physical.is_some() {
        return;
    }
    let Some(target) = w.entity(p.target) else {
        return;
    };
    let Some(ship) = w.entity(p.actor) else {
        return;
    };
    let pass = closest(
        p.origin.sub(p.target_origin.unwrap_or(target.pos)),
        ship.pos.sub(target.pos),
    ) * field("yards_per_nm");
    let band = super::super::detection2026::depth_band(target.depth_m).unwrap_or(usize::MAX);
    let maximum = target.max_speed_with_depth_rules(
        &w.db,
        super::super::navigation2026::submarine::configured_depth(w, p.target),
    );
    let cannot_evade = maximum.is_finite() && maximum <= 0.
        || super::super::expanded_damage2026::engineering_available(w, p.target)
            .is_ok_and(|available| !available);
    p.physical = Some(Physical {
        at: w.tick,
        pass_yards: pass,
        alive: target.alive,
        submarine: target.kind == PlatformKind::Sub,
        depth_m: target.depth_m,
        depth_band: band,
        speed_kts: target.speed_kts.abs(),
        cannot_evade,
    });
}
pub(super) fn modifier(speed: f64, band: usize) -> Result<f64, String> {
    if !speed.is_finite() || band == 0 || band > 7 {
        return Err("8.5.4 no printed speed/depth cell".into());
    }
    let col = if band <= 2 {
        0
    } else if band <= 4 {
        1
    } else if band <= 6 {
        2
    } else {
        3
    };
    let row = tables()["projector"]["speed_rows"]
        .as_array()
        .unwrap()
        .iter()
        .position(|r| {
            speed >= r[0].as_f64().unwrap() && (r[1].is_null() || speed <= r[1].as_f64().unwrap())
        })
        .ok_or("8.5.4 unprinted continuous speed gap; no interpolation")?;
    tables()["projector"]["modifiers"][row][col]
        .as_f64()
        .ok_or("8.5.4 source -- attack not possible".into())
}
fn adjudicate(w: &World, p: &Pattern) -> Option<Value> {
    let physical = p.physical.as_ref()?;
    let mut out = json!({"attack":p.id,"weapon":p.weapon,"charges":p.charges,"declared_depth":p.declared_depth,"distribution":p.distribution,"actual_pass_yards":physical.pass_yards,"actual_depth_m":physical.depth_m,"actual_depth_band":physical.depth_band,"actual_speed_kts":physical.speed_kts,"severity":"miss","raw_d100":null,"modified_d100":null,"major_threshold":null,"minor_threshold":null,"dp":null,"reason":null,"damage_queued":false});
    if !physical.alive || !physical.submarine || physical.depth_m <= 0. {
        out["reason"] = json!("no live submerged submarine at actual pass");
        return Some(out);
    }
    if physical.pass_yards > field("pass_yards") {
        out["reason"] = json!("actual movement did not pass within250 yards");
        return Some(out);
    }
    if physical.depth_band > p.max_depth_band {
        out["reason"] = json!("below sourced weapon maximum depth");
        return Some(out);
    }
    if !p.depth_selected {
        return None;
    }
    if p.declared_depth != physical.depth_band {
        out["reason"] = json!("declared depth did not match the original actual pass depth");
        return Some(out);
    }
    if physical.depth_m > field("matrix_deep_i_max_m") {
        out["severity"] = json!("pending");
        out["reason"] = json!(
            "source matrix stops at Deep I; collapsed Deep+ index is not permission to use DeepII"
        );
        return Some(out);
    }
    let evading = if physical.cannot_evade {
        false
    } else {
        p.evading?
    };
    let factor = match modifier(physical.speed_kts, physical.depth_band) {
        Ok(v) => v,
        Err(reason) => {
            out["severity"] = json!(if reason.contains("not possible") {
                "miss"
            } else {
                "pending"
            });
            out["reason"] = json!(reason);
            return Some(out);
        }
    };
    let base = f64::from(p.charges)
        * field("base_percent_per_charge")
        * if p.distribution == Distribution::Straight {
            field("straight_line_factor")
        } else {
            1.
        };
    let major = (base * factor).round() as u32;
    let minor = major.saturating_mul(2);
    let raw = super::die(w, p.actor, p.tick, &format!("{}:pattern_d100", p.id), 100);
    let modified = i32::from(raw)
        + if evading {
            0
        } else {
            field("non_evading_d100_modifier") as i32
        };
    let (severity, dp) = if modified <= major as i32 {
        ("major", Some(p.damage_major))
    } else if modified <= minor as i32 {
        ("minor", Some(p.damage_minor))
    } else {
        ("miss", None)
    };
    out["raw_d100"] = json!(raw);
    out["modified_d100"] = json!(modified);
    out["major_threshold"] = json!(major);
    out["minor_threshold"] = json!(minor);
    out["severity"] = json!(severity);
    out["dp"] = json!(dp);
    out["evading"] = json!(evading);
    out["source_depth_evidence"] = p.depth_evidence.clone();
    Some(out)
}
fn process_reload(w: &mut World, s: &mut State) {
    let jobs: Vec<_> = s.reloads.iter().map(|(id, j)| (*id, j.clone())).collect();
    for (actor, j) in jobs {
        if j.due > w.tick {
            continue;
        }
        s.reloads.remove(&actor);
        let result = reload_preflight(w, s, actor, &j.weapon, &j.allocations).and_then(|n| {
            if j.due == j.started + field("reload_seconds") as i64 && j.started <= w.tick {
                Ok(n)
            } else {
                Err("invalid saved reload clock".into())
            }
        });
        let Ok(total) = result else {
            if let Some(owner) = s
                .units
                .get(&actor)
                .and_then(|u| u.get(&j.weapon))
                .map(|u| u.owner)
            {
                private_trace(
                    w,
                    actor,
                    owner,
                    "8.5.4.reload_cancelled",
                    json!({"weapon":j.weapon,"reason":result.err(),"reserve_debited":0}),
                );
            }
            continue;
        };
        let u = s.units.get_mut(&actor).unwrap().get_mut(&j.weapon).unwrap();
        for a in &j.allocations {
            super::super::expanded_damage2026::credit_physical_weapon_stock(
                w,
                actor,
                &j.weapon,
                u.fit.mount_index,
                a.physical,
                a.charges,
            )
            .expect("all physical reload credits preflighted before commit");
        }
        u.reserve -= total;
        for a in &j.allocations {
            u.mounts.get_mut(&a.physical).unwrap().loaded += a.charges;
        }
        private_trace(
            w,
            actor,
            u.owner,
            "8.5.4.reload_completed",
            json!({"weapon":j.weapon,"started":j.started,"due":j.due,"allocations":j.allocations,"reserve_transferred":total,"aggregate_ammo_changed":false}),
        );
    }
}
/// One stable post-movement resolver, before the common damage batch closes.
/// Multiple different requests for the same operation reject as a set rather
/// than granting the first client's proposal priority over physical stock.
pub fn resolve_depth_charge_patterns(w: &mut World) {
    resolve_ship_patterns(w);
    super::air_depth_charge::resolve(w);
}
fn resolve_ship_patterns(w: &mut World) {
    if !active(w) {
        return;
    }
    let Some(mut s) = w
        .rules
        .asw_weapons
        .as_mut()
        .map(|a| std::mem::take(&mut a.depth_charges))
    else {
        return;
    };
    process_reload(w, &mut s);
    let mut grouped: BTreeMap<(u8, EntityId, String), BTreeMap<String, Action>> = BTreeMap::new();
    for r in std::mem::take(&mut s.requests) {
        let (phase, key) = match &r.action {
            Action::DepthChargeEvasion { .. } => (0, String::new()),
            Action::DepthChargeDepthReport { attack, .. } => (1, attack.clone()),
            Action::DepthChargeSelectDepth { attack, .. } => (2, attack.clone()),
            Action::DepthChargePattern { weapon, .. } => (3, weapon.clone()),
            Action::ReloadDepthCharges { .. } => (4, String::new()),
            Action::AirDepthChargePattern { .. }
            | Action::AirDepthChargeDepthReport { .. }
            | Action::AirDepthChargeSelectDepth { .. }
            | Action::ProjectorPattern { .. }
            | Action::ProjectorPairPattern { .. } => continue,
        };
        grouped
            .entry((phase, r.actor, key))
            .or_default()
            .insert(serde_json::to_string(&r.action).unwrap(), r.action);
    }
    for ((_, actor, _), choices) in grouped {
        let Some(e) = w.entity(actor).cloned() else {
            continue;
        };
        if choices.len() != 1 {
            private_trace(
                w,
                actor,
                e.side,
                "8.5.4.conflicting_requests",
                json!({"accepted":false,"reason":"different simultaneous requests for the same operation"}),
            );
            continue;
        }
        let a = choices.values().next().unwrap();
        let result = validate(w, &s, actor, a, true);
        if let Err(reason) = result {
            private_trace(
                w,
                actor,
                e.side,
                "8.5.4.request_rejected",
                json!({"reason":reason}),
            );
            continue;
        }
        match a {
            Action::DepthChargeEvasion { evading } => {
                s.defense.insert(
                    actor,
                    Defense {
                        evading: *evading,
                        at: w.tick,
                    },
                );
            }
            Action::DepthChargeDepthReport { attack, bands } => {
                let p = s
                    .patterns
                    .values_mut()
                    .find(|p| {
                        p.defender_alias.as_deref() == Some(attack.as_str()) && p.target == actor
                    })
                    .unwrap();
                p.depth_report = Some(*bands);
                private_trace(
                    w,
                    p.actor,
                    p.owner,
                    "8.5.4.depth_report_received",
                    json!({"attack":p.id,"bands":bands}),
                );
            }
            Action::DepthChargeSelectDepth { attack, depth_band } => {
                let p = s.patterns.get_mut(attack).unwrap();
                p.declared_depth = *depth_band;
                p.depth_selected = true;
            }
            Action::DepthChargePattern { .. } => {
                if let Err(reason) = commit(w, &mut s, actor, a) {
                    private_trace(
                        w,
                        actor,
                        e.side,
                        "8.5.4.request_rejected",
                        json!({"reason":reason}),
                    );
                }
            }
            Action::ReloadDepthCharges {
                weapon,
                allocations,
            } => {
                s.reloads.insert(
                    actor,
                    Reload {
                        weapon: weapon.clone(),
                        allocations: allocations.clone(),
                        started: w.tick,
                        due: w.tick + field("reload_seconds") as i64,
                    },
                );
                private_trace(
                    w,
                    actor,
                    e.side,
                    "8.5.4.reload_started",
                    json!({"weapon":weapon,"due":w.tick+field("reload_seconds") as i64,"allocations":allocations}),
                );
            }
            Action::AirDepthChargePattern { .. }
            | Action::AirDepthChargeDepthReport { .. }
            | Action::AirDepthChargeSelectDepth { .. }
            | Action::ProjectorPattern { .. }
            | Action::ProjectorPairPattern { .. } => {
                unreachable!("air requests have their own queue")
            }
        }
    }
    // Own-ship hazards are consequences of physical releases. Resolve every
    // original release before any target hit/miss or target applicability roll.
    safety::resolve(w, &mut s.safety, &s.patterns);
    let keys: Vec<_> = s
        .patterns
        .iter()
        .filter(|(_, p)| p.result.is_none())
        .map(|(k, _)| k.clone())
        .collect();
    for key in keys {
        let p = s.patterns.get_mut(&key).unwrap();
        capture(w, p);
        if p.defender_alias.is_none()
            && p.physical
                .as_ref()
                .is_some_and(|v| v.alive && v.submarine && v.pass_yards <= field("pass_yards"))
        {
            let n = s.incoming_sequences.entry(p.target).or_default();
            *n = n
                .checked_add(1)
                .expect("finite own incoming pattern sequence");
            p.defender_alias = Some(format!("own-dc-{}-{n}", p.target.0));
        }
        if p.evading.is_none() {
            p.evading = s.defense.get(&p.target).map(|d| d.evading);
        }
        if let Some(mut result) = adjudicate(w, p) {
            if let Some(dp) = result["dp"].as_f64() {
                if w.entity(p.target)
                    .is_some_and(|e| e.alive && e.kind == PlatformKind::Sub)
                {
                    match super::super::damage_runtime::queue_depth_charge(
                        w,
                        p.actor,
                        p.target,
                        &p.weapon,
                        dp,
                        result["severity"] == "major",
                        &p.id,
                    ) {
                        Ok(()) => result["damage_queued"] = json!(true),
                        Err(reason) => result["reason"] = json!(reason),
                    }
                } else {
                    result["reason"]=json!("original pass result retained; target no longer available for deferred damage");
                }
            }
            super::trace(w, p.actor, "8.5.4.pattern_result", result.clone());
            p.result = Some(result);
        }
    }
    w.rules.asw_weapons.as_mut().unwrap().depth_charges = s;
}
/// Referee-only movement origins; no enemy state participates in admission.
pub fn begin_depth_charge_phase(w: &mut World) {
    if !active(w) {
        return;
    }
    let Some(s) = w.rules.asw_weapons.as_ref().map(|a| &a.depth_charges) else {
        return;
    };
    if s.units.is_empty() {
        return;
    }
    let origins = w
        .entities
        .values()
        .filter(|e| e.alive && (e.kind == PlatformKind::Sub || s.units.contains_key(&e.id)))
        .map(|e| (e.id, e.pos))
        .collect();
    let s = &mut w.rules.asw_weapons.as_mut().unwrap().depth_charges;
    s.phase_origins = origins;
    s.phase_tick = Some(w.tick);
}
pub fn depth_charge_own_view(w: &World, actor: EntityId) -> Value {
    let mut ship = ship_own_view(w, actor);
    let air = super::air_depth_charge::own_view(w, actor);
    if air.is_object() {
        if !ship.is_object() {
            ship = json!({});
        }
        ship.as_object_mut()
            .unwrap()
            .extend(air.as_object().unwrap().clone());
    }
    ship
}
fn ship_own_view(w: &World, actor: EntityId) -> Value {
    if !active(w) {
        return Value::Null;
    }
    let Ok(s) = state(w) else { return Value::Null };
    let Some(e) = w.entity(actor) else {
        return Value::Null;
    };
    let mut weapons = BTreeMap::new();
    if let Some(units) = s.units.get(&actor) {
        for (weapon, u) in units {
            if u.owner != e.side {
                continue;
            }
            let pending = live_mounts(w, actor, weapon, u).err();
            let mounts:Vec<_>=u.mounts.values().map(|m|json!({"physical":m.fit.physical,"kind":m.fit.kind,"capacity":m.fit.capacity,"loaded":m.loaded,"used_this_turn":if m.epoch==Some(w.tick.div_euclid(180)){m.used}else{0}})).collect();
            weapons.insert(weapon,json!({"mount_index":u.fit.mount_index,"reserve":u.reserve,"mounts":mounts,"reload":s.reloads.get(&actor),"pending":pending,"source":u.fit.source}));
        }
    }
    let own:Vec<_>=s.patterns.values().filter(|p|p.actor==actor&&p.owner==e.side&&p.result.is_none()).map(|p|json!({"attack":p.id,"contact":p.contact,"weapon":p.weapon,"charges":p.charges,"declared_depth":p.declared_depth,"depth_report":p.depth_report,"select_depth_required":p.depth_report.is_some()&&!p.depth_selected,"waiting_for_depth_report":!p.accurate_depth&&p.depth_report.is_none(),"waiting_for_evasion_declaration":p.evading.is_none(),"declared_evasion":p.evading})).collect();
    let incoming:Vec<_>=s.patterns.values().filter(|p|p.target==actor&&p.result.is_none()).filter_map(|p|p.physical.as_ref().filter(|v|v.alive&&v.pass_yards<=field("pass_yards")).map(|v|json!({"attack":p.defender_alias,"own_depth_band_at_pass":v.depth_band,"depth_report_required":!p.accurate_depth&&p.depth_report.is_none(),"evasion_declaration_required":p.evading.is_none()}))).collect();
    let self_blast = safety::own_view(&s.safety, actor);
    if weapons.is_empty()
        && own.is_empty()
        && incoming.is_empty()
        && !s.defense.contains_key(&actor)
        && self_blast.is_null()
    {
        return Value::Null;
    }
    let mut view = json!({"weapons":weapons,"pending_attacks":own,"incoming_patterns":incoming,"evasion":s.defense.get(&actor)});
    if !self_blast.is_null() {
        view.as_object_mut()
            .unwrap()
            .insert("self_blast".into(), self_blast);
    }
    view
}

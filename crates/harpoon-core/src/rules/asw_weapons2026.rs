//! Source §8.5.2 delivery and §8.5.3/4 table calculations. Full-profile
//! deliveries never use the historical carrier Pk or 0.8nm truth radius.
use super::firecontrol2026::{Placement, Quality};
use crate::db::{Guidance, SensorKind, WeaponKind};
use crate::{ContactId, EntityId, EventKind, Rng, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub mod air_depth_charge;
pub mod depth_charge;
pub mod projector;
pub use depth_charge::{
    begin_depth_charge_phase, control, depth_charge_own_view, is_depth_charge_configured,
    resolve_depth_charge_patterns, validate_action, Action,
};
pub use projector::{
    commit_projector, is_projector_configured, projector_own_view, resolve_projector_patterns,
    validate_projector, ProjectorFit, ProjectorPermit,
};

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/asw-weapons-2026.json"))
            .expect("source ASW tables")
    })
}
pub fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub standoffs: BTreeMap<String, StandoffFit>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub projectors: BTreeMap<String, ProjectorFit>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub depth_charges: BTreeMap<String, depth_charge::Fit>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub air_depth_charges: BTreeMap<String, air_depth_charge::Fit>,
    /// Exact scenario unit name, never an inferred class-wide hull design.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub submarine_hulls: BTreeMap<String, super::damage_runtime::AswHullFit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandoffFit {
    pub source: String,
    pub minimum_range_nm: f64,
    pub maximum_range_nm: f64,
    pub carrier_speed_kts: f64,
    pub guided: Option<bool>,
    pub payload: Payload,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Payload {
    Torpedo {
        weapon: String,
        speed_index: usize,
        entry_depth_m: f64,
    },
    Nuclear {
        yield_kt: u32,
    },
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub config: Configuration,
    pub flights: BTreeMap<EntityId, Flight>,
    pub pending: BTreeMap<EntityId, String>,
    #[serde(default, skip_serializing_if = "projector::State::is_empty")]
    pub projectors: projector::State,
    #[serde(default, skip_serializing_if = "depth_charge::State::is_empty")]
    pub depth_charges: depth_charge::State,
    #[serde(default, skip_serializing_if = "air_depth_charge::State::is_empty")]
    pub air_depth_charges: air_depth_charge::State,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Flight {
    pub shooter: EntityId,
    pub side: Side,
    pub contact: ContactId,
    pub target: EntityId,
    pub weapon: String,
    pub fit: StandoffFit,
    pub origin: Vec2,
    pub aim: Vec2,
    pub launch_range_nm: f64,
    pub quality: Quality,
    pub evidence: Value,
    pub tdl_modifier: u8,
    pub launched_at: i64,
    pub last_tick: i64,
    pub placement: Option<Placement>,
    pub terminal: bool,
    pub payload_position: Option<Vec2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub miss_scatter: Option<Scatter>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scatter {
    pub distance_d6: u8,
    pub direction_d6: u8,
    pub distance_nm: f64,
    pub direction_deg: f64,
}
/// Not deserializable: only a qualified current report can produce this permit.
#[derive(Clone, Debug)]
pub struct ReleasePermit {
    flight: Flight,
}
impl ReleasePermit {
    pub fn aim(&self) -> Vec2 {
        self.flight.aim
    }
    pub fn target(&self) -> EntityId {
        self.flight.target
    }
    pub fn weapon(&self) -> &str {
        &self.flight.weapon
    }
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let config = w.context.asw_weapons_2026.clone().unwrap_or_default();
    if (!config.standoffs.is_empty()
        || !config.projectors.is_empty()
        || !config.depth_charges.is_empty()
        || !config.air_depth_charges.is_empty()
        || !config.submarine_hulls.is_empty())
        && config.source.trim().is_empty()
    {
        return Err("8.5 ASW configuration source required".into());
    }
    for (weapon, f) in &config.standoffs {
        let def =
            w.db.weapons
                .get(weapon)
                .ok_or("8.5.2 carrier definition absent")?;
        if def.kind != WeaponKind::AswRocket || def.guidance != Guidance::AswBallistic {
            return Err("8.5.2 fit must bind actual ASW ballistic carrier".into());
        }
        if f.source.trim().is_empty()
            || !f.minimum_range_nm.is_finite()
            || !f.maximum_range_nm.is_finite()
            || f.minimum_range_nm < 0.0
            || f.maximum_range_nm < f.minimum_range_nm
            || !f.carrier_speed_kts.is_finite()
            || f.carrier_speed_kts <= 0.0
        {
            return Err("8.5.2 explicit finite range/speed/source required".into());
        }
        if let Payload::Torpedo {
            weapon,
            speed_index,
            entry_depth_m,
        } = &f.payload
        {
            let payload =
                w.db.weapons
                    .get(weapon)
                    .ok_or("8.5.2 physical payload definition absent")?;
            if !matches!(payload.kind, WeaponKind::Torpedo | WeaponKind::Lwt) {
                return Err("8.5.2 payload must be a physical torpedo, not carrier damage".into());
            }
            let p = w
                .rules
                .torpedoes
                .as_ref()
                .and_then(|s| s.config.weapons.get(weapon))
                .ok_or("8.5.2 source torpedo fit absent")?;
            if !matches!(
                p.guidance,
                super::torpedo2026::Guidance::ActivePassive | super::torpedo2026::Guidance::Passive
            ) || p.settings.get(*speed_index).is_none()
                || !entry_depth_m.is_finite()
                || *entry_depth_m < 0.0
                || *entry_depth_m > p.max_depth_m
            {
                return Err("8.5.2 unsupported payload guidance/setting/entry depth".into());
            }
        }
    }
    // Construct both domains before publishing either one: an invalid final
    // projector row must not leave an accepted prefix of the configuration.
    let projectors = projector::initialize(w, &config)?;
    let depth_charges = depth_charge::initialize(w, &config)?;
    let air_depth_charges = air_depth_charge::initialize(w, &config)?;
    // The common initializer reads original context, validates every exact
    // body binding, then publishes its complete ledger once. All ASW domain
    // constructors above have succeeded before either runtime is committed.
    super::damage_runtime::initialize_asw_hulls(w)?;
    w.rules.asw_weapons = Some(State {
        config,
        projectors,
        depth_charges,
        air_depth_charges,
        ..State::default()
    });
    Ok(())
}
pub fn range_modifier(range: f64) -> Result<i32, String> {
    if !range.is_finite() || range < 0.0 {
        return Err("8.5.2 range invalid".into());
    }
    tables()["standoff"]["range_rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| range >= r[0].as_f64().unwrap() && range <= r[1].as_f64().unwrap())
        .map(|r| r[2].as_i64().unwrap() as i32)
        .ok_or("8.5.2 no printed range band; no interpolation/rounding adopted".into())
}
pub fn placement_chance(
    quality: Quality,
    range: f64,
    guided: bool,
    tdl: u8,
    torpedo_coated: bool,
    nuclear_yield: Option<u32>,
) -> Result<i32, String> {
    if quality == Quality::NA || tdl > 2 {
        return Err("8.5.2 qualified FCS/TDL required".into());
    }
    let mut chance = tables()["standoff"]["placement_quality"][format!("{quality:?}")]
        .as_i64()
        .unwrap() as i32
        + range_modifier(range)?
        + i32::from(guided)
        + i32::from(tdl);
    if torpedo_coated && nuclear_yield.is_none() {
        chance -= 2;
    }
    if let Some(y) = nuclear_yield {
        chance += if y <= 20 {
            1
        } else if (100..=200).contains(&y) {
            2
        } else {
            return Err("8.5.2 nuclear yield modifier not printed".into());
        };
    }
    Ok(chance)
}
fn active_sensor(w: &World, sensor: &str) -> bool {
    w.db.sensors
        .get(sensor)
        .is_some_and(|s| matches!(s.kind, SensorKind::Radar | SensorKind::SonarActive))
}
pub fn validate_fire(
    w: &World,
    actor: EntityId,
    contact: ContactId,
    weapon: &str,
) -> Result<ReleasePermit, String> {
    if !active(w) {
        return Err("8.5.2 Full source profile required".into());
    }
    let e = w
        .entity(actor)
        .filter(|e| e.alive)
        .ok_or("8.5.2 own launcher unavailable")?;
    let c = w.sides[e.side.index()]
        .contacts
        .get(&contact)
        .filter(|c| !c.stale && !c.bearing_only)
        .ok_or("8.5.2 own current located ContactId required")?;
    if super::detection2026::known_category(w, e.side, c.about)
        != Some(crate::types::PlatformKind::Sub)
    {
        return Err("8.5.2 contact must actually be classified submarine".into());
    }
    let f = w
        .rules
        .asw_weapons
        .as_ref()
        .and_then(|s| s.config.standoffs.get(weapon))
        .cloned()
        .ok_or("8.5.2 carrier/payload source fit absent; legacy Pk forbidden")?;
    let guided = f.guided.ok_or("8.5.2 guided/unguided source unresolved")?;
    let nuclear_yield = match f.payload {
        Payload::Nuclear { yield_kt } => Some(yield_kt),
        _ => None,
    };
    if let Some(yield_kt) = nuclear_yield {
        super::nuclear2026::release_gate(w, actor, weapon)?;
        let warhead = w
            .rules
            .nuclear
            .as_ref()
            .and_then(|s| s.fits.get(&actor))
            .and_then(|s| s.get(weapon))
            .ok_or("8.5.2 nuclear payload requires an exact nuclear release binding")?;
        if warhead.medium != super::nuclear2026::Medium::Subsurface
            || warhead.selected_yield_kt != yield_kt
        {
            return Err(
                "8.5.2 nuclear ASW requires matching subsurface yield; no inferred warhead".into(),
            );
        }
    }
    let payload_parameters = if let Payload::Torpedo {
        weapon: payload,
        speed_index,
        ..
    } = &f.payload
    {
        let payload_fit = w
            .rules
            .torpedoes
            .as_ref()
            .and_then(|s| s.config.weapons.get(payload))
            .ok_or("8.5.2 source payload fit absent")?;
        let speed = payload_fit
            .settings
            .get(*speed_index)
            .ok_or("8.5.2 payload setting absent")?
            .speed_kts;
        Some((payload_fit.generation, speed))
    } else {
        None
    };
    let mut solutions: Vec<(Quality, Vec2, Value)> = Vec::new();
    if let Some(mad)=super::detection2026::wide_mad::good_fix(w,actor,c.about,None) {
        if let Some(aim)=mad.report.measured_position.filter(|aim|e.pos.range_to(*aim)<=15.) {
            solutions.push((Quality::Good,aim,json!({"source":mad.source,"sensor":mad.report.sensor,
                "observer":actor,"at":mad.report.at,"usable_turn":mad.usable_turn,
                "rule":"5.7.1 Good MAD fix in the next Tactical Turn; no invented ContactTrack or range column"})));
        }
    }
    if let Some(aaw) = &w.rules.aaw {
        for t in aaw
            .firecontrol
            .tracks
            .values()
            .filter(|t| t.observer == actor && t.target == c.about)
        {
            let Some(report) = &t.report else { continue };
            let Some(aim) = report.measured_position else {
                continue;
            };
            let range = e.pos.range_to(aim);
            if range > 15.0 && !active_sensor(w, &t.sensor) {
                continue;
            }
            let solution = if let Some((generation, speed)) = payload_parameters {
                super::firecontrol2026::own_sensor_solution(
                    w, actor, c.about, &t.sensor, generation, speed,
                )
            } else {
                super::firecontrol2026::own_ballistic_asw_solution(w, actor, c.about, &t.sensor)
            };
            if let Ok(solution) = solution {
                solutions.push((solution.quality,aim,json!({"source":"own_native","sensor":t.sensor,"observer":actor,"at":report.at,"active":active_sensor(w,&t.sensor),"solution":solution})));
            }
        }
    }
    if let Ok(Some(remote)) = super::communications2026::received_solution(w, actor, c.about) {
        if matches!(remote.domain, super::communications2026::FcsDomain::Asw)
            && remote.solution.is_some()
        {
            if let Some(aim) = remote.aim {
                if e.pos.range_to(aim) <= 15.0 || active_sensor(w, &remote.source_sensor) {
                    solutions.push((remote.quality,aim,json!({"source":"qualified_remote","sensor":remote.source_sensor,"observer":remote.source_observer,"formed_at":remote.formed_at,"refs":remote.refs,"solution":remote.solution})));
                }
            }
        }
    }
    let (quality, aim, evidence) = solutions
        .into_iter()
        .filter(|(q, _, _)| *q != Quality::NA)
        .max_by_key(|(q, _, _)| *q)
        .ok_or("8.5.2 no qualified native/remote FCS; >15nm requires active source")?;
    let range = e.pos.range_to(aim);
    if range < f.minimum_range_nm || range > f.maximum_range_nm {
        return Err("8.5.2 outside source carrier range".into());
    }
    let cue = super::communications2026::cue_evidence(
        w,
        actor,
        c.about,
        super::communications2026::CuePurpose::AswPlacement,
    );
    // Read-only placement feasibility uses no enemy signature/life/depth truth.
    placement_chance(quality, range, guided, cue.modifier, false, nuclear_yield)?;
    Ok(ReleasePermit {
        flight: Flight {
            shooter: actor,
            side: e.side,
            contact,
            target: c.about,
            weapon: weapon.into(),
            fit: f,
            origin: e.pos,
            aim,
            launch_range_nm: range,
            quality,
            evidence,
            tdl_modifier: cue.modifier,
            launched_at: w.tick,
            last_tick: w.tick,
            placement: None,
            terminal: false,
            payload_position: None,
            miss_scatter: None,
        },
    })
}
pub fn register_launch(w: &mut World, id: EntityId, permit: ReleasePermit) -> Result<(), String> {
    let body = w
        .entity(id)
        .and_then(|e| e.projectile.as_ref())
        .ok_or("8.5.2 actual carrier projectile required")?;
    let f = &permit.flight;
    if body.weapon != f.weapon
        || body.launched_by != f.shooter
        || body.target != Some(f.target)
        || body.aim != f.aim
        || w.tick != f.launched_at
    {
        return Err("8.5.2 permit/projectile mismatch".into());
    }
    if w.rules
        .asw_weapons
        .as_ref()
        .ok_or("8.5.2 runtime missing")?
        .flights
        .contains_key(&id)
    {
        return Err("8.5.2 projectile already registered".into());
    }
    let e = w.entity_mut(id).unwrap();
    e.course_deg = e.pos.bearing_to(f.aim);
    e.ordered_course_deg = e.course_deg;
    e.speed_kts = f.fit.carrier_speed_kts;
    e.ordered_speed_kts = f.fit.carrier_speed_kts;
    w.rules
        .asw_weapons
        .as_mut()
        .unwrap()
        .flights
        .insert(id, permit.flight);
    Ok(())
}
fn trace(w: &mut World, actor: EntityId, rule: &str, v: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "asw_weapons2026".into(),
            rule: rule.into(),
            side: None,
            unit: Some(actor),
            contact: None,
            detail: v.to_string(),
        },
        Visibility::Referee,
    )
}
fn die(w: &World, id: EntityId, at: i64, purpose: &str, sides: u8) -> u8 {
    let mut r = w.rng.derive(Rng::hash_tag(at, id.0, purpose));
    1 + (r.next_f64() * f64::from(sides)).floor() as u8
}
fn pending(w: &mut World, id: EntityId, reason: &str) {
    if let Some(s) = w.rules.asw_weapons.as_mut() {
        if s.pending.insert(id, reason.into()).is_none() {
            trace(
                w,
                id,
                "8.5.2.pending",
                json!({"projectile":id,"reason":reason}),
            );
        }
    }
}
/// Returns true for every Full ASW ballistic body, even unsupported fits. The
/// caller must never continue into the historical Pk implementation.
pub fn advance(w: &mut World, id: EntityId) -> Result<bool, String> {
    let result = advance_inner(w, id);
    if let Err(reason) = &result {
        pending(w, id, reason);
    }
    result
}
fn advance_inner(w: &mut World, id: EntityId) -> Result<bool, String> {
    if !active(w) {
        return Ok(false);
    }
    let Some(body) = w.entity(id).cloned() else {
        return Ok(true);
    };
    let Some(data) = body.projectile.as_ref() else {
        return Ok(true);
    };
    if !w
        .db
        .weapons
        .get(&data.weapon)
        .is_some_and(|d| d.guidance == Guidance::AswBallistic)
    {
        return Ok(false);
    }
    let Some(mut f) = w
        .rules
        .asw_weapons
        .as_ref()
        .and_then(|s| s.flights.get(&id))
        .cloned()
    else {
        pending(w, id, "8.5.2 source permit/fit missing; no legacy fallback");
        return Ok(true);
    };
    if f.terminal
        || w.rules
            .asw_weapons
            .as_ref()
            .unwrap()
            .pending
            .contains_key(&id)
    {
        return Ok(true);
    }
    let seconds = (w.tick - f.last_tick).max(0) as f64;
    let distance = body.pos.range_to(f.aim);
    let travel = f.fit.carrier_speed_kts * seconds / 3600.;
    f.last_tick = w.tick;
    let course = body.pos.bearing_to(f.aim);
    let arrived = distance <= travel + 1e-9;
    if let Some(e) = w.entity_mut(id) {
        e.pos = if arrived {
            f.aim
        } else {
            e.pos.advanced(course, travel)
        };
        e.course_deg = course;
        e.speed_kts = f.fit.carrier_speed_kts;
        if let Some(p) = e.projectile.as_mut() {
            p.live_s += seconds;
        }
    }
    w.rules
        .asw_weapons
        .as_mut()
        .unwrap()
        .flights
        .insert(id, f.clone());
    if !arrived {
        return Ok(true);
    }
    let nuclear_yield = match f.fit.payload {
        Payload::Nuclear { yield_kt } => Some(yield_kt),
        _ => None,
    };
    let torpedo_payload = match f.fit.payload.clone() {
        Payload::Torpedo {
            weapon,
            speed_index,
            entry_depth_m,
        } => Some((weapon, speed_index, entry_depth_m)),
        _ => None,
    };
    let coated = if nuclear_yield.is_none() {
        let target = w
            .entity(f.target)
            .ok_or("8.5.2 physical target metadata unavailable at entry")?;
        let cfg = w
            .rules
            .torpedoes
            .as_ref()
            .and_then(|s| s.config.targets.get(&target.class))
            .ok_or("8.5.2 physical target coating source absent")?;
        super::signature_damage2026::project_acoustic(w, f.target, Some(cfg.noise), cfg.anechoic)?.1
    } else {
        false
    };
    let chance = placement_chance(
        f.quality,
        f.launch_range_nm,
        f.fit.guided.unwrap(),
        f.tdl_modifier,
        coated,
        nuclear_yield,
    )?;
    let d10 = die(w, id, f.launched_at, "8.5.2.placement", 10);
    let placed = i32::from(d10) <= chance;
    let p = Placement {
        identity: format!("8.5.2:{}:{}", id.0, f.launched_at),
        tick: w.tick,
        d10,
        chance,
        placed,
        deviation_d6: [0, 0],
        course_deviation_deg: 0,
    };
    let position = if placed {
        f.aim
    } else {
        let distance_d6 = die(w, id, f.launched_at, "8.5.2.miss_range", 6);
        let direction_d6 = die(w, id, f.launched_at, "8.5.2.miss_direction", 6);
        let radius = f64::from(distance_d6) * if f.launch_range_nm <= 15.0 { 0.5 } else { 1.0 };
        let direction = f64::from(direction_d6) * 60.;
        f.miss_scatter = Some(Scatter {
            distance_d6,
            direction_d6,
            distance_nm: radius,
            direction_deg: direction,
        });
        f.aim.advanced(direction, radius)
    };
    f.placement = Some(p.clone());
    f.terminal = true;
    f.payload_position = Some(position);
    w.rules
        .asw_weapons
        .as_mut()
        .unwrap()
        .flights
        .insert(id, f.clone());
    trace(
        w,
        f.shooter,
        "8.5.2.placement",
        json!({"projectile":id,"carrier":f.weapon,"payload":f.fit.payload,"placement":p,"source_fcs":f.evidence,"tdl_modifier":f.tdl_modifier,"torpedo_coating":coated,"datum":position,"miss_scatter_is_source":!placed}),
    );
    if let Some(e) = w.entity_mut(id) {
        e.pos = position;
        if let Some((_, _, entry_depth_m)) = &torpedo_payload {
            e.depth_m = *entry_depth_m;
        }
    }
    if nuclear_yield.is_some() {
        super::nuclear2026::accept_standoff_nuclear(w, id)?;
        return Ok(true);
    }
    let (weapon, speed_index, _) = torpedo_payload.unwrap();
    if let Err(reason) = super::torpedo2026::accept_standoff_payload(
        w,
        id,
        &weapon,
        speed_index,
        f.target,
        f.quality,
        p,
    ) {
        pending(w, id, &reason);
        return Err(reason);
    }
    Ok(true)
}

/// Pure source calculator, not a completed physical projector transaction.
pub fn projector_thresholds(
    base: [Option<f64>; 3],
    quality: Quality,
    speed: f64,
    depth_column: usize,
    sea_state: u8,
    one_of_required_pair: bool,
) -> Result<[u32; 3], String> {
    if sea_state >= 7 || depth_column > 3 {
        return Err("8.5.3 sea state/depth column prohibits attack".into());
    }
    if quality == Quality::Poor {
        return Ok([0; 3]);
    }
    if quality == Quality::NA {
        return Err("8.5.3 no FCS".into());
    }
    let row = tables()["projector"]["speed_rows"]
        .as_array()
        .unwrap()
        .iter()
        .position(|r| {
            speed >= r[0].as_f64().unwrap() && (r[1].is_null() || speed <= r[1].as_f64().unwrap())
        })
        .ok_or("8.5.3 no printed speed row")?;
    let mut factor = tables()["projector"]["modifiers"][row][depth_column]
        .as_f64()
        .ok_or("8.5.3 -- attack not possible")?;
    if quality == Quality::Fair {
        factor *= 0.5
    }
    if one_of_required_pair {
        factor *= 0.5
    }
    let mut result = [0; 3];
    for (i, p) in base.into_iter().enumerate() {
        if let Some(p) = p {
            if !p.is_finite() || !(0.0..=100.0).contains(&p) {
                return Err("8.5.3 invalid source hit percentage".into());
            }
            result[i] = (p * factor).round() as u32;
        }
    }
    Ok(result)
}

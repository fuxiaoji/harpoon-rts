//! Source3.1.4 hard rudder: one-risk episode, same-side circling after a
//! natural jam, and the dedicated25% Resolution unjam. The ordinary weapon
//! Rudder critical (D6 mode, D6 delay, three20/40/60 attempts) stays a
//! separate cause and is never invoked from here.
use super::{tables, Fit};
use crate::{
    geom::normalize_deg, rules::damage_runtime, world::TICK_SECONDS, EntityId, EventKind,
    Visibility, World,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const CAUSE: &str = "hard_rudder_3_1_4";

/// Preserve both historical absence and already explicit empty objects. A
/// mutable business-state access materializes the field before it can be
/// saved, so a later reset is identical before and after restoring a save.
#[derive(Clone, Debug, Default)]
pub struct HardRudder {
    present: bool,
    state: HardRudderState,
}
impl HardRudder {
    pub fn is_absent(&self) -> bool {
        !self.present
    }
}
impl std::ops::Deref for HardRudder {
    type Target = HardRudderState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
impl std::ops::DerefMut for HardRudder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.present = true;
        &mut self.state
    }
}
impl Serialize for HardRudder {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.state.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for HardRudder {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            present: true,
            state: HardRudderState::deserialize(deserializer)?,
        })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HardRudderState {
    pub next_episode: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_episode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planned: Option<Planned>,
    pub recovery_automatic: bool,
    pub recovery_armed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_resolution: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<String>,
    pub episodes: BTreeMap<String, Episode>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Planned {
    pub course_deg: f64,
    pub direction: super::TurnSide,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub cause: String,
    pub activated_at: i64,
    pub direction: super::TurnSide,
    pub target_course_deg: f64,
    /// turning|jammed|freed|completed|replaced — private phase history.
    pub phase: String,
    pub risk: Risk,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jammed_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jam_window: Option<i64>,
    pub checks: BTreeMap<String, Check>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_reason: Option<String>,
    pub window: i64,
    pub charged_degrees: f64,
    pub advanced_nm: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Risk {
    pub purpose: String,
    pub d100: u8,
    pub jammed: bool,
    pub applied: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Check {
    pub closed_window: i64,
    pub at_tick: i64,
    pub purpose: String,
    pub d100: u8,
    pub freed: bool,
    pub applied: bool,
}

fn turn_seconds() -> i64 {
    tables()["turn_seconds"].as_i64().unwrap()
}
/// Hard-rudder columns are indexes2/3 of the same verified source rows.
fn row(f: &Fit) -> Result<(f64, f64, f64), String> {
    let data = &tables()["turning"];
    let pivot = if f.waterjet {
        &data["waterjet_pivot_deg"]
    } else {
        &data["pivot_deg"]
    }
    .as_f64()
    .unwrap();
    let size = if f.waterjet {
        let mut chars = "ABCDEFG".chars();
        while chars.next().is_some_and(|c| c != f.size) {}
        chars
            .next()
            .ok_or("3.1.4 no printed smaller size thanG for waterjet; adjudication required")?
    } else {
        f.size
    };
    let (category, key) = if f.merchant {
        (
            "merchant",
            match size {
                'A' => "a",
                'B' | 'C' => "b_c",
                'D' | 'E' => "d_e",
                'F' | 'G' => "fg",
                _ => return Err("3.1.4 invalid source size".into()),
            },
        )
    } else {
        (
            "warship",
            match size {
                'A' => "a",
                'B' => "b",
                'C' => "c",
                'D' | 'E' => "d_e",
                'F' | 'G' => {
                    if f.undamaged_max_kts
                        >= tables()["acceleration"]["fast_threshold_kts"]
                            .as_f64()
                            .unwrap()
                    {
                        "fast_fg"
                    } else {
                        "slow_fg"
                    }
                }
                _ => return Err("3.1.4 invalid source size".into()),
            },
        )
    };
    let r = &data[category][key];
    Ok((r[2].as_f64().unwrap(), r[3].as_f64().unwrap(), pivot))
}
fn die(seed: u64, unit: u32, purpose: &str) -> u8 {
    let mut rng = crate::Rng::new(seed).derive(crate::Rng::hash_tag(0, unit, purpose));
    1 + (rng.next_f64() * 100.) as u8
}

pub fn validate_order(w: &World, id: EntityId, course_deg: f64) -> Result<(), String> {
    if !super::configured(w, id) {
        return Err("3 source navigation unit fit required (v4)".into());
    }
    if !course_deg.is_finite() {
        return Err("3.1.4 hard-rudder course must be a finite number".into());
    }
    let e = w
        .entity(id)
        .filter(|e| e.alive)
        .ok_or("3 navigation unit unavailable")?;
    if e.kind == crate::types::PlatformKind::Sub {
        return Err(
            "3.1.4 submarine hard rudder needs the source depth-control/crush procedure; pending adjudication"
                .into(),
        );
    }
    if e.kind != crate::types::PlatformKind::Ship {
        return Err("3.1.4 hard rudder requires a surface ship".into());
    }
    let u = &w.rules.navigation.as_ref().unwrap().units[&id];
    // Resolve the source row during the shared readonly preflight. An
    // unsupported waterjet size must fail before an order can be queued or
    // transfer control, rather than after creating its plan/episode.
    row(&u.fit)?;
    if u.heave_to {
        return Err(
            "3.1.4/3.1.5 heave-to and hard rudder combination requires adjudication".into(),
        );
    }
    if super::astern::engaged(w, id) {
        return Err("3.1.3/3.1.4 end astern steering before hard rudder".into());
    }
    if super::crashback::active(w, id) {
        return Err("3.1.2/3.1.4 end crash back before hard rudder".into());
    }
    for key in [
        "rudder_hit",
        "damage.rudder_jammed",
        "damage.rudder_disabled",
        "damage.hard_rudder_jammed",
    ] {
        if e.ext.get(key).is_some_and(|v| *v > 0.) {
            return Err(
                "3.1.4 existing steering casualty keeps its own repair procedure; hard rudder not combined"
                    .into(),
            );
        }
    }
    if damage_runtime::bridge_holds(e, w.tick) {
        return Err(
            "3.1.4 damaged bridge keeps its hold and delays; hard rudder not combined".into(),
        );
    }
    Ok(())
}
pub fn validate_recovery(w: &World, id: EntityId) -> Result<(), String> {
    if !super::configured(w, id) {
        return Err("3 source navigation unit fit required (v4)".into());
    }
    w.entity(id)
        .filter(|e| e.alive)
        .ok_or_else(|| "3 navigation unit unavailable".to_string())?;
    Ok(())
}

pub fn order(
    w: &mut World,
    id: EntityId,
    course_deg: f64,
    direction: super::TurnSide,
) -> Result<(), String> {
    validate_order(w, id, course_deg)?;
    let course = normalize_deg(course_deg);
    let unit = &mut w
        .rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap();
    let hard = &mut unit.hard_rudder;
    // A live jam keeps physics and its original risk; a repeated order is a
    // retained future intent and never re-rolls or flips the locked side.
    if let Some(active) = hard.active_episode.as_ref() {
        if hard.episodes[active].phase == "jammed" {
            return Ok(());
        }
    }
    if let Some(active) = hard.active_episode.take() {
        let ep = hard.episodes.get_mut(&active).unwrap();
        if ep.direction != direction {
            ep.phase = "replaced".into();
            ep.ended_at = Some(w.tick);
            ep.ended_reason = Some("opposite_side_new_order".into());
        } else {
            // Same-side retarget continues the same episode and its original
            // risk identity.
            ep.target_course_deg = course;
            hard.active_episode = Some(active);
            return Ok(());
        }
    }
    hard.planned = Some(Planned {
        course_deg: course,
        direction,
    });
    Ok(())
}
pub fn set_recovery(w: &mut World, id: EntityId, automatic: bool) -> Result<(), String> {
    validate_recovery(w, id)?;
    w.rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap()
        .hard_rudder
        .recovery_automatic = automatic;
    Ok(())
}
pub fn arm_free(w: &mut World, id: EntityId) -> Result<(), String> {
    validate_recovery(w, id)?;
    w.rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap()
        .hard_rudder
        .recovery_armed = true;
    Ok(())
}
/// A new ordinary steering order replaces only an un-jammed hard plan; a live
/// jam episode keeps its circling physics and repair clock untouched.
pub fn replaced_by_standard_order(w: &mut World, id: EntityId) {
    let Some(state) = w.rules.navigation.as_mut() else {
        return;
    };
    let Some(unit) = state.units.get_mut(&id) else {
        return;
    };
    let hard = &mut unit.hard_rudder;
    hard.planned = None;
    if let Some(active) = hard.active_episode.take() {
        let ep = hard.episodes.get_mut(&active).unwrap();
        if ep.phase == "turning" {
            ep.phase = "completed".into();
            ep.ended_at = Some(w.tick);
            ep.ended_reason = Some("standard_order".into());
        } else {
            hard.active_episode = Some(active);
        }
    }
}

/// Hard rudder owns this unit's steering and displacement while a plan is
/// pending actuation or an episode is actually turning/circling.
pub fn controls_movement(w: &World, id: EntityId) -> bool {
    let Some(unit) = w.rules.navigation.as_ref().and_then(|s| s.units.get(&id)) else {
        return false;
    };
    if unit.hard_rudder.planned.is_some() {
        return true;
    }
    unit.hard_rudder
        .active_episode
        .as_ref()
        .is_some_and(|active| {
            let phase = &unit.hard_rudder.episodes[active].phase;
            phase == "turning" || phase == "jammed"
        })
}

fn referee_trace(w: &mut World, id: EntityId, rule: &str, value: Value) {
    let side = w.entity(id).map(|e| e.side);
    w.emit(
        EventKind::RulesEvent {
            domain: "navigation2026".into(),
            rule: rule.into(),
            side,
            unit: Some(id),
            contact: None,
            detail: value.to_string(),
        },
        Visibility::Referee,
    );
}

/// Movement integration: activation risk, source advance legs,45/60-degree
/// pivots with the per-Tac loss budget, same-side circling while jammed.
pub fn advance(w: &mut World, id: EntityId) -> Result<(), String> {
    let mut runtime = w
        .rules
        .navigation
        .take()
        .ok_or("3.1.4 navigation runtime missing")?;
    let result = advance_inner(w, id, &mut runtime);
    w.rules.navigation = Some(runtime);
    result
}
fn advance_inner(w: &mut World, id: EntityId, runtime: &mut super::State) -> Result<(), String> {
    let turn = turn_seconds();
    let Some(unit) = runtime.units.get_mut(&id) else {
        return Ok(());
    };
    let fit = unit.fit.clone();
    let hard = &mut unit.hard_rudder;
    let mut e = match w.entity(id) {
        Some(e) => e.clone(),
        None => return Ok(()),
    };
    // First actual steering actuates the rudder: allocate the episode and
    // roll its single source risk. A zero-speed plan only waits.
    if hard.planned.is_some() {
        if !e.alive || e.speed_kts <= 1e-12 {
            return Ok(());
        }
        let planned = hard.planned.take().unwrap();
        hard.next_episode += 1;
        let identity = format!("3.1.4.hard:{}:{}", id.0, hard.next_episode);
        let purpose = format!("{identity}:risk");
        let d100 = die(w.seed, id.0, &purpose);
        let jammed = d100 <= tables()["turning"]["hard_jam_percent"].as_f64().unwrap() as u8;
        let phase = if jammed { "jammed" } else { "turning" }.to_string();
        let episode = Episode {
            id: identity.clone(),
            cause: CAUSE.into(),
            activated_at: w.tick,
            direction: planned.direction,
            target_course_deg: planned.course_deg,
            phase,
            risk: Risk {
                purpose: purpose.clone(),
                d100,
                jammed,
                applied: true,
            },
            jammed_at: jammed.then_some(w.tick),
            jam_window: jammed.then_some(w.tick / turn),
            checks: BTreeMap::new(),
            ended_at: None,
            ended_reason: None,
            window: w.tick / turn,
            charged_degrees: 0.,
            advanced_nm: 0.,
        };
        referee_trace(
            w,
            id,
            "3.1.4.hard_rudder_risk",
            json!({"identity":identity,"purpose":purpose,"d100":d100,"jammed":jammed,
                "jam_percent":tables()["turning"]["hard_jam_percent"],
                "source":fit.source,"activated_at":w.tick,"direction":planned.direction,
                "target_course_deg":planned.course_deg}),
        );
        if jammed {
            damage_runtime::register_hard_rudder_jam(w, id, &identity)?;
            w.emit(
                EventKind::RulesEvent {
                    domain: "navigation2026".into(),
                    rule: "3.1.4.hard_rudder_jammed".into(),
                    side: Some(e.side),
                    unit: Some(id),
                    contact: None,
                    detail: "Hard rudder jammed: the ship keeps circling to the ordered side until freed".into(),
                },
                Visibility::Side(e.side),
            );
        }
        hard.episodes.insert(identity.clone(), episode);
        hard.active_episode = Some(identity);
    }
    let Some(active) = hard.active_episode.clone() else {
        return Ok(());
    };
    let phase_now = hard.episodes[&active].phase.clone();
    if phase_now != "turning" && phase_now != "jammed" {
        return Ok(());
    }
    let window = w.tick / turn;
    if hard.episodes.get_mut(&active).unwrap().window != window {
        let ep = hard.episodes.get_mut(&active).unwrap();
        ep.window = window;
        ep.charged_degrees = 0.;
    }
    let (advance_yd, loss_per45, max_pivot) = row(&fit)?;
    let distance = advance_yd * tables()["turning"]["yards_to_nm"].as_f64().unwrap();
    let source45 = tables()["turning"]["pivot_deg"].as_f64().unwrap();
    let maximum = tables()["turning"]["maximum_loss_pivots"].as_f64().unwrap() * source45;
    let mut remaining = TICK_SECONDS;
    let mut events: Vec<Value> = Vec::new();
    let mut land_stop = false;
    let mut completed = false;
    while remaining > 1e-12 {
        let (phase, target, direction) = {
            let ep = &hard.episodes[&active];
            (ep.phase.clone(), ep.target_course_deg, ep.direction)
        };
        if phase == "turning" {
            let sign = match direction {
                super::TurnSide::Starboard => 1.,
                super::TurnSide::Port => -1.,
            };
            if ((target - e.course_deg) * sign).rem_euclid(360.) <= 1e-9 {
                completed = true;
                break;
            }
        }
        if e.speed_kts <= 1e-12 {
            break;
        }
        let sign = match direction {
            super::TurnSide::Starboard => 1.,
            super::TurnSide::Port => -1.,
        };
        let needed = (distance - hard.episodes[&active].advanced_nm).max(0.);
        let available = e.speed_kts * remaining / 3600.;
        let travel = available.min(needed);
        if !super::turning::segment(w, &mut e, travel) {
            land_stop = true;
            break;
        }
        hard.episodes.get_mut(&active).unwrap().advanced_nm += travel;
        remaining = (remaining - travel / e.speed_kts * 3600.).max(0.);
        if hard.episodes[&active].advanced_nm + 1e-12 < distance {
            break;
        }
        let angle = if phase == "jammed" {
            max_pivot
        } else {
            max_pivot.min(((target - e.course_deg) * sign).rem_euclid(360.))
        };
        let charged = angle.min((maximum - hard.episodes[&active].charged_degrees).max(0.));
        let loss = loss_per45 * charged / source45;
        let from = e.course_deg;
        e.course_deg = normalize_deg(from + sign * angle);
        e.speed_kts = (e.speed_kts - loss).max(0.);
        {
            let ep = hard.episodes.get_mut(&active).unwrap();
            ep.charged_degrees += charged;
            ep.advanced_nm = 0.;
            events.push(
                json!({"source":fit.source,"advance_yd":advance_yd,"advance_nm":distance,
                "from_course_deg":from,"to_course_deg":e.course_deg,"angle_deg":angle,
                "speed_loss_per45_kts":loss_per45,"speed_loss_kts":loss,
                "charged_degrees":ep.charged_degrees,"turn_window":window,
                "pivot_position":e.pos,"waterjet":fit.waterjet,"jammed":phase=="jammed",
                "partial_loss_policy":"rts_proportional_to_printed45deg"}),
            );
        }
    }
    if completed {
        hard.active_episode = None;
        e.ordered_course_deg = hard.episodes[&active].target_course_deg;
        let ep = hard.episodes.get_mut(&active).unwrap();
        ep.phase = "completed".into();
        ep.ended_at = Some(w.tick);
        ep.ended_reason = Some("target_reached".into());
        let left = e.speed_kts * remaining / 3600.;
        land_stop = !super::turning::segment(w, &mut e, left);
    }
    if land_stop {
        hard.episodes.get_mut(&active).unwrap().advanced_nm = 0.;
        let last = e.ext.get("terrain.last_block").copied().unwrap_or(-100000.);
        if w.tick as f64 - last >= 30. {
            e.ext.insert("terrain.last_block".into(), w.tick as f64);
            w.emit(crate::EventKind::RulesEvent{domain:"navigation".into(),rule:"land_stop".into(),side:Some(e.side),unit:Some(id),contact:None,
                detail:"Charted coast blocks a hard-rudder advance/pivot segment; stopped before land. Grounding damage not inferred; a jam keeps circling when motion resumes.".into()},crate::Visibility::Side(e.side));
        }
    }
    if !events.is_empty() {
        // Register actual pivots in their own Tactical Turn, as standard
        // steering does. Sampling the changed heading on the next tick would
        // charge a tick179 pivot to the fresh acceleration window at180.
        unit.turn_degrees += events
            .iter()
            .filter_map(|event| event["angle_deg"].as_f64())
            .sum::<f64>();
        unit.prior_course = e.course_deg;
    }
    w.entities.insert(id, e);
    for event in events {
        referee_trace(w, id, "3.1.4.hard_rudder_pivot", event);
    }
    Ok(())
}

/// Consume the just-closed180s window once per Tactical Turn, before that
/// tick's commands and movement. A jam from windowW is eligible at the
/// resolution ofW (including its own turn of jam).
pub fn resolution(w: &mut World) {
    if !super::active(w) {
        return;
    }
    let turn = turn_seconds();
    if w.tick <= 0 || w.tick % turn != 0 {
        return;
    }
    let closed = w.tick / turn - 1;
    let ids: Vec<EntityId> = w
        .rules
        .navigation
        .as_ref()
        .map(|s| s.units.keys().copied().collect())
        .unwrap_or_default();
    let release = tables()["turning"]["jam_release_percent_per_turn"]
        .as_f64()
        .unwrap();
    for id in ids {
        let active = {
            let Some(unit) = w.rules.navigation.as_ref().and_then(|s| s.units.get(&id)) else {
                continue;
            };
            let hard = &unit.hard_rudder;
            if hard.last_resolution.is_some_and(|seen| seen >= closed) {
                continue;
            }
            hard.active_episode.clone()
        };
        {
            let hard = &mut w
                .rules
                .navigation
                .as_mut()
                .unwrap()
                .units
                .get_mut(&id)
                .unwrap()
                .hard_rudder;
            hard.last_resolution = Some(closed);
        }
        let Some(active) = active else { continue };
        let (phase, jam_window, has_check, automatic, armed) = {
            let hard = &w.rules.navigation.as_ref().unwrap().units[&id].hard_rudder;
            let ep = &hard.episodes[&active];
            (
                ep.phase.clone(),
                ep.jam_window,
                ep.checks.contains_key(&closed.to_string()),
                hard.recovery_automatic,
                hard.recovery_armed,
            )
        };
        if phase != "jammed"
            || jam_window.is_some_and(|window| window > closed)
            || has_check
            || (!automatic && !armed)
        {
            continue;
        }
        let purpose = format!("{active}:free:{closed}");
        let d100 = die(w.seed, id.0, &purpose);
        let freed = (d100 as f64) <= release;
        {
            let hard = &mut w
                .rules
                .navigation
                .as_mut()
                .unwrap()
                .units
                .get_mut(&id)
                .unwrap()
                .hard_rudder;
            hard.episodes.get_mut(&active).unwrap().checks.insert(
                closed.to_string(),
                Check {
                    closed_window: closed,
                    at_tick: w.tick,
                    purpose: purpose.clone(),
                    d100,
                    freed,
                    applied: true,
                },
            );
            hard.recovery_armed = false;
        }
        let side = w.entity(id).map(|e| e.side);
        w.emit(
            EventKind::RulesEvent {
                domain: "navigation2026".into(),
                rule: "3.1.4.hard_rudder_free".into(),
                side,
                unit: Some(id),
                contact: None,
                detail: json!({"identity":active,"purpose":purpose,"closed_window":closed,
                    "d100":d100,"release_percent":release,"freed":freed,
                    "authorization":if automatic {"automatic"} else {"manual_slot"}})
                .to_string(),
            },
            Visibility::Referee,
        );
        if freed {
            let current = w.entity(id).map(|e| e.course_deg);
            if let Err(reason) = damage_runtime::clear_hard_rudder_jam(w, id, &active) {
                let hard = &mut w
                    .rules
                    .navigation
                    .as_mut()
                    .unwrap()
                    .units
                    .get_mut(&id)
                    .unwrap()
                    .hard_rudder;
                hard.pending = Some(reason);
            }
            {
                let hard = &mut w
                    .rules
                    .navigation
                    .as_mut()
                    .unwrap()
                    .units
                    .get_mut(&id)
                    .unwrap()
                    .hard_rudder;
                if let Some(ep) = hard.episodes.get_mut(&active) {
                    ep.phase = "freed".into();
                    ep.ended_at = Some(w.tick);
                    ep.ended_reason = Some("resolution_freed".into());
                }
                hard.active_episode = None;
            }
            if let (Some(course), Some(e)) = (current, w.entity_mut(id)) {
                e.ordered_course_deg = course;
            }
            if let Some(side) = side {
                w.emit(
                    EventKind::RulesEvent {
                        domain: "navigation2026".into(),
                        rule: "3.1.4.hard_rudder_freed".into(),
                        side: Some(side),
                        unit: Some(id),
                        contact: None,
                        detail:
                            "Rudder freed at Resolution; steering stabilizes on the current heading"
                                .into(),
                    },
                    Visibility::Side(side),
                );
            }
        }
    }
}

/// Public own-equipment whitelist. Raw identities, dice, and ledgers stay
/// referee-only.
pub fn steering_view(hard: &HardRudder, tick: i64) -> Value {
    let turn = turn_seconds();
    let active = hard
        .active_episode
        .as_ref()
        .and_then(|id| hard.episodes.get(id));
    let (mode, target, direction) = if let Some(ep) = active {
        match ep.phase.as_str() {
            "turning" => ("turning", Some(ep.target_course_deg), Some(ep.direction)),
            "jammed" => ("jammed", Some(ep.target_course_deg), Some(ep.direction)),
            _ => ("standard", None, None),
        }
    } else if let Some(planned) = hard.planned {
        ("planned", Some(planned.course_deg), Some(planned.direction))
    } else {
        ("standard", None, None)
    };
    let jammed = active.is_some_and(|ep| ep.phase == "jammed");
    let authorized = jammed && (hard.recovery_automatic || hard.recovery_armed);
    json!({
        "mode": mode,
        "target_course_deg": target,
        "direction": direction,
        "jammed": jammed,
        "recovery": {
            "automatic": hard.recovery_automatic,
            "armed": hard.recovery_armed,
            "next_resolution_at": authorized.then(|| (tick / turn + 1) * turn),
        },
        "pending": hard.pending,
    })
}

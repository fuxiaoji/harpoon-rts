//! Source3.3/3.4 physical encounters. No weapon/Contact provenance is fabricated.
use super::{active, Fit, Motion};
use crate::rules::damage_runtime::{
    queue_navigation_impact, NavigationImpact, NavigationImpactCause,
};
use crate::types::PlatformKind;
use crate::{EntityId, EventKind, Rng, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub collisions: bool,
    pub visibility_percent: Option<f64>,
    pub units: BTreeMap<String, HullFit>,
    pub grounding: Option<GroundingConfiguration>,
    pub joined_speed_policy: Option<JoinedSpeedPolicy>,
    pub aspect_policy: Option<AspectPolicy>,
    pub relative_speed_policy: Option<RelativeSpeedPolicy>,
    pub increment_policy: Option<IncrementPolicy>,
    pub initial_activities: Vec<Activity>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HullFit {
    pub source: String,
    pub civilian: Option<bool>,
    pub armor_points: Option<f64>,
    pub reinforced_bow: Option<bool>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinedSpeedPolicy {
    Faster,
    Slower,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectPolicy {
    #[serde(rename = "referenced_10_2")]
    Referenced10_2,
    CollisionDiagram30,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelativeSpeedPolicy {
    HeadingHemisphere,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncrementPolicy {
    CompleteGroups,
    Proportional,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundingConfiguration {
    pub source: String,
    pub shorelines: Vec<Vec<Vec2>>,
    #[serde(default)]
    pub safe_areas: Vec<SafeArea>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SafeArea {
    pub source: String,
    pub kind: SafeKind,
    pub polygon: Vec<Vec2>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafeKind {
    Harbor,
    Channel,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    pub source: String,
    pub kind: ActivityKind,
    pub units: [String; 2],
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Towing,
    DamageControlAssistance,
    UnderwayReplenishment,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub config: Configuration,
    pub fingerprint: u64,
    pub classes: BTreeMap<EntityId, String>,
    pub hulls: BTreeMap<EntityId, HullFit>,
    pub snapshots: BTreeMap<EntityId, Pose>,
    pub motion_ends: BTreeMap<EntityId, Pose>,
    pub encounters: BTreeMap<String, Encounter>,
    pub groundings: BTreeMap<EntityId, Grounded>,
    pub grounding_trials: BTreeMap<EntityId, i64>,
    pub bow_damage: BTreeSet<EntityId>,
    pub joined: BTreeMap<String, Joined>,
    pub rams: BTreeMap<EntityId, Ram>,
    pub ram_plots: BTreeMap<EntityId, (i64, f64)>,
    pub activities: Vec<(EntityId, EntityId, ActivityKind)>,
    pub pending: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pose {
    pub pos: Vec2,
    pub course: f64,
    pub speed: f64,
    pub depth: f64,
    pub side: Side,
    pub kind: PlatformKind,
    pub hp: f64,
    pub maximum_dp: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Encounter {
    pub sequence: u64,
    pub active: bool,
    pub resolved_at: i64,
    pub dice: [u8; 2],
    pub modified: i32,
    pub collided: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grounded {
    pub at: i64,
    pub refloat_at: i64,
    pub damage_d6: u8,
    pub freeing_d6: [u8; 2],
    pub source_speed_kts: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Joined {
    pub a: EntityId,
    pub b: EntityId,
    pub rammer: EntityId,
    pub course: f64,
    pub speed: Option<f64>,
    pub deceleration: f64,
    pub moved_at: i64,
    pub backing_seconds: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ram {
    pub contact: crate::ContactId,
    pub target: EntityId,
    pub aim: Vec2,
    pub plotted_at: i64,
    pub speed_kts: f64,
}
fn table() -> &'static Value {
    &super::tables()["hazards"]
}
fn n(group: &str, key: &str) -> f64 {
    table()[group][key]
        .as_f64()
        .expect("verified navigation hazard parameter")
}
fn c(key: &str) -> f64 {
    n("collision", key)
}
fn d(key: &str) -> f64 {
    table()["collision"]["damage"][key]
        .as_f64()
        .expect("source collision damage parameter")
}
fn age() -> i64 {
    table()["tactical_seconds"].as_i64().unwrap()
}
fn yards() -> f64 {
    table()["yards_to_nm"].as_f64().unwrap()
}
fn fingerprint(config: &Configuration) -> u64 {
    serde_json::to_vec(config)
        .unwrap()
        .iter()
        .fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
        })
}
fn unique(w: &World, name: &str) -> Result<EntityId, String> {
    let v: Vec<_> = w.entities.values().filter(|e| e.name == name).collect();
    if v.len() != 1 || !matches!(v[0].kind, PlatformKind::Ship | PlatformKind::Sub) {
        return Err("3.3/3.4 exact unique actual ship/submarine required".into());
    }
    Ok(v[0].id)
}
pub(super) fn initialize(
    w: &World,
    nav: &super::State,
    config: Configuration,
) -> Result<State, String> {
    if config.source.trim().is_empty()
        || config
            .visibility_percent
            .is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v))
    {
        return Err("3.3/3.4 source and finite explicit visibility required".into());
    }
    let mut s = State {
        fingerprint: fingerprint(&config),
        config: config.clone(),
        classes: BTreeMap::new(),
        hulls: BTreeMap::new(),
        snapshots: BTreeMap::new(),
        motion_ends: BTreeMap::new(),
        encounters: BTreeMap::new(),
        groundings: BTreeMap::new(),
        grounding_trials: BTreeMap::new(),
        bow_damage: BTreeSet::new(),
        joined: BTreeMap::new(),
        rams: BTreeMap::new(),
        ram_plots: BTreeMap::new(),
        activities: vec![],
        pending: BTreeMap::new(),
    };
    for (id, _) in &nav.units {
        s.classes.insert(
            *id,
            w.entity(*id)
                .ok_or("navigation hull missing")?
                .class
                .clone(),
        );
    }
    for (name, fit) in &config.units {
        let id = unique(w, name)?;
        if !nav.units.contains_key(&id)
            || fit.source.trim().is_empty()
            || fit.armor_points.is_some_and(|v| !v.is_finite() || v < 0.0)
        {
            return Err("3.3 actual typed navigation size and source/armor fit required".into());
        }
        s.hulls.insert(id, fit.clone());
    }
    if let Some(g) = &config.grounding {
        if g.source.trim().is_empty()
            || g.shorelines.is_empty()
            || g.shorelines.iter().any(|line| {
                line.len() < 2 || line.iter().any(|p| !p.x.is_finite() || !p.y.is_finite())
            })
            || g.safe_areas.iter().any(|a| {
                a.source.trim().is_empty()
                    || a.polygon.len() < 3
                    || a.polygon
                        .iter()
                        .any(|p| !p.x.is_finite() || !p.y.is_finite())
            })
        {
            return Err("3.4 explicit finite shore and harbor/channel geometry required; no guessed bathymetry".into());
        }
    }
    for a in &config.initial_activities {
        let x = unique(w, &a.units[0])?;
        let y = unique(w, &a.units[1])?;
        if x == y
            || a.source.trim().is_empty()
            || w.entity(x).unwrap().side != w.entity(y).unwrap().side
            || !nav.units.contains_key(&x)
            || !nav.units.contains_key(&y)
        {
            return Err("3.3 initial close-operation relation requires explicit source and two actual own fitted hulls".into());
        }
        s.activities.push((x, y, a.kind));
    }
    Ok(s)
}
fn state(w: &World) -> Option<&State> {
    if !active(w) {
        return None;
    }
    w.rules.navigation.as_ref()?.hazards.as_ref()
}
fn take(w: &mut World) -> Option<State> {
    w.rules.navigation.as_mut()?.hazards.take()
}
fn put(w: &mut World, s: State) {
    if let Some(n) = w.rules.navigation.as_mut() {
        n.hazards = Some(s)
    }
}
fn checked(w: &World, s: &State) -> bool {
    w.context
        .navigation_2026
        .as_ref()
        .and_then(|n| n.hazards.as_ref())
        .is_some_and(|c| fingerprint(c) == s.fingerprint)
}
fn pose(e: &crate::world::Entity) -> Pose {
    Pose {
        pos: e.pos,
        course: e.course_deg,
        speed: e.speed_kts,
        depth: e.depth_m,
        side: e.side,
        kind: e.kind,
        hp: e.hp,
        maximum_dp: e.hp_max,
    }
}
fn fit(w: &World, id: EntityId) -> Option<&Fit> {
    w.rules.navigation.as_ref()?.units.get(&id).map(|u| &u.fit)
}
fn size(w: &World, id: EntityId) -> Option<usize> {
    "ABCDEFG".find(fit(w, id)?.size)
}
fn private(w: &mut World, id: EntityId, rule: &str, value: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "navigation2026".into(),
            rule: rule.into(),
            side: w.entity(id).map(|e| e.side),
            unit: Some(id),
            contact: None,
            detail: value.to_string(),
        },
        Visibility::Referee,
    )
}
fn pending(w: &mut World, s: &mut State, key: String, id: EntityId, reason: String) {
    if s.pending.get(&key) != Some(&reason) {
        s.pending.insert(key, reason.clone());
        private(w, id, "3.hazard_pending", json!({"reason":reason}))
    }
}
fn roll(w: &World, at: i64, id: EntityId, purpose: &str, sides: u8) -> u8 {
    let mut r = w.rng.derive(Rng::hash_tag(at, id.0, purpose));
    (r.next_f64() * f64::from(sides)).floor() as u8 + 1
}
fn dice(w: &World, at: i64, id: EntityId, purpose: &str, sides: u8, count: usize) -> Vec<u8> {
    let mut r = w.rng.derive(Rng::hash_tag(at, id.0, purpose));
    (0..count)
        .map(|_| (r.next_f64() * f64::from(sides)).floor() as u8 + 1)
        .collect()
}
pub fn grounded(w: &World, id: EntityId) -> bool {
    state(w).is_some_and(|s| s.groundings.contains_key(&id))
}
pub(super) fn held(w: &World, id: EntityId) -> bool {
    state(w).is_some_and(|s| {
        s.groundings.contains_key(&id) || s.joined.values().any(|j| j.a == id || j.b == id)
    })
}
fn reverse(w: &World, id: EntityId) -> bool {
    w.rules
        .navigation
        .as_ref()
        .and_then(|n| n.units.get(&id))
        .and_then(|u| u.astern.as_ref())
        .is_some_and(|a| a.moving_astern)
}
fn reverse_requested(w: &World, id: EntityId) -> bool {
    w.rules
        .navigation
        .as_ref()
        .and_then(|n| n.units.get(&id))
        .and_then(|u| u.astern.as_ref())
        .is_some_and(|a| a.requested)
}
pub fn before_movement(w: &mut World) {
    let Some(mut s) = take(w) else { return };
    if !checked(w, &s) {
        pending(
            w,
            &mut s,
            "configuration".into(),
            EntityId(0),
            "3.3/3.4 source configuration changed; physical adjudication blocked".into(),
        );
        put(w, s);
        return;
    }
    s.groundings
        .retain(|id, _| w.entity(*id).is_some_and(|e| e.alive));
    s.activities.retain(|(a, b, _)| {
        let threshold = if size(w, *a)
            .zip(size(w, *b))
            .is_some_and(|(x, y)| x >= 5 || y >= 5)
        {
            c("small_risk_yards")
        } else {
            c("large_risk_yards")
        } * yards();
        w.entity(*a).zip(w.entity(*b)).is_some_and(|(x, y)| {
            x.alive && y.alive && x.side == y.side && x.pos.range_to(y.pos) <= threshold
        })
    });
    let freed: Vec<_> = s
        .groundings
        .iter()
        .filter(|(_, g)| w.tick >= g.refloat_at)
        .map(|(id, _)| *id)
        .collect();
    for id in freed {
        if super::super::beached2026::active(w,id) {
            if let Err(reason)=super::super::beached2026::refloat(w,id,true) {
                super::super::operations::blocked_once(w,id,"14.2.4.refloat_blocked",&reason);
                continue;
            }
        }
        s.groundings.remove(&id);
        private(w, id, "3.4.refloated", json!({"at":w.tick}));
    }
    s.joined.retain(|_, j| {
        [j.a, j.b]
            .iter()
            .all(|id| w.entity(*id).is_some_and(|e| e.alive))
    });
    for (id, r) in &mut s.rams {
        if let Some(e) = w.entity(*id).filter(|e| e.alive) {
            if let Some(contact) = w.sides[e.side.index()]
                .contacts
                .get(&r.contact)
                .filter(|c| !c.stale && !c.bearing_only)
            {
                r.aim = contact.estimate;
            }
            let course = e.pos.bearing_to(r.aim);
            if let Some(e) = w.entity_mut(*id) {
                e.ordered_course_deg = course;
            }
        }
    }
    s.snapshots = w
        .entities
        .values()
        .filter(|e| {
            e.alive
                && matches!(e.kind, PlatformKind::Ship | PlatformKind::Sub)
                && !crate::rules::ew2026::is_object(w, e.id)
        })
        .map(|e| (e.id, pose(e)))
        .collect();
    s.motion_ends.clear();
    put(w, s);
}
/// External contact displacement precedes self-propulsion, steering and depth.
/// A pushed powerless hull does not acquire a free self-propulsion source.
pub fn move_external(w: &mut World, id: EntityId) -> Result<bool, String> {
    let Some(mut s) = take(w) else {
        return Ok(false);
    };
    let result: Result<bool, String> = (|| {
        if !checked(w, &s) {
            return Err("3.3/3.4 changed source configuration".into());
        }
        if s.groundings.contains_key(&id) {
            if let Some(e) = w.entity_mut(id) {
                e.speed_kts = 0.;
            }
            return Ok(true);
        }
        let Some(key) = s
            .joined
            .iter()
            .find(|(_, j)| j.a == id || j.b == id)
            .map(|(k, _)| k.clone())
        else {
            return Ok(false);
        };
        let j = s.joined.get_mut(&key).unwrap();
        if j.moved_at == w.tick {
            return Ok(true);
        }
        if s.groundings.contains_key(&j.a) || s.groundings.contains_key(&j.b) {
            return Err("3.3 joined contact displacement is pending a physical constraint".into());
        }
        let speed = j
            .speed
            .ok_or("3.3 joined common initial speed requires explicit source/RTS policy")?;
        if speed <= 1e-9 && reverse_requested(w, j.rammer) {
            if id == j.rammer {
                return Ok(false);
            }
            if let Some(e) = w.entity_mut(id) {
                e.speed_kts = 0.;
            }
            return Ok(true);
        }
        let next = (speed - j.deceleration).max(0.);
        let land = [j.a, j.b].iter().any(|member| {
            w.entity(*member).is_some_and(|e| {
                w.context.terrain.as_ref().is_some_and(|chart| {
                    chart.blocked(e.pos, e.pos.advanced(j.course, next / 3600.))
                })
            })
        });
        if land {
            j.speed = Some(0.);
            j.moved_at = w.tick;
            for member in [j.a, j.b] {
                if let Some(e) = w.entity_mut(member) {
                    e.speed_kts = 0.;
                }
            }
            return Err(
                "3.3 paired path meets the public land chart; stopped without invented land damage"
                    .into(),
            );
        }
        j.speed = Some(next);
        j.moved_at = w.tick;
        for member in [j.a, j.b] {
            if let Some(e) = w.entity_mut(member).filter(|e| e.alive) {
                e.course_deg = j.course;
                e.ordered_course_deg = j.course;
                e.speed_kts = next;
                e.pos = e.pos.advanced(j.course, next / 3600.);
            }
        }
        Ok(true)
    })();
    put(w, s);
    result
}
pub(super) fn constrain(w: &World, id: EntityId, mut m: Motion) -> Result<Motion, String> {
    if let Some(s) = state(w) {
        if s.bow_damage.contains(&id) {
            m.maximum_kts *= d("bow_maximum_fraction");
            m.minimum_kts = m.minimum_kts.min(m.maximum_kts);
        }
        if s.groundings.contains_key(&id) {
            m.maximum_kts = 0.;
            m.minimum_kts = 0.;
            m.target_kts = Some(0.);
        }
        if let Some(r) = s.rams.get(&id) {
            m.target_kts = Some(r.speed_kts);
        }
    }
    Ok(m)
}
pub(super) fn validate_ram(
    w: &World,
    id: EntityId,
    contact: crate::ContactId,
    speed: Option<f64>,
) -> Result<(), String> {
    let s = state(w).ok_or("3.3 explicit collision configuration required")?;
    if !s.config.collisions || !s.hulls.contains_key(&id) || grounded(w, id) {
        return Err("3.3 source-fitted movable rammer required".into());
    }
    let e = w
        .entity(id)
        .filter(|e| e.alive && matches!(e.kind, PlatformKind::Ship | PlatformKind::Sub))
        .ok_or("3.3 rammer unavailable")?;
    if bridge(e, w.tick) {
        return Err("3.3 an unrepaired Bridge casualty prohibits ramming".into());
    }
    let c = w.sides[e.side.index()]
        .contacts
        .get(&contact)
        .filter(|c| {
            !c.stale && !c.bearing_only && c.estimate.x.is_finite() && c.estimate.y.is_finite()
        })
        .ok_or("3.3 own current located target contact required")?;
    if c.about == id {
        return Err("3.3 cannot ram self".into());
    }
    let speed = speed.unwrap_or(e.speed_kts);
    if !speed.is_finite()
        || speed < 0.
        || speed
            > fit(w, id)
                .ok_or("source navigation fit missing")?
                .undamaged_max_kts
    {
        return Err("3.3 finite source-bounded plotted ram speed required".into());
    }
    if s.ram_plots.get(&id).is_some_and(|(window, prior)| {
        *window == w.tick.div_euclid(age()) && (*prior - speed).abs() > 1e-9
    }) {
        return Err(
            "3.3 plotted ram speed is fixed during the actual Tactical movement turn".into(),
        );
    }
    Ok(())
}
pub(super) fn order_ram(
    w: &mut World,
    id: EntityId,
    contact: crate::ContactId,
    speed: Option<f64>,
) -> Result<(), String> {
    validate_ram(w, id, contact, speed)?;
    let e = w.entity(id).unwrap();
    let c = &w.sides[e.side.index()].contacts[&contact];
    let r = Ram {
        contact,
        target: c.about,
        aim: c.estimate,
        plotted_at: w.tick,
        speed_kts: speed.unwrap_or(e.speed_kts),
    };
    let s = w
        .rules
        .navigation
        .as_mut()
        .unwrap()
        .hazards
        .as_mut()
        .unwrap();
    s.ram_plots
        .insert(id, (w.tick.div_euclid(age()), r.speed_kts));
    s.rams.insert(id, r);
    Ok(())
}
pub(super) fn cancel_ram(w: &mut World, id: EntityId) {
    if let Some(s) = w.rules.navigation.as_mut().and_then(|n| n.hazards.as_mut()) {
        s.rams.remove(&id);
    }
}
fn bridge(e: &crate::world::Entity, now: i64) -> bool {
    e.ext
        .get("damage.bridge_order_delay")
        .copied()
        .unwrap_or(0.)
        > 0.
        || e.ext.get("damage.bridge_until").copied().unwrap_or(0.) > now as f64
}
fn rudder(e: &crate::world::Entity, include_hard: bool) -> bool {
    e.ext.get("damage.rudder_disabled").copied().unwrap_or(0.) > 0.
        || e.ext.get("damage.rudder_jammed").copied().unwrap_or(0.) > 0.
        || include_hard
            && e.ext
                .get("damage.hard_rudder_jammed")
                .copied()
                .unwrap_or(0.)
                > 0.
}
fn dot(a: Vec2, b: Vec2) -> f64 {
    a.x * b.x + a.y * b.y
}
fn lerp(a: Vec2, b: Vec2, t: f64) -> Vec2 {
    a.add(b.sub(a).scaled(t))
}
fn closest(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> (f64, f64) {
    let r = a0.sub(b0);
    let v = a1.sub(a0).sub(b1.sub(b0));
    let vv = dot(v, v);
    let t = if vv > 1e-20 {
        (-dot(r, v) / vv).clamp(0., 1.)
    } else {
        0.
    };
    (r.add(v.scaled(t)).norm(), t)
}
fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let v = b.sub(a);
    let vv = dot(v, v);
    let t = if vv > 1e-20 {
        dot(p.sub(a), v) / vv
    } else {
        0.
    };
    p.range_to(lerp(a, b, t.clamp(0., 1.)))
}
fn cross(a: Vec2, b: Vec2) -> f64 {
    a.x * b.y - a.y * b.x
}
fn intersection_t(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<f64> {
    let r = b.sub(a);
    let q = d.sub(c);
    let den = cross(r, q);
    if den.abs() < 1e-12 {
        return None;
    }
    let ca = c.sub(a);
    let t = cross(ca, q) / den;
    let u = cross(ca, r) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}
fn edge_distance(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> f64 {
    if intersection_t(a, b, c, d).is_some() {
        0.
    } else {
        segment_distance(a, c, d)
            .min(segment_distance(b, c, d))
            .min(segment_distance(c, a, b))
            .min(segment_distance(d, a, b))
    }
}
fn inside(poly: &[Vec2], p: Vec2) -> bool {
    let mut yes = false;
    for (a, b) in poly
        .iter()
        .copied()
        .zip(poly.iter().copied().cycle().skip(1))
        .take(poly.len())
    {
        if segment_distance(p, a, b) < 1e-10 {
            return true;
        }
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            yes = !yes;
        }
    }
    yes
}
fn shore_distance(g: &GroundingConfiguration, from: Vec2, to: Vec2) -> Option<f64> {
    let mut split = vec![0., 1.];
    for area in &g.safe_areas {
        for (a, b) in area
            .polygon
            .iter()
            .copied()
            .zip(area.polygon.iter().copied().cycle().skip(1))
            .take(area.polygon.len())
        {
            if let Some(t) = intersection_t(from, to, a, b) {
                split.push(t);
            }
        }
    }
    split.sort_by(f64::total_cmp);
    split.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
    let mut distance = f64::INFINITY;
    for pair in split.windows(2) {
        let a = lerp(from, to, pair[0]);
        let b = lerp(from, to, pair[1]);
        let middle = lerp(a, b, 0.5);
        if g.safe_areas.iter().any(|s| inside(&s.polygon, middle)) {
            continue;
        }
        for line in &g.shorelines {
            for edge in line.windows(2) {
                distance = distance.min(edge_distance(a, b, edge[0], edge[1]));
            }
        }
    }
    distance.is_finite().then_some(distance / yards())
}
fn native_visual(w: &World, observer: EntityId, target: EntityId) -> bool {
    w.rules.aaw.as_ref().is_some_and(|a| {
        a.firecontrol.tracks.values().any(|t| {
            t.observer == observer
                && t.target == target
                && w.tick - t.last <= age()
                && t.last <= w.tick
                && t.report.as_ref().is_some_and(|r| {
                    r.observer == observer && r.at == t.last && r.measured_position.is_some()
                })
                && w.entity(observer).is_some_and(|e| {
                    e.sensors.iter().any(|s| {
                        s.sensor == t.sensor && !crate::sensors::sensor_damaged(e, &s.sensor)
                    })
                })
                && w.db
                    .sensors
                    .get(&t.sensor)
                    .is_some_and(|s| s.kind == crate::db::SensorKind::Visual)
        })
    })
}
fn formation(w: &World, a: EntityId, b: EntityId) -> bool {
    let x = w.entity(a).and_then(|e| e.station.as_ref());
    let y = w.entity(b).and_then(|e| e.station.as_ref());
    x.is_some_and(|s| s.leader == b)
        || y.is_some_and(|s| s.leader == a)
        || x.zip(y).is_some_and(|(x, y)| x.leader == y.leader)
}
fn mobility(w: &World, id: EntityId) -> bool {
    let Some(e) = w.entity(id) else { return false };
    bridge(e, w.tick)
        || rudder(e, true)
        || e.ext.get("damage.power_lost").copied().unwrap_or(0.) > 0.
        || crate::rules::damage_runtime::platform(&e.class).is_some_and(|p| {
            p.dp_breaks.iter().enumerate().any(|(i, threshold)| {
                *threshold > 0.
                    && e.hp_max - e.hp >= *threshold
                    && p.speed_steps
                        .get(i)
                        .is_some_and(|v| p.speed_steps.first().is_some_and(|top| v < top))
            })
        })
}
fn ratio_modifier(ratio: f64) -> i32 {
    let rows = table()["collision"]["ram"]["speed_ratio_rows"]
        .as_array()
        .unwrap();
    if ratio < rows[0][0].as_f64().unwrap() {
        return rows[0][1].as_i64().unwrap() as i32;
    }
    rows.iter()
        .skip(1)
        .find(|row| ratio <= row[0].as_f64().unwrap())
        .unwrap_or(rows.last().unwrap())[1]
        .as_i64()
        .unwrap() as i32
}
fn chance_modifier(
    w: &World,
    s: &State,
    a: EntityId,
    b: EntityId,
    rammer: Option<EntityId>,
) -> Result<i32, String> {
    let ae = w.entity(a).ok_or("collision body missing")?;
    let be = w.entity(b).ok_or("collision body missing")?;
    if let Some(r) = rammer {
        let t = if r == a { b } else { a };
        let re = w.entity(r).unwrap();
        let te = w.entity(t).unwrap();
        if bridge(re, w.tick) {
            return Err("3.3 unrepaired rammer Bridge prohibits attempt".into());
        }
        let data = &table()["collision"]["ram"];
        let mut m = data["attempt"].as_i64().unwrap() as i32;
        if rudder(re, true) {
            m += data["rammer_rudder"].as_i64().unwrap() as i32;
        }
        if te.kind == PlatformKind::Sub && te.depth_m > 0. {
            if !native_visual(w, r, t) {
                return Err(
                    "3.3 submerged ram target requires rammer actual Visual detection".into(),
                );
            }
            m += data["submerged_visual_target"].as_i64().unwrap() as i32;
        }
        let target_speed = s.motion_ends.get(&t).map_or(te.speed_kts, |p| p.speed);
        let ram_speed = s.motion_ends.get(&r).map_or(re.speed_kts, |p| p.speed);
        if target_speed <= 1e-9 {
            m += data["target_diw"].as_i64().unwrap() as i32;
        } else {
            if ram_speed <= 1e-9 {
                return Err(
                    "3.3 no printed ram speed-ratio denominator at zero actual speed".into(),
                );
            }
            if bridge(te, w.tick) {
                m += data["target_bridge"].as_i64().unwrap() as i32;
            }
            if rudder(te, true) {
                m += data["target_rudder"].as_i64().unwrap() as i32;
            }
            m += ratio_modifier(target_speed / ram_speed);
        }
        Ok(m)
    } else {
        let data = &table()["collision"]["accidental"];
        let vis = s
            .config
            .visibility_percent
            .ok_or("3.3 source visibility percent unknown")?;
        let mut m = if vis <= data["low_visibility_percent"].as_f64().unwrap() {
            data["low_visibility"].as_i64().unwrap() as i32
        } else {
            0
        };
        for e in [ae, be] {
            let sz = size(w, e.id).ok_or("3.3 physical hull size unknown")?;
            if bridge(e, w.tick) {
                m += data["bridge_each"].as_i64().unwrap() as i32;
            }
            if rudder(e, false) {
                m += data["rudder_each"].as_i64().unwrap() as i32;
            }
            if sz >= 3 {
                m += data["small_each"].as_i64().unwrap() as i32;
            }
            if sz == 0 {
                m += data["large_each"].as_i64().unwrap() as i32;
            }
            if s.motion_ends.get(&e.id).map_or(e.speed_kts, |p| p.speed)
                > data["fast_threshold_kts"].as_f64().unwrap()
            {
                m += data["fast_each"].as_i64().unwrap() as i32;
            }
        }
        if formation(w, a, b) && (mobility(w, a) || mobility(w, b)) {
            m += data["formation_mobility_casualty"].as_i64().unwrap() as i32;
        }
        Ok(m)
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Aspect {
    Narrow,
    Quarter,
    Broad,
}
fn aspect(s: &State, angle: f64) -> Result<Aspect, String> {
    let angle = angle.abs();
    let narrow = match s.config.aspect_policy {
        Some(AspectPolicy::CollisionDiagram30) => d("illustrative_bow_angle_degrees"),
        _ => d("narrow_half_angle_degrees"),
    };
    if s.config.aspect_policy.is_none()
        && angle > d("narrow_half_angle_degrees")
        && angle <= d("illustrative_bow_angle_degrees")
    {
        return Err(
            "3.3 referenced20deg versus collision30deg diagram boundary requires explicit policy"
                .into(),
        );
    }
    if angle <= narrow || angle >= d("stern_narrow_min_degrees") {
        Ok(Aspect::Narrow)
    } else if angle >= d("broad_min_degrees") && angle <= d("broad_max_degrees") {
        Ok(Aspect::Broad)
    } else {
        Ok(Aspect::Quarter)
    }
}
fn increment(value: f64, step: f64, policy: Option<IncrementPolicy>) -> Result<f64, String> {
    let q = value / step;
    if (q - q.round()).abs() < 1e-9 {
        return Ok(q.round());
    }
    match policy {
        Some(IncrementPolicy::CompleteGroups) => Ok(q.floor()),
        Some(IncrementPolicy::Proportional) => Ok(q),
        None => Err("3.3 fractional printed speed/armor increment requires explicit policy".into()),
    }
}
fn collision_damage(
    w: &mut World,
    s: &mut State,
    a: EntityId,
    b: EntityId,
    t: f64,
    seq: u64,
    rammer: Option<EntityId>,
) -> Result<(), String> {
    let mut ae = w.entity(a).ok_or("collision hull unavailable")?.clone();
    let mut be = w.entity(b).ok_or("collision hull unavailable")?.clone();
    if let Some(p) = s.motion_ends.get(&a) {
        ae.speed_kts = p.speed;
    }
    if let Some(p) = s.motion_ends.get(&b) {
        be.speed_kts = p.speed;
    }
    let unknown = || HullFit {
        source: "unknown_runtime_hull_fit".into(),
        civilian: None,
        armor_points: None,
        reinforced_bow: None,
    };
    let af = s.hulls.get(&a).cloned().unwrap_or_else(unknown);
    let bf = s.hulls.get(&b).cloned().unwrap_or_else(unknown);
    let a0 = s
        .snapshots
        .get(&a)
        .cloned()
        .ok_or("3.3 physical motion snapshot absent")?;
    let b0 = s
        .snapshots
        .get(&b)
        .cloned()
        .ok_or("3.3 physical motion snapshot absent")?;
    let ap = lerp(a0.pos, ae.pos, t);
    let bp = lerp(b0.pos, be.pos, t);
    let bearing = if ap.range_to(bp) > 1e-9 {
        ap.bearing_to(bp)
    } else {
        a0.pos.bearing_to(b0.pos)
    };
    let aa = aspect(s, crate::geom::angle_diff(ae.course_deg, bearing))?;
    let ba = aspect(
        s,
        crate::geom::angle_diff(be.course_deg, (bearing + 180.) % 360.),
    )?;
    let (kind, striker) = if aa == Aspect::Narrow && ba == Aspect::Narrow {
        ("glancing", None)
    } else if aa == Aspect::Broad && ba == Aspect::Narrow {
        ("bow_on", Some(b))
    } else if ba == Aspect::Broad && aa == Aspect::Narrow {
        ("bow_on", Some(a))
    } else if aa == Aspect::Quarter || ba == Aspect::Quarter {
        ("quarter", None)
    } else {
        return Err("3.3 broad/broad contact geometry has no printed resolution".into());
    };
    let identity = format!("3.3.collision:{}:{}:{seq}", a.0, b.0);
    if kind == "quarter" {
        let sa = size(w, a).unwrap();
        let sb = size(w, b).unwrap();
        for (id, own, other) in [(a, sa, sb), (b, sb, sa)] {
            let difference = own as i32 - other as i32;
            let loss = (d("quarter_speed_loss_fraction")
                + if difference > 0 {
                    d("size_loss_increment") * difference as f64
                } else {
                    -d("size_loss_increment") * ((-difference) / 2) as f64
                })
            .clamp(0., 1.);
            if let Some(e) = w.entity_mut(id) {
                e.speed_kts *= 1. - loss;
            }
        }
    }
    if let Some(striking) = striker {
        if !s.bow_damage.insert(striking) {
            pending(w,s,format!("bow_repeat:{}",striking.0),striking,"3.3 repeated distinct bow maximum-speed compounding is not printed; original permanent25% remains, additional damage is committed".into());
        }
        // Until direct damage is resolved, do not guess severing versus a
        // joined velocity. Known bow max damage is already applied above.
        s.joined.insert(
            identity.clone(),
            Joined {
                a,
                b,
                rammer: striking,
                course: ae.course_deg,
                speed: None,
                deceleration: 0.,
                moved_at: w.tick,
                backing_seconds: 0,
            },
        );
    }
    let ad = dice(
        w,
        w.tick,
        a,
        &format!("3.3.damage:{a:?}:{b:?}:{seq}:a"),
        d("dice_sides") as u8,
        d("dice_count") as usize,
    );
    let bd = dice(
        w,
        w.tick,
        b,
        &format!("3.3.damage:{a:?}:{b:?}:{seq}:b"),
        d("dice_sides") as u8,
        d("dice_count") as usize,
    );
    let ar: f64 = ad.iter().map(|v| f64::from(*v)).sum();
    let br: f64 = bd.iter().map(|v| f64::from(*v)).sum();
    private(
        w,
        a,
        "3.3.damage_rolls",
        json!({"identity":identity,"other":b,"dice_a":ad,"dice_b":bd,"collision_type":kind}),
    );
    let relative = if let Some(id) = striker {
        if id == a {
            ae.speed_kts
        } else {
            be.speed_kts
        }
    } else {
        let delta = crate::geom::angle_diff(ae.course_deg, be.course_deg).abs();
        let opposite = if delta < 1e-8 {
            false
        } else if (delta - 180.).abs() < 1e-8 {
            true
        } else {
            match s.config.relative_speed_policy {
                Some(RelativeSpeedPolicy::HeadingHemisphere) => delta > 90.,
                None => {
                    return Err(
                        "3.3 oblique relative-speed interpretation requires explicit policy".into(),
                    )
                }
            }
        };
        let v = if opposite {
            ae.speed_kts + be.speed_kts
        } else {
            (ae.speed_kts - be.speed_kts).abs()
        };
        if kind == "quarter" {
            v * d("quarter_relative_multiplier")
        } else {
            v
        }
    };
    let speed_add = increment(relative, d("speed_step_kts"), s.config.increment_policy)?
        * d("percent_per_speed_step");
    let armor = |recipient: &HullFit, other: &HullFit, other_id: EntityId| -> Result<f64, String> {
        let points = recipient
            .armor_points
            .ok_or("3.3 recipient armor points unknown")?;
        if points == 0. {
            return Ok(0.);
        }
        if kind != "glancing" && striker == Some(other_id) {
            match other.reinforced_bow {
                Some(true) => return Ok(0.),
                Some(false) => {}
                None => return Err("3.3 reinforced striking bow applicability unknown".into()),
            }
        }
        Ok(
            increment(points, d("armor_step_points"), s.config.increment_policy)?
                * d("armor_step_percent"),
        )
    };
    let pct_ab = (ar + speed_add - armor(&bf, &af, a)?).max(0.);
    let pct_ba = (br + speed_add - armor(&af, &bf, b)?).max(0.);
    let effective = |p: &Pose, f: &HullFit| -> Result<f64, String> {
        Ok(p.maximum_dp
            * if f
                .civilian
                .ok_or("3.3 civilian hull applicability unknown")?
            {
                d("civilian_effective_dp_multiplier")
            } else {
                1.
            })
    };
    let mut to_b = effective(&a0, &af)? * pct_ab / 100.;
    let mut to_a = effective(&b0, &bf)? * pct_ba / 100.;
    if striker == Some(a) {
        to_a *= d("bow_received_multiplier")
    }
    if striker == Some(b) {
        to_b *= d("bow_received_multiplier")
    }
    if !to_a.is_finite() || !to_b.is_finite() {
        return Err("3.3 physical percent damage overflow; not committed".into());
    }
    queue_navigation_impact(
        w,
        NavigationImpact {
            target: a,
            identity: format!("{identity}:a"),
            direct_dp: to_a,
            cause: NavigationImpactCause::Collision { other: b, rammer },
        },
    )?;
    queue_navigation_impact(
        w,
        NavigationImpact {
            target: b,
            identity: format!("{identity}:b"),
            direct_dp: to_b,
            cause: NavigationImpactCause::Collision { other: a, rammer },
        },
    )?;
    if let Some(striking) = striker {
        let other = if striking == a { b } else { a };
        let severed = if other == a {
            to_a >= a0.hp
        } else {
            to_b >= b0.hp
        };
        if severed {
            s.joined.remove(&identity);
            if let Some(e) = w.entity_mut(striking) {
                e.speed_kts *= d("cut_speed_fraction");
            }
        } else {
            let sa = size(w, a).unwrap();
            let sb = size(w, b).unwrap();
            let larger = if sa < sb {
                a
            } else if sb < sa {
                b
            } else if ae.speed_kts >= be.speed_kts {
                a
            } else {
                b
            };
            let heading = w.entity(larger).unwrap().course_deg;
            let speed = match s.config.joined_speed_policy {
                Some(JoinedSpeedPolicy::Faster) => Some(ae.speed_kts.max(be.speed_kts)),
                Some(JoinedSpeedPolicy::Slower) => Some(ae.speed_kts.min(be.speed_kts)),
                None => None,
            };
            let decel =
                super::acceleration(fit(w, larger).unwrap(), w.entity(larger).unwrap().speed_kts)?
                    [2]
                    / age() as f64
                    * d("joined_deceleration_multiplier");
            s.joined.insert(
                identity.clone(),
                Joined {
                    a,
                    b,
                    rammer: striking,
                    course: heading,
                    speed,
                    deceleration: decel,
                    moved_at: w.tick,
                    backing_seconds: 0,
                },
            );
        }
    }
    private(
        w,
        a,
        "3.3.collision",
        json!({"identity":identity,"other":b,"rammer":rammer,"collision_type":kind,"relative_speed_kts":relative,"damage_to_a":to_a,"damage_to_b":to_b,"damage_percent_a_to_b":pct_ab,"damage_percent_b_to_a":pct_ba,"source_d3d10_totals":[ar,br],"physical_phase":"post_actual_motion_common_batch"}),
    );
    Ok(())
}
fn collision_step(w: &mut World, s: &mut State) {
    if !s.config.collisions {
        return;
    }
    let ids: Vec<_> = s.snapshots.keys().copied().collect();
    let mut contacted: BTreeMap<EntityId, u32> = BTreeMap::new();
    for i in 0..ids.len() {
        for j in i + 1..ids.len() {
            let (a, b) = (ids[i], ids[j]);
            if !s.hulls.contains_key(&a) && !s.hulls.contains_key(&b) {
                continue;
            }
            let Some(ae) = w.entity(a).filter(|e| e.alive) else {
                continue;
            };
            let Some(be) = w.entity(b).filter(|e| e.alive) else {
                continue;
            };
            if s.classes.get(&a).is_some_and(|class| class != &ae.class)
                || s.classes.get(&b).is_some_and(|class| class != &be.class)
            {
                pending(
                    w,
                    s,
                    format!("class:{}:{}", a.0, b.0),
                    a,
                    "3.3 original source hull class binding changed".into(),
                );
                continue;
            }
            if s.joined
                .values()
                .any(|p| (p.a == a && p.b == b) || (p.a == b && p.b == a))
            {
                continue;
            }
            let (a0, b0) = (&s.snapshots[&a], &s.snapshots[&b]);
            let (minimum, t) = closest(a0.pos, ae.pos, b0.pos, be.pos);
            let key = format!("{}:{}", a.0, b.0);
            let sz = size(w, a).zip(size(w, b));
            let threshold = if sz.is_some_and(|(x, y)| x >= 5 || y >= 5) {
                c("small_risk_yards")
            } else {
                c("large_risk_yards")
            } * yards();
            if minimum > threshold {
                if let Some(e) = s.encounters.get_mut(&key) {
                    if e.active {
                        e.active = false;
                        e.sequence += 1;
                    }
                }
                continue;
            }
            if sz.is_none() {
                pending(
                    w,
                    s,
                    key.clone(),
                    a,
                    "3.3 close physical hull has no exact source size/fit; no default adjudication"
                        .into(),
                );
                continue;
            }
            let rammer = s
                .rams
                .get(&a)
                .filter(|r| r.target == b)
                .map(|_| a)
                .or_else(|| s.rams.get(&b).filter(|r| r.target == a).map(|_| b));
            if ae.kind == PlatformKind::Sub && ae.depth_m > c("periscope_max_m")
                || be.kind == PlatformKind::Sub && be.depth_m > c("periscope_max_m")
            {
                continue;
            }
            if ae.kind == PlatformKind::Sub
                && be.kind == PlatformKind::Sub
                && (ae.depth_m > 0. || be.depth_m > 0.)
                && !(ae.depth_m > 0. && be.depth_m > 0. && rammer.is_some())
            {
                continue;
            }
            if rammer.is_none()
                && s.activities.iter().any(|(x, y, _)| {
                    (*x == a && *y == b || *x == b && *y == a) && ae.side == be.side
                })
            {
                continue;
            }
            let relative_motion = ae.pos.sub(a0.pos).sub(be.pos.sub(b0.pos)).norm();
            if relative_motion < 1e-12 {
                continue;
            }
            if s.encounters.get(&key).is_some_and(|e| e.active) {
                continue;
            }
            let seq = s.encounters.get(&key).map_or(0, |e| e.sequence);
            let modifier = match chance_modifier(w, s, a, b, rammer) {
                Ok(m) => m,
                Err(reason) => {
                    pending(w, s, key.clone(), a, reason);
                    continue;
                }
            };
            let rolls = dice(w, w.tick, a, &format!("3.3.contact.{key}.{seq}"), 10, 2);
            let modified = i32::from(rolls[0]) + i32::from(rolls[1]) + modifier;
            let collided = modified >= c("threshold") as i32;
            let mut encounter = Encounter {
                sequence: seq,
                active: true,
                resolved_at: w.tick,
                dice: [rolls[0], rolls[1]],
                modified,
                collided,
            };
            if ae.pos.range_to(be.pos) > threshold {
                encounter.active = false;
                encounter.sequence += 1;
            }
            s.encounters.insert(key.clone(), encounter);
            private(
                w,
                a,
                "3.3.encounter",
                json!({"other":b,"sequence":seq,"dice":rolls,"modifier":modifier,"modified":modified,"collided":collided,"minimum_distance_yards":minimum/yards(),"rammer":rammer}),
            );
            if collided {
                // The plotted approach has physically ended. Keep this turn's
                // speed ledger, but let a later backing order free the hulls.
                s.rams
                    .retain(|id, r| !((*id == a && r.target == b) || (*id == b && r.target == a)));
                *contacted.entry(a).or_default() += 1;
                *contacted.entry(b).or_default() += 1;
                if let Err(reason) = collision_damage(w, s, a, b, t, seq, rammer) {
                    pending(w, s, format!("damage:{key}:{seq}"), a, reason);
                }
            }
        }
    }
    for j in s.joined.values_mut() {
        if contacted.get(&j.a).copied().unwrap_or(0) > 1
            || contacted.get(&j.b).copied().unwrap_or(0) > 1
        {
            j.speed = None;
        }
    }
}
fn grounding_step(w: &mut World, s: &mut State) {
    let Some(g) = s.config.grounding.clone() else {
        return;
    };
    let window = (w.tick - 1).div_euclid(age());
    let stable_roll_at = (window + 1) * age();
    let ids: Vec<_> = s.classes.keys().copied().collect();
    for id in ids {
        if s.groundings.contains_key(&id) || s.grounding_trials.get(&id) == Some(&window) {
            continue;
        }
        let Some(mut e) = w
            .entity(id)
            .filter(|e| e.alive && matches!(e.kind, PlatformKind::Ship | PlatformKind::Sub))
            .cloned()
        else {
            continue;
        };
        if s.classes.get(&id) != Some(&e.class) {
            pending(
                w,
                s,
                format!("ground_class:{}", id.0),
                id,
                "3.4 original source hull class binding changed".into(),
            );
            continue;
        }
        if let Some(p) = s.motion_ends.get(&id) {
            e.speed_kts = p.speed;
        }
        let Some(before) = s.snapshots.get(&id) else {
            continue;
        };
        let Some(distance) = shore_distance(&g, before.pos, e.pos) else {
            continue;
        };
        if distance > 2000. {
            continue;
        }
        let Some(mut sz) = size(w, id) else {
            pending(
                w,
                s,
                format!("ground_size:{}", id.0),
                id,
                "3.4 hull size unknown".into(),
            );
            continue;
        };
        if e.kind == PlatformKind::Sub {
            if sz == 0 {
                pending(
                    w,
                    s,
                    format!("ground_size:{}", id.0),
                    id,
                    "3.4 submarine one larger thanA has no printed modifier".into(),
                );
                continue;
            }
            sz -= 1;
        }
        let row = table()["grounding"]["distance_rows_yards"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| distance >= r[0].as_f64().unwrap() && distance <= r[1].as_f64().unwrap());
        let Some(row) = row else {
            pending(
                w,
                s,
                format!("ground_gap:{}", id.0),
                id,
                "3.4 continuous distance lies in an unprinted500..501/1000..1001 yard gap".into(),
            );
            continue;
        };
        let letter = "ABCDEFG".chars().nth(sz).unwrap().to_string();
        let chance = (row[2].as_f64().unwrap()
            + table()["grounding"]["size_modifiers"][letter]
                .as_f64()
                .unwrap())
        .clamp(0., 100.);
        let die = roll(
            w,
            stable_roll_at,
            id,
            &format!("3.4.grounding.{window}"),
            100,
        );
        s.grounding_trials.insert(id, window);
        private(
            w,
            id,
            "3.4.grounding_trial",
            json!({"window":window,"d100":die,"chance":chance,"shore_yards":distance,"sample_policy":"first actual exposure in each Tactical Turn; original stable window-end RNG slot"}),
        );
        if f64::from(die) > chance {
            continue;
        }
        let damage = roll(w, w.tick, id, &format!("3.4.grounding_damage.{window}"), 6);
        let free = dice(w, w.tick, id, &format!("3.4.grounding_free.{window}"), 6, 2);
        let due = w.tick
            + i64::from(free[0] + free[1]) * table()["intermediate_seconds"].as_i64().unwrap();
        let dp = f64::from(damage) * e.speed_kts;
        if let Err(reason) = queue_navigation_impact(
            w,
            NavigationImpact {
                target: id,
                identity: format!("3.4.grounding:{}:{window}", id.0),
                direct_dp: dp,
                cause: NavigationImpactCause::Grounding,
            },
        ) {
            pending(w, s, format!("ground_damage:{}:{window}", id.0), id, reason);
            continue;
        }
        s.groundings.insert(
            id,
            Grounded {
                at: w.tick,
                refloat_at: due,
                damage_d6: damage,
                freeing_d6: [free[0], free[1]],
                source_speed_kts: e.speed_kts,
            },
        );
        if let Some(e) = w.entity_mut(id) {
            e.speed_kts = 0.;
            e.ordered_speed_kts = 0.;
            e.station = None;
            e.waypoints.clear();
        }
        if let Err(reason)=super::super::beached2026::mark(w,id,Some(due)) {
            super::super::operations::blocked_once(w,id,"14.2.4.grounded_scope_pending",&reason);
        }
        private(
            w,
            id,
            "3.4.grounded",
            json!({"at":w.tick,"shore_yards":distance,"damage_d6":damage,"impact_speed_kts":e.speed_kts,"direct_dp":dp,"freeing_d6":free,"refloat_at":due}),
        );
    }
}
pub fn after_movement(w: &mut World) {
    let Some(mut s) = take(w) else { return };
    if checked(w, &s) {
        s.motion_ends = w
            .entities
            .values()
            .filter(|e| e.alive && matches!(e.kind, PlatformKind::Ship | PlatformKind::Sub))
            .map(|e| (e.id, pose(e)))
            .collect();
        collision_step(w, &mut s);
        grounding_step(w, &mut s);
        let keys: Vec<_> = s.joined.keys().cloned().collect();
        for key in keys {
            let j = &s.joined[&key];
            let moving = reverse(w, j.rammer)
                && s.snapshots
                    .get(&j.rammer)
                    .zip(w.entity(j.rammer))
                    .is_some_and(|(a, b)| a.pos.range_to(b.pos) > 1e-12 && b.speed_kts > 0.);
            let j = s.joined.get_mut(&key).unwrap();
            j.backing_seconds = if moving { j.backing_seconds + 1 } else { 0 };
            if j.backing_seconds >= age() {
                let rammer = j.rammer;
                s.joined.remove(&key);
                private(
                    w,
                    rammer,
                    "3.3.separated",
                    json!({"actual_backing_seconds":age()}),
                );
            }
        }
    }
    put(w, s)
}
pub fn after_damage(w: &mut World) {
    let Some(mut s) = take(w) else { return };
    s.joined.retain(|_, p| {
        [p.a, p.b]
            .iter()
            .all(|id| w.entity(*id).is_some_and(|e| e.alive))
    });
    s.groundings
        .retain(|id, _| w.entity(*id).is_some_and(|e| e.alive));
    s.rams
        .retain(|id, _| w.entity(*id).is_some_and(|e| e.alive));
    put(w, s)
}
pub(super) fn own_view(w: &World, id: EntityId) -> Option<Value> {
    let s = state(w)?;
    if !s.classes.contains_key(&id) {
        return None;
    }
    Some(
        json!({"grounding":s.groundings.get(&id).map(|g|json!({"grounded":true,"since":g.at,"refloat_at":g.refloat_at,"remaining_s":(g.refloat_at-w.tick).max(0)})),
      "bow_maximum_fraction":s.bow_damage.contains(&id).then(||d("bow_maximum_fraction")),
      "ram":s.rams.get(&id).map(|r|json!({"contact":r.contact,"plotted_speed_kts":r.speed_kts,"plotted_at":r.plotted_at})),
      "joined_contact":s.joined.values().find(|j|j.a==id||j.b==id).map(|j|json!({"backing_work_seconds":(j.rammer==id).then_some(j.backing_seconds),"motion_pending":j.speed.is_none()}))}),
    )
}

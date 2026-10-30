//! Source5.6 first-class physical lasers. No Radar/IR/Visual impersonation.
use crate::types::PlatformKind;
use crate::{EntityId, EventKind, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Device {
    Rangefinder,
    Designator,
    Lws,
    Lst,
    Lrmts,
}
impl Device {
    fn id(self) -> &'static str {
        match self {
            Self::Rangefinder => "rangefinder",
            Self::Designator => "designator",
            Self::Lws => "lws",
            Self::Lst => "lst",
            Self::Lrmts => "lrmts",
        }
    }
    fn ranges(self) -> bool {
        matches!(self, Self::Rangefinder | Self::Lrmts)
    }
    fn tracks(self) -> bool {
        matches!(self, Self::Lst | Self::Lrmts)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Rangefinding,
    Designation,
    Other,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    pub source: String,
    pub device: Device,
    pub installed: bool,
    pub bearing_policy: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub range_nm: Option<f64>,
    #[serde(default)]
    pub range_source: Option<String>,
    #[serde(default)]
    pub spot_range_nm: Option<f64>,
    #[serde(default)]
    pub spot_range_source: Option<String>,
    #[serde(default)]
    pub spot_precipitation_multiplier: Option<Vec<f64>>,
    #[serde(default)]
    pub terrain_los_source: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Fits {
    One(Fit),
    Many(Vec<Fit>),
}
impl Fits {
    fn rows(&self) -> Vec<&Fit> {
        match self {
            Self::One(f) => vec![f],
            Self::Many(xs) => xs.iter().collect(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub source: String,
    pub enabled: bool,
    pub purpose: Purpose,
    pub known_aimpoint: Vec2,
    pub aimpoint_source: String,
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub aim_altitude_m: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub fits: BTreeMap<String, Fits>,
    #[serde(default)]
    pub initial_jobs: BTreeMap<String, Job>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Component {
    pub unit: EntityId,
    pub class: String,
    pub side: Side,
    pub id: String,
    pub fit: Fit,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundJob {
    pub component: String,
    pub job: Job,
    pub generation: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Beam {
    pub identity: String,
    pub emitter: EntityId,
    pub side: Side,
    pub component: String,
    pub purpose: Purpose,
    pub at: i64,
    pub origin: Vec2,
    pub origin_altitude_m: f64,
    pub point: Vec2,
    pub aim_altitude_m: f64,
    pub target: Option<EntityId>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaserReport {
    pub observer: EntityId,
    pub side: Side,
    pub device: String,
    pub role: Device,
    pub at: i64,
    pub observer_position: Vec2,
    pub bearing_deg: f64,
    pub range_nm: Option<f64>,
    pub measured_position: Option<Vec2>,
    target: EntityId,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Warning {
    pub alias: String,
    pub observer: EntityId,
    pub side: Side,
    pub device: String,
    pub at: i64,
    pub bearing_deg: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spot {
    pub alias: String,
    pub report: LaserReport,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub components: BTreeMap<String, Component>,
    pub jobs: BTreeMap<EntityId, BoundJob>,
    pub beams: BTreeMap<String, Beam>,
    pub warnings: BTreeMap<String, Warning>,
    pub spots: BTreeMap<String, Spot>,
    pub reports: BTreeMap<String, LaserReport>,
    pub next_alias: BTreeMap<EntityId, u64>,
    pub markers: BTreeMap<String, String>,
    pub last_step: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Emission { enabled: bool },
}
fn runtime(w: &World) -> Option<&State> {
    Some(&w.rules.detection.as_ref()?.laser)
}
fn binding(c: &Component) -> String {
    format!("laser:{}", c.id)
}
fn key(unit: EntityId, id: &str) -> String {
    format!("{}:{id}", unit.0)
}
fn can_emit(d: Device, p: Purpose) -> bool {
    match p {
        Purpose::Designation => d == Device::Designator,
        Purpose::Rangefinding => d.ranges(),
        Purpose::Other => d.ranges() || d == Device::Designator,
    }
}
fn body(w: &World, c: &Component) -> bool {
    w.entity(c.unit)
        .is_some_and(|e| e.class == c.class && e.side == c.side && c.fit.installed)
}
fn healthy(w: &World, c: &Component) -> bool {
    body(w, c)
        && w.entity(c.unit)
            .is_some_and(|e| e.alive && !crate::sensors::sensor_damaged(e, &binding(c)))
        && crate::rules::unmanned2026::detection_allowed(w, c.unit)
        && crate::rules::expanded_damage2026::sensor_operational(w, c.unit, &binding(c))
            .unwrap_or(false)
}
pub fn physical_sensor_components(w: &World, unit: EntityId) -> Vec<(String, String)> {
    runtime(w)
        .map(|s| {
            s.components
                .values()
                .filter(|c| c.unit == unit && body(w, c))
                .map(|c| {
                    let b = binding(c);
                    (format!("native.sensor:{b}"), b)
                })
                .collect()
        })
        .unwrap_or_default()
}
fn valid_source(s: &Option<String>) -> bool {
    s.as_ref().is_some_and(|s| !s.trim().is_empty())
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    let Some(config) = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| s.config.laser_sensors.clone())
    else {
        return Ok(());
    };
    if config.source.trim().is_empty() {
        return Err("5.6 physical laser configuration needs source".into());
    }
    let mut state = State::default();
    for (name, fs) in &config.fits {
        let units: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
        if units.len() != 1 {
            return Err(
                "5.6 installed laser component requires one exact physical unit name".into(),
            );
        }
        let e = units[0];
        for f in fs.rows() {
            if !f.installed || f.source.trim().is_empty() || f.bearing_policy != "exact_scene_ray" {
                return Err(
                    "5.6 explicit installed laser source and bearing policy required".into(),
                );
            }
            let platform = match f.device {
                Device::Designator | Device::Lst | Device::Lrmts => {
                    e.kind == PlatformKind::Aircraft
                }
                Device::Lws | Device::Rangefinder => matches!(
                    e.kind,
                    PlatformKind::Ship | PlatformKind::Sub | PlatformKind::Aircraft
                ),
            };
            if !e.alive || !platform {
                return Err("5.6 source laser device/platform applicability unresolved".into());
            }
            if f.device == Device::Designator
                && (f.range_nm.is_none() || !valid_source(&f.range_source))
                || f.range_nm.is_some_and(|x| !x.is_finite() || x <= 0.)
                || f.range_nm.is_some() && !valid_source(&f.range_source)
            {
                return Err(
                    "5.6 explicit source/author designator range or Annex range exception required"
                        .into(),
                );
            }
            if f.device.tracks()
                && (f.spot_range_nm.is_none_or(|x| !x.is_finite() || x <= 0.)
                    || !valid_source(&f.spot_range_source))
            {
                return Err("5.6.4 source supplies no universal LST range; exact fitted spot-view source required".into());
            }
            if f.spot_precipitation_multiplier
                .as_ref()
                .is_some_and(|v| v.len() != 6 || v.iter().any(|x| !x.is_finite() || *x < 0.))
            {
                return Err(
                    "5.6 explicit six-condition reflected-spot moisture fit invalid".into(),
                );
            }
            let id = f.id.clone().unwrap_or_else(|| f.device.id().into());
            if id.trim().is_empty() || id.contains(':') {
                return Err("5.6 canonical physical laser device id invalid".into());
            }
            let c = Component {
                unit: e.id,
                class: e.class.clone(),
                side: e.side,
                id: id.clone(),
                fit: f.clone(),
            };
            if state.components.insert(key(e.id, &id), c).is_some() {
                return Err("5.6 duplicate physical laser component id".into());
            }
        }
    }
    for (name, j) in &config.initial_jobs {
        if j.source.trim().is_empty()
            || j.aimpoint_source.trim().is_empty()
            || !j.known_aimpoint.x.is_finite()
            || !j.known_aimpoint.y.is_finite()
            || j.aim_altitude_m.is_some_and(|h| !h.is_finite() || h < 0.)
        {
            return Err("5.6 actual laser job requires allocated fixed aimpoint/source".into());
        }
        let units: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
        if units.len() != 1 {
            return Err("5.6 laser job body not unique".into());
        }
        let e = units[0];
        let cs: Vec<_> = state
            .components
            .iter()
            .filter(|(_, c)| {
                c.unit == e.id
                    && can_emit(c.fit.device, j.purpose)
                    && j.device.as_ref().is_none_or(|d| d == &c.id)
            })
            .map(|(k, _)| k.clone())
            .collect();
        if cs.len() != 1 {
            return Err(
                "5.6 laser job requires one purpose-qualified actual installed device".into(),
            );
        }
        state.jobs.insert(
            e.id,
            BoundJob {
                component: cs[0].clone(),
                job: j.clone(),
                generation: 1,
            },
        );
    }
    w.rules.detection.as_mut().unwrap().laser = state;
    Ok(())
}
pub fn validate_action(w: &World, unit: EntityId, action: &Action) -> Result<(), String> {
    let Action::Emission { enabled } = action;
    let s = runtime(w).ok_or("5.6 physical laser state absent")?;
    let job = s
        .jobs
        .get(&unit)
        .ok_or("5.6 no allocated own fixed-point laser job")?;
    let c = s
        .components
        .get(&job.component)
        .ok_or("5.6 laser component absent")?;
    if !body(w, c) || !w.entity(unit).is_some_and(|e| e.alive) {
        return Err("5.6 actual own laser body unavailable".into());
    }
    if *enabled && !healthy(w, c) {
        return Err("5.6 physical laser component unavailable".into());
    }
    Ok(())
}
pub fn control(w: &mut World, unit: EntityId, action: &Action) -> Result<(), String> {
    validate_action(w, unit, action)?;
    let Action::Emission { enabled } = action;
    let j = w
        .rules
        .detection
        .as_mut()
        .unwrap()
        .laser
        .jobs
        .get_mut(&unit)
        .unwrap();
    if *enabled && !j.job.enabled {
        j.generation = j.generation.saturating_add(1);
    }
    j.job.enabled = *enabled;
    Ok(())
}
fn event(w: &mut World, actor: EntityId, rule: &str, side: Option<Side>, detail: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "detection2026".into(),
            rule: rule.into(),
            side,
            unit: Some(actor),
            contact: None,
            detail: detail.to_string(),
        },
        side.map(Visibility::Side).unwrap_or(Visibility::Referee),
    );
}
fn private_once(w: &mut World, s: &mut State, actor: EntityId, tag: &str, detail: Value) {
    let k = format!("{}:{tag}", actor.0);
    let marker = detail.to_string();
    if s.markers.get(&k) != Some(&marker) {
        s.markers.insert(k, marker);
        event(w, actor, "5.6.laser_physical", None, detail);
    }
}
fn alias(s: &mut State, unit: EntityId, prefix: &str) -> String {
    let n = s.next_alias.entry(unit).or_default();
    *n += 1;
    format!("{prefix}-{n}")
}
fn environment(w: &World) -> Result<super::Environment, String> {
    w.rules
        .detection
        .as_ref()
        .and_then(|s| s.config.environment.clone())
        .filter(|e| !e.source.trim().is_empty())
        .ok_or("5.6 source laser moisture/cloud environment absent".into())
}
fn clear(
    w: &mut World,
    c: &Component,
    point: Vec2,
    alt: f64,
    target: EntityId,
    env: &super::Environment,
) -> Result<bool, String> {
    let e = w.entity(c.unit).ok_or("5.6 actual laser observer absent")?;
    if e.kind == PlatformKind::Sub && e.depth_m > 0. {
        return Err(
            "5.6 submarine optical aperture/mast geometry not supplied in this physical laser fit"
                .into(),
        );
    }
    if w.context
        .terrain
        .as_ref()
        .is_some_and(|chart| chart.blocked(e.pos, point))
    {
        if e.altitude_m <= 0. && alt <= 0. {
            return Ok(false);
        }
        if !valid_source(&c.fit.terrain_los_source) {
            return Err("5.6 airborne terrain elevation/clearance source absent; coastline is not a height model".into());
        }
    }
    let from = e.altitude_m;
    let seed = w.seed;
    let now = w.tick;
    let d = w
        .rules
        .detection
        .as_mut()
        .ok_or("5.6 detection state absent")?;
    Ok(super::cloud_effect(d, seed, now, c.unit, target, from, alt, env.cloud.as_ref())?.0)
}
fn report(c: &Component, w: &World, target: EntityId, point: Vec2, ranged: bool) -> LaserReport {
    let o = w.entity(c.unit).unwrap();
    let range = o.pos.range_to(point);
    let bearing = o.pos.bearing_to(point);
    LaserReport {
        observer: c.unit,
        side: c.side,
        device: c.id.clone(),
        role: c.fit.device,
        at: w.tick,
        observer_position: o.pos,
        bearing_deg: bearing,
        range_nm: ranged.then_some(range),
        measured_position: ranged.then(|| o.pos.advanced(bearing, range)),
        target,
    }
}
fn publish_contact(w: &mut World, r: &LaserReport) {
    use crate::{world::Contact, ContactId};
    let estimate = r
        .measured_position
        .unwrap_or_else(|| r.observer_position.advanced(r.bearing_deg, 1.));
    let old = w.sides[r.side.index()]
        .contacts
        .values()
        .find(|c| c.about == r.target && !c.stale)
        .map(|c| c.id);
    if let Some(cid) = old {
        let c = w.sides[r.side.index()].contacts.get_mut(&cid).unwrap();
        if r.range_nm.is_some() || c.bearing_only {
            c.estimate = estimate;
            c.bearing_only = r.range_nm.is_none();
            c.last_update_tick = r.at;
            c.quality = if r.range_nm.is_some() { 1. } else { 0.25 };
        }
        let sensor = format!("laser:{}", r.device);
        if !c.sensors.contains(&sensor) {
            c.sensors.push(sensor);
        }
        return;
    }
    let private_kind = w
        .entity(r.target)
        .map(|t| t.kind)
        .unwrap_or(PlatformKind::Ship);
    let s = &mut w.sides[r.side.index()];
    s.next_contact += 1;
    let cid = ContactId(s.next_contact);
    s.contacts.insert(
        cid,
        Contact {
            id: cid,
            about: r.target,
            category: private_kind,
            class_name: None,
            estimate,
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: r.range_nm.is_none(),
            quality: if r.range_nm.is_some() { 1. } else { 0.25 },
            last_update_tick: r.at,
            first_tick: r.at,
            stale: false,
            sensors: vec![format!("laser:{}", r.device)],
            offset: Vec2::zero(),
            announced: false,
        },
    );
    w.emit(
        EventKind::Detection {
            side: r.side,
            contact: cid,
            category: "unknown".into(),
            pos_hint: estimate,
            sensor: format!("laser:{}", r.device),
        },
        Visibility::Side(r.side),
    );
}
fn warn(w: &mut World, s: &mut State, beam: &Beam) {
    let Some(target) = beam.target else { return };
    let cs: Vec<_> = s
        .components
        .iter()
        .filter(|(_, c)| c.unit == target && c.fit.device == Device::Lws && healthy(w, c))
        .map(|(k, c)| (k.clone(), c.clone()))
        .collect();
    for (ck, c) in cs {
        let o = w.entity(target).unwrap();
        let bearing = o.pos.bearing_to(beam.origin);
        let k = format!("{ck}:ray:{bearing:.8}");
        let previous = s.warnings.get(&k).cloned();
        let local_alias = previous
            .as_ref()
            .map(|x| x.alias.clone())
            .unwrap_or_else(|| alias(s, target, "laser-warning"));
        s.warnings.insert(
            k,
            Warning {
                alias: local_alias.clone(),
                observer: target,
                side: c.side,
                device: c.id.clone(),
                at: w.tick,
                bearing_deg: bearing,
            },
        );
        if previous.is_none_or(|x| crate::geom::angle_diff(x.bearing_deg, bearing).abs() > 1e-6) {
            event(
                w,
                target,
                "5.6.3.laser_warning",
                Some(c.side),
                json!({"at":w.tick,"bearing_deg":bearing,"phase":"detection","observation":"laser illumination"}),
            );
        }
    }
}
fn beam(
    w: &mut World,
    s: &mut State,
    c: &Component,
    job: &BoundJob,
    env: &super::Environment,
) -> Result<Option<Beam>, String> {
    if !job.job.enabled || !healthy(w, c) {
        return Ok(None);
    }
    let o = w.entity(c.unit).unwrap();
    let point = job.job.known_aimpoint;
    let range = o.pos.range_to(point);
    let effective = super::laser_range(
        o.kind == PlatformKind::Ship,
        c.fit.range_nm,
        env.precipitation,
    )?;
    if range > effective {
        return Err("5.6 actual emitted beam exceeds source moisture-adjusted fitted range".into());
    }
    let alt = job.job.aim_altitude_m.unwrap_or(0.);
    let epsilon = super::tables()["laser_execution"]["point_epsilon_nm"]
        .as_f64()
        .unwrap();
    let zepsilon = super::tables()["laser_execution"]["altitude_epsilon_m"]
        .as_f64()
        .unwrap();
    let bodies: Vec<_> = w
        .entities
        .values()
        .filter(|t| {
            t.alive
                && t.id != c.unit
                && !crate::rules::ew2026::is_object(w, t.id)
                && !t.kind.is_projectile()
                && !(t.kind == PlatformKind::Sub && t.depth_m > 0.)
                && t.pos.range_to(point) <= epsilon
                && (t.altitude_m - alt).abs() <= zepsilon
        })
        .map(|t| t.id)
        .collect();
    if bodies.len() > 1 {
        return Err(
            "5.6 multiple collocated physical bodies; beam-hit footprint/occlusion unresolved"
                .into(),
        );
    }
    let target = bodies.first().copied();
    if !clear(w, c, point, alt, target.unwrap_or(EntityId(0)), env)? {
        return Ok(None);
    }
    let identity = format!("5.6.beam:{}:{}:{}", c.unit.0, c.id, job.generation);
    let b = Beam {
        identity: identity.clone(),
        emitter: c.unit,
        side: c.side,
        component: job.component.clone(),
        purpose: job.job.purpose,
        at: w.tick,
        origin: w.entity(c.unit).unwrap().pos,
        origin_altitude_m: w.entity(c.unit).unwrap().altitude_m,
        point,
        aim_altitude_m: alt,
        target,
    };
    private_once(
        w,
        s,
        c.unit,
        "emission",
        json!({"identity":identity,"purpose":b.purpose,"known_fixed_point":point,"actual_hit_body":target,"qualification":"source hardware/range/moisture/LOS and exact fixed-point scene geometry"}),
    );
    Ok(Some(b))
}
/// Sole normal Detection-phase producer/consumer; never reruns generic scans.
pub fn step(w: &mut World) {
    if !super::active(w) {
        return;
    }
    let Some(mut s) = w
        .rules
        .detection
        .as_mut()
        .map(|d| std::mem::take(&mut d.laser))
    else {
        return;
    };
    if s.components.is_empty() || s.last_step == Some(w.tick) {
        w.rules.detection.as_mut().unwrap().laser = s;
        return;
    }
    s.last_step = Some(w.tick);
    s.beams.clear();
    let env = match environment(w) {
        Ok(e) => e,
        Err(reason) => {
            let ids: Vec<_> = s.components.values().map(|c| c.unit).collect();
            for id in ids {
                private_once(w, &mut s, id, "blocked", json!({"reason":reason}));
            }
            w.rules.detection.as_mut().unwrap().laser = s;
            return;
        }
    };
    let jobs: Vec<_> = s.jobs.values().cloned().collect();
    for j in jobs {
        let Some(c) = s.components.get(&j.component).cloned() else {
            continue;
        };
        match beam(w, &mut s, &c, &j, &env) {
            Ok(Some(b)) => {
                s.beams.insert(b.identity.clone(), b);
            }
            Ok(None) => {}
            Err(reason) => private_once(w, &mut s, c.unit, "blocked", json!({"reason":reason})),
        }
    }
    let emitted: Vec<_> = s.beams.values().cloned().collect();
    for b in &emitted {
        warn(w, &mut s, b);
        if b.purpose == Purpose::Rangefinding {
            if let Some(target) = b.target {
                let c = s.components[&b.component].clone();
                let r = report(&c, w, target, b.point, true);
                s.reports
                    .insert(format!("{}:{}:{}", c.unit.0, c.id, target.0), r);
            }
        }
    }
    let trackers: Vec<_> = s
        .components
        .iter()
        .filter(|(_, c)| c.fit.device.tracks() && healthy(w, c))
        .map(|(k, c)| (k.clone(), c.clone()))
        .collect();
    for (ck, c) in trackers {
        for b in emitted.iter().filter(|b| b.purpose == Purpose::Designation) {
            let Some(target) = b.target else { continue };
            if c.unit == target {
                continue;
            }
            let o = w.entity(c.unit).unwrap();
            let distance = o.pos.range_to(b.point);
            let mut spot_limit = c.fit.spot_range_nm.unwrap();
            if env.precipitation > 0 {
                let Some(m) = &c.fit.spot_precipitation_multiplier else {
                    private_once(
                        w,
                        &mut s,
                        c.unit,
                        "spot-blocked",
                        json!({"reason":"5.6.4 reflected-spot moisture fit is not printed; explicit fitted factors required"}),
                    );
                    continue;
                };
                spot_limit *= m[env.precipitation];
            }
            if distance > spot_limit {
                continue;
            }
            match clear(w, &c, b.point, b.aim_altitude_m, target, &env) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(reason) => {
                    private_once(w, &mut s, c.unit, "spot-blocked", json!({"reason":reason}));
                    continue;
                }
            }
            let ranged = c.fit.device == Device::Lrmts
                && distance
                    <= super::laser_range(false, c.fit.range_nm, env.precipitation).unwrap_or(0.);
            let r = report(&c, w, target, b.point, ranged);
            let k = format!("{ck}:spot:{}", target.0);
            let old = s.spots.get(&k).cloned();
            let local_alias = old
                .as_ref()
                .map(|x| x.alias.clone())
                .unwrap_or_else(|| alias(&mut s, c.unit, "laser-spot"));
            s.spots.insert(
                k,
                Spot {
                    alias: local_alias.clone(),
                    report: r.clone(),
                },
            );
            s.reports
                .insert(format!("{}:{}:{}", c.unit.0, c.id, target.0), r.clone());
            publish_contact(w, &r);
            if old.is_none() {
                event(
                    w,
                    c.unit,
                    "5.6.4.laser_spot_acquired",
                    Some(c.side),
                    json!({"spot_alias":local_alias,"at":w.tick,"bearing_deg":r.bearing_deg,"phase":"detection"}),
                );
            }
            if ranged {
                // LRMTS performs a real own rangefinder emission, not a copy of
                // the designator's private range. Its pulse may itself warn LWS.
                let rb = Beam {
                    identity: format!("5.6.range:{}:{}", c.unit.0, local_alias),
                    emitter: c.unit,
                    side: c.side,
                    component: ck.clone(),
                    purpose: Purpose::Rangefinding,
                    at: w.tick,
                    origin: w.entity(c.unit).unwrap().pos,
                    origin_altitude_m: w.entity(c.unit).unwrap().altitude_m,
                    point: b.point,
                    aim_altitude_m: b.aim_altitude_m,
                    target: Some(target),
                };
                warn(w, &mut s, &rb);
                s.beams.insert(rb.identity.clone(), rb);
                if old.as_ref().is_none_or(|x| x.report.range_nm.is_none()) {
                    event(
                        w,
                        c.unit,
                        "5.6.5.lrmts_spot_ranged",
                        Some(c.side),
                        json!({"spot_alias":local_alias,"at":w.tick,"bearing_deg":r.bearing_deg,"range_nm":r.range_nm,"measured_position":r.measured_position,"phase":"detection"}),
                    );
                }
            }
        }
    }
    w.rules.detection.as_mut().unwrap().laser = s;
}
/// Direct ranging evidence for a separate native optical FCS consumer. This
/// does not invent a contact-track number, quality, generation or remote cue.
pub fn direct_range_report(w: &World, actor: EntityId, target: EntityId) -> Option<LaserReport> {
    let s = runtime(w)?;
    s.reports
        .values()
        .filter(|r| {
            r.observer == actor
                && r.target == target
                && r.at == w.tick
                && r.range_nm.is_some()
                && r.measured_position.is_some()
                && s.components
                    .get(&key(actor, &r.device))
                    .is_some_and(|c| healthy(w, c))
        })
        .max_by_key(|r| r.at)
        .cloned()
}
pub fn own_view(w: &World, side: Side, unit: EntityId) -> Value {
    let Some(s) = runtime(w) else {
        return Value::Null;
    };
    let components:Vec<_>=s.components.values().filter(|c|c.unit==unit&&c.side==side&&body(w,c)).map(|c|json!({"device":c.id,"role":c.fit.device,"installed":true,"available":healthy(w,c),"source":c.fit.source})).collect();
    let warnings: Vec<_> = s
        .warnings
        .values()
        .filter(|x| x.observer == unit && x.side == side)
        .map(|x| json!({"at":x.at,"bearing_deg":x.bearing_deg,"device":x.device}))
        .collect();
    let spots:Vec<_>=s.spots.values().filter(|x|x.report.observer==unit&&x.report.side==side).map(|x|json!({"spot_alias":x.alias,"at":x.report.at,"device":x.report.device,"bearing_deg":x.report.bearing_deg,"range_nm":x.report.range_nm,"measured_position":x.report.measured_position})).collect();
    let job=s.jobs.get(&unit).filter(|j|s.components.get(&j.component).is_some_and(|c|c.side==side&&body(w,c))).map(|j|json!({"enabled":j.job.enabled,"purpose":j.job.purpose,"known_fixed_aimpoint":j.job.known_aimpoint,"source":j.job.aimpoint_source}));
    json!({"components":components,"emission_job":job,"warnings":warnings,"spots":spots})
}

/// Immutable native SALH allocation. It names an actual friendly physical
/// designator, never an arbitrary spot or a remembered fused contact.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundDesignation {
    pub designator: EntityId,
    pub device: String,
    pub target: EntityId,
    pub side: Side,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DesignationSample {
    pub at: i64,
    pub active: bool,
    pub reason: Option<String>,
    pub physical_point: Option<Vec2>,
    pub provenance: Value,
}
fn designation_parts(w: &World, b: &BoundDesignation) -> Result<(Component, BoundJob), String> {
    let s = runtime(w).ok_or("9.6.8 native laser state absent")?;
    let c = s
        .components
        .get(&key(b.designator, &b.device))
        .cloned()
        .ok_or("9.6.8 bound actual designator component absent")?;
    let j = s
        .jobs
        .get(&b.designator)
        .cloned()
        .ok_or("9.6.8 bound allocated laser job absent")?;
    if b.source.trim().is_empty()
        || c.fit.device != Device::Designator
        || c.side != b.side
        || j.component != key(b.designator, &b.device)
        || j.job.purpose != Purpose::Designation
    {
        return Err("9.6.8 SALH requires the allocated friendly source Designator and its real designation job".into());
    }
    Ok((c, j))
}
fn designation_stop(w: &World, c: &Component, j: &BoundJob) -> Result<Option<String>, String> {
    if !body(w, c) || !w.entity(c.unit).is_some_and(|e| e.alive) {
        return Ok(Some(
            "allocated physical designator unavailable or changed binding".into(),
        ));
    }
    if !j.job.enabled {
        return Ok(Some("actual designator emission switched off".into()));
    }
    if !crate::rules::expanded_damage2026::sensor_operational(w, c.unit, &binding(c))? {
        return Ok(Some(
            "actual designator damaged, unpowered or CIC unavailable".into(),
        ));
    }
    if !healthy(w, c) {
        return Ok(Some("actual designator cannot operate".into()));
    }
    let e = w.entity(c.unit).unwrap();
    if e.kind == PlatformKind::Aircraft
        && crate::geom::angle_diff(e.course_deg, e.pos.bearing_to(j.job.known_aimpoint)).abs()
            > crate::rules::pgm2026::tables()["salh_execution"]["aircraft_designator_half_arc_deg"]
                .as_f64()
                .unwrap()
    {
        return Ok(Some(
            "target in the actual aircraft designator aft90degree blind zone".into(),
        ));
    }
    Ok(None)
}
/// Read-only admission uses an actually produced normal beam and checks the
/// current job/body/course; it cannot manufacture illumination at declaration.
pub fn validate_designation(w: &World, b: &BoundDesignation) -> Result<(), String> {
    let (c, j) = designation_parts(w, b)?;
    if let Some(reason) = designation_stop(w, &c, &j)? {
        return Err(reason);
    }
    if !runtime(w).is_some_and(|s| {
        s.beams.values().any(|x| {
            x.emitter == b.designator
                && x.component == j.component
                && x.purpose == Purpose::Designation
                && x.target == Some(b.target)
                && x.at <= w.tick
                && w.tick - x.at <= 1
        })
    }) {
        return Err(
            "9.6.8 actual normal-phase designation beam has not reached the allocated target"
                .into(),
        );
    }
    Ok(())
}
/// Reuses one claimed physical ray, not a second global scan. It samples
/// current control/movement/damage before impact and emits no public warnings
/// or spot reports outside the one normal Detection phase.
pub fn sample_designation(
    w: &mut World,
    b: &BoundDesignation,
) -> Result<DesignationSample, String> {
    let (c, j) = designation_parts(w, b)?;
    if let Some(reason) = designation_stop(w, &c, &j)? {
        return Ok(DesignationSample {
            at: w.tick,
            active: false,
            reason: Some(reason),
            physical_point: None,
            provenance: json!({"rule":"9.6.8","current_tick_physical_qualification":true}),
        });
    }
    let env = environment(w)?;
    let mut s = std::mem::take(&mut w.rules.detection.as_mut().unwrap().laser);
    let result = beam(w, &mut s, &c, &j, &env);
    w.rules.detection.as_mut().unwrap().laser = s;
    match result {
        Ok(Some(ray)) => {
            let active = ray.target == Some(b.target);
            Ok(DesignationSample {
                at: w.tick,
                active,
                reason: (!active)
                    .then(|| "actual allocated beam no longer reaches the bound target".into()),
                physical_point: active.then_some(ray.point),
                provenance: json!({"rule":"9.6.8","beam":ray.identity,"device":b.device,"source":b.source,"normal_public_scan_repeated":false}),
            })
        }
        Ok(None) => Ok(DesignationSample {
            at: w.tick,
            active: false,
            reason: Some("actual claimed laser LOS is obstructed".into()),
            physical_point: None,
            provenance: json!({"rule":"5.6.2/9.6.8"}),
        }),
        Err(reason) if reason.contains("exceeds source moisture-adjusted fitted range") => {
            Ok(DesignationSample {
                at: w.tick,
                active: false,
                reason: Some(reason),
                physical_point: None,
                provenance: json!({"rule":"5.6.2/9.6.8"}),
            })
        }
        Err(reason) => Err(reason),
    }
}

/// Source-control preflight is restricted to the allocated friendly hardware,
/// own fixed-point job and current own course/control/health. Actual reflected
/// target/LOS evidence is sampled after physical weapon release.
pub fn validate_designator_control(w: &World, b: &BoundDesignation) -> Result<(), String> {
    let (c, j) = designation_parts(w, b)?;
    if let Some(reason) = designation_stop(w, &c, &j)? {
        return Err(reason);
    }
    Ok(())
}

//! Source5.7.3 physical nonacoustic SOKS. Never masquerades as sonar/MAD.
use crate::types::PlatformKind;
use crate::{EntityId, EventKind, Rng, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub fits: BTreeMap<String, Fit>,
    pub geometry: Geometry,
    #[serde(default)]
    pub creator_power_sources: BTreeMap<String, PowerSource>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerSource {
    pub source: String,
    pub nuclear: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub source: String,
    pub crossing_policy: String,
    pub distinct_fix_separation_nm: f64,
    pub history_policy: String,
    pub trial_policy: String,
    pub course_policy: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mount {
    Hull,
    Towed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    pub source: String,
    pub system: String,
    pub installed: bool,
    pub enabled: bool,
    pub mount: Mount,
    #[serde(default)]
    pub towed_depth_m: Option<f64>,
    #[serde(default)]
    pub towed_lag_nm: Option<f64>,
    #[serde(default)]
    pub mount_source: Option<String>,
    #[serde(default)]
    pub alerted_source: Option<String>,
    #[serde(default)]
    pub radiological_source: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Component {
    pub fit: Fit,
    pub unit: EntityId,
    pub class: String,
    pub side: Side,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    pub at: i64,
    pub position: Vec2,
    pub depth_m: f64,
    pub speed_kts: f64,
    pub course_deg: f64,
    pub side: Side,
    pub class: String,
    pub kind: PlatformKind,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Segment {
    pub id: u64,
    pub creator: EntityId,
    pub side: Side,
    pub from: Vec2,
    pub to: Vec2,
    pub from_at: i64,
    pub to_at: i64,
    pub depth: usize,
    pub speed_kts: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attempt {
    pub point: Vec2,
    pub at: i64,
    pub identity: String,
    pub die: u8,
    pub probability: f64,
    pub detected: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cut {
    pub point: Vec2,
    pub at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnWake {
    pub wake_alias: String,
    pub observer: EntityId,
    pub side: Side,
    pub sensor: String,
    pub first_at: i64,
    pub last_at: i64,
    pub points: Vec<Cut>,
    pub course_deg: Option<f64>,
    pub nuclear: Option<bool>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Option<Configuration>,
    pub components: BTreeMap<EntityId, Component>,
    pub samples: BTreeMap<EntityId, Sample>,
    pub segments: Vec<Segment>,
    pub next_segment: u64,
    pub attempts: BTreeMap<String, Vec<Attempt>>,
    pub observations: BTreeMap<String, OwnWake>,
    pub next_alias: BTreeMap<EntityId, u64>,
    pub nuclear_creators: BTreeMap<EntityId, bool>,
    pub last_step: Option<i64>,
}
fn runtime(w: &World) -> Option<&State> {
    Some(&w.rules.detection.as_ref()?.soks)
}
fn binding(system: &str) -> String {
    format!("soks:{system}")
}
fn row(system: &str) -> &'static Value {
    &super::tables()["wake"]["execution"]["systems"][system]
}
fn max_age() -> i64 {
    super::tables()["wake"]["execution"]["maximum_history_seconds"]
        .as_i64()
        .unwrap()
}
fn sample(w: &World, e: &crate::world::Entity) -> Sample {
    Sample {
        at: w.tick,
        position: e.pos,
        depth_m: e.depth_m,
        speed_kts: e.speed_kts,
        course_deg: e.course_deg,
        side: e.side,
        class: e.class.clone(),
        kind: e.kind,
    }
}
fn body(w: &World, c: &Component) -> bool {
    w.entity(c.unit).is_some_and(|e| {
        e.class == c.class
            && e.side == c.side
            && c.fit.installed
            && match c.fit.mount {
                Mount::Hull => e.kind == PlatformKind::Sub,
                Mount::Towed => e.kind == PlatformKind::Ship,
            }
    })
}
fn available(w: &World, c: &Component) -> bool {
    body(w, c)
        && c.fit.enabled
        && w.entity(c.unit)
            .is_some_and(|e| e.alive && !crate::sensors::sensor_damaged(e, &binding(&c.fit.system)))
        && crate::rules::unmanned2026::detection_allowed(w, c.unit)
        && crate::rules::expanded_damage2026::sensor_operational(w, c.unit, &binding(&c.fit.system))
            .unwrap_or(false)
}
pub fn physical_sensor_components(w: &World, unit: EntityId) -> Vec<(String, String)> {
    runtime(w)
        .and_then(|s| s.components.get(&unit))
        .filter(|c| body(w, c))
        .map(|c| {
            let b = binding(&c.fit.system);
            vec![(format!("native.sensor:{b}"), b)]
        })
        .unwrap_or_default()
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    let Some(d) = w.rules.detection.as_ref() else {
        return Ok(());
    };
    let Some(config) = d.config.soks.clone() else {
        return Ok(());
    };
    let g = &config.geometry;
    if config.source.trim().is_empty()
        || g.source.trim().is_empty()
        || g.crossing_policy != "exact_centerline_intersection"
        || g.history_policy != "actual_runtime_only"
        || g.trial_policy != "one_per_discrete_crossing"
        || g.course_policy != "historical_local_tangent_after_three"
        || !g.distinct_fix_separation_nm.is_finite()
        || g.distinct_fix_separation_nm <= 0.
    {
        return Err("5.7.3 explicit sourced SOKS geometry/identity/course policy required".into());
    }
    let mut state = State {
        config: Some(config.clone()),
        ..Default::default()
    };
    for (name, f) in &config.fits {
        let model = row(&f.system);
        if f.source.trim().is_empty() || !f.installed || model.is_null() {
            return Err(
                "5.7.3 known source system and explicit actual installed fit required".into(),
            );
        }
        let units: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
        if units.len() != 1 {
            return Err("5.7.3 installed component needs one exact actual unit name".into());
        }
        let e = units[0];
        let mount_ok = match f.mount {
            Mount::Hull => e.kind == PlatformKind::Sub && model["mount"] == "hull",
            Mount::Towed => {
                e.kind == PlatformKind::Ship
                    && model["mount"] == "towed"
                    && f.mount_source
                        .as_ref()
                        .is_some_and(|s| !s.trim().is_empty())
                    && f.towed_depth_m.is_some_and(|d| d.is_finite() && d > 0.)
                    && f.towed_lag_nm.is_some_and(|d| d.is_finite() && d >= 0.)
            }
        };
        if !e.alive || !mount_ok {
            return Err(
                "5.7.3 SOKS actual source platform/mount/deployment geometry mismatch".into(),
            );
        }
        if f.alerted_source
            .as_ref()
            .is_some_and(|s| s.trim().is_empty())
            || f.radiological_source
                .as_ref()
                .is_some_and(|s| s.trim().is_empty())
        {
            return Err("5.7.3 explicit source label required for conditional channels".into());
        }
        state.components.insert(
            e.id,
            Component {
                fit: f.clone(),
                unit: e.id,
                class: e.class.clone(),
                side: e.side,
            },
        );
    }
    for (name, p) in &config.creator_power_sources {
        let units: Vec<_> = w
            .entities
            .values()
            .filter(|e| e.name == *name && e.kind == PlatformKind::Sub)
            .collect();
        if p.source.trim().is_empty() || units.len() != 1 {
            return Err("5.7.3 radiological source needs one exact sourced Sub body".into());
        }
        state.nuclear_creators.insert(units[0].id, p.nuclear);
    }
    for e in w.entities.values().filter(|e| {
        e.alive
            && !crate::rules::ew2026::is_object(w, e.id)
            && (e.kind == PlatformKind::Sub || state.components.contains_key(&e.id))
    }) {
        state.samples.insert(e.id, sample(w, e));
    }
    state.last_step = Some(w.tick);
    w.rules.detection.as_mut().unwrap().soks = state;
    Ok(())
}
pub fn control(w: &mut World, unit: EntityId, enabled: bool) -> Result<(), String> {
    let c = runtime(w)
        .and_then(|s| s.components.get(&unit))
        .ok_or("5.7.3 own installed SOKS component absent")?;
    if !body(w, c) || !w.entity(unit).is_some_and(|e| e.alive) {
        return Err("5.7.3 actual own SOKS body unavailable".into());
    }
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .soks
        .components
        .get_mut(&unit)
        .unwrap()
        .fit
        .enabled = enabled;
    Ok(())
}
fn cross(a: Vec2, b: Vec2) -> f64 {
    a.x * b.y - a.y * b.x
}
fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}
fn intersection(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    let r = sub(b, a);
    let s = sub(d, c);
    let den = cross(r, s);
    if den.abs() < 1e-12 {
        return None;
    }
    let q = sub(c, a);
    let t = cross(q, s) / den;
    let u = cross(q, r) / den;
    if !(-1e-8..=1. + 1e-8).contains(&t) || !(-1e-8..=1. + 1e-8).contains(&u) {
        return None;
    }
    Some(Vec2 {
        x: a.x + t.clamp(0., 1.) * r.x,
        y: a.y + t.clamp(0., 1.) * r.y,
    })
}
fn sensor_path(c: &Component, a: &Sample, b: &Sample) -> Option<(Vec2, Vec2, usize)> {
    match c.fit.mount {
        Mount::Hull => {
            let depth = super::depth_band(a.depth_m).ok()?;
            (a.depth_m > 0. && super::depth_band(b.depth_m).ok() == Some(depth))
                .then_some((a.position, b.position, depth))
        }
        Mount::Towed => Some((
            a.position
                .advanced(a.course_deg + 180., c.fit.towed_lag_nm?),
            b.position
                .advanced(b.course_deg + 180., c.fit.towed_lag_nm?),
            super::depth_band(c.fit.towed_depth_m?).ok()?,
        )),
    }
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
fn trace_pending(w: &mut World, actor: EntityId, reason: &str, detail: Value) {
    event(
        w,
        actor,
        "5.7.3.wake_pending",
        None,
        json!({"reason":reason,"evidence":detail}),
    );
}
/// One normal post-sensor consumer. It records actual completed movement,
/// and never runs generic sonar/MAD or creates a current combat contact.
pub fn step(w: &mut World) {
    if !super::active(w) {
        return;
    }
    let Some(mut state) = w
        .rules
        .detection
        .as_mut()
        .map(|s| std::mem::take(&mut s.soks))
    else {
        return;
    };
    if state.components.is_empty() || state.last_step == Some(w.tick) {
        w.rules.detection.as_mut().unwrap().soks = state;
        return;
    }
    let contiguous = state.last_step == Some(w.tick - 1);
    let previous = std::mem::take(&mut state.samples);
    for e in w.entities.values().filter(|e| {
        !crate::rules::ew2026::is_object(w, e.id)
            && (e.kind == PlatformKind::Sub || state.components.contains_key(&e.id))
    }) {
        state.samples.insert(e.id, sample(w, e));
    }
    state.last_step = Some(w.tick);
    if !contiguous {
        w.rules.detection.as_mut().unwrap().soks = state;
        return;
    }
    for (id, b) in &state.samples {
        let Some(a) = previous.get(id) else { continue };
        if a.kind != PlatformKind::Sub
            || a.class != b.class
            || a.side != b.side
            || a.depth_m <= 0.
            || b.depth_m <= 0.
            || a.position.range_to(b.position) <= 1e-12
        {
            continue;
        }
        let Ok(depth) = super::depth_band(a.depth_m) else {
            continue;
        };
        if super::depth_band(b.depth_m).ok() != Some(depth) {
            continue;
        }
        state.next_segment += 1;
        state.segments.push(Segment {
            id: state.next_segment,
            creator: *id,
            side: a.side,
            from: a.position,
            to: b.position,
            from_at: a.at,
            to_at: b.at,
            depth,
            speed_kts: a.position.range_to(b.position) * 3600.,
        });
    }
    state.segments.retain(|s| w.tick - s.to_at <= max_age());
    let Some(config) = state.config.clone() else {
        w.rules.detection.as_mut().unwrap().soks = state;
        return;
    };
    let sea = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| s.config.environment.as_ref())
        .map(|e| e.sea_state);
    let components: Vec<_> = state.components.values().cloned().collect();
    for c in components {
        if !available(w, &c) {
            continue;
        }
        let (Some(a), Some(b)) = (previous.get(&c.unit), state.samples.get(&c.unit)) else {
            continue;
        };
        let Some((from, to, depth)) = sensor_path(&c, a, b) else {
            continue;
        };
        if from.range_to(to) <= 1e-12 {
            continue;
        }
        let mut crossings: BTreeMap<EntityId, (Segment, Vec2)> = BTreeMap::new();
        for segment in &state.segments {
            if segment.side == c.side || segment.to_at >= w.tick || segment.depth != depth {
                continue;
            }
            let Some(point) = intersection(from, to, segment.from, segment.to) else {
                continue;
            };
            crossings
                .entry(segment.creator)
                .or_insert((segment.clone(), point));
        }
        if crossings.len() > 1 {
            trace_pending(
                w,
                c.unit,
                "multiple physical wakes cut at once; association/priority not source-resolved",
                json!({"creator_ids":crossings.keys().collect::<Vec<_>>() }),
            );
            continue;
        }
        for (creator, (segment, point)) in crossings {
            let key = format!("{}:{}", c.unit.0, creator.0);
            if state.attempts.get(&key).is_some_and(|xs| {
                xs.iter()
                    .any(|x| x.point.range_to(point) < config.geometry.distinct_fix_separation_nm)
            }) {
                continue;
            }
            let mut observer_speed = from.range_to(to) * 3600.;
            for boundary in [8., 9., 15., 16.] {
                if (observer_speed - boundary).abs() < 1e-6 {
                    observer_speed = boundary;
                    break;
                }
            }
            if observer_speed > 8. + 1e-6 && observer_speed < 9. - 1e-6 {
                trace_pending(
                    w,
                    c.unit,
                    "fractional8-9kt observer speed has no printed modifier band",
                    json!({"speed":observer_speed}),
                );
                continue;
            }
            let Some(sea) = sea else {
                trace_pending(w, c.unit, "explicit source sea state absent", Value::Null);
                continue;
            };
            // The first source domain accepts only an actually straight,
            // unchanged-speed trail from the crossing to the current sample.
            // Then historical age*speed equals the book's wake-distance model.
            let heading = segment.from.bearing_to(segment.to);
            let straight = state
                .segments
                .iter()
                .filter(|s| s.creator == creator && s.to_at >= segment.to_at)
                .all(|s| {
                    crate::geom::angle_diff(heading, s.from.bearing_to(s.to)).abs() < 1e-6
                        && (s.speed_kts - segment.speed_kts).abs() < 1e-5
                });
            if !straight {
                trace_pending(
                    w,
                    c.unit,
                    "turning or changed-speed wake requires an explicit time-late interpretation",
                    json!({"creator":creator,"segment":segment.id}),
                );
                continue;
            }
            let age = (w.tick - segment.to_at) as f64;
            let probability = match super::wake_probability(
                row(&c.fit.system)["generation"].as_u64().unwrap() as usize,
                segment.speed_kts * age / 3600.,
                segment.speed_kts,
                observer_speed,
                sea,
                depth,
                true,
                c.fit.alerted_source.is_some(),
            ) {
                Ok(p) => p,
                Err(reason) => {
                    trace_pending(w, c.unit, &reason, Value::Null);
                    continue;
                }
            };
            let sequence = state.attempts.get(&key).map_or(0, Vec::len);
            let identity = format!(
                "5.7.3:{}:{}:{}:{sequence}",
                c.unit.0, c.fit.system, creator.0
            );
            let mut rng = Rng::new(w.seed).derive(Rng::hash_tag(0, 0, &identity));
            let die = 1 + (rng.next_f64() * 100.) as u8;
            let detected = f64::from(die) <= probability;
            state
                .attempts
                .entry(key.clone())
                .or_default()
                .push(Attempt {
                    point,
                    at: w.tick,
                    identity: identity.clone(),
                    die,
                    probability,
                    detected,
                });
            event(
                w,
                c.unit,
                "5.7.3.wake_attempt",
                None,
                json!({"identity":identity,"truth_creator":creator,"historical_segment":segment,"intersection":point,"original_d100":die,"probability":probability,"detected":detected,"current_enemy_position_used":false}),
            );
            if !detected {
                continue;
            }
            if !state.observations.contains_key(&key) {
                let n = state.next_alias.entry(c.unit).or_default();
                *n += 1;
                state.observations.insert(
                    key.clone(),
                    OwnWake {
                        wake_alias: format!("wake-{n}"),
                        observer: c.unit,
                        side: c.side,
                        sensor: binding(&c.fit.system),
                        first_at: w.tick,
                        last_at: w.tick,
                        points: Vec::new(),
                        course_deg: None,
                        nuclear: None,
                    },
                );
            }
            let own = state.observations.get_mut(&key).unwrap();
            own.last_at = w.tick;
            own.points.push(Cut { point, at: w.tick });
            if c.fit.radiological_source.is_some() {
                own.nuclear = state.nuclear_creators.get(&creator).copied();
            }
            let mut detail = json!({"wake_alias":own.wake_alias,"point":point,"at":w.tick,"sensor":own.sensor,"knowledge":"a submarine passed recently; current submarine position and direction are not observed"});
            if let Some(nuclear) = own.nuclear {
                detail["nuclear_powered"] = json!(nuclear);
            }
            event(w, c.unit, "5.7.3.wake_detected", Some(c.side), detail);
            if own.points.len() >= 3 && own.course_deg.is_none() {
                own.course_deg = Some(heading);
                event(
                    w,
                    c.unit,
                    "5.7.3.wake_course",
                    Some(c.side),
                    json!({"wake_alias":own.wake_alias,"course_deg":heading,"at":w.tick,"source":"5.7.3 three distinct successful actual cuts; explicit historical-local-tangent RTS policy; no current enemy course"}),
                );
            }
        }
    }
    w.rules.detection.as_mut().unwrap().soks = state;
}
pub fn own_view(w: &World, side: Side, unit: EntityId) -> Value {
    let Some(s) = runtime(w) else {
        return Value::Null;
    };
    let component=s.components.get(&unit).filter(|c|c.side==side&&body(w,c)).map(|c|json!({"system":c.fit.system,"installed":c.fit.installed,"enabled":c.fit.enabled,"source":c.fit.source,"available":available(w,c)}));
    let observations:Vec<_>=s.observations.values().filter(|x|x.observer==unit&&x.side==side).map(|x|json!({"wake_alias":x.wake_alias,"sensor":x.sensor,"first_at":x.first_at,"last_at":x.last_at,"measured_points":x.points,"rough_course_deg":x.course_deg,"nuclear_powered":x.nuclear})).collect();
    json!({"component":component,"wake_reports":observations})
}

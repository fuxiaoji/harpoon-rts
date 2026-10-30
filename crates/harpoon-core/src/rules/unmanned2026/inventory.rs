//! Scenario-authored physical UUV handling. Exact predeclared bodies move
//! between active World and inventory; no body, payload, repair or fuel refill
//! is invented. Mechanical numbers are explicit scenario policy, not §2.5.
use super::*;
use crate::types::PlatformKind;
use crate::world::Entity;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UuvLauncherFit {
    pub source: String,
    pub stowed: Vec<String>,
    pub launch_seconds: i64,
    pub recovery_seconds: i64,
    pub launch_depth_m: f64,
    pub host_depth_min_m: f64,
    pub host_depth_max_m: f64,
    pub host_max_speed_kts: f64,
    pub max_sea_state: usize,
    pub recovery_range_nm: f64,
    pub recovery_max_depth_m: f64,
    pub recovery_max_speed_kts: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct State {
    pub initial: BTreeMap<EntityId, Location>,
    stored: BTreeMap<EntityId, Stored>,
    deployed: BTreeMap<EntityId, Location>,
    jobs: BTreeMap<String, Job>,
}
impl State {
    pub(super) fn is_empty(&self) -> bool {
        self.initial.is_empty()
            && self.stored.is_empty()
            && self.deployed.is_empty()
            && self.jobs.is_empty()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Location {
    host: EntityId,
    launcher: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Stored {
    location: Location,
    entity: Entity,
    vehicle: VehicleState,
    stored_at: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Launch,
    Recover,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Job {
    location: Location,
    vehicle: EntityId,
    kind: Kind,
    requested_at: i64,
    /// Only real continuous capture qualifies this private work clock. Its
    /// absence never changes public admission into a hidden survival oracle.
    work_started_at: Option<i64>,
}
fn key(host: EntityId, launcher: &str) -> String {
    format!("{}:{launcher}", host.0)
}
fn config<'a>(
    w: &World,
    s: &'a super::State,
    host: EntityId,
    launcher: &str,
) -> Result<&'a UuvLauncherFit, String> {
    let e = w
        .entity(host)
        .ok_or("2.5 source launcher host unavailable")?;
    s.config
        .launchers
        .get(&e.name)
        .and_then(|m| m.get(launcher))
        .ok_or_else(|| "2.5 exact own UUV launcher fit unknown".into())
}
fn numbers(f: &UuvLauncherFit) -> Result<(), String> {
    if !authored(&f.source)
        || f.stowed.is_empty()
        || f.launch_seconds <= 0
        || f.recovery_seconds <= 0
        || f.max_sea_state > 9
        || [
            f.launch_depth_m,
            f.host_depth_min_m,
            f.host_depth_max_m,
            f.host_max_speed_kts,
            f.recovery_range_nm,
            f.recovery_max_depth_m,
            f.recovery_max_speed_kts,
        ]
        .iter()
        .any(|x| !x.is_finite() || *x < 0.)
        || f.host_depth_min_m > f.host_depth_max_m
        || f.recovery_range_nm <= 0.
    {
        return Err("2.5 UUV handling needs complete explicit scenario mechanical policy".into());
    }
    Ok(())
}
fn host_ready(
    w: &World,
    s: &super::State,
    host: EntityId,
    f: &UuvLauncherFit,
) -> Result<(), String> {
    let e = w
        .entity(host)
        .filter(|e| e.alive)
        .ok_or("2.5 UUV handling host absent")?;
    super::can_control(w, e.side, host)?;
    if disabled(s, host)
        || e.speed_kts > f.host_max_speed_kts
        || e.depth_m < f.host_depth_min_m
        || e.depth_m > f.host_depth_max_m
        || !e.speed_kts.is_finite()
        || !e.depth_m.is_finite()
        || w.context
            .radar_environment
            .as_ref()
            .is_none_or(|env| usize::from(env.sea_state) > f.max_sea_state)
    {
        return Err(
            "2.5 actual host movement/depth/sea state outside declared UUV handling envelope"
                .into(),
        );
    }
    Ok(())
}
fn launch_depth(w: &World, body: &Entity, f: &UuvLauncherFit) -> Result<(), String> {
    // An authored handling depth cannot manufacture a deeper-rated hull.
    body.class_def(&w.db)
        .ok_or("2.5 original stored body class missing")?;
    let maximum = super::super::submarine_annex::maximum_depth_m(w, body);
    if !maximum.is_finite() || maximum <= 0. || f.launch_depth_m > maximum {
        return Err(
            "2.5 declared launch depth exceeds the original physical body capability".into(),
        );
    }
    if let Some(env) = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| s.config.environment.as_ref())
    {
        if !env.water_depth_m.is_finite() || f.launch_depth_m > env.water_depth_m {
            return Err("2.5 declared launch depth exceeds the explicit local water depth".into());
        }
    }
    if let Some(depth_fit) = w
        .rules
        .navigation
        .as_ref()
        .and_then(|s| s.units.get(&body.id))
        .and_then(|u| u.fit.submarine.as_ref())
    {
        let mut at_launch = body.clone();
        at_launch.depth_m = f.launch_depth_m;
        at_launch.speed_kts = 0.;
        super::super::navigation2026::submarine::validate_fit(w, &at_launch, depth_fit)?;
    }
    Ok(())
}
pub(super) fn prepare_initial(w: &World, s: &mut super::State) -> Result<(), String> {
    for (host_name, launchers) in &s.config.launchers {
        let host = named(w, host_name)?;
        let parent = w.entity(host).unwrap();
        if !matches!(parent.kind, PlatformKind::Ship | PlatformKind::Sub) {
            return Err("2.5 UUV host must be a physical surface/subsurface platform".into());
        }
        for (launcher, f) in launchers {
            numbers(f)?;
            if launcher.trim().is_empty() {
                return Err("2.5 exact launcher key required".into());
            }
            for name in &f.stowed {
                let id = named(w, name)?;
                let body = own(w, parent.side, id)?;
                let fit = s
                    .config
                    .vehicles
                    .get(name)
                    .ok_or("2.5 stowed body needs exact source-declared unmanned hardware")?;
                if id == host
                    || body.kind != PlatformKind::Sub
                    || fit.domain != VehicleDomain::Underwater
                    || !s.vehicles.contains_key(&id)
                    || s.inventory.initial.contains_key(&id)
                    || s.config.launchers.contains_key(name)
                {
                    return Err(
                        "2.5 unique own predeclared underwater physical body required".into(),
                    );
                }
                launch_depth(w, body, f)?;
                s.inventory.initial.insert(
                    id,
                    Location {
                        host,
                        launcher: launcher.clone(),
                    },
                );
            }
        }
    }
    Ok(())
}
/// Called by scenario only after every module has initialized the original
/// bodies. All storage-side clocks therefore exist before physical removal.
pub fn finalize_initial_stow(w: &mut World) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let pending = w
        .rules
        .unmanned
        .as_ref()
        .map(|s| s.inventory.initial.clone())
        .unwrap_or_default();
    if pending.is_empty() {
        return Ok(());
    }
    let mut next = w.clone();
    for (id, location) in pending {
        commit_stow(&mut next, id, location)?;
    }
    next.rules
        .unmanned
        .as_mut()
        .unwrap()
        .inventory
        .initial
        .clear();
    *w = next;
    finish_telemetry(w);
    Ok(())
}
fn safe_stow(w: &World, id: EntityId) -> bool {
    let Some(e) = w.entity(id).filter(|e| e.alive) else {
        return false;
    };
    // This narrow handling policy does not grant free damage control or hide a
    // running repair session in storage. Damaged structural HP itself is kept.
    e.burn_s<=0. && e.flooding_s<=0.
        && !w.rules.delayed_orders.iter().any(|d|d.command.unit_id()==id)
        && !w.rules.repairs.as_ref().and_then(|s|s.units.get(&id)).is_some_and(|u|u.session.is_some()&&u.criticals.values().any(|c|c.remaining_levels>0))
        // Handling does not complete a pending propulsion/depth maneuver or
        // repair a jammed rudder merely by removing its body from the map.
        && !w.rules.navigation.as_ref().and_then(|s|s.units.get(&id)).is_some_and(|u|
            u.ready_at.is_some() || u.depth_operation.is_some()
            || u.astern.as_ref().is_some_and(|a|a.requested||a.moving_astern)
            || u.crash_back.as_ref().is_some_and(|c|c.active)
            || u.hard_rudder.active_episode.is_some() || u.hard_rudder.planned.is_some()
            || u.hard_rudder.pending.is_some())
}
fn commit_stow(w: &mut World, id: EntityId, location: Location) -> Result<(), String> {
    if !safe_stow(w, id) {
        return Err("2.5 physical body has unresolved fire/flood/bridge/repair work; warehouse cannot adjudicate it".into());
    }
    super::super::battery2026::pause_stowed(w, id)?;
    super::super::detection2026::stow_platform(w, id)?;
    super::super::communications2026::stow_platform(w, id);
    super::super::operations::stow_platform(w, id);
    let mut entity = w
        .entities
        .remove(&id)
        .ok_or("2.5 actual stowed body missing")?;
    let s = w.rules.unmanned.as_mut().ok_or("2.5 state missing")?;
    let mut vehicle = s
        .vehicles
        .remove(&id)
        .ok_or("2.5 physical control state missing")?;
    vehicle.transition = None;
    vehicle.recall = None;
    vehicle.mission = None;
    vehicle.mast_since = None;
    vehicle.loss = None;
    vehicle.last_wander_epoch = None;
    if s.config
        .vehicles
        .get(&vehicle.name)
        .is_some_and(|f| f.acoustic_modem)
    {
        vehicle.link = ControlLink::AcousticModem;
    }
    entity.waypoints.clear();
    entity.station = None;
    entity.assigned = None;
    entity.fire_on = false;
    entity.speed_kts = 0.;
    entity.ordered_speed_kts = 0.;
    entity.ext.insert("radar_on".into(), 0.);
    for sensor in &mut entity.sensors {
        sensor.active = false;
    }
    s.telemetry.remove(&id);
    s.recall_broadcasts.remove(&id);
    s.onboard.retain(|_, r| r.report.observer != id);
    s.pending_projection.retain(|r| r.report.observer != id);
    s.inventory.deployed.remove(&id);
    s.inventory.stored.insert(
        id,
        Stored {
            location,
            entity,
            vehicle,
            stored_at: w.tick,
        },
    );
    Ok(())
}
pub(super) fn reservations(
    s: &super::State,
    controller: EntityId,
    element: &str,
    exclude: EntityId,
) -> Vec<(EntityId, Mode)> {
    s.inventory
        .jobs
        .values()
        .filter(|j| j.kind == Kind::Launch && j.vehicle != exclude)
        .filter_map(|j| s.inventory.stored.get(&j.vehicle))
        .filter(|body| {
            body.vehicle.controller == Some(controller)
                && body.vehicle.element.as_deref() == Some(element)
        })
        .map(|body| (body.entity.id, body.vehicle.mode))
        .collect()
}
fn body_capacity(w: &World, s: &super::State, body: &Stored) -> Result<(), String> {
    let fit = s
        .config
        .vehicles
        .get(&body.vehicle.name)
        .ok_or("2.5 original stored source fit absent")?;
    validate_fit_numbers(fit)?;
    super::annex::validate(&body.entity.class, fit)?;
    if let (Some(controller), Some(element)) =
        (body.vehicle.controller, body.vehicle.element.as_deref())
    {
        own(w, body.entity.side, controller)?;
        let cf = controller_for(w, s, controller)?;
        if !cf.operational || disabled(s, controller) || !cf.elements.iter().any(|e| e == element) {
            return Err("2.5 stored body's original controller unavailable".into());
        }
        if s.vehicles.values().any(|v| {
            v.transition.as_ref().is_some_and(|t| {
                t.controller == Some(controller) && t.element.as_deref() == Some(element)
            })
        }) {
            return Err(
                "2.5 original control element already reserved for a pending switch".into(),
            );
        }
        capacity(s, controller, element, body.entity.id, body.vehicle.mode)?;
    } else if body.vehicle.mode == Mode::Hitl {
        return Err("2.5 stored HITL body has no exact control assignment".into());
    }
    if w.entities.contains_key(&body.entity.id) {
        return Err("2.5 body is already active".into());
    }
    Ok(())
}
fn available<'a>(
    s: &'a super::State,
    host: EntityId,
    launcher: &str,
    fit: &UuvLauncherFit,
) -> Option<&'a Stored> {
    // The scenario's explicit stock order chooses the one reserved original
    // body. Completion never rerolls its identity or substitutes a spare.
    fit.stowed.iter().find_map(|name| {
        s.inventory.stored.values().find(|b| {
            b.entity.name == *name
                && b.location.host == host
                && b.location.launcher == launcher
                && !s.inventory.jobs.values().any(|j| j.vehicle == b.entity.id)
        })
    })
}
pub(super) fn validate(
    w: &World,
    side: Side,
    actor: EntityId,
    action: &Action,
) -> Result<(), String> {
    if !active(w) || w.over || crate::exercise::locked(w, side) {
        return Err("2.5 UUV handling unavailable".into());
    }
    own(w, side, actor)?;
    let s = w.rules.unmanned.as_ref().ok_or("2.5 state absent")?;
    check(w, s)?;
    let launcher = match action {
        Action::LaunchUuv { launcher } | Action::RecoverUuv { launcher, .. } => launcher,
        _ => return Err("2.5 not UUV handling".into()),
    };
    let fit = config(w, s, actor, launcher)?;
    numbers(fit)?;
    host_ready(w, s, actor, fit)?;
    if s.inventory.jobs.contains_key(&key(actor, launcher)) {
        return Err("2.5 this physical handling station is reserved".into());
    }
    match action {
        Action::LaunchUuv { .. } => {
            let body = available(s, actor, launcher, fit)
                .ok_or("2.5 physical UUV stock unavailable or reserved")?;
            body_capacity(w, s, body)?;
            launch_depth(w, &body.entity, fit)?;
        }
        Action::RecoverUuv { vehicle, .. } => {
            let location = s
                .inventory
                .deployed
                .get(vehicle)
                .ok_or("2.5 own launched body is not in this handling roster")?;
            if location.host != actor
                || location.launcher != *launcher
                || s.reported_destroyed.contains(vehicle)
                || !s
                    .known_roster
                    .get(&actor)
                    .is_some_and(|known| known.contains(vehicle))
                || s.inventory.jobs.values().any(|j| j.vehicle == *vehicle)
            {
                return Err("2.5 own known unreserved UUV recovery roster required".into());
            }
            // Physical hidden survival/range is checked only by actual search,
            // never as a public readonly command-admission truth oracle.
        }
        _ => unreachable!(),
    }
    Ok(())
}
fn recoverable(w: &World, job: &Job, fit: &UuvLauncherFit) -> bool {
    let (Some(host), Some(body)) = (w.entity(job.location.host), w.entity(job.vehicle)) else {
        return false;
    };
    body.alive
        && body.side == host.side
        && body.kind == PlatformKind::Sub
        && safe_stow(w, job.vehicle)
        && body.depth_m <= fit.recovery_max_depth_m
        && body.speed_kts <= fit.recovery_max_speed_kts
        && host.pos.range_to(body.pos) <= fit.recovery_range_nm
}
pub(super) fn apply(
    w: &mut World,
    side: Side,
    actor: EntityId,
    action: &Action,
) -> Result<(), String> {
    validate(w, side, actor, action)?;
    let s = w.rules.unmanned.as_ref().unwrap();
    let (launcher, vehicle, kind) = match action {
        Action::LaunchUuv { launcher } => (
            launcher.clone(),
            available(s, actor, launcher, config(w, s, actor, launcher)?)
                .unwrap()
                .entity
                .id,
            Kind::Launch,
        ),
        Action::RecoverUuv { launcher, vehicle } => (launcher.clone(), *vehicle, Kind::Recover),
        _ => unreachable!(),
    };
    let fit = config(w, s, actor, &launcher)?.clone();
    let mut job = Job {
        location: Location {
            host: actor,
            launcher: launcher.clone(),
        },
        vehicle,
        kind,
        requested_at: w.tick,
        work_started_at: None,
    };
    if kind == Kind::Launch || recoverable(w, &job, &fit) {
        job.work_started_at = Some(w.tick);
    }
    w.rules
        .unmanned
        .as_mut()
        .unwrap()
        .inventory
        .jobs
        .insert(key(actor, &launcher), job);
    emit(
        w,
        actor,
        "2.5.uuv_handling_requested",
        json!({"launcher":launcher,"vehicle":vehicle,"kind":kind,"mechanical_policy":"scenario_assumption"}),
        false,
    );
    Ok(())
}
fn commit_launch(w: &mut World, job: &Job, fit: &UuvLauncherFit) -> Result<(), String> {
    let s = w.rules.unmanned.as_ref().unwrap();
    let body = s
        .inventory
        .stored
        .get(&job.vehicle)
        .cloned()
        .ok_or("2.5 reserved body disappeared")?;
    body_capacity(w, s, &body)?;
    launch_depth(w, &body.entity, fit)?;
    let host = w.entity(job.location.host).ok_or("2.5 host missing")?;
    let mut entity = body.entity.clone();
    entity.pos = host.pos;
    entity.depth_m = fit.launch_depth_m;
    entity.ordered_depth_m = fit.launch_depth_m;
    let mut next = w.clone();
    next.entities.insert(job.vehicle, entity);
    let s = next.rules.unmanned.as_mut().unwrap();
    s.inventory.stored.remove(&job.vehicle);
    s.inventory
        .deployed
        .insert(job.vehicle, job.location.clone());
    s.vehicles.insert(job.vehicle, body.vehicle);
    s.known_roster
        .entry(job.location.host)
        .or_default()
        .insert(job.vehicle);
    super::super::battery2026::resume_stowed(&mut next, job.vehicle, body.stored_at)?;
    super::super::detection2026::resume_platform(&mut next, job.vehicle)?;
    next.rules
        .unmanned
        .as_mut()
        .unwrap()
        .inventory
        .jobs
        .remove(&key(job.location.host, &job.location.launcher));
    *w = next;
    emit(
        w,
        job.location.host,
        "2.5.uuv_launched",
        json!({"launcher":job.location.launcher,"vehicle":job.vehicle,"same_original_body":true,"replenished":false}),
        false,
    );
    Ok(())
}
pub(super) fn step(w: &mut World) {
    if !w
        .rules
        .unmanned
        .as_ref()
        .is_some_and(|s| check(w, s).is_ok())
    {
        return;
    }
    let jobs = w
        .rules
        .unmanned
        .as_ref()
        .map(|s| s.inventory.jobs.clone())
        .unwrap_or_default();
    for (key, mut job) in jobs {
        let Some(s) = w.rules.unmanned.as_ref() else {
            return;
        };
        let Ok(fit) = config(w, s, job.location.host, &job.location.launcher).cloned() else {
            continue;
        };
        if host_ready(w, s, job.location.host, &fit).is_err() {
            if job.kind == Kind::Launch {
                w.rules
                    .unmanned
                    .as_mut()
                    .unwrap()
                    .inventory
                    .jobs
                    .remove(&key);
                emit(
                    w,
                    job.location.host,
                    "2.5.uuv_handling_cancelled",
                    json!({"launcher":job.location.launcher,"reason":"own_host_handling_envelope_changed"}),
                    false,
                );
            } else {
                job.work_started_at = None;
                w.rules
                    .unmanned
                    .as_mut()
                    .unwrap()
                    .inventory
                    .jobs
                    .insert(key, job);
            }
            continue;
        }
        if job.kind == Kind::Recover && !recoverable(w, &job, &fit) {
            job.work_started_at = None;
            w.rules
                .unmanned
                .as_mut()
                .unwrap()
                .inventory
                .jobs
                .insert(key, job);
            continue;
        }
        let began = *job.work_started_at.get_or_insert(w.tick);
        let duration = if job.kind == Kind::Launch {
            fit.launch_seconds
        } else {
            fit.recovery_seconds
        };
        if w.tick - began < duration {
            w.rules
                .unmanned
                .as_mut()
                .unwrap()
                .inventory
                .jobs
                .insert(key, job);
            continue;
        }
        if job.kind == Kind::Launch {
            if let Err(reason) = commit_launch(w, &job, &fit) {
                w.rules
                    .unmanned
                    .as_mut()
                    .unwrap()
                    .inventory
                    .jobs
                    .remove(&key);
                emit(
                    w,
                    job.location.host,
                    "2.5.uuv_handling_cancelled",
                    json!({"launcher":job.location.launcher,"reason":reason}),
                    false,
                );
            }
        } else {
            let mut next = w.clone();
            if commit_stow(&mut next, job.vehicle, job.location.clone()).is_ok() {
                next.rules
                    .unmanned
                    .as_mut()
                    .unwrap()
                    .inventory
                    .jobs
                    .remove(&key);
                *w = next;
                emit(
                    w,
                    job.location.host,
                    "2.5.uuv_recovered",
                    json!({"launcher":job.location.launcher,"vehicle":job.vehicle,"same_original_body":true,"replenished":false}),
                    false,
                );
            }
        }
    }
}
pub(super) fn own_view(w: &World, s: &super::State, host: EntityId) -> Value {
    let Some(entity) = w.entity(host) else {
        return json!({});
    };
    let Some(fits) = s.config.launchers.get(&entity.name) else {
        return json!({});
    };
    let mut out = serde_json::Map::new();
    for launcher in fits.keys() {
        let bodies: Vec<_> = s
            .inventory
            .stored
            .values()
            .filter(|b| b.location.host == host && b.location.launcher == *launcher)
            .collect();
        let job = s.inventory.jobs.get(&key(host, launcher));
        let reserved = job.filter(|j| j.kind == Kind::Launch).map_or(0, |_| 1);
        out.insert(launcher.clone(),json!({"stock":bodies.len(),"reserved":reserved,
            "units":bodies.iter().map(|b|json!({"unit":b.entity.id,"name":b.entity.name,"class":b.entity.class})).collect::<Vec<_>>(),
            "job":job.map(|j|json!({"kind":j.kind,"vehicle":j.vehicle,"requested_at":j.requested_at})),
            "mechanical_policy":"scenario_assumption; same original body, no replenishment"}));
    }
    Value::Object(out)
}

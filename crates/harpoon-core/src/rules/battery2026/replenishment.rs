//! §3.2.5 printed return-to-port requirement, with explicit authored logistics.
use crate::{EntityId, Side, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/battery-replenishment-2026.json"
        ))
        .unwrap()
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub depots: BTreeMap<String, DepotConfig>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepotConfig {
    pub source: String,
    pub port: String,
    pub facility: String,
    pub maximum_distance_nm: f64,
    pub work_seconds: i64,
    pub fuel_units: f64,
    pub oxygen_units: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Depot {
    pub config: DepotConfig,
    pub port: EntityId,
    pub facility: EntityId,
    pub fuel_units: f64,
    pub oxygen_units: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub provider: String,
    pub units: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub request: Request,
    pub started: i64,
    pub due: i64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub depots: BTreeMap<String, Depot>,
    pub requests: BTreeMap<EntityId, Vec<Request>>,
    pub jobs: BTreeMap<EntityId, Job>,
}
fn named(w: &World, name: &str) -> Result<EntityId, String> {
    let ids: Vec<_> = w
        .entities
        .values()
        .filter(|e| e.name == name)
        .map(|e| e.id)
        .collect();
    if ids.len() == 1 {
        Ok(ids[0])
    } else {
        Err("3.2.5 exact unique port/depot entity name required".into())
    }
}
fn state(w: &World) -> Result<&State, String> {
    w.rules
        .battery
        .as_ref()
        .and_then(|s| s.replenishment.as_ref())
        .ok_or("3.2.5 authored port replenishment not configured".into())
}
fn mutable(w: &mut World) -> &mut State {
    w.rules
        .battery
        .as_mut()
        .unwrap()
        .replenishment
        .as_mut()
        .unwrap()
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    let Some(c) = w
        .context
        .battery_2026
        .as_ref()
        .and_then(|c| c.replenishment.clone())
    else {
        return Ok(());
    };
    if !c.source.starts_with("scenario_assumption:") {
        return Err("3.2.5 logistics contract must be an explicit scenario assumption".into());
    }
    let mut s = State::default();
    let mut physical_depots = BTreeSet::new();
    for (key, c) in c.depots {
        if key.is_empty()
            || !c.source.starts_with("scenario_assumption:")
            || !c.maximum_distance_nm.is_finite()
            || c.maximum_distance_nm <= 0.
            || c.work_seconds <= 0
            || !c.fuel_units.is_finite()
            || c.fuel_units < 0.
            || !c.oxygen_units.is_finite()
            || c.oxygen_units < 0.
        {
            return Err("3.2.5 invalid explicit work/stock/distance contract".into());
        }
        let port = named(w, &c.port)?;
        let facility = named(w, &c.facility)?;
        if !physical_depots.insert(facility) {
            return Err("3.2.5 one physical depot cannot declare duplicate alias stocks".into());
        }
        let g = w
            .rules
            .ground
            .as_ref()
            .ok_or("3.2.5 source physical port required")?;
        if !g
            .complexes
            .get(&port)
            .is_some_and(|p| p.kind == crate::rules::ground2026::ComplexKind::Port)
            || !g
                .facilities
                .get(&facility)
                .is_some_and(|f| f.parent == Some(port))
            || w.entity(port).unwrap().side != w.entity(facility).unwrap().side
        {
            return Err(
                "3.2.5 depot must be a same-side physical child of a source Port complex".into(),
            );
        }
        s.depots.insert(
            key,
            Depot {
                port,
                facility,
                fuel_units: c.fuel_units,
                oxygen_units: c.oxygen_units,
                config: c,
            },
        );
    }
    w.rules
        .battery
        .as_mut()
        .ok_or("3.2.5 battery ledger absent")?
        .replenishment = Some(s);
    Ok(())
}
fn eligible(w: &World, side: Side, id: EntityId, r: &Request, stock: bool) -> Result<(), String> {
    let s = state(w)?;
    let d = s
        .depots
        .get(&r.provider)
        .ok_or("3.2.5 unknown own supply provider")?;
    let e = w
        .entity(id)
        .filter(|e| e.alive && e.side == side && e.depth_m == 0. && e.speed_kts == 0.)
        .ok_or("3.2.5 live own surfaced stationary submarine required")?;
    let u = w
        .rules
        .battery
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .filter(|u| u.platform_class == e.class && u.aip_fitted && u.stowed_at.is_none())
        .ok_or("3.2.5 actual unstowed AIP fit required")?;
    if u.aip_operational || u.charging || !u.aip_critical_identities.is_empty() {
        return Err("3.2.5 AIP/charging must be stopped and AIP machinery operational".into());
    }
    let current = u
        .aip_remaining_units
        .ok_or("3.2.5 current AIP potential unknown")?;
    if !r.units.is_finite()
        || r.units <= 0.
        || current + r.units > tables()["aip_capacity_units"].as_f64().unwrap()
    {
        return Err("3.2.5 positive quantity within printed AIP capacity required".into());
    }
    let p = w
        .entity(d.port)
        .filter(|e| e.alive && e.side == side)
        .ok_or("3.2.5 friendly live port required")?;
    let f = w
        .entity(d.facility)
        .filter(|e| e.alive && e.side == side)
        .ok_or("3.2.5 friendly live depot required")?;
    let g = w.rules.ground.as_ref().unwrap();
    if !g
        .complexes
        .get(&p.id)
        .is_some_and(|p| p.kind == crate::rules::ground2026::ComplexKind::Port)
        || !g
            .facilities
            .get(&f.id)
            .is_some_and(|f| !f.disabled && !f.destroyed && f.parent == Some(p.id))
        || e.pos.range_to(f.pos) > d.config.maximum_distance_nm
    {
        return Err(
            "3.2.5 operational port depot and actual authored berth geometry required".into(),
        );
    }
    if stock && (d.fuel_units < r.units || d.oxygen_units < r.units) {
        return Err("3.2.5 insufficient paired finite fuel/oxygen stock".into());
    }
    w.tick
        .checked_add(d.config.work_seconds)
        .ok_or("3.2.5 work clock overflow")?;
    Ok(())
}
pub fn validate(w: &World, side: Side, id: EntityId, r: &Request) -> Result<(), String> {
    if state(w)?.jobs.contains_key(&id) {
        return Err("3.2.5 submarine already has active replenishment work".into());
    }
    eligible(w, side, id, r, true)
}
pub fn prepare(w: &mut World, side: Side, id: EntityId, r: Request) -> Result<(), String> {
    validate(w, side, id, &r)?;
    let requests = mutable(w).requests.entry(id).or_default();
    if !requests
        .iter()
        .any(|a| a.provider == r.provider && a.units == r.units)
    {
        requests.push(r)
    }
    Ok(())
}
pub fn validate_cancel(w: &World, side: Side, id: EntityId) -> Result<(), String> {
    if !w.entity(id).is_some_and(|e| e.alive && e.side == side) {
        return Err("3.2.5 own live submarine required".into());
    }
    let s = state(w)?;
    if !s.jobs.contains_key(&id) && !s.requests.contains_key(&id) {
        return Err("3.2.5 no replenishment work to cancel".into());
    }
    Ok(())
}
fn finish(w: &mut World, id: EntityId, job: Job, credit: bool, reason: &str) {
    if credit {
        let u = w
            .rules
            .battery
            .as_mut()
            .unwrap()
            .units
            .get_mut(&id)
            .unwrap();
        u.aip_remaining_units = Some(u.aip_remaining_units.unwrap() + job.request.units);
    } else {
        let d = mutable(w).depots.get_mut(&job.request.provider).unwrap();
        d.fuel_units += job.request.units;
        d.oxygen_units += job.request.units;
    }
    super::trace(
        w,
        id,
        if credit {
            "3.2.5.replenishment_complete"
        } else {
            "3.2.5.replenishment_cancelled"
        },
        json!({"provider":job.request.provider,"units":job.request.units,"reason":reason,"source":tables()["source"],"policy":tables()["execution"]["revision"]}),
    );
}
pub fn cancel(w: &mut World, side: Side, id: EntityId) -> Result<(), String> {
    validate_cancel(w, side, id)?;
    mutable(w).requests.remove(&id);
    if let Some(job) = mutable(w).jobs.remove(&id) {
        finish(w, id, job, false, "own cancellation; no AIP credit")
    }
    Ok(())
}
pub fn step(w: &mut World) {
    if state(w).is_err() {
        return;
    }
    let requests = std::mem::take(&mut mutable(w).requests);
    for (id, rs) in requests {
        if rs.len() != 1 {
            super::trace(
                w,
                id,
                "3.2.5.replenishment_blocked",
                json!({"reason":"conflicting intents, no reservation"}),
            );
            continue;
        }
        let r = rs.into_iter().next().unwrap();
        let side = w.entity(id).map(|e| e.side).unwrap_or(Side::Blue);
        if let Err(reason) = validate(w, side, id, &r) {
            super::trace(
                w,
                id,
                "3.2.5.replenishment_blocked",
                json!({"reason":reason}),
            );
            continue;
        }
        let now = w.tick;
        let s = mutable(w);
        let d = s.depots.get_mut(&r.provider).unwrap();
        let due = now + d.config.work_seconds;
        d.fuel_units -= r.units;
        d.oxygen_units -= r.units;
        s.jobs.insert(
            id,
            Job {
                request: r,
                started: now,
                due,
            },
        );
    }
    let jobs = std::mem::take(&mut mutable(w).jobs);
    for (id, job) in jobs {
        let result = w
            .entity(id)
            .map(|e| e.side)
            .ok_or("3.2.5 submarine absent".into())
            .and_then(|side| eligible(w, side, id, &job.request, false));
        match result {
            Err(reason) => finish(w, id, job, false, &reason),
            Ok(()) if w.tick >= job.due => finish(
                w,
                id,
                job,
                true,
                "continuous authored port work completed once",
            ),
            Ok(()) => {
                mutable(w).jobs.insert(id, job);
            }
        }
    }
}
pub fn own_job(w: &World, id: EntityId) -> Value {
    state(w).ok().and_then(|s|s.jobs.get(&id)).map(|j|json!({"provider":j.request.provider,"units":j.request.units,"started":j.started,"due":j.due})).unwrap_or(Value::Null)
}

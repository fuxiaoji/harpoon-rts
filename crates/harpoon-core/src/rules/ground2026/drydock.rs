//! Source11.1 pump and gate effects with explicitly declared RTS cycle time.
use super::{Action, State as GroundState};
use crate::types::PlatformKind;
use crate::{EntityId, EventKind, Side, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub controller: String,
    pub pump: String,
    pub gate: String,
    pub initial_flooded: bool,
    pub seconds_per_cycle: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dock {
    pub config: Configuration,
    pub owner: Side,
    pub controller: EntityId,
    pub pump: EntityId,
    pub gate: EntityId,
    pub flooded: bool,
    pub gate_failed: bool,
    pub job: Option<Job>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub started: i64,
    pub due: i64,
    pub flooded: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub controller: EntityId,
    pub service: String,
    pub flooded: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub services: BTreeMap<String, Dock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requests: Vec<Request>,
}
pub(super) fn initialize(
    w: &World,
    g: &GroundState,
    configs: &BTreeMap<String, Configuration>,
) -> Result<Option<State>, String> {
    if configs.is_empty() {
        return Ok(None);
    }
    let mut state = State::default();
    let mut hardware = BTreeSet::new();
    for (key, c) in configs {
        if key.trim().is_empty()
            || c.source.trim().is_empty()
            || c.seconds_per_cycle <= 0
            || c.seconds_per_cycle > i64::MAX - w.tick
        {
            return Err("11.drydock cycle time and explicit source assignment required".into());
        }
        let controller = super::find_named(w, &c.controller)?;
        let pump = super::find_named(w, &c.pump)?;
        let gate = super::find_named(w, &c.gate)?;
        let owner = w
            .entity(controller)
            .ok_or("11.drydock controller absent")?
            .side;
        if pump == gate
            || !hardware.insert(pump)
            || !hardware.insert(gate)
            || ![controller, pump, gate].iter().all(|id| {
                w.entity(*id)
                    .is_some_and(|e| e.alive && e.side == owner && e.kind == PlatformKind::Base)
            })
        {
            return Err(
                "11.drydock exact own stationary hardware must not serve multiple docks".into(),
            );
        }
        if !g
            .facilities
            .get(&pump)
            .is_some_and(|f| f.target.effect == "no_pump")
            || !g
                .facilities
                .get(&gate)
                .is_some_and(|f| f.target.effect == "drydock_flooded")
        {
            return Err(
                "11.drydock actual pumping station and gate source templates required".into(),
            );
        }
        state.services.insert(
            key.clone(),
            Dock {
                config: c.clone(),
                owner,
                controller,
                pump,
                gate,
                flooded: c.initial_flooded,
                gate_failed: false,
                job: None,
            },
        );
    }
    Ok(Some(state))
}
fn identity(w: &World, d: &Dock) -> Result<(), String> {
    for (id, name) in [
        (d.controller, &d.config.controller),
        (d.pump, &d.config.pump),
        (d.gate, &d.config.gate),
    ] {
        if super::find_named(w, name)? != id
            || !w.entity(id).is_some_and(|e| {
                e.alive
                    && e.hp > 0.
                    && e.side == d.owner
                    && e.kind == PlatformKind::Base
                    && e.speed_kts == 0.
            })
        {
            return Err("11.drydock original own facility identity unavailable".into());
        }
    }
    Ok(())
}
fn gate_failed(w: &World, d: &Dock) -> bool {
    d.gate_failed
        || !w
            .rules
            .ground
            .as_ref()
            .and_then(|g| g.facilities.get(&d.gate))
            .is_some_and(|f| f.target.effect == "drydock_flooded" && !f.disabled && !f.destroyed)
}
fn pumping(w: &World, d: &Dock) -> Result<(), String> {
    identity(w, d)?;
    if gate_failed(w, d) {
        return Err("11.1 damaged drydock gate floods dock and makes it unusable".into());
    }
    if !super::operational(w, d.controller)
        || !w
            .rules
            .ground
            .as_ref()
            .and_then(|g| g.facilities.get(&d.pump))
            .is_some_and(|f| f.target.effect == "no_pump" && !f.disabled && !f.destroyed)
    {
        return Err("11.1 pumping station cannot empty or flood the drydock".into());
    }
    Ok(())
}
pub fn validate_action(w: &World, controller: EntityId, action: &Action) -> Result<(), String> {
    let Action::PumpDock { service, flooded } = action else {
        return Err("11.drydock pump command required".into());
    };
    let d = w
        .rules
        .ground
        .as_ref()
        .and_then(|g| g.drydocks.as_ref())
        .and_then(|s| s.services.get(service))
        .ok_or("11.drydock exact sourced assignment missing")?;
    if controller != d.controller || w.over || crate::exercise::locked(w, d.owner) {
        return Err("11.drydock live own controller in active exercise required".into());
    }
    pumping(w, d)?;
    if d.job.is_some() || d.flooded == *flooded {
        return Err("11.drydock cycle already active or requested state already reached".into());
    }
    Ok(())
}
pub fn apply(w: &mut World, controller: EntityId, action: &Action) -> Result<(), String> {
    validate_action(w, controller, action)?;
    let Action::PumpDock { service, flooded } = action else {
        unreachable!()
    };
    w.rules
        .ground
        .as_mut()
        .unwrap()
        .drydocks
        .as_mut()
        .unwrap()
        .requests
        .push(Request {
            controller,
            service: service.clone(),
            flooded: *flooded,
        });
    Ok(())
}
fn trace(w: &mut World, d: &Dock, service: &str, rule: &str, detail: Value) {
    w.emit(EventKind::RulesEvent{domain:"ground".into(),rule:rule.into(),side:Some(d.owner),unit:Some(d.controller),contact:None,detail:json!({"service":service,"detail":detail,"source":d.config.source,"cycle_time":"explicit scenario assumption; no invented printed pump performance"}).to_string()},Visibility::Side(d.owner));
}
pub fn step(w: &mut World) {
    let Some(mut s) = w.rules.ground.as_mut().and_then(|g| g.drydocks.take()) else {
        return;
    };
    for (key, d) in &mut s.services {
        // Gate casualty is a physical flood, even with no pump request. It never
        // repairs itself when a queued cycle or arbitrary timer expires.
        if gate_failed(w, d) && !d.gate_failed {
            d.flooded = true;
            d.gate_failed = true;
            d.job = None;
            trace(
                w,
                d,
                key,
                "11.drydock.gate_flooded",
                json!({"flooded":true,"usable":false}),
            );
        }
        if let Some(job) = d.job.take() {
            let reason = pumping(w, d).err().or_else(|| {
                (job.started > w.tick
                    || job.due != job.started.saturating_add(d.config.seconds_per_cycle))
                .then(|| "invalid saved pump clock".into())
            });
            if let Some(reason) = reason {
                trace(
                    w,
                    d,
                    key,
                    "11.drydock.cycle_cancelled",
                    json!({"reason":reason,"flooded":d.flooded}),
                );
            } else if job.due <= w.tick {
                d.flooded = job.flooded;
                trace(
                    w,
                    d,
                    key,
                    "11.drydock.cycle_completed",
                    json!({"started":job.started,"due":job.due,"flooded":d.flooded}),
                );
            } else {
                d.job = Some(job);
            }
        }
    }
    let mut requests: BTreeMap<String, BTreeSet<(EntityId, bool)>> = BTreeMap::new();
    for r in std::mem::take(&mut s.requests) {
        requests
            .entry(r.service)
            .or_default()
            .insert((r.controller, r.flooded));
    }
    for (key, rs) in requests {
        let Some(d) = s.services.get_mut(&key) else {
            continue;
        };
        let &(controller, flooded) = rs.first().unwrap();
        let result = if rs.len() != 1 || controller != d.controller {
            Err("conflicting simultaneous dock requests".into())
        } else if d.job.is_some() || d.flooded == flooded {
            Err("pump cycle active or requested water state already reached".into())
        } else {
            pumping(w, d)
        };
        if let Err(reason) = result {
            trace(
                w,
                d,
                &key,
                "11.drydock.cycle_rejected",
                json!({"reason":reason}),
            );
            continue;
        }
        d.job = Some(Job {
            started: w.tick,
            due: w.tick + d.config.seconds_per_cycle,
            flooded,
        });
        trace(w, d, &key, "11.drydock.cycle_started", json!({"job":d.job}));
    }
    w.rules.ground.as_mut().unwrap().drydocks = Some(s);
}
pub(super) fn own_view(w: &World, controller: EntityId) -> Option<Value> {
    let side = w.entity(controller)?.side;
    let s = w.rules.ground.as_ref()?.drydocks.as_ref()?;
    let services:BTreeMap<_,_>=s.services.iter().filter(|(_,d)|d.owner==side&&d.controller==controller).map(|(key,d)|{
  (key,json!({"pump":d.pump,"gate":d.gate,"flooded":d.flooded,"usable":!d.flooded&&!gate_failed(w,d)&&identity(w,d).is_ok(),"job":d.job,"pumping_pending":pumping(w,d).err(),"source":d.config.source,"seconds_per_cycle":d.config.seconds_per_cycle}))
 }).collect();
    (!services.is_empty()).then(|| json!({"services":services}))
}

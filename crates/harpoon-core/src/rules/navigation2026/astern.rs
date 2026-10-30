//! Source3.1.3, with explicit continuous braking and per-TT risk timing.
//! Speed remains a nonnegative magnitude; bow heading and travel differ by180°.
use super::{tables, trace};
use crate::{geom::angle_diff, EntityId, Rng, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub requested: bool,
    pub moving_astern: bool,
    pub heading_deg: f64,
    pub trials: BTreeMap<i64, Trial>,
    pub pending: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trial {
    pub tick: i64,
    pub body: Body,
    pub loss_d100: u8,
    pub foul_d100: u8,
    pub lost: bool,
    pub fouled: bool,
    pub applied: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Body {
    Decoy { object: EntityId },
    Sonar { key: String, sensor: String },
}
pub fn engaged(w: &World, id: EntityId) -> bool {
    super::configured(w, id)
        && w.rules
            .navigation
            .as_ref()
            .and_then(|s| s.units.get(&id))
            .and_then(|u| u.astern.as_ref())
            .is_some_and(|s| s.requested || s.moving_astern)
}
pub fn order(w: &mut World, id: EntityId, enabled: bool) -> Result<(), String> {
    let e = w.entity(id).ok_or("3.1.3 ship missing")?.clone();
    let u = &w.rules.navigation.as_ref().unwrap().units[&id];
    if enabled && u.heave_to {
        return Err("3.1.3/3.1.5 heave-to reverse steering requires adjudication".into());
    }
    let u = w
        .rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap();
    if !enabled && u.astern.is_none() {
        return Ok(());
    }
    let state = u.astern.get_or_insert_with(|| State {
        requested: enabled,
        moving_astern: false,
        heading_deg: e.course_deg,
        trials: BTreeMap::new(),
        pending: None,
    });
    if !state.requested && !state.moving_astern && enabled {
        state.heading_deg = e.course_deg;
    }
    state.requested = enabled;
    // This is a deliberate standalone navigation order, not a reverse route planner.
    if enabled {
        u.turning = None;
        let e = w.entity_mut(id).unwrap();
        e.station = None;
        e.waypoints.clear();
        e.ordered_course_deg = e.course_deg;
        e.ext.remove("terrain.resume_speed");
    }
    Ok(())
}
fn pending(w: &mut World, id: EntityId, rule: &str, reason: &str, detail: Value) -> String {
    let state = w.rules.navigation.as_mut().unwrap();
    let u = state.units.get_mut(&id).unwrap();
    if let Some(a) = &mut u.astern {
        a.pending = Some(reason.into());
    }
    if state.diagnostics.insert(format!("{id:?}:{rule}:{reason}")) {
        trace(
            w,
            id,
            rule,
            json!({"status":"pending","reason":reason,"evidence":detail}),
        );
    }
    reason.into()
}
fn bodies(w: &World, id: EntityId) -> Result<Vec<Body>, String> {
    let mut bodies = Vec::new();
    let e = w.entity(id).ok_or("3.1.3 ship missing")?;
    if w.rules
        .mines
        .as_ref()
        .and_then(|s| s.gear.get(&id))
        .is_some_and(|g| !g.lost)
    {
        return Err("3.1.3 sweep-gear cable count/loss lifecycle requires adjudication".into());
    }
    if let Some(s) = &w.rules.detection {
        for (key, job) in &s.arrays {
            let Some(sensor) = key.strip_prefix(&format!("{}:", id.0)) else {
                continue;
            };
            if job.lost {
                continue;
            }
            match s.config.bindings.get(&format!("{}:{sensor}", e.class)) {
                Some(super::super::detection2026::Receiver::Sonar {
                    mount:
                        super::super::detection2026::SonarMount::Towed
                        | super::super::detection2026::SonarMount::Vds,
                    ..
                }) => {
                    if !job.deployed || job.recovering {
                        return Err(
                            "3.1.3 streaming/recovering cable exposure requires adjudication"
                                .into(),
                        );
                    }
                    bodies.push(Body::Sonar {
                        key: key.clone(),
                        sensor: sensor.into(),
                    });
                }
                _ => return Err("3.1.3 unbound deployed cable kind requires adjudication".into()),
            }
        }
    }
    if let Some(s) = &w.rules.ew {
        for o in s.objects.values().filter(|o| {
            o.owner == id
                && o.kind == super::super::ew2026::ObjectKind::TowedTorpedoDecoy
                && o.status == super::super::ew2026::Status::Deployed
        }) {
            if !o.effective(w.tick) {
                return Err("3.1.3 deploying towed-decoy cable requires adjudication".into());
            }
            bodies.push(Body::Decoy { object: o.id });
        }
    }
    Ok(bodies)
}
/// Before acceleration/steering, stop unsupported combinations. Direction only
/// changes after the previous tick has reached zero speed.
pub fn prepare(w: &mut World, id: EntityId) -> Result<(), String> {
    if !engaged(w, id) {
        return Ok(());
    }
    let e = w.entity(id).ok_or("3.1.3 ship missing")?.clone();
    let u = w.rules.navigation.as_ref().unwrap().units[&id].clone();
    let a = u.astern.as_ref().unwrap();
    if a.trials.values().any(|t| !t.applied) {
        return Err("3.1.3 retained original engineering trial requires referee resolution".into());
    }
    if angle_diff(e.course_deg, a.heading_deg).abs() > 1e-9
        || angle_diff(e.ordered_course_deg, a.heading_deg).abs() > 1e-9
        || e.station.is_some()
        || !e.waypoints.is_empty()
        || u.heave_to
        || super::super::damage_runtime::bridge_holds(&e, w.tick)
        || e.ext.get("damage.rudder_jammed").is_some_and(|v| *v > 0.)
        || e.ext.get("damage.rudder_disabled").is_some_and(|v| *v > 0.)
    {
        return Err(pending(
            w,
            id,
            "3.1.3.turn_pending",
            "3.1.3 reverse or transition steering/casualty combination not source-verified",
            json!({"heading_deg":a.heading_deg,"course_deg":e.course_deg,"ordered_course_deg":e.ordered_course_deg}),
        ));
    }
    if a.requested || a.moving_astern {
        let b = bodies(w, id)
            .map_err(|reason| pending(w, id, "3.1.3.tow_selection_pending", &reason, json!({})))?;
        if b.len() > 1 {
            return Err(pending(
                w,
                id,
                "3.1.3.tow_selection_pending",
                "3.1.3 multiple deployed bodies: no verified selection/priority",
                json!({"bodies":b}),
            ));
        }
    }
    let changed = {
        let a = w
            .rules
            .navigation
            .as_mut()
            .unwrap()
            .units
            .get_mut(&id)
            .unwrap()
            .astern
            .as_mut()
            .unwrap();
        a.pending = None;
        if a.requested != a.moving_astern && e.speed_kts <= 1e-9 {
            a.moving_astern = a.requested;
            Some(a.moving_astern)
        } else {
            None
        }
    };
    if let Some(reverse) = changed {
        trace(
            w,
            id,
            "3.1.3.direction",
            json!({"moving_astern":reverse,"speed_kts":e.speed_kts,"policy":"RTS_brake_to_zero_before_direction_change"}),
        );
    }
    Ok(())
}
fn risk(w: &mut World, id: EntityId) -> Result<(), String> {
    let window = w.tick / tables()["turn_seconds"].as_i64().unwrap();
    let old = w.rules.navigation.as_ref().unwrap().units[&id]
        .astern
        .as_ref()
        .unwrap()
        .trials
        .get(&window)
        .cloned();
    let mut trial = if let Some(t) = old {
        t
    } else {
        let b = bodies(w, id)?;
        if b.is_empty() {
            return Ok(());
        }
        if b.len() != 1 {
            return Err("3.1.3 source single-body selection required".into());
        }
        let roll = |purpose: &str| {
            let mut r = w.rng.derive(Rng::hash_tag(window, id.0, purpose));
            1 + (r.next_f64() * 100.) as u8
        };
        let loss_d100 = roll("3.1.3.towed_loss");
        let foul_d100 = roll("3.1.3.cable_foul");
        Trial {
            tick: w.tick,
            body: b[0].clone(),
            loss_d100,
            foul_d100,
            lost: loss_d100 as u64
                <= tables()["astern"]["towed_body_loss_percent"]
                    .as_u64()
                    .unwrap(),
            fouled: foul_d100 as u64 <= tables()["astern"]["cable_foul_percent"].as_u64().unwrap(),
            applied: false,
        }
    };
    if trial.applied {
        return Ok(());
    }
    // Save original rolls before trying a source critical, so an unavailable
    // platform critical fit cannot silently reroll on resume.
    w.rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap()
        .astern
        .as_mut()
        .unwrap()
        .trials
        .insert(window, trial.clone());
    if trial.fouled {
        let identity = format!("3.1.3.cable:{id:?}:{window}");
        super::super::damage_runtime::astern_cable_engineering(w, id, &identity).map_err(
            |reason| {
                pending(
                    w,
                    id,
                    "3.1.3.engineering_pending",
                    &reason,
                    json!({"trial":trial,"window":window}),
                )
            },
        )?;
    }
    if trial.lost {
        match &trial.body {
            Body::Decoy { object } => {
                if let Some(o) = w.rules.ew.as_mut().and_then(|s| s.objects.get_mut(object)) {
                    o.status = super::super::ew2026::Status::Destroyed;
                }
            }
            Body::Sonar { key, .. } => {
                if let Some(a) = w
                    .rules
                    .detection
                    .as_mut()
                    .and_then(|s| s.arrays.get_mut(key))
                {
                    a.lost = true;
                    a.deployed = false;
                }
            }
        }
    }
    trial.applied = true;
    w.rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap()
        .astern
        .as_mut()
        .unwrap()
        .trials
        .insert(window, trial.clone());
    trace(
        w,
        id,
        "3.1.3.towed_risk",
        json!({"window":window,"tick":trial.tick,"body":trial.body,"loss_d100":trial.loss_d100,"foul_d100":trial.foul_d100,"lost":trial.lost,"fouled":trial.fouled,"independent":true,"source":"English2026 PDF25/3-3 section3.1.3","timing":"RTS first actual reverse displacement per Tactical Turn"}),
    );
    Ok(())
}
/// Return true when this function owns reverse integration, including zero
/// distance. Ahead braking/normal forward movement still use source turning.
pub fn advance(w: &mut World, id: EntityId) -> Result<bool, String> {
    let reverse = w
        .rules
        .navigation
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .and_then(|u| u.astern.as_ref())
        .is_some_and(|s| s.moving_astern);
    if !reverse {
        return Ok(false);
    }
    let e = w.entity(id).ok_or("3.1.3 ship missing")?.clone();
    let distance = e.speed_kts * crate::world::TICK_SECONDS / 3600.;
    if distance <= 1e-12 {
        return Ok(true);
    }
    let next = e.pos.advanced(e.course_deg + 180., distance);
    if w.context
        .terrain
        .as_ref()
        .is_some_and(|t| t.blocked(e.pos, next))
    {
        let now = w.tick;
        let from = {
            let e = w.entity_mut(id).unwrap();
            e.speed_kts = 0.;
            e.ordered_speed_kts = 0.;
            e.station = None;
            let last = e.ext.get("terrain.last_block").copied().unwrap_or(-100000.);
            if now as f64 - last >= 30. {
                e.ext.insert("terrain.last_block".into(), now as f64);
                Some(e.pos)
            } else {
                None
            }
        };
        if let Some(from) = from {
            trace(
                w,
                id,
                "3.1.3.land_stop",
                json!({"from":from,"proposed":next,"source":"public chart swept reverse segment; navigation guard, no invented grounding damage"}),
            );
        }
        return Ok(true);
    }
    w.entity_mut(id).unwrap().pos = next;
    risk(w, id)?;
    Ok(true)
}

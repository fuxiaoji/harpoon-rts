//! §8.5.3.1 source-bound dedicated early attack sonar. It tracks an already
//! selected located contact and cannot search. Chapter6 still computes FCS.
use super::super::{active, Configuration};
use crate::db::{Guidance, SensorKind, WeaponKind};
use crate::rules::detection2026::{Receiver, SensorReport};
use crate::{ContactId, EntityId, Side, Vec2, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    units: BTreeMap<EntityId, BTreeMap<String, Bound>>,
    seeds: BTreeMap<String, Seed>,
}
impl State {
    pub fn is_empty(&self) -> bool {
        self.units.is_empty() && self.seeds.is_empty()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Bound {
    name: String,
    class: String,
    owner: Side,
    sensor: String,
    receiver: Receiver,
    projectors: BTreeSet<String>,
    sources: BTreeSet<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Seed {
    tick: i64,
    contact: ContactId,
    target: EntityId,
    aim: Vec2,
}
fn key(actor: EntityId, sensor: &str) -> String {
    format!("{}:{sensor}", actor.0)
}
fn state(w: &World) -> Option<&State> {
    w.rules
        .asw_weapons
        .as_ref()
        .map(|s| &s.projectors.attack_sonars)
}
fn binding<'a>(w: &'a World, actor: EntityId, sensor: &str) -> Option<&'a Bound> {
    state(w)?.units.get(&actor)?.get(sensor)
}
fn same_receiver(a: &Receiver, b: &Receiver) -> bool {
    serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
}
fn early_receiver(r: &Receiver) -> bool {
    matches!(r,Receiver::Sonar{source,generation:1..=3,active_range_nm:Some(range),..} if !source.trim().is_empty()&&range.is_finite()&&*range>0.)
}
/// Pure validation, before parent publishes any accepted configuration prefix.
pub(super) fn initialize(w: &World, c: &Configuration) -> Result<State, String> {
    let mut s = State::default();
    for (weapon, p) in &c.projectors {
        for (name, u) in &p.units {
            let Some(sensor) = &u.attack_sensor else {
                continue;
            };
            let found: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
            if found.len() != 1
                || sensor.trim().is_empty()
                || u.source.trim().is_empty()
                || p.source.trim().is_empty()
            {
                return Err(
                    "8.5.3.1 exact unique original attack-sonar installation/source required"
                        .into(),
                );
            }
            let e = found[0];
            let d =
                w.db.sensors
                    .get(sensor)
                    .ok_or("8.5.3.1 actual dedicated sensor definition absent")?;
            let receiver = w
                .rules
                .detection
                .as_ref()
                .and_then(|d| d.config.bindings.get(&format!("{}:{sensor}", e.class)))
                .ok_or("8.5.3.1 explicit dedicated receiver hardware absent")?;
            if !e.alive
                || e.kind != crate::types::PlatformKind::Ship
                || d.kind != SensorKind::SonarActive
                || !e.sensors.iter().any(|i| i.sensor == *sensor)
                || !early_receiver(receiver)
                || !e
                    .weapons
                    .get(u.mount_index)
                    .is_some_and(|m| m.weapon == *weapon && m.mounts > 0)
                || !w.db.weapons.get(weapon).is_some_and(|d| {
                    d.kind == WeaponKind::AswRocket && d.guidance == Guidance::AswBallistic
                })
            {
                return Err("8.5.3.1 physically installed active Gen1–3 dedicated sonar and exact original projector-line binding required".into());
            }
            let b = s
                .units
                .entry(e.id)
                .or_default()
                .entry(sensor.clone())
                .or_insert_with(|| Bound {
                    name: name.clone(),
                    class: e.class.clone(),
                    owner: e.side,
                    sensor: sensor.clone(),
                    receiver: receiver.clone(),
                    projectors: BTreeSet::new(),
                    sources: BTreeSet::new(),
                });
            if !same_receiver(&b.receiver, receiver) {
                return Err("8.5.3.1 conflicting source hardware for one dedicated device".into());
            }
            b.projectors.insert(weapon.clone());
            b.sources.insert(u.source.clone());
        }
    }
    Ok(s)
}
fn operational(w: &World, actor: EntityId, b: &Bound) -> bool {
    let Some(e) = w
        .entity(actor)
        .filter(|e| e.alive && e.name == b.name && e.class == b.class && e.side == b.owner)
    else {
        return false;
    };
    e.systems.sonar > 0.
        && e.sensors.iter().any(|i| i.sensor == b.sensor && i.active)
        && w.db
            .sensors
            .get(&b.sensor)
            .is_some_and(|d| d.kind == SensorKind::SonarActive)
        && !crate::sensors::sensor_damaged(e, &b.sensor)
        && super::super::super::expanded_damage2026::sensor_operational(w, actor, &b.sensor)
            .unwrap_or(false)
}
fn selected(w: &World, actor: EntityId) -> Option<(ContactId, EntityId)> {
    let e = w.entity(actor)?;
    let cid = e.assigned?;
    let c = w.sides[e.side.index()]
        .contacts
        .get(&cid)
        .filter(|c| !c.stale && !c.bearing_only)?;
    Some((cid, c.about))
}
/// Called once immediately before the ordinary detection runtime is taken out
/// of World. Existing native/delivered qualification needs that complete World.
pub fn prepare_detection(w: &mut World) {
    if !active(w) {
        return;
    }
    let Some(s) = state(w) else {
        return;
    };
    let roles: Vec<_> = s
        .units
        .iter()
        .flat_map(|(actor, m)| m.values().map(|b| (*actor, b.clone())))
        .collect();
    if roles.is_empty() {
        return;
    }
    let mut seeds = BTreeMap::new();
    for (actor, b) in roles {
        if !operational(w, actor, &b) {
            continue;
        }
        let Some((contact, target)) = selected(w, actor) else {
            continue;
        };
        let Ok(Some(aim)) = super::super::depth_charge::located_aim(w, actor, target) else {
            continue;
        };
        if !aim.x.is_finite() || !aim.y.is_finite() {
            continue;
        }
        seeds.insert(
            key(actor, &b.sensor),
            Seed {
                tick: w.tick,
                contact,
                target,
                aim,
            },
        );
    }
    w.rules
        .asw_weapons
        .as_mut()
        .unwrap()
        .projectors
        .attack_sonars
        .seeds = seeds;
}
/// Ordinary search receivers pass unchanged. A dedicated device never scans a
/// new target, even when the global scanner iterates that hidden enemy body.
pub fn scan_gate(
    w: &World,
    actor: EntityId,
    sensor: &str,
    target: EntityId,
    receiver: &Receiver,
) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let Some(b) = binding(w, actor, sensor) else {
        return Ok(());
    };
    if !operational(w, actor, b)
        || !early_receiver(receiver)
        || !same_receiver(&b.receiver, receiver)
    {
        return Err(
            "8.5.3.1 original active dedicated attack-sonar hardware unavailable/changed".into(),
        );
    }
    let Some(seed) = state(w).and_then(|s| s.seeds.get(&key(actor, sensor))) else {
        return Err("8.5.3.1 dedicated attack sonar cannot search; actual selected located-source seed required".into());
    };
    if seed.tick != w.tick
        || seed.target != target
        || selected(w, actor) != Some((seed.contact, target))
    {
        return Err("8.5.3.1 dedicated sonar tracks only its original currently selected source-located contact".into());
    }
    Ok(())
}
/// This is an independent early dedicated-device exception, never a generic
/// relaxation of the Gen4+ search-receiver projector qualification.
pub fn fcs_allowed(
    w: &World,
    actor: EntityId,
    target: EntityId,
    sensor: &str,
    report: &SensorReport,
) -> bool {
    if !active(w) {
        return false;
    }
    let Some(b) = binding(w, actor, sensor) else {
        return false;
    };
    let fresh = super::super::super::firecontrol2026::tables()["execution_revision"]["own_motion"]
        ["report_freshness_s"]
        .as_i64()
        .unwrap();
    operational(w, actor, b)
        && report.at <= w.tick
        && w.tick - report.at <= fresh
        && selected(w, actor).is_some_and(|(_, t)| t == target)
        && report.observer == actor
        && report.target == target
        && report.sensor == sensor
        && report.sensor_kind == SensorKind::SonarActive
        && report.bearing_alternatives.is_empty()
        && report
            .measured_position
            .is_some_and(|p| p.x.is_finite() && p.y.is_finite())
        && report
            .receiver
            .as_ref()
            .is_some_and(|r| early_receiver(r) && same_receiver(&b.receiver, r))
}
/// A dedicated sensor listed for another projector line cannot become an
/// unprinted shared early director. Normal Gen4+ search devices are unchanged.
pub(super) fn weapon_matches(w: &World, actor: EntityId, weapon: &str, sensor: &str) -> bool {
    binding(w, actor, sensor).is_none_or(|b| b.projectors.contains(weapon))
}
pub(super) fn own_view(w: &World, actor: EntityId, weapon: &str) -> Value {
    let Some(m) = state(w).and_then(|s| s.units.get(&actor)) else {
        return Value::Null;
    };
    let own:Vec<_>=m.values().filter(|b|b.projectors.contains(weapon)&&w.entity(actor).is_some_and(|e|e.side==b.owner&&e.class==b.class&&e.name==b.name)).map(|b|json!({"sensor":b.sensor,"source":b.sources,"receiver":b.receiver,"active_and_operational":operational(w,actor,b),"can_search":false,"selected_contact":selected(w,actor).map(|(cid,_)|cid)})).collect();
    if own.is_empty() {
        Value::Null
    } else {
        json!(own)
    }
}

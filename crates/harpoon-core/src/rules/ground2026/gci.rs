//! Explicit GCI radio assignments. Vectors settle after committed damage.
use super::{Action, State as GroundState};
use crate::types::PlatformKind;
use crate::{Command, EntityId, EventKind, Side, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceConfig {
    pub source: String,
    pub controller: String,
    pub command_bunker: String,
    pub fit: String,
    pub aircraft: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Service {
    pub config: ServiceConfig,
    pub owner: Side,
    pub controller: EntityId,
    pub bunker: EntityId,
    pub aircraft: BTreeSet<EntityId>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub controller: EntityId,
    pub service: String,
    pub aircraft: EntityId,
    pub course_deg: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub services: BTreeMap<String, Service>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requests: Vec<Request>,
}
pub(super) fn initialize(
    w: &World,
    ground: &GroundState,
    configs: &BTreeMap<String, ServiceConfig>,
) -> Result<Option<State>, String> {
    if configs.is_empty() {
        return Ok(None);
    }
    let mut state = State::default();
    for (key, config) in configs {
        if key.trim().is_empty()
            || config.source.trim().is_empty()
            || config.fit.trim().is_empty()
            || config.aircraft.is_empty()
        {
            return Err(
                "11.GCI requires explicit sourced controller/bunker/radio/aircraft assignments"
                    .into(),
            );
        }
        let controller = super::find_named(w, &config.controller)?;
        let bunker = super::find_named(w, &config.command_bunker)?;
        let owner = w
            .entity(controller)
            .ok_or("11.GCI controller missing")?
            .side;
        if ![controller, bunker].iter().all(|id| {
            w.entity(*id)
                .is_some_and(|e| e.alive && e.side == owner && e.kind == PlatformKind::Base)
        }) || !ground
            .facilities
            .get(&bunker)
            .is_some_and(|f| f.target.effect == "no_gci")
        {
            return Err("11.GCI actual own controller and exact Command Bunker required".into());
        }
        let mut aircraft = BTreeSet::new();
        for name in &config.aircraft {
            let id = super::find_named(w, name)?;
            if !aircraft.insert(id)
                || !w
                    .entity(id)
                    .is_some_and(|e| e.alive && e.side == owner && e.kind == PlatformKind::Aircraft)
            {
                return Err("11.GCI recipients must be unique actual friendly aircraft".into());
            }
        }
        // Initialization validates the real transport too; no guessed range or
        // shared Contact can substitute for a fitted operational radio.
        super::super::communications2026::validate_transport(
            w,
            owner,
            controller,
            &config.fit,
            &aircraft.iter().copied().collect::<Vec<_>>(),
        )?;
        state.services.insert(
            key.clone(),
            Service {
                config: config.clone(),
                owner,
                controller,
                bunker,
                aircraft,
            },
        );
    }
    Ok(Some(state))
}
fn live(
    w: &World,
    s: &Service,
    controller: EntityId,
    aircraft: EntityId,
    course: f64,
) -> Result<(), String> {
    if !course.is_finite() || s.controller != controller || !s.aircraft.contains(&aircraft) {
        return Err("11.GCI finite course and exact assigned controller/aircraft required".into());
    }
    for (id, name, kind) in [
        (controller, s.config.controller.as_str(), PlatformKind::Base),
        (
            s.bunker,
            s.config.command_bunker.as_str(),
            PlatformKind::Base,
        ),
    ] {
        if super::find_named(w, name)? != id
            || !w
                .entity(id)
                .is_some_and(|e| e.alive && e.hp > 0. && e.side == s.owner && e.kind == kind)
        {
            return Err("11.GCI source identity or ownership unavailable".into());
        }
    }
    if !super::operational(w, controller)
        || !w
            .rules
            .ground
            .as_ref()
            .and_then(|g| g.facilities.get(&s.bunker))
            .is_some_and(|f| f.target.effect == "no_gci" && !f.disabled && !f.destroyed)
    {
        return Err("11.1 Command Bunker casualty prevents GCI control".into());
    }
    if !w.entity(aircraft).is_some_and(|e| {
        e.alive
            && e.side == s.owner
            && e.kind == PlatformKind::Aircraft
            && e.ext.get("control.manual") != Some(&1.0)
    }) {
        return Err("11.GCI recipient unavailable or explicitly under manual control".into());
    }
    super::super::unmanned2026::can_control(w, s.owner, aircraft)?;
    super::super::communications2026::validate_transport(
        w,
        s.owner,
        controller,
        &s.config.fit,
        &[aircraft],
    )?;
    Ok(())
}
pub fn validate_action(w: &World, controller: EntityId, action: &Action) -> Result<(), String> {
    let Action::GciVector {
        service,
        aircraft,
        course_deg,
    } = action
    else {
        return Err("11.GCI vector action required".into());
    };
    let s = w
        .rules
        .ground
        .as_ref()
        .and_then(|g| g.gci.as_ref())
        .and_then(|g| g.services.get(service))
        .ok_or("11.GCI exact service not configured")?;
    live(w, s, controller, *aircraft, *course_deg)
}
pub fn apply(w: &mut World, controller: EntityId, action: &Action) -> Result<(), String> {
    validate_action(w, controller, action)?;
    let Action::GciVector {
        service,
        aircraft,
        course_deg,
    } = action
    else {
        unreachable!()
    };
    w.rules
        .ground
        .as_mut()
        .unwrap()
        .gci
        .as_mut()
        .unwrap()
        .requests
        .push(Request {
            controller,
            service: service.clone(),
            aircraft: *aircraft,
            course_deg: *course_deg,
        });
    Ok(())
}
pub fn step(w: &mut World) {
    let Some(mut state) = w.rules.ground.as_mut().and_then(|g| g.gci.take()) else {
        return;
    };
    let mut requests: BTreeMap<EntityId, Vec<Request>> = BTreeMap::new();
    for r in std::mem::take(&mut state.requests) {
        requests.entry(r.aircraft).or_default().push(r);
    }
    for (aircraft, mut rs) in requests {
        rs.sort_by(|a, b| {
            a.controller
                .cmp(&b.controller)
                .then_with(|| a.service.cmp(&b.service))
                .then_with(|| a.course_deg.total_cmp(&b.course_deg))
        });
        let distinct: BTreeSet<_> = rs
            .iter()
            .map(|r| {
                (
                    r.controller,
                    r.service.clone(),
                    crate::geom::normalize_deg(r.course_deg).to_bits(),
                )
            })
            .collect();
        let r = &rs[0];
        let Some(s) = state.services.get(&r.service) else {
            continue;
        };
        let result = if distinct.len() != 1 {
            Err("conflicting simultaneous GCI vectors; no client-order priority".into())
        } else {
            live(w, s, r.controller, aircraft, r.course_deg)
        };
        if result.is_ok() {
            crate::sim::deliver_gci_order(
                w,
                s.owner,
                &Command::SetCourse {
                    unit: aircraft,
                    course_deg: r.course_deg,
                },
            );
        }
        w.emit(EventKind::RulesEvent{domain:"ground".into(),rule:if result.is_ok(){"11.GCI.vector_delivered"}else{"11.GCI.vector_rejected"}.into(),side:Some(s.owner),unit:Some(r.controller),contact:None,detail:json!({"service":r.service,"aircraft":aircraft,"course_deg":r.course_deg,"reason":result.err(),"source":s.config.source,"phase":"post_committed_damage","mapping":"RTS explicit single course order through normal navigation"}).to_string()},Visibility::Side(s.owner));
    }
    w.rules.ground.as_mut().unwrap().gci = Some(state);
}
pub(super) fn own_view(w: &World, unit: EntityId) -> Option<Value> {
    let side = w.entity(unit)?.side;
    let state = w.rules.ground.as_ref()?.gci.as_ref()?;
    let services:BTreeMap<_,_>=state.services.iter().filter(|(_,s)|s.owner==side && s.controller==unit).map(|(key,s)|{
        let aircraft:Vec<_>=s.aircraft.iter().copied().filter(|id|w.entity(*id).is_some_and(|e|e.side==side)).collect();
        (key,json!({"command_bunker":s.bunker,"aircraft":aircraft,"fit":s.config.fit,"source":s.config.source,"available":super::operational(w,s.bunker)}))
    }).collect();
    (!services.is_empty()).then(|| json!({"services":services}))
}

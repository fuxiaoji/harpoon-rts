//! Source3.2.1 opt-in depth bands and separate180s surface operations.
use super::{configured, tables, trace};
use crate::{EntityId, World};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fit {
    pub source: String,
    pub maximum_depth_m: f64,
    pub surface_max_kts: f64,
    pub water_depth_m: f64,
    pub very_small: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Bands,
    EmergencyBlow,
    Surface,
    Submerge,
    PeriscopeShift,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operation {
    pub kind: Kind,
    pub target_m: f64,
    pub started_at: i64,
    pub worked_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blow_origin_band: Option<f64>,
}
fn fit(w: &World, id: EntityId) -> Option<&Fit> {
    if !configured(w, id) {
        return None;
    }
    w.rules
        .navigation
        .as_ref()?
        .units
        .get(&id)?
        .fit
        .submarine
        .as_ref()
}
pub fn configured_depth(w: &World, id: EntityId) -> bool {
    fit(w, id).is_some()
}
pub fn emergency_blowing(w: &World, id: EntityId) -> bool {
    configured_depth(w, id)
        && w.rules.navigation.as_ref().unwrap().units[&id]
            .depth_operation
            .as_ref()
            .is_some_and(|o| o.kind == Kind::EmergencyBlow)
}
fn minimum_blow_bands() -> f64 {
    tables()["submarine"]["emergency_blow"]["minimum_upward_bands"]
        .as_f64()
        .unwrap()
}
fn blow_committed(w: &World, id: EntityId, depth: f64) -> bool {
    w.rules.navigation.as_ref().unwrap().units[&id]
        .depth_operation
        .as_ref()
        .is_some_and(|o| {
            o.kind == Kind::EmergencyBlow
                && o.blow_origin_band
                    .is_none_or(|origin| origin - coordinate(depth) < minimum_blow_bands() - 1e-9)
        })
}
/// Read-only source admission, also consumed before HTTP/MCP/AI queue mutation.
pub fn validate_emergency_blow(w: &World, id: EntityId, target: f64) -> Result<(), String> {
    if super::hazards::held(w, id) {
        return Err("3.3/3.4 physical contact or grounding must be freed before depth movement".into());
    }
    let f = fit(w, id).ok_or("3.2.1 source submarine depth fit absent")?;
    let e = w.entity(id).ok_or("3.2.1 submarine unavailable")?;
    validate_fit(w, e, f)?;
    if !e.alive {
        return Err("3.2.1 submarine unavailable".into());
    }
    let periscope = tables()["submarine"]["periscope_max_m"].as_f64().unwrap();
    if !target.is_finite()
        || target <= periscope
        || target >= e.depth_m
        || target > f.maximum_depth_m
        || target > f.water_depth_m
    {
        return Err("3.2.1 Blow must ascend within verified depth bands to Shallow or deeper; P/S and surface are separate operations".into());
    }
    blow_bands(e.speed_kts)?;
    let unit = &w.rules.navigation.as_ref().unwrap().units[&id];
    if unit
        .depth_operation
        .as_ref()
        .is_some_and(|o| o.kind == Kind::EmergencyBlow && o.target_m == target)
    {
        return Ok(());
    }
    if blow_committed(w, id, e.depth_m)
        || coordinate(e.depth_m) - coordinate(target) < minimum_blow_bands() - 1e-9
    {
        return Err("3.2.1 Emergency Blow must complete at least three source depth bands".into());
    }
    if unit.heave_to
        || unit.coasting
        || super::astern::engaged(w, id)
        || super::crashback::active(w, id)
    {
        return Err("3.2.1 end incompatible navigation maneuver before Emergency Blow; combined procedure pending".into());
    }
    Ok(())
}
pub fn order_emergency_blow(w: &mut World, id: EntityId, target: f64) -> Result<(), String> {
    validate_emergency_blow(w, id, target)?;
    let e = w.entity(id).unwrap().clone();
    let u = w
        .rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap();
    if u.depth_operation
        .as_ref()
        .is_some_and(|o| o.kind == Kind::EmergencyBlow && o.target_m == target)
    {
        return Ok(());
    }
    let operation = Operation {
        kind: Kind::EmergencyBlow,
        target_m: target,
        started_at: w.tick,
        worked_seconds: 0.,
        blow_origin_band: Some(coordinate(e.depth_m)),
    };
    // Declaring a mandatory Loud blow also ends the quiet acceleration mode.
    // This happens only after successful admission, never on a rejected order.
    u.quiet = false;
    u.depth_operation = Some(operation.clone());
    w.entity_mut(id).unwrap().ordered_depth_m = target;
    trace(
        w,
        id,
        "3.2.1.emergency_blow_started",
        json!({"operation":operation,
        "from_m":e.depth_m,"source":tables()["submarine"]["emergency_blow"]}),
    );
    Ok(())
}
pub fn validate_fit(w: &World, e: &crate::world::Entity, f: &Fit) -> Result<(), String> {
    let positive = [f.maximum_depth_m, f.surface_max_kts, f.water_depth_m];
    if e.kind != crate::types::PlatformKind::Sub
        || f.source.trim().is_empty()
        || positive.iter().any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err(
            "3.2.1 explicit sourced submarine depth/surface-speed/water fit required".into(),
        );
    }
    let db = e.class_def(&w.db).ok_or("3.2.1 missing submarine class")?;
    if f.maximum_depth_m > super::super::submarine_annex::maximum_depth_m(w, e)
        || f.maximum_depth_m
            > tables()["depth_bands_max_m"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()
                .as_f64()
                .unwrap()
        || f.surface_max_kts > db.max_speed_kts
        || super::super::submarine_annex::surface_max_kts(w, e)
            .is_some_and(|maximum| f.surface_max_kts > maximum)
    {
        return Err(
            "3.2.1 depth fit exceeds database/source capability; Very Deep separately pending"
                .into(),
        );
    }
    if e.depth_m > f.maximum_depth_m
        || e.depth_m > f.water_depth_m
        || (e.depth_m > 0. && f.water_depth_m < minimum_water(f))
    {
        return Err("3.2.1 initial depth incompatible with declared hull/water depth".into());
    }
    if e.depth_m == 0. && e.speed_kts > f.surface_max_kts {
        return Err("3.2.1 initial surfaced speed exceeds explicit surface capability".into());
    }
    Ok(())
}
fn minimum_water(f: &Fit) -> f64 {
    tables()["submarine"][if f.very_small {
        "vsmall_minimum_water_m"
    } else {
        "minimum_water_m"
    }]
    .as_f64()
    .unwrap()
}
pub fn surface_speed_limit(w: &World, id: EntityId) -> Option<f64> {
    let f = fit(w, id)?;
    let e = w.entity(id)?;
    (e.depth_m == 0.).then_some(f.surface_max_kts)
}
/// Shared read-only admission for fitted procedures and exact static Annex caps.
/// Static capability does not supply a water depth or a source movement clock.
pub fn validate_depth_declaration(w: &World, id: EntityId, target: f64) -> Result<(), String> {
    if super::hazards::held(w, id) {
        return Err("3.3/3.4 physical contact or grounding must be freed before depth movement".into());
    }
    if configured_depth(w, id) {
        return validate_order_depth(w, id, target);
    }
    let Some(e) = w
        .entity(id)
        .filter(|e| e.kind == crate::types::PlatformKind::Sub)
    else {
        return Ok(());
    };
    let Some(maximum) = super::super::submarine_annex::verified_maximum_depth_m(w, &e.class) else {
        return Ok(());
    };
    if !target.is_finite() || target < 0. || target > maximum {
        return Err("3.2.1 requested depth outside verified Annex game band; source band is not a measured engineering hull limit".into());
    }
    Ok(())
}
fn depth_kind(w: &World, id: EntityId, target: f64) -> Result<Kind, String> {
    if super::hazards::held(w, id) {
        return Err("3.3/3.4 physical contact or grounding must be freed before depth movement".into());
    }
    let f = fit(w, id).ok_or("3.2.1 source depth fit absent")?.clone();
    let e = w.entity(id).ok_or("3.2.1 submarine unavailable")?.clone();
    validate_fit(w, &e, &f)?;
    if !e.alive {
        return Err("3.2.1 submarine unavailable".into());
    }
    if !target.is_finite() || target < 0. || target > f.maximum_depth_m || target > f.water_depth_m
    {
        return Err("3.2.1 requested depth outside verified hull/water capability".into());
    }
    if target > 0. && f.water_depth_m < minimum_water(&f) {
        return Err("3.2.1 water too shallow to submerge this source size".into());
    }
    if blow_committed(w, id, e.depth_m) {
        return Err("3.2.1 Emergency Blow three-band commitment not yet completed".into());
    }
    if (e.depth_m - target).abs() < 1e-9 {
        return Ok(Kind::Bands);
    }
    let shallow = tables()["submarine"]["shallow_max_m"].as_f64().unwrap();
    let periscope = tables()["submarine"]["periscope_max_m"].as_f64().unwrap();
    let kind = if target == 0. {
        if e.depth_m > shallow {
            return Err(
                "3.2.1 reach Shallow/P-S before ordering separate surface operation".into(),
            );
        }
        Kind::Surface
    } else if e.depth_m == 0. {
        if target > shallow {
            return Err(
                "3.2.1 submerge to P-S or Shallow first; crash dive needs separate verified fit"
                    .into(),
            );
        }
        Kind::Submerge
    } else if (e.depth_m <= periscope) != (target <= periscope) {
        if e.depth_m > shallow || target > shallow {
            return Err(
                "3.2.1 P-S is separate one-turn shift from Shallow; reach Shallow first".into(),
            );
        }
        Kind::PeriscopeShift
    } else {
        Kind::Bands
    };
    Ok(kind)
}
pub fn validate_order_depth(w: &World, id: EntityId, target: f64) -> Result<(), String> {
    depth_kind(w, id, target).map(|_| ())
}
pub fn order_depth(w: &mut World, id: EntityId, target: f64) -> Result<(), String> {
    let kind = depth_kind(w, id, target)?;
    let f = fit(w, id).unwrap().clone();
    let e = w.entity(id).unwrap().clone();
    let u = &w.rules.navigation.as_ref().unwrap().units[&id];
    if u.depth_operation
        .as_ref()
        .is_some_and(|o| o.target_m == target && o.kind == kind)
    {
        return Ok(());
    }
    if (e.depth_m - target).abs() < 1e-9 {
        w.rules
            .navigation
            .as_mut()
            .unwrap()
            .units
            .get_mut(&id)
            .unwrap()
            .depth_operation = None;
        w.entity_mut(id).unwrap().ordered_depth_m = target;
        return Ok(());
    }
    let operation = Operation {
        kind,
        target_m: target,
        started_at: w.tick,
        worked_seconds: 0.,
        blow_origin_band: None,
    };
    w.rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap()
        .depth_operation = Some(operation.clone());
    w.entity_mut(id).unwrap().ordered_depth_m = target;
    trace(
        w,
        id,
        "3.2.1.depth_started",
        json!({"source":f.source,"operation":operation,"from_m":e.depth_m,"turn_seconds":tables()["turn_seconds"],"policy":tables()["submarine"]["continuous_policy"]}),
    );
    Ok(())
}
fn coordinate(depth: f64) -> f64 {
    let upper = tables()["depth_bands_max_m"].as_array().unwrap();
    let first = upper[0].as_f64().unwrap();
    if depth <= first {
        return (depth - first) / first;
    }
    for i in 1..upper.len() {
        let hi = upper[i].as_f64().unwrap();
        let lo = upper[i - 1].as_f64().unwrap();
        if depth <= hi {
            return (i - 1) as f64 + (depth - lo) / (hi - lo);
        }
    }
    unreachable!("verified source max depth<=1200")
}
fn depth_at(position: f64) -> f64 {
    let upper = tables()["depth_bands_max_m"].as_array().unwrap();
    let first = upper[0].as_f64().unwrap();
    if position < 0. {
        return first * (position + 1.);
    }
    let index = (position.floor() as usize).min(upper.len() - 2);
    let lo = upper[index].as_f64().unwrap();
    let hi = upper[index + 1].as_f64().unwrap();
    lo + (hi - lo) * (position - index as f64)
}
fn bands(speed: f64) -> Result<f64, String> {
    if !speed.is_finite() || speed <= 0. {
        return Err(
            "3.2.1 normal depth change has no printed zero-speed rate; adjudication required"
                .into(),
        );
    }
    for row in tables()["submarine_depth_change_bands_per_turn"]
        .as_array()
        .unwrap()
    {
        if row[0].as_f64().is_none_or(|max| speed <= max) {
            return Ok(row[1].as_f64().unwrap());
        }
    }
    unreachable!()
}
fn blow_bands(speed: f64) -> Result<f64, String> {
    bands(speed)?;
    let rows = tables()["submarine_depth_change_bands_per_turn"]
        .as_array()
        .unwrap();
    let index = rows
        .iter()
        .position(|row| row[0].as_f64().is_none_or(|max| speed <= max))
        .unwrap();
    Ok(rows[(index + 1).min(rows.len() - 1)][1].as_f64().unwrap())
}
pub fn advance(w: &mut World, id: EntityId) -> Result<(), String> {
    if !configured_depth(w, id) {
        return Ok(());
    }
    let e = w.entity(id).ok_or("3.2.1 submarine missing")?.clone();
    if e.ext
        .get("damage.broach_until")
        .is_some_and(|until| *until > w.tick as f64)
    {
        w.entity_mut(id).unwrap().depth_m = 0.;
        w.rules
            .navigation
            .as_mut()
            .unwrap()
            .units
            .get_mut(&id)
            .unwrap()
            .depth_operation = None;
        return Ok(());
    }
    let mut target = e.ordered_depth_m;
    if let Some(forced) = super::super::damage_runtime::forced_depth(&e) {
        let shallow = tables()["submarine"]["shallow_max_m"].as_f64().unwrap();
        target = target.min(if forced == 0. && e.depth_m > shallow {
            shallow
        } else {
            forced
        });
    }
    let op = w.rules.navigation.as_ref().unwrap().units[&id]
        .depth_operation
        .clone();
    // Complete the committed blow before separately processing damage-forced
    // shallower orders. A hard broach above remains an immediate casualty.
    if op
        .as_ref()
        .is_none_or(|o| o.kind != Kind::EmergencyBlow && o.target_m != target)
        && (target - e.depth_m).abs() > 1e-9
    {
        if let Err(reason) = order_depth(w, id, target) {
            // A standing/autopilot order uses the same public fit validation.
            // Invalid metre orders do not silently fall through to legacy3m/s.
            w.entity_mut(id).unwrap().ordered_depth_m = e.depth_m;
            return Err(reason);
        }
    }
    let Some(mut op) = w.rules.navigation.as_ref().unwrap().units[&id]
        .depth_operation
        .clone()
    else {
        return Ok(());
    };
    let turn = if matches!(op.kind, Kind::Bands | Kind::EmergencyBlow) {
        tables()["turn_seconds"].as_f64().unwrap()
    } else {
        tables()["submarine"]["separate_operation_seconds"]
            .as_f64()
            .unwrap()
    };
    op.worked_seconds += crate::world::TICK_SECONDS;
    let next = if matches!(op.kind, Kind::Bands | Kind::EmergencyBlow) {
        let rate = if op.kind == Kind::EmergencyBlow {
            blow_bands(e.speed_kts)?
        } else {
            bands(e.speed_kts)?
        } / turn;
        let from = coordinate(e.depth_m);
        let to = coordinate(op.target_m);
        if (to - from).abs() <= rate + 1e-12 {
            op.target_m
        } else {
            depth_at(from + (to - from).signum() * rate)
        }
    } else if op.worked_seconds + 1e-9 >= turn {
        op.target_m
    } else {
        e.depth_m
    };
    w.entity_mut(id).unwrap().depth_m = next;
    let done = (next - op.target_m).abs() < 1e-8;
    w.rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap()
        .depth_operation = if done { None } else { Some(op.clone()) };
    if done {
        trace(
            w,
            id,
            "3.2.1.depth_complete",
            json!({"operation":op,"depth_m":next,"tick":w.tick}),
        );
    }
    Ok(())
}

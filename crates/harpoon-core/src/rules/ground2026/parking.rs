//! §11.5.1 taxiway-cut parking routing as an explicit author declaration.
//!
//! Printed basis (English2026 PDF118/11-3 "Generic Land Targets", Taxiways
//! row, personally viewed 2026-10-30): Size C, no damage points, armor 0,
//! "Cut isolates aircraft from runways". The printed rule is the isolation
//! FUNCTION only — no duration, area geometry or probability is printed —
//! so which parking area serves which base/class is declared by the scenario
//! author here. This is a deployment declaration for a wargame scenario,
//! never a real-world site claim or per-airframe tracking; dynamic aircraft
//! relocation is out of scope for this leaf.
//!
//! Fixed ABI the root wires (this file does not touch shared files):
//! 1. `rules/ground2026.rs`: `pub mod parking;` plus the root-owned
//!    `parking: Option<parking::Configuration>` field on Configuration,
//!    read through `w.context.ground_2026`.
//! 2. ground2026::initialize calls `parking::initialize` last (this file
//!    requires the ground state's facilities/air_services to be present).
//! 3. ground2026::queue_launch calls `gate` after its existing
//!    runway/legacy parking gates; ground2026::step calls `gate` again
//!    before the real complete_queued_launch so a later cut keeps the job
//!    queued without a second stock debit; recovery_ready calls `gate`
//!    before alignment, lane occupation or a recovery reservation.
//! 4. Without a parking configuration — or for a base with no declared
//!    routes — the legacy behavior is preserved exactly (gate returns Ok).
//!
//! The gate is a pure read-only precheck: it re-runs the key integrity
//! checks of initialize against the current World, so a restored save
//! passes through the same qualification without mutating anything.

use crate::types::PlatformKind;
use crate::{EntityId, World};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/facility-parking-2026.json"
        ))
        .expect("data/rules/facility-parking-2026.json must parse")
    })
}

/// No default: the author must explicitly declare that the route covers
/// every individual airframe of this class staged at this base.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    AllAirframesOfClass,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub source: String,
    pub base: String,
    pub class: String,
    pub parking_area: String,
    pub scope: Scope,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub routes: Vec<Route>,
}

fn named(w: &World, name: &str) -> Result<EntityId, String> {
    let matches: Vec<_> = w.entities.values().filter(|e| e.name == name).collect();
    if matches.len() != 1 {
        Err(format!("11.5.1 parking route unit name must be unique: {name}"))
    } else {
        Ok(matches[0].id)
    }
}

/// The source table only grounds the printed isolation function; the
/// route declarations themselves are author scenario data.
fn source_table_guard() -> Result<(), String> {
    let row = tables()
        .get("taxiway_isolation")
        .and_then(|t| t.get("printed_row"))
        .ok_or("11.5.1 facility-parking source table is missing the printed Taxiways row")?;
    if row["result"].as_str() != Some("Cut isolates aircraft from runways") {
        return Err(
            "11.5.1 facility-parking source table does not match the viewed PDF118/11-3 Taxiways row"
                .into(),
        );
    }
    Ok(())
}

fn validate_route(w: &World, route: &Route) -> Result<(), String> {
    if !route.source.starts_with("scenario_assumption:") {
        return Err(format!(
            "11.5.1 parking route {} -> {} requires an explicit scenario_assumption source",
            route.base, route.parking_area
        ));
    }
    match route.scope {
        Scope::AllAirframesOfClass => {}
    }
    let base = named(w, &route.base)?;
    let base_e = w.entity(base).ok_or("11.5.1 routed base missing")?;
    let area = named(w, &route.parking_area)?;
    let area_e = w.entity(area).ok_or("11.5.1 routed parking area missing")?;
    if area_e.side != base_e.side {
        return Err(format!(
            "11.5.1 parking route {} -> {} must not cross sides",
            route.base, route.parking_area
        ));
    }
    let ground = w
        .rules
        .ground
        .as_ref()
        .ok_or("11.5.1 parking routes require the chapter-11 ground state")?;
    if !ground.air_services.contains_key(&base) {
        return Err(format!(
            "11.5.1 parking route base {} must be a declared land airfield service",
            route.base
        ));
    }
    if ground.facilities.get(&area).and_then(|f| f.parent) != Some(base) {
        return Err(format!(
            "11.5.1 parking area {} must be an explicitly configured child facility of base {}",
            route.parking_area, route.base
        ));
    }
    for (id,f) in &ground.facilities {
        if f.config.taxiway.as_ref().is_some_and(|t|t.isolated_parking.contains(&route.parking_area))
            && (f.parent != Some(base) || w.entity(*id).is_none_or(|e|e.side != base_e.side)) {
            return Err("11.5.1 taxiway isolation must belong to the routed parking area's own base and side".into());
        }
    }
    let class = w
        .db
        .platforms
        .get(route.class.as_str())
        .ok_or_else(|| format!("11.5.1 parking route class {} is unknown", route.class))?;
    if class.kind != PlatformKind::Aircraft {
        return Err(format!(
            "11.5.1 parking route class {} is not an aircraft",
            route.class
        ));
    }
    Ok(())
}

pub fn initialize(w: &World) -> Result<(), String> {
    let Some(config) = w
        .context
        .ground_2026
        .as_ref()
        .and_then(|c| c.parking.as_ref())
    else {
        return Ok(());
    };
    if !config.source.starts_with("scenario_assumption:") {
        return Err(
            "11.5.1 parking route configuration requires an explicit scenario_assumption source"
                .into(),
        );
    }
    source_table_guard()?;
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for route in &config.routes {
        if !seen.insert((route.base.as_str(), route.class.as_str())) {
            return Err(format!(
                "11.5.1 duplicate parking route for base {} class {}",
                route.base, route.class
            ));
        }
        validate_route(w, route)?;
    }
    Ok(())
}

/// Read-only precheck for the shared launch/recovery paths. No configuration
/// or no declared route for this base preserves the legacy behavior; a base
/// with routes but no route for this class stays pending instead of
/// presuming through-use; the matched route must still qualify against the
/// current World (restore-safe), and only a taxiway of the same base — same
/// parent, same side — may isolate the routed area. The legacy global
/// `parking_gate` stays the single consumer of the existing disabled
/// taxiway/isolated_parking state.
pub fn gate(w: &World, base: EntityId, class: &str) -> Result<(), String> {
    let Some(config) = w
        .context
        .ground_2026
        .as_ref()
        .and_then(|c| c.parking.as_ref())
    else {
        return Ok(());
    };
    if config.routes.is_empty() {
        return Ok(());
    }
    let mut has_base_route = false;
    let mut matched: Option<&Route> = None;
    for route in &config.routes {
        if named(w, &route.base)? != base {
            continue;
        }
        has_base_route = true;
        if route.class == class {
            matched = Some(route);
        }
    }
    let Some(route) = matched else {
        return if has_base_route {
            Err(format!(
                "11.5.1 no parking route declared for class {class} at this base; through-use is not presumed (pending author route)"
            ))
        } else {
            Ok(())
        };
    };
    validate_route(w, route)?;
    let base_e = w.entity(base).ok_or("11.5.1 routed base missing")?;
    let ground = w
        .rules
        .ground
        .as_ref()
        .ok_or("11.5.1 parking routes require the chapter-11 ground state")?;
    for (facility, f) in &ground.facilities {
        if !f.disabled {
            continue;
        }
        let Some(twy) = f.config.taxiway.as_ref() else {
            continue;
        };
        if !twy.isolated_parking.iter().any(|n| n == &route.parking_area) {
            continue;
        }
        let same_parent = f.parent == Some(base);
        let same_side = w
            .entity(*facility)
            .is_some_and(|e| e.side == base_e.side);
        if same_parent && same_side {
            return Err(
                "11.5.1 taxiway cut isolates the routed parking area from its base runways".into(),
            );
        }
    }
    // The scoped check above consumes the same physical cut state. Do not
    // fall back to the legacy unscoped name search after verifying its owner.
    Ok(())
}

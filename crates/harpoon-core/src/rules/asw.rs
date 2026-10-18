//! ASW depth: sound-layer propagation, towed-array state, sonobuoys, MAD,
//! diesel-battery endurance.
//!
//! Hooks:
//! - `propagation_modifier(observer, target)` — multiplies sonar effective
//!   ranges inside the sensor pass (layer physics per §5.4).
//! - `step(world)` — battery drain/regen, buoy activation, passive cross-fix
//!   bookkeeping.

use crate::world::{Entity, World};

/// Extra DB (sonobuoy class, MAD sensor) — filled by the ASW workstream.
pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

/// Sonar propagation multiplier applied to sonar effective range.
pub fn propagation_modifier(_observer: &Entity, _target: &Entity) -> f64 {
    1.0
}

/// Per-tick ASW stateful behavior.
pub fn step(_world: &mut World) {}

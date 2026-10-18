//! Damage closure: damage-control repair over time, armor, extended critical
//! rows (bridge / rudder / CIC).
//!
//! Hook: `step(world)` advances repair progress stored on `entity.ext`
//! ("dc_progress", "dc_target_system") and applies partial system
//! restoration; armor is consulted via `armor_factor`.
//!
//! NOTE: this workstream also owns `crate::damage` (critical tables) and may
//! extend roll_critical there; keep the surface of this module minimal.

use crate::world::{Entity, World};

/// Extra DB (armor values, DC crew ratings) — filled by the damage workstream.
pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

/// Armor mitigation multiplier for a hit by `weapon` on `target`
/// (1.0 = no armor data; guns are mitigated, torpedoes less so).
pub fn armor_factor(_target: &Entity, _weapon: &crate::db::WeaponDef) -> f64 {
    1.0
}

/// Per-tick damage-control behavior.
pub fn step(_world: &mut World) {}

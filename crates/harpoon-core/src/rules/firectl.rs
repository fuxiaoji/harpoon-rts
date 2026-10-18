//! P1 rules entry workstream module — implemented by its owning agent.
//! (placeholder that keeps the workspace compiling until entry lands)

use crate::world::World;

pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

/// Per-tick fire-control/data-link behavior.
pub fn step(_world: &mut World) {}

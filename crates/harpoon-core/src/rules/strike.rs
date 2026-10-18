//! Strike / land-attack depth: runway & base component state machines,
//! Tattletail regression scenario support.
//!
//! Hook: `step(world)` advances base component states stored on `entity.ext`
//! ("runway_ops" 0..1, "radar_ops", ...) scaling deck regeneration with
//! damage per the land-strike chapters.

use crate::world::World;

/// Extra DB — filled by the strike workstream.
pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

/// Per-tick base component behavior.
pub fn step(_world: &mut World) {}

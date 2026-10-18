//! Air combat depth: merge/dogfight adjudication, maneuver levels, air-to-air
//! gunnery.
//!
//! Hook: `step(world)` runs the merge engagement model each tick — opposing
//! aircraft within merge range exchange gun/AAM passes with maneuver-modified
//! Pk until one side disengages (passes exhausted or fuel threshold).

use crate::world::World;

/// Extra DB (gun Pk vs air, merge parameters) — filled by the air-combat workstream.
pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

/// Per-tick dogfight adjudication.
pub fn step(_world: &mut World) {}

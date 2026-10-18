//! P1 rules entry workstream module — implemented by its owning agent.
//! (placeholder that keeps the workspace compiling until entry lands)

use crate::world::World;

pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

/// Radar-generation / weather-clutter modifier (§5.2/§5.5). Default neutral.
pub fn radar_type_modifier(_def: &crate::db::SensorDef, _t: &crate::world::Entity) -> f64 {
    1.0
}

/// Visual/EO-IR conditions modifier. Default neutral.
pub fn visual_modifier(_o: &crate::world::Entity, _t: &crate::world::Entity) -> f64 {
    1.0
}

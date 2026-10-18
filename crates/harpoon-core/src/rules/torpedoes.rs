//! P1 rules entry workstream module — implemented by its owning agent.
//! (placeholder that keeps the workspace compiling until entry lands)

use crate::world::World;

pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

/// Decoy seduction multiplier for a torpedo seeker (§7.5 generations).
/// 1.0 = base 55% seduction chance; later generations resist.
pub fn decoy_resistance(_def: &crate::db::WeaponDef) -> f64 {
    1.0
}

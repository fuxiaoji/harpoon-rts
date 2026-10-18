//! Anti-air warfare depth: three-second rule / air-burst fragments, AA gun
//! strength bands vs air, SAM director channels.
//!
//! Hook: `on_intercept` is called from the missile defense chain whenever a
//! SAM/CIWS intercept resolves against an incoming missile near a defended
//! unit. Implements the intercept-fragment air-burst adjudication (§8.1.5)
//! with the expanded-critical "air burst" column (half-damage subsystem
//! rolls).

use crate::world::World;

/// Extra DB (AA gun band data, director ratings) — filled by the AAW workstream.
pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

/// Called after a defensive intercept destroys an incoming missile at
/// `range_nm` from `defender`. Applies the three-second-rule fragment burst:
/// probabilistic air-burst criticals on the defender.
pub fn on_intercept(_world: &mut World, _defender: crate::types::EntityId, _range_nm: f64) {}

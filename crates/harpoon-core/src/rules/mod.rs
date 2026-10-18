//! Rules extension layer — per-domain entry points for parallel rule entry.
//!
//! Each domain module owns its rules text transcription and exposes:
//! - `db()` — extra database entries merged into `Database::built_in()`
//! - `step(world)` — per-tick stateful behavior (called from sim)
//! plus domain-specific hooks documented per module.
//!
//! File ownership: each module is owned by exactly one workstream; shared
//! files (world/sim/sensors/weapons) only call these pre-agreed hooks.

pub mod aaw;
pub mod aircombat;
pub mod asw;
pub mod damage_ext;
pub mod strike;

/// Per-tick rules pass (battery, buoy activation, damage control, ...).
pub fn step(world: &mut crate::world::World) {
    asw::step(world);
    damage_ext::step(world);
    aircombat::step(world);
    strike::step(world);
}

/// Extra database entries merged at load.
pub fn database_extras() -> Vec<crate::db::Database> {
    vec![aaw::db(), asw::db(), aircombat::db(), damage_ext::db(), strike::db()]
}

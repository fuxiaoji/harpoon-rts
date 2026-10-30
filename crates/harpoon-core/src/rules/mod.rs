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
pub mod aaw2026;
pub mod aam2026;
pub mod aviation2026;
pub mod aircombat2026;
pub mod pgm2026;
pub mod rocket2026;
pub mod cluster2026;
pub mod strafe2026;
pub mod detection2026;
pub mod ew2026;
pub mod torpedo2026;
pub mod navigation2026;
pub mod mines2026;
pub mod firecontrol2026;
pub mod surface2026;
pub mod ground2026;
pub mod operations;
pub mod aircombat;
pub mod asw;
pub mod asw_weapons2026;
pub mod damage_ext;
pub mod damage_tables;
pub mod damage_runtime;
pub mod signature_damage2026;
pub mod communications2026;
pub mod unmanned2026;
pub mod battery2026;
pub mod beached2026;
pub mod nuclear2026;
pub mod expanded_damage2026;
pub mod submarine_annex;
pub mod repairs2026;
pub mod db_fleets;
pub mod firectl;
pub mod electronic_warfare;
pub mod flight_profiles;
pub mod sensing;
pub mod strike;
pub mod torpedoes;
pub mod unguided;

/// Per-tick rules pass (battery, buoy activation, damage control, ...).
pub fn step(world: &mut crate::world::World) {
    battery2026::step(world);
    ew2026::step(world);
    torpedo2026::step(world);
    aaw2026::update(world);
    mines2026::step(world);
    ground2026::step(world);
    asw::step(world);
    if damage_runtime::active(world) {damage_runtime::step(world);} else {damage_ext::step(world);}
    repairs2026::step(world);
    expanded_damage2026::step(world);
    if world.rules.profile==damage_runtime::RuleProfile::Rts2026Full{aircombat2026::step(world);}else{aircombat::step(world);}
    strike::step(world);
    firectl::step(world);
    torpedoes::step(world);
    unguided::step(world);
    sensing::step(world);
    electronic_warfare::trace(world);
}

/// Extra database entries merged at load.
pub fn database_extras() -> Vec<crate::db::Database> {
    vec![
        aaw::db(),
        asw::db(),
        aircombat::db(),
        damage_ext::db(),
        strike::db(),
        sensing::db(),
        torpedoes::db(),
        unguided::db(),
        db_fleets::db(),
    ]
}

//! Exact Annex A submarine capability atoms, isolated to the v4 source profile.
//! Maximum depth is a game-band upper-bound conversion, not a measured hull
//! engineering limit. This module neither edits the historical database nor
//! supplies a navigation fit, water depth, battery state or missing equipment.
use super::damage_runtime::RuleProfile;
use crate::types::PlatformKind;
use crate::world::Entity;
use crate::World;
use serde_json::Value;
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static TABLES: OnceLock<Value> = OnceLock::new();
    TABLES.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../data/rules/submarine-annex-2025.json"
        ))
        .expect("parent-verified exact submarine annex capability table")
    })
}

/// Per-field permission is separate from a whole-platform or by-name claim.
/// The unresolved ASCM cross-category audit remains in the same source file,
/// but is not an input to any new numeric weapon rule.
pub fn applicability_tables() -> &'static Value {
    static TABLES: OnceLock<Value> = OnceLock::new();
    TABLES.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../data/rules/annex-applicability-2026.json"
        ))
        .expect("exact per-field Annex applicability registry")
    })
}

pub fn field_applicability(w: &World, class: &str, field: &str) -> Option<&'static Value> {
    if w.rules.profile != RuleProfile::Rts2026Full {
        return None;
    }
    applicability_tables()["runtime_field_registry"]["platforms"]
        .get(class).and_then(|p| p.get("fields")).and_then(|f| f.get(field))
        .or_else(|| if field == "battery_fit" { super::battery2026::annex_extra::permission(class) } else { None })
}

fn source_field(w: &World, class: &str, field: &str) -> Option<&'static Value> {
    let permission = field_applicability(w, class, field)?;
    if permission["source_verified"] != true
        || permission["runtime_fit"].as_str() != Some("source_bound_exact_row") {
        return None;
    }
    capability(w, class).and_then(|row| row.get(field)).or_else(|| {
        if field == "battery_fit" { super::battery2026::annex_extra::battery_fit(class) } else { None }
    })
}

/// Conventional battery rating/technology/AIP only; not current charge,
/// chemistry, snorkel equipment or nuclear emergency-battery applicability.
pub fn battery_fit(w: &World, class: &str) -> Option<&'static Value> {
    source_field(w, class, "battery_fit")
}

pub fn verified_maximum_depth_m(w: &World, class: &str) -> Option<f64> {
    source_field(w,class,"maximum_game_depth_m").and_then(Value::as_f64)
}

/// Class lookup deliberately does not infer a kind or match a name/prefix.
/// Consumers handling an entity must separately require PlatformKind::Sub.
pub fn capability(w: &World, class: &str) -> Option<&'static Value> {
    if w.rules.profile != RuleProfile::Rts2026Full {
        return None;
    }
    tables()["platforms"].get(class)
}

/// Keep the pre-existing database value and unknown-class fallback exactly for
/// historical profiles, unqualified classes and non-submarine entity bodies.
pub fn maximum_depth_m(w: &World, e: &Entity) -> f64 {
    if e.kind == PlatformKind::Sub {
        if let Some(depth) = source_field(w, &e.class, "maximum_game_depth_m") {
            return depth
                .as_f64()
                .expect("verified source game-band depth upper bound");
        }
    }
    w.db.platforms
        .get(&e.class)
        .map(|platform| platform.max_depth_m)
        .unwrap_or(200.)
}

/// A source surface-speed ceiling is not the submerged speed or a charge-rate
/// denominator. Absence here leaves the caller's historical behavior intact.
pub fn surface_max_kts(w: &World, e: &Entity) -> Option<f64> {
    if e.kind != PlatformKind::Sub {
        return None;
    }
    source_field(w, &e.class, "surface_max_kts").map(|speed| {
        speed
            .as_f64()
            .expect("verified source undamaged surface speed")
    })
}

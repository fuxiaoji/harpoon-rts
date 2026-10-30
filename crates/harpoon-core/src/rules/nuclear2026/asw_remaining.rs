//! 8.5.2/14.1.8 remaining-item witness contracts and parent-table drift guards.
//! Every mandated mechanism (bystander distance from the actual scatter GZ,
//! guided-miss 30-second continuation, lost-acquired-lock detonation) already has
//! a production consumer in the shared parent or in surface2026/antiship.rs, and
//! this leaf must never re-implement any of them; it carries the typed contract
//! the remaining-item cases adjudicate against, plus guards that fail loudly if a
//! parent revision silently drifts a printed value this domain depends on.
use super::*;

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(
            include_str!("../../../../../data/rules/nuclear-asw-remaining-2026.json")
                .trim_start_matches('\u{feff}'),
        )
        .expect("source nuclear-asw-remaining witness table")
    })
}
pub fn policy(name: &str) -> &'static str {
    tables()["execution_policy"][name]
        .as_str()
        .unwrap_or_else(|| panic!("witness policy entry {name} missing"))
}
/// Printed-value drift guards: the reference constants in the witness table must
/// equal the parent printed table. A parent revision that changes one of these
/// printed values must fail here before any remaining-item case is trusted.
pub fn cross_check_parent_rules() -> Result<(), String> {
    let witness = tables()["rules"].as_object().ok_or("witness rules missing")?;
    let check = |key: &str, parent: f64| -> Result<(), String> {
        let mine = witness[key]
            .as_f64()
            .ok_or_else(|| format!("witness {key} missing"))?;
        if (mine - parent).abs() > 1e-9 {
            return Err(format!("witness {key}={mine} drifted from parent printed {parent}"));
        }
        Ok(())
    };
    check(
        "miss_continue_seconds_reference",
        super::rule("miss_continue_seconds"),
    )?;
    check(
        "opposite_medium_range_factor_reference",
        super::rule("opposite_medium_range_factor"),
    )?;
    check(
        "armed_carrier_destroyed_probability_reference",
        super::rule("armed_carrier_destroyed_probability"),
    )?;
    Ok(())
}

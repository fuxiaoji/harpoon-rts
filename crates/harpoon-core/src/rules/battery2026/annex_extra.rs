//! Exact individually sourced additional battery fields; no whole-platform fit.
use serde_json::Value;
use std::sync::OnceLock;
pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(include_str!("../../../../../data/rules/battery-annex-extra-2025.json")).expect("extra battery annex"))
}
pub fn permission(class: &str) -> Option<&'static Value> {
    let row = tables()["platforms"].get(class)?;
    if row["source_verified"] != true || row["runtime_fit"].as_str() != Some("source_bound_exact_row") { return None; }
    Some(row)
}
pub fn battery_fit(class: &str) -> Option<&'static Value> {
    permission(class)?.get("battery_fit")
}

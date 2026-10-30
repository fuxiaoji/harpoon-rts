//! Independently verified America’s Navy 2025 Annex C HE rows.
use serde_json::Value;
pub(super) fn merge(data: &mut Value) {
    let extra: Value = serde_json::from_str(include_str!("../../../../../data/rules/gun-annex-extra-2025.json")).expect("verified Annex C rows");
    for (id, row) in extra["guns"].as_object().expect("gun rows") {
        assert!(data["guns"].get(id).is_none(), "duplicate source gun ID");
        data["guns"][id] = row.clone();
    }
}

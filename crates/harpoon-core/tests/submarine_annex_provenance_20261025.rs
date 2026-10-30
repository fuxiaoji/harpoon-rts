//! Source provenance assertions derived from US2025 PDF14-16 and rules2026
//! PDF28. Numerical capabilities are deliberately outside this correction.
use serde_json::Value;
#[test]
fn all_eleven_exact_atoms_have_the_real_authority_and_band_specific_policy() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../data/rules/submarine-annex-2025.json"
    ))
    .unwrap();
    assert_eq!(data["platforms"].as_object().unwrap().len(), 11);
    assert!(
        data["scope"].as_str().unwrap().contains("eleven"),
        "scope must describe the actually adopted eleven atoms"
    );
    let authority = data["authorities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "us_navy_2025")
        .expect("US atoms need their original US authority, not only CN/RU authorities");
    assert_eq!(
        authority["sha256"],
        "189416d65b92d5e2fa50c59c99e309ca5715a5db287e5323295152034e61c5c8"
    );
    assert_eq!(authority["pdf_pages"], serde_json::json!([14, 15, 16]));
    for id in [
        "us_virginia",
        "us_virginia_blk5",
        "us_ohio_ssgn",
        "us_ohio_ssbn",
    ] {
        let policy = data["platforms"][id]["conversion_policy"].as_str().unwrap();
        assert!(
            policy.contains("Int V 401-500m") && policy.contains("upper500m"),
            "{id} must cite its own IntV band: {policy}"
        );
        assert!(!policy.contains("Deep I") && !policy.contains("upper600"));
    }
    let seawolf = data["platforms"]["us_seawolf"]["conversion_policy"]
        .as_str()
        .unwrap();
    assert!(seawolf.contains("Deep I") && seawolf.contains("600"));
}

//! Parent-qualified US five-ID Annex A atoms: exact depth band and undamaged
//! surface speed, the same verified shape as the six Russian/Chinese IDs.
use harpoon_core::{load_scenario, observe, scenario_from_json, Database, Side};

struct Case {
    class: &'static str,
    band: &'static str,
    game_max: f64,
    surface: f64,
    db_old_max: f64,
}
const CASES: [Case; 5] = [
    Case { class: "us_virginia", band: "Int V", game_max: 500., surface: 15., db_old_max: 450. },
    Case { class: "us_virginia_blk5", band: "Int V", game_max: 500., surface: 15., db_old_max: 450. },
    Case { class: "us_seawolf", band: "Deep I", game_max: 600., surface: 18., db_old_max: 600. },
    Case { class: "us_ohio_ssgn", band: "Int V", game_max: 500., surface: 15., db_old_max: 300. },
    Case { class: "us_ohio_ssbn", band: "Int V", game_max: 500., surface: 15., db_old_max: 300. },
];

fn fixture(class: &str, depth: Option<f64>, speed: f64) -> harpoon_core::World {
    let sc = scenario_from_json(&format!(
        r#"{{"id":"us-annex-depth","name":"us five-id annex acceptance","seed":7,"duration_s":600,
        "context":{{"rules_profile":"hv2026-rts-v4","radar_environment":{{"sea_state":0,"precipitation":"None"}}}},
        "weather":{{"visibility_nm":1,"jam":[0,0]}},
        "units":[{{"side":"Blue","class":"{class}","name":"SUB","pos":[0,0],"speed_kts":{speed},"depth_m":{depth}}}]}}"#,
        depth = depth.map(|d| d.to_string()).unwrap_or_else(|| "null".into()),
        speed = speed,
    ))
    .unwrap();
    static DB: std::sync::OnceLock<Database> = std::sync::OnceLock::new();
    let db = DB
        .get_or_init(|| {
            Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
        })
        .clone();
    load_scenario(sc, db).unwrap()
}

fn own(world: &harpoon_core::World) -> serde_json::Value {
    serde_json::to_value(observe(world, Side::Blue, 0, 0)).unwrap()
}

#[test]
fn exact_us_annex_capability_replaces_historical_database_depth_and_adds_surface_speed() {
    for c in CASES {
        let w = fixture(c.class, Some(200.), 8.);
        let unit = w.entities.values().find(|e| e.class == c.class).unwrap();
        // Physical cap follows the qualified Annex game band, not the old DB row.
        assert_eq!(
            harpoon_core::rules::submarine_annex::maximum_depth_m(&w, unit),
            c.game_max,
            "{} must use the exact Annex band upper bound",
            c.class
        );
        let cap = harpoon_core::rules::submarine_annex::capability(&w, c.class)
            .unwrap_or_else(|| panic!("{} exact annex capability missing", c.class));
        assert_eq!(
            cap["maximum_depth_band_as_printed"].as_str(),
            Some(c.band),
            "{} band",
            c.class
        );
        assert_eq!(cap["maximum_game_depth_m"], c.game_max);
        assert_eq!(cap["surface_max_kts"], c.surface);
        assert!(cap["source"].as_str().is_some_and(|s| !s.trim().is_empty()));
        // The historical database value stays untouched for legacy profiles.
        assert_eq!(w.db.platforms[c.class].max_depth_m, c.db_old_max);
    }
}

#[test]
fn deep_i_band_definition_and_initial_depth_admission_follow_the_source() {
    // Deep I spans 501-600 m per the 2026 rules depth-band table. A Seawolf may
    // be admitted at 600 m but not deeper; a Virginia (Int V) may not start at
    // 501 m even though the old DB row would have allowed it.
    let ok = fixture("us_seawolf", Some(600.), 5.);
    assert!(ok.entities.values().any(|e| e.class == "us_seawolf"));
    let deep = scenario_from_json(
        r#"{"id":"d","name":"d","seed":7,"duration_s":600,"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"}},"weather":{"visibility_nm":1,"jam":[0,0]},
        "units":[{"side":"Blue","class":"us_seawolf","name":"S","pos":[0,0],"speed_kts":5,"depth_m":601}]}"#,
    )
    .unwrap();
    let deep = load_scenario(deep, {
        static DB: std::sync::OnceLock<Database> = std::sync::OnceLock::new();
        DB.get_or_init(|| Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()).clone()
    });
    assert!(deep.is_err());
    let over_int_v = scenario_from_json(
        r#"{"id":"d2","name":"d2","seed":7,"duration_s":600,"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"}},"weather":{"visibility_nm":1,"jam":[0,0]},
        "units":[{"side":"Blue","class":"us_virginia","name":"S","pos":[0,0],"speed_kts":5,"depth_m":501}]}"#,
    )
    .unwrap();
    let over = load_scenario(over_int_v, {
        static DB2: std::sync::OnceLock<Database> = std::sync::OnceLock::new();
        DB2.get_or_init(|| Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()).clone()
    });
    assert!(over.is_err());
}

#[test]
fn own_projection_discloses_us_capability_without_fit_and_hides_it_from_red() {
    for c in CASES {
        let w = fixture(c.class, Some(200.), 8.);
        let public = own(&w);
        let rules = &public["units"][0]["navigation"]["source_rules"];
        let cap = &rules["annex_capability"];
        assert_eq!(cap["maximum_game_depth_m"], c.game_max, "{}", c.class);
        assert_eq!(cap["surface_max_kts"], c.surface, "{}", c.class);
        assert!(rules["fit"].is_null() && rules["depth_operation"].is_null());
        // Red gains no free report of the Blue submarine or its capability.
        let opposite = observe(&w, Side::Red, 0, 1000);
        let opposite = serde_json::to_value(&opposite).unwrap();
        assert!(
            opposite["units"].as_array().is_none_or(|units| units.is_empty()),
            "{} leaked as a free Red unit report",
            c.class
        );
        let text = opposite.to_string();
        assert!(!text.contains(c.class), "{} leaked to Red", c.class);
        assert!(
            !text.contains("maximum_game_depth_m") && !text.contains("annex_capability"),
            "annex capability keys must never appear in the opposite side observation"
        );
    }
}

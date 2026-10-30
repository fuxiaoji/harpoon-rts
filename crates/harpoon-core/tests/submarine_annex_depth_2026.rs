//! Source-only boundary promoted by the parent for exactly four IDs.
//! CN Navy2025 PDF12/A-5 cn_039a: MaxDepth Int V / Surf13 / Subm19.
//! CN Navy2025 PDF13/A-6 cn_kilo: Int IV / Surf11 / Subm20.
//! RU Navy2025 PDF23/A-20 ru_kilo (636.3), ru_kilo_636: Int IV / Surf11 / Subm20.
//! Frozen qualified atoms: artifacts/rules-battery-annex-20261024/peer/qualified-atoms.json.
//! English2026 PDF28/3-6 gives Int V401-500m and Int IV301-400m.
//! Parent-approved game mapping uses the band upper500/400, NOT an engineering
//! measured hull limit. No battery candidate context or current charge is used.
//!
//! Scenario assumptions: sea0, water1000m (or an explicit shallower boundary),
//! 10kt, initial50m, fixed course, non-steam/underway, no CPP/waterjet, and inert
//! remote red retainer. All sensors/weapons are cleared after scenario loading
//! solely to isolate depth physics; no depth, operation, RNG or source capability
//! is installed directly. Only native SetDepth/ticks cause depth changes.
//! Four real database rows and operations-v3 bytes must remain unchanged.

use harpoon_core::rules::navigation2026::submarine;
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone, Copy)]
struct Case {
    class: &'static str,
    band: &'static str,
    historical_max: f64,
    game_max: f64,
    surface: f64,
    submerged: f64,
}
const CASES: [Case; 4] = [
    Case {
        class: "cn_039a",
        band: "Int V",
        historical_max: 250.,
        game_max: 500.,
        surface: 13.,
        submerged: 19.,
    },
    Case {
        class: "cn_kilo",
        band: "Int IV",
        historical_max: 200.,
        game_max: 400.,
        surface: 11.,
        submerged: 20.,
    },
    Case {
        class: "ru_kilo",
        band: "Int IV",
        historical_max: 240.,
        game_max: 400.,
        surface: 11.,
        submerged: 20.,
    },
    Case {
        class: "ru_kilo_636",
        band: "Int IV",
        historical_max: 200.,
        game_max: 400.,
        surface: 11.,
        submerged: 20.,
    },
];

fn database() -> Database {
    Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
}
fn declared_fit(c: Case, maximum: f64, water: f64, surface: f64) -> Value {
    json!({
        "source":"scenario_assumption: controlled navigation geometry; exact annex depth/surface atoms only, not complete real equipment",
        "size":"C", "merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,
        "undamaged_max_kts":c.submerged,"propulsion":"NonSteam","readiness":"Underway",
        "submarine":{
            "source":"Annex MaxDepth band + parent-approved upper-band game conversion; explicit author water",
            "maximum_depth_m":maximum,"surface_max_kts":surface,"water_depth_m":water,"very_small":false
        }
    })
}
fn scenario(class: &str, profile: &str, fit: Option<Value>) -> Scenario {
    let mut context = json!({
        "rules_profile":profile,"radar_environment":{"sea_state":0,"precipitation":"None"}
    });
    if let Some(fit) = fit {
        context["navigation_2026"] = json!({
            "source":"scenario_assumption: explicitly bound depth mechanism input, not automatic hardware fit",
            "units":{"OWN":fit}
        });
    }
    serde_json::from_value(json!({
        "id":"submarine-annex-depth-source", "name":"Annex band game-limit test",
        "description":"scenario_assumption: isolated depth physics, no native sonar or battery proof",
        "duration_s":10000,"seed":20261024,"context":context,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[
            {"side":"Blue","class":class,"name":"OWN","pos":[0,0],"course_deg":0,"speed_kts":10,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"us_ford","name":"REMOTE RETAINER","pos":[1000,1000],"course_deg":0,"speed_kts":0,"fire_on":false}
        ]
    })).unwrap()
}
fn loaded(sc: Scenario, db: Database) -> World {
    let mut world = harpoon_core::load_scenario(sc, db).unwrap_or_else(|reason| {
        panic!("qualified source game-band fit must load without raising historical database max: {reason}")
    });
    for entity in world.entities.values_mut() {
        entity.sensors.clear();
        entity.weapons.clear();
    }
    world
}
fn fixture(c: Case, fit: Option<Value>) -> World {
    loaded(scenario(c.class, "hv2026-rts-v4", fit), database())
}
fn actor(w: &World) -> EntityId {
    w.entities.values().find(|e| e.name == "OWN").unwrap().id
}
fn tick(w: &mut World, n: usize) {
    for _ in 0..n {
        assert!(
            !w.over,
            "remote retainer must keep native depth clock running"
        );
        harpoon_core::sim::tick(w, &[]);
    }
}
fn order(w: &mut World, depth: f64) {
    let unit = actor(w);
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::SetDepth {
                unit,
                depth_m: depth,
            },
        )],
    );
}
fn own(w: &World, side: Side) -> Value {
    let obs = harpoon_core::observe(w, side, 0, 1000);
    serde_json::to_value(obs.units.iter().find(|u| u.id == actor(w))).unwrap()
}
fn assert_original_rows(w: &World) {
    for c in CASES {
        assert_eq!(
            w.db.platforms[c.class].max_depth_m, c.historical_max,
            "{} database row rewritten",
            c.class
        );
    }
}
fn verify_real_source_dive(c: Case) {
    let mut w = fixture(c, Some(declared_fit(c, c.game_max, 1000., c.surface)));
    let original_database = serde_json::to_value(&w.db).unwrap();
    let unit = actor(&w);
    assert!(submarine::validate_order_depth(&w, unit, c.game_max).is_ok());
    order(&mut w, c.game_max);
    assert!(w.entity(unit).unwrap().depth_m > 50. && w.entity(unit).unwrap().depth_m < 51.);
    tick(&mut w, 499);
    assert!(
        (w.entity(unit).unwrap().depth_m - c.game_max).abs() < 1e-8,
        "{} must physically reach source band upper",
        c.class
    );
    assert!(w.entity(unit).unwrap().depth_m > c.historical_max);
    tick(&mut w, 60);
    assert_eq!(
        w.entity(unit).unwrap().depth_m,
        c.game_max,
        "no free overshoot after arrival"
    );
    assert_original_rows(&w);
    assert_eq!(serde_json::to_value(&w.db).unwrap(), original_database);
}

#[test]
fn cn_039a_real_set_depth_reaches_int_v_game_upper_500_without_rewriting_250() {
    verify_real_source_dive(CASES[0]);
}
#[test]
fn cn_kilo_real_set_depth_reaches_int_iv_game_upper_400_without_rewriting_200() {
    verify_real_source_dive(CASES[1]);
}
#[test]
fn ru_kilo_6363_real_set_depth_reaches_int_iv_game_upper_400_without_rewriting_240() {
    verify_real_source_dive(CASES[2]);
}
#[test]
fn ru_kilo_636_real_set_depth_reaches_int_iv_game_upper_400_without_rewriting_200() {
    verify_real_source_dive(CASES[3]);
}

#[test]
fn four_exact_annex_ids_also_bound_real_unfitted_v4_movement_without_inventing_a_runtime_fit() {
    for c in CASES {
        let mut w = fixture(c, None);
        let unit = actor(&w);
        order(&mut w, c.game_max);
        tick(&mut w, 249);
        assert_eq!(
            w.entity(unit).unwrap().depth_m,
            c.game_max,
            "{} unconfigured physical cap still used historical database max",
            c.class
        );
        assert!(w.entity(unit).unwrap().depth_m > c.historical_max);
        let public = own(&w, Side::Blue);
        let rules = &public["navigation"]["source_rules"];
        assert!(
            rules["fit"].is_null() && rules["depth_operation"].is_null(),
            "static source capability must not manufacture fitted navigation or a depth clock"
        );
        assert_original_rows(&w);
    }
}

#[test]
fn own_static_annex_capability_is_disclosed_without_fit_and_never_exposes_enemy_capability() {
    for c in CASES {
        let w = fixture(c, None);
        let public = own(&w, Side::Blue);
        let cap = &public["navigation"]["source_rules"]["annex_capability"];
        assert!(
            cap.is_object(),
            "{} exact source capability missing from real own observation",
            c.class
        );
        assert_eq!(cap["maximum_depth_band_as_printed"], c.band);
        assert_eq!(cap["maximum_game_depth_m"], c.game_max);
        assert_eq!(cap["surface_max_kts"], c.surface);
        assert!(cap["source"].as_str().is_some_and(|s| !s.trim().is_empty()));
        assert!(cap["conversion_policy"]
            .as_str()
            .is_some_and(|s| !s.trim().is_empty()));
        assert!(public["navigation"]["source_rules"]["fit"].is_null());
        assert!(public["navigation"]["source_rules"]["depth_operation"].is_null());
        let opposite = harpoon_core::observe(&w, Side::Red, 0, 1000);
        assert!(!opposite.units.iter().any(|u| u.id == actor(&w)));
        assert!(!serde_json::to_string(&opposite)
            .unwrap()
            .contains("annex_capability"));
    }
}

#[test]
fn source_band_upper_fit_limit_and_actual_water_each_remain_independent_admission_bounds() {
    let c = CASES[0];
    for (fit_limit, water, forbidden, legal) in [
        (500., 1000., 500.001, 500.),
        (350., 1000., 350.001, 350.),
        (500., 350., 350.001, 350.),
    ] {
        let mut w = fixture(c, Some(declared_fit(c, fit_limit, water, c.surface)));
        let unit = actor(&w);
        let before = serde_json::to_value(&w).unwrap();
        assert!(submarine::validate_order_depth(&w, unit, forbidden).is_err());
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            before,
            "readonly admission changed complete World"
        );
        order(&mut w, forbidden);
        tick(&mut w, 10);
        assert_eq!(w.entity(unit).unwrap().depth_m, 50.);
        assert_eq!(w.entity(unit).unwrap().ordered_depth_m, 50.);
        order(&mut w, legal);
        tick(&mut w, 499);
        assert!((w.entity(unit).unwrap().depth_m - legal).abs() < 1e-8);
        assert!(w.entity(unit).unwrap().depth_m <= fit_limit.min(water).min(c.game_max));
        assert_original_rows(&w);
    }
}

#[test]
fn an_author_fit_cannot_raise_even_a_qualified_id_above_its_source_band() {
    for c in CASES {
        let sc = scenario(
            c.class,
            "hv2026-rts-v4",
            Some(declared_fit(c, c.game_max + 0.001, 1000., c.surface)),
        );
        assert!(
            harpoon_core::load_scenario(sc, database()).is_err(),
            "{} fit raised source band upper",
            c.class
        );
    }
}

#[test]
fn sourced_surface_13_or_11_is_not_replaceable_by_a_higher_submerged_speed() {
    for c in CASES {
        // The historical lower depth is intentional here: it isolates the
        // surface-speed gate instead of failing first on the new depth band.
        let sc = scenario(
            c.class,
            "hv2026-rts-v4",
            Some(declared_fit(c, c.historical_max, 1000., c.surface + 0.1)),
        );
        assert!(
            harpoon_core::load_scenario(sc, database()).is_err(),
            "{} accepted above sourced surface capability",
            c.class
        );
    }
}

#[test]
fn above_old_depth_full_world_restore_keeps_native_operation_and_identical_continuation() {
    let c = CASES[0];
    let mut original = fixture(c, Some(declared_fit(c, c.game_max, 1000., c.surface)));
    let unit = actor(&original);
    order(&mut original, c.game_max);
    tick(&mut original, 299);
    assert!(original.entity(unit).unwrap().depth_m > c.historical_max);
    assert!(
        own(&original, Side::Blue)["navigation"]["source_rules"]["depth_operation"].is_object()
    );
    let mut restored: World =
        serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
    restored.rules.validate_tables().unwrap();
    tick(&mut original, 150);
    tick(&mut restored, 150);
    assert_eq!(original.entity(unit).unwrap().depth_m, c.game_max);
    assert_eq!(
        serde_json::to_value(&original).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert_original_rows(&restored);
}

#[test]
fn unknown_or_prefix_similar_ids_and_unmapped_ir_kilo_do_not_inherit_qualified_atoms() {
    let mut db = database();
    let mut fake = db.platforms["cn_039a"].clone();
    fake.id = "cn_039a_scenario_unqualified".into();
    // Deliberately keep the exact real display name to disprove name matching.
    db.platforms.insert(fake.id.clone(), fake);
    for (class, expected_cap) in [("cn_039a_scenario_unqualified", 250.), ("ir_kilo", 300.)] {
        let mut w = loaded(scenario(class, "hv2026-rts-v4", None), db.clone());
        let unit = actor(&w);
        order(&mut w, 500.);
        tick(&mut w, 249);
        assert_eq!(w.entity(unit).unwrap().depth_m, expected_cap);
        assert!(own(&w, Side::Blue)["navigation"]["source_rules"]["annex_capability"].is_null());
        assert_original_rows(&w);
    }
}

#[test]
fn legacy_v2_and_operations_v3_keep_all_four_historical_database_depths() {
    for profile in ["rts-legacy-v1", "hv2026-rts-v2", "hv2026-rts-v3"] {
        for c in CASES {
            let mut w = loaded(scenario(c.class, profile, None), database());
            let unit = actor(&w);
            order(&mut w, c.game_max);
            tick(&mut w, 249);
            assert_eq!(
                w.entity(unit).unwrap().depth_m,
                c.historical_max,
                "{} {profile} historical cap changed",
                c.class
            );
            assert!(
                own(&w, Side::Blue)["navigation"]["source_rules"]["annex_capability"].is_null()
            );
            assert_original_rows(&w);
        }
    }
}

fn fnv64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, byte| {
        (h ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}
#[test]
fn historical_database_and_operations_v3_raw_bytes_stay_frozen_across_source_motion() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let expected = [
        ("database/consolidated.json", 0x356b3d028dae8a10_u64),
        ("database/volumes/cn_navy.json", 0x3f8534344e8a49ea_u64),
        ("database/volumes/ru_navy.json", 0x9d6964dcb843f918_u64),
        ("data/rules/operations-v3.json", 0x9a20fd414e6ede81_u64),
    ];
    let before: Vec<_> = expected
        .iter()
        .map(|(path, hash)| {
            let bytes = std::fs::read(root.join(path)).unwrap();
            assert_eq!(
                fnv64(&bytes),
                *hash,
                "frozen historical input {path} changed"
            );
            bytes
        })
        .collect();
    let c = CASES[0];
    let mut w = fixture(c, None);
    order(&mut w, c.game_max);
    tick(&mut w, 249);
    for ((path, _), original) in expected.iter().zip(before) {
        assert_eq!(
            std::fs::read(root.join(path)).unwrap(),
            original,
            "source runtime rewrote {path}"
        );
    }
    assert_original_rows(&w);
}

#[test]
fn known_annex_game_band_rejects_an_illegal_initial_depth_without_a_silent_first_tick_teleport() {
    for c in CASES {
        let mut sc = scenario(c.class, "hv2026-rts-v4", None);
        sc.units[0].depth_m = Some(c.game_max + 1.);
        assert!(
            harpoon_core::load_scenario(sc, database()).is_err(),
            "{} initial depth above the known source game cap must be rejected",
            c.class
        );
    }
}

#[test]
fn the_four_sourced_undamaged_surface_ceilings_apply_to_actual_unfitted_surface_motion() {
    for c in CASES {
        let mut sc = scenario(c.class, "hv2026-rts-v4", None);
        sc.units[0].depth_m = Some(0.);
        let mut world = loaded(sc, database());
        let unit = actor(&world);
        harpoon_core::sim::tick(
            &mut world,
            &[(
                Side::Blue,
                Command::SetSpeed {
                    unit,
                    speed_kts: c.submerged,
                },
            )],
        );
        tick(&mut world, 199);
        assert_eq!(world.entity(unit).unwrap().depth_m, 0.);
        assert!(
            (world.entity(unit).unwrap().speed_kts - c.surface).abs() < 1e-9,
            "{} surface motion used submerged maximum",
            c.class
        );
        assert!(!submarine::configured_depth(&world, unit));
        assert_original_rows(&world);
    }
}

fn verify_following_annex_case(c: Case) {
    assert_eq!(database().platforms[c.class].max_depth_m, c.historical_max);
    verify_real_source_dive(c);
    let world = fixture(c, None);
    let cap = own(&world, Side::Blue)["navigation"]["source_rules"]["annex_capability"].clone();
    assert_eq!(cap["maximum_depth_band_as_printed"], c.band);
    assert_eq!(cap["maximum_game_depth_m"], c.game_max);
    assert_eq!(cap["surface_max_kts"], c.surface);
    for profile in ["rts-legacy-v1", "hv2026-rts-v2", "hv2026-rts-v3"] {
        let mut old = loaded(scenario(c.class, profile, None), database());
        order(&mut old, c.game_max);
        tick(&mut old, 249);
        assert_eq!(old.entity(actor(&old)).unwrap().depth_m, c.historical_max);
    }
    let mut surfaced = scenario(c.class, "hv2026-rts-v4", None);
    surfaced.units[0].depth_m = Some(0.);
    let mut surfaced = loaded(surfaced, database());
    let unit = actor(&surfaced);
    harpoon_core::sim::tick(
        &mut surfaced,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit,
                speed_kts: c.submerged,
            },
        )],
    );
    tick(&mut surfaced, 199);
    assert!((surfaced.entity(unit).unwrap().speed_kts - c.surface).abs() < 1e-9);
}

#[test]
fn exact_039g1_continued_a6_row_supports_int_v_500_and_surface12_without_borrowing_039g() {
    verify_following_annex_case(Case {
        class: "cn_039g",
        band: "Int V",
        historical_max: 165.,
        game_max: 500.,
        surface: 12.,
        submerged: 18.,
    });
}

#[test]
fn generic_lada_a19_supports_int_iv_400_and_surface10_without_named_boat_battery_assumptions() {
    verify_following_annex_case(Case {
        class: "ru_lada",
        band: "Int IV",
        historical_max: 300.,
        game_max: 400.,
        surface: 10.,
        submerged: 21.,
    });
}

#[test]
fn unfitted_exact_ids_reject_native_depth_orders_above_the_source_game_band() {
    // Parent strengthened the common admission contract: an unfitted source
    // capability is still a real ceiling. Do not accept an illegal order and
    // silently clamp it as the old positive fixture did. Simulated time and
    // position may advance; rejection must preserve the legal standing depth.
    for c in CASES {
        let mut w = fixture(c, None);
        let unit = actor(&w);
        order(&mut w, c.game_max + 1.);
        assert_eq!(w.entity(unit).unwrap().ordered_depth_m, 50.);
        assert_eq!(w.entity(unit).unwrap().depth_m, 50.);
        assert!(
            w.events.iter().any(|event| {
                matches!(&event.kind, harpoon_core::EventKind::RulesEvent {
                domain, rule, unit: Some(rejected_unit), detail, ..
            } if domain == "command" && rule == "rejected"
                && *rejected_unit == unit && detail.contains("set_depth"))
            }),
            "{} illegal depth order lacks a real native rejection receipt",
            c.class
        );
        tick(&mut w, 10);
        assert_eq!(w.entity(unit).unwrap().ordered_depth_m, 50.);
        assert_eq!(w.entity(unit).unwrap().depth_m, 50.);
        assert_original_rows(&w);
    }
}

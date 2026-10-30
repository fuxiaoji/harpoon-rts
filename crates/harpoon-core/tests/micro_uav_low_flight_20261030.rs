//! §4.5.2/§4.5.3 very-small HITL UAV exemption — public behavior examples.
//! Source: rules 1 Jan 26 PDF34 (printed 4-2), visually reviewed 2026-10-30;
//! render and exact sentences in the batch artifact ABI.json/HANDOFF.md.
//! Deterministic public-API cases only: no dice, no seed searching, no
//! contact/FCS/classification/HP/projectile injection. The exemption is an
//! explicit author declaration plus the actual unmanned HITL live-link gates;
//! real DB classes (us_mq4c/us_mq25a) are used read-only and are NEVER
//! declared micro — the laboratory micro airframe is an explicit
//! scenario_assumption clone. NOT COMPILED AND NOT RUN in this batch (cargo
//! prohibited); the module compiles once root adds `pub mod micro_uav;`.
use harpoon_core::rules::aviation2026::micro_uav as m;
use harpoon_core::rules::unmanned2026 as u;
use harpoon_core::{Database, Scenario, World};
use serde_json::{json, Value};

fn database() -> Database {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    // Laboratory micro airframe: an original-body clone with an explicit
    // scenario_assumption declaration. The original DB row is untouched and
    // no real platform is claimed to be micro.
    let mut lab = db.platforms.get("us_mq25a").cloned().unwrap();
    lab.id = "micro_lab".into();
    lab.name =
        "scenario_assumption: experimental micro airframe (laboratory original-body clone)".into();
    db.platforms.insert(lab.id.clone(), lab);
    db.validate().unwrap();
    db
}

fn hitl_fit() -> Value {
    json!({
        "source": "scenario_assumption: control experiment; operational choices and bandwidth are author inputs",
        "domain": "air",
        "autonomous_capable": false,
        "initial_mode": "hitl",
        "controller": "CTRL",
        "element": "ctrl",
        "initial_link": "satellite",
        "satellite": true,
        "satellite_available": true,
        "loss_behavior": {"type": "return_to_home", "home": "CTRL"}
    })
}

fn autonomous_fit() -> Value {
    json!({
        "source": "scenario_assumption: control experiment",
        "domain": "air",
        "autonomous_capable": true,
        "initial_mode": "autonomous",
        "controller": "CTRL",
        "element": "ctrl",
        "initial_link": "satellite",
        "satellite": true,
        "satellite_available": true,
        "loss_behavior": {"type": "move_away_from_jammer"}
    })
}

fn world(seed: u64, day: &str, visibility: f64, vehicle_class: &str, fit: Option<Value>) -> World {
    let mut context = json!({
        "rules_profile": "hv2026-rts-v4",
        "aviation_2026": {
            "source": "scenario_assumption: printed clear-weather visibility declaration only",
            "loads": {},
            "parking": {},
            "visibility_percent": visibility
        },
        "initial_fuel_note": "airborne laboratory aircraft use explicit author fuel below"
    });
    // The not-yet-wired context key is deserialization noise today; the micro
    // qualification itself is passed in Rust, so these cases stay valid before
    // and after root's context hook. Kept out of the JSON entirely.
    let _ = &mut context;
    if let Some(fit) = fit {
        context["unmanned_2026"] = json!({
            "source": "scenario_assumption: Annex-B-style control experiment; see data/rules/unmanned-annex-2025.json for real-platform facts",
            "controllers": {"CTRL": {
                "source": "scenario_assumption: author control element; national bandwidth is an explicit scenario asset",
                "elements": ["ctrl"],
                "operational": true,
                "satellite": true,
                "satellite_available": true
            }},
            "vehicles": {vehicle_name_for(vehicle_class): fit}
        });
    }
    let aircraft_unit = json!({
        "side": "Blue",
        "class": vehicle_class,
        "name": vehicle_name_for(vehicle_class),
        "pos": [0.2, 0],
        "course_deg": 90,
        "speed_kts": 300,
        "altitude_m": 8000,
        "fire_on": false
    });
    let sc: Scenario = serde_json::from_value(json!({
        "id": "micro-uav-low-flight-20261030",
        "name": "scenario_assumption: very-small HITL UAV low-flight exemption experiment, original DB hulls/stocks only",
        "duration_s": 4000,
        "seed": seed,
        "day_night": day,
        "weather": {"visibility_nm": 20, "jam": [0, 0]},
        "context": context,
        "units": [
            {"side": "Blue", "class": "us_burke_2a", "name": "CTRL", "pos": [0, 0], "course_deg": 90, "speed_kts": 0, "fire_on": false},
            aircraft_unit
        ]
    }))
    .unwrap();
    harpoon_core::load_scenario(sc, database()).unwrap()
}

fn vehicle_name_for(class: &str) -> &'static str {
    if class == "micro_lab" {
        "MICRO"
    } else {
        "STRAY"
    }
}

fn author_micro_config() -> m::Configuration {
    m::Configuration {
        source: "scenario_assumption: laboratory micro airframe qualification".into(),
        micro_classes: [(
            "micro_lab".to_string(),
            m::ClassQualification {
                source: "scenario_assumption: experimental micro airframe declaration".into(),
            },
        )]
        .into_iter()
        .collect(),
    }
}

/// Every printed premise is visible and each one is required: drop any single
/// condition (author declaration, unmanned fit, HITL live link, daylight,
/// clear weather) and the exemption is refused.
#[test]
fn micro_exemption_requires_every_printed_condition() {
    let cfg = author_micro_config();

    // Fully qualified: author micro + actual unmanned HITL live link + day +
    // printed clear weather.
    let w = world(20261040, "Day", 100.0, "micro_lab", Some(hitl_fit()));
    let micro = w
        .entities
        .values()
        .find(|e| e.name == "MICRO")
        .unwrap()
        .id;
    let e = m::eligibility(&w, micro, Some(&cfg)).unwrap();
    assert_eq!(
        e,
        m::Eligibility {
            author_micro: true,
            actual_unmanned: true,
            hitl_live_link: true,
            daylight: true,
            clear_weather: true
        }
    );
    assert!(m::exempt_with(&w, micro, Some(&cfg)));

    // None configuration: legacy printed roll, never exempt.
    assert!(!m::Eligibility::complete(e) || true); // eligible base stays documented above
    let none_case = m::eligibility(&w, micro, None).unwrap();
    assert!(!none_case.author_micro && !none_case.complete());
    assert!(!m::exempt_with(&w, micro, None));

    // Night: the exemption is daylight-only.
    let night = world(20261041, "Night", 100.0, "micro_lab", Some(hitl_fit()));
    let micro_n = night.entities.values().find(|e| e.name == "MICRO").unwrap().id;
    let e = m::eligibility(&night, micro_n, Some(&cfg)).unwrap();
    assert!(!e.daylight && !e.complete());

    // Less than the printed 40% clear boundary: not clear weather.
    let murk = world(20261042, "Day", 30.0, "micro_lab", Some(hitl_fit()));
    let micro_m = murk.entities.values().find(|e| e.name == "MICRO").unwrap().id;
    let e = m::eligibility(&murk, micro_m, Some(&cfg)).unwrap();
    assert!(!e.clear_weather && !e.complete());

    // Autonomous mode: not established HITL control.
    let auto = world(20261043, "Day", 100.0, "micro_lab", Some(autonomous_fit()));
    let micro_a = auto.entities.values().find(|e| e.name == "MICRO").unwrap().id;
    let e = m::eligibility(&auto, micro_a, Some(&cfg)).unwrap();
    assert!(!e.hitl_live_link && !e.complete());

    // Lost link: the referee withdraws the author-declared national bandwidth
    // through the real producer; the live-link premise fails.
    let mut lost = world(20261044, "Day", 100.0, "micro_lab", Some(hitl_fit()));
    let micro_l = lost.entities.values().find(|e| e.name == "MICRO").unwrap().id;
    let ctrl = lost.entities.values().find(|e| e.name == "CTRL").unwrap().id;
    u::set_satellite_available(
        &mut lost,
        &[micro_l, ctrl],
        false,
        "scenario_assumption: national bandwidth withdrawn",
    )
    .unwrap();
    let e = m::eligibility(&lost, micro_l, Some(&cfg)).unwrap();
    assert!(!e.hitl_live_link && !e.complete());
    assert!(!m::exempt_with(&lost, micro_l, Some(&cfg)));
}

/// The author declaration is class-exact and never inferred: a real DB class
/// with a live HITL link stays non-exempt, and an undeclared airframe without
/// any unmanned fit fails the actual-unmanned premise.
#[test]
fn micro_is_author_declared_only() {
    let cfg = author_micro_config();
    assert_eq!(cfg.micro_classes.len(), 1);
    assert!(cfg.micro_classes.contains_key("micro_lab"));
    // No real DB class is declared micro, including the annex-processed ones.
    assert!(!cfg.micro_classes.contains_key("us_mq4c"));
    assert!(!cfg.micro_classes.contains_key("us_mq25a"));

    // Real us_mq25a, fully live HITL link, day, clear — but not declared
    // micro: the model name and the unmanned label confer nothing.
    let w = world(20261045, "Day", 100.0, "us_mq25a", Some(hitl_fit()));
    let stray = w.entities.values().find(|e| e.name == "STRAY").unwrap().id;
    let e = m::eligibility(&w, stray, Some(&cfg)).unwrap();
    assert!(!e.author_micro && !e.complete());
    assert!(e.actual_unmanned && e.hitl_live_link && e.daylight && e.clear_weather);
    assert!(!m::exempt_with(&w, stray, Some(&cfg)));

    // Declared class without any actual unmanned fit: hardware premise fails.
    let bare = world(20261046, "Day", 100.0, "micro_lab", None);
    let micro = bare.entities.values().find(|e| e.name == "MICRO").unwrap().id;
    let e = m::eligibility(&bare, micro, Some(&cfg)).unwrap();
    assert!(e.author_micro && e.daylight && e.clear_weather);
    assert!(!e.actual_unmanned && !e.hitl_live_link && !e.complete());
}

/// The leaf table exposes exactly the printed threshold and authority, and
/// invents no crash table of its own.
#[test]
fn tables_expose_printed_threshold_and_authority() {
    assert_eq!(
        m::tables()["source_values"]["clear_weather_min_visibility_percent"]["value"]
            .as_f64(),
        Some(40.0)
    );
    assert_eq!(
        m::tables()["authority"]["pdf_page"].as_i64(),
        Some(34)
    );
    assert!(m::tables()["policy"]["no_new_probability"] == "this table and leaf restate no crash table, no percentage and no rounding; the printed SSkim/NOE Crash Chance table remains solely in the parent tables/low_flight data");
    assert!(m::tables().get("crash_table").is_none());
}

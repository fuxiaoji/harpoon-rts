//! Source candidates only. Future source-labelled first-class laser components;
//! no Radar/IR/Visual adapters, no Contact/illumination/HP/dice injection.
use harpoon_core::{Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
fn input(designator: bool, enabled: bool, tracker: Option<&str>) -> Value {
    let emitter = if designator { "ru_ka27" } else { "us_burke_2a" };
    let device = if designator {
        "designator"
    } else {
        "rangefinder"
    };
    let mut v = json!({"id":"laser-source","name":"scenario_assumption: controlled physical laser retrofit and fixed aimpoint","seed":610,"duration_s":100,
 "weather":{"visibility_nm":40,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4",
 "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: clear LOS and no moisture attenuation","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
 "laser_sensors":{"source":"English2026 PDF60 5.6.1-5; author physical test retrofits, no Annex completeness claim",
 "fits":{"ILLUMINATOR":{"source":"scenario_assumption: actual controlled installed laser retrofit","device":device,"installed":true,"range_nm":if designator{json!(5)}else{Value::Null},"range_source":if designator{json!("scenario_assumption: controlled5nm designator envelope, not an unprinted universal performance")}else{json!("English2026 5.6.1 shipboard laser rangefinder10nm")},"bearing_policy":"exact_scene_ray"},
 "TARGET":{"source":"scenario_assumption: physically installed LWS on this exact ship","device":"lws","installed":true,"bearing_policy":"exact_scene_ray"}},
 "initial_jobs":{"ILLUMINATOR":{"source":"scenario_assumption: allocated fixed aimpoint beam order, not hidden target tracking","enabled":enabled,"purpose":if designator{"designation"}else{"rangefinding"},"known_aimpoint":{"x":0,"y":0},"aimpoint_source":"scenario_assumption: controlled ship remains at this published fixed point"}}}}},
 "units":[{"side":"Blue","class":emitter,"name":"ILLUMINATOR","pos":[0,-2],"course_deg":0,"speed_kts":0,"altitude_m":if designator{1000}else{0},"fire_on":false},
 {"side":"Red","class":"cn_001","name":"TARGET","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false}]});
    if let Some(device) = tracker {
        v["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":"ru_ka27","name":"TRACKER","pos":[0,-1],"course_deg":0,"speed_kts":0,"altitude_m":1000,"fire_on":false}));
        v["context"]["detection_2026"]["laser_sensors"]["fits"]["TRACKER"] = json!({"source":"scenario_assumption: actual separate installed aircraft spot receiver, no pre-existing contact","device":device,"installed":true,"spot_range_nm":5,"spot_range_source":"scenario_assumption: controlled fitted spot-view envelope, no universal unprinted LST range","bearing_policy":"exact_scene_ray"});
    }
    v
}
fn world(v: Value) -> World {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for class in ["ru_ka27", "us_burke_2a", "cn_001"] {
        let p = db.platforms.get_mut(class).unwrap();
        p.sensors.clear();
        p.weapons.clear();
    }
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(v).unwrap(), db).unwrap()
}
fn report(w: &World, side: Side, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter(|e| e.vis == harpoon_core::events::Visibility::Side(side))
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule: r, detail, ..
            } if r == rule => Some(serde_json::from_str(detail).unwrap()),
            _ => None,
        })
        .collect()
}
#[test]
fn laser01_any_actual_rangefinding_or_designation_illumination_automatically_warns_lws_in_detection_phase(
) {
    for designator in [false, true] {
        for enabled in [false, true] {
            let mut w = world(input(designator, enabled, None));
            harpoon_core::sim::tick(&mut w, &[]);
            let warnings = report(&w, Side::Red, "5.6.3.laser_warning");
            if enabled {
                assert!(!warnings.is_empty(),"a real fitted target illuminated by a qualified beam must receive an automatic Detection-phase LWS warning");
                for r in warnings {
                    assert_eq!(r["phase"], "detection");
                    assert!((r["bearing_deg"].as_f64().unwrap() - 180.).abs() < 1e-6);
                    for private in ["emitter", "emitter_id", "range_nm", "truth_target", "class"] {
                        assert!(r.get(private).is_none());
                    }
                }
            } else {
                assert!(warnings.is_empty());
            }
            assert!(
                w.find_contact(Side::Red, EntityId(1)).is_none(),
                "illumination direction is not a free current emitter contact"
            );
        }
    }
}
#[test]
fn laser02_lst_automatically_acquires_only_designation_spots_and_lrmts_adds_its_actual_range() {
    for device in ["lst", "lrmts"] {
        for designator in [false, true] {
            let mut w = world(input(designator, true, Some(device)));
            let mut saved: World =
                serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
            for _ in 0..2 {
                harpoon_core::sim::tick(&mut w, &[]);
                harpoon_core::sim::tick(&mut saved, &[]);
            }
            let rule = if device == "lrmts" {
                "5.6.5.lrmts_spot_ranged"
            } else {
                "5.6.4.laser_spot_acquired"
            };
            let found = report(&w, Side::Blue, rule);
            if designator {
                assert!(!found.is_empty(),"a fitted tracker must automatically detect an actual designator spot without a previously fused contact");
                let r = &found[0];
                if device == "lrmts" {
                    assert!((r["range_nm"].as_f64().unwrap() - 1.).abs() < 1e-6);
                } else {
                    assert!(r.get("range_nm").is_none_or(|v| v.is_null()));
                    assert!(r.get("measured_position").is_none_or(|v| v.is_null()));
                }
            } else {
                assert!(
                    found.is_empty(),
                    "a rangefinder-only beam does not become a designator spot"
                );
            }
            assert_eq!(
                serde_json::to_value(&w).unwrap(),
                serde_json::to_value(&saved).unwrap()
            );
        }
    }
}

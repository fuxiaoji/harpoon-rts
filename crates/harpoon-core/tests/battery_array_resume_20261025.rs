//! Native tick/command candidates for the resumed battery batch. Original
//! database hulls/DP/weapon stocks are loaded unchanged. Initial battery state,
//! chemistry, snorkel fit and charge denominator are explicit scenario inputs.
//! No contact, FCS, die, HP, Flight or runtime battery-state injection is used.
use harpoon_core::rules::{battery2026 as b, navigation2026 as n};
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

fn scenario(
    class: &str,
    resolution: &str,
    speed: f64,
    depth: f64,
    charge: Option<f64>,
    aip: Option<f64>,
    enabled: bool,
) -> Value {
    json!({"id":"battery-resume-native","name":"scenario_assumption: explicit initial battery and snorkel experiment","seed":20261025,"duration_s":10000,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "battery_2026":{"source":"scenario_assumption: authored state, exact Annex capacity","resolution":resolution,"old_10kt_policy":null,"units":{"SUB":{
                "source":"scenario_assumption: initial charge/AIP and explicitly fitted snorkel; non-lithium; explicit charge era/denominator",
                "platform_class":class,"initial_charge_units":charge,"initial_aip_remaining_units":aip,"initial_aip_operating":enabled,
                "initial_charging":false,"initial_power_mode":"battery","initial_mast_raised":false,"snorkel_fitted":true,"lithium_ion":false,
                "charge_rate_era":"post1970","full_speed_reference_kts":20.0,"charge_combination_policy":"multiply_half_rate","aip_output_units_per_intermediate_turn":null}}}},
        "units":[{"side":"Blue","class":class,"name":"SUB","pos":[0,0],"course_deg":0,"speed_kts":speed,"depth_m":depth,"fire_on":false},
                 {"side":"Red","class":"us_virginia","name":"DISTANT","pos":[400,400],"course_deg":0,"speed_kts":0,"depth_m":100,"fire_on":false}]})
}
fn load(value: Value) -> Result<World, String> {
    let scenario: Scenario = serde_json::from_value(value).unwrap();
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    harpoon_core::load_scenario(scenario, db).map_err(|e| e.to_string())
}
fn fixture(
    resolution: &str,
    speed: f64,
    charge: Option<f64>,
    aip: Option<f64>,
    enabled: bool,
) -> World {
    load(scenario(
        "cn_039a", resolution, speed, 25.0, charge, aip, enabled,
    ))
    .unwrap()
}
fn id(w: &World) -> EntityId {
    w.entities.values().find(|e| e.name == "SUB").unwrap().id
}
fn state(w: &World) -> &b::UnitState {
    &w.rules.battery.as_ref().unwrap().units[&id(w)]
}
fn run(w: &mut World, to: i64) {
    while w.tick < to {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn issue(w: &mut World, action: b::Action) {
    let actor = id(w);
    b::validate_action(w, Side::Blue, actor, &action).unwrap();
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::Navigation {
                unit: actor,
                action: n::Action::Battery { action },
            },
        )],
    );
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-8, "{actual} != {expected}");
}

#[test]
fn native_tactical_debit_uses_one_source_column_only() {
    let mut w = fixture("tactical", 9.0, Some(100.0), Some(20.0), false);
    run(&mut w, 179);
    close(state(&w).charge_units.unwrap(), 100.0);
    run(&mut w, 180);
    close(state(&w).charge_units.unwrap(), 99.2);
    run(&mut w, 1800);
    close(state(&w).charge_units.unwrap(), 92.0);
    close(state(&w).aip_remaining_units.unwrap(), 20.0);
}

#[test]
fn native_intermediate_aip_uses_printed_nine_knot_example() {
    let mut w = fixture("intermediate", 9.0, Some(100.0), Some(20.0), true);
    run(&mut w, 1799);
    close(state(&w).charge_units.unwrap(), 100.0);
    run(&mut w, 1800);
    close(state(&w).charge_units.unwrap(), 95.0);
    close(state(&w).aip_remaining_units.unwrap(), 17.0);
}

#[test]
fn aip_four_knot_example_spends_two_units_without_battery_draw() {
    let mut w = fixture("tactical", 4.0, Some(100.0), Some(20.0), true);
    run(&mut w, 1800);
    close(state(&w).charge_units.unwrap(), 100.0);
    close(state(&w).aip_remaining_units.unwrap(), 18.0);
}

#[test]
fn switching_aip_off_does_not_erase_already_used_potential_or_retroactively_change_draw() {
    let mut w = fixture("tactical", 9.0, Some(100.0), Some(20.0), true);
    run(&mut w, 900);
    issue(&mut w, b::Action::SetAipOperating { enabled: false });
    run(&mut w, 1800);
    close(state(&w).charge_units.unwrap(), 93.5);
    close(state(&w).aip_remaining_units.unwrap(), 18.5);
}

#[test]
fn potential_smaller_than_one_output_is_not_overcredited() {
    let mut w = fixture("tactical", 9.0, Some(100.0), Some(0.2), true);
    run(&mut w, 1800);
    close(state(&w).charge_units.unwrap(), 92.2);
    close(state(&w).aip_remaining_units.unwrap(), 0.0);
    assert!(!state(&w).aip_operational);
}

#[test]
fn snorkel_actions_form_reachable_atomic_diesel_and_battery_transitions() {
    let mut w = fixture("tactical", 4.0, Some(100.0), Some(20.0), false);
    issue(&mut w, b::Action::RaiseSnorkel {});
    assert!(state(&w).mast_raised);
    assert_eq!(state(&w).power_mode, b::PowerMode::Diesel);
    issue(&mut w, b::Action::StartCharging {});
    let actor = id(&w);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: actor,
                speed_kts: 19.0,
            },
        )],
    );
    run(&mut w, 30);
    assert!(w.entity(actor).unwrap().speed_kts <= 15.0);
    issue(&mut w, b::Action::RetractMasts {});
    assert!(!state(&w).mast_raised);
    assert!(!state(&w).charging);
    assert_eq!(state(&w).power_mode, b::PowerMode::Battery);
}

#[test]
fn raising_snorkel_below_source_depth_and_wrong_side_are_read_only_rejections() {
    let w = load(scenario(
        "cn_039a",
        "tactical",
        4.0,
        100.0,
        Some(100.0),
        Some(20.0),
        false,
    ))
    .unwrap();
    let before = serde_json::to_vec(&w).unwrap();
    assert!(b::validate_action(&w, Side::Blue, id(&w), &b::Action::RaiseSnorkel {}).is_err());
    assert!(b::validate_action(
        &w,
        Side::Red,
        id(&w),
        &b::Action::SetAipOperating { enabled: true }
    )
    .is_err());
    assert_eq!(before, serde_json::to_vec(&w).unwrap());
}

#[test]
fn already_resolved_critical_contract_deduplicates_and_cannot_be_repaired_by_operating_order() {
    let mut w = fixture("tactical", 4.0, Some(100.0), Some(20.0), true);
    let actor = id(&w);
    b::critical_capacity_loss(&mut w, actor, "explicit-source-critical-1", 0.10).unwrap();
    b::critical_capacity_loss(&mut w, actor, "explicit-source-critical-1", 0.10).unwrap();
    close(state(&w).capacity_units, 225.0);
    close(state(&w).charge_units.unwrap(), 90.0);
    b::critical_aip_disable(&mut w, actor, "explicit-source-aip-critical-2").unwrap();
    let before = serde_json::to_vec(&w).unwrap();
    assert!(b::apply(
        &mut w,
        Side::Blue,
        actor,
        &b::Action::SetAipOperating { enabled: true }
    )
    .is_err());
    assert_eq!(before, serde_json::to_vec(&w).unwrap());
}

#[test]
fn native_pending_aip_reservation_survives_full_world_roundtrip() {
    let mut w = fixture("tactical", 9.0, Some(100.0), Some(20.0), true);
    run(&mut w, 911);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    run(&mut w, 1900);
    run(&mut restored, 1900);
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
}

#[test]
fn missing_current_charge_remains_unknown_and_zero_potential_cannot_start_operating() {
    let mut w = fixture("tactical", 9.0, None, Some(20.0), false);
    run(&mut w, 180);
    assert!(state(&w).charge_units.is_none());
    assert!(!state(&w).pending_adjudications.is_empty());
    assert!(load(scenario(
        "cn_039a",
        "tactical",
        4.0,
        25.0,
        Some(100.0),
        Some(0.0),
        true
    ))
    .is_err());
}

#[test]
fn native_streamed_towed_detection_keeps_two_mirrored_bearings_until_real_maneuver_and_reacquisition(
) {
    use harpoon_core::rules::detection2026 as d;
    const SENSOR: &str = "resume_lab_towed";
    let mut value = scenario(
        "cn_039a",
        "tactical",
        5.0,
        25.0,
        Some(100.0),
        Some(20.0),
        false,
    );
    value["units"][1]["pos"] = json!([0.5, 1.0]);
    value["units"][1]["depth_m"] = json!(25.0);
    value["context"]["detection_2026"] = json!({
        "optional_towed_array":{"enabled":true,"arrays":{(format!("cn_039a:{SENSOR}")):{"kind":"passive_towed","generation":6,"length_category":"long"}}},
        "bindings":{(format!("cn_039a:{SENSOR}")):{"type":"sonar","source":"scenario_assumption: experimental Gen6 LMF passive long towed fit","generation":6,"passive_range_nm":20.0,"active_range_nm":null,"frequency":"lmf","mount":"towed","active_torpedo_detection":false,"high_speed_tow":false}},
        "environment":{"source":"scenario_assumption: open deep water","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
        "targets":{"us_virginia":{"source":"scenario_assumption: controlled acoustic target; no real-class acoustic claim","size":2,"noise":0,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}});
    let scenario: Scenario = serde_json::from_value(value).unwrap();
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut sensor = db.sensors.values().next().unwrap().clone();
    sensor.id = SENSOR.into();
    sensor.name = "scenario_assumption: experimental towed receiver".into();
    sensor.kind = harpoon_core::db::SensorKind::SonarPassive;
    sensor.air_range_nm = 0.0;
    sensor.surface_range_nm = 0.0;
    sensor.missile_range_nm = 0.0;
    sensor.sub_range_nm = 20.0;
    db.sensors.insert(SENSOR.into(), sensor);
    db.platforms.get_mut("cn_039a").unwrap().sensors = vec![SENSOR.into()];
    db.platforms.get_mut("us_virginia").unwrap().sensors.clear();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let actor = id(&w);
    d::order_array(&mut w, actor, SENSOR, true, 25.0).unwrap();
    run(&mut w, 1440);
    let report = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .find_map(|t| {
            t.report
                .as_ref()
                .filter(|r| r.observer == actor && r.sensor == SENSOR)
        })
        .expect("actual native towed report checkpoint required");
    assert_eq!(report.bearing_alternatives.len(), 2);
    assert!(report.measured_position.is_none());
    close(
        (report.bearing_alternatives[0] + report.bearing_alternatives[1]).rem_euclid(360.0),
        0.0,
    );
    let target = report.target;
    assert!(!d::array_optional_world::fire_control_allowed(
        &w, actor, SENSOR, target
    ));
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetCourse {
                unit: actor,
                course_deg: 90.0,
            },
        )],
    );
    run(&mut w, 1540);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    run(&mut w, 2200);
    run(&mut restored, 2200);
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
    assert!(d::array_optional_world::fire_control_allowed(
        &w, actor, SENSOR, target
    ));
    let report = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .find_map(|t| {
            t.report
                .as_ref()
                .filter(|r| r.observer == actor && r.sensor == SENSOR)
        })
        .unwrap();
    assert!(report.bearing_alternatives.is_empty());
}

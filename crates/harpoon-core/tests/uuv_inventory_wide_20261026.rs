//! Source-red candidates: explicitly declared lifecycle laboratory. The body
//! uses an unchanged original submarine class, name, DP and ordnance; this is
//! not a claim that the real ship carries that submarine or that it is a real
//! Annex UUV. Mechanical timing/range and current fuel are explicit scene policy.
use harpoon_core::rules::{battery2026 as b, unmanned2026 as u};
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

fn scenario(stowed: bool) -> Value {
    let mut v = json!({"id":"wide-uuv-physical-inventory","name":"scenario_assumption: explicit original-body lifecycle experiment; no real UUV/launcher fit claim",
      "seed":20261026,"duration_s":12000,"weather":{"visibility_nm":0,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
        "battery_2026":{"source":"scenario_assumption: exact class capacity with experimental current state/rate; no UUV battery inference","resolution":"tactical","old_10kt_policy":null,
          "units":{"UUV_BODY":{"source":"scenario_assumption: original cn039a body with explicit lifecycle clock experiment, not a real UUV","platform_class":"cn_039a","initial_charge_units":50.,"initial_aip_remaining_units":20.,"initial_aip_operating":false,"initial_charging":false,"initial_power_mode":"battery","initial_mast_raised":false,"snorkel_fitted":true,"lithium_ion":false,"charge_rate_era":"post1970","full_speed_reference_kts":13.,"charge_combination_policy":"multiply_half_rate","aip_output_units_per_intermediate_turn":2.}}},
        "detection_2026":{"max_masts":{"cn_039a":1},"environment":{"source":"scenario_assumption: open water and explicit mast experiment","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
        "unmanned_2026":{"source":"scenario_assumption: original body and declared laboratory handling equipment, not a real autonomy audit",
          "controllers":{"HOST":{"source":"scenario_assumption: physical laboratory operator console","elements":["console"],"operational":true,"los_range_nm":100.,"antenna_height_m":30.,"satellite":true,"satellite_available":true,"acoustic_modem":true,"overt_recall_transmitter":true}},
          "vehicles":{"UUV_BODY":{"source":"scenario_assumption: original physical submarine used solely as a lifecycle harness; no real UUV equipment claim","domain":"underwater","autonomous_capable":true,"initial_mode":"hitl","controller":"HOST","element":"console","initial_link":"acoustic_modem","los_range_nm":100.,"antenna_height_m":10.,"satellite":true,"satellite_available":true,"acoustic_modem":true,"periscope_depth_m":15.,"endurance_s":10000.,"loss_behavior":{"type":"return_to_home","home":"HOST"}}}}},
      "units":[{"side":"Blue","class":"us_ford","name":"HOST","pos":[0,0],"course_deg":90,"speed_kts":0,"fire_on":false},
        {"side":"Blue","class":"cn_039a","name":"UUV_BODY","pos":[0,0],"course_deg":90,"speed_kts":0,"depth_m":15,"fire_on":false},
        {"side":"Red","class":"us_virginia","name":"DISTANT","pos":[400,400],"course_deg":0,"speed_kts":0,"depth_m":100,"fire_on":false}]});
    if stowed {
        v["context"]["unmanned_2026"]["launchers"] = json!({"HOST":{"lab":{
          "source":"scenario_assumption: lifecycle laboratory inventory, no physical real-hull capacity claim",
          "stowed":["UUV_BODY"],"launch_seconds":3,"recovery_seconds":3,"launch_depth_m":15.,
          "host_depth_min_m":0.,"host_depth_max_m":0.,"host_max_speed_kts":0.,"max_sea_state":0,
          "recovery_range_nm":2.,"recovery_max_depth_m":15.,"recovery_max_speed_kts":5.}}});
    }
    v
}
fn load(v: Value) -> World {
    let sc: Scenario = serde_json::from_value(v)
        .expect("future explicit physical inventory ABI, not invented runtime fields");
    harpoon_core::load_scenario(
        sc,
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap(),
    )
    .unwrap()
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn run(w: &mut World, to: i64) {
    while w.tick < to {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn issue(w: &mut World, actor: EntityId, action: Value) {
    let c: Command =
        serde_json::from_value(json!({"cmd":"unmanned","unit":actor,"action":action})).unwrap();
    c.validate_source(w, Side::Blue)
        .expect("actual source-qualified handling admission");
    harpoon_core::sim::tick(w, &[(Side::Blue, c)]);
}
fn inventory(w: &World, host: EntityId) -> Value {
    u::own_view(w, host).unwrap()["launchers"]["lab"].clone()
}
fn same(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} != {b}");
}

#[test]
fn uuv_i01_actual_stock_same_body_recovery_and_long_storage_freeze_all_energy_clocks() {
    let probe = load(scenario(false));
    let body = id(&probe, "UUV_BODY");
    let original = probe.entity(body).unwrap().clone();
    let mut w = load(scenario(true));
    let host = id(&w, "HOST");
    assert!(
        w.entity(body).is_none(),
        "a stowed original body is not an active sensor/movement entity or a dead casualty"
    );
    assert_eq!(inventory(&w, host)["stock"], 1);
    let command = json!({"type":"launch_uuv","launcher":"lab"});
    issue(&mut w, host, command);
    assert_eq!(inventory(&w, host)["reserved"], 1);
    let mut mid_job: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    let due = w.tick + 3;
    run(&mut w, due);
    run(&mut mid_job, due);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&mid_job).unwrap(),
        "restoring a real reserved launch must preserve body identity, clock and transaction"
    );
    let launched = w
        .entity(body)
        .expect("same original EntityId must physically enter the world");
    assert_eq!(launched.name, original.name);
    assert_eq!(launched.class, original.class);
    same(launched.hp, original.hp);
    assert_eq!(
        serde_json::to_value(&launched.weapons).unwrap(),
        serde_json::to_value(&original.weapons).unwrap()
    );
    assert_eq!(inventory(&w, host)["stock"], 0);
    issue(
        &mut w,
        host,
        json!({"type":"request_radio","vehicle":body,"link":"line_of_sight"}),
    );
    let deadline = w.tick + 2300;
    // Acoustic steering has no bulk own-unit telemetry. Completion becomes
    // owner-visible only after the actual radio route delivers that telemetry.
    while w.tick < deadline
        && !u::own_view(&w, body)
            .is_some_and(|view| view["radio_pending"] == false && view["link"] == "line_of_sight")
    {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(u::own_view(&w,body).unwrap()["radio_pending"],false,"real overt recall, 2D6 delay and actual PD/mast dwell must complete before the bulk-control order");
    let power:Command=serde_json::from_value(json!({"cmd":"navigation","unit":body,"action":{"type":"battery","action":{"type":"set_aip_operating","enabled":true}}})).unwrap();
    power.validate_source(&w, Side::Blue).unwrap();
    harpoon_core::sim::tick(
        &mut w,
        &[
            (Side::Blue, power),
            (
                Side::Blue,
                Command::SetSpeed {
                    unit: body,
                    speed_kts: 3.,
                },
            ),
        ],
    );
    assert!(
        w.rules.battery.as_ref().unwrap().units[&body].aip_operational,
        "the real power command must establish the configured original AIP"
    );
    // The first Tactical boundary contains pre-start battery-only samples.
    // Its mixed <4kt mean has no printed battery row, so it may correctly stay
    // pending. Earn one subsequent entire AIP Tactical interval, then preserve
    // a real partial interval away from the settlement edge. If that boundary
    // is also the Intermediate settlement, its reserved output is spent there
    // and the next Tactical interval supplies the persistent partial reserve.
    // Never inject output or infer an unprinted mixed-speed discharge rate.
    let tactical = b::tables()["turns"]["tactical_seconds"].as_i64().unwrap();
    let output_deadline = w.tick + 3 * tactical + 120;
    while w.tick < output_deadline {
        let s = &w.rules.battery.as_ref().unwrap().units[&body];
        if s.turn_sample_seconds >= 20
            && s.turn_sample_seconds < 120
            && s.intermediate_aip_spent_units > 0.
        {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.entity(body).unwrap().pos.range_to(original.pos) > 0.);
    assert!(
        w.rules.battery.as_ref().unwrap().units[&body].intermediate_aip_spent_units > 0.,
        "fixture must reserve real AIP output, never inject the reservation"
    );
    assert!(
        (20..120).contains(&w.rules.battery.as_ref().unwrap().units[&body].turn_sample_seconds),
        "fixture must retain a real partial active Tactical sample before recovery"
    );
    let fuel_before_recovery = w.entity(body).unwrap().endurance_s.unwrap();
    assert!(
        fuel_before_recovery < original.endurance_s.unwrap(),
        "real active operation must consume its original physical endurance"
    );
    issue(
        &mut w,
        host,
        json!({"type":"recover_uuv","launcher":"lab","vehicle":body}),
    );
    let recovery_due = w.tick + 3;
    run(&mut w, recovery_due);
    assert!(w.entity(body).is_none());
    assert_eq!(inventory(&w, host)["stock"], 1);
    let energy = w.rules.battery.as_ref().unwrap().units[&body].clone();
    let raw = serde_json::to_value(&w).unwrap();
    let mut restored: World = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(&restored).unwrap(), raw);
    let stored_until = w.tick + 1900;
    for state in [&mut w, &mut restored] {
        run(state, stored_until);
        let stored = &state.rules.battery.as_ref().unwrap().units[&body];
        assert_eq!(stored.charge_units, energy.charge_units);
        assert_eq!(stored.aip_remaining_units, energy.aip_remaining_units);
        assert_eq!(stored.turn_sample_seconds, energy.turn_sample_seconds);
        same(
            stored.intermediate_aip_spent_units,
            energy.intermediate_aip_spent_units,
        );
        issue(state, host, json!({"type":"launch_uuv","launcher":"lab"}));
        let launch_due = state.tick + 3;
        run(state, launch_due);
        let active = &state.rules.battery.as_ref().unwrap().units[&body];
        assert_eq!(
            active.charge_units, energy.charge_units,
            "storage gap cannot settle phantom battery work"
        );
        assert_eq!(
            active.aip_remaining_units, energy.aip_remaining_units,
            "no phantom AIP use/output or reservation refund"
        );
        same(
            active.intermediate_aip_spent_units,
            energy.intermediate_aip_spent_units,
        );
        assert!(active.turn_sample_seconds <= energy.turn_sample_seconds + 4);
        assert!(!active.charging && !active.aip_operational && !active.mast_raised);
        assert!(state
            .rules
            .detection
            .as_ref()
            .unwrap()
            .masts
            .get(&body)
            .is_none_or(|m| m.count == 0));
        assert!(
            !u::allow_side_projection(state, body),
            "relaunch must not inherit an established radio path"
        );
        assert!(
            u::own_view(state, body).is_none(),
            "old bulk telemetry cannot reappear during acoustic-only redeployment"
        );
        assert_eq!(state.entity(body).unwrap().name, original.name);
        let relaunched_fuel = state.entity(body).unwrap().endurance_s.unwrap();
        assert!(
            relaunched_fuel <= fuel_before_recovery
                && relaunched_fuel >= fuel_before_recovery - 10.,
            "the 1900s storage gap must neither consume nor refill original active endurance"
        );
        same(state.entity(body).unwrap().hp, original.hp);
        assert_eq!(
            serde_json::to_value(&state.entity(body).unwrap().weapons).unwrap(),
            serde_json::to_value(&original.weapons).unwrap()
        );
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn uuv_i03_reserved_launch_element_cannot_be_taken_by_a_pending_control_switch() {
    let mut v = scenario(true);
    let mut other = v["context"]["unmanned_2026"]["vehicles"]["UUV_BODY"].clone();
    other["controller"] = Value::Null;
    other["element"] = Value::Null;
    other["initial_mode"] = json!("autonomous");
    v["context"]["unmanned_2026"]["vehicles"]["OTHER_BODY"] = other;
    v["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":"cn_039a","name":"OTHER_BODY","pos":[0,0],"course_deg":90,"speed_kts":0,"depth_m":15,"fire_on":false}));
    let mut w = load(v);
    let host = id(&w, "HOST");
    let other = id(&w, "OTHER_BODY");
    let take:Command=serde_json::from_value(json!({"cmd":"unmanned","unit":other,
        "action":{"type":"take_control","controller":host,"element":"console","link":"acoustic_modem"}})).unwrap();
    assert!(
        take.validate_source(&w, Side::Blue).is_ok(),
        "the declared target console is originally available"
    );
    issue(&mut w, host, json!({"type":"launch_uuv","launcher":"lab"}));
    let raw = serde_json::to_value(&w).unwrap();
    let reason = take
        .validate_source(&w, Side::Blue)
        .expect_err("reserved physical launch owns its original control slot");
    assert!(reason.contains("physical UUV launch"));
    assert_eq!(serde_json::to_value(&w).unwrap(), raw);
}

#[test]
fn uuv_i04_authored_mechanics_cannot_launch_beyond_original_hull_depth() {
    let mut v = scenario(true);
    v["context"]["unmanned_2026"]["launchers"]["HOST"]["lab"]["launch_depth_m"] = json!(100000.);
    let sc: Scenario = serde_json::from_value(v).unwrap();
    let result = harpoon_core::load_scenario(
        sc,
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap(),
    );
    let reason = result.expect_err(
        "scenario policy cannot manufacture a different original hull's pressure capability",
    );
    assert!(reason.contains("original physical body capability"));
}

#[test]
fn uuv_i02_invalid_source_or_reserved_inventory_admission_is_completely_readonly() {
    let mut w = load(scenario(true));
    let host = id(&w, "HOST");
    let enemy = id(&w, "DISTANT");
    for (side, actor, action) in [
        (
            Side::Blue,
            host,
            json!({"type":"launch_uuv","launcher":"missing"}),
        ),
        (
            Side::Red,
            host,
            json!({"type":"launch_uuv","launcher":"lab"}),
        ),
        (
            Side::Blue,
            host,
            json!({"type":"recover_uuv","launcher":"lab","vehicle":enemy}),
        ),
    ] {
        let raw = serde_json::to_value(&w).unwrap();
        let c: Command =
            serde_json::from_value(json!({"cmd":"unmanned","unit":actor,"action":action})).unwrap();
        assert!(c.validate_source(&w, side).is_err());
        assert_eq!(serde_json::to_value(&w).unwrap(), raw);
    }
    issue(&mut w, host, json!({"type":"launch_uuv","launcher":"lab"}));
    let raw = serde_json::to_value(&w).unwrap();
    let c: Command = serde_json::from_value(
        json!({"cmd":"unmanned","unit":host,"action":{"type":"launch_uuv","launcher":"lab"}}),
    )
    .unwrap();
    assert!(
        c.validate_source(&w, Side::Blue).is_err(),
        "a reserved body cannot be borrowed a second time"
    );
    assert_eq!(serde_json::to_value(&w).unwrap(), raw);
    assert!(w.entities.values().all(|e| e.name != "UUV_BODY"));
    assert!(b::own_view(&w, host).is_none());
}

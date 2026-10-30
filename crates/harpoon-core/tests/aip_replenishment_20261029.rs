//! Source §3.2.5 return-to-port requirement; logistics quantities/timing are
//! explicit fictional scenario contracts. Public commands, real movement,
//! real finite stock and save/restore only; no runtime state injection.
use harpoon_core::rules::battery2026::{self as battery, Action};
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};
fn scenario(stock: f64) -> Value {
    let unit = json!({"source":"scenario_assumption: explicit test energy state","platform_class":"cn_039a","initial_charge_units":100.,"initial_aip_remaining_units":0.,"initial_aip_operating":false,"initial_charging":false,"initial_power_mode":"diesel","initial_mast_raised":false,"snorkel_fitted":true,"lithium_ion":false,"charge_rate_era":"post1970","full_speed_reference_kts":19.,"charge_combination_policy":null,"aip_output_units_per_intermediate_turn":null});
    json!({"id":"aip-port-contract","name":"scenario_assumption: finite AIP port supply","seed":29,"duration_s":4000,"weather":{"visibility_nm":0,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
       "ground_2026":{"source":"scenario_assumption: physical test port","facilities":{"DEPOT":{"source":"scenario_assumption: port supply room","template":"magazine_bunker","parent":"PORT"}},"complexes":{"PORT":{"source":"scenario_assumption: game port","kind":"Port","components":{"PortFacilities":["DEPOT"]}}}},
       "battery_2026":{"source":"scenario_assumption: laboratory state","resolution":"intermediate","units":{"SUB1":unit.clone(),"SUB2":unit},
        "replenishment":{"source":"scenario_assumption: explicit work and paired supply equivalents","depots":{"SUPPLY":{"source":"scenario_assumption: finite fuel and oxygen inventory, 3 second job, 0.2nm berth","port":"PORT","facility":"DEPOT","maximum_distance_nm":0.2,"work_seconds":3,"fuel_units":stock,"oxygen_units":stock}}}}},
      "units":[{"side":"Blue","class":"cn_039a","name":"SUB1","pos":[0,0],"depth_m":0,"speed_kts":0,"fire_on":false},
       {"side":"Blue","class":"cn_039a","name":"SUB2","pos":[0,0.18],"depth_m":0,"speed_kts":0,"fire_on":false},
       {"side":"Blue","class":"airfield","name":"PORT","pos":[0,0],"fire_on":false},
       {"side":"Blue","class":"airfield","name":"DEPOT","pos":[0,0],"fire_on":false},
       {"side":"Red","class":"us_virginia","name":"DISTANT","pos":[300,300],"depth_m":100,"speed_kts":0,"fire_on":false}]})
}
fn load(v: Value) -> World {
    harpoon_core::load_scenario(
        serde_json::from_value::<Scenario>(v).unwrap(),
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap(),
    )
    .unwrap()
}
fn command(id: u32, action: Action) -> Command {
    Command::Navigation {
        unit: EntityId(id),
        action: harpoon_core::rules::navigation2026::Action::Battery { action },
    }
}
fn request(id: u32) -> Command {
    command(
        id,
        Action::StartReplenishment {
            provider: "SUPPLY".into(),
            units: 10.,
        },
    )
}
fn tick(w: &mut World, c: &[(Side, Command)]) {
    harpoon_core::sim::tick(w, c);
}
fn own(w: &World, id: u32) -> Value {
    battery::own_view(w, EntityId(id)).unwrap()
}
#[test]
fn actual_port_work_debits_both_stocks_once_and_survives_save() {
    let mut w = load(scenario(15.));
    let c = request(1);
    let before = serde_json::to_vec(&w).unwrap();
    c.validate_source(&w, Side::Blue).unwrap();
    assert_eq!(before, serde_json::to_vec(&w).unwrap());
    tick(&mut w, &[(Side::Blue, c.clone()), (Side::Blue, c)]);
    assert_eq!(own(&w, 1)["aip_remaining_units"], 0.);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..4 {
        tick(&mut w, &[]);
        tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    let v = own(&w, 1);
    assert_eq!(v["aip_remaining_units"], 10.);
    assert_eq!(v["aip_operational"], false);
    assert_eq!(v["charge_units"], 100.);
    let s = w
        .rules
        .battery
        .as_ref()
        .unwrap()
        .replenishment
        .as_ref()
        .unwrap();
    assert_eq!(s.depots["SUPPLY"].fuel_units, 5.);
    assert_eq!(s.depots["SUPPLY"].oxygen_units, 5.);
}
#[test]
fn contention_is_sorted_before_atomic_reservation_and_cancel_refunds() {
    let mut a = load(scenario(15.));
    let mut b = load(scenario(15.));
    tick(
        &mut a,
        &[(Side::Blue, request(2)), (Side::Blue, request(1))],
    );
    tick(
        &mut b,
        &[(Side::Blue, request(1)), (Side::Blue, request(2))],
    );
    assert_eq!(
        serde_json::to_value(&a.rules.battery).unwrap(),
        serde_json::to_value(&b.rules.battery).unwrap()
    );
    assert!(a
        .rules
        .battery
        .as_ref()
        .unwrap()
        .replenishment
        .as_ref()
        .unwrap()
        .jobs
        .contains_key(&EntityId(1)));
    tick(
        &mut a,
        &[(Side::Blue, command(1, Action::CancelReplenishment {}))],
    );
    let s = a
        .rules
        .battery
        .as_ref()
        .unwrap()
        .replenishment
        .as_ref()
        .unwrap();
    assert_eq!(s.depots["SUPPLY"].fuel_units, 15.);
    assert_eq!(s.depots["SUPPLY"].oxygen_units, 15.);
    assert_eq!(own(&a, 1)["aip_remaining_units"], 0.);
}
#[test]
fn wrong_side_or_submerged_port_work_cannot_mutate_or_refill() {
    let mut v = scenario(15.);
    v["units"][0]["depth_m"] = json!(25.);
    v["context"]["battery_2026"]["units"]["SUB1"]["initial_power_mode"]=json!("battery");
    let w = load(v);
    let before = serde_json::to_vec(&w).unwrap();
    assert!(request(1).validate_source(&w, Side::Blue).is_err());
    assert!(request(2).validate_source(&w, Side::Red).is_err());
    assert_eq!(before, serde_json::to_vec(&w).unwrap());
}
#[test]
fn actual_departure_aborts_work_and_refunds_both_supplies() {
    let mut w = load(scenario(15.));
    tick(&mut w, &[(Side::Blue, request(1))]);
    let before = w.entity(EntityId(1)).unwrap().pos;
    tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: EntityId(1),
                speed_kts: 4.,
            },
        )],
    );
    assert!(w.entity(EntityId(1)).unwrap().speed_kts > 0.);
    assert!(w.entity(EntityId(1)).unwrap().pos.range_to(before) > 0.);
    let s = w
        .rules
        .battery
        .as_ref()
        .unwrap()
        .replenishment
        .as_ref()
        .unwrap();
    assert!(s.jobs.is_empty());
    assert_eq!(s.depots["SUPPLY"].fuel_units, 15.);
    assert_eq!(s.depots["SUPPLY"].oxygen_units, 15.);
    assert_eq!(own(&w, 1)["aip_remaining_units"], 0.);
}

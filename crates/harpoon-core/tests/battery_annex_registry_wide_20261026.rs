//! Additional field-scope and old-save guards; original14 source-red tests stay frozen.
use harpoon_core::rules::{battery2026 as b, submarine_annex as a};
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};
fn value() -> Value {
    json!({"id":"battery-wide-registry","name":"scenario_assumption: source applicability regression","seed":26,"duration_s":10000,
      "weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4",
       "radar_environment":{"sea_state":0,"precipitation":"None"},
       "battery_2026":{"source":"scenario_assumption: source-row capacity with explicit runtime initial state","resolution":"tactical","units":{"SUB":{
         "source":"scenario_assumption: declared chemistry/snorkel/era/current energy, not an Annex equipment claim","platform_class":"cn_039a",
         "initial_charge_units":50,"initial_aip_remaining_units":null,"initial_aip_operating":false,"initial_charging":false,
         "initial_power_mode":"battery","initial_mast_raised":false,"snorkel_fitted":true,"lithium_ion":false,
         "charge_rate_era":"post1970","full_speed_reference_kts":13,"charge_combination_policy":"multiply_half_rate","aip_output_units_per_intermediate_turn":null}}}},
      "units":[{"side":"Blue","class":"cn_039a","name":"SUB","pos":[0,0],"course_deg":0,"speed_kts":4,"depth_m":25,"fire_on":false},
       {"side":"Red","class":"us_virginia","name":"DISTANT","pos":[400,400],"speed_kts":0,"depth_m":100,"fire_on":false}]})
}
fn db() -> Database { Database::from_json(include_str!("../../../database/consolidated.json")).unwrap() }
fn load(v: Value) -> World {harpoon_core::load_scenario(serde_json::from_value::<Scenario>(v).unwrap(),db()).unwrap()}
fn id(w: &World) -> EntityId {w.entities.values().find(|e|e.name=="SUB").unwrap().id}
fn order(w: &mut World, action:b::Action) {
    let c=Command::Navigation{unit:id(w),action:harpoon_core::rules::navigation2026::Action::Battery{action}};
    c.validate_source(w,Side::Blue).unwrap();harpoon_core::sim::tick(w,&[(Side::Blue,c)]);
}
#[test]
fn r01_runtime_field_permission_does_not_turn_depth_qualification_into_conventional_or_named_battery() {
    let w=load(value());
    assert_eq!(a::tables()["platforms"].as_object().unwrap().len(),12);
    assert!(a::battery_fit(&w,"se_ss_a19").is_some(),"new exact Swedish base-row battery permission");
    assert!(a::field_applicability(&w,"se_ss_a19","maximum_game_depth_m").is_none(),"battery-only registration cannot confer a depth field");
    assert!(a::field_applicability(&w,"se_ss_a19","surface_max_kts").is_none());
    for class in ["cn_039a","cn_kilo","cn_039g","ru_kilo","ru_kilo_636","ru_lada"] {
        assert!(a::battery_fit(&w,class).is_some(),"actual consumed field permission missing for {class}");
    }
    for class in ["us_virginia","us_virginia_blk5","us_seawolf","us_ohio_ssgn","us_ohio_ssbn"] {
        assert!(a::capability(&w,class).is_some());
        assert!(a::battery_fit(&w,class).is_none(),"5Emerg nuclear role is not a conventional fit");
        assert_eq!(a::field_applicability(&w,class,"battery_fit").unwrap()["runtime_fit"],"not_applicable_to_conventional_executor");
    }
    assert!(a::battery_fit(&w,"Project677").is_none());
    assert!(a::battery_fit(&w,"ru_lada_kronshtadt").is_none());
}
#[test]
fn r02_actual_repeat_raise_preserves_exposure_age_and_old_missing_timestamp_remains_unknown() {
    let mut w=load(value());let actor=id(&w);
    order(&mut w,b::Action::RaiseSnorkel{});
    let first=b::raised_snorkel(&w,actor).unwrap();assert_eq!(first,(true,Some(1)));
    order(&mut w,b::Action::RaiseSnorkel{});assert_eq!(b::raised_snorkel(&w,actor),Some(first));
    // Explicit old serialization compatibility experiment: remove only the
    // field absent in old saves, not a physical state/position/contact/draw.
    let mut saved=serde_json::to_value(&w).unwrap();
    saved["rules"]["battery"]["units"][actor.0.to_string()].as_object_mut().unwrap().remove("mast_raised_since");
    let mut old:World=serde_json::from_value(saved).unwrap();
    assert_eq!(b::raised_snorkel(&old,actor),Some((true,None)));
    order(&mut old,b::Action::RaiseSnorkel{});assert_eq!(b::raised_snorkel(&old,actor),Some((true,None)));
    order(&mut old,b::Action::RetractMasts{});assert_eq!(b::raised_snorkel(&old,actor),Some((false,None)));
    order(&mut old,b::Action::RaiseSnorkel{});assert_eq!(b::raised_snorkel(&old,actor),Some((true,Some(old.tick))));
}
#[test]
fn r03_exact_annex_name_cannot_authorize_a_real_non_submarine_body() {
    let mut database=db();database.platforms.get_mut("cn_039a").unwrap().kind=harpoon_core::types::PlatformKind::Base;
    assert!(harpoon_core::load_scenario(serde_json::from_value::<Scenario>(value()).unwrap(),database).is_err());
}

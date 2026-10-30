use harpoon_core::rules::{battery2026 as b, navigation2026 as n};
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

fn value(class: &str, charge: f64, resolution: &str) -> Value {
    json!({"id":"battery-annex-next-20261028","name":"scenario_assumption: printed-rate battery consumers",
        "seed":20261028,"duration_s":12000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "battery_2026":{"source":"scenario_assumption: exact Annex capacity, explicit laboratory energy state",
                "resolution":resolution,"old_10kt_policy":null,"units":{"SUB":{
                    "source":"scenario_assumption: declared initial charge/AIP/snorkel/chemistry/era and13kt full-speed reference; not Annex hardware",
                    "platform_class":class,"initial_charge_units":charge,
                    "initial_aip_remaining_units":null,
                    "initial_aip_operating":false,"initial_charging":false,
                    "initial_power_mode":"battery","initial_mast_raised":false,
                    "snorkel_fitted":true,"lithium_ion":false,"charge_rate_era":"post1970",
                    "full_speed_reference_kts":13.0,"charge_combination_policy":"multiply_half_rate",
                    "aip_output_units_per_intermediate_turn":null}}}},
        "units":[{"side":"Blue","class":class,"name":"SUB","pos":[0,0],"course_deg":0,"speed_kts":4,"depth_m":25,"fire_on":false},
            {"side":"Red","class":"us_virginia","name":"DISTANT","pos":[400,400],"course_deg":0,"speed_kts":0,"depth_m":100,"fire_on":false}]})
}
fn cfg(v: &mut Value) -> &mut Value { &mut v["context"]["battery_2026"]["units"]["SUB"] }
fn db() -> Database {
    Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap()
}
fn load(v: Value) -> Result<World,String> {
    let sc: Scenario=serde_json::from_value(v).map_err(|e|e.to_string())?;
    harpoon_core::load_scenario(sc,db())
}
fn id(w: &World,name: &str) -> EntityId {w.entities.values().find(|e|e.name==name).unwrap().id}
fn own(w: &World) -> Value {b::own_view(w,id(w,"SUB")).unwrap()}
fn close(a: f64,e: f64) {assert!((a-e).abs()<1e-9,"{a} != {e}");}
fn run(w: &mut World,to: i64) {while w.tick<to {assert!(!w.over);harpoon_core::sim::tick(w,&[]);}}
fn order(w: &mut World,action: b::Action) {
    let c=Command::Navigation{unit:id(&w,"SUB"),action:n::Action::Battery{action}};
    c.validate_source(w,Side::Blue).unwrap();
    harpoon_core::sim::tick(w,&[(Side::Blue,c)]);
}
fn charging(v: &mut Value) {
    v["units"][0]["depth_m"]=json!(5);
    cfg(v)["initial_charge_units"]=json!(175.0);
    cfg(v)["initial_mast_raised"]=json!(true);
    cfg(v)["initial_power_mode"]=json!("diesel");
    cfg(v)["initial_charging"]=json!(true);
}


#[test]
fn exact_new_rows_drive_capacity_aip_and_public_commands() {
    for (class, cap, aip) in [("se_ss_a17s",150.0,true),("au_collins",240.0,false),("ca_victoria",150.0,false)] {
        let mut v=value(class,cap,"intermediate");
        if aip { cfg(&mut v)["initial_aip_remaining_units"]=json!(2000.0); }
        let mut w=load(v.clone()).expect("qualified exact row must initialize actual battery state");
        let annex = harpoon_core::rules::submarine_annex::field_applicability;
        assert!(annex(&w,class,"battery_fit").is_some());
        assert!(annex(&w,class,"maximum_game_depth_m").is_none());
        assert!(annex(&w,"unknown_class","battery_fit").is_none());
        let mut legacy=w.clone();
        legacy.rules.profile=harpoon_core::rules::damage_runtime::RuleProfile::Legacy;
        assert!(annex(&legacy,class,"battery_fit").is_none());
        let fit=harpoon_core::rules::submarine_annex::battery_fit(&w,class).unwrap();
        assert_eq!(fit["rating_charge_units"].as_f64(),Some(cap));
        assert_eq!(fit["technology_as_printed"],json!("new"));
        assert_eq!(fit["aip_fit"],json!(aip));
        let actor=id(&w,"SUB");
        let cmd=Command::Navigation{unit:actor,action:n::Action::Battery{action:b::Action::SetAipOperating{enabled:true}}};
        assert_eq!(cmd.validate_source(&w,Side::Blue).is_ok(),aip);
        if aip { harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]); assert_eq!(own(&w)["aip_operational"],json!(true)); }
        cfg(&mut v)["initial_charge_units"]=json!(cap+1.0);
        assert!(load(v).is_err(),"source capacity enforces upper bound");
    }
    let mut v=value("se_ss_a17s",100.0,"intermediate");
    cfg(&mut v)["initial_aip_remaining_units"]=json!(2400.0);
    assert!(load(v).err().unwrap().contains("source_pending"));
}
#[test]
fn qualified_new_rows_use_real_charge_and_stow_consumer() {
    for class in ["au_collins","ca_victoria","se_ss_a17s"] {
        let mut v=value(class,0.0,"intermediate");
        v["units"][0]["depth_m"]=json!(5.0);
        cfg(&mut v)["initial_mast_raised"]=json!(true);
        cfg(&mut v)["initial_power_mode"]=json!("diesel");
        cfg(&mut v)["initial_charging"]=json!(true);
        let mut w=load(v).unwrap(); run(&mut w,1800);
        assert!(own(&w)["charge_units"].as_f64().unwrap()>0.0);
        let actor=id(&w,"SUB"); b::pause_stowed(&mut w,actor).unwrap();
        assert_eq!(own(&w)["charging"],json!(false));
    }
}

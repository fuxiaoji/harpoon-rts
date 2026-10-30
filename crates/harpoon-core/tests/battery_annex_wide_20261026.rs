//! Wide-batch source candidates, not compiled/run by this writer.
//! Real load/tick/public commands; initial energy/chemistry/snorkel/denominator
//! are declared experiments. Original DB files and performance stay unchanged.
use harpoon_core::rules::{battery2026 as b, detection2026 as d, navigation2026 as n};
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

fn value(class: &str, charge: Option<f64>, aip: bool) -> Value {
    json!({"id":"wide-battery-annex-20261026","name":"scenario_assumption: explicit energy consumers",
        "seed":20261026,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "battery_2026":{"source":"scenario_assumption: exact Annex capacity, explicit laboratory current state",
                "resolution":"tactical","old_10kt_policy":null,"units":{"SUB":{
                    "source":"scenario_assumption: declared initial charge/AIP, snorkel, chemistry, era and13kt full-speed reference; not Annex hardware",
                    "platform_class":class,"initial_charge_units":charge,
                    "initial_aip_remaining_units":if aip {json!(20.0)} else {Value::Null},
                    "initial_aip_operating":aip,"initial_charging":false,
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
fn close(a: f64,b: f64) {assert!((a-b).abs()<1e-8,"{a} != {b}");}
fn run(w: &mut World,to: i64) {
    while w.tick<to {assert!(!w.over);harpoon_core::sim::tick(w,&[]);}
}
fn cmd(w: &World,action: b::Action) -> Command {
    Command::Navigation{unit:id(w,"SUB"),action:n::Action::Battery{action}}
}
fn order(w: &mut World,action: b::Action) {
    let c=cmd(w,action);c.validate_source(w,Side::Blue).unwrap();
    harpoon_core::sim::tick(w,&[(Side::Blue,c)]);
}
fn charging(lithium: Option<bool>) -> World {
    let mut v=value("cn_039a",Some(50.0),false);
    cfg(&mut v)["initial_mast_raised"]=json!(true);
    cfg(&mut v)["initial_power_mode"]=json!("diesel");
    cfg(&mut v)["initial_charging"]=json!(true);
    cfg(&mut v)["lithium_ion"]=json!(lithium);
    load(v).unwrap()
}

#[test]
fn w01_exact_039g1_annex_165new_row_initializes_and_runs_the_real_battery_consumer() {
    let mut w=load(value("cn_039g",Some(100.0),false))
        .expect("CN2025 PDF12/A5 Type039G1 battery165new is not neighboring039/039G150new");
    assert_eq!(own(&w)["capacity_units"],165.0);
    assert_eq!(own(&w)["aip_fitted"],false);
    run(&mut w,180);close(own(&w)["charge_units"].as_f64().unwrap(),99.8);
}
#[test]
fn w02_generic_677_300new_does_not_borrow_the_two_named_500_lithium_variants() {
    let mut w=load(value("ru_lada",Some(250.0),false))
        .expect("RU2025 PDF22/A19 generic677 prints300new; named500 cells are not the generic fit");
    assert_eq!(own(&w)["capacity_units"],300.0);
    assert_eq!(own(&w)["aip_fitted"],false);
    run(&mut w,180);close(own(&w)["charge_units"].as_f64().unwrap(),249.8);
}
#[test]
fn w03_exact_id_and_nuclear_emergency_rating_do_not_authorize_conventional_capacity() {
    let mut v=value("cn_039a",Some(100.0),false);
    cfg(&mut v)["platform_class"]=json!("Type039A");
    assert!(load(v).is_err(),"a display-name alias is not an exact stable Annex class");
    assert!(load(value("us_virginia",Some(5.0),false)).is_err(),
        "Annex5(Emerg) on a nuclear submarine is not a conventional diesel battery fit");
}
#[test]
fn w04_explicit_lithium_charge_multiplier_is_real_and_unknown_chemistry_stays_pending() {
    let mut ordinary=charging(Some(false));let mut lithium=charging(Some(true));let mut unknown=charging(None);
    run(&mut ordinary,1800);run(&mut lithium,1800);run(&mut unknown,1800);
    close(own(&ordinary)["charge_units"].as_f64().unwrap(),66.0);
    close(own(&lithium)["charge_units"].as_f64().unwrap(),82.0);
    close(own(&unknown)["charge_units"].as_f64().unwrap(),50.0);
    assert!(!own(&unknown)["pending_adjudications"].as_array().unwrap().is_empty());
}
#[test]
fn w05_speed_after_charging_stops_cannot_retroactively_halve_the_earned_interval() {
    let mut w=charging(Some(false));run(&mut w,900);
    // First900 eligible seconds really run4kt, below the declared13/2 gate.
    // The following900 seconds are not charging and cannot change that rate.
    order(&mut w,b::Action::StopCharging{});
    let actor=id(&w,"SUB");
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::SetSpeed{unit:actor,speed_kts:15.0})]);
    run(&mut w,1800);
    assert!(w.entity(actor).unwrap().speed_kts>13.0/2.0);
    close(own(&w)["charge_units"].as_f64().unwrap(),58.0);
}
#[test]
fn w06_native_loss_does_not_continue_spending_a_dead_submarines_energy() {
    let mut w=load(value("cn_039a",Some(100.0),false)).unwrap();run(&mut w,179);
    let actor=id(&w,"SUB");let before=own(&w);
    harpoon_core::damage::sink_by(&mut w,actor,Side::Red);
    harpoon_core::sim::tick(&mut w,&[]);
    assert!(!w.entity(actor).unwrap().alive);
    assert_eq!(own(&w)["charge_units"],before["charge_units"],"retained wreck is not an operating battery consumer");
}
#[test]
fn w07_one_named_configuration_must_not_silently_select_one_of_two_real_entities() {
    let mut v=value("cn_039a",Some(100.0),false);
    let mut other=v["units"][0].clone();other["pos"]=json!([2,0]);
    v["units"].as_array_mut().unwrap().push(other);
    assert!(load(v).is_err(),"one exact-name battery fit cannot silently bind the first duplicate entity");
}
#[test]
fn w08_known_empty_battery_without_aip_cannot_keep_sustained_submerged_propulsion() {
    let mut w=load(value("cn_039a",Some(0.0),false)).unwrap();let actor=id(&w,"SUB");
    run(&mut w,180);
    assert_eq!(own(&w)["charge_units"],0.0);
    assert!(w.entity(actor).unwrap().speed_kts<=1e-9,
        "PDF30 depleted batteries must surface/snorkel; no automatic depth order is supplied by this test");
    assert_eq!(w.entity(actor).unwrap().ordered_depth_m,25.0,"energy gate must not invent an automatic surfacing order");
}
#[test]
fn w09_real_low_speed_aip_keeps_an_empty_battery_submarine_moving_with_actual_budget() {
    let mut w=load(value("cn_039a",Some(0.0),true)).unwrap();let actor=id(&w,"SUB");
    let before=w.entity(actor).unwrap().pos;run(&mut w,1800);
    assert!(w.entity(actor).unwrap().pos.range_to(before)>1.0);
    close(own(&w)["charge_units"].as_f64().unwrap(),0.0);
    close(own(&w)["aip_remaining_units"].as_f64().unwrap(),18.0);
}
#[test]
fn w10_unknown_charge_is_not_relabelled_as_zero_in_state_or_observation() {
    let mut w=load(value("cn_039a",None,false)).unwrap();run(&mut w,180);
    assert!(own(&w)["charge_units"].is_null());
    assert!(!own(&w)["pending_adjudications"].as_array().unwrap().is_empty());
}

fn mast_fixture() -> World {
    let mut v=value("cn_039a",Some(100.0),false);v["units"][0]["speed_kts"]=json!(0.0);
    v["units"][1]=json!({"side":"Red","class":"us_ford","name":"WATCH","pos":[0.01,0],"speed_kts":0,"fire_on":false});
    let profile=|size|json!({"source":"scenario_assumption: controlled physical visual target profile","size":size,"noise":3,
        "anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    v["context"]["detection_2026"]=json!({
        "bindings":{"us_ford:visual":{"type":"visual","source":"scenario_assumption: actually fitted surface lookout"}},
        "targets":{"us_ford":profile(0),"cn_039a":profile(2)},
        "environment":{"source":"scenario_assumption: clear day open sea, no clouds or ice","sea_state":0,"precipitation":0,
            "visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}});
    let mut database=db();database.platforms.get_mut("us_ford").unwrap().sensors=vec!["visual".into()];
    database.platforms.get_mut("cn_039a").unwrap().sensors.clear();
    harpoon_core::load_scenario(serde_json::from_value(v).unwrap(),database).unwrap()
}
fn visual(w: &World) -> Result<d::ScanDecision,String> {
    let mut receiver_state=w.rules.detection.as_ref().unwrap().clone();
    d::evaluate_world(w,&mut receiver_state,id(w,"WATCH"),"visual",id(w,"SUB"))
}
#[test]
fn w11_actual_visual_consumer_reads_the_physically_raised_battery_snorkel() {
    let mut w=mast_fixture();order(&mut w,b::Action::RaiseSnorkel{});
    assert_eq!(own(&w)["mast_raised"],true);
    let scan=visual(&w).expect("native5.8.5 must see the same actual battery-raised snorkel, not demand an unrelated author mast command");
    assert!(scan.trace.get("mast").is_some(),"real periscope/snorkel source probability consumer is reached");
}
#[test]
fn w12_retraction_removes_only_the_actual_snorkel_exposure_without_fabricating_contacts() {
    let mut w=mast_fixture();order(&mut w,b::Action::RaiseSnorkel{});order(&mut w,b::Action::RetractMasts{});
    assert_eq!(own(&w)["mast_raised"],false);
    assert!(visual(&w).map_or(true,|s|!s.detected));
}
#[test]
fn w13_deep_order_with_an_exposed_snorkel_requires_explicit_retraction_first() {
    let mut w=load(value("cn_039a",Some(100.0),false)).unwrap();order(&mut w,b::Action::RaiseSnorkel{});
    let actor=id(&w,"SUB");let deep=Command::SetDepth{unit:actor,depth_m:100.0};
    let before=serde_json::to_value(&w).unwrap();
    assert!(deep.validate_source(&w,Side::Blue).is_err(),"an exposed source snorkel cannot be carried into a deep dive by an unrelated command");
    assert_eq!(serde_json::to_value(&w).unwrap(),before);
    order(&mut w,b::Action::RetractMasts{});
    assert!(deep.validate_source(&w,Side::Blue).is_ok());
}
#[test]
fn w14_charging_interval_samples_survive_full_world_restore_without_double_gain() {
    let mut w=charging(Some(true));run(&mut w,911);
    let mut restored: World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    run(&mut w,1900);run(&mut restored,1900);
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
}

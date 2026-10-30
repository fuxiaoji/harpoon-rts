//! battery-annex-next candidates (2026-10-28). Public JSON/API only:
//! normal construction, movement, settlement and save/read. No runtime
//! truth, no fabricated inventory, no private state writes. Cases 1-3
//! are acceptance cases for already-implemented production behavior;
//! case 4 is the deliberate source RED for the se_ss_a19 fit row that
//! only root may register. Unprinted inputs stay null.
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

// B09-a: the printed charge-rate column headers are exactly "0-70%" and
// "71%+" (rules PDF29/3-7). Exact printed-band fractions settle at the
// printed rates; a continuous value inside the 70..71 gap has no printed
// rate and stays pending without interpolation.
#[test]
fn b09_exact_band_fractions_settle_printed_rates_and_the_70_71_gap_stays_pending() {
    let mut low=load({let mut v=value("cn_039a",50.0,"intermediate");charging(&mut v);
        cfg(&mut v)["initial_charge_units"]=json!(175.0);v})
        .expect("175.0/250 is exactly the printed 0-70% band edge");
    run(&mut low,1800);
    close(own(&low)["charge_units"].as_f64().unwrap(),191.0); // band0 +16/IT
    run(&mut low,3600);
    close(own(&low)["charge_units"].as_f64().unwrap(),199.0); // 76.4% -> band1 +8

    let mut high=load({let mut v=value("cn_039a",50.0,"intermediate");charging(&mut v);
        cfg(&mut v)["initial_charge_units"]=json!(177.5);v})
        .expect("177.5/250 is exactly the printed 71%+ band edge");
    run(&mut high,1800);
    close(own(&high)["charge_units"].as_f64().unwrap(),185.5); // band1 +8/IT

    let mut gap=load({let mut v=value("cn_039a",50.0,"intermediate");charging(&mut v);
        cfg(&mut v)["initial_charge_units"]=json!(176.25);v})
        .expect("176.25/250 is the printed 0-70%/71%+ continuous gap itself");
    run(&mut gap,1800);
    close(own(&gap)["charge_units"].as_f64().unwrap(),176.25);
    assert!(!own(&gap)["pending_adjudications"].as_array().unwrap().is_empty(),
        "70<%<71 has no printed rate; pending, not interpolated");
    run(&mut gap,3600);
    close(own(&gap)["charge_units"].as_f64().unwrap(),176.25);
}

// B19/B20 residual family: an AIP interval speed with no printed unit
// output (here the ordered 20 kt converges to the registered cn_039a
// 19 kt ceiling) must credit nothing, must not read as off, and must not
// silently debit the printed battery column either way: the whole
// intermediate settlement is deferred to pending. Root design note:
// "defer-all" (current) vs "debit printed column, credit unknown" is an
// open policy; this case freezes the current honest behavior.
#[test]
fn aip_interval_speed_without_a_printed_output_defers_settlement_and_stays_operational() {
    let mut v=value("cn_039a",200.0,"intermediate");
    cfg(&mut v)["initial_aip_remaining_units"]=json!(20.0);
    cfg(&mut v)["initial_aip_operating"]=json!(true);
    cfg(&mut v)["initial_power_mode"]=json!("battery");
    v["units"][0]["speed_kts"]=json!(19);
    v["units"][0]["depth_m"]=json!(100);
    let mut w=load(v).unwrap();
    let actor=id(&w,"SUB");let before=w.entity(actor).unwrap().pos;
    run(&mut w,1800);
    assert!(w.entity(actor).unwrap().pos.range_to(before)>1.0,"real moving consumer");
    close(own(&w)["charge_units"].as_f64().unwrap(),200.0);
    close(own(&w)["aip_remaining_units"].as_f64().unwrap(),20.0);
    assert_eq!(own(&w)["aip_operational"],json!(true),"unquantified output is not zero/off");
    assert!(!own(&w)["pending_adjudications"].as_array().unwrap().is_empty());
}

// B21: exhaustion follows the printed exhaustion instruction (return to
// port to replenish fuel and oxygen), sets the own-side due flag, blocks
// restart, and nothing - ticks, port-adjacent loiter, save/read - refills
// it, because no rate/provider/duration is printed.
#[test]
fn exhausted_aip_sets_replenishment_due_and_nothing_automatic_refills_it() {
    let mut v=value("cn_039a",100.0,"intermediate");
    cfg(&mut v)["initial_aip_remaining_units"]=json!(1.0);
    cfg(&mut v)["initial_aip_operating"]=json!(true);
    v["units"][0]["speed_kts"]=json!(9);
    let mut w=load(v).unwrap();
    run(&mut w,1800); // 9 kt IT: printed 8 demand = AIP 3 attempted, only 1.0 available
    close(own(&w)["charge_units"].as_f64().unwrap(),93.0);
    close(own(&w)["aip_remaining_units"].as_f64().unwrap(),0.0);
    assert_eq!(own(&w)["aip_operational"],json!(false));
    assert_eq!(own(&w)["aip_replenishment_due"],json!(true));
    let restart=Command::Navigation{unit:id(&w,"SUB"),action:n::Action::Battery{action:b::Action::SetAipOperating{enabled:true}}};
    assert!(restart.validate_source(&w,Side::Blue).is_err(),
        "zero remaining potential cannot restart; only port replenishment, which is unimplemented and unprinted");
    run(&mut w,5400); // loiter wherever it is: no port service exists in the engine
    close(own(&w)["aip_remaining_units"].as_f64().unwrap(),0.0);
    assert_eq!(own(&w)["aip_replenishment_due"],json!(true));
    let bytes=serde_json::to_vec(&w).unwrap();
    let mut restored: World=serde_json::from_slice(&bytes).unwrap();
    run(&mut w,6000);
    run(&mut restored,6000);
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap(),
        "exhausted-AIP state and due flag survive full-world save/read");
    assert_eq!(own(&w)["aip_replenishment_due"],own(&restored)["aip_replenishment_due"]);
}

// A19 binding candidate: se_ss_a19 is a real stable DB ID whose exact
// annex row (Naval SITREP #63 PDF3/printed page 2) prints Type A19 SSP,
// AIP-Electric, 150 (new), 'Stirling AIP system provides 2,000 charge
// units', corroborated by rules PDF30/3-8. DELIBERATE SOURCE RED today:
// battery2026::initialize must reject with "3.2 exact Annex row has no
// verified battery fit" until root registers annex_platforms.se_ss_a19.
// After registration this case is the printed Gotland example: at 4 kt
// the battery stays full and AIP pays 2 units/IT.
#[test]
fn se_ss_a19_gotland_row_is_the_next_exact_aip_fit_binding() {
    let mut v=value("se_ss_a19",150.0,"intermediate");
    cfg(&mut v)["initial_aip_remaining_units"]=json!(2000.0);
    cfg(&mut v)["initial_aip_operating"]=json!(true);
    let mut w=match load(v) {
        Ok(w)=>w,
        Err(message)=>panic!("RED until root registers the A19 row: {message}"),
    };
    run(&mut w,1800);
    close(own(&w)["capacity_units"].as_f64().unwrap(),150.0);
    assert_eq!(own(&w)["aip_fitted"],json!(true));
    close(own(&w)["charge_units"].as_f64().unwrap(),150.0); // <=5 kt: AIP meets all demand
    close(own(&w)["aip_remaining_units"].as_f64().unwrap(),1998.0); // 2 units/IT at 4 kt
}

// The already-registered Yuan ceiling is19kt. The exact A19 experimental
// fixture permits20kt, so the printed qualitative >=20kt branch is reachable
// without inventing a quantified AIP credit or editing live speed/charge.
#[test]
fn a19_actual_twenty_knot_interval_keeps_unknown_aip_credit_pending() {
    let mut v=value("se_ss_a19",150.,"intermediate");
    cfg(&mut v)["initial_aip_remaining_units"]=json!(20.);
    cfg(&mut v)["initial_aip_operating"]=json!(true);
    v["units"][0]["speed_kts"]=json!(20.);
    // Below the old unqualified-hull >60m speed cap, which this battery-only
    // field registration must preserve. No Annex depth qualification claimed.
    v["units"][0]["depth_m"]=json!(50.);
    let mut w=load(v).expect("exact battery-only A19 source fit must initialize");
    let actor=id(&w,"SUB");let start=w.entity(actor).unwrap().pos;
    let preflight=Command::SetDepth{unit:actor,depth_m:50.};
    let before=serde_json::to_vec(&w).unwrap();
    preflight.validate_source(&w,Side::Blue).unwrap();
    assert_eq!(before,serde_json::to_vec(&w).unwrap(),"battery-only entry cannot imply a depth field or panic in read-only admission");
    run(&mut w,1800);
    assert!(w.entity(actor).unwrap().pos.range_to(start)>1.);
    close(w.entity(actor).unwrap().speed_kts,20.);
    close(own(&w)["charge_units"].as_f64().unwrap(),150.);
    close(own(&w)["aip_remaining_units"].as_f64().unwrap(),20.);
    assert_eq!(own(&w)["aip_operational"],true);
    assert!(!own(&w)["pending_adjudications"].as_array().unwrap().is_empty(),
        "less-than10-percent is qualitative; no fabricated output or silent zero credit");
}

//! Source9.4.7 candidates using only existing public APIs before implementation.
//! Native first report and real Declare; no Contact/FCS/ext/draw injection.
use harpoon_core::rules::{aircombat2026 as air, communications2026 as comm};
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
fn scenario(enabled:bool, friend:bool)->Value {
    let fit=|peers:Vec<&str>|json!({"source":"scenario_assumption: explicit HF voice experiment; no Annex radio inference",
        "protocol":"wide-gci-voice","send":true,"receive":true,"operational":true,"peers":peers,
        "automatic_warning":false,"transport":{"type":"hf","max_range_nm":20}});
    let unit=|attempt|json!({"source":"scenario_assumption: explicit single-seat/RWR/ES and passive-gun starting plan, not a real hardware audit",
        "single_seat_fighter":true,"rwr":false,"es":false,"attack_method":"passive_gun",
        "attempt_surprise":attempt,"out_of_sun":false});
    let target=json!({"source":"scenario_assumption: controlled visual signature","size":5,"noise":null,
        "anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    let mut v=json!({"id":"wide-air-surprise","name":"scenario_assumption: real native starting conditions",
      "seed":26,"duration_s":10000,"weather":{"visibility_nm":40,"jam":[0,0]},"context":{
       "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
       "aviation_2026":{"source":"scenario_assumption: source-mechanism load/fuel experiment",
          "loads":{"cn_j15":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"},
           "us_f35c":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"}},"parking":{},
          "initial_fuel_nm":{"ATTACK":700,"DEFEND":700}},
       "aircombat_2026":{"source":"English2026 PDF103-104 §9.4.7; scenario_assumption: both sides agree optional conditions",
          "enabled":enabled,"starting_speed":true,"astern":true,"surprise":true,"units":{"ATTACK":unit(true),"DEFEND":unit(false)},
          "groups":{"A":{"source":"scenario_assumption: actual attacking formation roster","units":["ATTACK"]},
           "D":{"source":"scenario_assumption: actual defending formation roster","units":["DEFEND"]}}},
       "detection_2026":{"bindings":{"cn_j15:visual":{"type":"visual","source":"scenario_assumption: actually fitted forward observer"},
          "us_ford:visual":{"type":"visual","source":"scenario_assumption: actually fitted GCI observer"}},
        "targets":{"cn_j15":target,"us_f35c":target,"us_ford":{"source":"scenario_assumption: physical observer vessel", "size":0,
          "noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":0}},
        "environment":{"source":"scenario_assumption: clear daylight no obscuration","sea_state":0,"precipitation":0,"visibility_percent":100,
          "shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
      "units":[{"side":"Blue","class":"cn_j15","name":"ATTACK","pos":[-0.1,0],"course_deg":90,"speed_kts":400,"altitude_m":4000,"fire_on":false},
        {"side":"Red","class":"us_f35c","name":"DEFEND","pos":[0,0],"course_deg":90,"speed_kts":400,"altitude_m":4000,"fire_on":false}]});
    if friend {
        v["units"].as_array_mut().unwrap().push(json!({"side":"Red","class":"us_ford","name":"GCI","pos":[0,-0.05],"speed_kts":0,"fire_on":false}));
        v["context"]["communications_2026"]=json!({"source":"scenario_assumption: real compatible operational link",
            "units":{"GCI":{"source":"scenario_assumption: declared observer radio","fits":{"voice":fit(vec!["DEFEND"])}},
              "DEFEND":{"source":"scenario_assumption: declared receiving radio","fits":{"voice":fit(vec![])}}}});
    }
    v
}
fn load(v:Value)->World {
    let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
    for class in ["cn_j15","us_f35c","us_ford"] {let p=db.platforms.get_mut(class).unwrap();p.weapons.clear();
        p.sensors=if class=="us_f35c"{vec![]}else{vec!["visual".into()]};}
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(v).unwrap(),db).unwrap()
}
fn id(w:&World,name:&str)->EntityId {w.entities.values().find(|e|e.name==name).unwrap().id}
fn declare(w:&mut World) {
    harpoon_core::sim::tick(w,&[]);let actor=id(w,"ATTACK");let defender=id(w,"DEFEND");
    let contact=w.find_contact(Side::Blue,defender).expect("actual native observer must discover the defender");
    assert!(w.rules.aaw.as_ref().unwrap().firecontrol.tracks.values().any(|t|t.observer==actor&&t.target==defender&&t.report.is_some()));
    let c=Command::AirCombat{unit:actor,action:air::Action::Declare{contact}};
    c.validate_source(w,Side::Blue).unwrap();harpoon_core::sim::tick(w,&[(Side::Blue,c)]);
    assert!(air::in_dogfight(w,actor));
}
fn episode(w:&World)->Value {
    w.events.iter().find_map(|e|match &e.kind {EventKind::RulesEvent{rule,detail,..} if rule=="9.4.7.starting"=>
        Some(serde_json::from_str(detail).unwrap()),_=>None}).expect("actual agreed optional declaration must produce a persisted source starting episode")
}
#[test]
fn s01_actual_native_undetected_astern_entry_produces_one_source_starting_episode() {
    let mut w=load(scenario(true,false));declare(&mut w);let e=episode(&w);
    assert_eq!(e["attacking_count"],1);assert_eq!(e["defending_count"],1);
    assert_eq!(e["chance_percent"].as_f64().unwrap(), 30.0);assert!(e["d100"].is_number());
    let original=e.clone();let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..30 {harpoon_core::sim::tick(&mut w,&[]);harpoon_core::sim::tick(&mut restored,&[]);}
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&restored).unwrap());
    assert_eq!(episode(&w),original);assert_eq!(w.events.iter().filter(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="9.4.7.starting")).count(),1);
}
#[test]
fn s02_optional_off_does_not_silently_change_the_existing_base_fight() {
    let mut w=load(scenario(false,false));declare(&mut w);
    assert!(air::maneuvering(&w,id(&w,"DEFEND")));
    assert!(!w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="9.4.7.starting")));
}
#[test]
fn s03_real_friendly_observer_track_and_qualified_transport_prevent_surprise_without_a_free_defender_track() {
    let mut w=load(scenario(true,true));declare(&mut w);
    let (a,d,g)=(id(&w,"ATTACK"),id(&w,"DEFEND"),id(&w,"GCI"));
    assert!(w.rules.aaw.as_ref().unwrap().firecontrol.tracks.values().any(|t|t.observer==g&&t.target==a&&t.report.is_some()));
    assert!(!w.rules.aaw.as_ref().unwrap().firecontrol.tracks.values().any(|t|t.observer==d&&t.target==a));
    comm::validate_transport(&w,Side::Red,g,"voice",&[d]).unwrap();
    let e=episode(&w);assert_eq!(e["surprised"],false);assert_eq!(e["immunity"],"communicating_friendly_track");
    assert!(e["d100"].is_null(),"a real immunity is not a failed attack die or a free track");
}

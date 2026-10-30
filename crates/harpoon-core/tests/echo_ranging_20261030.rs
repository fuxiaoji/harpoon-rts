//! Source-only5.4.6.3 conventional passive-directional selected localization.
//! Actual finite drops, independent native bearings, ordinary RF reception.
//! No report, contact, quality, target point, mode, stock or die is inserted.
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, Vec2, World};
use serde_json::{json, Value};
const MODEL: &str = "lab_passive_directional";
const SHORT: &str = "lab_passive_directional_short";
const CLASS: &str = "lab_passive_buoy";
const SENSOR: &str = "lab_passive_buoy_sensor";
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn command(actor: EntityId, action: Value) -> Command {
    serde_json::from_value(json!({"cmd":"detection","unit":actor,"action":action}))
        .expect("normal typed source buoy command envelope")
}
fn issue(w: &mut World, c: Command) {
    c.validate_source(w, Side::Blue)
        .expect("actual original hardware/source phase admission");
    harpoon_core::sim::tick(w, &[(Side::Blue, c)]);
}
fn to(w: &mut World, t: i64) {
    while w.tick < t {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn own(w: &World, actor: EntityId) -> Value {
    let o = harpoon_core::observe::observe(w, Side::Blue, 0, 0);
    serde_json::to_value(o.units.iter().find(|u| u.id == actor).unwrap()).unwrap()["detection"]
        ["sonobuoys"]
        .clone()
}
fn scenario_seed(short_third: bool) -> u64 {
    // Six ordinary original bodies are allocated first; normal A/B/C drop
    // commands then allocate7/8/9. This only selects an ordinary scenario seed.
    // No trial is stored or changed. All fixture prerequisites still require
    // actual native reports from the real allocated bodies below.
    (20261203..20261203 + if short_third { 512 } else { 4096 })
        .find(|seed| {
            { let mut q=Rng::new(*seed).derive(Rng::hash_tag(362,4,"5.4.6.3.ieer:7:6")); if 1+(q.next_f64()*100.) as u8 > 30 {return false;} }
            [
                (1i64, vec![7u32, 8, 9]),
                (2, vec![7, 8, 9]),
                (3, vec![7, 8]),
            ]
            .into_iter()
            .all(|(epoch, ids)| {
                ids.into_iter().all(|buoy| {
                    let key = format!("5.4.6:{buoy}:{SENSOR}:6");
                    let mut rng = Rng::new(*seed).derive(Rng::hash_tag(epoch, 0, &key));
                    let maximum = if !short_third && epoch == 2 && buoy == 9 {
                        10
                    } else {
                        60
                    };
                    1 + (rng.next_f64() * 100.) as u8 <= maximum
                })
            })
        })
        .expect("bounded pure source RNG preselection; not an engine replay")
}
fn fixture(short_third: bool) -> World { fixture_with_mixed(short_third,false) }
fn fixture_with_mixed(short_third: bool,mixed: bool) -> World {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    db.platforms.get_mut("us_mh60r").unwrap().sensors.clear();
    db.platforms.get_mut("us_mh60r").unwrap().weapons.clear();
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    let mut sensor = db.sensors.values().next().unwrap().clone();
    sensor.id = SENSOR.into();
    sensor.name =
        "scenario_assumption: conventional directional passive laboratory hydrophone".into();
    sensor.kind = harpoon_core::db::SensorKind::SonarPassive;
    sensor.air_range_nm = 0.;
    sensor.surface_range_nm = 0.;
    sensor.missile_range_nm = 0.;
    sensor.sub_range_nm = 2.;
    sensor.alt_m = 0.;
    db.sensors.insert(SENSOR.into(), sensor);
    let mut float = db.platforms["us_ford"].clone();
    float.id = CLASS.into();
    float.name = "scenario_assumption: unarmed passive physical sensor float".into();
    float.hp = 1.;
    float.max_speed_kts = 0.;
    float.cruise_speed_kts = 0.;
    float.turn_rate_deg_s = 0.;
    float.noise_db = 0.;
    float.vp_value = 0.;
    float.endurance_s = 0.;
    float.max_depth_m = 0.;
    float.chaff = 0;
    float.decoys = 0;
    float.deck = None;
    float.rule_data = None;
    float.rcs = serde_json::from_value(json!("VerySmall")).unwrap();
    float.sensors = vec![SENSOR.into()];
    float.weapons.clear();
    db.platforms.insert(CLASS.into(), float);
    if mixed {
        let mut old=db.sensors[SENSOR].clone(); old.id="lab_old_sensor".into(); db.sensors.insert("lab_old_sensor".into(),old);
        let mut old=db.platforms[CLASS].clone(); old.id="lab_old_float".into(); old.sensors=vec!["lab_old_sensor".into()]; db.platforms.insert("lab_old_float".into(),old);
    }
    db.validate().unwrap();
    let model = |life| json!({"source":"scenario_assumption: one1kg conventional passive-directional float, no real K2 claim","platform_class":CLASS,"sensor":SENSOR,"active":false,"directional":true,"allowed_depths_m":[15.],"lifetime_s":life,"mass_kg":1.,"sea_state6_capable":false,"radio_antenna_height_m":0.2,"radio_channels":[1,2,3],"protocol":"lab-passive-vhf"});
    let monitor = |channels| json!({"source":"scenario_assumption: independently installed search and passive localization channels, no real K3 claim","search_channels":1,"passive_localization_channels":channels,"active_channels":0,"processing_per_tactical_turn":channels,"protocol":"lab-passive-vhf"});
    // Quiet target modifier0, stationary relative modifier0, SeaState0
    // passive environment+1 and no layer: modified50=2*1.25=2.5nm.
    // The book's0.75 spacing produces this real intended triangle. These
    // physical release positions are not a guessed uncertainty/field radius.
    let modified50 = 2. * harpoon_core::rules::detection2026::sonar_range_modifier(1).unwrap();
    let spacing = 0.75 * modified50;
    let height = spacing * (3f64).sqrt() / 2.;
    let mut scene = json!({"id":"passive-selected-localization","name":"scenario_assumption: actual field and separate receiver channel experiment","seed":scenario_seed(short_third),"duration_s":4000,
      "weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
      "aviation_2026":{"source":"scenario_assumption: original real Flight and one1kg explicit trial payload on each original aircraft","loads":{"us_mh60r":{"payload_kg":1.,"tanks":[],"source":"scenario_assumption: laboratory float or receiver board mass only"}},"parking":{},"initial_fuel_nm":{"LAYER_A":200.,"LAYER_B":200.,"LAYER_C":200.,"TWO":200.,"THREE":200.}},
      "detection_2026":{"bindings":{format!("{CLASS}:{SENSOR}"):{"type":"sonar","source":"scenario_assumption: actual conventional Gen5 LMF2nm50percent passive directional buoy","generation":6,"passive_range_nm":2.,"active_range_nm":2.,"frequency":"lmf","mount":"buoy","active_torpedo_detection":false,"high_speed_tow":false}},
       "environment":{"source":"scenario_assumption: source laboratory quiet static open water","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":200.,"layer":null,"ice":0,"cloud":null},
       "targets":{"cn_093":{"source":"scenario_assumption: controlled Quiet target signature, no anechoic/pumpjet/layer","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}},
       "sonobuoys":{"source":"scenario_assumption: actual finite directional passive experiment, conventional not EER/AQA7","models":{MODEL:model(900),SHORT:model(if short_third{537}else{900})},
        "carriers":{"LAYER_A":{"source":"scenario_assumption: one real item on original aircraft","loads":{MODEL:1}},"LAYER_B":{"source":"scenario_assumption: one real item on original aircraft","loads":{MODEL:1}},"LAYER_C":{"source":"scenario_assumption: one real item on original aircraft","loads":{if short_third{SHORT}else{MODEL}:1}}},
        "monitors":{"TWO":monitor(2),"THREE":monitor(3)},"echo_fits":{"TWO":{"source":"scenario_assumption: experimental charge and acoustic radius","charges":1,"charge_mass_kg":1.,"audible_radius_nm":10.}}}}},
      "units":[
       {"side":"Blue","class":"us_mh60r","name":"LAYER_A","pos":[0,0],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"LAYER_B","pos":[spacing,0],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"LAYER_C","pos":[spacing/2.,height],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"TWO","pos":[0.8,-1.],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"THREE","pos":[1.,-1.],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Red","class":"cn_093","name":"TARGET","pos":[spacing/2.,0.1],"speed_kts":0,"depth_m":50,"fire_on":false}]});
    if mixed {
        let config=&mut scene["context"]["detection_2026"];
        config["sonobuoys"]["models"][SHORT]["platform_class"]=json!("lab_old_float");
        config["sonobuoys"]["models"][SHORT]["sensor"]=json!("lab_old_sensor");
        config["sonobuoys"]["carriers"]["LAYER_B"]["loads"]=json!({(SHORT):1});
        config["bindings"]["lab_old_float:lab_old_sensor"]=json!({"type":"sonar","source":"scenario_assumption: Gen5 conventional no IEER active receiver","generation":5,"passive_range_nm":2.,"active_range_nm":null,"frequency":"lmf","mount":"buoy","active_torpedo_detection":false,"high_speed_tow":false});
    }
    let sc: Scenario = serde_json::from_value(scene)
        .expect("source candidate through existing scenario schema, no nonexistent Rust symbol");
    let mut w = harpoon_core::load_scenario(sc, db)
        .expect("source passive model/receiver configuration must actually initialize");
    let h:Vec<_>=["LAYER_A","LAYER_B","LAYER_C","TWO","THREE"].into_iter().map(|name| {
        let c:Command=serde_json::from_value(json!({"cmd":"aviation","unit":id(&w,name),"action":{"type":"throttle","setting":"Hover"}})).unwrap();
        c.validate_source(&w,Side::Blue).unwrap();(Side::Blue,c)
    }).collect();
    harpoon_core::sim::tick(&mut w, &h);
    let drops: Vec<_> = [
        ("LAYER_A", MODEL, 1),
        ("LAYER_B", if mixed {SHORT} else {MODEL}, 2),
        ("LAYER_C", if short_third { SHORT } else { MODEL }, 3),
    ]
    .into_iter()
    .map(|(name, model, channel)| {
        let c = command(
            id(&w, name),
            json!({"type":"drop_sonobuoy","model":model,"depth_m":15.,"radio_channel":channel}),
        );
        c.validate_source(&w, Side::Blue).unwrap();
        (Side::Blue, c)
    })
    .collect();
    harpoon_core::sim::tick(&mut w, &drops);
    // Plot the first search field before TacticalTurn1 opens. Nothing can
    // listen before180 or change its actual field during that Detection turn.
    let ids = buoys(&w);
    let search: Vec<_> = ["TWO", "THREE"]
        .into_iter()
        .map(|name| {
            let c = select(&w, id(&w, name), &[ids[0]], "search");
            c.validate_source(&w, Side::Blue).unwrap();
            (Side::Blue, c)
        })
        .collect();
    harpoon_core::sim::tick(&mut w, &search);
    w
}
fn buoys(w: &World) -> Vec<EntityId> {
    ["LAYER_A", "LAYER_B", "LAYER_C"]
        .into_iter()
        .map(|name| {
            let v = own(w, id(w, name));
            let xs = v["deployed"].as_array().unwrap();
            assert_eq!(xs.len(), 1);
            serde_json::from_value(xs[0]["id"].clone()).unwrap()
        })
        .collect()
}
fn select(w: &World, actor: EntityId, buoys: &[EntityId], purpose: &str) -> Command {
    command(
        actor,
        json!({"type":"listen_sonobuoys","buoys":buoys,"purpose":purpose}),
    )
}

#[test]
fn ieer_actual_buoy_receipt_charge_and_good_fix() {
    let mut w=fixture(false); let ids=buoys(&w); let actor=id(&w,"TWO");
    to(&mut w,181);
    assert!(!w.rules.detection.as_ref().unwrap().sonobuoys.alerts.is_empty(),"real selected buoy search must detect first");
    let selected=select(&w,actor,&ids[..2],"localization"); issue(&mut w,selected);
    to(&mut w,360);
    let cruise:Command=serde_json::from_value(json!({"cmd":"aviation","unit":actor,"action":{"type":"throttle","setting":"Cruise"}})).unwrap();
    issue(&mut w,cruise);
    let echo=command(actor,json!({"type":"echo_range","initial_buoy":ids[0]}));
    echo.validate_source(&w,Side::Blue).unwrap();
    issue(&mut w,echo);
    let state=&w.rules.detection.as_ref().unwrap().sonobuoys;
    assert_eq!(state.echo.as_ref().unwrap().charges[&actor],0);
    let e=state.echo.as_ref().unwrap().fixes.values().next().expect("actual qualified receiver must receive IEER result");
    assert!(e.good); assert!((e.d100 as f64)<e.probability/2.);
    let sol=harpoon_core::rules::firecontrol2026::attack_solution(&w,actor,id(&w,"TARGET"),0,0.).unwrap();
    assert_eq!(sol.quality,harpoon_core::rules::firecontrol2026::Quality::Good);
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="5.4.6.3.echo_warning")));
    let restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap(); assert_eq!(serde_json::to_value(w).unwrap(),serde_json::to_value(restored).unwrap());
}

#[test]
fn echo_admission_ignores_hidden_target_life_and_commits_charge() {
    let mut a=fixture(false); let ids=buoys(&a); let actor=id(&a,"TWO");
    to(&mut a,181); let selected=select(&a,actor,&ids[..2],"localization"); issue(&mut a,selected); to(&mut a,360);
    let cruise:Command=serde_json::from_value(json!({"cmd":"aviation","unit":actor,"action":{"type":"throttle","setting":"Cruise"}})).unwrap();
    issue(&mut a,cruise);
    let mut b=a.clone(); let target=id(&b,"TARGET"); {let e=b.entity_mut(target).unwrap();e.alive=false;e.hp=0.;}
    let echo=command(actor,json!({"type":"echo_range","initial_buoy":ids[0]}));
    assert!(echo.validate_source(&a,Side::Blue).is_ok());
    assert_eq!(echo.validate_source(&a,Side::Blue).is_ok(),echo.validate_source(&b,Side::Blue).is_ok());
    harpoon_core::sim::tick(&mut a,&[(Side::Blue,echo.clone())]); harpoon_core::sim::tick(&mut b,&[(Side::Blue,echo.clone())]);
    assert_eq!(a.rules.detection.as_ref().unwrap().sonobuoys.echo.as_ref().unwrap().charges[&actor],0);
    assert_eq!(b.rules.detection.as_ref().unwrap().sonobuoys.echo.as_ref().unwrap().charges[&actor],0);
    assert!(echo.validate_source(&a,Side::Blue).is_err());
}

#[test]
fn ieer_mixed_incompatible_selected_receivers_rejects_without_debit() {
    let mut w=fixture_with_mixed(false,true); let ids=buoys(&w); let actor=id(&w,"TWO");
    to(&mut w,181); let selected=select(&w,actor,&ids[..2],"localization"); issue(&mut w,selected); to(&mut w,360);
    let cruise:Command=serde_json::from_value(json!({"cmd":"aviation","unit":actor,"action":{"type":"throttle","setting":"Cruise"}})).unwrap(); issue(&mut w,cruise);
    let cmd=command(actor,json!({"type":"echo_range","initial_buoy":ids[0]}));
    let before=serde_json::to_vec(&w).unwrap(); let err=cmd.validate_source(&w,Side::Blue).unwrap_err();
    assert!(err.contains("compatible receivers")); assert_eq!(before,serde_json::to_vec(&w).unwrap());
    assert_eq!(w.rules.detection.as_ref().unwrap().sonobuoys.echo.as_ref().unwrap().charges[&actor],1);
}

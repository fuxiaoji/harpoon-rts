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
fn fixture(short_third: bool) -> World {
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
    let scene = json!({"id":"passive-selected-localization","name":"scenario_assumption: actual field and separate receiver channel experiment","seed":scenario_seed(short_third),"duration_s":4000,
      "weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
      "aviation_2026":{"source":"scenario_assumption: original real Flight and one1kg explicit trial payload on each original aircraft","loads":{"us_mh60r":{"payload_kg":1.,"tanks":[],"source":"scenario_assumption: laboratory float or receiver board mass only"}},"parking":{},"initial_fuel_nm":{"LAYER_A":200.,"LAYER_B":200.,"LAYER_C":200.,"TWO":200.,"THREE":200.}},
      "detection_2026":{"bindings":{format!("{CLASS}:{SENSOR}"):{"type":"sonar","source":"scenario_assumption: actual conventional Gen5 LMF2nm50percent passive directional buoy","generation":5,"passive_range_nm":2.,"active_range_nm":null,"frequency":"lmf","mount":"buoy","active_torpedo_detection":false,"high_speed_tow":false}},
       "environment":{"source":"scenario_assumption: source laboratory quiet static open water","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":200.,"layer":null,"ice":0,"cloud":null},
       "targets":{"cn_093":{"source":"scenario_assumption: controlled Quiet target signature, no anechoic/pumpjet/layer","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}},
       "sonobuoys":{"source":"scenario_assumption: actual finite directional passive experiment, conventional not EER/AQA7","models":{MODEL:model(900),SHORT:model(if short_third{537}else{900})},
        "carriers":{"LAYER_A":{"source":"scenario_assumption: one real item on original aircraft","loads":{MODEL:1}},"LAYER_B":{"source":"scenario_assumption: one real item on original aircraft","loads":{MODEL:1}},"LAYER_C":{"source":"scenario_assumption: one real item on original aircraft","loads":{if short_third{SHORT}else{MODEL}:1}}},
        "monitors":{"TWO":monitor(2),"THREE":monitor(3)}}}},
      "units":[
       {"side":"Blue","class":"us_mh60r","name":"LAYER_A","pos":[0,0],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"LAYER_B","pos":[spacing,0],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"LAYER_C","pos":[spacing/2.,height],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"TWO","pos":[0.8,-1.],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"THREE","pos":[1.,-1.],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Red","class":"cn_093","name":"TARGET","pos":[spacing/2.,0.1],"speed_kts":0,"depth_m":50,"fire_on":false}]});
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
        ("LAYER_B", MODEL, 2),
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
fn observations(w: &World, actor: EntityId) -> Vec<Value> {
    own(w, actor)["observations"]
        .as_array()
        .expect("actual selected received native buoy observations")
        .clone()
}
fn actual_same_turn(w: &World, ids: &[EntityId]) -> bool {
    let target = id(w, "TARGET");
    let turn = w.tick.div_euclid(180);
    let reports = &w.rules.detection.as_ref().unwrap().sonobuoys.native;
    ids.iter().all(|id| {
        reports.values().any(|r| {
            r.observer == *id
                && r.target == target
                && r.at.div_euclid(180) == turn
                && r.sensor_kind == harpoon_core::db::SensorKind::SonarPassive
                && r.measured_position.is_none()
        })
    })
}
fn acquired_search(w: &mut World, actor: EntityId, first: EntityId) {
    let c = select(w, actor, &[first], "search");
    issue(w, c);
    let until = 350;
    while w.tick < until && observations(w, actor).is_empty() {
        harpoon_core::sim::tick(w, &[]);
    }
    let rs = observations(w, actor);
    assert_eq!(
        rs.len(),
        1,
        "one installed search channel receives only one actual selected float"
    );
    assert_eq!(
        serde_json::from_value::<EntityId>(rs[0]["observer"].clone()).unwrap(),
        first
    );
    assert!(
        rs[0]["measured_position"].is_null(),
        "one passive bearing never supplies a target range"
    );
    assert!(rs[0]["bearing_deg"].as_f64().is_some_and(|b| b.is_finite()));
    assert!(
        rs[0]["target"].is_null(),
        "own raw report must not expose enemy EntityId"
    );
    assert!(
        harpoon_core::rules::firecontrol2026::attack_solution(w, actor, id(w, "TARGET"), 0, 0.)
            .is_err(),
        "one passive search detection is No Attack, not a free ranged FCS"
    );
}
fn fix(w: &World, actor: EntityId, quality: &str) -> Option<Value> {
    own(w, actor)["fixes"]
        .as_array()?
        .iter()
        .find(|f| f["quality"] == quality)
        .cloned()
}
fn intersection(received: &[Value], a: EntityId, b: EntityId) -> Vec2 {
    let ray = |id| {
        let r = received
            .iter()
            .find(|r| serde_json::from_value::<EntityId>(r["observer"].clone()).unwrap() == id)
            .unwrap();
        assert!(r["measured_position"].is_null());
        let p: Vec2 = serde_json::from_value(r["observer_position"].clone()).unwrap();
        let angle = r["bearing_deg"].as_f64().unwrap().to_radians();
        (p, Vec2::new(angle.sin(), angle.cos()))
    };
    let (pa, da) = ray(a);
    let (pb, db) = ray(b);
    let cross = |x: Vec2, y: Vec2| x.x * y.y - x.y * y.x;
    let denominator = cross(da, db);
    assert!(
        denominator.abs() > 1e-6,
        "actual received bearings must give an independent geometric crossing"
    );
    let delta = Vec2::new(pb.x - pa.x, pb.y - pa.y);
    let t = cross(delta, db) / denominator;
    let u = cross(delta, da) / denominator;
    assert!(
        t > 0. && u > 0.,
        "both original measured forward rays must reach the intersection"
    );
    Vec2::new(pa.x + t * da.x, pa.y + t * da.y)
}
fn original_trial(w: &World, buoy: EntityId) -> Value {
    let target = id(w, "TARGET");
    w.events
        .iter()
        .rev()
        .find_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule, unit, detail, ..
            } if rule == "5.scan"
                && *unit == Some(buoy)
                && e.tick.div_euclid(180) == w.tick.div_euclid(180) =>
            {
                let v: Value = serde_json::from_str(detail).unwrap();
                (v["truth_target"] == json!(target)
                    && v["sensor"] == SENSOR
                    && v["result"]["rule"] == "5.4.6")
                    .then(|| v["result"].clone())
            }
            _ => None,
        })
        .expect("the actual source sonar trial, not a fabricated probability, is the baseline")
}
fn localization_trials_use_actual_monitor_alert(
    w: &World,
    actor: EntityId,
    selected: &[EntityId],
    baseline: &[Value],
    search_at: i64,
    search_buoy: EntityId,
) {
    for (buoy, original) in selected.iter().zip(baseline) {
        let t=w.events.iter().rev().find_map(|e|match &e.kind {
            EventKind::RulesEvent{rule,unit,detail,..} if rule=="5.4.6.3.localization_trial"&&*unit==Some(actor)&&e.tick.div_euclid(180)==w.tick.div_euclid(180)=>{
                let v:Value=serde_json::from_str(detail).unwrap();
                (v["buoy"]==json!(buoy)&&v["receiver"]==json!(actor)).then_some(v)
            },_=>None
        }).expect("each selected float needs an actual receiver-qualified localization trial; raw native count alone is insufficient");
        let base = original["probability"].as_f64().unwrap();
        assert_eq!(t["base_probability"].as_f64(), Some(base));
        assert_eq!(t["alert_modifier"].as_f64(), Some(15.0));
        assert_eq!(t["probability"].as_f64(), Some((base + 15.).min(90.)));
        assert_eq!(t["prior_detection_at"].as_i64(), Some(search_at));
        assert_eq!(t["alert_source_buoy"], json!(search_buoy));
        assert_eq!(
            t["d100"],
            original_trial(w, *buoy)["d100"],
            "search/selection polling cannot reroll the original float's current-Tac die"
        );
        assert_eq!(
            t["detected"].as_bool(),
            Some(t["d100"].as_f64().unwrap() <= t["probability"].as_f64().unwrap())
        );
    }
}
#[test]
fn ps01_one_real_search_channel_then_two_selected_same_turn_passive_bearings_yield_poor_not_fair_or_good(
) {
    let mut w = fixture(false);
    let actor = id(&w, "TWO");
    let target = id(&w, "TARGET");
    let ids = buoys(&w);
    to(&mut w, 185);
    let search_cid = w
        .find_contact(Side::Blue, target)
        .expect("Search plotted at3 genuinely receives a target bearing after TacticalTurn1 opens");
    assert!(w.contact(Side::Blue,search_cid).unwrap().bearing_only,
        "one actual monitored passive Search report is a bearing contact, not a measured target range");
    let searched = observations(&w, actor);
    assert_eq!(searched.len(), 1);
    assert_eq!(searched[0]["observer"], json!(ids[0]));
    assert!(searched[0]["measured_position"].is_null());
    assert!(
        harpoon_core::rules::firecontrol2026::attack_solution(&w, actor, target, 0, 0.).is_err(),
        "the genuinely monitored single Search bearing still gives No Attack"
    );
    let two_search = select(&w, actor, &ids[..2], "search");
    assert!(
        two_search.validate_source(&w, Side::Blue).is_err(),
        "two localization channels do not turn into two search channels"
    );
    acquired_search(&mut w, actor, ids[0]);
    let baselines: Vec<_> = ids[..2]
        .iter()
        .map(|bid| original_trial(&w, *bid))
        .collect();
    let active = select(&w, actor, &ids[..2], "active");
    assert!(
        active.validate_source(&w, Side::Blue).is_err(),
        "passive localization hardware does not supply an active channel"
    );
    let over = select(&w, actor, &ids, "localization");
    assert!(over.validate_source(&w, Side::Blue).is_err());
    let c = select(&w, actor, &ids[..2], "localization");
    issue(&mut w, c);
    to(&mut w, 359);
    assert!(fix(&w,actor,"Poor").is_none()&&fix(&w,actor,"Good").is_none(),"new localization subset/purpose is only a next-turn intent, never same-Tac Detection modification");
    let search_at = observations(&w, actor)[0]["at"].as_i64().unwrap();
    assert_eq!(search_at.div_euclid(180), 1);
    let until = 530;
    while w.tick < until && fix(&w, actor, "Poor").is_none() {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let f = fix(&w, actor, "Poor")
        .expect("ordinary passive-directional two actual same-Tac selected detections are Poor");
    assert_eq!(w.tick.div_euclid(180), 2);
    assert!(actual_same_turn(&w,&ids),
        "in this actual localization Tac the unselected third must really detect too, so free counting it would be observable");
    localization_trials_use_actual_monitor_alert(
        &w,
        actor,
        &ids[..2],
        &baselines,
        search_at,
        ids[0],
    );
    assert_eq!(f["buoys"], json!(&ids[..2]));
    assert!(
        fix(&w, actor, "Fair").is_none(),
        "two Fair requires the special EER footnote, not ordinary passive hardware"
    );
    assert!(
        fix(&w, actor, "Good").is_none(),
        "unselected third native detection cannot silently complete Good"
    );
    let received = observations(&w, actor);
    assert_eq!(received.len(), 2);
    let center = intersection(&received, ids[0], ids[1]);
    let fcs =
        harpoon_core::rules::firecontrol2026::attack_solution(&w, actor, target, 0, 0.).unwrap();
    assert_eq!(
        fcs.quality,
        harpoon_core::rules::firecontrol2026::Quality::Poor
    );
    assert!(
        fcs.reported_aim
            .expect("independent actual bearing crossing supplies the aim")
            .range_to(center)
            < 1e-6,
        "derived aim must use received original bearing geometry, never hidden target position"
    );
    assert!(received
        .iter()
        .all(|r| r["at"].as_i64().unwrap().div_euclid(180) == w.tick.div_euclid(180)));
}
#[test]
fn ps02_three_real_same_turn_selected_passive_bearings_yield_good_then_expired_old_turn_third_cannot_be_banked(
) {
    let mut w = fixture(true);
    let actor = id(&w, "THREE");
    let target = id(&w, "TARGET");
    let ids = buoys(&w);
    to(&mut w, 185);
    acquired_search(&mut w, actor, ids[0]);
    let baselines: Vec<_> = ids.iter().map(|bid| original_trial(&w, *bid)).collect();
    assert!(baselines[2]["probability"].as_f64().unwrap()<90.,"the never-searched third float must have a real non-capped baseline so the operator+15 changes the actual probability");
    let c = select(&w, actor, &ids, "localization");
    issue(&mut w, c);
    to(&mut w, 359);
    assert!(
        fix(&w, actor, "Good").is_none(),
        "new localization field cannot affect the actual Search turn"
    );
    let search_at = observations(&w, actor)[0]["at"].as_i64().unwrap();
    assert_eq!(search_at.div_euclid(180), 1);
    let until = 530;
    while w.tick < until && fix(&w, actor, "Good").is_none() {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let f = fix(&w, actor, "Good").expect(
        "three independently detecting selected passive directional floats in one Tac yield Good",
    );
    localization_trials_use_actual_monitor_alert(&w, actor, &ids, &baselines, search_at, ids[0]);
    assert_eq!(f["buoys"], json!(ids));
    assert!(actual_same_turn(&w, &ids));
    let received = observations(&w, actor);
    assert_eq!(received.len(), 3);
    let center = intersection(&received, ids[0], ids[1]);
    let fcs =
        harpoon_core::rules::firecontrol2026::attack_solution(&w, actor, target, 0, 0.).unwrap();
    assert_eq!(
        fcs.quality,
        harpoon_core::rules::firecontrol2026::Quality::Good
    );
    assert!(fcs.reported_aim.unwrap().range_to(center) < 1e-6);
    let third = received
        .iter()
        .find(|r| serde_json::from_value::<EntityId>(r["observer"].clone()).unwrap() == ids[2])
        .unwrap();
    let origin: Vec2 = serde_json::from_value(third["observer_position"].clone()).unwrap();
    assert!(harpoon_core::geom::angle_diff(origin.bearing_to(center),third["bearing_deg"].as_f64().unwrap()).abs()<1e-6,
        "the third actual independent bearing must agree; no truth-point replacement or arbitrary averaging");
    // The short author model expires at539: the old third's real epoch2
    // report must not be banked alongside the first two's real epoch3 reports.
    to(&mut w, 541);
    assert!(w.entity(ids[2]).is_none_or(|e| !e.alive));
    assert!(
        actual_same_turn(&w, &ids[..2]),
        "the surviving two must genuinely detect in this new Tac"
    );
    assert!(!actual_same_turn(&w, &ids));
    assert!(fix(&w, actor, "Good").is_none());
    let f = fix(&w, actor, "Poor")
        .expect("two live actual current-Tac detections remain Poor after the old third expires");
    assert_eq!(f["buoys"], json!(&ids[..2]));
    let fcs =
        harpoon_core::rules::firecontrol2026::attack_solution(&w, actor, target, 0, 0.).unwrap();
    assert_eq!(
        fcs.quality,
        harpoon_core::rules::firecontrol2026::Quality::Poor
    );
    for name in ["LAYER_A", "LAYER_B", "LAYER_C"] {
        let view = own(&w, id(&w, name));
        assert!(view["carried"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v == 0));
        assert!(
            w.rules.aviation.as_ref().unwrap().flights[&id(&w, name)]
                .payload_fraction
                .abs()
                < 1e-12
        );
    }
}

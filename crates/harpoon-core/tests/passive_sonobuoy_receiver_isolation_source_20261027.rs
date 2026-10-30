//! Source-only5.4.6.3 one original die, receiver-specific alert/reception/FCS.
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
fn fixture_baselines() -> [f64; 3] {
    use harpoon_core::rules::detection2026 as d;
    let env: d::Environment = serde_json::from_value(json!({
        "source":"scenario_assumption: same declared static open-water experiment",
        "sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,
        "biologics":0,"water_depth_m":200.,"layer":null,"ice":0,"cloud":null
    }))
    .unwrap();
    // Actual200m shallow-water LMF modifier -3 combines with Sea0 +1.
    // Existing author positions stay unchanged; they are not silently moved
    // to improve this release fixture's sonar probability.
    let environment = d::sonar_environment(false, d::Frequency::Lmf, &env).unwrap();
    assert_eq!(environment, -2);
    let modified = 2. * d::sonar_range_modifier(environment).unwrap();
    let spacing = 0.75 * 2. * d::sonar_range_modifier(1).unwrap();
    let target = Vec2::new(spacing / 2., 0.1);
    let positions = [
        Vec2::zero(),
        Vec2::new(spacing, 0.),
        Vec2::new(spacing / 2., spacing * (3f64).sqrt() / 2.),
    ];
    let result =
        positions.map(|p| d::sonar_probability(modified, p.range_to(target), false).unwrap());
    assert_eq!(result, [50., 50., 20.]);
    result
}
fn scenario_seed(_short_third: bool) -> u64 {
    let baseline = fixture_baselines();
    let bonus = harpoon_core::rules::detection2026::sonobuoys::tables()["passive_localization"]
        ["alerted_operator_modifier_percent"]
        .as_f64()
        .unwrap();
    (20261203..20261203 + 65536)
        .find(|seed| {
            [1i64, 2].into_iter().all(|epoch| {
                [7u32, 8, 9].into_iter().all(|buoy| {
                    let key = format!("5.4.6:{buoy}:{SENSOR}:6");
                    let mut rng = Rng::new(*seed).derive(Rng::hash_tag(epoch, 0, &key));
                    let roll = f64::from(1 + (rng.next_f64() * 100.) as u8);
                    let base = baseline[(buoy - 7) as usize];
                    if epoch == 1 {
                        roll <= base
                    } else if buoy == 7 {
                        base < roll && roll <= (base + bonus).min(90.)
                    } else {
                        roll <= (base + bonus).min(90.)
                    }
                })
            })
        })
        .expect("bounded pure scenario seed; actual receiver trial assertions remain mandatory")
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
    // Preserve the original author triangle positions. SeaState0 alone is+1,
    // but actual200m water also appliesLMF-3: full source modified50=1.2nm.
    // These physical author stations are not an uncertainty/field radius or
    // a claim that their spacing is0.75 of the full modified sonar range.
    let modified50 = 2. * harpoon_core::rules::detection2026::sonar_range_modifier(1).unwrap();
    let spacing = 0.75 * modified50;
    let height = spacing * (3f64).sqrt() / 2.;
    let mut scene = json!({"id":"passive-selected-localization","name":"scenario_assumption: actual field and separate receiver channel experiment","seed":scenario_seed(short_third),"duration_s":4000,
      "weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
      "aviation_2026":{"source":"scenario_assumption: original real Flight and one1kg explicit trial payload on each original aircraft","loads":{"us_mh60r":{"payload_kg":1.,"tanks":[],"source":"scenario_assumption: laboratory float or receiver board mass only"}},"parking":{},"initial_fuel_nm":{"LAYER_A":200.,"LAYER_B":200.,"LAYER_C":200.,"TWO":200.,"THREE":200.}},
      "detection_2026":{"bindings":{format!("{CLASS}:{SENSOR}"):{"type":"sonar","source":"scenario_assumption: actual conventional Gen5 LMF2nm50percent passive directional buoy","generation":5,"passive_range_nm":2.,"active_range_nm":null,"frequency":"lmf","mount":"buoy","active_torpedo_detection":false,"high_speed_tow":false}},
       "environment":{"source":"scenario_assumption: source laboratory quiet static open water","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":200.,"layer":null,"ice":0,"cloud":null},
       "targets":{"cn_093":{"source":"scenario_assumption: controlled Quiet target signature, no anechoic/pumpjet/layer","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}},
       "sonobuoys":{"source":"scenario_assumption: actual finite directional passive experiment, conventional not EER/AQA7","models":{MODEL:model(1800),SHORT:model(1800)},
        "carriers":{"LAYER_A":{"source":"scenario_assumption: one real item on original aircraft","loads":{MODEL:1}},"LAYER_B":{"source":"scenario_assumption: one real item on original aircraft","loads":{MODEL:1}},"LAYER_C":{"source":"scenario_assumption: one real item on original aircraft","loads":{if short_third{SHORT}else{MODEL}:1}}},
        "monitors":{"TWO":monitor(2),"THREE":monitor(3)}}}},
      "units":[
       {"side":"Blue","class":"us_mh60r","name":"LAYER_A","pos":[0,0],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"LAYER_B","pos":[spacing,0],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"LAYER_C","pos":[spacing/2.,height],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"TWO","pos":[0.8,-1.],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Blue","class":"us_mh60r","name":"THREE","pos":[1.,-1.],"speed_kts":0,"altitude_m":3000,"fire_on":false},
       {"side":"Red","class":"cn_093","name":"TARGET","pos":[spacing/2.,0.1],"speed_kts":0,"depth_m":50,"fire_on":false}]});
    // This is the existing exact source four-Mk54 carriage. The separately
    // declared1kg laboratory receiver board is an author retrofit; no real
    // K3 hardware/channel applicability is inferred for this airframe.
    scene["units"][4]["class"] = json!("us_mh60r_mk54x4");
    // B owns the same complete real weapon fit; no missing carriage or
    // illegal altitude may explain the later source-FCS refusal.
    scene["units"][3]["class"] = json!("us_mh60r_mk54x4");
    scene["units"][3]["altitude_m"] = json!(1950.);
    // Medium initial altitude remains above the Low release envelope. A
    // normal later altitude command must complete the150m descent.
    scene["units"][4]["altitude_m"] = json!(2100.);
    scene["context"]["aviation_2026"]["loads"]["us_mh60r_mk54x4"] = json!({
        "source":"US Aircraft2025 PDF69/B64 four Mk54; PDF93/E1 noteC285kg helicopter version; plus explicit scenario_assumption1kg laboratory monitor board",
        "payload_kg":1141.,"tanks":[],"external_weapon_rounds":{"us_mk54":4}
    });
    scene["context"]["detection_2026"]["sonobuoys"]["monitors"]["THREE"]["source"] = json!(
        "scenario_assumption: independently installed1kg three-channel directional passive monitor on source-exact four-Mk54 carrier; not a verified real K3 fit");
    scene["context"]["torpedo_2026"] = json!({
        "source":"scenario_assumption: isolated actual airborne-Mk54 consumer, no authored solution or initial order",
        "environment":{"source":"scenario_assumption: same clear laboratory water, no layer or reverberation","layer_m":null,"reverberant":false},
        "targets":{"cn_093":{"source":"scenario_assumption: same static Quiet nonanechoic trial body","size":2,"noise":2,"anechoic":false,"foil_borne":false}}
    });
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

fn rounds(w: &World, actor: EntityId) -> u16 {
    w.entity(actor)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_mk54")
        .expect("the original source four-Mk54 physical mount")
        .ammo
}
fn payload_kg(w: &World, actor: EntityId) -> f64 {
    let flight = &w.rules.aviation.as_ref().unwrap().flights[&actor];
    flight.payload_fraction * flight.spec.max_payload_kg
}
fn throttle(actor: EntityId, setting: &str) -> Command {
    serde_json::from_value(json!({"cmd":"aviation","unit":actor,
        "action":{"type":"throttle","setting":setting}}))
    .unwrap()
}
fn plot(actor: EntityId, contact: harpoon_core::ContactId) -> Command {
    serde_json::from_value(json!({"cmd":"torpedo","unit":actor,
        "action":{"type":"plot","contact":contact,"weapon":"us_mk54",
        "speed_index":0,"depth_m":50.,"enable_run_nm":0.,"bearing_deg":null,
        "spread_deg":[],"cut_wire":false}}))
    .expect("normal Mk54 acoustic nonwire Plot, with no author solution")
}

#[test]
fn ri01_one_original_passive_die_earns_alerted_a_reception_but_failed_b_gets_no_receipt_fix_fcs_or_mk54_release(
) {
    let mut w = fixture(true);
    let a = id(&w, "THREE");
    let b = id(&w, "TWO");
    let target = id(&w, "TARGET");
    let ids = buoys(&w);
    assert_eq!(rounds(&w, a), 4);
    assert_eq!(rounds(&w, b), 4);
    assert!(w.entity(a).unwrap().sensors.is_empty() && w.entity(b).unwrap().sensors.is_empty());
    assert!((payload_kg(&w, b) - 1141.).abs() < 1e-9);
    to(&mut w, 185);
    acquired_search(&mut w, a, ids[0]);
    acquired_search(&mut w, b, ids[0]);
    let c = select(&w, a, &ids, "localization");
    issue(&mut w, c);
    to(&mut w, 359);
    assert_eq!(own(&w, a)["purpose"], json!("search"));
    assert_eq!(own(&w, b)["purpose"], json!("search"));
    assert!(fix(&w, a, "Good").is_none() && fix(&w, b, "Good").is_none());
    let search_key = format!("{}:{SENSOR}:{}", ids[0].0, target.0);
    let held = &w.rules.detection.as_ref().unwrap().tracks[&search_key];
    assert!(
        held.held && held.first_success.is_some_and(|t| t.div_euclid(180) == 1),
        "normal Search contact must really survive the preceding turn before classification"
    );
    while w.tick < 530 && fix(&w, a, "Good").is_none() {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let a_fix =
        fix(&w, a, "Good").expect("A actually receives all three selected localization bearings");
    assert_eq!(w.tick.div_euclid(180), 2);
    assert_eq!(own(&w, a)["purpose"], json!("localization"));
    assert_eq!(
        own(&w, b)["purpose"],
        json!("search"),
        "B never changes its frozen field or purpose during Detection"
    );
    let state = &w.rules.detection.as_ref().unwrap().sonobuoys;
    let trial = |receiver| {
        state
            .receiver_trials
            .values()
            .find(|t| {
                t.receiver == receiver && t.buoy == ids[0] && t.target == target && t.at == w.tick
            })
            .expect(
                "both real selected/radio-qualified receivers need their actual original-die trial",
            )
            .clone()
    };
    let ta = trial(a);
    let tb = trial(b);
    assert_eq!(ta.d100, tb.d100);
    let native = original_trial(&w, ids[0]);
    assert_eq!(
        json!(ta.d100),
        native["d100"],
        "receiver polling cannot reroll the original float/Tac die"
    );
    assert_eq!(ta.base_probability, 50.);
    assert_eq!(tb.base_probability, ta.base_probability);
    assert_eq!(ta.probability, 65.);
    assert_eq!(tb.probability, 50.);
    assert!(
        tb.probability < f64::from(tb.d100) && f64::from(ta.d100) <= ta.probability,
        "actual Bbase < same original D100 <= A+15, not a seeded private success flag"
    );
    assert!(ta.detected && !tb.detected);
    assert!(ta.alert.is_some() && tb.alert.is_none());
    assert!(native["classification"]["level"].as_u64().is_some_and(|level|level>=1),
        "A's actual qualified native LMF classification must be earned before testing receiver isolation");
    assert!(state.received.values().any(|r| r.receiver == a
        && r.report.observer == ids[0]
        && r.report.target == target
        && r.report.at.div_euclid(180) == 2));
    assert!(
        !state.received.values().any(|r| r.receiver == b
            && r.report.target == target
            && r.report.at.div_euclid(180) == 2),
        "A's successful native report cannot become B's failed receipt"
    );
    assert!(!state
        .fixes
        .values()
        .any(|f| f.monitor == b && f.target == target));
    assert!(observations(&w, b).is_empty());
    assert!(own(&w, b)["fixes"].as_array().unwrap().is_empty());
    let a_reports = observations(&w, a);
    assert_eq!(a_reports.len(), 3);
    let own_aim: Vec2 = serde_json::from_value(a_fix["measured_position"].clone()).unwrap();
    assert!(own_aim.range_to(intersection(&a_reports, ids[0], ids[1])) < 1e-6);
    // Existing side-level sharing may know Sub from A. This test deliberately
    // permits that knowledge; it does not invent per-aircraft invisibility.
    assert_eq!(harpoon_core::rules::detection2026::known_category(&w,Side::Blue,target),
        Some(harpoon_core::types::PlatformKind::Sub),
        "the real qualified A classification must reach existing side knowledge before B's final weapon gate is tested");
    let profile = &w.rules.torpedoes.as_ref().unwrap().config.weapons["us_mk54"];
    let seeker = profile.generation.max(1);
    let speed = profile.settings[0].speed_kts;
    let allowed =
        harpoon_core::rules::firecontrol2026::attack_solution(&w, a, target, seeker, speed)
            .unwrap();
    assert_eq!(
        allowed.quality,
        harpoon_core::rules::firecontrol2026::Quality::Good
    );
    assert!(
        harpoon_core::rules::detection2026::sonobuoys::attack_solution(&w, b, target).is_none()
    );
    let denied =
        harpoon_core::rules::firecontrol2026::attack_solution(&w, b, target, seeker, speed)
            .expect_err("B's failed reception cannot borrow A's source FCS");
    let contact: harpoon_core::ContactId =
        serde_json::from_value(a_fix["contact"].clone()).unwrap();
    harpoon_core::rules::aircombat2026::air_torpedo_gate(&w,b,contact)
        .expect("B has real source Flight, four weapons, legal1950m hover and legitimate shared Sub contact; its denial must be missing actor FCS");
    let plan = plot(b, contact);
    issue(&mut w, plan);
    assert!(
        harpoon_core::rules::firecontrol2026::attack_solution(&w, b, target, seeker, speed)
            .is_err()
    );
    let payload_before = payload_kg(&w, b);
    let launches_before = w.stats.projectiles_launched;
    let fire = Command::Fire {
        unit: b,
        contact,
        weapon: "us_mk54".into(),
        rounds: 1,
    };
    // Generic Fire public validation currently checks hardware/source policy;
    // no new requirement for a preliminary Plot or preview is asserted here.
    // Final real source_torpedo_release must independently consume B's FCS.
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for world in [&mut w, &mut restored] {
        harpoon_core::sim::tick(world, &[(Side::Blue, fire.clone())]);
    }
    assert_eq!(
        rounds(&w, b),
        4,
        "failed actor reception cannot spend B's physical Mk54"
    );
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&b].external_weapon_rounds["us_mk54"],
        4
    );
    assert!((payload_kg(&w, b) - payload_before).abs() < 1e-9);
    assert_eq!(w.stats.projectiles_launched, launches_before);
    assert!(!w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .launches
        .values()
        .any(|r| r.shooter == b));
    assert!(w.events.iter().any(|event|matches!(&event.kind,EventKind::RulesEvent{rule,unit,detail,..}
        if rule=="10.source_fire_blocked"&&*unit==Some(b)
            &&serde_json::from_str::<Value>(detail).is_ok_and(|value|value["reason"]==denied))),
        "the actual final Fire must fail for B's source-observer FCS, not unrelated stock/domain/envelope conditions");
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

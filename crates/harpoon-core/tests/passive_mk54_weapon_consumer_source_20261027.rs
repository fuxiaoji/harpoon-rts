//! Source-only5.4.6.3 ->5.4.6.6 ->6.3.7 ->9.7.1 physical Mk54 consumer.
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
    // Six ordinary original bodies are allocated first; normal A/B/C drop
    // commands then allocate7/8/9. This only selects an ordinary scenario seed.
    // No trial is stored or changed. All fixture prerequisites still require
    // actual native reports from the real allocated bodies below.
    // The first genuinely monitored Search float must also pass ordinary
    // unboosted detection, retaining native contact into the next Tac.
    // This adds only a pure master-seed prerequisite, never a stored trial.
    let baseline = fixture_baselines();
    let bonus = harpoon_core::rules::detection2026::sonobuoys::tables()["passive_localization"]
        ["alerted_operator_modifier_percent"]
        .as_f64()
        .unwrap();
    (20261203..20261203 + 65536)
        .find(|seed| {
            [(1i64, vec![7u32, 8, 9]), (2, vec![7, 8, 9])]
                .into_iter()
                .all(|(epoch, ids)| {
                    ids.into_iter().all(|buoy| {
                        let key = format!("5.4.6:{buoy}:{SENSOR}:6");
                        let mut rng = Rng::new(*seed).derive(Rng::hash_tag(epoch, 0, &key));
                        let maximum = if epoch == 1 || buoy == 7 {
                            baseline[(buoy - 7) as usize]
                        } else {
                            (baseline[(buoy - 7) as usize] + bonus).min(90.)
                        };
                        f64::from(1 + (rng.next_f64() * 100.) as u8) <= maximum
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
fn pw01_actual_passive_localization_and_earned_sub_classification_feed_normal_mk54_plot_and_physical_drop(
) {
    // An ordinary pure scenario seed preserves real first-float Search
    // detection into the classification turn. Longer lifetime preserves the finite
    // actual floats during the short navigation/release experiment.
    let mut w = fixture(true);
    let actor = id(&w, "THREE");
    let target = id(&w, "TARGET");
    let ids = buoys(&w);
    assert_eq!(w.entity(actor).unwrap().class, "us_mh60r_mk54x4");
    assert!(w.entity(actor).unwrap().sensors.is_empty());
    assert_eq!(rounds(&w, actor), 4);
    assert!((payload_kg(&w, actor) - 1141.).abs() < 1e-9);
    assert_eq!(w.stats.projectiles_launched, 0);

    to(&mut w, 185);
    acquired_search(&mut w, actor, ids[0]);
    let search_contact = w.find_contact(Side::Blue, target).unwrap();
    assert!(w.contact(Side::Blue, search_contact).unwrap().bearing_only);
    assert!(
        plot(actor, search_contact)
            .validate_source(&w, Side::Blue)
            .is_err(),
        "one received passive Search bearing cannot launch a sourced Mk54 plan"
    );
    assert_eq!(rounds(&w, actor), 4);
    let localize = select(&w, actor, &ids, "localization");
    issue(&mut w, localize);
    to(&mut w, 359);
    assert!(fix(&w, actor, "Good").is_none());
    let search_key = format!("{}:{SENSOR}:{}", ids[0].0, target.0);
    let search_track = &w.rules.detection.as_ref().unwrap().tracks[&search_key];
    assert!(search_track.held && search_track.first_success.is_some_and(|at|at.div_euclid(180)==1),
        "fixture needs the actually monitored first float's real ordinary Search contact retained from TacticalTurn1, before classification in Tac2");
    while w.tick < 530 && fix(&w, actor, "Good").is_none() {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let located = fix(&w, actor, "Good")
        .expect("the actually selected three current-Tac passive bearings earn Good");
    assert_eq!(w.tick.div_euclid(180), 2);
    assert_eq!(located["buoys"], json!(ids));
    let received = observations(&w, actor);
    assert_eq!(received.len(), 3);
    assert!(received.iter().all(|r| r["measured_position"].is_null()));
    let aim: Vec2 = serde_json::from_value(located["measured_position"].clone())
        .expect("own published bearing-derived position is the only navigation aim");
    assert!(aim.range_to(intersection(&received, ids[0], ids[1])) < 1e-6);
    let contact: harpoon_core::ContactId = serde_json::from_value(located["contact"].clone())
        .expect("use this actual receiver's own published contact alias");

    // The source boundary is the actual ScanDecision, not the SensorReport's
    // referee target pointer or hidden body kind. LMF can earn only the
    // ship/sub domain after a retained prior-Tac passive detection.
    let classified_buoy = ids[0];
    assert!(received.iter().any(|report|report["observer"]==json!(classified_buoy)
        && report["at"].as_i64().is_some_and(|at|at.div_euclid(180)==2)),
        "the actually classified Search source must be selected and genuinely received in the localization turn");
    let native = original_trial(&w, classified_buoy);
    let expected = fixture_baselines();
    for (index, bid) in ids.iter().enumerate() {
        let trial = w
            .rules
            .detection
            .as_ref()
            .unwrap()
            .sonobuoys
            .receiver_trials
            .values()
            .find(|t| t.receiver == actor && t.buoy == *bid && t.target == target && t.at == w.tick)
            .unwrap();
        assert_eq!(
            trial.base_probability, expected[index],
            "the seed factory must match actual source baseline for each physical float"
        );
        assert_eq!(trial.probability, (expected[index] + 15.).min(90.));
        assert!(trial.detected);
    }
    assert!(native["classification"]["level"].as_u64().is_some_and(|v|v>=1),
        "at least one actually selected/received native Search source must earn next-Tac domain classification; no rule requires all three positioning floats to classify independently: {native}");
    assert_eq!(
        harpoon_core::rules::detection2026::known_category(&w, Side::Blue, target),
        Some(harpoon_core::types::PlatformKind::Sub),
        "5.4.6.6 classification actually earned by selected received native sonar must reach the legitimate monitor; a report's hidden target pointer supplies no category"
    );
    let profile = &w.rules.torpedoes.as_ref().unwrap().config.weapons["us_mk54"];
    let seeker = profile.generation.max(1);
    let torpedo_speed = profile.settings[0].speed_kts;
    let solution = harpoon_core::rules::firecontrol2026::attack_solution(
        &w,
        actor,
        target,
        seeker,
        torpedo_speed,
    )
    .expect("the receiver's real passive solution, never an injected aircraft contact track");
    assert_eq!(
        solution.quality,
        harpoon_core::rules::firecontrol2026::Quality::Good
    );
    assert!(solution.native.is_none());
    assert!(solution.reported_aim.unwrap().range_to(aim) < 1e-6);
    assert!(
        harpoon_core::rules::detection2026::sonobuoys::located_report(&w, actor, target).is_none(),
        "passive intersected bearings must never be rewritten as a ranged SensorReport"
    );

    // These are real commands using only the own published aim. The author
    // initial2100m Medium station requires a normal150m descent to1950m Low.
    issue(
        &mut w,
        Command::SetAltitude {
            unit: actor,
            altitude_m: 1950.,
        },
    );
    issue(&mut w, throttle(actor, "Cruise"));
    issue(
        &mut w,
        Command::Goto {
            unit: actor,
            waypoints: vec![aim],
        },
    );
    while w.tick < 520 && w.entity(actor).unwrap().pos.range_to(aim) > 0.04 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(
        w.entity(actor).unwrap().pos.range_to(aim) <= 0.04,
        "normal navigation must physically reach the own derived trial station"
    );
    issue(&mut w, throttle(actor, "Hover"));
    while w.tick < 530
        && (w.entity(actor).unwrap().altitude_m > 1950.0001
            || w.entity(actor).unwrap().speed_kts > 0.0001)
    {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!((w.entity(actor).unwrap().altitude_m - 1950.).abs() < 1e-4);
    assert!(w.entity(actor).unwrap().speed_kts <= 1e-4);
    assert!(
        w.entity(actor).unwrap().pos.range_to(aim) < 0.1,
        "explicit author trial station tolerance, not a book uncertainty radius"
    );
    assert_eq!(rounds(&w, actor), 4);
    let plan = plot(actor, contact);
    issue(&mut w, plan);
    let before = w.entity(actor).unwrap().pos;
    let payload_before = payload_kg(&w, actor);
    let earned = harpoon_core::rules::firecontrol2026::attack_solution(
        &w,
        actor,
        target,
        seeker,
        torpedo_speed,
    )
    .expect("the actual receiver solution must remain current at weapon release");
    assert_eq!(
        earned.quality,
        harpoon_core::rules::firecontrol2026::Quality::Good
    );
    let fire = Command::Fire {
        unit: actor,
        contact,
        weapon: "us_mk54".into(),
        rounds: 1,
    };
    fire.validate_source(&w, Side::Blue).expect(
        "normal source Fire must accept this classified own passive fix and legal hover envelope",
    );
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for world in [&mut w, &mut restored] {
        harpoon_core::sim::tick(world, &[(Side::Blue, fire.clone())]);
    }
    assert_eq!(w.stats.projectiles_launched, 1,
        "the real Fire consumer must create one physical torpedo from the qualified passive solution");
    assert_eq!(rounds(&w, actor), 3);
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&actor].external_weapon_rounds["us_mk54"],
        3
    );
    assert!(
        (payload_before - payload_kg(&w, actor) - 285.).abs() < 1e-9,
        "one real helo Mk54 removes exactly its sourced285kg, preserving the author1kg monitor"
    );
    let state = w.rules.torpedoes.as_ref().unwrap();
    let (projectile, release) = state
        .launches
        .iter()
        .find(|(_, r)| r.shooter == actor && r.weapon == "us_mk54")
        .expect("the actual torpedo release ledger, not merely accepted intent");
    assert_eq!(release.contact, Some(contact));
    let flight = &state.flights[projectile];
    assert_eq!(flight.quality, earned.quality);
    assert!(
        flight.order.solution.is_none(),
        "no authored Good bypass may enter the physical flight"
    );
    assert!(flight.origin.range_to(before) < 1e-9);
    let entry = flight
        .airdrop_entry
        .as_ref()
        .expect("the real aircraft creates its actual water-entry record");
    assert!(entry.position.range_to(before) < 1e-9);
    assert!(!entry.source_standoff_placed);
    // A successful §9.7.1 entry can legitimately place, attack and consume
    // its real body within this same tick. Require the actual launch body's
    // typed receipt plus its concrete final lifecycle, not an artificial
    // requirement that a spent projectile survive the normal cleanup pass.
    let relevant: Vec<Value> = w
        .events
        .iter()
        .filter_map(|event| {
            if event.tick != w.tick {
                return None;
            }
            let EventKind::RulesEvent {
                domain,
                rule,
                detail,
                ..
            } = &event.kind
            else {
                return None;
            };
            let detail: Value = serde_json::from_str(detail).ok()?;
            let id = json!(projectile);
            (detail.get("torpedo") == Some(&id) || detail.get("projectile") == Some(&id))
                .then(|| json!({"tick":event.tick,"domain":domain,"rule":rule,"detail":detail}))
        })
        .collect();
    let diagnostic = json!({
        "tick":w.tick,"torpedo":projectile,
        "body":w.entity(*projectile).map(|body|json!({"kind":body.kind,"alive":body.alive,
            "pos":body.pos,"depth_m":body.depth_m})),
        "flight":{"phase":flight.phase,"placement":flight.placement,
            "entry":flight.airdrop_entry,"last_probability":flight.last_probability,
            "last_d100":flight.last_d100,"fuel":flight.fuel},
        "pending_adjudication":w.rules.pending_adjudications.contains_key(projectile),
        "same_tick_rules":relevant
    });
    let physical_launches: Vec<_> = relevant
        .iter()
        .filter(|row| {
            row["domain"] == "torpedo2026"
                && row["rule"] == "10.2.source_launch"
                && row["tick"] == json!(release.launch_tick)
        })
        .collect();
    assert_eq!(
        physical_launches.len(),
        1,
        "exactly one actual physical source launch receipt is required: {diagnostic}"
    );
    let physical = &physical_launches[0]["detail"]["physical_launch"];
    assert_eq!(physical["kind"],json!(harpoon_core::types::PlatformKind::Torpedo),
        "launch receipt must read the genuinely constructed torpedo body, never infer it from stats: {diagnostic}");
    let physical_position: Vec2 = serde_json::from_value(physical["pos"].clone())
        .expect("actual physical launch position in source receipt");
    assert!(
        physical_position.range_to(before) < 1e-9,
        "physical body, original launch origin and actual entry must coincide: {diagnostic}"
    );
    assert_eq!(
        physical["depth_m"].as_f64(),
        Some(flight.order.depth_m),
        "actual physical launch depth must be the original declared setting: {diagnostic}"
    );
    if let Some(body) = w.entity(*projectile) {
        assert_eq!(
            body.kind,
            harpoon_core::types::PlatformKind::Torpedo,
            "a surviving body must still have its real physical kind: {diagnostic}"
        );
    } else {
        // Absence alone is not success. Prove same-id same-tick entry,
        // successful actual placement and the concrete terminal hit that
        // retires the body before the ordinary projectile cleanup barrier.
        assert_eq!(
            entry.resolved_at,
            Some(w.tick),
            "a consumed body needs its actual same-tick resolved water entry: {diagnostic}"
        );
        assert!(
            flight
                .placement
                .as_ref()
                .is_some_and(|placement| placement.placed),
            "terminal consumption needs the real successful placement: {diagnostic}"
        );
        let terminals: Vec<_> = relevant
            .iter()
            .filter(|row| row["domain"] == "torpedo2026" && row["rule"] == "10.2.5.source_terminal")
            .collect();
        assert_eq!(
            terminals.len(),
            1,
            "a consumed entry body needs one exact concrete terminal receipt: {diagnostic}"
        );
        let terminal = &terminals[0]["detail"];
        assert_eq!(terminal["hit"],json!(true),
            "a miss or unrelated disappearance cannot stand in for physical terminal consumption: {diagnostic}");
        assert_eq!(
            terminal["d100"].as_u64(),
            flight.last_d100.map(u64::from),
            "terminal receipt must retain the original actual flight die: {diagnostic}"
        );
        assert_eq!(
            terminal["probability"].as_f64(),
            flight.last_probability,
            "terminal receipt must retain the actual flight probability: {diagnostic}"
        );
        assert_eq!(
            terminal["pass"],
            json!(flight.pass),
            "terminal receipt must belong to this exact actual pass: {diagnostic}"
        );
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

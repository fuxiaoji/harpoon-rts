//! Source-only5.4.4/5.4.6.3 minimum physical buoy -> real radio monitor chain.
//! Author laboratory carriage/mass/channel/range/depth values are explicit.
//! No runtime buoy body, contact, detection, FCS, inventory or die is inserted.
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
const MODEL: &str = "lab_active_directional";
const CLASS: &str = "lab_sonobuoy";
const SENSOR: &str = "lab_sonobuoy_active";
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn command(actor: EntityId, action: Value) -> Command {
    serde_json::from_value(json!({"cmd":"detection","unit":actor,"action":action}))
        .expect("typed buoy action through existing normal command envelope")
}
fn own(w: &World, actor: EntityId) -> Value {
    let o = harpoon_core::observe::observe(w, Side::Blue, 0, 0);
    let u = o.units.iter().find(|u| u.id == actor).unwrap();
    serde_json::to_value(u).unwrap()["detection"]["sonobuoys"].clone()
}
fn carried(w: &World) -> u64 {
    own(w, id(w, "LAYER"))["carried"][MODEL]
        .as_u64()
        .expect("actual finite carried stock must be observable by its original owner")
}
fn deployed(w: &World) -> Vec<EntityId> {
    own(w, id(w, "LAYER"))["deployed"]
        .as_array()
        .expect("own actual dropped-body inventory")
        .iter()
        .filter_map(|b| serde_json::from_value(b["id"].clone()).ok())
        .collect()
}
fn drop_command(w: &World, depth: f64, channel: u16) -> Command {
    command(
        id(w, "LAYER"),
        json!({"type":"drop_sonobuoy","model":MODEL,"depth_m":depth,"radio_channel":channel}),
    )
}
fn submit(w: &mut World, c: Command) {
    harpoon_core::sim::tick(w, &[(Side::Blue, c)]);
}
fn step_to(w: &mut World, t: i64) {
    while w.tick < t {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn fixture(altitude: f64, sea: u8) -> World {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    db.platforms.get_mut("us_mh60r").unwrap().sensors.clear();
    db.platforms.get_mut("us_mh60r").unwrap().weapons.clear();
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    db.platforms.get_mut("us_ford").unwrap().sensors = vec![SENSOR.into()];
    let mut sensor = db.sensors.values().next().unwrap().clone();
    sensor.id = SENSOR.into();
    sensor.name =
        "scenario_assumption: independent active directional laboratory buoy receiver".into();
    sensor.kind = harpoon_core::db::SensorKind::SonarActive;
    sensor.air_range_nm = 0.;
    sensor.surface_range_nm = 0.;
    sensor.missile_range_nm = 0.;
    sensor.sub_range_nm = 8.;
    sensor.alt_m = 0.;
    db.sensors.insert(SENSOR.into(), sensor);
    let mut buoy = db.platforms["us_ford"].clone();
    buoy.id = CLASS.into();
    buoy.name = "scenario_assumption: noncombatant physical sensor float".into();
    buoy.hp = 1.;
    buoy.max_speed_kts = 0.;
    buoy.cruise_speed_kts = 0.;
    buoy.turn_rate_deg_s = 0.;
    buoy.noise_db = 0.;
    buoy.vp_value = 0.;
    buoy.endurance_s = 0.;
    buoy.max_depth_m = 0.;
    buoy.chaff = 0;
    buoy.decoys = 0;
    buoy.deck = None;
    buoy.rule_data = None;
    buoy.rcs = serde_json::from_value(json!("VerySmall")).unwrap();
    buoy.sensors = vec![SENSOR.into()];
    buoy.weapons.clear();
    db.platforms.insert(CLASS.into(), buoy);
    db.validate().unwrap();
    let m = json!({"source":"scenario_assumption: two actual1kg test floats, no real aircraft/K2/K3 fitting claim","platform_class":CLASS,"sensor":SENSOR,"active":true,"directional":true,"allowed_depths_m":[15.],"lifetime_s":1800,"mass_kg":1.,"sea_state6_capable":false,"radio_antenna_height_m":0.2,"radio_channels":[1,2,3],"protocol":"lab-buoy-vhf"});
    let monitor = json!({"source":"scenario_assumption: explicitly installed one-channel experimental receiver/processor, not a real K3 claim","search_channels":0,"passive_localization_channels":0,"active_channels":1,"processing_per_tactical_turn":1,"protocol":"lab-buoy-vhf"});
    let scene = json!({"id":"sonobuoy-physical-source","name":"scenario_assumption: minimum source buoy lifecycle and qualified monitoring","seed":20261203,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{
        "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":sea,"precipitation":"None"},
        "aviation_2026":{"source":"scenario_assumption: original real helicopter Flight with explicit trial payload","loads":{"us_mh60r":{"payload_kg":2.,"tanks":[],"source":"scenario_assumption: layer carries two1kg floats; monitor test boards use the same explicit2kg trial load"}},"parking":{},"initial_fuel_nm":{"LAYER":200.,"NEAR":200.,"FAR":200.,"LOW":200.}},
        "detection_2026":{"bindings":{format!("{CLASS}:{SENSOR}"):{"type":"sonar","source":"scenario_assumption: real deployed Gen5 active LMF8nm50percent receiver,360degree Buoy mount","generation":5,"passive_range_nm":null,"active_range_nm":8.,"frequency":"lmf","mount":"buoy","active_torpedo_detection":false,"high_speed_tow":false},format!("us_ford:{SENSOR}"):{"type":"sonar","source":"scenario_assumption: separately muted ordinary director enabled only for the later legacy-source-bypass check","generation":5,"passive_range_nm":null,"active_range_nm":8.,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false}},
          "environment":{"source":"scenario_assumption: explicit controlled open water","sea_state":sea,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":200.,"layer":null,"ice":0,"cloud":null},
          "targets":{"cn_093":{"source":"scenario_assumption: original target controlled acoustic attributes","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}},
          "sonobuoys":{"source":"scenario_assumption: physical laboratory carries and radio/processor hardware; source algorithms only","models":{MODEL:m},"carriers":{"LAYER":{"source":"scenario_assumption: two sealed actual floats on this original body, accounted by its real Flight payload","loads":{MODEL:2}}},"monitors":{"NEAR":monitor.clone(),"FAR":monitor.clone(),"LOW":monitor}}
        }},"units":[
          {"side":"Blue","class":"us_mh60r","name":"LAYER","pos":[0,0],"speed_kts":0,"altitude_m":altitude,"fire_on":false},
          {"side":"Blue","class":"us_mh60r","name":"NEAR","pos":[1,0],"speed_kts":0,"altitude_m":1000,"fire_on":false},
          {"side":"Blue","class":"us_mh60r","name":"FAR","pos":[41,0],"speed_kts":0,"altitude_m":1000,"fire_on":false},
          {"side":"Blue","class":"us_mh60r","name":"LOW","pos":[25,0],"speed_kts":0,"altitude_m":61,"fire_on":false},
          {"side":"Blue","class":"us_ford","name":"DIRECTOR","pos":[0,1],"course_deg":180,"speed_kts":0,"sonar_active":false,"fire_on":false},
          {"side":"Red","class":"cn_093","name":"TARGET","pos":[0.2,0],"speed_kts":0,"depth_m":50,"fire_on":false}]});
    let sc:Scenario=serde_json::from_value(scene).expect("normal source red must reach strict sonobuoy configuration, not refer to a nonexistent Rust variant");
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let commands: Vec<_> = ["LAYER", "NEAR", "FAR", "LOW"]
        .iter()
        .map(|name| {
            let a = id(&w, name);
            let c: Command = serde_json::from_value(
                json!({"cmd":"aviation","unit":a,"action":{"type":"throttle","setting":"Hover"}}),
            )
            .unwrap();
            c.validate_source(&w, Side::Blue)
                .expect("each actual original helicopter can hover with its real source Flight");
            (Side::Blue, c)
        })
        .collect();
    harpoon_core::sim::tick(&mut w, &commands);
    w
}
#[test]
fn sb01_real_finite_carriage_depth_setting_mass_and_next_turn_transmission_include_broken_drop_losses(
) {
    let mut w = fixture(1000., 0);
    let layer = id(&w, "LAYER");
    let target = id(&w, "TARGET");
    assert_eq!(carried(&w), 2);
    let bad_depth = drop_command(&w, 17., 1);
    assert!(
        bad_depth.validate_source(&w, Side::Blue).is_err(),
        "unknown physical depth setting is not a free adjustable hydrophone"
    );
    assert_eq!(carried(&w), 2);
    let initial_payload = w.rules.aviation.as_ref().unwrap().flights[&layer].payload_fraction;
    let max_payload = w.rules.aviation.as_ref().unwrap().flights[&layer]
        .spec
        .max_payload_kg;
    let at = w.tick;
    let position = w.entity(layer).unwrap().pos;
    for channel in [1, 2] {
        let c = drop_command(&w, 15., channel);
        c.validate_source(&w, Side::Blue)
            .expect("actual airborne original store is physically releasable");
        submit(&mut w, c);
    }
    assert_eq!(carried(&w), 0);
    assert!(
        (w.rules.aviation.as_ref().unwrap().flights[&layer].payload_fraction
            - (initial_payload - 2. / max_payload))
            .abs()
            < 1e-9,
        "real Flight payload decreases by exactly the two physically released masses"
    );
    let ids = deployed(&w);
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    for b in &ids {
        let e = w.entity(*b).expect("actual water body exists");
        assert!(e.alive);
        assert_eq!(e.class, CLASS);
        assert!(
            e.pos.range_to(position) < 1e-9,
            "no target-estimate teleport; the actor actually flies over its own release point"
        );
    }
    let extra = drop_command(&w, 15., 3);
    assert!(extra.validate_source(&w, Side::Blue).is_err());
    assert_eq!(deployed(&w).len(), 2);
    let activation = (at.div_euclid(180) + 1) * 180;
    step_to(&mut w, activation - 1);
    assert!(
        own(&w, layer)["deployed"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b["transmitting"] == false),
        "no radio/acoustic publication in the drop Tactical Turn"
    );
    step_to(&mut w, activation + 2);
    assert!(own(&w, layer)["deployed"]
        .as_array()
        .unwrap()
        .iter()
        .all(|b| b["transmitting"] == true));
    assert!(w.find_contact(Side::Blue,target).is_none(),"unmonitored physical buoy detections cannot silently enter the side-fused intelligence book");
    assert!(
        harpoon_core::rules::firecontrol2026::attack_solution(&w, layer, target, 0, 0.).is_err(),
        "the dropping plane is not automatically the monitoring receiver or a Good FCS"
    );
    for (altitude, sea) in [(30., 0), (1000., 6)] {
        let mut broken = fixture(altitude, sea);
        let c = drop_command(&broken, 15., 1);
        c.validate_source(&broken, Side::Blue)
            .expect("a deliberate physical release is distinct from survival at impact");
        submit(&mut broken, c);
        assert_eq!(
            carried(&broken),
            1,
            "a buoy broken by VLow impact or unsupported sea state is still physically expended"
        );
        assert!(
            broken
                .entities
                .values()
                .all(|e| e.class != CLASS || !e.alive),
            "a broken float must never become an operational observer"
        );
    }
}
#[test]
fn sb02_actual_selected_in_range_one_channel_receiver_gets_one_buoy_fair_without_free_far_or_layer_fcs(
) {
    let mut w = fixture(1000., 0);
    let layer = id(&w, "LAYER");
    let near = id(&w, "NEAR");
    let far = id(&w, "FAR");
    let low = id(&w, "LOW");
    let target = id(&w, "TARGET");
    for channel in [1, 2] {
        let c = drop_command(&w, 15., channel);
        c.validate_source(&w, Side::Blue).unwrap();
        submit(&mut w, c);
    }
    let ids = deployed(&w);
    assert_eq!(ids.len(), 2);
    step_to(&mut w, 185);
    assert!(
        w.find_contact(Side::Blue, target).is_none(),
        "raw physical detections before actual listening are not remotely earned reports"
    );
    let over = command(near, json!({"type":"listen_sonobuoys","buoys":ids}));
    assert!(over.validate_source(&w,Side::Blue).is_err(),"K3/author active channel and processing capacity is one, not the total number of floats in the water");
    let c = command(near, json!({"type":"listen_sonobuoys","buoys":[ids[0]]}));
    c.validate_source(&w, Side::Blue)
        .expect("real installed in-range one-channel receiver selects a known own float");
    let distant = command(far, json!({"type":"listen_sonobuoys","buoys":[ids[0]]}));
    let low_horizon = command(low, json!({"type":"listen_sonobuoys","buoys":[ids[0]]}));
    harpoon_core::sim::tick(
        &mut w,
        &[
            (Side::Blue, c),
            (Side::Blue, distant),
            (Side::Blue, low_horizon),
        ],
    );
    let until = w.tick + 720;
    while w.tick < until
        && !own(&w, near)["fixes"]
            .as_array()
            .is_some_and(|a| a.iter().any(|f| f["quality"] == "Fair"))
    {
        assert!(!w.over);
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let fixes = own(&w, near)["fixes"]
        .as_array()
        .expect("actual qualified monitor fixes")
        .clone();
    let fair = fixes
        .iter()
        .find(|f| f["quality"] == "Fair")
        .expect("one actually detecting/selected active-directional float yields source Fair");
    assert_eq!(fair["buoys"], json!([ids[0]]));
    assert!(
        fair["target"].is_null(),
        "no enemy true EntityId in own monitoring projection"
    );
    let fcs = harpoon_core::rules::firecontrol2026::attack_solution(&w, near, target, 0, 0.)
        .expect("the actual received source localization must reach the ordinary FCS consumer");
    assert_eq!(
        fcs.quality,
        harpoon_core::rules::firecontrol2026::Quality::Fair
    );
    assert!(harpoon_core::rules::firecontrol2026::attack_solution(&w,far,target,0,0.).is_err(),"41nm is outside the printed40nm maximum even though that plane declares listening and shares a side CID");
    assert!(
        harpoon_core::rules::firecontrol2026::attack_solution(&w, layer, target, 0, 0.).is_err(),
        "laying the field does not itself qualify the aircraft as a radio monitor"
    );
    assert!(
        harpoon_core::rules::firecontrol2026::attack_solution(&w, low, target, 0, 0.).is_err(),
        "25nm is inside40nm but outside the actual61m aircraft/0.2m float radio horizon"
    );
    assert!(own(&w, low)["fixes"]
        .as_array()
        .is_none_or(|a| a.is_empty()));
    assert!(own(&w, far)["fixes"]
        .as_array()
        .is_none_or(|a| a.is_empty()));
    // Isolate the historical producer's actual quality>0.4 prerequisite
    // using an ordinary real sensor, never an inserted contact or quality.
    let director = id(&w, "DIRECTOR");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSonar {
                unit: director,
                active: true,
            },
        )],
    );
    let native_until = w.tick + 360;
    while w.tick < native_until
        && !w.sides[0]
            .contacts
            .values()
            .any(|c| c.about == target && !c.stale && c.quality > 0.4)
    {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.sides[0].contacts.values().any(|c|c.about==target&&!c.stale&&c.quality>0.4),"real native acquisition establishes the existing historical-producer precondition without changing quality");
    let before = w
        .entities
        .values()
        .filter(|e| e.class == CLASS && e.alive)
        .count();
    let legacy_before = w
        .entities
        .values()
        .filter(|e| e.class == "sonobuoy" && e.alive)
        .count();
    let legacy_events = w.event_seq;
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::RetaskAircraft {
                unit: layer,
                mission: harpoon_core::commands::AirMission::Asw {
                    area: harpoon_core::Vec2::new(0., 0.),
                    radius_nm: 1.,
                },
                manual: false,
            },
        )],
    );
    let until = w.tick + 125;
    step_to(&mut w, until);
    assert_eq!(carried(&w), 0);
    assert_eq!(
        w.entities
            .values()
            .filter(|e| e.class == CLASS && e.alive)
            .count(),
        before
    );
    assert_eq!(
        w.entities
            .values()
            .filter(|e| e.class == "sonobuoy" && e.alive)
            .count(),
        legacy_before,
        "v4 source inventory cannot be bypassed by the historical unlimited120s automatic producer"
    );
    assert!(!w.events.iter().any(|e| e.seq >= legacy_events
        && matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="sonobuoy_laid")));
}

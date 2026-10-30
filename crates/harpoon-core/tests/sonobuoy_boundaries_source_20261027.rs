//! Boundary follow-up to the accepted source buoy2 chain; no production mutation.
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
fn fixture_parameters(
    altitude: f64,
    sea: u8,
    lifetime: i64,
    with_mk54: bool,
    payload: f64,
) -> Result<World, String> {
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
    let m = json!({"source":"scenario_assumption: two actual1kg test floats, no real aircraft/K2/K3 fitting claim","platform_class":CLASS,"sensor":SENSOR,"active":true,"directional":true,"allowed_depths_m":[15.],"lifetime_s":lifetime,"mass_kg":1.,"sea_state6_capable":false,"radio_antenna_height_m":0.2,"radio_channels":[1,2,3],"protocol":"lab-buoy-vhf"});
    let monitor = json!({"source":"scenario_assumption: explicitly installed one-channel experimental receiver/processor, not a real K3 claim","search_channels":0,"passive_localization_channels":0,"active_channels":1,"processing_per_tactical_turn":1,"protocol":"lab-buoy-vhf"});
    let mut scene = json!({"id":"sonobuoy-physical-source","name":"scenario_assumption: minimum source buoy lifecycle and qualified monitoring","seed":20261203,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{
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
    if with_mk54 {
        // Reuse the already source-qualified exact four-Mk54 helicopter fit.
        // The extra test floats remain an authored mass-accounting experiment,
        // not a new real-world MH60R/K2/K3 combined load claim.
        scene["units"][0]["class"] = json!("us_mh60r_mk54x4");
        scene["context"]["aviation_2026"]["loads"]["us_mh60r_mk54x4"] = json!({
            "payload_kg":payload,"tanks":[],"external_weapon_rounds":{"us_mk54":4},
            "source":"US Aircraft2025 PDF69/B64 and PDF93/E1 noteC existing four285kg helo weapons; scenario_assumption: separately declared two1kg laboratory floats"});
    }
    let sc:Scenario=serde_json::from_value(scene).expect("normal source red must reach strict sonobuoy configuration, not refer to a nonexistent Rust variant");
    let mut w = harpoon_core::load_scenario(sc, db)?;
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
    Ok(w)
}
fn full_world_equal(a: &World, b: &World) {
    let a_bytes = serde_json::to_vec(a).unwrap();
    let b_bytes = serde_json::to_vec(b).unwrap();
    assert!(
        a_bytes == b_bytes,
        "complete World diverged at T{} / T{}; lengths {} / {}; first differing byte {:?}",
        a.tick,
        b.tick,
        a_bytes.len(),
        b_bytes.len(),
        a_bytes.iter().zip(&b_bytes).position(|(x, y)| x != y)
    );
}
fn pair_to(a: &mut World, b: &mut World, tick: i64) {
    while a.tick < tick {
        assert!(!a.over && !b.over);
        harpoon_core::sim::tick(a, &[]);
        harpoon_core::sim::tick(b, &[]);
    }
    full_world_equal(a, b);
}
fn fair(w: &World, monitor: EntityId) -> bool {
    own(w, monitor)["fixes"]
        .as_array()
        .is_some_and(|fs| fs.iter().any(|f| f["quality"] == "Fair"))
}
fn float_view(w: &World, buoy: EntityId) -> Value {
    own(w, id(w, "LAYER"))["deployed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| serde_json::from_value::<EntityId>(f["id"].clone()).unwrap() == buoy)
        .unwrap()
        .clone()
}
#[test]
fn sb03_actual_selected_field_full_world_restore179_to180_and_expiry_keeps_inventory_mass_and_source_clock(
) {
    // The exact expiry is an explicit short author trial value, not an Annex
    // lifetime assertion. All original acoustic/radio hardware stays identical.
    let mut w = fixture_parameters(1000., 0, 780, false, 2.).unwrap();
    let layer = id(&w, "LAYER");
    let near = id(&w, "NEAR");
    let target = id(&w, "TARGET");
    for channel in [1, 2] {
        let c = drop_command(&w, 15., channel);
        c.validate_source(&w, Side::Blue).unwrap();
        submit(&mut w, c);
    }
    let ids = deployed(&w);
    assert_eq!(ids.len(), 2);
    assert_eq!(carried(&w), 0);
    let empty_payload = w.rules.aviation.as_ref().unwrap().flights[&layer].payload_fraction;
    assert!(empty_payload.abs() < 1e-12);
    step_to(&mut w, 178);
    let select = command(near, json!({"type":"listen_sonobuoys","buoys":[ids[0]]}));
    select.validate_source(&w, Side::Blue).unwrap();
    submit(&mut w, select);
    assert_eq!(w.tick, 179);
    assert_eq!(float_view(&w, ids[0])["transmitting"], false);
    assert!(!fair(&w, near));
    let checkpoint = serde_json::to_vec(&w).unwrap();
    let mut restored: World = serde_json::from_slice(&checkpoint).unwrap();
    full_world_equal(&w, &restored);
    pair_to(&mut w, &mut restored, 180);
    for body in ids.iter().copied() {
        assert_eq!(float_view(&w, body)["transmitting"], true);
        assert_eq!(float_view(&w, body)["active_at"], 180);
        assert!(w.entity(body).is_some_and(|e| e.alive));
    }
    let until = 720;
    while w.tick < until && !fair(&w, near) {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    full_world_equal(&w, &restored);
    assert!(
        fair(&w, near),
        "original real detector/radio pipeline must earn the monitor's Fair, never an inserted fix"
    );
    let fcs =
        harpoon_core::rules::firecontrol2026::attack_solution(&w, near, target, 0, 0.).unwrap();
    assert_eq!(
        fcs.quality,
        harpoon_core::rules::firecontrol2026::Quality::Fair
    );
    // Replacing a selection in the same Tac cannot reuse the one distinct-
    // float processing budget. A refusal leaves the complete state untouched.
    let switch = command(near, json!({"type":"listen_sonobuoys","buoys":[ids[1]]}));
    let before = serde_json::to_vec(&w).unwrap();
    assert!(switch.validate_source(&w, Side::Blue).is_err());
    assert!(serde_json::to_vec(&w).unwrap() == before);
    let stop = command(near, json!({"type":"listen_sonobuoys","buoys":[]}));
    stop.validate_source(&w, Side::Blue).unwrap();
    submit(&mut w, stop.clone());
    submit(&mut restored, stop);
    full_world_equal(&w, &restored);
    assert!(!fair(&w, near));
    assert!(
        harpoon_core::rules::firecontrol2026::attack_solution(&w, near, target, 0, 0.).is_err(),
        "held side knowledge is not an active selected radio fix"
    );
    let resume = command(near, json!({"type":"listen_sonobuoys","buoys":[ids[0]]}));
    resume.validate_source(&w, Side::Blue).unwrap();
    submit(&mut w, resume.clone());
    submit(&mut restored, resume);
    full_world_equal(&w, &restored);
    let expiry = float_view(&w, ids[0])["expires_at"].as_i64().unwrap();
    pair_to(&mut w, &mut restored, expiry - 1);
    assert_eq!(float_view(&w, ids[0])["transmitting"], true);
    pair_to(&mut w, &mut restored, expiry);
    assert_eq!(float_view(&w, ids[0])["transmitting"], false);
    assert_eq!(float_view(&w, ids[0])["lost"], true);
    assert!(!fair(&w, near));
    assert!(
        harpoon_core::rules::firecontrol2026::attack_solution(&w, near, target, 0, 0.).is_err()
    );
    assert!(w.entity(ids[0]).is_none_or(|e| !e.alive));
    let lost = command(near, json!({"type":"listen_sonobuoys","buoys":[ids[0]]}));
    assert!(lost.validate_source(&w, Side::Blue).is_err());
    let last_expiry = ids
        .iter()
        .map(|id| float_view(&w, *id)["expires_at"].as_i64().unwrap())
        .max()
        .unwrap();
    pair_to(&mut w, &mut restored, last_expiry + 1);
    assert_eq!(carried(&w), 0);
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&layer].payload_fraction,
        empty_payload
    );
    assert!(ids.iter().all(|id| float_view(&w, *id)["lost"] == true));
    assert_eq!(
        w.entities
            .values()
            .filter(|e| e.class == CLASS && e.alive)
            .count(),
        0
    );
}
#[test]
fn sb04_existing_declared_four_mk54_mass_cannot_be_reused_for_two_physical_float_items() {
    // Only the already verified existing four-round/285kg helicopter fit is
    // reused. No new weapon, sensor, real combined fit or table value is added.
    let err = fixture_parameters(1000., 0, 1800, true, 1140.).unwrap_err();
    assert!(err.contains("5.4 original Flight payload must include every buoy plus"),
        "known4*285kg weapons already fill1140kg; buoy initialization must be the rejecting consumer, got: {err}");
    let mut w = fixture_parameters(1000., 0, 1800, true, 1142.).unwrap();
    let layer = id(&w, "LAYER");
    let av = &w.rules.aviation.as_ref().unwrap().flights[&layer];
    let max = av.spec.max_payload_kg;
    assert!((av.payload_fraction * max - 1142.).abs() < 1e-9);
    assert_eq!(av.external_weapon_rounds["us_mk54"], 4);
    assert!(
        (harpoon_core::rules::aviation2026::remaining_declared_store_mass_kg(&w, layer).unwrap()
            - 1140.)
            .abs()
            < 1e-9
    );
    let actual_rounds = |w: &World| {
        w.entity(layer)
            .unwrap()
            .weapons
            .iter()
            .filter(|m| m.weapon == "us_mk54")
            .map(|m| m.ammo as u32)
            .sum::<u32>()
    };
    assert_eq!(actual_rounds(&w), 4);
    for (channel, remaining, mass) in [(1, 1, 1141.), (2, 0, 1140.)] {
        let c = drop_command(&w, 15., channel);
        c.validate_source(&w, Side::Blue).unwrap();
        submit(&mut w, c);
        assert_eq!(carried(&w), remaining);
        assert!(
            (w.rules.aviation.as_ref().unwrap().flights[&layer].payload_fraction * max - mass)
                .abs()
                < 1e-9
        );
        assert_eq!(actual_rounds(&w), 4);
        assert_eq!(
            w.rules.aviation.as_ref().unwrap().flights[&layer].external_weapon_rounds["us_mk54"],
            4
        );
        assert!(
            (harpoon_core::rules::aviation2026::remaining_declared_store_mass_kg(&w, layer)
                .unwrap()
                - 1140.)
                .abs()
                < 1e-9
        );
    }
    let before = serde_json::to_vec(&w).unwrap();
    assert!(drop_command(&w, 15., 3)
        .validate_source(&w, Side::Blue)
        .is_err());
    assert!(serde_json::to_vec(&w).unwrap() == before);
    assert_eq!(deployed(&w).len(), 2);
}

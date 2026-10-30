//! Artifact-only §4.6.5 /4.6.6 shared author seaplane domain.
//! No future Rust enums/fields, no Ka27/real-Annex capability alias, no injected
//! HP/draw/clock/mode/Flight. Explicit Scenario card enters normal JSON paths.
use harpoon_core::rules::aviation2026 as av;
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, Vec2, World};
use serde_json::{json, Value};

const CLASS: &str = "lab_seaplane_max240_fmp200";
fn card() -> Value {
    json!({
        "source":"scenario_assumption: complete authored Small turboprop seaplane card; no real Annex identity or Ka27 water capability claim",
        "seaplane":true,
        "maximum_speed_kts":240.0,
        "fmp_low_kts":200.0,
        "low_altitude_m":300.0,
        "maneuver_light":1.0,"maneuver_heavy":1.0,"ceiling_m":2000.0,
        "flight_spec":{"source":"scenario_assumption: authored laboratory engine/fuel/mass and Low-only flight row, not real Annex","engine":"TP","size":1,"cruise_kts":120.0,"cruise_range_nm":800.0,"fuel_kg":1200.0,"max_payload_kg":1000.0,"receive":null,"tanker_slots":0,"centerline":false},
        "load":{"source":"scenario_assumption: unarmed author laboratory with zero carried stores","payload_kg":0.0,"tanks":[],"buddy":false,"external_weapon_rounds":{},"internal_weapon_rounds":{}},
        "platform":{"id":CLASS,"name":"Author Small turboprop floatplane max240 FMP200","kind":"Aircraft","hp":10.0,"max_speed_kts":240.0,"cruise_speed_kts":120.0,"turn_rate_deg_s":20.0,"accel_kts_per_s":10.0,"rcs":"Small","noise_db":50.0,"visual_factor":1.0,"vp_value":1.0,"sensors":[],"weapons":[],"chaff":0,"decoys":0,"endurance_s":24000.0,"max_depth_m":0.0,"dipping_sonar":false,"is_base":false,"deck":null}
    })
}
fn fixture(swamping: bool) -> World {
    let unit = |name: &str, point: Value| json!({"side":"Blue","class":CLASS,"name":name,"pos":point,"course_deg":90.0,"speed_kts":0.0,"altitude_m":0.0,"fire_on":false});
    let mut units = vec![unit("OPEN", json!([0.0, 0.0]))];
    let mut bindings = json!({"OPEN":{"source":"scenario_assumption: this same physical airframe starts resting on open sea, engines stopped","phase":"waterborne","location":"open","initial_fuel_nm":800.0,"mooring":null}});
    if swamping {
        units.extend([
            unit("LEE1", json!([0.0, 10.0])),
            unit("LEE2", json!([0.0, 20.0])),
            unit("MOORED", json!([0.0, 30.0])),
        ]);
        bindings["LEE1"] = json!({"source":"scenario_assumption: author lee-one location binding","phase":"waterborne","location":"lee_one","initial_fuel_nm":800.0,"mooring":null});
        bindings["LEE2"] = json!({"source":"scenario_assumption: author lee-two location binding","phase":"waterborne","location":"lee_two","initial_fuel_nm":800.0,"mooring":null});
        bindings["MOORED"] = json!({"source":"scenario_assumption: aircraft initially moored at declared author buoy; no winch/approach duration claim","phase":"waterborne","location":"open","initial_fuel_nm":800.0,"mooring":{"source":"scenario_assumption: initial secured mooring","kind":"buoy","key":"LAB_BUOY","point":{"x":0.0,"y":30.0}}});
    }
    units.extend([
        json!({"side":"Blue","class":"us_virginia","name":"CONTROL","pos":[40.0,0.0],"speed_kts":0.0,"depth_m":50.0,"fire_on":false}),
        json!({"side":"Red","class":"cn_093","name":"OTHER","pos":[80.0,0.0],"speed_kts":0.0,"depth_m":50.0,"fire_on":false})
    ]);
    let input = json!({"id":"waterborne-source","name":"author floatplane movement and independent natural onceTac swamping; no real Annex card","seed":20261027,"duration_s":3600,
        "weather":{"visibility_nm":0.0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":if swamping {6}else{3},"precipitation":"None"},
        "aviation_2026":{"loads":{},"parking":{},"source":"scenario_assumption: ordinary aviation state plus separately declared complete waterborne cards","visibility_percent":100.0},
        "waterborne_2026":{"source":"scenario_assumption: explicit laboratory water positions and Movement-phase adapter, not cable/slipway physics","cards":{(CLASS):card()},"units":bindings,
            "environment":{"source":"scenario_assumption: wind from east12kt, explicitly described calm/lee locations","sea_state":if swamping {6}else{3},"wind_from_deg":90.0,"wind_speed_kts":12.0,
                "locations":{"open":{"source":"scenario_assumption: open water, no shelter","shelter_states":0},"lee_one":{"source":"scenario_assumption: described coastal lee reduction1","shelter_states":1},"lee_two":{"source":"scenario_assumption: described coastal lee reduction2","shelter_states":2}}},
            "rts_mapping":{"source":"scenario_assumption: source end-of-Movement completion maps to the normal RTS movement-end barrier; no printed elapsed takeoff/landing seconds inferred","completion":"normal_movement_end","resting_motion":"hold_author_position","airborne_scope":"Low level flight only"}}},"units":units});
    let sc: Scenario = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(serde_json::to_value(&sc).unwrap()["context"]["waterborne_2026"],input["context"]["waterborne_2026"],"source-red checkpoint: complete card and source-local water/shelter declarations must survive normal Scenario config");
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    db.platforms.insert(
        CLASS.into(),
        serde_json::from_value(card()["platform"].clone()).unwrap(),
    );
    for class in ["us_virginia", "cn_093"] {
        db.platforms.get_mut(class).unwrap().sensors.clear();
    }
    harpoon_core::load_scenario(sc, db).unwrap()
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn action(unit: EntityId, kind: &str) -> Command {
    serde_json::from_value(json!({"cmd":"aviation","unit":unit,"action":{"type":kind}})).expect("source-red checkpoint: water_takeoff/water_land are ordinary JSON actions, no future Rust enum compile error")
}
fn steer(unit: EntityId, kind: &str, value: f64) -> Command {
    let v = if kind == "set_speed" {
        json!({"cmd":kind,"unit":unit,"speed_kts":value})
    } else {
        json!({"cmd":kind,"unit":unit,"course_deg":value})
    };
    serde_json::from_value(v).unwrap()
}
fn local(w: &World, unit: EntityId) -> Value {
    av::own_view(w, unit).unwrap()["waterborne"].clone()
}
fn receipt(w: &World, unit: EntityId, ok: bool) {
    assert!(w.events.iter().any(|e|e.tick==w.tick&&matches!(&e.kind,EventKind::RulesEvent {domain,rule,unit:Some(u),detail,..} if domain=="command"&&*u==unit&&rule==if ok{"executed"}else{"rejected"}&&detail.starts_with("aviation:"))),"normal source command receipt");
}
fn source_rows(w: &World, unit: EntityId, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                domain,
                rule: r,
                unit: Some(u),
                detail,
                ..
            } if domain == "operations" && r == rule && *u == unit => {
                Some(serde_json::from_str::<Value>(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn close(actual: Vec2, expected: Vec2) {
    assert!(
        actual.range_to(expected) < 1e-9,
        "real physical body displacement: {actual:?} vs {expected:?}"
    );
}
fn clone_world(w: &World) -> World {
    serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap()
}
fn pair(w: &mut World, r: &mut World, commands: &[(Side, Command)]) {
    harpoon_core::sim::tick(w, commands);
    harpoon_core::sim::tick(r, commands);
    assert_eq!(
        serde_json::to_value(&*w).unwrap(),
        serde_json::to_value(&*r).unwrap(),
        "native save preserves same airframe/mode/card/fuel/dice and real losses"
    );
}

#[test]
fn wa01_same_body_water_air_water_uses_fmp_for_takeoff_maximum_for_landing_and_real_wind() {
    let mut w = fixture(false);
    let actor = id(&w, "OPEN");
    let initial_count = w.entities.len();
    assert_eq!(local(&w, actor)["phase"], "waterborne");
    assert!(
        !av::own_view(&w, actor).unwrap()["flight_data_blocked"]
            .as_bool()
            .unwrap(),
        "complete author card must feed actual shared flight/fuel state, not only a water helper"
    );
    let point = w.entity(actor).unwrap().pos;
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, action(actor, "water_takeoff"))]);
    receipt(&w, actor, true);
    let e = w.entity(actor).unwrap();
    assert_eq!(local(&w, actor)["phase"], "airborne");
    assert_eq!(e.class, CLASS);
    assert_eq!(e.id, actor);
    assert_eq!(e.speed_kts, 50.0, "25% of authored FMP200, not maximum240");
    assert_eq!(e.ordered_speed_kts, 50.0);
    assert_eq!(e.altitude_m, 300.0);
    assert_eq!(e.ordered_altitude_m, 300.0);
    assert_eq!(e.course_deg, 90.0, "into actual wind-from direction");
    close(e.pos, point);
    assert_eq!(
        w.entities.len(),
        initial_count,
        "no new deck airframe or replacement body"
    );
    let launch_rows = source_rows(&w, actor, "4.6.5.water_takeoff");
    assert_eq!(launch_rows.len(), 1);
    assert_eq!(launch_rows[0]["speed_basis"], "fmp");
    let mut r = clone_world(&w);
    let start = w.entity(actor).unwrap().pos;
    pair(&mut w, &mut r, &[]);
    close(
        w.entity(actor).unwrap().pos,
        start.advanced(90.0, 50.0 / 3600.0),
    );
    let commands = [(Side::Blue, action(actor, "water_land"))];
    pair(&mut w, &mut r, &commands);
    receipt(&w, actor, false);
    assert_eq!(
        local(&w, actor)["phase"],
        "airborne",
        "50kt is25%FMP and must not pass the separate25%maximum landing gate"
    );
    pair(
        &mut w,
        &mut r,
        &[(Side::Blue, steer(actor, "set_speed", 60.0))],
    );
    assert_eq!(w.entity(actor).unwrap().speed_kts, 60.0);
    pair(
        &mut w,
        &mut r,
        &[(Side::Blue, steer(actor, "set_course", 0.0))],
    );
    for _ in 0..10 {
        if w.entity(actor).unwrap().course_deg.abs() < 1e-9 {
            break;
        }
        pair(&mut w, &mut r, &[]);
    }
    assert!(w.entity(actor).unwrap().course_deg.abs() < 1e-9);
    pair(&mut w, &mut r, &[(Side::Blue, action(actor, "water_land"))]);
    receipt(&w, actor, false);
    assert_eq!(
        local(&w, actor)["phase"],
        "airborne",
        "wrong actual wind heading cannot land"
    );
    pair(
        &mut w,
        &mut r,
        &[(Side::Blue, steer(actor, "set_course", 90.0))],
    );
    for _ in 0..10 {
        if (w.entity(actor).unwrap().course_deg - 90.0).abs() < 1e-9 {
            break;
        }
        pair(&mut w, &mut r, &[]);
    }
    assert!((w.entity(actor).unwrap().course_deg - 90.0).abs() < 1e-9);
    let before = w.entity(actor).unwrap().pos;
    let fuel_before = av::own_view(&w, actor).unwrap()["flight"]["fuel_cruise_equivalent_nm"]
        .as_f64()
        .unwrap();
    pair(&mut w, &mut r, &[(Side::Blue, action(actor, "water_land"))]);
    receipt(&w, actor, true);
    let e = w.entity(actor).unwrap();
    close(e.pos, before.advanced(90.0, 60.0 / 3600.0));
    assert_eq!(e.id, actor);
    assert_eq!(e.altitude_m, 0.0);
    assert_eq!(e.speed_kts, 0.0);
    assert_eq!(e.ordered_altitude_m, 0.0);
    assert_eq!(e.ordered_speed_kts, 0.0);
    assert_eq!(local(&w, actor)["phase"], "waterborne");
    assert_eq!(w.entities.len(), initial_count);
    let land_rows = source_rows(&w, actor, "4.6.5.water_land");
    assert_eq!(land_rows.len(), 1);
    assert_eq!(land_rows[0]["speed_basis"], "maximum");
    assert!(
        av::own_view(&w, actor).unwrap()["flight"]["fuel_cruise_equivalent_nm"]
            .as_f64()
            .unwrap()
            < fuel_before,
        "actual airborne movement uses complete fuel consumer, no free restore/refuel"
    );
    let parked = e.pos;
    for _ in 0..3 {
        pair(&mut w, &mut r, &[]);
        close(w.entity(actor).unwrap().pos, parked);
        assert!(
            source_rows(&w, actor, "4.5.2.low_crash").is_empty(),
            "surface-resting aircraft is not airborne SSkim"
        );
    }
    assert_eq!(w.losses[Side::Blue.index()], 0);
}

#[test]
fn wa02_natural_once_tac_swamp_with_explicit_lee_and_mooring_reaches_actual_aircraft_loss() {
    let mut w = fixture(true);
    let actors = [
        id(&w, "OPEN"),
        id(&w, "LEE1"),
        id(&w, "LEE2"),
        id(&w, "MOORED"),
    ];
    let points = actors.map(|a| w.entity(a).unwrap().pos);
    for _ in 0..179 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(w.tick, 179);
    for (i, a) in actors.iter().enumerate() {
        assert!(source_rows(&w, *a, "4.6.6.swamp").is_empty());
        close(w.entity(*a).unwrap().pos, points[i]);
        assert!(source_rows(&w, *a, "4.5.2.low_crash").is_empty());
    }
    assert_eq!(w.losses[0], 0);
    let mut r = clone_world(&w);
    pair(&mut w, &mut r, &[]);
    assert_eq!(w.tick, 180);
    for (i, chance) in [0.3, 0.2, 0.1].into_iter().enumerate() {
        let a = actors[i];
        let rows = source_rows(&w, a, "4.6.6.swamp");
        assert_eq!(rows.len(), 1, "one source trial at normal completedTac");
        let row = &rows[0];
        assert!((row["chance"].as_f64().unwrap() - chance).abs() < 1e-12);
        assert_eq!(row["sea_state"].as_u64(), Some(6));
        assert_eq!(row["shelter_states"].as_u64(), Some(i as u64));
        let natural = w.scope_rng(a, "4.6.6.swamp").next_f64();
        assert_eq!(
            row["roll_uniform"].as_f64(),
            Some(natural),
            "ordinary independent world scope draw, no injected die"
        );
        assert_eq!(row["lost"].as_bool(), Some(natural < chance));
        assert_eq!(w.entity(a).is_none_or(|e| !e.alive), natural < chance);
    }
    assert!(
        source_rows(&w, actors[3], "4.6.6.swamp").is_empty(),
        "moored exception consumes no swamping draw"
    );
    assert!(w.entity(actors[3]).unwrap().alive);
    assert_eq!(local(&w, actors[3])["moored"], true);
    assert!(w.entity(actors[2]).is_none_or(|e|!e.alive),"ordinary seed20261027: lee2 scoped draw is below10%, so real swamping death must be reached");
    assert_eq!(w.losses[0], 1);
    assert_eq!(w.losses_by_kind["aircraft"][0], 1);
    assert!(
        w.events
            .iter()
            .any(|e| matches!(&e.kind,EventKind::Ditched{unit,..}if *unit==actors[2])),
        "existing actual common aircraft loss receipt"
    );
    for _ in 0..179 {
        pair(&mut w, &mut r, &[]);
    }
    assert_eq!(w.tick, 359);
    for a in actors.iter().take(3) {
        assert_eq!(
            source_rows(&w, *a, "4.6.6.swamp").len(),
            1,
            "extra frames/restore in sameTac cannot reroll"
        );
    }
    let alive_before: Vec<_> = actors
        .iter()
        .take(3)
        .copied()
        .filter(|a| w.entity(*a).is_some_and(|e| e.alive))
        .collect();
    pair(&mut w, &mut r, &[]);
    assert_eq!(w.tick, 360);
    for a in alive_before {
        assert_eq!(
            source_rows(&w, a, "4.6.6.swamp").len(),
            2,
            "next naturalTac has exactly one new independent trial"
        );
    }
    assert_eq!(
        source_rows(&w, actors[2], "4.6.6.swamp").len(),
        1,
        "dead body cannot be repeatedly lost/charged"
    );
    assert!(w.entity(actors[3]).unwrap().alive);
    assert!(source_rows(&w, actors[3], "4.6.6.swamp").is_empty());
}


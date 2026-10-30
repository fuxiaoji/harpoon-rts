//! Native Wake budget candidates, unregistered/uncompiled/unexecuted.
//! Source50kt/27nm exact65-76A unchanged; author wake geometry/Good/CID91
//! isolate source mechanics. No Flight/phase/die/fuel/HP injection.
use harpoon_core::{Command, ContactId, Database, EntityId, Scenario, Side, Vec2, World};
use serde_json::{json, Value};

fn fixture(seed: u64, history: bool, offset: bool, geometry: bool) -> (World, EntityId, EntityId) {
    let db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let mut weapon = json!({"source":"Russia2025 PDF90/F-1 exact65-76A Kit;F-2 remarkE; historical1998 fixture",
        "guidance":"Wake","generation":3,"settings":[{"speed_kts":50,"range_nm":27}],
        "max_depth_m":25,"shallow_water_capable":false,"surface":true,"submarine":false,"max_sea_state":4});
    if offset {
        weapon["launch_offset"] = json!({"source":"scenario_assumption: controlled650mm tube permits stated bearing only; not a book Wake default","degrees":0});
    }
    let mut cfg = json!({"source":"scenario_assumption: controlled historical1998 Wake mechanism acceptance",
        "weapons":{"ru_65k":weapon},
        "environment":{"source":"scenario_assumption: deep open water no thermocline or bottom reverberation","layer_m":null,"reverberant":false},
        "targets":{"cn_001":{"source":"scenario_assumption: controlled carrier size/noise/coating","size":0,"noise":2,"anechoic":false,"foil_borne":false}},
        "platforms":{"ru_oscar2":{"source":"scenario_assumption: exercise enables exactly one initial650mm test tube; full Oscar reload/refit not claimed",
            "wire_capacity":0,"reload_parallel":1,"tubes":[{"bearing_deg":90,"loaded":"ru_65k"}],"tube_ids":{"TEST650":0},"reserves":{}}},
        "orders":{"SHOOTER:ru_65k":{"source":"scenario_assumption: declared bearing crosses astern of known northbound target",
            "speed_index":0,"depth_m":5,"enable_run_nm":0,"bearing_deg":90,"spread_deg":[],"cut_wire":true,
            "solution":{"quality":"Good","source":"scenario_assumption: known source firing solution"}}}});
    if geometry {
        cfg["wake"] = json!({"source":"scenario_assumption: declared geometric footprint, not a source constant",
        "half_width_nm":0.02,"lifetime_s":600,"history_policy":"explicit_segments_only",
        "initial_segments":if history{json!([{"target_unit":"TARGET","from":{"x":0,"y":-1},"to":{"x":0,"y":1},"created_tick":-30}])}else{json!([])}});
    }
    let sc:Scenario=serde_json::from_value(json!({"id":"wake-source-1998","name":"Source Wake physical crossing",
        "duration_s":4000,"seed":seed,"context":{"rules_profile":"hv2026-rts-v4",
        "radar_environment":{"sea_state":4,"precipitation":"None"},"torpedo_2026":cfg},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"ru_oscar2","name":"SHOOTER","pos":[-1,-0.5],"course_deg":0,"speed_kts":0,"depth_m":25,"fire_on":false},
            {"side":"Red","class":"cn_001","name":"TARGET","pos":[0,1],"course_deg":0,"speed_kts":0,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let s = w
        .entities
        .values()
        .find(|e| e.name == "SHOOTER")
        .unwrap()
        .id;
    let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    w.entity_mut(s).unwrap().sensors.clear();
    w.entity_mut(t).unwrap().sensors.clear();
    w.entity_mut(t).unwrap().weapons.clear();
    w.sides[0].contacts.insert(
        ContactId(91),
        harpoon_core::world::Contact {
            id: ContactId(91),
            about: t,
            category: harpoon_core::types::PlatformKind::Ship,
            class_name: None,
            estimate: Vec2::new(0., 1.),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec![],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", t.0),
        harpoon_core::types::PlatformKind::Ship,
    );
    (w, s, t)
}
fn ammo(w: &World, s: EntityId) -> u16 {
    w.entity(s)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "ru_65k")
        .unwrap()
        .ammo
}
fn fire(s: EntityId) -> (Side, Command) {
    (
        Side::Blue,
        Command::Fire {
            unit: s,
            contact: ContactId(91),
            weapon: "ru_65k".into(),
            rounds: 1,
        },
    )
}
fn events(w: &World, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent {
                rule: r, detail, ..
            } if r == rule => Some(serde_json::from_str(detail).unwrap()),
            _ => None,
        })
        .collect()
}
fn launch(w: &mut World, s: EntityId) -> EntityId {
    let before = ammo(w, s);
    harpoon_core::sim::tick(w, &[fire(s)]);
    assert_eq!(
        ammo(w, s),
        before - 1,
        "a fully bound source Wake example must enter the real release pipeline"
    );
    *w.rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .expect("source Flight for the real released torpedo")
}

fn source_roll(seed: u64, purpose: &str) -> u8 {
    let mut rng = harpoon_core::Rng::new(seed).derive(harpoon_core::Rng::hash_tag(0, 3, purpose));
    1 + (rng.next_f64() * 100.) as u8
}

#[test]
fn native_wake_never_acquired_last_tick_cannot_exceed_remaining_source_budget() {
    let (mut world, shooter, _) = fixture(20261023, false, true, true);
    let torpedo = launch(&mut world, shooter);
    let mut reached_last = false;
    for _ in 0..2500 {
        let f = &world.rules.torpedoes.as_ref().unwrap().flights[&torpedo];
        let before = f.run_nm;
        let old_fuel = f.fuel;
        let setting = &f.weapon.settings[f.speed_index];
        let affordable = (1. - old_fuel).max(0.) * setting.range_nm;
        // 27nm/50kt is an integer1944s: include an exact-boundary final tick
        // within numeric tolerance, without waiting for an already retired body.
        let final_tick = affordable <= setting.speed_kts / 3600. + 1e-10;
        harpoon_core::sim::tick(&mut world, &[]);
        assert!(!world.rules.pending_adjudications.contains_key(&torpedo));
        if final_tick {
            reached_last = true;
            let f = &world.rules.torpedoes.as_ref().unwrap().flights[&torpedo];
            assert!(
                f.run_nm - before <= affordable + 1e-10,
                "Wake source remaining range {affordable}nm cannot pay for added run {}nm",
                f.run_nm - before
            );
            assert!(
                (f.fuel - 1.).abs() < 1e-12,
                "Wake spent fraction stops at1, got {}",
                f.fuel
            );
            assert!(!world.entity(torpedo).is_some_and(|e| e.alive));
            break;
        }
    }
    assert!(
        reached_last,
        "real50kt/27nm Wake run must actually exhaust source endurance"
    );
    assert!(events(&world, "10.2.6.wake_crossing").is_empty());
    assert!(events(&world, "10.2.6.wake_terminal").is_empty());
    assert_eq!(events(&world, "10.2.6.wake_runout").len(), 1);
}

#[test]
fn native_wake_miss_and_right_reverse_cannot_recharge_endurance_by_approaching_origin() {
    let seed = (1..=100000)
        .find(|seed| source_roll(*seed, "10.2.6.attack:0") > 70)
        .unwrap();
    let (mut world, shooter, _) = fixture(seed, true, true, true);
    let torpedo = launch(&mut world, shooter);
    for _ in 0..600 {
        if !events(&world, "10.2.7.wake_reverse").is_empty() {
            break;
        }
        harpoon_core::sim::tick(&mut world, &[]);
        assert!(!world.rules.pending_adjudications.contains_key(&torpedo));
    }
    assert_eq!(
        events(&world, "10.2.6.wake_crossing").len(),
        1,
        "real source wake capture precedes the first attack"
    );
    let attacks = events(&world, "10.2.6.wake_terminal");
    assert_eq!(attacks.len(), 1);
    assert_eq!(
        attacks[0]["hit"], false,
        "native source D100 actually misses"
    );
    assert_eq!(
        events(&world, "10.2.7.wake_reverse").len(),
        1,
        "actual500yd overshoot then source180deg right turn"
    );
    let f = &world.rules.torpedoes.as_ref().unwrap().flights[&torpedo];
    assert!(matches!(
        f.phase,
        harpoon_core::rules::torpedo2026::Phase::Search { .. }
    ));
    let old_fuel = f.fuel;
    let old_run = f.run_nm;
    let range = f.weapon.settings[f.speed_index].range_nm;
    let speed = f.weapon.settings[f.speed_index].speed_kts;
    let before = world.entity(torpedo).unwrap().pos;
    let origin = f.origin;
    harpoon_core::sim::tick(&mut world, &[]);
    assert!(!world.rules.pending_adjudications.contains_key(&torpedo));
    let after = world.entity(torpedo).unwrap().pos;
    assert!(
        after.range_to(origin) < before.range_to(origin),
        "real reversed travel approaches its launch point"
    );
    let f = &world.rules.torpedoes.as_ref().unwrap().flights[&torpedo];
    let nominal = speed / 3600.;
    assert!(
        f.run_nm >= old_run,
        "cumulative source run cannot shrink when moving toward the origin"
    );
    assert!((f.fuel-old_fuel-nominal/range).abs()<1e-10,
  "a full source straight search second must consume nominal/range, never recharge fuel: {} -> {}",old_fuel,f.fuel);
}

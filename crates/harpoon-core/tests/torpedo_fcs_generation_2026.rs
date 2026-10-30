//! Virginia(ii) Cbt Sys Gen6, US2025 PDF16/A13, is an independent
//! §6.3 acoustic modifier input; it does not establish §6.4 tube capacities.
use harpoon_core::rules::{firecontrol2026 as fcs, torpedo2026 as torp};
use harpoon_core::{ContactId, Database, EntityId, Scenario, Side, Vec2, World};
use serde_json::json;

fn fixture() -> (World, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc:Scenario=serde_json::from_value(json!({"id":"virginia-own-fcs","name":"Controlled own acoustic evidence",
        "duration_s":1000,"seed":20261024,"context":{"rules_profile":"hv2026-rts-v4","torpedo_2026":{
            "source":"scenario_assumption: controlled physical tube/target setup, no author FCS",
            "environment":{"source":"scenario_assumption: no layer","layer_m":null,"reverberant":false},
            "targets":{"cn_093":{"source":"scenario_assumption: controlled target metadata","size":2,"noise":2,"anechoic":false,"foil_borne":false}},
            "platforms":{"us_virginia":{"source":"scenario_assumption: two loaded tubes, not a full source fit",
                "wire_capacity":2,"reload_parallel":2,"tubes":[{"bearing_deg":0,"loaded":"us_mk48"},{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}},
            "orders":{"SHOOTER:us_mk48":{"source":"scenario_assumption: settings only, solution absent","speed_index":0,"depth_m":50,
                "enable_run_nm":0,"bearing_deg":0,"cut_wire":true,"spread_deg":[],"solution":null}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,0.5],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let actor = EntityId(1);
    let target = EntityId(2);
    for e in w.entities.values_mut() {
        e.fire_on = false;
    }
    let c=serde_json::from_value(json!({"id":99,"about":target,"category":"Sub","class_name":null,
        "estimate":{"x":0,"y":0.5},"est_course_deg":0,"est_speed_kts":0,"bearing_only":false,"quality":1,
        "last_update_tick":0,"first_tick":0,"stale":false,"sensors":["sonar_a_hull"],"offset":{"x":0,"y":0},"announced":true})).unwrap();
    w.sides[0].contacts.insert(ContactId(99), c);
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert("0:2".into(), harpoon_core::types::PlatformKind::Sub);
    assert!(w
        .entity(actor)
        .unwrap()
        .sensors
        .iter()
        .any(|s| s.sensor == "sonar_a_hull"));
    // Explicit own measured successes isolate FCS generation. These are
    // referee-fixture reports, not a claim that a generic sonar is a real fit.
    for tick in 0..=180 {
        w.tick = tick;
        w.sides[0]
            .contacts
            .get_mut(&ContactId(99))
            .unwrap()
            .last_update_tick = tick;
        fcs::record_own_contact(&mut w, Side::Blue, actor, "sonar_a_hull", target).unwrap();
    }
    (w, actor, target)
}

#[test]
fn exact_virginia_cbt_system_binding_has_source_and_does_not_fill_asw_capacities() {
    let row = &fcs::tables()["platform_combat_systems"]["us_virginia"];
    assert_eq!(row["generation"], 6);
    assert!(row["source"]
        .as_str()
        .is_some_and(|s| s.contains("A-13") && s.contains("2025")));
    assert!(row.get("wire_capacity").is_none());
    assert!(row.get("track_capacity").is_none());
    assert!(fcs::tables()["platform_combat_systems"]
        .get("scenario_assumption_unknown_sub")
        .is_none());
}

#[test]
fn actual_own_measured_acoustic_track_uses_generation_six_active_modifier() {
    let (w, actor, target) = fixture();
    let s = fcs::world_solution(&w, actor, target, 4, 45.)
        .expect("source own track with exact Virginia combat system");
    assert_eq!(s.modifiers["combat_system"], 1);
    assert_eq!(s.quality, fcs::Quality::Good);
}

#[test]
fn acoustic_source_solution_does_not_read_secret_target_lifecycle_or_motion() {
    let (w, actor, target) = fixture();
    let a = fcs::world_solution(&w, actor, target, 4, 45.).unwrap();
    let mut counter = w.clone();
    let hidden = counter.entity_mut(target).unwrap();
    hidden.alive = false;
    hidden.class = "unknown_hidden_hull".into();
    hidden.pos = Vec2::new(1000., 1000.);
    hidden.speed_kts = 30.;
    hidden.depth_m = 600.;
    let b = fcs::world_solution(&counter, actor, target, 4, 45.).unwrap();
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
    counter.entity_mut(actor).unwrap().class = "scenario_assumption_unknown_sub".into();
    assert!(
        fcs::world_solution(&counter, actor, target, 4, 45.).is_err(),
        "an unknown class cannot borrow Virginia Gen6"
    );
}

#[test]
fn actual_torpedo_release_uses_own_fcs_without_author_good_solution() {
    let (mut w, actor, _) = fixture();
    let mount = w
        .entity(actor)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_mk48")
        .unwrap();
    let before = w.entity(actor).unwrap().weapons[mount].ammo;
    assert_eq!(
        torp::fire_at_contact(&mut w, actor, ContactId(99), mount, 1).unwrap(),
        1
    );
    assert_eq!(w.entity(actor).unwrap().weapons[mount].ammo, before - 1);
    assert_eq!(w.stats.projectiles_launched, 1);
    let state = w.rules.torpedoes.as_ref().unwrap();
    let f = state.flights.values().next().unwrap();
    assert_eq!(f.quality, fcs::Quality::Good);
    assert!(f.order.solution.is_none());
    assert!(state
        .launches
        .values()
        .next()
        .unwrap()
        .order
        .as_ref()
        .unwrap()
        .solution
        .is_none());
}

#[test]
fn acoustic_aircraft_cannot_invent_combat_generation_one_from_platform_kind() {
    let (mut w, actor, target) = fixture();
    let own = w.entity_mut(actor).unwrap();
    own.kind = harpoon_core::types::PlatformKind::Aircraft;
    own.class = "scenario_assumption_unknown_acoustic_aircraft".into();
    assert!(
        fcs::world_solution(&w, actor, target, 4, 45.).is_err(),
        "an acoustic modifier needs an actual combat generation; aircraft kind alone does not establish Gen1"
    );
}

#[test]
fn measured_passive_fcs_uses_virginia_generation_six_passive_modifier() {
    let (mut w, actor, target) = fixture();
    w.rules.aaw.as_mut().unwrap().firecontrol.tracks.clear();
    // Controlled own measured reports isolate the passive table modifier;
    // they do not establish a real passive ranging-array hardware fit.
    for tick in 0..=180 {
        w.tick = tick;
        w.sides[0]
            .contacts
            .get_mut(&ContactId(99))
            .unwrap()
            .last_update_tick = tick;
        fcs::record_own_contact(&mut w, Side::Blue, actor, "sonar_p_hull", target).unwrap();
    }
    let solution = fcs::world_solution(&w, actor, target, 4, 45.).unwrap();
    assert_eq!(solution.modifiers["combat_system"], 2);
}

#[test]
fn actual_player_plot_then_fire_uses_measured_fcs_and_preserves_the_selected_report() {
    let (mut w, actor, _) = fixture();
    let action = torp::Action::Plot {
        contact: ContactId(99),
        weapon: "us_mk48".into(),
        speed_index: 0,
        depth_m: 50.,
        enable_run_nm: 0.,
        bearing_deg: Some(0.),
        spread_deg: vec![0.],
        cut_wire: true,
    };
    torp::apply(&mut w, actor, &action).unwrap();
    let mount = w
        .entity(actor)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_mk48")
        .unwrap();
    let before = w.entity(actor).unwrap().weapons[mount].ammo;
    assert_eq!(
        torp::fire_at_contact(&mut w, actor, ContactId(99), mount, 1).unwrap(),
        1
    );
    assert_eq!(w.entity(actor).unwrap().weapons[mount].ammo, before - 1);
    let state = w.rules.torpedoes.as_ref().unwrap();
    let release = state.launches.values().next().unwrap();
    assert_eq!(release.contact, Some(ContactId(99)));
    assert!(release.order.as_ref().unwrap().solution.is_none());
    assert_eq!(
        state.flights.values().next().unwrap().quality,
        fcs::Quality::Good
    );
}

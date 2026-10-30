//! §7.5.4 receive-status eligibility and R-050: the raw wire roll is not
//! an implemented, qualified DualWire report. Actual release remains visible.
use harpoon_core::rules::torpedo2026 as torp;
use harpoon_core::{
    ContactId, Database, EntityId, EventKind, Scenario, Side, Vec2, Visibility, World,
};
use serde_json::{json, Value};

fn fixture() -> (World, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"source-wire-private","name":"Controlled physical tube and declared FCS fixture",
        "duration_s":1000,"seed":20261023,"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
        "navigation_2026":{"source":"scenario_assumption: maintained test speed, not real hardware size fit","units":{"SHOOTER":{
            "source":"scenario_assumption: author size-D underway mechanism fit","size":"D","merchant":false,"passenger_liner":false,
            "cpp":false,"waterjet":false,"undamaged_max_kts":34,"propulsion":"NonSteam","readiness":"Underway"}}},
        "torpedo_2026":{
            "source":"scenario_assumption: mechanism test, not a real complete fit",
            "environment":{"source":"scenario_assumption: no layer or reverberation","layer_m":null,"reverberant":false},
            "targets":{"cn_093":{"source":"scenario_assumption: controlled target signature","size":2,"noise":2,"anechoic":false,"foil_borne":false}},
            "platforms":{"us_virginia":{"source":"scenario_assumption: two loaded mechanism tubes","wire_capacity":2,"reload_parallel":2,
                "tubes":[{"bearing_deg":0,"loaded":"us_mk48"},{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}},
            "orders":{"SHOOTER:us_mk48":{"source":"scenario_assumption: source mechanics settings",
                "speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":0,"spread_deg":[],"cut_wire":false,
                "solution":{"quality":"Good","source":"scenario_assumption: isolates release/wire privacy; not native detection evidence"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":21,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,5],"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut world = harpoon_core::load_scenario(scenario, db).unwrap();
    let actor = EntityId(1);
    for entity in world.entities.values_mut() {
        entity.fire_on = false;
        entity.sensors.clear();
    }
    world.sides[0].contacts.insert(
        ContactId(99),
        harpoon_core::world::Contact {
            id: ContactId(99),
            about: EntityId(2),
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 5.),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: 0,
            stale: false,
            sensors: vec![],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    world
        .rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert("0:2".into(), harpoon_core::types::PlatformKind::Sub);
    (world, actor)
}

fn launch(world: &mut World, actor: EntityId) -> EntityId {
    harpoon_core::sim::tick(
        world,
        &[(
            Side::Blue,
            harpoon_core::Command::Fire {
                unit: actor,
                contact: ContactId(99),
                weapon: "us_mk48".into(),
                rounds: 1,
            },
        )],
    );
    assert_eq!(world.stats.projectiles_launched, 1);
    let state = world.rules.torpedoes.as_ref().unwrap();
    let id = *state.flights.keys().next().unwrap();
    assert!(state.flights[&id].wire, "native seed20261023 launch D10021");
    id
}

fn referee_trace(world: &World, rule_name: &str) -> Value {
    let event = world
        .events
        .iter()
        .find(|e| matches!(&e.kind, EventKind::RulesEvent { rule, .. } if rule == rule_name))
        .expect("actual source event occurred");
    assert!(
        matches!(event.vis, Visibility::Referee),
        "raw {rule_name} event cannot stand in for a qualified source status report"
    );
    let EventKind::RulesEvent { detail, .. } = &event.kind else {
        unreachable!()
    };
    serde_json::from_str(detail).unwrap()
}

fn assert_no_wire_truth_in_own_observation(world: &World) {
    let own = harpoon_core::observe(world, Side::Blue, 0, 1000);
    assert!(!own.events.iter().any(|e| matches!(&e.kind, EventKind::RulesEvent { rule, .. } if rule == "10.2.source_launch" || rule == "7.5.4.wire_check")));
    assert!(
        own.events.iter().any(|e| matches!(
            &e.kind,
            EventKind::WeaponsRelease {
                side: Side::Blue,
                ..
            }
        )),
        "actual own release notification must remain"
    );
    let status = own.units[0].torpedoes.as_ref().unwrap();
    assert_eq!(status["releases"][0]["status"], "unknown");
    assert!(status["releases"][0]["physical_alive"].is_null());
    assert!(!serde_json::to_string(&own).unwrap().contains("wire_intact"));
    assert!(!serde_json::to_string(&own)
        .unwrap()
        .contains("launch_wire_d100"));
}

#[test]
fn real_source_launch_keeps_raw_wire_and_author_quality_in_referee_but_own_release_visible() {
    let (mut world, actor) = fixture();
    launch(&mut world, actor);
    let detail = referee_trace(&world, "10.2.source_launch");
    assert_eq!(detail["launch_wire_d100"], 21);
    assert_eq!(detail["wire_intact"], true);
    assert_eq!(detail["quality"], "Good");
    assert_no_wire_truth_in_own_observation(&world);
}

#[test]
fn real_periodic_wire_roll_and_true_break_remain_referee_and_survive_full_world_restore() {
    let (mut world, actor) = fixture();
    let projectile = launch(&mut world, actor);
    world.entity_mut(actor).unwrap().speed_kts = 21.;
    for _ in 0..180 {
        harpoon_core::sim::tick(&mut world, &[]);
    }
    assert_eq!(world.tick, 181);
    let detail = referee_trace(&world, "7.5.4.wire_check");
    assert!(detail["d100"]
        .as_u64()
        .is_some_and(|v| (1..=100).contains(&v)));
    assert_eq!(detail["probability"], 1.);
    assert_eq!(detail["intact"], false);
    assert!(!world.rules.torpedoes.as_ref().unwrap().flights[&projectile].wire);
    assert_no_wire_truth_in_own_observation(&world);
    let restored: World = serde_json::from_value(serde_json::to_value(&world).unwrap()).unwrap();
    assert_eq!(detail, referee_trace(&restored, "7.5.4.wire_check"));
    assert_no_wire_truth_in_own_observation(&restored);
    assert_eq!(
        serde_json::to_value(&world).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert!(torp::own_view(&restored, Side::Blue, actor).is_some());
}

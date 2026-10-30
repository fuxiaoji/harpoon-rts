//! Actual player queue and AI validation must preserve different authority.
use super::*;
fn fixture() -> Game {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc=scenario_from_json(r#"{"id":"player-source-authority","name":"Player and AI authority","seed":37,"duration_s":300000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4"},"units":[{"side":"Blue","class":"us_ford","name":"PLAYER","pos":[0,0],"fire_on":false},{"side":"Red","class":"cn_001","name":"REMOTE","pos":[1000,0],"fire_on":false}]}"#).unwrap();
    let w = load_scenario(sc, db).unwrap();
    let mut g = Game::new(w.db.clone());
    g.recording = Some(persistence::Recording::new(&w));
    g.world = Some(w);
    for s in &mut g.tactical {
        s.enabled = false;
    }
    g
}
fn apply_queued(g: &mut Game) {
    let c = std::mem::take(&mut g.inbox);
    g.step(&c);
}

fn torpedo_plot_game() -> Game {
    let mut g = fixture();
    let actor = harpoon_core::EntityId(1);
    let target = harpoon_core::EntityId(2);
    let w = g.world.as_mut().unwrap();
    let e = w.entity_mut(actor).unwrap();
    let mut mount = e.weapons[0].clone();
    mount.weapon = "us_mk48".into();
    mount.ammo = 4;
    mount.mounts = 1;
    mount.cooldown_s = 0.;
    e.weapons.push(mount);
    // Own author report isolates the planning boundary: the true remote hull
    // remains a Ship, 1000nm away, while this report is a located Sub at .5nm.
    let contact = serde_json::from_value(json!({"id":99,"about":target,"category":"Sub","class_name":null,
        "estimate":{"x":0,"y":0.5},"est_course_deg":0,"est_speed_kts":0,"bearing_only":false,"quality":1,
        "last_update_tick":0,"first_tick":0,"stale":false,"sensors":["scenario_assumption: own report"],
        "offset":{"x":0,"y":0},"announced":true})).unwrap();
    w.sides[0]
        .contacts
        .insert(harpoon_core::ContactId(99), contact);
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", target.0),
        harpoon_core::types::PlatformKind::Sub,
    );
    g.recording = Some(persistence::Recording::new(w));
    g
}

#[test]
fn source_torpedo_plot_rejects_invalid_parameters_before_player_reservation() {
    let mut g = torpedo_plot_game();
    let actor = harpoon_core::EntityId(1);
    let value = json!({"cmd":"torpedo","unit":actor,"action":{"type":"plot","contact":99,"weapon":"us_mk48",
        "speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":0,"spread_deg":[],"cut_wire":false}});
    for (field, bad) in [
        ("speed_index", json!(999)),
        ("depth_m", json!(1e9)),
        ("enable_run_nm", json!(1e9)),
        ("contact", json!(99999)),
    ] {
        let mut invalid = value.clone();
        invalid["action"][field] = bad;
        assert!(
            g.queue_player_command(Side::Blue, serde_json::from_value(invalid).unwrap())
                .is_err(),
            "invalid {field} must be refused before reserving control"
        );
        assert!(g.inbox.is_empty());
        assert!(!observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0).units[0].manual_control);
    }
    g.queue_player_command(Side::Blue, serde_json::from_value(value).unwrap())
        .unwrap();
    apply_queued(&mut g);
    let own = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    assert!(own.units[0].manual_control);
    assert_eq!(
        own.units[0].torpedoes.as_ref().unwrap()["plans"][0]["contact"],
        99
    );
    assert!(g
        .recording
        .as_ref()
        .unwrap()
        .verify(g.world.as_ref().unwrap())
        .is_ok());
}

#[test]
fn source_torpedo_ai_batch_preflight_rejects_invalid_spread_before_policy_reservation() {
    let mut g = torpedo_plot_game();
    let actor = harpoon_core::EntityId(1);
    let value = json!({"cmd":"torpedo","unit":actor,"action":{"type":"plot","contact":99,"weapon":"us_mk48",
        "speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":0,"spread_deg":[0,1],"cut_wire":false}});
    let invalid: Command = serde_json::from_value(value.clone()).unwrap();
    let own = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    let (candidates, mut rejected) = agent::validate_commands(&own, Side::Blue, &[invalid]);
    let (accepted, errors) = g.supervise_ai(Side::Blue, candidates);
    rejected.extend(errors);
    assert!(
        accepted.is_empty(),
        "batch must use the same source Plot preflight as a player"
    );
    assert!(rejected.iter().any(|e| e.contains("15deg")), "{rejected:?}");
    assert!(g.inbox.is_empty());
    assert!(!observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0).units[0].manual_control);
    assert!(g
        .world
        .as_ref()
        .unwrap()
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .plans
        .is_empty());
    let mut legal = value;
    legal["action"]["spread_deg"] = json!([0]);
    let own = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    let (candidates, rejected) =
        agent::validate_commands(&own, Side::Blue, &[serde_json::from_value(legal).unwrap()]);
    assert!(rejected.is_empty());
    let (accepted, rejected) = g.supervise_ai(Side::Blue, candidates);
    assert!(rejected.is_empty(), "{rejected:?}");
    assert_eq!(
        accepted.len(),
        1,
        "a legal source Plot still reaches the batch"
    );
}

#[test]
fn invalid_source_antiship_plot_is_rejected_before_reserving_player_control() {
    let mut g = fixture();
    let actor = harpoon_core::EntityId(1);
    let target = harpoon_core::EntityId(2);
    let w = g.world.as_mut().unwrap();
    let own = w.entity_mut(actor).unwrap();
    let mut mount = own.weapons[0].clone();
    mount.weapon = "us_rgm84d_ic".into();
    mount.ammo = 4;
    mount.mounts = 1;
    mount.cooldown_s = 0.;
    own.weapons.push(mount);
    // Author-defined located report. It says60nm; the unobserved physical hull
    // remains1000nm away and is deliberately irrelevant to plan validation.
    let report=serde_json::from_value(json!({"id":71,"about":target,"category":"Ship","class_name":null,
        "estimate":{"x":0,"y":60},"est_course_deg":0,"est_speed_kts":0,"bearing_only":false,"quality":1,
        "last_update_tick":0,"first_tick":-1800,"stale":false,"sensors":["scenario_assumption: own report"],"offset":{"x":0,"y":0},"announced":true})).unwrap();
    w.sides[0]
        .contacts
        .insert(harpoon_core::ContactId(71), report);
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", target.0),
        harpoon_core::types::PlatformKind::Ship,
    );
    g.recording = Some(persistence::Recording::new(w));
    let value = json!({"cmd":"antiship","unit":actor,"action":{"type":"plot","contact":71,"weapon":"us_rgm84d_ic","course_deg":0,"speed_kts":561,"enable_run_nm":40,"waypoints":[],"popup":false}});
    for (field, bad) in [
        ("speed_kts", json!(777)),
        ("enable_run_nm", json!(81)),
        ("popup", json!(true)),
        ("waypoints", json!([{"x":1,"y":2}])),
    ] {
        let mut wrong = value.clone();
        wrong["action"][field] = bad;
        let c = serde_json::from_value(wrong).unwrap();
        assert!(
            g.queue_player_command(Side::Blue, c).is_err(),
            "unsupported {field} must fail before control transfer"
        );
        assert!(g.inbox.is_empty());
        assert!(!observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0).units[0].manual_control);
    }
    g.queue_player_command(Side::Blue, serde_json::from_value(value).unwrap())
        .unwrap();
    apply_queued(&mut g);
    let obs = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    assert!(obs.units[0].manual_control);
    assert_eq!(
        obs.units[0].antiship.as_ref().unwrap()["plots"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        g.recording
            .as_ref()
            .unwrap()
            .verify(g.world.as_ref().unwrap())
            .unwrap()["verified"],
        true
    );
}
#[test]
fn player_authority_continues_after_reservation_while_ai_cannot_release_it() {
    let mut g = fixture();
    let id = harpoon_core::EntityId(1);
    g.queue_player_command(
        Side::Blue,
        Command::SetCourse {
            unit: id,
            course_deg: 40.,
        },
    )
    .unwrap();
    apply_queued(&mut g);
    assert!(observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0).units[0].manual_control);
    g.queue_player_command(
        Side::Blue,
        Command::SetSpeed {
            unit: id,
            speed_kts: 8.,
        },
    )
    .expect("human must be able to continue commanding their reserved unit");
    g.queue_player_command(
        Side::Blue,
        Command::SetCourse {
            unit: id,
            course_deg: 80.,
        },
    )
    .unwrap();
    apply_queued(&mut g);
    let o = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    assert_eq!(o.units[0].ordered_speed_kts, 8.);
    assert_eq!(o.units[0].ordered_course_deg, 80.);
    assert!(agent::validate_commands(
        &o,
        Side::Blue,
        &[Command::SetUnitAutomation {
            unit: id,
            enabled: true
        }]
    )
    .0
    .is_empty());
    g.queue_player_command(
        Side::Blue,
        Command::SetUnitAutomation {
            unit: id,
            enabled: true,
        },
    )
    .expect("only a human may explicitly return their reserved unit to automation");
    apply_queued(&mut g);
    assert!(!observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0).units[0].manual_control);
    assert_eq!(
        g.recording
            .as_ref()
            .unwrap()
            .verify(g.world.as_ref().unwrap())
            .unwrap()["verified"],
        true
    );
}
#[test]
fn a_human_can_begin_real_source_repairs_and_select_assistance_through_the_same_public_queue() {
    let mut g = fixture();
    let id = harpoon_core::EntityId(1);
    harpoon_core::rules::damage_runtime::crash_back_engineering(
        g.world.as_mut().unwrap(),
        id,
        "player-engineering-critical",
    )
    .unwrap();
    g.recording = Some(persistence::Recording::new(g.world.as_ref().unwrap()));
    let o = observe(g.world.as_ref().unwrap(), Side::Blue, 0, 0);
    let begin = Command::DamageControl {
        unit: id,
        action: "begin_repairs".into(),
    };
    assert!(
        agent::validate_commands(&o, Side::Blue, std::slice::from_ref(&begin))
            .0
            .is_empty(),
        "an AI cannot declare the battle over"
    );
    g.queue_player_command(Side::Blue, begin)
        .expect("the human declaration must reach the actual engine");
    apply_queued(&mut g);
    assert!(g
        .world
        .as_ref()
        .unwrap()
        .rules
        .repairs
        .as_ref()
        .unwrap()
        .units[&id]
        .session
        .is_some());
    g.queue_player_command(
        Side::Blue,
        Command::DamageControl {
            unit: id,
            action: "repairs_auto".into(),
        },
    )
    .unwrap();
    apply_queued(&mut g);
    assert!(
        g.world
            .as_ref()
            .unwrap()
            .rules
            .repairs
            .as_ref()
            .unwrap()
            .units[&id]
            .session
            .as_ref()
            .unwrap()
            .automatic
    );
    assert!(g
        .queue_player_command(
            Side::Red,
            Command::DamageControl {
                unit: id,
                action: "repair_due".into()
            }
        )
        .is_err());
    assert_eq!(
        g.recording
            .as_ref()
            .unwrap()
            .verify(g.world.as_ref().unwrap())
            .unwrap()["verified"],
        true
    );
}

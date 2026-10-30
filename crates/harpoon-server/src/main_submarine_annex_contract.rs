//! Four exact Annex game-band caps must reach the shared player/AI gates.
use super::*;
use harpoon_core::EntityId;

fn fixture() -> Game {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"annex-controller", "name":"Source game-band controller boundary",
        "seed":20261024,"duration_s":5000,
        "context":{"rules_profile":"hv2026-rts-v4"},
        "weather":{"visibility_nm":40,"jam":[0,0]},
        "units":[{"side":"Blue","class":"cn_kilo","name":"OWN","pos":[0,0],
                  "depth_m":50,"speed_kts":0,"fire_on":false},
                 {"side":"Red","class":"us_ford","name":"REMOTE","pos":[1000,0],"fire_on":false}]
    }))
    .unwrap();
    let mut world = load_scenario(scenario, db).unwrap();
    for entity in world.entities.values_mut() {
        entity.sensors.clear();
        entity.fire_on = false;
    }
    let mut game = Game::new(world.db.clone());
    for controller in &mut game.tactical {
        controller.enabled = false;
    }
    game.recording = Some(persistence::Recording::new(&world));
    game.world = Some(world);
    game
}

#[test]
fn annex_player_depth_above_source_band_is_refused_before_queue_or_manual_takeover() {
    let mut game = fixture();
    let before = serde_json::to_value(game.world.as_ref().unwrap()).unwrap();
    let rejected = game.queue_player_command(
        Side::Blue,
        Command::SetDepth {
            unit: EntityId(1),
            depth_m: 401.,
        },
    );
    assert!(
        rejected.is_err(),
        "source Int IV cap must reject401 before queued receipt"
    );
    assert!(game.inbox.is_empty());
    assert_eq!(
        before,
        serde_json::to_value(game.world.as_ref().unwrap()).unwrap()
    );
    game.queue_player_command(
        Side::Blue,
        Command::SetDepth {
            unit: EntityId(1),
            depth_m: 400.,
        },
    )
    .unwrap();
    assert!(
        !game.inbox.is_empty(),
        "400m game band is permitted despite historical200m database scalar"
    );
}

#[test]
fn annex_ai_batch_cannot_create_a_task_for_a_depth_above_its_own_source_band() {
    let mut game = fixture();
    let own = observe(game.world.as_ref().unwrap(), Side::Blue, 0, 0);
    let (commands, mut errors) = agent::validate_commands(
        &own,
        Side::Blue,
        &[Command::SetDepth {
            unit: EntityId(1),
            depth_m: 401.,
        }],
    );
    let (accepted, rejected) = game.supervise_ai(Side::Blue, commands);
    errors.extend(rejected);
    assert!(
        accepted.is_empty(),
        "source-invalid depth must not survive either AI entry"
    );
    assert!(!errors.is_empty());
    assert!(game
        .ai_supervision
        .as_ref()
        .is_none_or(|p| !p[0].tasks.contains_key(&EntityId(1))));
    assert!(game.inbox.is_empty());
    assert!(!observe(game.world.as_ref().unwrap(), Side::Blue, 0, 0).units[0].manual_control);
}

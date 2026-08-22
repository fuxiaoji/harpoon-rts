from pathlib import Path

from harpoon.adapters import load_saved_game, load_scenario
from harpoon.domain.models import Command, Phase

SCENARIO = Path("data/scenarios/tattletail.json")


def advance_to(game, target: Phase) -> None:
    while game.state.phase != target:
        if game.state.phase == Phase.PLOTTING:
            game.lock("nato")
            game.lock("soviet")
        game.resolve()


def test_stale_command_is_rejected_without_state_change() -> None:
    game = load_scenario(SCENARIO)
    before = game.state.state_hash()
    result = game.submit(
        Command(
            command_id="old",
            side_id="soviet",
            action_id="wait:smyshlennyy-mod-kashin",
            observation_version=99,
        )
    )
    assert not result.accepted
    assert game.state.state_hash() == before


def test_nato_cannot_fire_first_and_soviet_can() -> None:
    game = load_scenario(SCENARIO)
    nato_fire = next(a for a in game.list_actions("nato") if a.action_type == "fire")
    soviet_fire = next(a for a in game.list_actions("soviet") if a.action_type == "fire")
    assert not nato_fire.available
    assert soviet_fire.available


def test_simultaneous_plots_are_order_independent() -> None:
    def run(reverse: bool) -> str:
        game = load_scenario(SCENARIO)
        commands = [
            Command(
                command_id="a",
                side_id="nato",
                action_id="move:poolster-a835",
                observation_version=0,
                parameters={"course_deg": 25, "speed_knots": 12},
            ),
            Command(
                command_id="b",
                side_id="soviet",
                action_id="move:smyshlennyy-mod-kashin",
                observation_version=0,
                parameters={"course_deg": 0, "speed_knots": 12},
            ),
        ]
        for command in reversed(commands) if reverse else commands:
            assert game.submit(command).accepted
        game.lock("nato")
        game.lock("soviet")
        game.resolve()
        game.resolve()
        return game.state.state_hash()

    assert run(False) == run(True)


def test_launch_moves_next_turn_and_damage_is_recorded() -> None:
    game = load_scenario(SCENARIO)
    command = Command(
        command_id="strike",
        side_id="soviet",
        action_id="fire:smyshlennyy-mod-kashin:p-15m",
        observation_version=0,
        parameters={"target_id": "poolster-a835"},
    )
    assert game.submit(command).accepted
    game.lock("nato")
    game.lock("soviet")
    game.resolve()
    game.resolve()
    events = game.resolve()
    launch = next(e for e in events if e.event_type == "weapon_launched")
    assert launch.payload["eta_turns"] >= 1
    entity = game.state.in_flight_weapons[0]
    assert entity.turns_to_arrival == launch.payload["eta_turns"]
    while not game.state.in_flight_weapons[0].awaiting_adjudication:
        advance_to(game, Phase.MOVEMENT)
        game.resolve()
    event = game.adjudicate_arrival(entity.entity_id, hit=True)
    assert event.random_record and event.random_record["kind"] == "d6"
    assert game.state.units["poolster-a835"].damage_dp >= 55


def test_soviet_launch_starts_nato_readiness_transition() -> None:
    game = load_scenario(SCENARIO)
    command = Command(
        command_id="strike",
        side_id="soviet",
        action_id="fire:smyshlennyy-mod-kashin:p-15m",
        observation_version=0,
        parameters={"target_id": "poolster-a835"},
    )
    assert game.submit(command).accepted
    game.lock("nato")
    game.lock("soviet")
    game.resolve()
    game.resolve()
    game.resolve()
    assert all(
        unit.readiness.value == "transitioning"
        for unit in game.state.units.values()
        if unit.side_id == "nato"
    )


def test_same_seed_and_commands_produce_identical_events() -> None:
    def run() -> tuple[str, list[dict[str, object]]]:
        game = load_scenario(SCENARIO)
        assert game.submit(
            Command(
                command_id="stable-strike",
                side_id="soviet",
                action_id="fire:smyshlennyy-mod-kashin:p-15m",
                observation_version=0,
                parameters={"target_id": "poolster-a835"},
            )
        ).accepted
        game.lock("nato")
        game.lock("soviet")
        game.resolve()
        game.resolve()
        game.resolve()
        return game.state.state_hash(), [event.model_dump(mode="json") for event in game.events]

    assert run() == run()


def test_save_restore_preserves_hash_and_random_progress() -> None:
    game = load_scenario(SCENARIO)
    command = Command(
        command_id="pending",
        side_id="nato",
        action_id="wait:poolster-a835",
        observation_version=0,
    )
    assert game.submit(command).accepted
    path = Path("tmp/tests/save-restore.json")
    path.parent.mkdir(parents=True, exist_ok=True)
    game.save(path)
    restored = load_saved_game(path)
    assert restored.state.state_hash() == game.state.state_hash()
    assert restored.snapshot() == game.snapshot()
    assert restored._commands == game._commands

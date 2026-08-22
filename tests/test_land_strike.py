from pathlib import Path

from harpoon.adapters import load_scenario
from harpoon.domain.models import Command, Phase

SCENARIO = Path("data/scenarios/ryukyu_shockwave_2025.json")


def test_ryukyu_package_has_provenance_and_no_hidden_facility_truth_in_observation() -> None:
    game = load_scenario(SCENARIO)
    assert game.platforms["liaoning-2025"].maximum_dp == 1010
    assert game.platforms["shandong-2025"].maximum_dp == 1023
    assert game.platforms["ronald-reagan-2025"].maximum_dp == 1665
    assert game.ground_facilities["kadena-fuel-depot"].provenance == "scenario_assumption"
    assert game.ground_facilities["naha-runway-18l36r"].provenance == "public_source"
    assert not hasattr(game.observe("china"), "ground_facilities")


def test_land_strike_is_a_legal_public_action_and_launches_in_planned_fire() -> None:
    game = load_scenario(SCENARIO)
    action = next(
        option
        for option in game.list_actions("china", "plarf-cj10-battery")
        if option.action_type == "land_strike"
    )
    assert action.available
    result = game.submit(
        Command(
            command_id="cj10-1",
            side_id="china",
            action_id=action.action_id,
            observation_version=game.state.version,
            parameters={"target_id": "kadena-runway-05r23l"},
        )
    )
    assert result.accepted
    game.lock("china")
    game.lock("usa")
    while game.state.phase != Phase.PLANNED_FIRE:
        game.resolve()
    events = game.resolve()
    launch = next(event for event in events if event.event_type == "land_attack_missile_launched")
    assert launch.actors == ("plarf-cj10-battery", "kadena-runway-05r23l")
    assert game.state.in_flight_weapons[0].target_kind == "ground_facility"

from pathlib import Path

from harpoon.adapters import load_scenario

SCENARIO = Path("data/scenarios/tattletail.json")


def test_scenario_loads_and_hash_is_stable() -> None:
    first = load_scenario(SCENARIO)
    second = load_scenario(SCENARIO)
    assert len(first.state.units) == 6
    assert first.state.state_hash() == second.state.state_hash()
    assert first.platforms["bremen-1986"].maximum_dp == 175
    assert first.platforms["kortenaer-1982"].maximum_dp == 177
    assert first.platforms["poolster-1983"].maximum_dp == 249
    assert first.platforms["leander-b3-1989"].maximum_dp == 157
    assert first.platforms["oh-perry-1989"].maximum_dp == 154
    assert first.platforms["mod-kashin-1975"].maximum_dp == 161


def test_observation_hides_enemy_truth() -> None:
    game = load_scenario(SCENARIO)
    observation = game.observe("nato")
    assert all(unit.side_id == "nato" for unit in observation.own_units)
    assert "smyshlennyy-mod-kashin" not in {unit.unit_id for unit in observation.own_units}
    assert observation.contacts[0].target_unit_id == "smyshlennyy-mod-kashin"


def test_actions_include_unavailable_capabilities_with_explanation() -> None:
    game = load_scenario(SCENARIO)
    actions = game.list_actions("nato")
    air = next(action for action in actions if action.action_type == "air_mission")
    assert not air.available
    assert air.rule.rule_id == "H5-4-AIR"
    assert air.unavailable_reason

from pathlib import Path

from hypothesis import given
from hypothesis import strategies as st

from harpoon.adapters import load_scenario
from harpoon.domain.models import Command

SCENARIO = Path("data/scenarios/tattletail.json")


@given(
    course=st.floats(min_value=0, max_value=359.999, allow_nan=False),
    speed=st.floats(min_value=0, max_value=21, allow_nan=False),
)
def test_valid_poolster_move_never_mutates_before_resolution(course: float, speed: float) -> None:
    game = load_scenario(SCENARIO)
    before = game.state.state_hash()
    result = game.submit(
        Command(
            command_id="property-move",
            side_id="nato",
            action_id="move:poolster-a835",
            observation_version=0,
            parameters={"course_deg": course, "speed_knots": speed},
        )
    )
    assert result.accepted
    assert game.state.state_hash() == before


@given(
    speed=st.one_of(
        st.floats(max_value=-0.001, allow_nan=False),
        st.floats(min_value=21.001, max_value=1000, allow_nan=False),
    )
)
def test_invalid_poolster_speed_is_rejected_without_mutation(speed: float) -> None:
    game = load_scenario(SCENARIO)
    before = game.state.state_hash()
    result = game.submit(
        Command(
            command_id="property-invalid",
            side_id="nato",
            action_id="move:poolster-a835",
            observation_version=0,
            parameters={"course_deg": 25, "speed_knots": speed},
        )
    )
    assert not result.accepted
    assert game.state.state_hash() == before

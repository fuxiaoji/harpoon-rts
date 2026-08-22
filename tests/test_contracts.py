import pytest
from pydantic import ValidationError

from harpoon.domain.models import Command


def test_command_rejects_unknown_fields() -> None:
    with pytest.raises(ValidationError):
        Command.model_validate(
            {
                "command_id": "x",
                "side_id": "nato",
                "action_id": "wait:x",
                "observation_version": 0,
                "extra": 1,
            }
        )

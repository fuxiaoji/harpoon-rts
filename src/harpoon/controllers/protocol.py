from __future__ import annotations

from typing import Protocol

from harpoon.domain.models import ActionOption, Command, Observation


class Controller(Protocol):
    """Human, LLM, FSM and RL controllers share this boundary."""

    def choose(
        self, observation: Observation, actions: tuple[ActionOption, ...]
    ) -> tuple[Command, ...]: ...

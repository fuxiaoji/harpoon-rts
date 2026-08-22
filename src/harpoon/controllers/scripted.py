from __future__ import annotations

from harpoon.domain.models import ActionOption, Command, Observation


class WaitController:
    def choose(
        self, observation: Observation, actions: tuple[ActionOption, ...]
    ) -> tuple[Command, ...]:
        commands = []
        for option in actions:
            if option.available and option.action_type == "wait":
                commands.append(
                    Command(
                        command_id=f"wait-{observation.turn}-{option.unit_id}",
                        side_id=observation.side_id,
                        action_id=option.action_id,
                        observation_version=observation.version,
                    )
                )
        return tuple(commands)

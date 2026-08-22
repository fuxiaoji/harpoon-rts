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


class FireFirstController:
    """Deterministic tutorial opponent: fire once, then hold every other unit."""

    def choose(
        self, observation: Observation, actions: tuple[ActionOption, ...]
    ) -> tuple[Command, ...]:
        commands: list[Command] = []
        fired = False
        unit_ids = sorted(unit.unit_id for unit in observation.own_units)
        for unit_id in unit_ids:
            available = [
                option for option in actions if option.unit_id == unit_id and option.available
            ]
            fire = next(
                (option for option in available if option.action_type == "fire"), None
            )
            option = fire if fire is not None and not fired else next(
                option for option in available if option.action_type == "wait"
            )
            parameters = (
                {"target_id": option.candidate_targets[0]}
                if option.action_type == "fire"
                else {}
            )
            fired = fired or option.action_type == "fire"
            commands.append(
                Command(
                    command_id=f"tutorial-{observation.turn}-{unit_id}",
                    side_id=observation.side_id,
                    action_id=option.action_id,
                    observation_version=observation.version,
                    parameters=parameters,
                )
            )
        return tuple(commands)

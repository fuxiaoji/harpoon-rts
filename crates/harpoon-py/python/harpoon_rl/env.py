"""Gymnasium-style environment for the harpoon-core naval wargame engine.

No ``gymnasium`` dependency is required: this module implements the minimal
expected API (``reset`` / ``step`` / ``render`` / ``close``) over the compiled
``harpoon_py`` extension, with observations and actions expressed as plain
Python dictionaries / JSON.

Example
-------
>>> from harpoon_rl import HarpoonGymEnv
>>> env = HarpoonGymEnv("scenarios/gunnery_dual.json", side="Blue", seed=7)
>>> observation, info = env.reset()
>>> action = [{"cmd": "set_course", "unit": 1, "course_deg": 90}]
>>> obs, reward, terminated, truncated, info = env.step(action)
"""

from __future__ import annotations

import json
from collections.abc import Callable, Iterable, Mapping
from typing import Any

import harpoon_py

Observation = dict[str, Any]
Command = dict[str, Any]
RewardFn = Callable[[Observation, list[dict[str, Any]]], float]

# Events whose ``side`` field names the *victim*, used by the default reward.
_LOSS_EVENTS = frozenset({"sunk", "ditched", "air_shot_down", "base_destroyed"})
_UNIT_LOSS_REWARD = 1.0
_TERMINAL_WIN_REWARD = 10.0


class HarpoonGymEnv:
    """One side of one scenario, advanced one simulated second per ``step``.

    Parameters
    ----------
    scenario_json_path:
        Path to a scenario JSON file (see ``scenarios/``).
    side:
        ``"Blue"`` or ``"Red"`` (case-insensitive). Only commands for units on
        this side are accepted by the engine binding.
    seed:
        Optional seed overriding the seed embedded in the scenario file. The
        seed is also used by ``reset(None)``, so episodes are reproducible.
    render_mode:
        ``None`` (return text from ``render``), ``"human"`` (print it), or
        ``"ansi"`` (return it).
    reward_fn:
        Optional ``(observation, events) -> float`` override for the reward.
        It replaces the built-in shaping and terminal bonus entirely.

    Notes
    -----
    ``step`` returns the Gymnasium 5-tuple. ``terminated`` mirrors the engine's
    ``over`` flag (victory/annihilation/time limit); ``truncated`` is always
    ``False`` because the engine ends episodes itself. Events in the returned
    observation / ``info["events"]`` are incremental since the previous
    ``step`` or ``reset``; ``current_observation()`` returns the full event
    history visible to this side.

    The default reward is a small deterministic shaping heuristic: +1 for a
    visible loss of an opposing unit/base (``sunk``, ``ditched``,
    ``air_shot_down``, ``base_destroyed``), -1 for one of your own, and a
    one-time +10 / -10 bonus on win / loss at the terminal tick. Pass
    ``reward_fn`` for task-specific rewards.
    """

    metadata = {
        "render_modes": ["human", "ansi"],
        "render_fps": 1,
    }

    def __init__(
        self,
        scenario_json_path: str,
        side: str = "Blue",
        seed: int | None = None,
        render_mode: str | None = None,
        reward_fn: RewardFn | None = None,
    ) -> None:
        if not isinstance(side, str):
            raise TypeError("side must be 'Blue' or 'Red'")
        side_name = side.strip().capitalize()
        if side_name not in ("Blue", "Red"):
            raise ValueError(f"side must be 'Blue' or 'Red', got {side!r}")
        if render_mode not in (None, "human", "ansi"):
            raise ValueError(
                f"render_mode must be None, 'human' or 'ansi', got {render_mode!r}"
            )

        self.scenario_json_path = str(scenario_json_path)
        self.side = side_name
        self.render_mode = render_mode
        self._reward_fn = reward_fn
        self._seed = seed
        # The extension validates the scenario path/JSON and the seed type.
        self._env = harpoon_py.HarpoonEnv(self.scenario_json_path, self.side, seed)
        self._last_observation: Observation | None = None
        self._episode_ended = False

    # ------------------------------------------------------------------ API
    def reset(
        self, seed: int | None = None, options: Mapping[str, Any] | None = None
    ) -> tuple[Observation, dict[str, Any]]:
        """Start a new episode and return ``(observation, info)``.

        ``options`` is accepted for Gymnasium compatibility and ignored. With
        ``seed=None`` the seed used by the current episode is reused.
        """
        del options  # accepted for API compatibility; no reset options exist
        if seed is not None:
            self._seed = seed
        observation = json.loads(self._env.reset(seed))
        self._last_observation = observation
        self._episode_ended = False
        info = self._info(observation)
        return observation, info

    def step(
        self, action: Iterable[Command] | None
    ) -> tuple[Observation, float, bool, bool, dict[str, Any]]:
        """Apply a list of command dicts and advance one tick.

        Returns the Gymnasium tuple
        ``(observation, reward, terminated, truncated, info)``.
        """
        commands = self._normalize_action(action)
        payload = json.loads(self._env.step(json.dumps(commands)))
        observation: Observation = payload["observation"]
        self._last_observation = observation

        terminated = bool(payload.get("over", observation.get("over", False)))
        truncated = False
        events = list(observation.get("events", []))

        if self._reward_fn is not None:
            reward = float(self._reward_fn(observation, events))
        elif self._episode_ended:
            # Repeated steps after termination are no-ops in the engine; do not
            # pay the terminal bonus twice.
            reward = 0.0
        else:
            reward = self._default_reward(observation, events)
            if terminated:
                self._episode_ended = True

        info = self._info(observation)
        return observation, reward, terminated, truncated, info

    def render(self) -> str | None:
        """Render the last observation as plain text.

        Prints and returns ``None`` when ``render_mode == "human"``; otherwise
        returns the text (also for ``render_mode is None``).
        """
        if self._last_observation is None:
            raise RuntimeError("reset() must be called before render()")
        text = self._format(self._last_observation)
        if self.render_mode == "human":
            print(text)
            return None
        return text

    def close(self) -> None:
        """No-op; the extension owns no external resources."""

    # ------------------------------------------------------------- helpers
    @property
    def harpoon(self) -> harpoon_py.HarpoonEnv:
        """The underlying compiled extension object."""
        return self._env

    def current_observation(self) -> Observation:
        """Return the current observation with the full visible event history."""
        observation = json.loads(self._env.observation())
        self._last_observation = observation
        return observation

    def _normalize_action(self, action: Iterable[Command] | None) -> list[Command]:
        if action is None:
            return []
        if isinstance(action, Mapping):
            raise TypeError(
                "action must be a list of command dicts, not a single dict"
            )
        if isinstance(action, (str, bytes)):
            raise TypeError("action must be a list of command dicts, not a string")
        commands = list(action)
        for index, command in enumerate(commands):
            if not isinstance(command, Mapping):
                raise TypeError(f"action[{index}] must be a command dict")
        return [dict(command) for command in commands]

    def _default_reward(
        self, observation: Observation, events: list[dict[str, Any]]
    ) -> float:
        reward = 0.0
        own_side = observation.get("your_side", self.side)
        for event in events:
            if event.get("event") not in _LOSS_EVENTS:
                continue
            victim = event.get("side")
            if victim == own_side:
                reward -= _UNIT_LOSS_REWARD
            elif victim in ("Blue", "Red"):
                reward += _UNIT_LOSS_REWARD

        result = observation.get("result")
        if observation.get("over") and isinstance(result, Mapping):
            winner = result.get("winner")
            if winner == own_side:
                reward += _TERMINAL_WIN_REWARD
            elif winner in ("Blue", "Red"):
                reward -= _TERMINAL_WIN_REWARD
        return reward

    @staticmethod
    def _info(observation: Observation) -> dict[str, Any]:
        return {
            "events": list(observation.get("events", [])),
            "tick": observation.get("tick"),
            "result": observation.get("result"),
        }

    @staticmethod
    def _format(observation: Observation) -> str:
        lines = [
            (
                f"tick={observation['tick']} time={observation['time_s']:.0f}s "
                f"side={observation['your_side']} over={observation['over']}"
            ),
            (
                f"scenario: {observation['scenario_name']} "
                f"({observation['scenario_id']})"
            ),
        ]
        result = observation.get("result")
        if result:
            lines.append(
                "result: winner={winner} reason={reason} vp={vp_blue}-{vp_red}".format(
                    **result
                )
            )
        lines.append("units:")
        for unit in observation["units"]:
            pos = unit["pos"]
            lines.append(
                "  #{id} {name} [{kind}] pos=({x:.1f},{y:.1f}) "
                "course={course_deg:.1f} speed={speed_kts:.1f} hp={hp:.0f}/{hp_max:.0f}".format(
                    x=pos["x"], y=pos["y"], **unit
                )
            )
        lines.append("contacts:")
        if not observation["contacts"]:
            lines.append("  (none)")
        for contact in observation["contacts"]:
            estimate = contact["estimate"]
            lines.append(
                "  C{id} {category} quality={quality:.2f} "
                "estimate=({x:.1f},{y:.1f}) stale={stale}".format(
                    x=estimate["x"], y=estimate["y"], **contact
                )
            )
        return "\n".join(lines)

    def __repr__(self) -> str:
        return (
            f"HarpoonGymEnv(side={self.side!r}, scenario={self.scenario_json_path!r}, "
            f"seed={self._seed!r})"
        )

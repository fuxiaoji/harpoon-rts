from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path

from pydantic import BaseModel, ConfigDict

from harpoon.domain.models import (
    Command,
    Contact,
    Event,
    PlatformDefinition,
    UnitState,
    WeaponDefinition,
    WorldState,
)
from harpoon.engine.game import GameEngine


class ScenarioDocument(BaseModel):
    model_config = ConfigDict(extra="forbid")
    schema_version: str
    scenario_id: str
    seed: int
    start: datetime
    objectives: dict[str, list[str]]
    weapons: list[WeaponDefinition]
    platforms: list[PlatformDefinition]
    units: list[UnitState]
    contacts: dict[str, list[Contact]]


def load_scenario(path: Path) -> GameEngine:
    raw = json.loads(path.read_text(encoding="utf-8"))
    document = ScenarioDocument.model_validate(raw)
    platforms = {item.platform_id: item for item in document.platforms}
    weapons = {item.weapon_id: item for item in document.weapons}
    state = WorldState(
        scenario_id=document.scenario_id,
        seed=document.seed,
        timestamp=document.start,
        units={item.unit_id: item for item in document.units},
        contacts={side: tuple(items) for side, items in document.contacts.items()},
    )
    return GameEngine(
        state, platforms, weapons, {k: tuple(v) for k, v in document.objectives.items()}
    )


def default_scenario_path() -> Path:
    return Path(__file__).resolve().parents[3] / "data" / "scenarios" / "tattletail.json"


def load_saved_game(path: Path) -> GameEngine:
    raw = json.loads(path.read_text(encoding="utf-8"))
    snapshot = raw["snapshot"]
    state = WorldState.model_validate(snapshot["state"])
    platforms = {
        item.platform_id: item
        for item in (PlatformDefinition.model_validate(value) for value in raw["platforms"])
    }
    weapons = {
        item.weapon_id: item
        for item in (WeaponDefinition.model_validate(value) for value in raw["weapons"])
    }
    game = GameEngine(
        state,
        platforms,
        weapons,
        {key: tuple(value) for key, value in raw["objectives"].items()},
    )
    game.events = [Event.model_validate(value) for value in raw["events"]]
    game._commands = {
        command.command_id: command
        for command in (Command.model_validate(value) for value in raw.get("pending_commands", []))
    }
    game._event_sequence = len(game.events)
    if game.state.state_hash() != snapshot["state_hash"]:
        raise ValueError("保存文件状态哈希不匹配")
    return game

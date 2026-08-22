from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path

from pydantic import BaseModel, ConfigDict

from harpoon.domain.models import (
    Command,
    Contact,
    Event,
    GroundFacilityDefinition,
    GroundFacilityState,
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
    ground_facility_definitions: list[GroundFacilityDefinition] = []
    ground_facilities: list[GroundFacilityState] = []
    data_packages: list[str] = []


class ScenarioDataPackage(BaseModel):
    model_config = ConfigDict(extra="forbid")
    schema_version: str
    package_id: str
    weapons: list[WeaponDefinition] = []
    platforms: list[PlatformDefinition] = []
    ground_facility_definitions: list[GroundFacilityDefinition] = []


class ScenarioCatalogEntry(BaseModel):
    model_config = ConfigDict(extra="forbid")
    scenario_id: str
    title: str
    description: str
    path: str
    sides: list[str]
    status: str


def scenario_catalog() -> tuple[ScenarioCatalogEntry, ...]:
    root = Path(__file__).resolve().parents[3]
    raw = json.loads((root / "data" / "scenarios" / "catalog.json").read_text(encoding="utf-8"))
    return tuple(ScenarioCatalogEntry.model_validate(item) for item in raw["scenarios"])


def scenario_path(scenario_id: str) -> Path:
    root = Path(__file__).resolve().parents[3]
    entry = next((item for item in scenario_catalog() if item.scenario_id == scenario_id), None)
    if entry is None:
        raise KeyError(f"未知想定：{scenario_id}")
    candidate = (root / entry.path).resolve()
    scenario_root = (root / "data" / "scenarios").resolve()
    if scenario_root not in candidate.parents:
        raise ValueError("想定路径越界")
    return candidate


def load_scenario(path: Path) -> GameEngine:
    raw = json.loads(path.read_text(encoding="utf-8"))
    document = ScenarioDocument.model_validate(raw)
    packages: list[ScenarioDataPackage] = []
    root = Path(__file__).resolve().parents[3]
    data_root = (root / "data").resolve()
    for relative in document.data_packages:
        package_path = (root / relative).resolve()
        if data_root not in package_path.parents:
            raise ValueError("数据包路径越界")
        packages.append(
            ScenarioDataPackage.model_validate(
                json.loads(package_path.read_text(encoding="utf-8"))
            )
        )
    platforms = {
        item.platform_id: item
        for item in document.platforms
        + [entry for package in packages for entry in package.platforms]
    }
    weapons = {
        item.weapon_id: item
        for item in document.weapons
        + [entry for package in packages for entry in package.weapons]
    }
    state = WorldState(
        scenario_id=document.scenario_id,
        seed=document.seed,
        timestamp=document.start,
        units={item.unit_id: item for item in document.units},
        ground_facilities={item.facility_id: item for item in document.ground_facilities},
        contacts={side: tuple(items) for side, items in document.contacts.items()},
    )
    facilities = {
        item.facility_id: item
        for item in document.ground_facility_definitions
        + [
            entry
            for package in packages
            for entry in package.ground_facility_definitions
        ]
    }
    return GameEngine(
        state,
        platforms,
        weapons,
        {k: tuple(v) for k, v in document.objectives.items()},
        facilities,
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
        {
            item.facility_id: item
            for item in (
                GroundFacilityDefinition.model_validate(value)
                for value in raw.get("ground_facility_definitions", [])
            )
        },
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

from __future__ import annotations

import json
from datetime import datetime
from enum import StrEnum
from hashlib import sha256
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field


class FrozenModel(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")


class Phase(StrEnum):
    PLOTTING = "plotting"
    MOVEMENT = "movement"
    PLANNED_FIRE = "planned_fire"
    DETECTION = "detection"
    REACTION_FIRE = "reaction_fire"
    RESOLUTION = "resolution"


PHASE_ORDER = tuple(Phase)


class Visibility(StrEnum):
    PUBLIC = "public"
    SIDE = "side"
    REFEREE = "referee"


class Readiness(StrEnum):
    CONDITION_III = "condition_iii"
    TRANSITIONING = "transitioning"
    CONDITION_I = "condition_i"


class Position(FrozenModel):
    x_nm: float
    y_nm: float


class WeaponDefinition(FrozenModel):
    weapon_id: str
    name: str
    minimum_range_nm: float
    maximum_range_nm: float
    speed_knots: float
    base_damage_dp: int
    damage_die: int = 6
    damage_divisor: int = 1
    seeker_generation: int | None = None
    signature: str | None = None
    penetration: int | None = None
    missile_impact_criticals: bool = False
    attack_domain: Literal["surface", "land"] = "surface"
    land_attack_type: Literal["ballistic", "land_attack_cruise"] | None = None
    guidance: str | None = None
    rule_id: str
    source: str
    verified: bool = True


class PlatformDefinition(FrozenModel):
    platform_id: str
    name: str
    side_id: str
    maximum_dp: int
    maximum_speed_knots: float
    size_class: str = ""
    target_signature: str = ""
    ecm_generation: int | None = None
    countermeasure: Literal["base", "jamming", "decoy", "jamming_and_decoy"] = "base"
    damage_modifier_percent: int = 0
    damage_breakpoints_dp: tuple[int, ...] = ()
    speed_breakpoints_knots: tuple[float, ...] = ()
    fire_flooding_thresholds_percent: tuple[int, ...] = ()
    weapons: tuple[str, ...] = ()
    sensors: tuple[str, ...] = ()
    source: str
    verified: bool = True


class UnitState(FrozenModel):
    unit_id: str
    platform_id: str
    side_id: str
    position: Position
    course_deg: float
    speed_knots: float
    damage_dp: int = 0
    readiness: Readiness = Readiness.CONDITION_III
    readiness_turns_remaining: int = 0
    ammunition: dict[str, int] = Field(default_factory=dict)
    active_sensors: tuple[str, ...] = ()
    crippled: bool = False
    sunk: bool = False
    critical_hits: tuple[str, ...] = ()
    fire_critical_count: int = 0


class GroundFacilityDefinition(FrozenModel):
    facility_id: str
    name: str
    side_id: str
    facility_type: str
    size_class: str
    maximum_dp: int
    armor: int
    latitude: float
    longitude: float
    runway_length_m: int | None = None
    source: str
    provenance: Literal["official_database", "public_source", "scenario_assumption"]
    verified: bool = True


class GroundFacilityState(FrozenModel):
    facility_id: str
    damage_dp: int = 0
    runway_cuts: int = 0
    operational: bool = True
    destroyed: bool = False


class Contact(FrozenModel):
    contact_id: str
    observer_side_id: str
    target_unit_id: str | None = None
    quality: Literal["bearing", "detected", "classified", "identified", "good_fcs"]
    last_known_position: Position | None = None
    updated_turn: int = 0


class InFlightWeapon(FrozenModel):
    entity_id: str
    weapon_id: str
    shooter_id: str
    target_id: str
    owner_side_id: str
    turns_to_arrival: int
    launch_range_nm: float
    awaiting_adjudication: bool = False
    target_kind: Literal["unit", "ground_facility"] = "unit"


class WorldState(FrozenModel):
    scenario_id: str
    seed: int
    timestamp: datetime
    turn: int = 0
    phase: Phase = Phase.PLOTTING
    version: int = 0
    random_draw_index: int = 0
    units: dict[str, UnitState]
    ground_facilities: dict[str, GroundFacilityState] = Field(default_factory=dict)
    contacts: dict[str, tuple[Contact, ...]] = Field(default_factory=dict)
    in_flight_weapons: tuple[InFlightWeapon, ...] = ()
    locked_sides: frozenset[str] = frozenset()
    winner: str | None = None

    def state_hash(self) -> str:
        payload = json.dumps(self.model_dump(mode="json"), sort_keys=True, separators=(",", ":"))
        return sha256(payload.encode("utf-8")).hexdigest()


class RuleExplanation(FrozenModel):
    rule_id: str
    title_zh: str
    title_en: str
    explanation: str
    source_document: str
    source_section: str
    verification: Literal["verified", "partial", "blocked"]


class ActionOption(FrozenModel):
    action_id: str
    action_type: str
    unit_id: str | None = None
    parameter_schema: dict[str, Any] = Field(default_factory=dict)
    candidate_targets: tuple[str, ...] = ()
    available: bool
    unavailable_reason: str | None = None
    rule: RuleExplanation


class Command(FrozenModel):
    command_id: str
    side_id: str
    action_id: str
    observation_version: int
    parameters: dict[str, Any] = Field(default_factory=dict)


class ValidationResult(FrozenModel):
    accepted: bool
    errors: tuple[str, ...] = ()
    rule_ids: tuple[str, ...] = ()


class Event(FrozenModel):
    event_id: str
    turn: int
    phase: Phase
    event_type: str
    actors: tuple[str, ...] = ()
    visibility: Visibility = Visibility.PUBLIC
    side_id: str | None = None
    rule_ids: tuple[str, ...] = ()
    payload: dict[str, Any] = Field(default_factory=dict)
    random_record: dict[str, Any] | None = None


class Observation(FrozenModel):
    side_id: str
    scenario_id: str
    timestamp: datetime
    turn: int
    phase: Phase
    version: int
    own_units: tuple[UnitState, ...]
    contacts: tuple[Contact, ...]
    visible_events: tuple[Event, ...]
    objectives: tuple[str, ...]


class PhaseStatus(FrozenModel):
    phase: Phase
    locked_sides: frozenset[str]
    all_locked: bool


class GameSnapshot(FrozenModel):
    schema_version: str = "0.1.0"
    state: WorldState
    event_cursor: int
    state_hash: str

from __future__ import annotations

from datetime import datetime
from enum import StrEnum
from typing import Literal

from pydantic import Field

from harpoon.domain.models import FrozenModel


class PlanStatus(StrEnum):
    DRAFT = "draft"
    APPROVED = "approved"
    INVALIDATED = "invalidated"
    COMPLETE = "complete"


class CommanderIntent(FrozenModel):
    intent_id: str
    side_id: str
    objective: str = Field(min_length=1, max_length=2000)
    priorities: tuple[str, ...] = ()
    constraints: tuple[str, ...] = ()
    deadline_turn: int | None = None
    acceptable_losses: str = "avoid_unnecessary_losses"
    authorization: frozenset[Literal["movement", "sensors", "routine_defense", "fire", "withdraw"]] = (
        frozenset({"movement", "sensors", "routine_defense"})
    )


class PlanStep(FrozenModel):
    step_id: str
    objective: str
    action_types: tuple[str, ...]
    unit_ids: tuple[str, ...]
    trigger: str
    requires_confirmation: bool = False


class OperationalPlan(FrozenModel):
    plan_id: str
    intent_id: str
    side_id: str
    observation_version: int
    created_at: datetime
    valid_until_turn: int
    status: PlanStatus = PlanStatus.DRAFT
    summary: str
    assumptions: tuple[str, ...] = ()
    risks: tuple[str, ...] = ()
    steps: tuple[PlanStep, ...]


class ActionProposal(FrozenModel):
    proposal_id: str
    plan_id: str
    side_id: str
    action_id: str
    observation_version: int
    parameters: dict[str, str | int | float]
    plan_step_id: str
    rationale: str
    requires_confirmation: bool = False


class ApprovalRequest(FrozenModel):
    approval_id: str
    plan_id: str
    proposal: ActionProposal
    reason: str
    status: Literal["pending", "approved", "rejected"] = "pending"


class IntelligenceItem(FrozenModel):
    contact_id: str
    assessment: Literal["confirmed", "assessed", "stale", "lost"]
    confidence: float = Field(ge=0, le=1)
    statement: str
    updated_turn: int


class StaffBriefing(FrozenModel):
    side_id: str
    scenario_id: str
    timestamp: datetime
    observation_version: int
    mission_progress: str
    confirmed_intelligence: tuple[IntelligenceItem, ...]
    assessed_intelligence: tuple[IntelligenceItem, ...]
    own_force_status: tuple[str, ...]
    recent_events: tuple[str, ...]
    threats: tuple[str, ...]
    recommendations: tuple[str, ...]
    decisions_required: tuple[str, ...]


class ModelAuditRecord(FrozenModel):
    request_id: str
    provider: str
    operation: Literal["plan", "execute", "brief"]
    side_id: str
    observation_version: int
    input_fingerprint: str
    output_type: str
    elapsed_ms: int
    token_usage: dict[str, int] = Field(default_factory=dict)
    error: str | None = None

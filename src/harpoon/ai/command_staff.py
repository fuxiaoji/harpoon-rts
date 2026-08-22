from __future__ import annotations

from collections.abc import Iterable
from hashlib import sha256
from typing import Literal, Protocol

from harpoon.ai.contracts import (
    ActionProposal,
    ApprovalRequest,
    CommanderIntent,
    IntelligenceItem,
    OperationalPlan,
    PlanStatus,
    PlanStep,
    StaffBriefing,
)
from harpoon.domain.models import ActionOption, Command, Observation, ValidationResult


class StaffProvider(Protocol):
    def create_plan(
        self, intent: CommanderIntent, observation: Observation, actions: tuple[ActionOption, ...]
    ) -> OperationalPlan: ...

    def propose_actions(
        self,
        plan: OperationalPlan,
        observation: Observation,
        actions: tuple[ActionOption, ...],
    ) -> tuple[ActionProposal, ...]: ...


class DeterministicStaffProvider:
    """Offline baseline and safe fallback; consumes only masked public contracts."""

    def create_plan(
        self, intent: CommanderIntent, observation: Observation, actions: tuple[ActionOption, ...]
    ) -> OperationalPlan:
        unit_ids = tuple(sorted(unit.unit_id for unit in observation.own_units))
        types = tuple(sorted({item.action_type for item in actions if item.available}))
        return OperationalPlan(
            plan_id=f"plan-{intent.intent_id}",
            intent_id=intent.intent_id,
            side_id=intent.side_id,
            observation_version=observation.version,
            created_at=observation.timestamp,
            valid_until_turn=intent.deadline_turn or observation.turn + 4,
            summary=f"执行指挥意图：{intent.objective}",
            assumptions=("只依据本方 Observation 与当前合法行动",),
            risks=("接触信息可能过时；开火必须再次确认",),
            steps=(
                PlanStep(
                    step_id="maintain-force",
                    objective="保持编队行动并更新态势",
                    action_types=types,
                    unit_ids=unit_ids,
                    trigger="当前决策点",
                ),
                PlanStep(
                    step_id="engage-confirmed",
                    objective="对已确认目标实施获授权打击",
                    action_types=("fire", "air_attack", "land_strike"),
                    unit_ids=unit_ids,
                    trigger="存在合法攻击行动",
                    requires_confirmation=True,
                ),
            ),
        )

    def propose_actions(
        self,
        plan: OperationalPlan,
        observation: Observation,
        actions: tuple[ActionOption, ...],
    ) -> tuple[ActionProposal, ...]:
        if plan.status != PlanStatus.APPROVED or observation.turn > plan.valid_until_turn:
            return ()
        proposals: list[ActionProposal] = []
        by_unit: dict[str, list[ActionOption]] = {}
        for option in actions:
            if option.available and option.unit_id:
                by_unit.setdefault(option.unit_id, []).append(option)
        for unit_id, options in sorted(by_unit.items()):
            fire = next((item for item in options if item.action_type == "fire"), None)
            selected = fire or next((item for item in options if item.action_type == "wait"), None)
            if selected is None:
                selected = options[0]
            parameters: dict[str, str | int | float] = {}
            if selected.action_type == "fire" and selected.candidate_targets:
                parameters["target_id"] = selected.candidate_targets[0]
            proposals.append(
                ActionProposal(
                    proposal_id=f"proposal-{plan.plan_id}-{observation.version}-{unit_id}",
                    plan_id=plan.plan_id,
                    side_id=plan.side_id,
                    action_id=selected.action_id,
                    observation_version=observation.version,
                    parameters=parameters,
                    plan_step_id="engage-confirmed" if fire else "maintain-force",
                    rationale="选择当前计划允许的合法攻击" if fire else "无获授权攻击时维持行动",
                    requires_confirmation=selected.action_type in {"fire", "air_attack", "land_strike"},
                )
            )
        return tuple(proposals)


class CommandStaff:
    def __init__(self, provider: StaffProvider | None = None) -> None:
        self.provider = provider or DeterministicStaffProvider()
        self.intents: dict[str, CommanderIntent] = {}
        self.plans: dict[str, OperationalPlan] = {}
        self.approvals: dict[str, ApprovalRequest] = {}

    def issue_intent(
        self, intent: CommanderIntent, observation: Observation, actions: tuple[ActionOption, ...]
    ) -> OperationalPlan:
        if intent.side_id != observation.side_id:
            raise ValueError("指挥意图阵营与 Observation 不一致")
        plan = self.provider.create_plan(intent, observation, actions)
        self.intents[intent.intent_id] = intent
        self.plans[plan.plan_id] = plan
        return plan

    def approve_plan(self, plan_id: str, side_id: str) -> OperationalPlan:
        plan = self.plans[plan_id]
        if plan.side_id != side_id:
            raise ValueError("不能批准其他阵营的计划")
        approved = plan.model_copy(update={"status": PlanStatus.APPROVED})
        self.plans[plan_id] = approved
        return approved

    def propose(
        self, plan_id: str, observation: Observation, actions: tuple[ActionOption, ...]
    ) -> tuple[ActionProposal, ...]:
        plan = self.plans[plan_id]
        if plan.side_id != observation.side_id:
            raise ValueError("计划无权读取该阵营 Observation")
        return self.provider.propose_actions(plan, observation, actions)

    def commands_or_approvals(
        self, proposals: Iterable[ActionProposal], approved_ids: frozenset[str] = frozenset()
    ) -> tuple[tuple[Command, ...], tuple[ApprovalRequest, ...]]:
        commands: list[Command] = []
        pending: list[ApprovalRequest] = []
        for proposal in proposals:
            approval_id = f"approval-{proposal.proposal_id}"
            if proposal.requires_confirmation and approval_id not in approved_ids:
                request = ApprovalRequest(
                    approval_id=approval_id,
                    plan_id=proposal.plan_id,
                    proposal=proposal,
                    reason="重大开火、对陆攻击或撤离需要指挥官确认",
                )
                self.approvals[approval_id] = request
                pending.append(request)
                continue
            commands.append(
                Command(
                    command_id=f"staff-{proposal.proposal_id}",
                    side_id=proposal.side_id,
                    action_id=proposal.action_id,
                    observation_version=proposal.observation_version,
                    parameters=proposal.parameters,
                )
            )
        return tuple(commands), tuple(pending)

    def briefing(self, observation: Observation, plan_id: str | None = None) -> StaffBriefing:
        confirmed: list[IntelligenceItem] = []
        assessed: list[IntelligenceItem] = []
        quality_confidence = {
            "good_fcs": 1.0,
            "identified": 0.9,
            "classified": 0.75,
            "detected": 0.55,
            "bearing": 0.35,
        }
        for contact in observation.contacts:
            confidence = quality_confidence[contact.quality]
            assessment: Literal["confirmed", "assessed"] = (
                "confirmed" if contact.quality in {"good_fcs", "identified"} else "assessed"
            )
            item = IntelligenceItem(
                contact_id=contact.contact_id,
                assessment=assessment,
                confidence=confidence,
                statement=f"接触 {contact.contact_id}，质量 {contact.quality}",
                updated_turn=contact.updated_turn,
            )
            (confirmed if assessment == "confirmed" else assessed).append(item)
        own = tuple(
            f"{unit.unit_id}: DP损伤{unit.damage_dp}，航速{unit.speed_knots:g}节，弹药{sum(unit.ammunition.values())}"
            for unit in observation.own_units
        )
        recent = tuple(event.event_type for event in observation.visible_events[-6:])
        decisions = tuple(
            request.reason
            for request in self.approvals.values()
            if request.proposal.side_id == observation.side_id and request.status == "pending"
        )
        plan = self.plans.get(plan_id) if plan_id else None
        return StaffBriefing(
            side_id=observation.side_id,
            scenario_id=observation.scenario_id,
            timestamp=observation.timestamp,
            observation_version=observation.version,
            mission_progress=plan.summary if plan else "尚未批准作战计划",
            confirmed_intelligence=tuple(confirmed),
            assessed_intelligence=tuple(assessed),
            own_force_status=own,
            recent_events=recent,
            threats=("敌方位置与能力仅按当前接触质量判断",),
            recommendations=("保持情报更新；重大攻击前核对目标与交战授权",),
            decisions_required=decisions,
        )


def masked_context_fingerprint(observation: Observation) -> str:
    """Fingerprint only the side-scoped contract; never accepts WorldState."""
    payload = observation.model_dump_json(exclude={"visible_events": {"__all__": {"payload"}}})
    return sha256(payload.encode()).hexdigest()


def validate_provider_result(
    proposal: ActionProposal, actions: tuple[ActionOption, ...]
) -> ValidationResult:
    option = next((item for item in actions if item.action_id == proposal.action_id), None)
    if option is None or not option.available:
        return ValidationResult(accepted=False, errors=("LLM 提议不在当前合法行动集合",))
    if proposal.observation_version < 0:
        return ValidationResult(accepted=False, errors=("无效 observation version",))
    return ValidationResult(accepted=True, rule_ids=(option.rule.rule_id,))

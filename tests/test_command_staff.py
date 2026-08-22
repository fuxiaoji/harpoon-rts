from harpoon.adapters import load_scenario
from harpoon.ai import CommanderIntent, CommandStaff, DeepSeekChatProvider
from harpoon.ai.command_staff import masked_context_fingerprint, validate_provider_result
from harpoon.ai.contracts import PlanStatus
from harpoon.domain.models import Observation


def game():  # type: ignore[no-untyped-def]
    from harpoon.adapters import default_scenario_path

    return load_scenario(default_scenario_path())


def test_staff_sees_only_observation_and_requires_plan_approval() -> None:
    engine = game()
    observation = engine.observe("nato")
    assert isinstance(observation, Observation)
    staff = CommandStaff()
    intent = CommanderIntent(
        intent_id="intent-1",
        side_id="nato",
        objective="保持编队并在遭到攻击后反击",
    )
    plan = staff.issue_intent(intent, observation, engine.list_actions("nato"))
    assert plan.status == PlanStatus.DRAFT
    assert staff.propose(plan.plan_id, observation, engine.list_actions("nato")) == ()
    approved = staff.approve_plan(plan.plan_id, "nato")
    proposals = staff.propose(approved.plan_id, observation, engine.list_actions("nato"))
    assert proposals
    assert all(item.side_id == "nato" for item in proposals)
    assert masked_context_fingerprint(observation) == masked_context_fingerprint(observation)


def test_staff_cannot_select_action_outside_legal_mask() -> None:
    engine = game()
    observation = engine.observe("nato")
    staff = CommandStaff()
    plan = staff.issue_intent(
        CommanderIntent(intent_id="intent-2", side_id="nato", objective="等待"),
        observation,
        engine.list_actions("nato"),
    )
    staff.approve_plan(plan.plan_id, "nato")
    proposal = staff.propose(plan.plan_id, observation, engine.list_actions("nato"))[0]
    result = validate_provider_result(proposal, engine.list_actions("soviet"))
    assert not result.accepted


def test_fire_proposal_is_held_for_human_confirmation() -> None:
    engine = game()
    observation = engine.observe("soviet")
    staff = CommandStaff()
    plan = staff.issue_intent(
        CommanderIntent(intent_id="intent-3", side_id="soviet", objective="攻击补给编队"),
        observation,
        engine.list_actions("soviet"),
    )
    staff.approve_plan(plan.plan_id, "soviet")
    proposals = staff.propose(plan.plan_id, observation, engine.list_actions("soviet"))
    commands, approvals = staff.commands_or_approvals(proposals)
    assert approvals
    assert all(command.action_id.startswith("wait:") for command in commands)
    assert any(item.proposal.action_id.startswith("fire:") for item in approvals)


def test_briefing_never_upgrades_contact_quality() -> None:
    engine = game()
    briefing = CommandStaff().briefing(engine.observe("nato"))
    assert briefing.confirmed_intelligence
    assert all(item.confidence <= 1 for item in briefing.confirmed_intelligence)
    assert "smyshlennyy-mod-kashin" not in briefing.model_dump_json()


def test_deepseek_executor_disables_thinking_and_rejects_illegal_action() -> None:
    engine = game()
    observation = engine.observe("nato")
    seen: list[dict[str, object]] = []

    def transport(
        url: str,
        payload: dict[str, object],
        headers: dict[str, str],
        timeout: float,
    ) -> dict[str, object]:
        seen.append(payload)
        return {
            "choices": [
                {
                    "message": {
                        "content": (
                            '{"proposals":[{"proposal_id":"p","plan_id":"plan-intent-4",'
                            '"side_id":"nato","action_id":"cheat:hidden-state",'
                            '"observation_version":0,"parameters":{},"plan_step_id":"x",'
                            '"rationale":"x","requires_confirmation":false}]}'
                        )
                    }
                }
            ]
        }

    provider = DeepSeekChatProvider("runtime-only", transport=transport)
    staff = CommandStaff()
    plan = staff.issue_intent(
        CommanderIntent(intent_id="intent-4", side_id="nato", objective="保持"),
        observation,
        engine.list_actions("nato"),
    )
    plan = staff.approve_plan(plan.plan_id, "nato")
    import pytest

    with pytest.raises(ValueError, match="合法行动掩码"):
        provider.propose_actions(plan, observation, engine.list_actions("nato"))
    assert seen[0]["thinking"] == {"type": "disabled"}

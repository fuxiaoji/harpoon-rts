from __future__ import annotations

import json
from typing import Any

from harpoon.ai.contracts import ActionProposal, CommanderIntent, OperationalPlan
from harpoon.ai.openai_provider import ProposalBatch, Transport, _urlopen_transport
from harpoon.domain.models import ActionOption, Observation


def _chat_content(response: dict[str, Any]) -> str:
    try:
        content = response["choices"][0]["message"]["content"]
    except (KeyError, IndexError, TypeError) as exc:
        raise ValueError("DeepSeek 未返回 message content") from exc
    if not isinstance(content, str) or not content.strip():
        raise ValueError("DeepSeek 返回了空内容")
    return content


class DeepSeekChatProvider:
    """DeepSeek adapter with separate planning and low-latency execution modes."""

    def __init__(
        self,
        api_key: str,
        model: str = "deepseek-v4-flash",
        base_url: str = "https://api.deepseek.com",
        transport: Transport = _urlopen_transport,
    ) -> None:
        if not api_key:
            raise ValueError("DeepSeek API key 不能为空")
        self.api_key = api_key
        self.model = model
        self.base_url = base_url.rstrip("/")
        self.transport = transport

    def create_plan(
        self, intent: CommanderIntent, observation: Observation, actions: tuple[ActionOption, ...]
    ) -> OperationalPlan:
        content = self._call(
            "strategic_planner",
            OperationalPlan.model_json_schema(),
            {
                "intent": intent.model_dump(mode="json"),
                "observation": observation.model_dump(mode="json"),
                "legal_actions": [item.model_dump(mode="json") for item in actions],
            },
            thinking=True,
            reasoning_effort="medium",
            max_tokens=5000,
            timeout=45,
        )
        plan = OperationalPlan.model_validate_json(content)
        if plan.side_id != observation.side_id or plan.observation_version != observation.version:
            raise ValueError("DeepSeek 计划与当前阵营或情报版本不一致")
        return plan

    def propose_actions(
        self,
        plan: OperationalPlan,
        observation: Observation,
        actions: tuple[ActionOption, ...],
    ) -> tuple[ActionProposal, ...]:
        available = tuple(item for item in actions if item.available)
        content = self._call(
            "tactical_executor",
            ProposalBatch.model_json_schema(),
            {
                "approved_plan": plan.model_dump(mode="json"),
                "observation": observation.model_dump(mode="json"),
                "legal_actions": [item.model_dump(mode="json") for item in available],
            },
            thinking=False,
            reasoning_effort="low",
            max_tokens=1800,
            timeout=12,
        )
        batch = ProposalBatch.model_validate_json(content)
        allowed = {item.action_id for item in available}
        for proposal in batch.proposals:
            if proposal.side_id != observation.side_id:
                raise ValueError("DeepSeek 动作阵营越权")
            if proposal.observation_version != observation.version:
                raise ValueError("DeepSeek 动作使用过期 Observation")
            if proposal.action_id not in allowed:
                raise ValueError("DeepSeek 动作不在合法行动掩码中")
        return batch.proposals

    def _call(
        self,
        operation: str,
        schema: dict[str, Any],
        data: dict[str, Any],
        thinking: bool,
        reasoning_effort: str,
        max_tokens: int,
        timeout: float,
    ) -> str:
        prompt = {
            "instruction": (
                "Return JSON only and follow json_schema exactly. Never infer hidden state. "
                "The tactical executor may only copy action_id from legal_actions. "
                "Attacks and withdrawal require confirmation."
            ),
            "operation": operation,
            "json_schema": schema,
            "data": data,
        }
        payload = {
            "model": self.model,
            "messages": [
                {"role": "system", "content": "You are a naval wargame command staff. Output JSON only."},
                {"role": "user", "content": json.dumps(prompt, ensure_ascii=False)},
            ],
            "response_format": {"type": "json_object"},
            "thinking": {"type": "enabled" if thinking else "disabled"},
            "reasoning_effort": reasoning_effort,
            "max_tokens": max_tokens,
            "stream": False,
        }
        response = self.transport(
            f"{self.base_url}/chat/completions",
            payload,
            {"Authorization": f"Bearer {self.api_key}", "Content-Type": "application/json"},
            timeout,
        )
        return _chat_content(response)

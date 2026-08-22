from __future__ import annotations

import json
from collections.abc import Callable
from typing import Any
from urllib.request import Request, urlopen

from pydantic import BaseModel, ConfigDict

from harpoon.ai.contracts import ActionProposal, CommanderIntent, OperationalPlan
from harpoon.domain.models import ActionOption, Observation


class ProposalBatch(BaseModel):
    model_config = ConfigDict(extra="forbid")
    proposals: tuple[ActionProposal, ...]


Transport = Callable[[str, dict[str, Any], dict[str, str], float], dict[str, Any]]


def _urlopen_transport(
    url: str, payload: dict[str, Any], headers: dict[str, str], timeout: float
) -> dict[str, Any]:
    request = Request(
        url,
        data=json.dumps(payload, ensure_ascii=False).encode(),
        headers=headers,
        method="POST",
    )
    with urlopen(request, timeout=timeout) as response:
        value: dict[str, Any] = json.loads(response.read())
        return value


def _output_text(response: dict[str, Any]) -> str:
    if isinstance(response.get("output_text"), str):
        return str(response["output_text"])
    for item in response.get("output", []):
        for content in item.get("content", []):
            if content.get("type") == "output_text" and isinstance(content.get("text"), str):
                return str(content["text"])
    raise ValueError("Responses API 未返回结构化 output_text")


class OpenAIResponsesProvider:
    """Replaceable Responses API adapter; the API key is injected, never persisted."""

    def __init__(
        self,
        api_key: str,
        model: str,
        base_url: str = "https://api.openai.com/v1",
        transport: Transport = _urlopen_transport,
    ) -> None:
        if not api_key:
            raise ValueError("OpenAI API key 不能为空")
        self.api_key = api_key
        self.model = model
        self.base_url = base_url.rstrip("/")
        self.transport = transport

    def create_plan(
        self, intent: CommanderIntent, observation: Observation, actions: tuple[ActionOption, ...]
    ) -> OperationalPlan:
        result = self._structured(
            "operational_plan",
            OperationalPlan.model_json_schema(),
            {
                "role": "strategic_planner",
                "intent": intent.model_dump(mode="json"),
                "observation": observation.model_dump(mode="json"),
                "legal_actions": [item.model_dump(mode="json") for item in actions],
                "constraints": [
                    "Never infer hidden truth.",
                    "Return a draft plan; attacks and withdrawal require confirmation.",
                ],
            },
            reasoning_effort="medium",
            timeout=45,
        )
        plan = OperationalPlan.model_validate_json(result)
        if plan.side_id != observation.side_id or plan.observation_version != observation.version:
            raise ValueError("模型计划与当前情报版本不一致")
        return plan

    def propose_actions(
        self,
        plan: OperationalPlan,
        observation: Observation,
        actions: tuple[ActionOption, ...],
    ) -> tuple[ActionProposal, ...]:
        result = self._structured(
            "action_proposals",
            ProposalBatch.model_json_schema(),
            {
                "role": "tactical_executor",
                "approved_plan": plan.model_dump(mode="json"),
                "observation": observation.model_dump(mode="json"),
                "legal_actions": [
                    item.model_dump(mode="json") for item in actions if item.available
                ],
                "constraints": [
                    "Choose only action_id values present in legal_actions.",
                    "Do not revise strategy.",
                    "Mark fire, air_attack, land_strike and withdrawal for confirmation.",
                ],
            },
            reasoning_effort="low",
            timeout=12,
        )
        batch = ProposalBatch.model_validate_json(result)
        allowed = {item.action_id for item in actions if item.available}
        if any(item.action_id not in allowed for item in batch.proposals):
            raise ValueError("模型输出包含非法 action_id")
        if any(item.observation_version != observation.version for item in batch.proposals):
            raise ValueError("模型动作使用了过期 Observation")
        return batch.proposals

    def _structured(
        self,
        name: str,
        schema: dict[str, Any],
        content: dict[str, Any],
        reasoning_effort: str,
        timeout: float,
    ) -> str:
        payload = {
            "model": self.model,
            "input": json.dumps(content, ensure_ascii=False, separators=(",", ":")),
            "reasoning": {"effort": reasoning_effort},
            "text": {
                "verbosity": "low",
                "format": {"type": "json_schema", "name": name, "strict": True, "schema": schema},
            },
            "store": False,
        }
        response = self.transport(
            f"{self.base_url}/responses",
            payload,
            {"Authorization": f"Bearer {self.api_key}", "Content-Type": "application/json"},
            timeout,
        )
        return _output_text(response)

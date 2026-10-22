"""One observation -> one real model response, via authenticated Codex exec.

No credentials are read/copied by this module. No server is accessed. Tools are
disabled where configurable and any tool event invalidates the model sample.
"""
from __future__ import annotations

import hashlib
import json
import math
import os
import shutil
import subprocess
import time
import uuid
from types import SimpleNamespace
from pathlib import Path


MODEL = "gpt-6.1-sol"
SCHEMA = {
    "type": "object",
    "properties": {
        "action": {"type": "string", "enum": ["hold", "goto"]},
        "task_revision": {"type": "integer"},
        "unit": {"type": "integer"},
        "waypoint": {"type": "object", "properties": {
            "x": {"type": "number"}, "y": {"type": "number"}},
            "required": ["x", "y"], "additionalProperties": False},
        "brief": {"type": "string"},
    },
    "required": ["action", "task_revision", "unit", "waypoint", "brief"],
    "additionalProperties": False,
}
INSTRUCTIONS = """You control a vessel in a simulated navigation experiment.
Use only the supplied observation and public task. Do not use tools, files,
internet, code execution or external information. Return only the requested JSON.
The public task is to visit its goal within radius_nm by deadline_tick. It may
replace an earlier task. The initial standing order already follows revision 0.
If the standing_revision matches public_task.revision, normally hold the current
order. Otherwise issue goto for the own vessel to the currently public goal.
Use the own vessel id from observation.units. For hold, still fill unit and
waypoint with that vessel and the current task goal. Give a brief explanation
in at most one short sentence; do not provide a reasoning trace.
"""


def clean_events(stdout: str) -> tuple[list[dict], dict | None, str | None, list[str]]:
    events, usage, final, violations = [], None, None, []
    for line in stdout.splitlines():
        if not line.strip():
            continue
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            events.append({"type": "client_parse_error", "partial_line_omitted": True})
            continue
        body = item.get("item", {})
        kind = body.get("type")
        if item.get("type", "").startswith("item.") and kind not in ("reasoning", "agent_message", "error"):
            violations.append(str(kind))
        if kind == "reasoning":
            item = {"type": item["type"], "item": {"type": "reasoning", "text_omitted": True}}
        if kind == "agent_message" and body.get("text"):
            final = body["text"]
        if item.get("type") == "turn.completed":
            usage = item.get("usage")
        events.append(item)
    return events, usage, final, violations


def validate_action(answer: dict, model_input: dict) -> None:
    if set(answer) != set(SCHEMA["required"]) or answer["action"] not in ("hold", "goto"):
        raise ValueError("model action schema mismatch")
    task = model_input["public_task"]
    if answer["task_revision"] != task["revision"]:
        raise ValueError("model replied for a different task revision")
    if answer["unit"] not in {u["id"] for u in model_input["observation"]["units"]}:
        raise ValueError("model selected a non-own unit")
    if set(answer["waypoint"]) != {"x", "y"}:
        raise ValueError("invalid waypoint")
    if any(type(answer["waypoint"][k]) not in (int, float) or not math.isfinite(answer["waypoint"][k]) for k in ("x", "y")):
        raise ValueError("non-finite or non-numeric waypoint")
    # V1 action space allows only hold or navigation to the public task goal.
    if any(abs(float(answer["waypoint"][k]) - task["goal"][k]) > 1e-9 for k in ("x", "y")):
        raise ValueError("model waypoint is outside the public task action space")
    if not isinstance(answer["brief"], str):
        raise ValueError("invalid public brief")


def call(model_input: dict, effort: str, directory: Path, timeout_s: int = 180,
         schema_override: dict | None = None, instructions_override: str | None = None,
         validator=None) -> dict:
    if effort not in ("low", "medium", "high"):
        raise ValueError("initial calibration uses low/medium/high")
    directory.mkdir(parents=True, exist_ok=False)
    # The CLI exposes its cwd to the model. Never put case/arm labels there.
    work = directory.parent / "controller-workspaces" / uuid.uuid4().hex
    work.mkdir(parents=True)
    schema = directory / "response-schema.json"
    selected_schema = schema_override or SCHEMA
    selected_instructions = instructions_override or INSTRUCTIONS
    schema.write_text(json.dumps(selected_schema), encoding="utf-8")
    instructions_file = directory / "controller-instructions.txt"
    instructions_file.write_text(selected_instructions, encoding="utf-8")
    prompt = "CURRENT INPUT:\n" + json.dumps(model_input, ensure_ascii=False, sort_keys=True)
    (directory / "input.json").write_text(json.dumps(model_input, ensure_ascii=False, indent=2), encoding="utf-8")
    (directory / "prompt.txt").write_text(prompt, encoding="utf-8")
    executable = shutil.which("codex") or os.environ.get("CODEX_CLI_PATH")
    if not executable:
        raise RuntimeError("authenticated Codex CLI is unavailable")
    command = [executable, "exec", "--ignore-user-config", "--ephemeral", "--skip-git-repo-check",
               "--sandbox", "read-only", "--model", MODEL, "--json", "--color", "never",
               "--output-schema", str(schema.resolve()),
               "--config", f'model_reasoning_effort="{effort}"',
               "--config", f'model_instructions_file={json.dumps(str(instructions_file.resolve()))}',
               "--config", 'model_reasoning_summary="none"',
               "--config", 'model_provider="research_http"',
               "--config", 'model_providers.research_http={name="Research HTTPS",wire_api="responses",supports_websockets=false,requires_openai_auth=true,request_max_retries=0,stream_max_retries=0,stream_idle_timeout_ms=120000}',
               "--config", 'web_search="disabled"',
               "--disable", "shell_tool", "--disable", "multi_agent", "--disable", "apps",
               "--disable", "browser_use", "--disable", "computer_use", "--disable", "view_image", "-"]
    environment = {k: v for k, v in os.environ.items() if not k.startswith("CODEX_")}
    started = time.perf_counter()
    start_unix_ns = time.time_ns()
    try:
        result = subprocess.run(command, input=prompt, cwd=work, env=environment, capture_output=True,
                                text=True, encoding="utf-8", errors="replace", timeout=timeout_s)
    except subprocess.TimeoutExpired as error:
        decode = lambda s: s.decode("utf-8", errors="replace") if isinstance(s, bytes) else (s or "")
        result = SimpleNamespace(returncode=124, stdout=decode(error.stdout), stderr=decode(error.stderr))
    elapsed = time.perf_counter() - started
    (directory / "stderr.log").write_text(result.stderr, encoding="utf-8")
    events, usage, final, violations = clean_events(result.stdout)
    (directory / "events.jsonl").write_text("".join(json.dumps(e, ensure_ascii=False) + "\n" for e in events), encoding="utf-8")
    record = {"transport": "codex_exec_chatgpt_auth", "requested_model": MODEL, "effort": effort,
              "requested_unix_ns": start_unix_ns, "elapsed_s": elapsed, "usage": usage,
              "returncode": result.returncode, "tool_violations": violations, "answer": None,
              "transport_events": [e for e in events if e.get("type") == "error" or e.get("item", {}).get("type") == "error"],
              "adapter_version": "research-https-controller-v2", "client_command": command,
              "instructions_sha256": hashlib.sha256(selected_instructions.encode()).hexdigest(),
              "schema_sha256": hashlib.sha256(json.dumps(selected_schema, sort_keys=True).encode()).hexdigest(),
              "input_sha256": hashlib.sha256(json.dumps(model_input, sort_keys=True).encode()).hexdigest(),
              "money_cost": None, "status": "failed"}
    if result.returncode == 0 and not violations and final and usage:
        try:
            answer = json.loads(final)
            (validator or validate_action)(answer, model_input)
            record.update(answer=answer, status="ok")
        except (ValueError, TypeError, KeyError) as error:
            record["validation_error"] = str(error)
    (directory / "result.json").write_text(json.dumps(record, ensure_ascii=False, indent=2), encoding="utf-8")
    return record

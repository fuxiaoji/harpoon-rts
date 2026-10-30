"""Reproduce the saved N1 EM/HM plan-age scheduler differential, offline.

Only the frozen pure-Python scheduler is loaded. This script does not import a
model client, launch a subprocess, start the native engine, or change source
receipts. The output is created exclusively and is never overwritten.

Example:
    python scripts/audit-realtime-naval-plan-age.py --stage <N1-stage> \
        --out <new-output.json>
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import importlib.util
import json
from pathlib import Path
import sys


CASES = (("01-EM", "EM"), ("06-HM600", "HM600"))
CANONICAL_NAME = "naval-plan-age-differential-audit.json"


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def read_jsonl(path):
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()
            if line.strip()]


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_scheduler(path):
    # Avoid writing __pycache__ inside the immutable evidence archive.
    previous = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec = importlib.util.spec_from_file_location("frozen_naval_plan_age_scheduler", path)
        require(spec is not None and spec.loader is not None, "cannot load frozen scheduler")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    finally:
        sys.dont_write_bytecode = previous


def simulate(scheduler, policy, requests, timing, inputs, cap, postaccept):
    state = scheduler.new_state(policy)
    audit, issued, checks = [], [], []
    last_accepted = 0
    delivered = [row for row in requests if row["received_tick"] is not None]
    receipts = {row["received_tick"]: row for row in delivered}
    require(len(receipts) == len(delivered), "multiple receipts at the same tick")
    for row in timing:
        tick = row["tick"]
        # A dispatch at this same tick may already have replaced the trace's
        # plan_age_s with zero. Rebuild its pre-acceptance value independently.
        features = dict(row["features"], plan_age_s=tick - last_accepted)
        state, events = scheduler.observe(state, features)
        audit.extend(events)
        if tick in receipts:
            receipt = receipts[tick]
            require(state["inflight"] is not None and
                    state["inflight"]["request_id"] == receipt["id"],
                    "saved receipt does not match the replayed in-flight request")
            if receipt["usable"]:
                last_accepted = tick
            finished_features = (dict(features, plan_age_s=0)
                                 if postaccept and receipt["usable"] else features)
            state, events = scheduler.finish_request(
                state, receipt["id"], finished_features,
                success=receipt["usable"],
                wake_conditions=(receipt.get("wake_conditions")
                                 if receipt["usable"] else None))
            audit.extend(events)
        if len(issued) < cap and scheduler.is_ready(state):
            fresh = dict(features, plan_age_s=tick - last_accepted)
            state, events = scheduler.observe(state, fresh)
            audit.extend(events)
            ident = "request-%03d" % len(issued)
            state, _ = scheduler.start_request(state, ident, fresh)
            require(ident in inputs,
                    "counterfactual issues an unsaved request; cannot reuse future observations")
            check = {"id": ident, "tick": tick,
                     "matches_saved_request_tick": inputs[ident]["observation"]["tick"] == tick,
                     "features_equal_saved_input": fresh == inputs[ident]["scheduler_features"]}
            checks.append(check)
            require(check["matches_saved_request_tick"] and check["features_equal_saved_input"],
                    "counterfactual leaves the saved input trajectory; no closed-loop claim is valid")
            issued.append({"id": ident, "tick": tick})
    return {"state": state, "audit": audit, "issued": issued, "input_checks": checks}


def core_signature(result):
    """Ignore creation time/producer metadata when comparing saved audits."""
    keys = ("run", "requests", "source_audit_rows", "old_audit_rows_exactly_equal_saved",
            "old_final_scheduler_exactly_equal_saved", "old_issued", "postaccept_issued",
            "dispatch_times_equal", "all_dispatch_features_equal_saved_model_inputs",
            "dispatch_input_checks", "installation_comparison", "audit_differences",
            "postaccept_final_scheduler_equal_original")
    return [{key: row[key] for key in keys} for row in result["runs"]]


def analyze(stage):
    base = stage / "live-pilot"
    frozen = base / "resume-execution-source"
    scheduler_path = frozen / "naval_scheduler.py"
    frozen_manifest = read_json(base / "resume-source-manifest.json")
    require(sha256(scheduler_path) == frozen_manifest["naval_scheduler.py"],
            "frozen scheduler does not match its source manifest")
    scheduler = load_scheduler(scheduler_path)
    results, source_paths = [], []
    for name, policy in CASES:
        directory = base / name
        requests = read_json(directory / "requests.json")
        timing = read_jsonl(directory / "timing.jsonl")
        saved = read_jsonl(directory / "scheduler-events.jsonl")
        protocol = read_json(directory / "protocol.json")
        require(protocol["policy"] == policy and protocol["backend"] == "model",
                "expected the saved real-model EM/HM interface pilot")
        horizon, cap = protocol["horizon_s"], protocol["max_calls"]
        require(horizon == 1200 and cap == 3, "this audit covers the N1 1200-second/3-call pilot")
        require([row["tick"] for row in timing] == list(range(horizon)),
                "timing trace is not the complete consecutive pilot prefix")
        inputs = {row["id"]: read_json(directory / "inputs" / f'{row["id"]}.json')
                  for row in requests}
        old = simulate(scheduler, policy, requests, timing, inputs, cap, False)
        fixed = simulate(scheduler, policy, requests, timing, inputs, cap, True)
        require(old["audit"] == saved, "old scheduler audit does not exactly reproduce the archive")
        require(old["state"] == read_json(directory / "scheduler-final.json"),
                "old final scheduler state differs from the archive")
        expected = [{"id": row["id"], "tick": row["request_tick"]} for row in requests]
        require(old["issued"] == fixed["issued"] == expected,
                "dispatches differ; do not infer unchanged model execution")
        require(len(old["audit"]) == len(fixed["audit"]), "differential audit rows cannot be aligned")
        differences = [{"row_index": index, "sequence": left["sequence"],
                        "kind": left["kind"], "old": left, "postaccept_age_zero": right}
                       for index, (left, right) in enumerate(zip(old["audit"], fixed["audit"]))
                       if left != right]
        old_hits = {row["observed_tick"]: row for row in old["audit"] if row.get("on_installation")}
        fixed_hits = {row["observed_tick"]: row for row in fixed["audit"] if row.get("on_installation")}
        installations = []
        for request in requests:
            tick = request["received_tick"]
            left, right = old_hits.get(tick), fixed_hits.get(tick)
            installations.append({"request_id": request["id"], "response_tick": tick,
                                  "old_hit_clauses": left["clause_ids"] if left else [],
                                  "postaccept_hit_clauses": right["clause_ids"] if right else [],
                                  "old_features": left.get("features") if left else None,
                                  "postaccept_features": right.get("features") if right else None})
        paths = [directory / filename for filename in
                 ("requests.json", "timing.jsonl", "scheduler-events.jsonl", "scheduler-final.json",
                  "protocol.json", "dispatch-ledger.json")]
        paths.extend(directory / "inputs" / f'{row["id"]}.json' for row in requests)
        source_paths.extend(paths)
        results.append({"run": name, "policy": policy, "requests": len(requests),
                        "source_audit_rows": len(saved), "old_audit_rows_exactly_equal_saved": True,
                        "old_final_scheduler_exactly_equal_saved": True,
                        "old_issued": old["issued"], "postaccept_issued": fixed["issued"],
                        "dispatch_times_equal": True,
                        "all_dispatch_features_equal_saved_model_inputs": True,
                        "dispatch_input_checks": fixed["input_checks"],
                        "installation_comparison": installations, "audit_differences": differences,
                        "postaccept_final_scheduler_equal_original": fixed["state"] == old["state"],
                        "model_requests_added": 0, "native_trajectories_added": 0})
    source_paths.extend(frozen / name for name in
                        ("naval_runner.py", "naval_scheduler.py", "naval_inputs.py",
                         "m3b_llm_client.py", "naval-n1-protocol.md"))
    source_paths.extend(base / name for name in
                        ("resume-source-manifest.json", "execution-source-manifest.json", "resume-intent.json"))
    return {"version": "naval-plan-age-differential-audit-v1",
            "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(), "passed": True,
            "producer": {"path": "scripts/audit-realtime-naval-plan-age.py",
                         "sha256": sha256(Path(__file__).resolve())},
            "question": "Does pre-acceptance age in newly installed watches alter these saved EM/HM dispatches?",
            "semantic_defect": "PublicFeatures.accepted resets last_plan_tick, but the original runner passed pre-acceptance age to finish_request. New watches should use age zero after acceptance.",
            "source_basis": "Immutable live-pilot/resume-execution-source; current corrected runner is not executed.",
            "algorithm": "Replay the frozen pure scheduler over all archived feature ticks twice with identical saved receipts. Reconstruct pre-acceptance plan age from saved usable responses; change only finish_request age to zero after acceptance. Verify exact original audit/final state, issued times and input features.",
            "runs": results,
            "sources": [{"path": path.relative_to(stage).as_posix(), "bytes": path.stat().st_size,
                         "sha256": sha256(path)} for path in sorted(set(source_paths))],
            "scope_limits": [
                "Conditional on saved observations, response contents, delivery ticks and cap=3; not a new model/engine run.",
                "Only identical dispatch times/input features allow the saved future trajectory to be reused here.",
                "Installation attribution and old-age metadata differ as listed; source audits and receipts remain unchanged.",
                "The defect is not harmless generally: an age-only watch can incorrectly cause immediate dispatch.",
                "No claims about calls beyond the saved cap, other scenes, replies, delivery times or API reconnection.",
                "Native World/controller replay is outside this pure scheduler check."],
            "model_requests_added": 0, "native_trajectories_added": 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--out", type=Path,
                        help="New JSON output; default is the canonical stage audit, never overwritten")
    args = parser.parse_args()
    stage = args.stage.resolve()
    output = (args.out or stage / CANONICAL_NAME).resolve()
    if output.exists():
        parser.error(f"output already exists; choose a new --out: {output}")
    result = analyze(stage)
    canonical = stage / CANONICAL_NAME
    if canonical.exists() and canonical != output:
        previous = read_json(canonical)
        require(core_signature(result) == core_signature(previous),
                "core conclusions differ from the original independent audit")
        result["original_audit_comparison"] = {"path": canonical.relative_to(stage).as_posix(),
                                                "sha256": sha256(canonical),
                                                "core_results_exactly_equal": True}
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("x", encoding="utf-8") as stream:
        json.dump(result, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print(json.dumps({"passed": result["passed"], "output": str(output), "sha256": sha256(output),
                      "runs": [{"run": row["run"], "audit_rows": row["source_audit_rows"],
                                "dispatches": row["postaccept_issued"]} for row in result["runs"]],
                      "original_core_equal": result.get("original_audit_comparison", {}).get("core_results_exactly_equal"),
                      "model_requests_added": 0, "native_trajectories_added": 0}))


if __name__ == "__main__":
    main()

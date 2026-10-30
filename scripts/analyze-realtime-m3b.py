"""Independent, provider-free audit and replay of the bounded M3b R3 pilot.

Never writes into r3 or frozen-source. Full analysis requires terminal request
blocks; --audit-only inspects the completed prefix without starting navigation.
--smoke-count permits a bounded replay while acquisition continues, in a NEW
explicit output directory. Neither mode can send or retry a model request.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import gc
import hashlib
import importlib
import json
import math
from pathlib import Path
import random
import statistics
import sys
import time

EFFORTS = ("low", "medium", "high")
DELAYS = (0, 60, 180)
USAGE_KEYS = ("input_tokens", "output_tokens", "cached_input_tokens",
              "cache_write_input_tokens", "reasoning_output_tokens")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def save(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2,
                                    allow_nan=False) + "\n", encoding="utf-8")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False,
                                    separators=(",", ":"), allow_nan=False).encode()).hexdigest()


def mean(values):
    values = list(values)
    return statistics.mean(values) if values else None


def usage_totals(rows):
    result = {}
    for key in USAGE_KEYS:
        known = [r["usage"][key] for r in rows
                 if r.get("usage") is not None and r["usage"].get(key) is not None]
        require(all(isinstance(v, (int, float)) and v >= 0 for v in known), "Invalid usage")
        result[key] = {"known_sum": sum(known) if known else None,
                       "known_requests": len(known), "unknown_requests": len(rows)-len(known)}
    return result


def summarize(rows):
    valid = [r for r in rows if r["status"] == "ok"]
    return {"completed_receipts": len(rows), "valid_plans": len(valid),
            "fallback_requests": len(rows)-len(valid),
            "status_counts": dict(Counter(r["status"] for r in rows)),
            "failure_categories": dict(Counter(r["failure_category"] for r in rows if r["fallback"])),
            "late_actual_delay_ge_240": sum(r["late"] for r in rows),
            "elapsed_median_s": statistics.median(r["elapsed_s"] for r in rows) if rows else None,
            "valid_elapsed_median_s": statistics.median(r["elapsed_s"] for r in valid) if valid else None,
            "all_requested_risk_mean": mean(r["risk_value"] for r in rows),
            "valid_conditional_risk_mean": mean(r["risk_value"] for r in valid),
            "valid_conditional_risk_gap_mean": mean(r["risk_gap"] for r in valid),
            "risk_optimal_valid": sum(abs(r["risk_gap"]) < 1e-9 for r in valid),
            "all_requested_mean_values": {k: mean(r["values"][k] for r in rows)
                                          for k in ("0", "60", "180", "actual")},
            "valid_conditional_mean_values": {k: mean(r["values"][k] for r in valid)
                                              for k in ("0", "60", "180", "actual")},
            "usage": usage_totals(rows),
            "money_known_sum": sum(r["money"] for r in rows if r["money"] is not None)
                if any(r["money"] is not None for r in rows) else None,
            "money_unknown_requests": sum(r["money"] is None for r in rows)}


def percentile(values, fraction):
    values = sorted(values)
    q = (len(values)-1)*fraction
    lo, hi = math.floor(q), math.ceil(q)
    return values[lo] + (values[hi]-values[lo])*(q-lo)


def paired_summary(rows, case_ids, bootstrap_n=5000):
    """Aggregate the TWO repeats first, then resample case cells, never calls."""
    grouped = defaultdict(list)
    for row in rows:
        grouped[row["case_id"], row["effort"]].append(row)
    cases = []
    for case_id in case_ids:
        arms = {}
        for effort in EFFORTS:
            rr = grouped[case_id, effort]
            arms[effort] = {"completed": len(rr), "valid": sum(r["status"] == "ok" for r in rr),
                           "repeats": sorted(r["repeat"] for r in rr),
                           "risk_mean": mean(r["risk_value"] for r in rr),
                           "actual_mean": mean(r["values"]["actual"] for r in rr)}
        cases.append({"case_id": case_id, "efforts": arms,
                      "all_efforts_both_repeats_delivered": all(
                          a["completed"] == 2 and a["valid"] == 2 for a in arms.values()),
                      "all_efforts_both_repeats_attempted": all(a["completed"] == 2 for a in arms.values())})
    comparisons = []
    for conditional in (False, True):
        for lhs, rhs in (("medium", "low"), ("high", "low"), ("high", "medium")):
            for metric in ("risk_value", "actual_value"):
                diffs = []
                for case_id in case_ids:
                    aa, bb = grouped[case_id, lhs], grouped[case_id, rhs]
                    if len(aa) != 2 or len(bb) != 2:
                        continue
                    if conditional and any(r["status"] != "ok" for r in aa+bb):
                        continue
                    get = lambda r: r["risk_value"] if metric == "risk_value" else r["values"]["actual"]
                    diffs.append({"case_id": case_id, "difference": mean(map(get, aa))-mean(map(get, bb))})
                values = [r["difference"] for r in diffs]
                interval = None
                if len(values) >= 2:
                    rng = random.Random(2026102301)
                    boot = [mean(rng.choices(values, k=len(values))) for _ in range(bootstrap_n)]
                    interval = [percentile(boot, .025), percentile(boot, .975)]
                comparisons.append({"contrast": lhs+"-"+rhs, "metric": metric,
                    "population": "both_repeats_valid_per_arm" if conditional else "both_repeats_attempted_per_arm_including_fallback",
                    "paired_case_count": len(values), "case_differences": diffs,
                    "mean_case_difference": mean(values), "descriptive_case_bootstrap_95_interval": interval})
    return {"case_aggregates": cases, "comparisons": comparisons,
            "bootstrap_draws": bootstrap_n, "bootstrap_seed": 2026102301,
            "scope": "Exploratory resampling of the observed fixed case cells, each aggregating two repeats. Twelve designed cells share three templates; these intervals do not establish population generalization, formal significance, or control template dependence. Partial/valid-only subsets may be selected by availability."}


def complete_triplets(rows, case_ids):
    """Keep full case x repeat triplets; quota-affected groups never enter."""
    indexed = {(r["case_id"], r["repeat"], r["effort"]): r for r in rows}
    require(len(indexed) == len(rows), "Duplicate case/repeat/effort row")
    triplets, eligible = [], []
    for case in case_ids:
        for repeat in (0, 1):
            found = {e: indexed.get((case, repeat, e)) for e in EFFORTS}
            complete = all(r is not None for r in found.values())
            valid = complete and all(r["status"] == "ok" for r in found.values())
            same_block = complete and len({r["block"] for r in found.values()}) == 1
            row = {"case_id": case, "repeat": repeat,
                   "slots": {e: r["index"] if r else None for e, r in found.items()},
                   "statuses": {e: r["status"] if r else "unattempted" for e, r in found.items()},
                   "complete_attempted": complete, "complete_valid": valid,
                   "single_acquisition_block": same_block,
                   "quota_affected": any(r and r["failure_category"] == "quota_refusal" for r in found.values()),
                   "included_in_complete_valid_comparison": valid and same_block}
            triplets.append(row)
            if valid and same_block:
                eligible.append(found)
    comparisons = []
    for lhs, rhs in (("medium", "low"), ("high", "low"), ("high", "medium")):
        for metric in ("risk_value", "actual_value"):
            by_case = defaultdict(list)
            for triplet in eligible:
                a, b = triplet[lhs], triplet[rhs]
                get = lambda r: r["risk_value"] if metric == "risk_value" else r["values"]["actual"]
                by_case[a["case_id"]].append(get(a)-get(b))
            case_means = [{"case_id": c, "complete_repeat_count": len(v), "mean_difference": mean(v)}
                          for c, v in by_case.items()]
            comparisons.append({"contrast": lhs+"-"+rhs, "metric": metric,
                "complete_case_repeat_triplets": len(eligible), "distinct_case_cells": len(case_means),
                "case_means": case_means, "mean_case_difference": mean(r["mean_difference"] for r in case_means)})
    return {"triplets": triplets, "complete_attempted_triplets": sum(r["complete_attempted"] for r in triplets),
            "complete_valid_same_block_triplets": len(eligible), "comparisons": comparisons,
            "scope": "Selected complete-delivery triplets only. Availability exclusion is not missing-at-random, does not erase failed requests from all-request results, and provides no causal reliability estimate by effort."}


def load_frozen(stage, binary):
    source = stage / "frozen-source/research/realtime"
    sys.dont_write_bytecode = True
    sys.path.insert(0, str(source))
    pilot = importlib.import_module("run_m3b_pilot")
    require(Path(pilot.__file__).resolve().parent == source, "Wrong pilot module")
    pilot.check_gate(stage, str(binary))
    # Defense in depth: imports may expose provider APIs; the analyzer disables them.
    def forbidden(*args, **kwargs):
        raise RuntimeError("Provider execution is forbidden in evidence analysis")
    pilot.client.call = forbidden
    for module in (pilot.study, sys.modules["real_nav"], pilot.client):
        require(Path(module.__file__).resolve().parent == source, "Non-frozen module imported")
    return pilot


def audit_receipts(stage, pilot, terminal):
    r3 = stage / "r3"
    manifest = read(r3 / "manifest.json")
    schedule = read(r3 / "schedule.json")
    snapshots = {s["id"]: s for s in read(r3 / "snapshots.json")}
    files = set()
    def use(path):
        files.add(path)
        return read(path)
    for name in ("manifest.json", "schedule.json", "snapshots.json", "references.json"):
        files.add(r3/name)
    original_start = use(r3/"request-run.json")
    require(original_start["manifest_sha256"] == sha(r3/"manifest.json"), "Original run freeze mismatch")
    original_end = use(r3/"request-completed.json") if (r3/"request-completed.json").exists() else None
    require(not terminal or original_end is not None, "Original request block is still running")
    original_n = original_end["sent"] if original_end else 72
    blocks = {"original": {"cli_version": manifest["cli_version"],
              "start": original_start["start_unix_ns"],
              "end": original_end["end_unix_ns"] if original_end else None,
              "indices": list(range(original_n))}}
    for directory in sorted(r3.glob("recovery-block-*")):
        m = use(directory/"manifest.json")
        require(m["frozen_manifest_sha256"] == sha(r3/"manifest.json"), "Recovery freeze mismatch")
        require(m["total_attempt_cap"] == 72, "Recovery cap changed")
        require(m["previous_slots"] == list(range(len(m["previous_slots"]))), "Recovery prior slots not prefix")
        require(m["slots"] == schedule[len(m["previous_slots"]):], "Recovery schedule changed")
        require(m["max_additional_calls"] == len(m["slots"]), "Recovery extra call cap mismatch")
        end = use(directory/"completed.json") if (directory/"completed.json").exists() else None
        require(not terminal or end is not None, "Recovery request block is still running")
        if end:
            sent = end["sent"]
            require(end["count"] == len(sent), "Recovery completion count mismatch")
            require(sent == [s["index"] for s in m["slots"][:len(sent)]], "Recovery duplicate or skipped slot")
        else:
            sent = [s["index"] for s in m["slots"]]
        blocks[directory.name] = {"cli_version": m["cli_version"], "start": m["created_unix_ns"],
                                 "end": end["end_unix_ns"] if end else None, "indices": sent}
        for name in ("stopped.json",):
            if (directory/name).exists(): files.add(directory/name)
    for name in ("stopped.json",):
        if (r3/name).exists(): files.add(r3/name)
    owners = {}
    for name, block in blocks.items():
        for index in block["indices"]:
            require(index not in owners, "A request slot was allocated to multiple blocks")
            owners[index] = name
    directories = sorted((r3/"calls").iterdir())
    require(all(d.is_dir() and d.name.isdigit() and len(d.name) == 3 for d in directories), "Unexpected request path")
    indices = [int(d.name) for d in directories]
    require(indices == list(range(len(indices))) and len(indices) <= 72, "Requests not unique capped prefix")
    rows, in_flight, identities = [], [], set()
    for directory in directories:
        index = int(directory.name)
        require(index in owners, "Receipt outside declared request block")
        if not (directory/"result.json").exists():
            in_flight.append(index)
            continue
        slot, rec = schedule[index], use(directory/"result.json")
        observation = snapshots[slot["case_id"]]["input"]
        require(use(directory/"input.json") == observation, "Input projection changed")
        require(rec["input_sha256"] == pilot.input_digest(observation), "Receipt input hash mismatch")
        schema = use(directory/"response-schema.json")
        require(schema == manifest["schema"] == pilot.study.SCHEMA, "Schema mismatch")
        require(rec["schema_sha256"] == hashlib.sha256(json.dumps(schema, sort_keys=True).encode()).hexdigest(), "Schema hash mismatch")
        instructions = (directory/"controller-instructions.txt").read_text(encoding="utf-8")
        require(instructions == manifest["instructions"] == pilot.INSTRUCTIONS, "Instruction changed")
        require(rec["instructions_sha256"] == hashlib.sha256(instructions.encode()).hexdigest(), "Instruction hash mismatch")
        prompt = "CURRENT INPUT:\n" + json.dumps(observation, ensure_ascii=False, sort_keys=True)
        require((directory/"prompt.txt").read_text(encoding="utf-8") == prompt, "Prompt changed")
        require(rec["requested_model"] == manifest["model"] == pilot.client.MODEL and rec["effort"] == slot["effort"], "Model/effort mismatch")
        require(rec["transport"] == "codex_exec_chatgpt_auth", "Unexpected transport")
        require(math.isfinite(rec["elapsed_s"]) and rec["elapsed_s"] >= 0, "Invalid duration")
        require(rec["status"] in ("ok", "failed"), "Unknown receipt status")
        command = rec["client_command"]
        require(command[command.index("--model")+1] == manifest["model"], "Command model mismatch")
        require(f'model_reasoning_effort="{slot["effort"]}"' in command, "Command effort mismatch")
        require("--ignore-user-config" in command and "--ephemeral" in command and "project_doc_max_bytes=0" in command, "Isolation flags missing")
        require(any("request_max_retries=0,stream_max_retries=0" in arg for arg in command), "Provider retries not disabled")
        require(rec["cwd_empty_after_call"] and len(Path(rec["cwd"]).name) == 32, "Unlabeled empty cwd check failed")
        identity = (rec["requested_unix_ns"], rec["cwd"])
        require(identity not in identities, "Duplicate request identity")
        identities.add(identity)
        block_name = owners[index]
        block = blocks[block_name]
        require(rec["requested_unix_ns"] >= block["start"], "Request before block start")
        require(block["end"] is None or rec["requested_unix_ns"] <= block["end"], "Request outside block interval")
        events_path = directory/"events.jsonl"
        text = events_path.read_text(encoding="utf-8")
        events, event_usage, final, violations = pilot.client.clean_events(text)
        require(event_usage == rec["usage"] and violations == rec["tool_violations"], "Event/receipt accounting mismatch")
        if rec["status"] == "ok":
            require(rec["returncode"] == 0 and not violations and final is not None, "Invalid success receipt")
            require(json.loads(final) == rec["answer"], "Final answer/receipt mismatch")
            pilot.study.validate(rec["answer"], observation)
        else:
            require(rec["answer"] is None, "Failed reply retained actionable answer")
        category = None
        if rec["status"] != "ok":
            errors = json.dumps(rec.get("transport_events", []), ensure_ascii=False).lower()
            category = ("quota_refusal" if "usage limit" in errors else
                        "tool_violation" if violations else
                        "timeout" if rec["returncode"] == 124 else
                        "validation_error" if rec.get("validation_error") else "transport_or_other")
        for path in directory.iterdir():
            require(path.is_file(), "Unexpected nested request directory")
            files.add(path)
        rows.append({**slot, "receipt": rec, "block": block_name,
                     "cli_version": block["cli_version"], "failure_category": category})
    require(not terminal or not in_flight, "Uncertain in-flight request; final analysis prohibited")
    require([r["index"] for r in rows] == list(range(len(rows))), "Completed receipts must form prefix")
    if terminal:
        require(sorted(owners) == indices, "Terminal block declarations disagree with receipt directories")
    for prev, nxt in zip(rows, rows[1:]):
        a, b = prev["receipt"], nxt["receipt"]
        require(b["requested_unix_ns"] > a["requested_unix_ns"], "Non-monotonic request order")
        # perf_counter starts immediately before time_ns; allow 10ms clock granularity.
        require(b["requested_unix_ns"] + 10_000_000 >= a["requested_unix_ns"] + a["elapsed_s"]*1e9,
                "Overlapping serial request windows")
    return rows, snapshots, read(r3/"references.json"), sorted(files), {
        "scheduled": 72, "completed_receipts": len(rows), "in_flight_slots": in_flight,
        "unattempted_slots": list(range(len(indices), 72)), "blocks": blocks,
        "per_effort_completed": {e: sum(r["effort"] == e for r in rows) for e in EFFORTS},
        "all_input_model_effort_schedule_checks_passed": True,
        "explicit_provider_retries_zero": True,
        "no_duplicate_receipt_identity": True,
        "scope": "Verifies saved invocation/receipt ledger and declared provider retry flags; does not infer unreported provider-side activity."}


def evaluate_request(pilot, nav, item, snapshot, frozen_ref):
    study = pilot.study
    rec, case = item["receipt"], snapshot["case"]
    action = rec["answer"]["choice"] if rec["status"] == "ok" else "wait"
    delays = {str(d): d for d in DELAYS}
    delays["actual"] = math.ceil(rec["elapsed_s"]*5)
    refs, evaluations, audits = {}, {}, {}
    for key, delay in delays.items():
        evaluation = study.evaluate(nav, case, action, delay)
        reference = study.same_delay_optimum(nav, case, delay)
        audits[key] = {"selected": study.audit_result(case, evaluation),
                       "all_reference_actions": {a: study.audit_result(case, value)
                            for a, value in reference["evaluations"].items()}}
        require(abs(evaluation["expected_value"]-reference["action_values"][action]) < 1e-9, "Action/reference value differs")
        if key != "actual":
            require(pilot.strip_timing(reference) == pilot.strip_timing(frozen_ref["by_delay"][key]), "Frozen same-delay reference changed")
        refs[key], evaluations[key] = reference, evaluation
    action_values = {a: mean(refs[str(d)]["action_values"][a] for d in DELAYS) for a in study.actions(case)}
    require(action_values == frozen_ref["risk_reference"]["action_values"], "Risk objective reference changed")
    risk = mean(evaluations[str(d)]["expected_value"] for d in DELAYS)
    gap = frozen_ref["risk_reference"]["expected_value"]-risk
    require(gap >= -1e-9, "Plan exceeds frozen risk bound")
    row = {k: item[k] for k in ("index", "case_id", "repeat", "effort", "block", "cli_version", "failure_category")}
    row.update(status=rec["status"], fallback=rec["status"] != "ok", action=action,
        structure=case["structure"], information=case["information"], slack_s=case["slack_s"],
        elapsed_s=rec["elapsed_s"], actual_delay_s=delays["actual"], late=delays["actual"] >= 240,
        risk_value=risk, risk_gap=gap, risk_reference=frozen_ref["risk_reference"]["expected_value"],
        values={k: v["expected_value"] for k, v in evaluations.items()},
        same_delay_frontier_values={k: v["expected_value"] for k, v in refs.items()},
        same_delay_gaps={k: refs[k]["expected_value"]-v["expected_value"] for k, v in evaluations.items()},
        usage=rec["usage"], money=rec.get("money_cost"), input_sha256=rec["input_sha256"])
    return {"summary": row, "evaluations": evaluations, "references": refs, "audits": audits}


def self_test():
    # Unknown usage must stay unknown, and pairs with one repeat cannot enter CI.
    assert usage_totals([{"usage": None}])["input_tokens"] == {
        "known_sum": None, "known_requests": 0, "unknown_requests": 1}
    assert usage_totals([{"usage": {"input_tokens": 0}}, {"usage": None}])["input_tokens"]["known_sum"] == 0
    rr = [{"case_id": "x", "effort": e, "repeat": r, "status": "ok",
           "risk_value": 1 if e == "low" else 3, "values": {"actual": r}}
          for e in EFFORTS for r in (0, 1)]
    paired = paired_summary(rr, ["x"], 10)
    assert paired["comparisons"][0]["mean_case_difference"] == 2
    assert paired["comparisons"][0]["descriptive_case_bootstrap_95_interval"] is None
    assert paired_summary(rr[:-1], ["x"], 10)["case_aggregates"][0]["all_efforts_both_repeats_attempted"] is False
    assert percentile([0, 10], .5) == 5
    for index, row in enumerate(rr):
        row.update(index=index, failure_category=None, block="b")
    assert complete_triplets(rr, ["x"])["complete_valid_same_block_triplets"] == 2
    rr[0].update(status="failed", failure_category="quota_refusal")
    assert complete_triplets(rr, ["x"])["complete_valid_same_block_triplets"] == 1
    print(json.dumps({"self_tests": 8, "passed": True, "provider_calls": 0}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--audit-only", action="store_true")
    parser.add_argument("--smoke-count", type=int, default=0)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    require(args.stage is not None, "--stage is required")
    require(args.smoke_count >= 0, "Smoke count must be nonnegative")
    stage = args.stage.resolve()
    binary = (args.binary or stage/"target/release/examples/realtime_nav_service.exe").resolve()
    pilot = load_frozen(stage, binary)
    items, snapshots, references, inputs, audit = audit_receipts(
        stage, pilot, terminal=not args.audit_only and not args.smoke_count)
    if args.audit_only:
        print(json.dumps(audit, ensure_ascii=False, indent=2))
        return
    require(items, "No complete receipts to analyze")
    if args.smoke_count:
        require(args.out is not None, "Smoke requires an explicit NEW --out directory")
        items = items[:args.smoke_count]
    out = (args.out or stage/"r3-analysis-20261023").resolve()
    require(out != stage and out.parent == stage, "Output must be a new immediate child of stage")
    require(out.name.startswith("r3-analysis-"), "Output name must start r3-analysis-")
    require(not out.exists(), "Existing analysis preserved; choose a new output directory")
    out.mkdir()
    (out/"inputs").mkdir()
    (out/"replays").mkdir()
    consumed = []
    for path in inputs:
        rel = path.relative_to(stage)
        target = out/"inputs"/rel
        target.parent.mkdir(parents=True, exist_ok=True)
        data = path.read_bytes()
        target.write_bytes(data)
        consumed.append({"path": rel.as_posix(), "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    Path(out/"analysis-script.py").write_bytes(Path(__file__).read_bytes())
    save(out/"analysis-intent.json", {"created_unix_ns": time.time_ns(), "stage": str(stage),
        "analysis_script_sha256": sha(__file__), "binary_sha256": sha(binary),
        "receipt_audit": audit, "analyzed_slots": [r["index"] for r in items],
        "smoke_only": bool(args.smoke_count), "new_model_requests": 0,
        "nav_batch_receipts": 16, "consumed_inputs": consumed,
        "note": "Replays are conditional diagnostic branches of saved requests. Reference/continuation computation uses paused-quality clock, not a free deployable real-time policy."})
    rows, replay_checks, batches = [], [], []
    for start in range(0, len(items), 16):
        chunk = items[start:start+16]
        with pilot.RealNav(str(binary)) as nav:
            first_pid = nav.process.pid
            for item in chunk:
                bundle = evaluate_request(pilot, nav, item, snapshots[item["case_id"]], references[item["case_id"]])
                rows.append(bundle["summary"])
                save(out/"replays"/f'{item["index"]:03d}.json', bundle)
                print(json.dumps({"phase": "evaluated", "slot": item["index"], "risk_value": bundle["summary"]["risk_value"]}), flush=True)
                del bundle
            first_requests = nav.requests
        del nav
        gc.collect()
        with pilot.RealNav(str(binary)) as nav:
            second_pid = nav.process.pid
            for item in chunk:
                saved = read(out/"replays"/f'{item["index"]:03d}.json')
                replay = evaluate_request(pilot, nav, item, snapshots[item["case_id"]], references[item["case_id"]])
                a, b = pilot.strip_timing(saved), pilot.strip_timing(replay)
                require(a == b, "Fresh-process full evaluation replay mismatch, slot " + str(item["index"]))
                replay_checks.append({"index": item["index"], "primary_pid": first_pid,
                    "replay_pid": second_pid, "all_evaluations_and_candidate_worlds_match": True,
                    "payload_without_timing_sha256": digest(a), "fresh_replay_without_timing_sha256": digest(b),
                    "output_sha256": sha(out/"replays"/f'{item["index"]:03d}.json')})
                del saved, replay, a, b
            second_requests = nav.requests
        del nav
        gc.collect()
        batches.append({"slots": [i["index"] for i in chunk], "primary_pid": first_pid,
                       "replay_pid": second_pid, "native_requests": [first_requests, second_requests]})
        print(json.dumps({"phase": "batch_verified", "slots": batches[-1]["slots"]}), flush=True)
    strata = {}
    for field in ("block", "cli_version", "structure", "information", "slack_s"):
        strata[field] = {str(value): {e: summarize([r for r in rows if r[field] == value and r["effort"] == e]) for e in EFFORTS}
                         for value in sorted({r[field] for r in rows})}
    summary = {"status": "verified_smoke" if args.smoke_count else "verified_final_available_receipts",
        "receipt_audit": audit, "analyzed_receipts": len(rows), "rows": rows,
        "per_effort": {e: summarize([r for r in rows if r["effort"] == e]) for e in EFFORTS},
        "per_snapshot": {case: {e: summarize([r for r in rows if r["case_id"] == case and r["effort"] == e]) for e in EFFORTS} for case in snapshots},
        "strata": strata, "paired_by_case": paired_summary(rows, list(snapshots)),
        "complete_valid_triplets": complete_triplets(rows, list(snapshots)),
        "paired_by_acquisition_block": {block: paired_summary([r for r in rows if r["block"] == block], list(snapshots))
                                         for block in sorted({r["block"] for r in rows})},
        "all_requests": summarize(rows),
        "scope": {"scheduled_slots": 72, "fixed_cases": 12, "templates": 3,
            "not_closed_loop": True, "fallback": "failed requests retain wait first-action plus common public continuation",
            "primary": "posterior expected value equally averaging predefined delays 0/60/180",
            "actual_delay": "ceil(end-to-end wall seconds * 5), not pure reasoning time",
            "unknown_usage": "missing receipt/field remains unknown; known sums are partial",
            "usage_subsets": "reasoning_output is a subset of output; cached_input a subset of input",
            "time_blocks": "CLI and availability changed across blocks; pooled effects are exploratory and not clean causal budget estimates. Quota refusals are not evidence that an effort level is less reliable.",
            "grid_scope": "Twelve fixed condition cells on three templates; tight/wide variants can have identical action values. Case-cell bootstrap intervals describe this condition grid, not a task population.",
            "independence": "replays, delays and repeated requests are not new independent cases"}}
    save(out/"summary.json", summary)
    # Verify the consumed evidence bytes stayed immutable throughout analysis.
    require(all(sha(stage/r["path"]) == r["sha256"] for r in consumed), "Source evidence changed while analyzing")
    pilot.check_gate(stage, str(binary))
    save(out/"validation.json", {"passed": True, "new_model_requests": 0,
        "requests": len(rows), "successes": sum(r["status"] == "ok" for r in rows),
        "failures": sum(r["status"] != "ok" for r in rows),
        "unknown_usage": sum(r["usage"] is None for r in rows),
        "cross_process_replay_passed": True,
        "request_count": len(rows), "selected_evaluations": len(rows)*4,
        "all_same_delay_reference_candidates_audited": True,
        "all_frozen_equal_delay_references_matched": True,
        "all_resources_service_and_return_audits_passed": True,
        "receipt_audit": audit, "batches": batches, "fresh_process_replays": replay_checks,
        "consumed_evidence_unchanged": True, "smoke_only": bool(args.smoke_count)})
    inventory = [{"path": p.relative_to(out).as_posix(), "bytes": p.stat().st_size, "sha256": sha(p)}
                 for p in sorted(out.rglob("*")) if p.is_file()]
    save(out/"artifact-manifest.json", {"files": inventory, "manifest_excludes_itself": True})
    print(json.dumps({"output": str(out), "requests": len(rows), "passed": True}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()

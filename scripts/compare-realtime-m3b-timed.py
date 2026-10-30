"""Post-hoc M3b answer/timing matching, using saved successful requests only.

This is NOT a deployable policy: the target delay is learned from another
request's future end-to-end duration. Negative additional waits are explicitly
infeasible. Replies at/after checkpoint 240 are ignored by the shared public
continuation, so equal late outcomes cannot establish equal answer quality.
No provider calls, request retries, or changes to prior evidence are permitted.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import gc
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import statistics
import sys
import time

EFFORTS = ("low", "medium", "high")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def save(path, data):
    Path(path).write_text(json.dumps(data, ensure_ascii=False, indent=2,
                                    allow_nan=False) + "\n", encoding="utf-8")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False,
                                    separators=(",", ":"), allow_nan=False).encode()).hexdigest()


def mean(values):
    values = list(values)
    return statistics.mean(values) if values else None


def timing_class(low_delay, target_delay, checkpoint=240):
    require(type(low_delay) is int and type(target_delay) is int and min(low_delay, target_delay) >= 0,
            "Delays must be nonnegative integers")
    return {"low_delay_s": low_delay, "target_delay_s": target_delay,
            "additional_wait_s": target_delay-low_delay,
            "nonnegative_additional_wait": target_delay >= low_delay,
            "target_reply_ignored_at_checkpoint": target_delay >= checkpoint,
            "low_original_reply_ignored_at_checkpoint": low_delay >= checkpoint,
            "timing_match_class": "nonnegative_wait" if target_delay >= low_delay else "negative_wait_infeasible"}


def select_pairs(rows):
    grouped = defaultdict(dict)
    for row in rows:
        key = (row["case_id"], row["repeat"])
        require(row["effort"] not in grouped[key], "Duplicate case/repeat/effort")
        grouped[key][row["effort"]] = row
    pairs, exclusions = [], []
    for (case, repeat), arms in sorted(grouped.items()):
        reason = None
        if set(arms) != set(EFFORTS):
            reason = "incomplete_three_effort_group"
        elif any(r["status"] != "ok" for r in arms.values()):
            reason = "at_least_one_failed_request"
        elif len({(r["block"], r["cli_version"]) for r in arms.values()}) != 1:
            reason = "different_acquisition_block_or_cli"
        if reason:
            exclusions.append({"case_id": case, "repeat": repeat, "reason": reason,
                "slots": {e: r["index"] for e, r in arms.items()},
                "statuses": {e: r["status"] for e, r in arms.items()}})
            continue
        for effort in ("medium", "high"):
            low, other = arms["low"], arms[effort]
            require(low["input_sha256"] == other["input_sha256"], "Matched inputs differ")
            pairs.append({"id": f'{low["index"]:03d}-{other["index"]:03d}',
                          "case_id": case, "repeat": repeat, "other_effort": effort,
                          "low": low, "other": other})
    return pairs, exclusions


def summarize_pairs(rows):
    """Report raw counts, then average repeats within each designed case cell."""
    metrics = ("low_original_value", "low_matched_value", "other_matched_value",
               "known_delay_reference_value", "other_minus_low_at_matched_time",
               "low_timing_shift_value", "other_original_minus_low_original")
    by_case = defaultdict(list)
    for row in rows:
        by_case[row["case_id"]].append(row)
    cases = [{"case_id": case, "paired_repeats": len(rr),
              "repeats": [r["repeat"] for r in rr],
              "means": {k: mean(r[k] for r in rr) for k in metrics}}
             for case, rr in sorted(by_case.items())]
    eps = 1e-9
    return {"comparison_pairs": len(rows), "distinct_case_cells": len(cases),
        "same_choice_pairs": sum(r["same_action_label"] for r in rows),
        "nonnegative_wait_pairs": sum(r["nonnegative_additional_wait"] for r in rows),
        "target_late_masked_pairs": sum(r["target_reply_ignored_at_checkpoint"] for r in rows),
        "matched_score_counts_other_minus_low": {
            "positive": sum(r["other_minus_low_at_matched_time"] > eps for r in rows),
            "zero": sum(abs(r["other_minus_low_at_matched_time"]) <= eps for r in rows),
            "negative": sum(r["other_minus_low_at_matched_time"] < -eps for r in rows)},
        "low_wait_shift_counts": {"positive": sum(r["low_timing_shift_value"] > eps for r in rows),
                                  "zero": sum(abs(r["low_timing_shift_value"]) <= eps for r in rows),
                                  "negative": sum(r["low_timing_shift_value"] < -eps for r in rows)},
        "case_aggregates": cases, "equal_case_means": {k: mean(c["means"][k] for c in cases) for k in metrics},
        "scope": "Counts are observed paired comparisons, not independent sample size. Means first aggregate repeated pairs within a fixed case cell. No significance test or population confidence interval."}


def evaluate_pair(pilot, nav, pair, case):
    study = pilot.study
    low, other = pair["low"], pair["other"]
    timing = timing_class(low["actual_delay_s"], other["actual_delay_s"], case["checkpoint"])
    low_original = study.evaluate(nav, case, low["action"], timing["low_delay_s"])
    low_matched = study.evaluate(nav, case, low["action"], timing["target_delay_s"])
    other_matched = study.evaluate(nav, case, other["action"], timing["target_delay_s"])
    reference = study.same_delay_optimum(nav, case, timing["target_delay_s"])
    evaluations = {"low_original": low_original, "low_matched": low_matched, "other_matched": other_matched}
    audits = {key: study.audit_result(case, value) for key, value in evaluations.items()}
    audits["reference_candidates"] = {a: study.audit_result(case, value)
                                      for a, value in reference["evaluations"].items()}
    require(abs(low_original["expected_value"]-low["values"]["actual"]) < 1e-9,
            "Low original outcome differs from verified main analysis")
    require(abs(other_matched["expected_value"]-other["values"]["actual"]) < 1e-9,
            "Other original outcome differs from verified main analysis")
    require(abs(reference["expected_value"]-other["same_delay_frontier_values"]["actual"]) < 1e-9,
            "Reference differs from verified main analysis")
    if low["action"] == other["action"]:
        require(pilot.strip_timing(low_matched) == pilot.strip_timing(other_matched), "Same action/time trajectory differs")
    if timing["target_reply_ignored_at_checkpoint"]:
        require(abs(low_matched["expected_value"]-other_matched["expected_value"]) < 1e-9,
                "Late replies should share continuation")
    lv, lm, om = (x["expected_value"] for x in (low_original, low_matched, other_matched))
    action_contrast, time_shift, total = om-lm, lm-lv, om-lv
    require(abs(total-action_contrast-time_shift) < 1e-9, "Arithmetic contrast decomposition mismatch")
    row = {"id": pair["id"], "case_id": pair["case_id"], "repeat": pair["repeat"],
        "other_effort": pair["other_effort"], "block": low["block"], "cli_version": low["cli_version"],
        "structure": case["structure"], "information": case["information"], "slack_s": case["slack_s"],
        "low_slot": low["index"], "other_slot": other["index"],
        "low_action": low["action"], "other_action": other["action"],
        "same_action_label": low["action"] == other["action"], **timing,
        "low_original_value": lv, "low_matched_value": lm, "other_matched_value": om,
        "known_delay_reference_value": reference["expected_value"],
        "low_matched_gap": reference["expected_value"]-lm,
        "other_matched_gap": reference["expected_value"]-om,
        "other_minus_low_at_matched_time": action_contrast,
        "low_timing_shift_value": time_shift, "other_original_minus_low_original": total,
        "action_contrast_at_applied_reply": action_contrast if timing["target_delay_s"] < case["checkpoint"] else None,
        "deployable_advantage_claim": False,
        "interpretation": "Both target replies are ignored; equality is continuation masking, not answer equivalence."
             if timing["target_reply_ignored_at_checkpoint"] else
             "Negative extra wait cannot be implemented from the observed low request; diagnostic only."
             if not timing["nonnegative_additional_wait"] else
             "Saved low choice waits for a hindsight-selected later timestamp; no online timing policy has been tested."}
    return {"summary": row, "evaluations": evaluations, "target_reference": reference, "audits": audits}


def verify_manifest(directory):
    manifest_path = directory/"artifact-manifest.json"
    manifest = read(manifest_path)
    for row in manifest["files"]:
        path = (directory/row["path"]).resolve()
        require(directory in path.parents, "Manifest path escapes analysis")
        require(path.stat().st_size == row["bytes"] and sha(path) == row["sha256"], "Main analysis artifact mismatch: "+row["path"])
    return manifest


def load_analysis(stage, analysis, binary):
    manifest = verify_manifest(analysis)
    validation = read(analysis/"validation.json")
    summary = read(analysis/"summary.json")
    intent = read(analysis/"analysis-intent.json")
    require(validation["passed"] and validation["cross_process_replay_passed"] and not validation["smoke_only"],
            "A passed final main analysis is required")
    require(summary["status"] == "verified_final_available_receipts", "Smoke/partial summary cannot be final source")
    require(sha(analysis/"analysis-script.py") == intent["analysis_script_sha256"], "Saved analyzer identity mismatch")
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location("verified_m3b_analyzer", analysis/"analysis-script.py")
    analyzer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(analyzer)
    pilot = analyzer.load_frozen(stage, binary)
    items, snapshots, refs, source_files, audit = analyzer.audit_receipts(stage, pilot, terminal=True)
    require(validation["requests"] == len(items) == summary["analyzed_receipts"], "New/unaccounted receipts after main analysis")
    require([r["index"] for r in summary["rows"]] == [r["index"] for r in items], "Main rows do not cover audited receipts")
    for item, row in zip(items, summary["rows"]):
        rec = item["receipt"]
        require(all(row[k] == item[k] for k in ("index", "case_id", "repeat", "effort", "block", "cli_version")), "Summary slot attribution mismatch")
        require(row["status"] == rec["status"] and row["input_sha256"] == rec["input_sha256"], "Summary receipt mismatch")
        require(row["actual_delay_s"] == math.ceil(rec["elapsed_s"]*5), "Summary applied delay mismatch")
        expected = rec["answer"]["choice"] if rec["status"] == "ok" else "wait"
        require(row["action"] == expected, "Summary answer mismatch")
    for artifact in intent["consumed_inputs"]:
        require(sha(stage/artifact["path"]) == artifact["sha256"], "Frozen evidence changed after main analysis")
    return pilot, snapshots, summary, manifest, audit


def self_test():
    assert timing_class(100, 180)["additional_wait_s"] == 80
    assert not timing_class(180, 100)["nonnegative_additional_wait"]
    assert timing_class(100, 240)["target_reply_ignored_at_checkpoint"]
    assert not timing_class(100, 239)["target_reply_ignored_at_checkpoint"]
    assert timing_class(250, 300)["low_original_reply_ignored_at_checkpoint"]
    rr = [{"case_id": "c", "repeat": 0, "effort": e, "status": "ok", "block": "b",
           "cli_version": "v", "index": i, "input_sha256": "x"} for i, e in enumerate(EFFORTS)]
    assert len(select_pairs(rr)[0]) == 2
    rr[1]["status"] = "failed"
    assert len(select_pairs(rr)[0]) == 0
    rr[1].update(status="ok", block="other")
    assert len(select_pairs(rr)[0]) == 0
    assert len(select_pairs(rr[:-1])[1]) == 1
    print(json.dumps({"self_tests": 9, "passed": True, "new_model_requests": 0}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path)
    parser.add_argument("--analysis", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    require(args.stage is not None, "--stage is required")
    stage = args.stage.resolve()
    analysis = (args.analysis or stage/"r3-analysis-20261023").resolve()
    binary = (args.binary or stage/"target/release/examples/realtime_nav_service.exe").resolve()
    pilot, snapshots, source_summary, source_manifest, receipt_audit = load_analysis(stage, analysis, binary)
    pairs, exclusions = select_pairs(source_summary["rows"])
    out = (args.out or stage/"timed-execution-diagnostic-20261023").resolve()
    require(out.parent == stage and out.name.startswith("timed-execution-diagnostic-"), "Output must be a new diagnostic child of stage")
    require(not out.exists(), "Existing diagnostic preserved; select a new output directory")
    out.mkdir()
    (out/"replays").mkdir()
    (out/"inputs").mkdir()
    source_files = [analysis/name for name in ("artifact-manifest.json", "analysis-script.py", "analysis-intent.json", "validation.json", "summary.json")]
    sources = []
    for path in source_files:
        data = path.read_bytes()
        (out/"inputs"/path.name).write_bytes(data)
        sources.append({"path": str(path), "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    (out/"diagnostic-script.py").write_bytes(Path(__file__).read_bytes())
    save(out/"intent.json", {"created_unix_ns": time.time_ns(), "script_sha256": sha(__file__),
        "stage": str(stage), "main_analysis": str(analysis), "source_files": sources,
        "main_manifest_verified_files": len(source_manifest["files"]), "receipt_audit": receipt_audit,
        "selected_pairs": [{k: p[k] for k in ("id", "case_id", "repeat", "other_effort")} for p in pairs],
        "excluded_groups": exclusions, "new_model_requests": 0, "exploratory_post_hoc": True,
        "protocol": "Match saved low choice to medium/high observed future application delay; retain negative-wait diagnostics separately, classify checkpoint masking, and aggregate repeats by fixed case."})
    rows, replay_checks, batches = [], [], []
    # Eight pairs involve at most sixteen source receipts; reset caches each batch.
    for start in range(0, len(pairs), 8):
        chunk = pairs[start:start+8]
        with pilot.RealNav(str(binary)) as nav:
            primary_pid = nav.process.pid
            for pair in chunk:
                result = evaluate_pair(pilot, nav, pair, snapshots[pair["case_id"]]["case"])
                rows.append(result["summary"])
                save(out/"replays"/(pair["id"]+".json"), result)
                del result
            primary_requests = nav.requests
        del nav
        gc.collect()
        with pilot.RealNav(str(binary)) as nav:
            replay_pid = nav.process.pid
            for pair in chunk:
                original = read(out/"replays"/(pair["id"]+".json"))
                repeated = evaluate_pair(pilot, nav, pair, snapshots[pair["case_id"]]["case"])
                a, b = pilot.strip_timing(original), pilot.strip_timing(repeated)
                require(a == b, "Independent native replay mismatch: "+pair["id"])
                replay_checks.append({"id": pair["id"], "primary_pid": primary_pid,
                    "replay_pid": replay_pid, "payload_without_timing_sha256": digest(a),
                    "replay_without_timing_sha256": digest(b), "output_sha256": sha(out/"replays"/(pair["id"]+".json")),
                    "all_candidate_worlds_and_resource_service_return_checks_match": True})
                del original, repeated, a, b
            replay_requests = nav.requests
        del nav
        gc.collect()
        batches.append({"pair_ids": [p["id"] for p in chunk], "primary_pid": primary_pid,
                        "replay_pid": replay_pid, "native_requests": [primary_requests, replay_requests]})
        print(json.dumps({"verified_pairs": start+len(chunk), "total": len(pairs)}), flush=True)
    summaries = {}
    for effort in ("medium", "high"):
        rr = [r for r in rows if r["other_effort"] == effort]
        summaries[effort] = {
            "all_hindsight_pairs": summarize_pairs(rr),
            "nonnegative_wait_before_checkpoint": summarize_pairs([r for r in rr if r["nonnegative_additional_wait"] and not r["target_reply_ignored_at_checkpoint"]]),
            "nonnegative_wait_checkpoint_masked": summarize_pairs([r for r in rr if r["nonnegative_additional_wait"] and r["target_reply_ignored_at_checkpoint"]]),
            "negative_wait_infeasible_diagnostic_only": summarize_pairs([r for r in rr if not r["nonnegative_additional_wait"]])}
    save(out/"summary.json", {"status": "verified_exploratory_post_hoc", "new_model_requests": 0,
        "matched_pairs": len(rows), "complete_case_repeat_triplets": len(pairs)//2,
        "excluded_groups": exclusions, "rows": rows, "by_other_effort": summaries,
        "by_acquisition_block": {block: {e: summarize_pairs([r for r in rows if r["block"] == block and r["other_effort"] == e])
                                        for e in ("medium", "high")} for block in sorted({r["block"] for r in rows})},
        "interpretation": [
            "Exact saved-action/time interventions conditional on fixed public inputs; target delays are known in hindsight from separate requests.",
            "Nonnegative waiting is physically possible, but choosing this target time online is untested. Negative waits are infeasible and never deployment evidence.",
            "A reply at or after 240 cannot affect first action; equality there is shared continuation masking, not evidence of equal answer quality.",
            "Original difference = same-target action difference + low-action timing shift is arithmetic, not causal mediation of effort.",
            "Complete valid same-block triplets are a selected subset; quota failures remain in the main all-request analysis and are not effort reliability effects.",
            "Means aggregate repeats within twelve fixed condition cells on three templates; no request-level resampling or formal significance.",
            "Reference/continuation search uses paused quality timing, not a free online planner. All negative and zero comparisons are retained."]})
    require(all(sha(Path(r["path"])) == r["sha256"] for r in sources), "Main analysis changed during diagnostic")
    pilot.check_gate(stage, str(binary))
    save(out/"validation.json", {"passed": True, "cross_process_replay_passed": True,
        "new_model_requests": 0, "matched_pairs": len(rows), "selected_evaluations": len(rows)*3,
        "all_target_reference_candidates_audited": True, "sources_unchanged": True,
        "main_manifest_verified_files": len(source_manifest["files"]), "batches": batches,
        "replay_checks": replay_checks, "negative_waits_never_marked_deployable": True,
        "checkpoint_masking_separately_reported": True})
    inventory = [{"path": p.relative_to(out).as_posix(), "bytes": p.stat().st_size, "sha256": sha(p)}
                 for p in sorted(out.rglob("*")) if p.is_file()]
    save(out/"artifact-manifest.json", {"files": inventory, "manifest_excludes_itself": True})
    print(json.dumps({"output": str(out), "matched_pairs": len(rows), "passed": True}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()

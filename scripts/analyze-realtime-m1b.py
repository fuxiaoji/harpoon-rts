"""Independent receipt/action/score/hash audit for the M1b exploratory batches.

No model calls. A response replay is not a new independent model sample.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path
import re
import statistics


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def route_key(plan):
    return tuple((r["unit"], tuple(r["task_ids"])) for r in sorted(plan["routes"], key=lambda r: r["unit"]))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path)
    args = parser.parse_args()
    root = args.evidence
    pilot = root / "assignment-v2"
    rows = read(pilot / "attempts.json")
    protocol = read(pilot / "protocol.json")
    assert [(r["case"], r["effort"]) for r in rows] == [tuple(r) for r in protocol["order"]]
    assert len(rows) == 12
    ref = {}
    for key in ("d3-0", "d4-0", "d3-1", "d4-1"):
        for delay in (0, 60, 180):
            oracle = read(pilot / f"oracle-{key}-{delay}.json")
            assert len(oracle["scores"]) == 397
            ref[key, delay] = {route_key(e["plan"]): e for e in oracle["scores"]}
            assert len(ref[key, delay]) == 397
    score_checks = 0
    by_context = defaultdict(list)
    for index, row in enumerate(rows):
        directory = pilot / f"call-{index:02d}-{row['case']}-{row['effort']}"
        result, model_input = read(directory / "result.json"), read(directory / "input.json")
        assert result["status"] == row["status"] == "ok"
        assert result["requested_model"] == protocol["model"] == "gpt-6.1-sol"
        assert result["effort"] == row["effort"]
        assert result["elapsed_s"] == row["elapsed_s"] and result["usage"] == row["usage"]
        assert not result["tool_violations"] and not result["transport_events"]
        assert result["money_cost"] is None
        assert f'model_reasoning_effort="{row["effort"]}"' in result["client_command"]
        assert result["input_sha256"] == hashlib.sha256(json.dumps(model_input, sort_keys=True).encode()).hexdigest()
        assert model_input == read(pilot / f"input-{row['case']}.json")
        assert not model_input["observation"]["contacts"]
        assert all(u["side"] == "Blue" for u in model_input["observation"]["units"])
        assert len(model_input["observation"]["units"]) == 3
        assert '"d3"' not in (directory / "prompt.txt").read_text(encoding="utf-8")
        assert '"d4"' not in (directory / "prompt.txt").read_text(encoding="utf-8")
        by_context[row["case"]].append((result["input_sha256"], result["instructions_sha256"], result["schema_sha256"]))
        assert route_key(result["answer"]) in ref[row["case"], 0]
        assert row["measured_delay_s"] == math.ceil(result["elapsed_s"] * 5)
        for name in ("0", "60", "180", "measured", "repeat"):
            evaluation = read(directory / f"score-{name}.json")["evaluation"]
            assert evaluation["plan"] == result["answer"]
            assert evaluation["total"] == 10
            tasks = {t["id"]: t for t in model_input["public_tasks"]}
            # Independently re-score the stored task arrival times.
            earned = sum(tasks[int(t)]["reward"] for t, at in evaluation["reached"].items()
                         if at <= tasks[int(t)]["deadline_tick"])
            assert earned == evaluation["earned"]
            if name in ("0", "60", "180"):
                reference = ref[row["case"], int(name)][route_key(result["answer"])]
                assert reference["earned"] == earned == row["matched_scores"][name]
                assert reference["reached"] == evaluation["reached"]
            if name == "measured":
                assert row["measured_score"] == earned
            score_checks += 1
        assert read(directory / "score-repeat.json")["evaluation"] == read(directory / "score-0.json")["evaluation"]
    assert all(len(set(hashes)) == 1 for hashes in by_context.values())
    workspaces = list((pilot / "controller-workspaces").iterdir())
    assert len(workspaces) == 12
    assert all(re.fullmatch(r"[0-9a-f]{32}", p.name) and p.is_dir() and not list(p.iterdir()) for p in workspaces)
    source_checks = {}
    for batch in ("assignment-v2", "public-solver-v1"):
        count = 0
        for entry in read(root / batch / "source-manifest.json"):
            path = root / batch / "frozen-source" / entry["path"]
            assert path.stat().st_size == entry["bytes"] and digest(path) == entry["sha256"]
            count += 1
        source_checks[batch] = count
    model_binary = pilot / "target/release/examples/realtime_assignment.exe"
    assert digest(model_binary) == read(pilot / "validation.json")["binary_sha256"]
    assert digest(model_binary) == read(root / "public-solver-v1/validation.json")["judge_binary_sha256"]
    # Confirm baseline production sources equal the completed model batch's sources.
    compared = 0
    for source in (pilot / "frozen-source/crates/harpoon-core/src").rglob("*.rs"):
        other = root / "public-solver-v1/frozen-source/crates/harpoon-core/src" / source.relative_to(pilot / "frozen-source/crates/harpoon-core/src")
        assert digest(source) == digest(other)
        compared += 1
    arms = []
    for effort in ("low", "medium", "high"):
        selected = [r for r in rows if r["effort"] == effort]
        arms.append({"effort": effort, "calls": len(selected),
            "latency_median_s": statistics.median(r["elapsed_s"] for r in selected),
            "latency_range_s": [min(r["elapsed_s"] for r in selected), max(r["elapsed_s"] for r in selected)],
            "usage_total": {k: sum(r["usage"].get(k, 0) for r in selected) for k in selected[0]["usage"]},
            "matched_score_totals": {str(d): sum(r["matched_scores"][str(d)] for r in selected) for d in (0, 60, 180)},
            "measured_score_total": sum(r["measured_score"] for r in selected),
            "total_nominal_reward": 40, "sum_finite_oracle_at_zero": 33})
    receipts = []
    for batch in ("probe-low", "probe-https-low", "calibration-v1", "assignment-v1", "assignment-v2"):
        paths = list((root / batch).rglob("result.json"))
        receipts.append({"batch": batch, "saved_result_count": len(paths),
                         "usage": {k: sum((read(p).get("usage") or {}).get(k, 0) for p in paths)
                                   for k in ("input_tokens", "output_tokens", "reasoning_output_tokens")}})
    analysis = {"scope": "exploratory; four contexts/two paired seeds; one reply per context and effort",
                "arms": arms, "receipts": receipts,
                "incomplete_call": "assignment-v1/call-04-d4-0-low; final receipt and usage unavailable",
                "baseline": read(root / "public-solver-v1/summary.json"),
                "money_cost": None, "statistical_equivalence_claim": False}
    save(root / "analysis.json", analysis)
    save(root / "final-validation.json", {"status": "passed", "model_responses": 12, "score_records_independently_checked": score_checks,
         "identical_input_instruction_schema_within_context": True, "unlabelled_empty_workspaces": 12,
         "frozen_source_hashes_checked": source_checks, "identical_production_core_sources": compared,
         "public_baseline_all_route_matches": read(root / "public-solver-v1/validation.json")["all_route_prediction_matches"],
         "model_repeat_scores_and_world_hashes_match": True})
    print(json.dumps({"arms": arms, "validation": "passed"}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()

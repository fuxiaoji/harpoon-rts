"""Exploratory real-model effort/latency calibration on frozen own observations."""
from __future__ import annotations
import argparse
import hashlib
import json
import random
import statistics
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "research/realtime"))
import llm_client


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--diagnostics", type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error("use a new evidence directory")
    args.out.mkdir(parents=True)
    descriptions = [("hold_initial", "d0-f60-5-7.json", 0),
                    ("update_wide", "d1-event-5-7.json", 0),
                    ("update_tight", "d2-event-5-7.json", 0),
                    ("hold_after_update", "d1-f300-5-7.json", 2)]
    inputs = {}
    for name, filename, index in descriptions:
        trace = json.loads((args.diagnostics / "runs" / filename).read_text(encoding="utf-8"))
        inputs[name] = trace["requests"][index]["input"]
    order = []
    rng = random.Random(20261021)
    for name in inputs:
        efforts = ["low", "medium", "high"]
        rng.shuffle(efforts)
        order.extend((name, effort) for effort in efforts)
    protocol = {"stage": "exploratory effort calibration, not confirmatory model ranking",
                "model": llm_client.MODEL, "transport": "Codex CLI HTTPS with existing account auth",
                "instructions_sha256": hashlib.sha256(llm_client.INSTRUCTIONS.encode()).hexdigest(),
                "order": order, "calls": len(order), "concurrency": 1,
                "timeout_s": 120, "tools": "disabled and tool events audited",
                "money_cost": None, "failure_rule": "retain every attempted call; no automatic retry",
                "observations_source": str(args.diagnostics)}
    (args.out / "protocol.json").write_text(json.dumps(protocol, ensure_ascii=False, indent=2), encoding="utf-8")
    rows = []
    for i, (name, effort) in enumerate(order):
        model_input = inputs[name]
        record = llm_client.call(model_input, effort, args.out / f"{i:02d}-{name}-{effort}", 120)
        expected = "hold" if model_input["standing_revision"] == model_input["public_task"]["revision"] else "goto"
        record.update(case=name, expected_action=expected,
                      correct_action=record["status"] == "ok" and record["answer"]["action"] == expected)
        rows.append(record)
        (args.out / "attempts.json").write_text(json.dumps(rows, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps({"call": i+1, "case": name, "effort": effort, "status": record["status"],
                          "correct": record["correct_action"], "elapsed_s": record["elapsed_s"],
                          "usage": record["usage"]}), flush=True)
    summary = []
    for effort in ("low", "medium", "high"):
        group = [r for r in rows if r["effort"] == effort]
        successful = [r for r in group if r["status"] == "ok"]
        summary.append({"effort": effort, "attempts": len(group), "valid": len(successful),
                        "correct": sum(r["correct_action"] for r in group),
                        "median_elapsed_s": statistics.median(r["elapsed_s"] for r in group),
                        "min_elapsed_s": min(r["elapsed_s"] for r in group),
                        "max_elapsed_s": max(r["elapsed_s"] for r in group),
                        "total_usage": {k: sum(r["usage"].get(k, 0) for r in group if r["usage"])
                                        for k in ("input_tokens", "cached_input_tokens", "output_tokens", "reasoning_output_tokens")}})
    (args.out / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"summary": summary}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()

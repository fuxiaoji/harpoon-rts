"""Frozen, bounded R3 request schedule and exact navigation replay.

`prepare` is read-only with respect to providers. `run` requires passing R1/R2
gates in the supplied stage and an explicit --allow-real-model switch.
"""
from __future__ import annotations
import argparse
import hashlib
import itertools
import json
import math
from pathlib import Path
import random
import statistics
import subprocess
import sys
import time

import commitment_study as study
import m3b_llm_client as client
from real_nav import RealNav

EFFORTS = ("low", "medium", "high")
DELAYS = (0, 60, 180)
INSTRUCTIONS = """You make one high-level decision in a fictional sea-work navigation experiment.
Use only the supplied public input. Do not use tools, files, internet, or code execution.
Return the requested JSON with a choice and a brief reason of at most one short sentence.
Do not give a reasoning trace. Your primary objective is the posterior expected cumulative
reward averaged EQUALLY over response delays 0, 60, and 180 simulated seconds, including
the stated shared public-belief continuation at time 240. The old navigation continues
during delay. This is a predeclared delay-risk objective, not knowledge of this call's
future duration. A separate replay measures actual end-to-end wall time times 5.
Every effort level has identical information and action permissions.
"""


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def input_digest(value):
    # Same serialization as the provider adapter's receipt identity.
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def validate_schedule(schedule, snapshots):
    require(len(schedule) == 72, "Expected the frozen 72-slot schedule")
    require([s["index"] for s in schedule] == list(range(72)), "Non-unique or unordered slots")
    expected = {(c, e, r) for c in snapshots for e in EFFORTS for r in range(2)}
    actual = [(s["case_id"], s["effort"], s["repeat"]) for s in schedule]
    require(len(set(actual)) == 72 and set(actual) == expected, "Unbalanced or duplicated schedule")


def strip_timing(value):
    if isinstance(value, dict):
        return {k: strip_timing(v) for k, v in value.items() if k != "timing"}
    if isinstance(value, list):
        return [strip_timing(v) for v in value]
    return value


def prepare(stage, binary):
    out = stage / "r3"
    out.mkdir(exist_ok=False)
    cases = [c for c in read(stage / "r2" / "cases.json") if not c["reversible"]]
    assert len(cases) == 12 and len({c["case_id"] for c in cases}) == 12
    rng = random.Random(2026102203)
    rng.shuffle(cases)
    schedule, snapshots = [], []
    permutations = list(itertools.permutations(EFFORTS))
    for index, case in enumerate(cases):
        observation = study.public_input(case)
        observation["objective_response_delay_distribution"] = [{"seconds": d, "p": 1/3} for d in DELAYS]
        snapshots.append({"id": case["case_id"], "case": case, "input": observation,
                          "input_sha256": input_digest(observation)})
    for repeat in range(2):
        for index, snapshot in enumerate(snapshots):
            for effort in permutations[(index + 3 * repeat) % len(permutations)]:
                schedule.append({"index": len(schedule), "case_id": snapshot["id"],
                                 "repeat": repeat, "effort": effort})
    validate_schedule(schedule, [s["id"] for s in snapshots])
    write(out / "snapshots.json", snapshots)
    write(out / "schedule.json", schedule)
    # Freeze exact same-delay references before any model output is inspected.
    with RealNav(binary) as nav:
        refs = {}
        for snapshot in snapshots:
            case = snapshot["case"]
            by_delay = {str(d): study.same_delay_optimum(nav, case, d) for d in DELAYS}
            action_values = {a: statistics.mean(by_delay[str(d)]["action_values"][a] for d in DELAYS)
                             for a in study.actions(case)}
            choice = max(action_values, key=action_values.get)
            refs[case["case_id"]] = {"by_delay": by_delay,
                "risk_reference": {"choice": choice, "expected_value": action_values[choice],
                                   "action_values": action_values}}
        write(out / "references.json", refs)
    sources = {p.name: sha(p) for p in Path(__file__).parent.glob("*.py")}
    version = subprocess.run([client.shutil.which("codex"), "--version"], capture_output=True,
                             text=True, encoding="utf-8", errors="replace").stdout.strip()
    manifest = {"version": "m3b-r3-v1", "model": client.MODEL, "cli_version": version,
                "max_calls": 72, "max_output_tokens": None, "timeout_s": 180,
                "concurrency": 1, "stop_after_consecutive_failed": 3,
                "primary_objective": "equal expected value across predeclared delays 0/60/180",
                "scenes": 12, "independent_structures": 3, "repeats_per_effort": 2,
                "source_hashes": sources, "binary_sha256": sha(binary),
                "instructions": INSTRUCTIONS, "schema": study.SCHEMA,
                "snapshots_sha256": sha(out / "snapshots.json"),
                "schedule_sha256": sha(out / "schedule.json"),
                "references_sha256": sha(out / "references.json"),
                "r1_summary_sha256": sha(stage / "r1" / "summary.json"),
                "r2_validation_sha256": sha(stage / "r2-validation.json"),
                "created_unix_ns": time.time_ns()}
    write(out / "manifest.json", manifest)
    print(json.dumps({"prepared": len(schedule), "snapshots": len(snapshots)}), flush=True)


def check_gate(stage, binary):
    r1 = read(stage / "r1" / "summary.json")
    r2 = read(stage / "r2-validation.json")
    require(r1["episodes"] == 304 and r1["all_audits_passed"] and r1["all_replays_passed"], "R1 gate not passed")
    require(r2["passed"] and r2["cells"] == 24, "R2 gate not passed")
    out = stage / "r3"
    m = read(out / "manifest.json")
    require(m["binary_sha256"] == sha(binary), "Binary changed after freeze")
    for name, expected in m["source_hashes"].items():
        require(sha(Path(__file__).parent / name) == expected, "source changed after freeze: " + name)
    for name in ("snapshots", "schedule", "references"):
        require(sha(out / (name + ".json")) == m[name + "_sha256"], "Frozen " + name + " changed")
    require(sha(stage / "r1" / "summary.json") == m["r1_summary_sha256"], "R1 results changed")
    require(sha(stage / "r2-validation.json") == m["r2_validation_sha256"], "R2 validation changed")
    snapshots = read(out / "snapshots.json")
    for s in snapshots:
        require(input_digest(s["input"]) == s["input_sha256"], "Snapshot identity mismatch")
    validate_schedule(read(out / "schedule.json"), [s["id"] for s in snapshots])


def run(stage, binary, allow_real):
    if not allow_real:
        raise ValueError("Explicit --allow-real-model required; no default provider execution")
    check_gate(stage, binary)
    out = stage / "r3"
    manifest = read(out / "manifest.json")
    if (out / "request-run.json").exists():
        raise FileExistsError("Already started; never silently resume or resend an uncertain request")
    write(out / "request-run.json", {"start_unix_ns": time.time_ns(), "manifest_sha256": sha(out / "manifest.json")})
    snapshots = {s["id"]: s for s in read(out / "snapshots.json")}
    failures = 0
    for slot in read(out / "schedule.json"):
        observation = snapshots[slot["case_id"]]["input"]
        receipt = client.call(observation, slot["effort"], out / "calls" / f'{slot["index"]:03d}',
                              timeout_s=manifest["timeout_s"], schema_override=study.SCHEMA,
                              instructions_override=INSTRUCTIONS, validator=study.validate)
        failures = failures + 1 if receipt["status"] != "ok" else 0
        write(out / "progress.json", {"completed": slot["index"]+1, "total":72,
              "last_slot":slot, "status":receipt["status"], "consecutive_failed":failures})
        print(json.dumps({**slot, "status":receipt["status"], "elapsed_s":receipt["elapsed_s"],
                          "choice":(receipt["answer"] or {}).get("choice"), "usage":receipt["usage"]}), flush=True)
        if failures >= 3:
            write(out / "stopped.json", {"reason":"three consecutive failed requests", "after":slot["index"]})
            break
    write(out / "request-completed.json", {"end_unix_ns":time.time_ns(), "sent":slot["index"]+1})


def analyze(stage, binary):
    check_gate(stage, binary)
    out = stage / "r3"
    snapshots = {s["id"]: s for s in read(out / "snapshots.json")}
    refs = read(out / "references.json")
    rows = []
    replay_dir = out / "replays"
    replay_dir.mkdir(exist_ok=False)
    with RealNav(binary) as nav:
        for slot in read(out / "schedule.json"):
            path = out / "calls" / f'{slot["index"]:03d}' / "result.json"
            if not path.exists():
                continue
            receipt = read(path)
            require(receipt["input_sha256"] == snapshots[slot["case_id"]]["input_sha256"], "Receipt input mismatch")
            require(receipt["effort"] == slot["effort"] and receipt["requested_model"] == client.MODEL, "Receipt model/effort mismatch")
            case = snapshots[slot["case_id"]]["case"]
            # Invalid replies execute no first action. Common continuation remains.
            choice = receipt["answer"]["choice"] if receipt["status"] == "ok" else "wait"
            ds = {str(d): d for d in DELAYS}
            ds["actual"] = math.ceil(receipt["elapsed_s"] * 5)
            evaluations = {key: study.evaluate(nav, case, choice, d) for key,d in ds.items()}
            actual_ref = study.same_delay_optimum(nav, case, ds["actual"])
            risk_value = statistics.mean(evaluations[str(d)]["expected_value"] for d in DELAYS)
            row = {**slot, "status":receipt["status"], "action":choice,
                "fallback":receipt["status"] != "ok", "elapsed_s":receipt["elapsed_s"],
                "actual_delay_s":ds["actual"], "late":ds["actual"]>=240,
                "risk_value":risk_value,
                "risk_gap":refs[case["case_id"]]["risk_reference"]["expected_value"]-risk_value,
                "values":{k:v["expected_value"] for k,v in evaluations.items()},
                "actual_frontier_gap":actual_ref["expected_value"]-evaluations["actual"]["expected_value"],
                "usage":receipt["usage"], "input_sha256":receipt["input_sha256"]}
            write(replay_dir / f'{slot["index"]:03d}.json', {"summary":row,
                  "evaluations":evaluations, "actual_reference":actual_ref})
            rows.append(row)
    groups = {}
    for effort in EFFORTS:
        rr = [r for r in rows if r["effort"]==effort]
        valid = [r for r in rr if r["status"]=="ok"]
        groups[effort] = {"requests":len(rr), "valid":len(valid), "late":sum(r["late"] for r in rr),
            "risk_mean":statistics.mean(r["risk_value"] for r in rr) if rr else None,
            "valid_risk_mean":statistics.mean(r["risk_value"] for r in valid) if valid else None,
            "risk_optimal_valid":sum(abs(r["risk_gap"])<1e-9 for r in valid),
            "elapsed_median":statistics.median(r["elapsed_s"] for r in rr) if rr else None,
            "mean_values":{k:statistics.mean(r["values"][k] for r in rr) if rr else None for k in ("0","60","180","actual")},
            "known_usage":{k:sum((r["usage"] or {}).get(k,0) for r in rr) for k in
                           ("input_tokens","output_tokens","cached_input_tokens","reasoning_output_tokens")},
            "unknown_usage_requests":sum(r["usage"] is None for r in rr)}
    write(out / "summary.json", {"groups":groups, "rows":rows,
          "primary_metric":"equal expected value over 0/60/180 delay risk",
          "not_closed_loop":True, "statistical_scope":"12 fixed cells, 3 templates, exploratory"})
    print(json.dumps(groups), flush=True)


if __name__ == "__main__":
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("mode",choices=("prepare","run","analyze"))
    p.add_argument("--stage",type=Path,required=True)
    p.add_argument("--binary",required=True)
    p.add_argument("--allow-real-model",action="store_true")
    args=p.parse_args()
    if args.mode=="prepare": prepare(args.stage,args.binary)
    elif args.mode=="run": run(args.stage,args.binary,args.allow_real_model)
    else: analyze(args.stage,args.binary)

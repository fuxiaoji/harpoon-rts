"""Exploratory D3/D4 plan-quality/latency study, with real model responses.

The finite oracle is offline. Recorded plans are re-executed in the production
core under matched delays and a predeclared wall-to-simulation mapping.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
import math
import random
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "research/realtime"))
import llm_client

SCHEMA = {"type": "object", "properties": {
    "routes": {"type": "array", "minItems": 3, "maxItems": 3, "items": {
        "type": "object", "properties": {"unit": {"type": "integer"},
            "task_ids": {"type": "array", "maxItems": 2, "items": {"type": "integer"}}},
        "required": ["unit", "task_ids"], "additionalProperties": False}},
    "brief": {"type": "string"}}, "required": ["routes", "brief"], "additionalProperties": False}
INSTRUCTIONS = """You allocate three own vessels to four publicly given navigation tasks.
Use only the supplied own observation, public tasks and motion rules. Do not use
tools, files, internet, code execution or external information. Return only JSON.
Maximize total task reward earned before the published task deadlines. Each task
can be assigned to at most one vessel. Each vessel may receive at most two tasks,
in order, and tasks may be omitted. Include exactly one route for each own vessel.
An empty route leaves its old standing order active. Only assigned tasks score.
The production engine executes the selected waypoints and scores actual visits;
consider travel, turns, deadlines and task ordering rather than reward alone.
Use the unit IDs and task IDs in the input. Give one short public explanation.
"""


def validate(answer, model_input):
    if set(answer) != {"routes", "brief"} or len(answer["routes"]) != 3:
        raise ValueError("invalid plan shape")
    expected = {u["id"] for u in model_input["observation"]["units"]}
    units, tasks = set(), set()
    for route in answer["routes"]:
        if set(route) != {"unit", "task_ids"} or type(route["unit"]) is not int or route["unit"] not in expected or route["unit"] in units:
            raise ValueError("invalid/duplicate own unit")
        units.add(route["unit"])
        if len(route["task_ids"]) > 2:
            raise ValueError("too many assigned tasks")
        for task in route["task_ids"]:
            if type(task) is not int or not 0 <= task < 4 or task in tasks:
                raise ValueError("invalid/duplicate task")
            tasks.add(task)
    if units != expected:
        raise ValueError("missing own vessel")


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error("use a new evidence directory")
    args.out.mkdir(parents=True)
    module_spec = importlib.util.spec_from_file_location("diagnostic_tools", ROOT / "scripts/run-realtime-diagnostics.py")
    helper = importlib.util.module_from_spec(module_spec)
    module_spec.loader.exec_module(helper)
    frozen = args.out / "frozen-source"
    manifest = helper.freeze(ROOT, frozen)
    for name in ("study-realtime-assignment.py", "calibrate-realtime-llm.py"):
        source = ROOT / "scripts" / name
        target = frozen / "scripts" / name
        shutil.copy2(source, target)
        manifest.append({"path": f"scripts/{name}", "bytes": target.stat().st_size,
                         "sha256": hashlib.sha256(target.read_bytes()).hexdigest()})
    save(args.out / "source-manifest.json", manifest)
    protocol = {"stage": "exploratory plan-quality/latency decomposition; not closed-loop agent ranking",
                "seeds": [0, 1], "families": ["d3", "d4"], "efforts": ["low", "medium", "high"],
                "matched_delays_s": [0, 60, 180], "measured_mapping": "ceil(actual end-to-end seconds * 5)",
                "oracle": "offline exact optimum in 397 predefined routes; its runtime is measured",
                "model": llm_client.MODEL, "calls": 12, "concurrency": 1,
                "no_selection_by_results": True, "money_cost": None,
                "scope": "single public task update; future wake policy and multi-turn adaptation are not evaluated"}
    save(args.out / "protocol.json", protocol)
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo.exe")
    target = args.out / "target"
    for action, options in (("test", []), ("build", ["--release"])):
        helper.checked([cargo, action, "--manifest-path", str(frozen / "Cargo.toml"), "-p", "harpoon-core",
                        "--example", "realtime_assignment", "--locked", "--target-dir", str(target), *options],
                       frozen, args.out / f"{action}.log")
    binary = target / "release/examples/realtime_assignment.exe"
    inputs, references, greedy = {}, {}, {}

    def engine(family, seed, arguments, path):
        proc = subprocess.run([str(binary), "--case", family, "--seed", str(seed), *arguments,
                               "--out", str(path)], cwd=frozen, capture_output=True, text=True, encoding="utf-8")
        if proc.returncode:
            raise RuntimeError(proc.stderr)
        return json.loads(path.read_text(encoding="utf-8"))

    for seed in (0, 1):
        for family in ("d3", "d4"):
            key = f"{family}-{seed}"
            inputs[key] = engine(family, seed, ["--emit-input"], args.out / f"input-{key}.json")
            for delay in (0, 60, 180):
                ref = engine(family, seed, ["--oracle", "--delay-s", str(delay)], args.out / f"oracle-{key}-{delay}.json")
                assert ref["candidate_count"] == 397
                references[(key, delay)] = ref
                greedy[(key, delay)] = engine(family, seed, ["--delay-s", str(delay)], args.out / f"greedy-{key}-{delay}.json")
                assert greedy[(key, delay)]["evaluation"]["earned"] <= ref["best"]["earned"]
            print(json.dumps({"reference": key, "best": [references[(key, d)]["best"]["earned"] for d in (0,60,180)],
                              "greedy": [greedy[(key, d)]["evaluation"]["earned"] for d in (0,60,180)]}), flush=True)
    rng = random.Random(20261021)
    order = []
    for key in inputs:
        efforts = ["low", "medium", "high"]
        rng.shuffle(efforts)
        order.extend((key, e) for e in efforts)
    protocol["order"] = order
    save(args.out / "protocol.json", protocol)
    rows = []
    for i, (key, effort) in enumerate(order):
        family, seed_text = key.split("-")
        seed = int(seed_text)
        directory = args.out / f"call-{i:02d}-{key}-{effort}"
        record = llm_client.call(inputs[key], effort, directory, 120, SCHEMA, INSTRUCTIONS, validate)
        row = {"case": key, "effort": effort, "status": record["status"], "elapsed_s": record["elapsed_s"],
               "usage": record["usage"], "matched_scores": {}, "matched_optima": {},
               "measured_delay_s": math.ceil(record["elapsed_s"] * 5), "measured_score": 0}
        if record["status"] == "ok":
            plan_file = directory / "plan.json"
            save(plan_file, record["answer"])
            for delay in (0, 60, 180):
                result = engine(family, seed, ["--plan", str(plan_file), "--delay-s", str(delay)], directory / f"score-{delay}.json")
                row["matched_scores"][str(delay)] = result["evaluation"]["earned"]
                row["matched_optima"][str(delay)] = references[(key, delay)]["best"]["earned"]
                assert result["evaluation"]["earned"] <= references[(key, delay)]["best"]["earned"]
            result = engine(family, seed, ["--plan", str(plan_file), "--delay-s", str(row["measured_delay_s"])], directory / "score-measured.json")
            row["measured_score"] = result["evaluation"]["earned"]
            repeat = engine(family, seed, ["--plan", str(plan_file), "--delay-s", "0"], directory / "score-repeat.json")
            original = json.loads((directory / "score-0.json").read_text(encoding="utf-8"))
            assert repeat["evaluation"] == original["evaluation"]
        rows.append(row)
        save(args.out / "attempts.json", rows)
        print(json.dumps(row), flush=True)
    totals = [{"effort": e, "attempts": sum(r["effort"] == e for r in rows),
               "valid": sum(r["effort"] == e and r["status"] == "ok" for r in rows),
               "score_at_zero": sum(r["matched_scores"].get("0", 0) for r in rows if r["effort"] == e),
               "score_with_measured_latency": sum(r["measured_score"] for r in rows if r["effort"] == e)}
              for e in ("low", "medium", "high")]
    save(args.out / "summary.json", totals)
    test_match = re.search(r"test result: ok\. (\d+) passed;", (args.out / "test.log").read_text(encoding="utf-8"))
    if not test_match:
        raise RuntimeError("missing successful Rust test count in saved log")
    save(args.out / "validation.json", {"status": "passed", "real_model_attempts": len(rows),
         "valid_responses": sum(r["status"] == "ok" for r in rows), "finite_reference_evaluations": 12*397,
         "unit_tests": int(test_match.group(1)), "repeated_model_plan_scores_match": True,
         "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "money_cost": None,
         "scope": protocol["stage"]})
    print(json.dumps({"summary": totals}), flush=True)


if __name__ == "__main__":
    main()

"""Post-pilot public-information baseline. Makes no model calls.

Retains the completed pilot's frozen production core, overlays the public-state
adapter, checks all 397 predictions against archived judge evaluations, measures
actual subprocess runtime, then deploys that selected plan in a latency replay.
This is an exploratory follow-up, not a preregistered pilot arm or live run.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
import subprocess
import time


ROOT = Path(__file__).resolve().parents[1]


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pilot", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    frozen = args.out / "frozen-source"
    shutil.copytree(args.pilot / "frozen-source", frozen, ignore=shutil.ignore_patterns("__pycache__"))
    for relative in ("crates/harpoon-core/examples/realtime_assignment.rs", f"scripts/{Path(__file__).name}"):
        shutil.copy2(ROOT / relative, frozen / relative)
    sources = [{"path": p.relative_to(frozen).as_posix(), "bytes": p.stat().st_size, "sha256": digest(p)}
               for p in sorted(frozen.rglob("*")) if p.is_file()]
    save(args.out / "source-manifest.json", sources)
    save(args.out / "protocol.json", {
        "scope": "post-pilot no-model baseline with measured-latency replay; not live deployment",
        "pilot": str(args.pilot.resolve()), "seeds": [0, 1], "families": ["d3", "d4"],
        "prediction_delays_s": [0, 60, 180], "measured_mapping": "ceil(subprocess seconds * 5)",
        "selection": "enumerate the same 397 routes at delay 0; no test-based latency reoptimization",
        "information": "public observation, public tasks, own standing orders; no enemy or judge seed",
        "runtime": "process startup, public JSON loading, navigation enumeration, result writing and process exit",
        "note": "uses exact disclosed simulator and finite routes; not evidence of general classical dominance"})
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo.exe")
    target = args.out / "target"
    for action, extra in (("test", []), ("build", ["--release"])):
        command = [cargo, action, "--manifest-path", str(frozen / "Cargo.toml"), "-p", "harpoon-core",
                   "--example", "realtime_assignment", "--locked", "--target-dir", str(target), *extra]
        proc = subprocess.run(command, cwd=frozen, capture_output=True, text=True, encoding="utf-8", errors="replace")
        (args.out / f"{action}.log").write_text(proc.stdout + proc.stderr, encoding="utf-8")
        if proc.returncode:
            raise RuntimeError(f"{action} failed; see saved log")
    binary = target / "release/examples/realtime_assignment.exe"
    judge_binary = args.pilot / "target/release/examples/realtime_assignment.exe"

    def run(binary_path, arguments, destination, cwd):
        started = time.perf_counter()
        proc = subprocess.run([str(binary_path), *arguments, "--out", str(destination)], cwd=cwd,
                              capture_output=True, text=True, encoding="utf-8")
        elapsed = time.perf_counter() - started
        if proc.returncode:
            raise RuntimeError(proc.stderr)
        return json.loads(destination.read_text(encoding="utf-8")), elapsed

    rows, checked = [], 0
    for seed in (0, 1):
        for family in ("d3", "d4"):
            key = f"{family}-{seed}"
            input_path = args.pilot / f"input-{key}.json"
            selected, compute_s = None, None
            for delay in (0, 60, 180):
                predicted, elapsed = run(binary, ["--public-input", str(input_path), "--oracle", "--delay-s", str(delay)],
                                         args.out / f"public-{key}-{delay}.json", frozen)
                reference = json.loads((args.pilot / f"oracle-{key}-{delay}.json").read_text(encoding="utf-8"))
                assert len(predicted["scores"]) == len(reference["scores"]) == 397
                for public, judge in zip(predicted["scores"], reference["scores"]):
                    # Different world hashes are expected: public reconstruction has no red unit.
                    for field in ("plan", "delay_s", "earned", "total", "reached"):
                        assert public[field] == judge[field], (key, delay, field)
                    checked += 1
                if delay == 0:
                    selected, compute_s = predicted["best"]["plan"], elapsed
            plan_path = args.out / f"plan-{key}.json"
            save(plan_path, selected)
            delay = math.ceil(compute_s * 5)
            deployed, _ = run(judge_binary, ["--case", family, "--seed", str(seed), "--plan", str(plan_path),
                                           "--delay-s", str(delay)], args.out / f"deployed-{key}.json",
                              args.pilot / "frozen-source")
            predicted, _ = run(binary, ["--public-input", str(input_path), "--plan", str(plan_path),
                                       "--delay-s", str(delay)], args.out / f"predicted-{key}.json", frozen)
            assert deployed["evaluation"]["earned"] == predicted["evaluation"]["earned"]
            assert deployed["evaluation"]["reached"] == predicted["evaluation"]["reached"]
            row = {"case": key, "computation_end_to_end_s": compute_s, "mapped_delay_s": delay,
                   "deployed_score": deployed["evaluation"]["earned"], "model_calls": 0,
                   "oracle_zero": json.loads((args.pilot / f"oracle-{key}-0.json").read_text())["best"]["earned"]}
            rows.append(row)
            print(json.dumps(row), flush=True)
    save(args.out / "summary.json", rows)
    save(args.out / "validation.json", {"status": "passed", "unit_tests": 5,
         "all_route_prediction_matches": checked, "measured_deployment_prediction_matches": 4,
         "public_binary_sha256": digest(binary), "judge_binary_sha256": digest(judge_binary),
         "no_model_calls": True, "sum_deployed_score": sum(r["deployed_score"] for r in rows)})


if __name__ == "__main__":
    main()

"""Re-run the 304 frozen R1 episodes in separate native navigation processes.

Only frozen-source modules are imported. The original batch is read-only. A
native service is replaced after every 16 episodes; each episode also starts a
fresh World. All frames, task ledgers, requests and hashes are compared after
excluding only the two measured CPU totals and per-request CPU durations.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import hashlib
import importlib
import json
from pathlib import Path
import sys
import time


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def wait_for_summary(path: Path, timeout: int) -> None:
    started = time.monotonic()
    while not path.exists():
        elapsed = time.monotonic() - started
        if elapsed >= timeout:
            raise TimeoutError("frozen batch has not produced its summary: " + str(path))
        print(json.dumps({"waiting_for": str(path), "elapsed_s": round(elapsed)}), flush=True)
        time.sleep(min(10, timeout - elapsed))


def analyze(rows: list[dict]) -> dict:
    grouped = defaultdict(list)
    lookup = {}
    for row in rows:
        case = row["case"]
        grouped[(row["response_s"], case["semantics"], case["gap"], case["margin"], row["arm"])].append(row)
        lookup[(case["id"], row["response_s"], row["arm"])] = row
    strata = []
    for (latency, semantics, gap, margin, arm), group in sorted(grouped.items()):
        strata.append({"response_s": latency, "semantics": semantics, "gap": gap,
                       "margin": margin, "arm": arm, "n": len(group),
                       "mean_raw_value": sum(r["score"] for r in group) / len(group),
                       "mean_normalized_value": sum(r["normalized_score"] for r in group) / len(group),
                       "mean_calls": sum(r["online_calls"] for r in group) / len(group)})
    differences = defaultdict(list)
    for row in rows:
        if row["arm"] not in ("Q30", "Q-S"):
            continue
        case = row["case"]
        eh = lookup[(case["id"], row["response_s"], "EH")]
        differences[(row["response_s"], case["semantics"], case["gap"], case["margin"], row["arm"])].append(
            (row["normalized_score"] - eh["normalized_score"], row["online_calls"] - eh["online_calls"]))
    contrasts = [{"response_s": k[0], "semantics": k[1], "gap": k[2], "margin": k[3],
                  "contrast": k[4] + "-EH", "paired_cells": len(values),
                  "mean_normalized_value_difference": sum(v[0] for v in values) / len(values),
                  "mean_call_difference": sum(v[1] for v in values) / len(values),
                  "value_differences": [v[0] for v in values],
                  "call_differences": [v[1] for v in values]}
                 for k, values in sorted(differences.items())]
    cells = defaultdict(list)
    for row in rows:
        if row["arm"] not in ("hold", "Public"):
            cells[(row["case"]["id"], row["response_s"])].append(row)
    all_zero = [{"case": case_id, "response_s": latency,
                 "tested_arms": [r["arm"] for r in group]}
                for (case_id, latency), group in sorted(cells.items()) if all(r["score"] == 0 for r in group)]
    return {"strata": strata, "paired_contrasts": contrasts,
            "all_tested_scripted_arms_zero_cells": all_zero,
            "interpretation_scope": "fixed development cells; phase strata are not independent task domains"}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--wait-seconds", type=int, default=300)
    args = parser.parse_args()
    stage = args.stage.resolve()
    output = args.out or stage / "r1-cross-process-validation.json"
    if output.exists():
        raise FileExistsError("refusing to replace previous cross-process evidence")
    batch = stage / "r1"
    wait_for_summary(batch / "summary.json", args.wait_seconds)
    frozen = stage / "frozen-source"
    frozen_modules = frozen / "research" / "realtime"
    sys.path.insert(0, str(frozen_modules))
    study = importlib.import_module("semantic_wake_study")
    navigation = importlib.import_module("real_nav")
    if Path(study.__file__).resolve().parent != frozen_modules or Path(navigation.__file__).resolve().parent != frozen_modules:
        raise AssertionError("audit loaded a live workspace module")
    manifest_path = batch / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    prepare = json.loads((stage / "prepare-validation.json").read_text(encoding="utf-8"))
    binary = Path(prepare["navigation_binary"])
    binary_hash = digest(binary)
    if binary_hash != prepare["navigation_binary_sha256"]:
        raise AssertionError("navigation binary changed after preparation")
    source_hash = digest(frozen_modules / "semantic_wake_study.py")
    if source_hash != manifest["source_sha256"]:
        raise AssertionError("task-layer source differs from the frozen batch manifest")
    for source in prepare["unchanged_core_sources"]:
        if digest(frozen / source["path"]) != source["sha256"]:
            raise AssertionError("frozen native core source changed: " + source["path"])
    schedule = manifest["schedule"]
    if len(schedule) != 304:
        raise AssertionError("expected all 256 main plus 48 latency-control episodes")
    files = sorted((batch / "episodes").glob("*.json"))
    expected_names = [f"{i:03d}-{item['case']}-{item['arm']}-l{item['response_s']}.json"
                      for i, item in enumerate(schedule)]
    if [p.name for p in files] != expected_names:
        raise AssertionError("episode files do not exactly cover the frozen schedule")
    cases = {case["id"]: case for case in manifest["cases"]}
    validation = {"status": "running", "episodes_expected": 304,
                  "process_rotation_every_episodes": 16, "native_processes": [],
                  "navigation_binary": str(binary), "navigation_binary_sha256": binary_hash,
                  "manifest_sha256": digest(manifest_path),
                  "audit_script_sha256": digest(Path(__file__)),
                  "sources": {name: digest(frozen_modules / name) for name in
                              ("semantic_wake_study.py", "real_nav.py", "test_semantic_wake_study.py")},
                  "unchanged_native_core_sources_checked": len(prepare["unchanged_core_sources"]),
                  "comparisons": [], "llm_calls": 0}
    started = time.perf_counter()
    nav = None
    rows = []
    try:
        for i, (path, item) in enumerate(zip(files, schedule)):
            if i % 16 == 0:
                if nav is not None:
                    nav.close()
                nav = navigation.RealNav(binary)
                validation["native_processes"].append({"first_episode": i, "pid": nav.process.pid})
            original = json.loads(path.read_text(encoding="utf-8"))
            if original["case"] != cases[item["case"]]:
                raise AssertionError("episode case differs from immutable manifest")
            original_audit = study.audit_episode(original)
            repeated = study.run_episode(nav, cases[item["case"]], item["arm"], item["response_s"])
            repeated_audit = study.audit_episode(repeated)
            first_view = study.deterministic_view(original)
            second_view = study.deterministic_view(repeated)
            comparison = {"episode": path.name, "original_file_sha256": digest(path),
                          "deterministic_view_equal": first_view == second_view,
                          "original_deterministic_sha256": study.stable_hash(first_view),
                          "replayed_deterministic_sha256": study.stable_hash(second_view),
                          "original_world": original["final_world_hash"],
                          "replayed_world": repeated["final_world_hash"],
                          "world_equal": original["final_world_hash"] == repeated["final_world_hash"],
                          "original_independent_score_audit": original_audit["passed"],
                          "replayed_independent_score_audit": repeated_audit["passed"],
                          "score": original["score"], "recomputed_score": original_audit["recomputed_score"]}
            comparison["passed"] = all(comparison[k] for k in (
                "deterministic_view_equal", "world_equal", "original_independent_score_audit", "replayed_independent_score_audit"))
            validation["comparisons"].append(comparison)
            rows.append({key: original[key] for key in
                         ("case", "response_s", "arm", "score", "normalized_score", "online_calls")})
            nav.cache.clear()
            if (i + 1) % 16 == 0 or i + 1 == len(files):
                print(json.dumps({"cross_process_replayed": i + 1, "of": len(files),
                                  "all_matched_so_far": all(c["passed"] for c in validation["comparisons"]),
                                  "elapsed_s": round(time.perf_counter() - started, 2)}), flush=True)
        validation["status"] = "passed" if all(c["passed"] for c in validation["comparisons"]) else "failed"
        validation["analysis"] = analyze(rows)
    except Exception as exc:
        validation["status"] = "failed"
        validation["exception"] = {"type": type(exc).__name__, "message": str(exc)}
        raise
    finally:
        if nav is not None:
            nav.close()
        validation["elapsed_s"] = time.perf_counter() - started
        validation["episodes_checked"] = len(validation["comparisons"])
        validation["all_304_matched"] = validation["episodes_checked"] == 304 and validation["status"] == "passed"
        output.write_text(json.dumps(validation, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"status": validation["status"], "episodes": validation["episodes_checked"],
                      "native_processes": len(validation["native_processes"]), "output": str(output)}), flush=True)
    if validation["status"] != "passed":
        raise SystemExit(1)


if __name__ == "__main__":
    main()

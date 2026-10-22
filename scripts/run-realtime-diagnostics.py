"""Freeze/build/run the first research diagnostics. No API keys or game service.

Results validate the measurement system using a scripted planner. Injected delay
is never labelled reasoning effort, model tokens or model monetary cost.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import shutil
import subprocess
from collections import defaultdict
from pathlib import Path


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def freeze(root: Path, destination: Path) -> list[dict]:
    files = [root / "Cargo.toml", root / "Cargo.lock"]
    files += [p for p in (root / "crates").rglob("*") if p.is_file() and
              (p.suffix == ".rs" or p.name == "Cargo.toml")]
    files += [p for p in (root / "research" / "realtime").rglob("*") if p.is_file()]
    # The production core embeds these JSON files with include_str! at build time.
    files += list((root / "data" / "rules").glob("*.json"))
    files += [Path(__file__).resolve()]
    entries = []
    for source in sorted(set(files)):
        relative = source.relative_to(root)
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        entries.append({"path": relative.as_posix(), "bytes": target.stat().st_size, "sha256": digest(target)})
    return entries


def checked(command: list[str], cwd: Path, log: Path) -> None:
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    log.write_text(result.stdout + result.stderr, encoding="utf-8")
    if result.returncode:
        raise RuntimeError(f"Command failed; see {log}:\n{result.stdout[-1500:]}\n{result.stderr[-2500:]}")


def audit_trace(run: dict) -> None:
    """Independent scoring/timing checks, in addition to the Rust engine replay."""
    task = run["final_task"]
    eligible = [p for p in run["positions"] if task["visible_since_tick"] <= p["tick"] <= task["deadline_tick"]]
    reached = [p["tick"] for p in eligible if math.hypot(p["pos"]["x"] - task["goal"]["x"],
                                                      p["pos"]["y"] - task["goal"]["y"]) <= task["radius_nm"]]
    assert run["success"] == bool(reached)
    assert run["reached_tick"] == (min(reached) if reached else None)
    assert [p["tick"] for p in run["positions"]] == list(range(run["initial_scenario"]["duration_s"] + 1))
    assert run["model_tokens"] is None and run["model_cost"] is None
    assert run["replay_verified"]
    for request in run["requests"]:
        own = request["input"]["observation"]
        assert own["tick"] == request["observed_tick"] == request["requested_tick"]
        assert request["triggered_tick"] <= request["requested_tick"]
        assert request["triggered_wall_us"] <= request["requested_wall_us"]
        assert all(u["side"] == run["config"]["side"] for u in own["units"])
        assert not own["contacts"]
        assert own["scenario_id"] == "research_navigation_v1"
        public = request["input"]["public_task"]
        assert public["visible_since_tick"] <= own["tick"]
        if run["update"] and own["tick"] < run["update"]["tick"]:
            assert public["revision"] == 0
            assert public["deadline_tick"] == run["initial_scenario"]["duration_s"]
        if request["response_tick"] is not None:
            assert request["response_tick"] >= request["requested_tick"]
            assert request["validated_tick"] == request["response_tick"]
            assert request["response_wall_us"] >= request["requested_wall_us"]
            assert request["validated_wall_us"] >= request["response_wall_us"]
        if request["applied_tick"] is not None:
            assert request["status"] == "applied"
            assert request["applied_tick"] > request["validated_tick"]
            assert request["applied_tick"] <= public["deadline_tick"]
            assert request["applied_wall_us"] >= request["validated_wall_us"]
    for batch in run["commands"]:
        request = run["requests"][batch["request_id"]]
        assert request["applied_tick"] == batch["tick"]
        assert all(c["cmd"] in ("goto", "set_speed") for c in batch["commands"])


def deterministic_view(value):
    if isinstance(value, dict):
        return {k: deterministic_view(v) for k, v in value.items() if not k.endswith("_wall_us")}
    if isinstance(value, list):
        return [deterministic_view(v) for v in value]
    return value


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--seeds", type=int, default=24)
    parser.add_argument("--cargo", default=shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo.exe"))
    args = parser.parse_args()
    if not 2 <= args.seeds <= 1000:
        parser.error("--seeds must be 2..1000 (exploratory instrumentation checks)")
    root = Path(__file__).resolve().parents[1]
    out = args.out.resolve()
    if out.exists() and any(out.iterdir()):
        parser.error("--out must be new or empty; existing evidence is preserved")
    if out == root or root in out.parents:
        parser.error("use an evidence directory outside the source checkout")
    out.mkdir(parents=True, exist_ok=True)
    frozen = out / "frozen-source"
    source_manifest = freeze(root, frozen)
    write_json(out / "source-manifest.json", source_manifest)
    print(f"Frozen {len(source_manifest)} source files", flush=True)
    target = out / "target"
    common = [args.cargo, "--manifest-path", str(frozen / "Cargo.toml")]
    for action in ("test", "build"):
        checked([common[0], action, *common[1:], "-p", "harpoon-core", "--example", "realtime_research",
                 "--locked", "--target-dir", str(target)], frozen, out / f"{action}.log")
    binary = target / "debug/examples/realtime_research.exe"
    if not binary.exists():
        binary = target / "debug/examples/realtime_research"
    rows = []

    def execute(case, policy, latency, seed, suffix="", extra=()):
        relative = Path("runs") / f"{case}-{policy}-{latency}-{seed}{suffix}.json"
        command = [str(binary), "--case", case, "--policy", policy, "--latency-s", str(latency),
                   "--seed", str(seed), "--out", str(out / relative), *extra]
        proc = subprocess.run(command, cwd=frozen, capture_output=True, text=True, encoding="utf-8")
        if proc.returncode:
            raise RuntimeError(f"run failed: {command}: {proc.stderr}")
        row = json.loads(proc.stdout)
        row["trace"] = relative.as_posix()
        run = json.loads((out / relative).read_text(encoding="utf-8"))
        audit_trace(run)
        return row, run

    for case in ("d0", "d1", "d2"):
        for policy in ("hold", "event", "f60", "f300", "f600"):
            for latency in (5, 180):
                for seed in range(args.seeds):
                    row, _ = execute(case, policy, latency, seed)
                    rows.append(row)
        print(f"Validated {case}: {len(rows)} cumulative controlled runs", flush=True)
    mirror_checks = []
    for seed in range(min(8, args.seeds)):
        baseline, _ = execute("d2", "event", 5, seed, "-balance-baseline")
        for side, mirror in (("red", False), ("blue", True), ("red", True)):
            row, _ = execute("d2", "event", 5, seed, f"-{side}-mirror{int(mirror)}",
                             ("--side", side, *( ("--mirror",) if mirror else () )))
            assert row["success"] == baseline["success"]
            assert row["reached_tick"] is not None and abs(row["reached_tick"] - baseline["reached_tick"]) <= 1
            mirror_checks.append(row)
    first, first_run = execute("d2", "event", 5, 7, "-repeat-a")
    second, second_run = execute("d2", "event", 5, 7, "-repeat-b")
    assert deterministic_view(first_run) == deterministic_view(second_run)
    wall = []
    for case in ("d1", "d2"):
        for latency in (5, 180):
            row, run = execute(case, "event", latency, 7, "-wall", ("--clock", "wall", "--tick-ms", "2"))
            assert row["success"] == (case == "d1" or latency == 5)
            request = run["requests"][0]
            assert request["response_tick"] > request["requested_tick"]
            assert row["episode_wall_us"] >= 2_300_000
            wall.append(row)
    replay_files = [first["trace"], *[r["trace"] for r in wall]]
    for i, path in enumerate(replay_files):
        checked([str(binary), "--replay", str(out / path)], frozen, out / f"replay-{i}.log")
    grouped = defaultdict(list)
    for row in rows:
        policy = row["policy"]["kind"] + str(row["policy"].get("every_s", ""))
        grouped[(row["family"], policy, row["latency_s"])].append(row)
    summary = []
    for (family, policy, latency), group in sorted(grouped.items()):
        summary.append({"family": family, "policy": policy, "latency_s": latency, "episodes": len(group),
                        "successes": sum(r["success"] for r in group),
                        "mean_calls": sum(r["requests"] for r in group) / len(group)})
    for family in ("D1", "D2"):
        for seed in range(args.seeds):
            hold = next(r for r in rows if r["family"] == family and r["seed"] == seed and
                        r["policy"]["kind"] == "hold" and r["latency_s"] == 5)
            assert not hold["success"]
    assert all(r["success"] for r in rows if r["family"] == "D0")
    write_json(out / "summary.json", summary)
    write_json(out / "episodes.json", rows)
    with (out / "summary.csv").open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(summary[0]))
        writer.writeheader()
        writer.writerows(summary)
    evidence = [{"path": p.relative_to(out).as_posix(), "bytes": p.stat().st_size, "sha256": digest(p)}
                for p in sorted((out / "runs").glob("*.json"))]
    write_json(out / "trace-manifest.json", evidence)
    validation = {"stage": "M1a instrumentation controls, not LLM evaluation", "status": "passed",
                  "controlled_episodes": len(rows), "saved_traces": len(evidence), "seed_count": args.seeds,
                  "unit_tests": 7, "mirror_comparisons": len(mirror_checks), "wall_clock_runs": wall,
                  "cross_process_replays": len(replay_files), "deterministic_repeat": first["world_hash"],
                  "all_traces_independently_scored": True, "all_runs_replayed": True,
                  "binary_sha256": digest(binary), "source_manifest_sha256": digest(out / "source-manifest.json"),
                  "model_calls": 0, "model_token_cost": None,
                  "next_gate": "M0 full prior-art matrix and M1 real-model effort/latency calibration remain pending"}
    write_json(out / "validation.json", validation)
    print(json.dumps({"status": "passed", "controlled_episodes": len(rows), "saved_traces": len(evidence),
                      "out": str(out)}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()

"""Audit complete tactical_csg artifacts, including intent vs actual releases.

python scripts/check-csg-tactics.py E:/CodexBuilds/harpoon-csg-tactics-20261020
"""
import collections
import hashlib
import json
import pathlib
import sys


def audit(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["tick"] == 10800 and data["result"], path
    events = data["events"]
    assert [e["seq"] for e in events] == list(range(1, events[-1]["seq"] + 1)), "event gap"
    known = {}
    actions = collections.Counter()
    waves = collections.Counter()
    recoveries = collections.Counter()
    for event in events:
        if event["event"] == "contact_identified":
            known[event["side"], event["contact"]] = event["class_name"]
        if event["event"] == "air_launch" and event["mission"] == "strike":
            waves[event["side"]] += 1
        if event["event"] == "air_recovered":
            recoveries[event["side"]] += event["count"]
        if event.get("domain") == "tactical_ai":
            assert event["vis"] == {"Side": event["side"]}, "private intent leaked"
            actions[event["side"], event["rule"]] += 1
            if event["rule"] == "strike_launch":
                name = known.get((event["side"], event.get("contact")), "unknown")
                assert "buoy" not in name.lower(), f"strike at identified sensor: {event}"
    air_asm = [r for r in data["releases"] if r["kind"] == "Aircraft"
               and ("YJ-83K" in r["weapon"] or "Harpoon" in r["weapon"])]
    assert air_asm and all(r["altitude_m"] == 50.0 for r in air_asm), "not VLow at release"
    sides = {}
    for side in ("Blue", "Red"):
        releases = [r for r in air_asm if r["side"] == side]
        assert waves[side] >= 2 and recoveries[side] > 0, side
        assert (actions[side, "strike_ingress"] + actions[side, "strike_reacquire"]) > 0, side
        # A defended strike may legitimately fail before releasing. Require
        # actual air combat in that case, never force a favourable dice result.
        if not releases:
            assert any(e["event"] == "air_shot_down" and e["side"] == side for e in events), side
        sides[side] = {"strike_waves": waves[side], "air_asm_rounds": sum(r["count"] for r in releases),
                       "air_recovered": recoveries[side], "first_asm_tick": min((r["tick"] for r in releases), default=None),
                       "last_asm_tick": max((r["tick"] for r in releases), default=None),
                       "ingress_orders": actions[side, "strike_ingress"], "salvo_orders": actions[side, "strike_salvo"]}
    return data, {"file": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                  "seed": data["seed"], "result": data["result"], "event_count": len(events),
                  "world_fnv64": data["world_fnv64"], "sides": sides}


def main():
    root = pathlib.Path(sys.argv[1])
    summary = []
    for seed in (20261020, 20261021, 20261022):
        _, result = audit(root / f"csg-{seed}.json")
        summary.append(result)
    for side in ("Blue", "Red"):
        assert sum(run["sides"][side]["air_asm_rounds"] for run in summary) > 0, "no demonstrated air strike for side"
    first, _ = audit(root / "csg-20261020.json")
    repeat, _ = audit(root / "csg-repeat-20261020.json")
    first.pop("wall_seconds")
    repeat.pop("wall_seconds")
    assert first == repeat, "repeat differs in state, events, snapshots or exact release evidence"
    report = {"passed": True, "deterministic_repeat": True, "runs": summary}
    (root / "validation.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()

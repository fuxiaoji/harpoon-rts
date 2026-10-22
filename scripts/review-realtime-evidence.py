"""Read frozen M3a evidence and derive a post-hoc diagnostic; never call a model.

This is an arithmetic reanalysis of saved executions, not a new experiment.
Every consumed artifact must match the frozen manifest. The output is separate.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    stage, out = args.stage.resolve(), args.out.resolve()
    if out == stage or stage in out.parents:
        raise SystemExit("Output must be outside the frozen evidence directory")
    if out.exists():
        raise SystemExit("Use a new output directory; existing results are preserved")
    manifest_path = stage / "artifact-manifest.json"
    manifest_bytes = manifest_path.read_bytes()
    manifest = json.loads(manifest_bytes)
    expected = {row["path"]: row for row in manifest["files"]}
    inputs = []

    def read(rel: str):
        data = (stage / rel).read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        assert digest == expected[rel]["sha256"], rel
        assert len(data) == expected[rel]["bytes"], rel
        inputs.append({"path": rel, "bytes": len(data), "sha256": digest})
        return json.loads(data)

    timing = read("timing/summary.json")
    quality = read("quality/analysis.json")
    frontier = read("delay-frontier/summary.json")
    constraints = read("constraint-controls/summary.json")
    planner = read("public-cost-guard30-verified/summary.json")
    bounds = {(r["tag"], r["delay"]): r["finite_optimum"] for r in frontier}

    grouped = defaultdict(list)
    for row in timing:
        grouped[row["case"]["method"]].append(row)
    timing_totals = {}
    by_cell = {}
    for method, rows in grouped.items():
        timing_totals[method] = {
            "episodes": len(rows), "completed": sum(r["success"] for r in rows),
            "online_calls": sum(r["calls"] for r in rows),
            "calls_with_logical_initialization": sum(r["calls_total"] for r in rows),
        }
        by_cell[method] = {
            tuple(r["case"][k] for k in ("phase", "gap", "window")): r
            for r in rows
        }
    assert len(timing) == 112
    assert len(by_cell["eh"]) == len(by_cell["em"]) == 16
    same_eh_em = sum(
        r["world_hash"] == by_cell["em"][cell]["world_hash"]
        and r["calls"] == by_cell["em"][cell]["calls"]
        for cell, r in by_cell["eh"].items()
    )
    same_quiet_completion = sum(
        r["success"] == by_cell["ehq30"][cell]["success"]
        for cell, r in by_cell["eh"].items()
    )

    decompositions = []
    valid = [r for r in quality["rows"] if r["status"] == "ok"]
    failures = [r for r in quality["rows"] if r["status"] != "ok"]
    for row in valid:
        tag, scores = row["tag"], row["scores"]
        assert scores["wide-0"] == bounds[tag, 0] == row["optimum"]
        assert scores["tight-0"] == scores["wide-180"]
        for delay in (0, 60, 180):
            baseline, bound = bounds[tag, 0], bounds[tag, delay]
            actual = scores[f"wide-{delay}"]
            opportunity_loss = baseline - bound
            plan_gap = bound - actual
            assert opportunity_loss >= 0 and plan_gap >= 0
            assert baseline - actual == opportunity_loss + plan_gap
            decompositions.append({
                "tag": tag, "effort": row["effort"], "request_index": row["index"],
                "delay_sim_s": delay, "zero_delay_optimum": baseline,
                "known_delay_optimum": bound, "saved_plan_score": actual,
                "finite_class_opportunity_loss": opportunity_loss,
                "saved_plan_gap_to_known_delay_optimum": plan_gap,
            })
    assert len(valid) == 17 and len(failures) == 1
    assert same_eh_em == 16
    assert same_quiet_completion == 16
    assert all(r["score"] == bounds[r["tag"], 0] for r in planner)

    summary = {
        "status": "validated_post_hoc_reanalysis",
        "source_root": str(stage),
        "source_manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
        "consumed_artifacts_verified": inputs,
        "new_model_calls": 0, "new_environment_trajectories": 0,
        "timing_totals": timing_totals, "eh_em_identical_world_and_calls_cells": same_eh_em,
        "eh_q30_identical_completion_cells": same_quiet_completion,
        "quality": {"requests": len(quality["rows"]), "valid_optimal": len(valid),
                    "failures_excluded_from_conditional_plan_decomposition": failures},
        "static_time_translation_pairs_checked": len(valid),
        "delay_decompositions": decompositions,
        "constraint_controls": constraints, "costed_guard30_planner": planner,
        "interpretation": [
            "Arithmetic decomposition within the frozen finite action class, not causal mediation.",
            "Known-delay optimum is a hindsight diagnostic bound, not a deployable free policy.",
            "Saved-plan branches are repeated measurements of the original requests, not new samples.",
            "The failed request remains in all-request results; no plan is invented for it.",
            "Only the listed consumed files are hash-verified here, not the complete archive.",
        ],
    }
    out.mkdir(parents=True)
    (out / "evidence-review.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, axes = plt.subplots(1, 2, figsize=(9.2, 4.7), sharey=True)
    for ax, delay in zip(axes, (60, 180)):
        rows = {r["effort"]: r for r in decompositions
                if r["tag"] == "complex-11" and r["delay_sim_s"] == delay}
        efforts = ["low", "medium", "high"]
        floor = [rows[e]["finite_class_opportunity_loss"] for e in efforts]
        gap = [rows[e]["saved_plan_gap_to_known_delay_optimum"] for e in efforts]
        ax.bar(efforts, floor, color="#54799b", label="Opportunity loss in finite action class")
        ax.bar(efforts, gap, bottom=floor, color="#d08543", label="Saved-plan gap to known-delay optimum")
        for i, e in enumerate(efforts):
            ax.text(i, floor[i] + gap[i] + .8, f"Score {rows[e]['saved_plan_score']}",
                    ha="center", fontsize=10)
        ax.set_title(f"Common delay: {delay} simulated seconds")
        ax.set_ylim(0, 48)
        ax.spines[["top", "right"]].set_visible(False)
        ax.set_axisbelow(True)
        ax.grid(axis="y", alpha=.2)
    axes[0].set_ylabel("Points lost from zero-delay optimum (43)")
    fig.suptitle("Equal static scores conceal different tolerance to delay", fontsize=14, y=.98)
    handles, labels = axes[0].get_legend_handles_labels()
    fig.legend(handles, labels, loc="lower center", bbox_to_anchor=(.5, .07), frameon=False, fontsize=9)
    fig.text(.5, .025, "One development snapshot (complex-11). Hindsight bound = 42 in both panels; no new model calls.",
             ha="center", fontsize=8)
    fig.tight_layout(rect=(0, .19, 1, .93))
    for ext in ("png", "svg"):
        fig.savefig(out / f"delay-loss-decomposition.{ext}", dpi=180)
    plt.close(fig)
    print(json.dumps({"out": str(out), "consumed_hashes_verified": len(inputs),
                      "decompositions": len(decompositions), "timing": timing_totals}, ensure_ascii=False))


if __name__ == "__main__":
    main()

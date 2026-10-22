"""Reproduce the arithmetic in Astra's conditional arguments; no model calls."""
from __future__ import annotations
import argparse
import itertools
import json
from pathlib import Path


def verify():
    grid = (0, 0.25, 0.6, 0.95, 1)
    for wait, low, high in itertools.product(grid, repeat=3):
        values = [wait, low, high]
        joint = max(range(3), key=values.__getitem__)
        budget = 1 if low >= high else 2
        decomposed = budget if values[budget] > wait else 0
        assert joint == decomposed
    def effective(pairs):
        q = sum(a for a, _ in pairs)/len(pairs)
        timely = sum(t <= 2 for _, t in pairs)/len(pairs)
        realized = sum(a * (t <= 2) for a, t in pairs)/len(pairs)
        return {"mean_quality": q, "timely_probability": timely,
                "effective_value": realized, "covariance": realized-q*timely}
    a, b = effective([(1, 1), (0, 3)]), effective([(0, 1), (1, 3)])
    assert a["effective_value"] == 0.5 and b["effective_value"] == 0
    scores = {}
    for direction in (-0.5, 0.5):
        scores[str(direction)] = [quality if delay+3+direction*delay <= 5 else 0
                                  for delay, quality in ((1, 0.6), (3, 0.95))]
    assert scores["0.5"] == [0.6, 0]
    assert scores["-0.5"] == [0.6, 0.95]
    conditional = (max(scores["0.5"])+max(scores["-0.5"]))/2
    assert abs(conditional-0.775) < 1e-12
    # Counting all permutations split among three routes of length <= 2.
    import math
    count = sum(sum(sum(v)==r for v in itertools.product(range(3),repeat=3))
                * math.factorial(4)//math.factorial(4-r) for r in range(5))
    assert count == 397
    return {"status": "passed", "joint_equals_exact_decomposition_grid_states": 125,
            "quality_latency_counterexamples": [a, b], "old_plan_direction_scores": scores,
            "conditional_score": conditional, "best_direction_blind_score": 0.6,
            "finite_route_count": count,
            "scope": "constructed arithmetic and standard identities; not Harpoon empirical results or novel theorem"}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    result = verify()
    args.out.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps(result))

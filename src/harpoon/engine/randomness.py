from __future__ import annotations

import random
from typing import Any


class RecordedRandom:
    def __init__(self, seed: int) -> None:
        self.seed = seed

    def d6(self, draw_index: int, count: int = 1) -> tuple[int, dict[str, Any]]:
        rng = random.Random(f"{self.seed}:{draw_index}")
        rolls = [rng.randint(1, 6) for _ in range(count)]
        return sum(rolls), {
            "kind": "d6",
            "draw_index": draw_index,
            "count": count,
            "rolls": rolls,
            "total": sum(rolls),
        }

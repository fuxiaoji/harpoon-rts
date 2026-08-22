from __future__ import annotations

import random
from typing import Any


class RecordedRandom:
    def __init__(self, seed: int) -> None:
        self.seed = seed

    def d6(self, draw_index: int, count: int = 1) -> tuple[int, dict[str, Any]]:
        return self.die(draw_index, 6, count)

    def d20(self, draw_index: int) -> tuple[int, dict[str, Any]]:
        return self.die(draw_index, 20)

    def d100(self, draw_index: int) -> tuple[int, dict[str, Any]]:
        return self.die(draw_index, 100)

    def die(self, draw_index: int, sides: int, count: int = 1) -> tuple[int, dict[str, Any]]:
        rng = random.Random(f"{self.seed}:{draw_index}")
        rolls = [rng.randint(1, sides) for _ in range(count)]
        return sum(rolls), {
            "kind": f"d{sides}",
            "draw_index": draw_index,
            "count": count,
            "rolls": rolls,
            "total": sum(rolls),
        }

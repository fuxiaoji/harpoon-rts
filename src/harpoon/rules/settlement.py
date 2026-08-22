from __future__ import annotations

import json
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal, cast

Countermeasure = Literal["base", "jamming", "decoy", "jamming_and_decoy"]


def _rules_path(name: str) -> Path:
    return Path(__file__).resolve().parents[3] / "data" / "rules" / name


def _load(name: str) -> dict[str, Any]:
    return cast(dict[str, Any], json.loads(_rules_path(name).read_text(encoding="utf-8")))


@dataclass(frozen=True)
class TableLookup:
    table_id: str
    source_page: str
    coordinates: dict[str, Any]
    result: float | int | str

    def trace(self) -> dict[str, Any]:
        return {
            "table_id": self.table_id,
            "source_page": self.source_page,
            "coordinates": self.coordinates,
            "result": self.result,
        }


class SettlementTables:
    def __init__(self) -> None:
        self.attack = _load("surface_missile_attack.json")
        self.damage = _load("damage_tables.json")

    def antiship_probability(
        self,
        target_signature: str,
        ecm_generation: int,
        seeker_generation: int,
        countermeasure: Countermeasure,
    ) -> TableLookup:
        signatures = self.attack["probabilities"]
        if target_signature not in signatures:
            raise ValueError(f"未知目标特征: {target_signature}")
        if not 1 <= ecm_generation <= 4 or not 1 <= seeker_generation <= 4:
            raise ValueError("ECM 与导引头世代必须在 1..4")
        column = self.attack["countermeasures"].index(countermeasure)
        probability = signatures[target_signature][ecm_generation - 1][seeker_generation - 1][
            column
        ]
        return TableLookup(
            table_id=self.attack["table_id"],
            source_page=self.attack["source"]["printed_page"],
            coordinates={
                "target_signature": target_signature,
                "ecm_generation": ecm_generation,
                "seeker_generation": seeker_generation,
                "countermeasure": countermeasure,
            },
            result=float(probability),
        )

    def damage_ratio_criticals(
        self, damage: int, remaining_dp: int, original_dp: int, d6: int
    ) -> tuple[int, TableLookup]:
        table = self.damage["critical_hit_damage_ratio"]
        if damage < original_dp * float(table["minimum_damage_fraction"]):
            row = "minimum_damage_exemption"
            criticals = 0
        elif remaining_dp <= 0:
            row = "sunk"
            criticals = 0
        else:
            ratio = damage / remaining_dp
            if ratio >= 3:
                row = "3.00+"
                criticals = 0
            else:
                floored = math.floor(min(ratio, 1.0) * 10) / 10
                row = f"{floored:.2f}"
                criticals = int(table["rows"][row][d6 - 1])
                if ratio > 1:
                    criticals += math.floor((ratio - 1.0) / 0.2 + 1e-9)
        return criticals, TableLookup(
            table_id=table["table_id"],
            source_page=table["printed_page"],
            coordinates={
                "damage": damage,
                "remaining_dp": remaining_dp,
                "original_dp": original_dp,
                "damage_ratio_row": row,
                "d6": d6,
            },
            result=criticals,
        )

    def critical_type(self, column: str, d20: int) -> TableLookup:
        table = self.damage["critical_hit_table"]
        values = table["columns"][column]
        if len(values) != 20:
            raise ValueError(f"致命损伤列 {column} 不是 20 行")
        result = str(values[d20 - 1])
        return TableLookup(
            table_id=table["table_id"],
            source_page=table["printed_page"],
            coordinates={"column": column, "d20": d20},
            result=result,
        )

    def missile_extra_criticals(self, weapon_dp: int, d6: int) -> tuple[int, TableLookup]:
        table = self.damage["missile_impacts"]
        if weapon_dp < 10:
            result, band = 0, "0-9"
        elif weapon_dp <= 24:
            result, band = d6 // 3, "10-24"
        elif weapon_dp <= 44:
            result, band = d6 // 2, "25-44"
        else:
            result, band = d6, "45+"
        return result, TableLookup(
            table_id=table["table_id"],
            source_page=table["printed_page"],
            coordinates={"weapon_dp_band": band, "d6": d6, "rounding": "down"},
            result=result,
        )

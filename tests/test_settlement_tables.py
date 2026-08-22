from harpoon.rules.settlement import SettlementTables


def test_all_antiship_attack_cells_are_present_and_bounded() -> None:
    tables = SettlementTables()
    for signature in ("large", "medium", "small", "vsmall", "stealthy"):
        for ecm in range(1, 5):
            for seeker in range(1, 5):
                for countermeasure in (
                    "base",
                    "jamming",
                    "decoy",
                    "jamming_and_decoy",
                ):
                    result = tables.antiship_probability(
                        signature, ecm, seeker, countermeasure
                    )
                    assert 0 <= float(result.result) <= 1


def test_tattletail_reference_lookups_match_source_table() -> None:
    tables = SettlementTables()
    assert tables.antiship_probability("medium", 2, 2, "decoy").result == 0.56
    assert tables.antiship_probability("small", 2, 3, "jamming_and_decoy").result == 0.80


def test_damage_ratio_rounds_down_and_uses_d6_column() -> None:
    tables = SettlementTables()
    criticals, lookup = tables.damage_ratio_criticals(55, 113, 168, 4)
    assert lookup.coordinates["damage_ratio_row"] == "0.40"
    assert criticals == 3


def test_missile_impact_bands_and_critical_table_have_twenty_rows() -> None:
    tables = SettlementTables()
    assert tables.missile_extra_criticals(40, 5)[0] == 2
    assert tables.missile_extra_criticals(54, 5)[0] == 5
    for roll in range(1, 21):
        assert tables.critical_type("guided_vs_surface_combatant_size_a_e", roll).result


def test_land_attack_table_visual_reference_cells_and_blocked_cells() -> None:
    tables = SettlementTables()
    assert tables.land_missile_probability("ballistic", "T2", "A").result == 0.80
    assert tables.land_missile_probability("land_attack_cruise", "D2", "D").result == 0.25
    assert tables.land_missile_probability("land_attack_cruise", "T1+", "H").result == 0.01
    try:
        tables.land_missile_probability("ballistic", "M1", "B")
    except ValueError as exc:
        assert "不能攻击" in str(exc)
    else:
        raise AssertionError("表中的 -- 单元必须阻断裁决")


def test_runway_use_boundaries_and_airbase_component_ranges() -> None:
    tables = SettlementTables()
    assert tables.runway_aircraft_limit(3700, 1).result == "large"
    assert tables.runway_aircraft_limit(3700, 8).result == "stol"
    assert tables.runway_aircraft_limit(3700, 9).result == "none"
    assert tables.air_base_component(1).result == "runways"
    assert tables.air_base_component(60).result == "defenses"
    assert tables.air_base_component(100).result == "empty_space"

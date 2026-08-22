from __future__ import annotations

import json
import math
from datetime import timedelta
from pathlib import Path
from typing import Any

from harpoon.domain.models import (
    PHASE_ORDER,
    ActionOption,
    Command,
    Event,
    GameSnapshot,
    GroundFacilityDefinition,
    InFlightWeapon,
    Observation,
    Phase,
    PhaseStatus,
    Position,
    Readiness,
    RuleExplanation,
    UnitState,
    ValidationResult,
    Visibility,
    WeaponDefinition,
    WorldState,
)
from harpoon.engine.randomness import RecordedRandom
from harpoon.rules.catalog import explain
from harpoon.rules.settlement import SettlementTables


class GameEngine:
    """Authoritative single-process engine. Controllers only use this public facade."""

    def __init__(
        self,
        state: WorldState,
        platforms: dict[str, Any],
        weapons: dict[str, WeaponDefinition],
        objectives: dict[str, tuple[str, ...]],
        ground_facilities: dict[str, GroundFacilityDefinition] | None = None,
    ) -> None:
        self.state = state
        self.platforms = platforms
        self.weapons = weapons
        self.objectives = objectives
        self.ground_facilities = ground_facilities or {}
        self.events: list[Event] = []
        self._commands: dict[str, Command] = {}
        self._rng = RecordedRandom(state.seed)
        self._event_sequence = 0
        self._tables = SettlementTables()

    def auto_adjudicate_arrival(self, entity_id: str) -> Event:
        """Resolve an arrived SSM from the verified attack and damage tables."""
        entity = next(
            x for x in self.state.in_flight_weapons if x.entity_id == entity_id and x.awaiting_adjudication
        )
        weapon = self.weapons[entity.weapon_id]
        target = self.state.units[entity.target_id]
        platform = self.platforms[target.platform_id]
        if weapon.seeker_generation is None or platform.ecm_generation is None:
            raise RuntimeError("缺少导引头或 ECM 世代，禁止自动裁决")
        lookup = self._tables.antiship_probability(
            platform.target_signature,
            platform.ecm_generation,
            weapon.seeker_generation,
            platform.countermeasure,
        )
        roll, record = self._rng.d100(self.state.random_draw_index)
        hit = roll <= round(float(lookup.result) * 100)
        return self._apply_arrival_result(
            entity,
            hit,
            "verified_antiship_missile_attack_table",
            hit_trace={
                "lookup": lookup.trace(),
                "roll": record,
                "comparison": f"D100 <= {round(float(lookup.result) * 100)}",
                "result": "seeker_lock" if hit else "soft_kill",
                "hard_kill_defense": "not_available_or_not_yet_resolved",
            },
            initial_draws=1,
        )

    def observe(self, side_id: str) -> Observation:
        visible = tuple(
            e
            for e in self.events
            if e.visibility == Visibility.PUBLIC
            or (e.visibility == Visibility.SIDE and e.side_id == side_id)
        )
        return Observation(
            side_id=side_id,
            scenario_id=self.state.scenario_id,
            timestamp=self.state.timestamp,
            turn=self.state.turn,
            phase=self.state.phase,
            version=self.state.version,
            own_units=tuple(u for u in self.state.units.values() if u.side_id == side_id),
            contacts=self.state.contacts.get(side_id, ()),
            visible_events=visible,
            objectives=self.objectives.get(side_id, ()),
        )

    def list_actions(self, side_id: str, unit_id: str | None = None) -> tuple[ActionOption, ...]:
        units = [
            u
            for u in self.state.units.values()
            if u.side_id == side_id and (unit_id is None or u.unit_id == unit_id)
        ]
        options: list[ActionOption] = []
        for unit in sorted(units, key=lambda x: x.unit_id):
            platform = self.platforms[unit.platform_id]
            alive = not unit.sunk
            planning = self.state.phase == Phase.PLOTTING and side_id not in self.state.locked_sides
            verified_platform = bool(platform.verified)
            base_ok = alive and planning and verified_platform
            base_reason = (
                None
                if base_ok
                else "平台数据库条目尚未逐页核验，禁止执行"
                if not verified_platform
                else "仅存活单位可在未锁定的计划阶段行动"
            )
            options.append(
                ActionOption(
                    action_id=f"move:{unit.unit_id}",
                    action_type="move",
                    unit_id=unit.unit_id,
                    parameter_schema={
                        "type": "object",
                        "properties": {
                            "course_deg": {"type": "number", "minimum": 0, "exclusiveMaximum": 360},
                            "speed_knots": {"type": "number", "minimum": 0},
                        },
                        "required": ["course_deg", "speed_knots"],
                    },
                    available=base_ok,
                    unavailable_reason=base_reason,
                    rule=explain("H5-3.1-MOVE"),
                )
            )
            targets = tuple(
                c.target_unit_id
                for c in self.state.contacts.get(side_id, ())
                if c.quality == "good_fcs" and c.target_unit_id
            )
            for weapon_id in platform.weapons:
                ammo = unit.ammunition.get(weapon_id, 0)
                weapon_definition = self.weapons[weapon_id]
                if weapon_definition.attack_domain == "land":
                    land_targets = tuple(
                        facility_id
                        for facility_id, facility in self.ground_facilities.items()
                        if facility.side_id != side_id
                        and not self.state.ground_facilities[facility_id].destroyed
                    )
                    strike_ok = base_ok and ammo > 0 and bool(land_targets)
                    options.append(
                        ActionOption(
                            action_id=f"landstrike:{unit.unit_id}:{weapon_id}",
                            action_type="land_strike",
                            unit_id=unit.unit_id,
                            parameter_schema={
                                "type": "object",
                                "properties": {"target_id": {"type": "string"}},
                                "required": ["target_id"],
                            },
                            candidate_targets=land_targets,
                            available=strike_ok,
                            unavailable_reason=None if strike_ok else "需要弹药和可用陆上目标",
                            rule=explain("H5-11-STRIKE"),
                        )
                    )
                    continue
                fire_ok = base_ok and ammo > 0 and bool(targets)
                reason = None if fire_ok else "需要计划阶段、可用弹药和 Good 火控解"
                if side_id == "nato" and not self._nato_may_fire():
                    fire_ok, reason = False, "Tattletail 和平时期交战规则禁止先开火"
                options.append(
                    ActionOption(
                        action_id=f"fire:{unit.unit_id}:{weapon_id}",
                        action_type="fire",
                        unit_id=unit.unit_id,
                        parameter_schema={
                            "type": "object",
                            "properties": {"target_id": {"type": "string"}},
                            "required": ["target_id"],
                        },
                        candidate_targets=targets,
                        available=fire_ok,
                        unavailable_reason=reason,
                        rule=explain("H5-8-SURFACE" if side_id == "soviet" else "TAT-ROE"),
                    )
                )
            options.append(
                ActionOption(
                    action_id=f"wait:{unit.unit_id}",
                    action_type="wait",
                    unit_id=unit.unit_id,
                    available=base_ok,
                    unavailable_reason=base_reason,
                    rule=explain("H5-2.4-PLOT"),
                )
            )
        for kind, rid in (
            ("air_mission", "H5-4-AIR"),
            ("air_attack", "H5-9-AIRWAR"),
            ("submarine_attack", "H5-10-SUBWAR"),
            ("electronic_warfare", "H5-12-EW"),
            ("mine_warfare", "H5-13-MINES"),
        ):
            options.append(
                ActionOption(
                    action_id=f"capability:{kind}",
                    action_type=kind,
                    available=False,
                    unavailable_reason="当前想定无适用实体或裁决表尚未 verified",
                    rule=explain(rid),
                )
            )
        return tuple(options)

    def explain_action(self, action_id: str) -> RuleExplanation:
        for side in self.objectives:
            for option in self.list_actions(side):
                if option.action_id == action_id:
                    return option.rule
        raise KeyError(action_id)

    def submit(self, command: Command) -> ValidationResult:
        if command.observation_version != self.state.version:
            return ValidationResult(accepted=False, errors=("observation_version 已过期",))
        option = next(
            (x for x in self.list_actions(command.side_id) if x.action_id == command.action_id),
            None,
        )
        if option is None:
            return ValidationResult(accepted=False, errors=("未知行动",))
        if not option.available:
            return ValidationResult(
                accepted=False,
                errors=(option.unavailable_reason or "行动不可用",),
                rule_ids=(option.rule.rule_id,),
            )
        prefix, *parts = command.action_id.split(":")
        if prefix == "move":
            speed = float(command.parameters.get("speed_knots", -1))
            course = float(command.parameters.get("course_deg", -1))
            platform = self.platforms[self.state.units[parts[0]].platform_id]
            if not 0 <= course < 360 or not 0 <= speed <= platform.maximum_speed_knots:
                return ValidationResult(
                    accepted=False,
                    errors=("航向或速度超出平台限制",),
                    rule_ids=(option.rule.rule_id,),
                )
        if prefix in ("fire", "landstrike") and command.parameters.get("target_id") not in option.candidate_targets:
            return ValidationResult(
                accepted=False, errors=("目标不在候选列表",), rule_ids=(option.rule.rule_id,)
            )
        self._commands[command.command_id] = command
        return ValidationResult(accepted=True, rule_ids=(option.rule.rule_id,))

    def lock(self, side_id: str) -> PhaseStatus:
        locked = set(self.state.locked_sides)
        locked.add(side_id)
        self.state = self.state.model_copy(
            update={"locked_sides": frozenset(locked), "version": self.state.version + 1}
        )
        return PhaseStatus(
            phase=self.state.phase,
            locked_sides=self.state.locked_sides,
            all_locked=self.state.locked_sides == frozenset(self.objectives),
        )

    def resolve(self) -> tuple[Event, ...]:
        if self.state.phase == Phase.PLOTTING and self.state.locked_sides != frozenset(
            self.objectives
        ):
            raise RuntimeError("所有阵营必须锁定后才能裁决")
        created: list[Event] = []
        if self.state.phase == Phase.PLOTTING:
            created.extend(self._resolve_plots())
        elif self.state.phase == Phase.MOVEMENT:
            created.extend(self._resolve_movement())
        elif self.state.phase == Phase.PLANNED_FIRE:
            created.extend(self._resolve_fire())
        elif self.state.phase == Phase.DETECTION:
            created.extend(self._resolve_detection())
        elif self.state.phase == Phase.RESOLUTION:
            created.extend(self._resolve_resolution())
        self.events.extend(created)
        next_phase = PHASE_ORDER[(PHASE_ORDER.index(self.state.phase) + 1) % len(PHASE_ORDER)]
        update: dict[str, Any] = {
            "phase": next_phase,
            "version": self.state.version + 1,
            "locked_sides": frozenset(),
        }
        if next_phase == Phase.PLOTTING:
            update["turn"] = self.state.turn + 1
            update["timestamp"] = self.state.timestamp + timedelta(minutes=3)
            self._commands.clear()
        self.state = self.state.model_copy(update=update)
        return tuple(created)

    def adjudicate_arrival(
        self,
        entity_id: str,
        hit: bool,
        referee_side: str = "referee",
        adjudication_basis: str = "referee_ruling_unverified_hit_table",
    ) -> Event:
        weapon_entity = next(
            x
            for x in self.state.in_flight_weapons
            if x.entity_id == entity_id and x.awaiting_adjudication
        )
        return self._apply_arrival_result(
            weapon_entity, hit, adjudication_basis, hit_trace=None, initial_draws=0
        )

    def _apply_arrival_result(
        self,
        weapon_entity: InFlightWeapon,
        hit: bool,
        adjudication_basis: str,
        hit_trace: dict[str, Any] | None,
        initial_draws: int,
    ) -> Event:
        weapon = self.weapons[weapon_entity.weapon_id]
        payload: dict[str, Any] = {
            "hit": hit,
            "target_id": weapon_entity.target_id,
            "adjudication_basis": adjudication_basis,
            "hit_table_status": "verified" if hit_trace else "manual_referee_ruling",
        }
        if hit_trace:
            payload["hit_resolution"] = hit_trace
        random_record = None
        units = dict(self.state.units)
        draws = initial_draws
        if hit:
            die, random_record = self._rng.d6(self.state.random_draw_index + draws)
            draws += 1
            raw_damage = weapon.base_damage_dp + die // weapon.damage_divisor
            target = units[weapon_entity.target_id]
            platform = self.platforms[target.platform_id]
            damage = math.floor(raw_damage * (100 + platform.damage_modifier_percent) / 100)
            new_dp = min(target.damage_dp + damage, self.platforms[target.platform_id].maximum_dp)
            maximum = self.platforms[target.platform_id].maximum_dp
            remaining = maximum - new_dp
            ratio_die, ratio_record = self._rng.d6(self.state.random_draw_index + draws)
            draws += 1
            ratio_criticals, ratio_lookup = self._tables.damage_ratio_criticals(
                damage, remaining, maximum, ratio_die
            )
            extra_die, extra_record = self._rng.d6(self.state.random_draw_index + draws)
            draws += 1
            extra_criticals, extra_lookup = self._tables.missile_extra_criticals(
                raw_damage, extra_die
            )
            criticals: list[str] = []
            critical_traces: list[dict[str, Any]] = []
            for origin, count in (("damage_ratio", ratio_criticals), ("missile_airframe", extra_criticals)):
                for _ in range(count):
                    d20, d20_record = self._rng.d20(self.state.random_draw_index + draws)
                    draws += 1
                    critical_lookup = self._tables.critical_type(
                        "guided_vs_surface_combatant_size_a_e", d20
                    )
                    criticals.append(str(critical_lookup.result))
                    critical_traces.append(
                        {"origin": origin, "roll": d20_record, "lookup": critical_lookup.trace()}
                    )
            if weapon.missile_impact_criticals:
                criticals.append("fire")
            units[target.unit_id] = target.model_copy(
                update={
                    "damage_dp": new_dp,
                    "speed_knots": min(
                        target.speed_knots,
                        next(
                            speed
                            for threshold, speed in reversed(
                                list(
                                    zip(
                                        platform.damage_breakpoints_dp,
                                        platform.speed_breakpoints_knots,
                                        strict=True,
                                    )
                                )
                            )
                            if new_dp >= threshold
                        ),
                    ),
                    "crippled": new_dp >= platform.damage_breakpoints_dp[2],
                    "sunk": new_dp >= maximum,
                    "critical_hits": target.critical_hits + tuple(criticals),
                    "fire_critical_count": target.fire_critical_count + criticals.count("fire"),
                }
            )
            payload["damage_dp"] = damage
            payload["calculation"] = {
                "formula": "base_damage_dp + floor(D6 / damage_divisor)",
                "base_damage_dp": weapon.base_damage_dp,
                "die_roll": die,
                "damage_divisor": weapon.damage_divisor,
                "raw_damage_dp": raw_damage,
                "target_damage_modifier_percent": platform.damage_modifier_percent,
                "result_damage_dp": damage,
                "target_previous_damage_dp": target.damage_dp,
                "target_new_damage_dp": new_dp,
                "crippled_threshold_dp": math.ceil(maximum / 2),
                "sunk_threshold_dp": maximum,
            }
            payload["critical_hit_resolution"] = {
                "damage_ratio_roll": ratio_record,
                "damage_ratio_lookup": ratio_lookup.trace(),
                "missile_airframe_roll": extra_record,
                "missile_airframe_lookup": extra_lookup.trace(),
                "automatic_fire_critical": weapon.missile_impact_criticals,
                "critical_rolls": critical_traces,
                "results": criticals,
            }
        remaining = tuple(
            x for x in self.state.in_flight_weapons if x.entity_id != weapon_entity.entity_id
        )
        self.state = self.state.model_copy(
            update={
                "units": units,
                "in_flight_weapons": remaining,
                "version": self.state.version + 1,
                "random_draw_index": self.state.random_draw_index + draws,
            }
        )
        event = self._event(
            "weapon_adjudicated",
            (weapon_entity.shooter_id, weapon_entity.target_id),
            ("H5-8-SURFACE", "H5-14-DAMAGE"),
            payload,
            random_record=random_record,
        )
        self.events.append(event)
        return event

    def auto_adjudicate_land_arrival(self, entity_id: str) -> Event:
        entity = next(
            item
            for item in self.state.in_flight_weapons
            if item.entity_id == entity_id
            and item.awaiting_adjudication
            and item.target_kind == "ground_facility"
        )
        weapon = self.weapons[entity.weapon_id]
        facility = self.ground_facilities[entity.target_id]
        if weapon.land_attack_type is None or weapon.guidance is None:
            raise RuntimeError("缺少对陆导弹类型或制导等级，禁止自动裁决")
        lookup = self._tables.land_missile_probability(
            weapon.land_attack_type, weapon.guidance, facility.size_class
        )
        roll, record = self._rng.d100(self.state.random_draw_index)
        hit = roll <= round(float(lookup.result) * 100)
        facilities = dict(self.state.ground_facilities)
        target = facilities[entity.target_id]
        damage = 0
        damage_record: dict[str, Any] | None = None
        if hit:
            if weapon.penetration is None:
                raise RuntimeError("缺少武器穿透值，禁止对陆毁伤裁决")
            if weapon.penetration < facility.armor:
                raise RuntimeError("穿透规则分支尚未核验，当前武器不足以穿透该目标")
            damage_die, damage_record = self._rng.d6(self.state.random_draw_index + 1)
            damage = weapon.base_damage_dp + damage_die // weapon.damage_divisor
            new_damage = min(facility.maximum_dp, target.damage_dp + damage)
            facilities[entity.target_id] = target.model_copy(
                update={
                    "damage_dp": new_damage,
                    "runway_cuts": target.runway_cuts
                    + (1 if facility.facility_type == "runway" else 0),
                    "operational": new_damage < facility.maximum_dp / 2,
                    "destroyed": new_damage >= facility.maximum_dp,
                }
            )
        remaining = tuple(
            item for item in self.state.in_flight_weapons if item.entity_id != entity_id
        )
        self.state = self.state.model_copy(
            update={
                "ground_facilities": facilities,
                "in_flight_weapons": remaining,
                "version": self.state.version + 1,
                "random_draw_index": self.state.random_draw_index + (2 if hit else 1),
            }
        )
        event = self._event(
            "land_strike_adjudicated",
            (entity.shooter_id, entity.target_id),
            ("H5-11-3-LAND-MISSILE-ATTACK",),
            {
                "hit": hit,
                "damage_dp": damage,
                "damage_roll": damage_record,
                "damage_formula": "base_damage_dp + floor(D6 / damage_divisor)",
                "penetration": weapon.penetration,
                "target_armor": facility.armor,
                "lookup": lookup.trace(),
                "roll": record,
                "comparison": f"D100 <= {round(float(lookup.result) * 100)}",
            },
            random_record=record,
        )
        self.events.append(event)
        return event

    def snapshot(self) -> GameSnapshot:
        return GameSnapshot(
            state=self.state, event_cursor=len(self.events), state_hash=self.state.state_hash()
        )

    def save(self, path: Path) -> None:
        payload = {
            "snapshot": self.snapshot().model_dump(mode="json"),
            "events": [e.model_dump(mode="json") for e in self.events],
            "platforms": [p.model_dump(mode="json") for p in self.platforms.values()],
            "weapons": [w.model_dump(mode="json") for w in self.weapons.values()],
            "ground_facility_definitions": [
                item.model_dump(mode="json") for item in self.ground_facilities.values()
            ],
            "objectives": self.objectives,
            "pending_commands": [
                command.model_dump(mode="json") for command in self._commands.values()
            ],
        }
        path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")

    def export_jsonl(self, path: Path) -> None:
        path.write_text(
            "".join(
                json.dumps(e.model_dump(mode="json"), ensure_ascii=False, sort_keys=True) + "\n"
                for e in self.events
            ),
            encoding="utf-8",
        )

    def _resolve_plots(self) -> list[Event]:
        events = [
            self._event(
                "orders_locked",
                tuple(sorted(self.objectives)),
                ("H5-2.4-PLOT",),
                {"command_count": len(self._commands)},
            )
        ]
        if self.state.turn > 0:
            _, record = self._rng.d6(self.state.random_draw_index, self.state.turn)
            received = 6 in record["rolls"]
            self.state = self.state.model_copy(
                update={"random_draw_index": self.state.random_draw_index + 1}
            )
            events.append(
                self._event(
                    "attack_word_check",
                    (),
                    ("TAT-ROE",),
                    {"received": received},
                    random_record=record,
                ).model_copy(update={"visibility": Visibility.REFEREE})
            )
        return events

    def _resolve_movement(self) -> list[Event]:
        units = dict(self.state.units)
        weapons: list[InFlightWeapon] = []
        events: list[Event] = []
        for entity in self.state.in_flight_weapons:
            remaining = entity.turns_to_arrival - 1
            entity = entity.model_copy(
                update={
                    "turns_to_arrival": max(0, remaining),
                    "awaiting_adjudication": remaining <= 0,
                }
            )
            weapons.append(entity)
            if remaining <= 0:
                events.append(
                    self._event(
                        "weapon_arrival_requires_adjudication",
                        (entity.shooter_id, entity.target_id),
                        ("H5-8-SURFACE",),
                        {"entity_id": entity.entity_id},
                    )
                )
        for command in sorted(self._commands.values(), key=lambda c: c.command_id):
            if not command.action_id.startswith("move:"):
                continue
            unit_id = command.action_id.split(":")[1]
            unit = units[unit_id]
            speed = float(command.parameters["speed_knots"])
            course = float(command.parameters["course_deg"])
            distance = speed * 3 / 60
            radians = math.radians(course)
            pos = Position(
                x_nm=unit.position.x_nm + math.sin(radians) * distance,
                y_nm=unit.position.y_nm + math.cos(radians) * distance,
            )
            units[unit_id] = unit.model_copy(
                update={"position": pos, "speed_knots": speed, "course_deg": course}
            )
            events.append(
                self._event(
                    "unit_moved",
                    (unit_id,),
                    ("H5-3.1-MOVE",),
                    {
                        "from": unit.position.model_dump(mode="json"),
                        "to": pos.model_dump(mode="json"),
                        "calculation": {
                            "formula": "distance_nm = speed_knots × 3 / 60",
                            "speed_knots": speed,
                            "tactical_turn_minutes": 3,
                            "distance_nm": distance,
                            "course_deg": course,
                        },
                    },
                )
            )
        self.state = self.state.model_copy(
            update={"units": units, "in_flight_weapons": tuple(weapons)}
        )
        return events

    def _resolve_detection(self) -> list[Event]:
        events: list[Event] = []
        for side_id, contacts in sorted(self.state.contacts.items()):
            for contact in contacts:
                events.append(
                    self._event(
                        "contact_solution_report",
                        (contact.target_unit_id,) if contact.target_unit_id else (),
                        ("H5-5-DETECT", "H5-6-FCS"),
                        {
                            "contact_id": contact.contact_id,
                            "quality": contact.quality,
                            "source": "scenario_provided_initial_solution",
                            "automatic_detection_roll": False,
                            "reason": "完整传感器探测表尚未 verified；本想定直接给定 Good 火控解",
                        },
                    ).model_copy(update={"visibility": Visibility.SIDE, "side_id": side_id})
                )
        return events

    def _resolve_fire(self) -> list[Event]:
        units = dict(self.state.units)
        inflight = list(self.state.in_flight_weapons)
        events = []
        soviet_attack = False
        for command in sorted(self._commands.values(), key=lambda c: c.command_id):
            if command.action_id.startswith("landstrike:"):
                _, shooter_id, weapon_id = command.action_id.split(":")
                target_id = str(command.parameters["target_id"])
                shooter = units[shooter_id]
                weapon = self.weapons[weapon_id]
                ammo = dict(shooter.ammunition)
                ammo[weapon_id] -= 1
                units[shooter_id] = shooter.model_copy(update={"ammunition": ammo})
                eta = max(
                    1,
                    math.ceil(
                        weapon.maximum_range_nm
                        / max(weapon.speed_knots * 3 / 60, 0.001)
                    ),
                )
                inflight.append(
                    InFlightWeapon(
                        entity_id=f"weapon-{self.state.turn}-{command.command_id}",
                        weapon_id=weapon_id,
                        shooter_id=shooter_id,
                        target_id=target_id,
                        owner_side_id=shooter.side_id,
                        turns_to_arrival=eta,
                        launch_range_nm=weapon.maximum_range_nm,
                        target_kind="ground_facility",
                    )
                )
                events.append(
                    self._event(
                        "land_attack_missile_launched",
                        (shooter_id, target_id),
                        ("H5-11-STRIKE", weapon.rule_id),
                        {"weapon_id": weapon_id, "eta_turns": eta},
                    )
                )
                continue
            if not command.action_id.startswith("fire:"):
                continue
            _, shooter_id, weapon_id = command.action_id.split(":")
            target_id = str(command.parameters["target_id"])
            shooter = units[shooter_id]
            target = units[target_id]
            weapon = self.weapons[weapon_id]
            distance = math.hypot(
                target.position.x_nm - shooter.position.x_nm,
                target.position.y_nm - shooter.position.y_nm,
            )
            if not weapon.minimum_range_nm <= distance <= weapon.maximum_range_nm:
                events.append(
                    self._event(
                        "launch_rejected_at_resolution",
                        (shooter_id, target_id),
                        (weapon.rule_id,),
                        {"reason": "range", "range_nm": distance},
                    )
                )
                continue
            eta = max(1, math.ceil(distance / (weapon.speed_knots * 3 / 60)))
            inflight.append(
                InFlightWeapon(
                    entity_id=f"weapon-{self.state.turn}-{command.command_id}",
                    weapon_id=weapon_id,
                    shooter_id=shooter_id,
                    target_id=target_id,
                    owner_side_id=shooter.side_id,
                    turns_to_arrival=eta,
                    launch_range_nm=distance,
                )
            )
            ammo = dict(shooter.ammunition)
            ammo[weapon_id] -= 1
            units[shooter_id] = shooter.model_copy(update={"ammunition": ammo})
            events.append(
                self._event(
                    "weapon_launched",
                    (shooter_id, target_id),
                    (weapon.rule_id, "H5-7-GUIDANCE"),
                    {"weapon_id": weapon_id, "range_nm": distance, "eta_turns": eta},
                )
            )
            events[-1] = events[-1].model_copy(
                update={
                    "payload": {
                        **events[-1].payload,
                        "calculation": {
                            "range_formula": "sqrt((target_x-shooter_x)^2 + (target_y-shooter_y)^2)",
                            "minimum_range_nm": weapon.minimum_range_nm,
                            "maximum_range_nm": weapon.maximum_range_nm,
                            "range_gate_passed": True,
                            "eta_formula": "ceil(range_nm / (speed_knots × 3 / 60))",
                            "weapon_speed_knots": weapon.speed_knots,
                            "eta_turns": eta,
                        },
                    }
                }
            )
            soviet_attack = soviet_attack or shooter.side_id == "soviet"
        if soviet_attack:
            for unit_id, unit in sorted(units.items()):
                if unit.side_id != "nato" or unit.readiness != Readiness.CONDITION_III:
                    continue
                roll, record = self._rng.d6(self.state.random_draw_index)
                self.state = self.state.model_copy(
                    update={"random_draw_index": self.state.random_draw_index + 1}
                )
                turns = 1 if roll <= 3 else 2
                units[unit_id] = unit.model_copy(
                    update={
                        "readiness": Readiness.TRANSITIONING,
                        "readiness_turns_remaining": turns,
                    }
                )
                events.append(
                    self._event(
                        "readiness_transition_started",
                        (unit_id,),
                        ("TAT-READINESS",),
                        {"turns": turns},
                        random_record=record,
                    )
                )
                events[-1] = events[-1].model_copy(
                    update={
                        "payload": {
                            **events[-1].payload,
                            "table_lookup": "D6 1–3 → 1 回合；D6 4–6 → 2 回合",
                            "die_roll": roll,
                            "result_turns": turns,
                        }
                    }
                )
        self.state = self.state.model_copy(
            update={"units": units, "in_flight_weapons": tuple(inflight)}
        )
        return events

    def _resolve_resolution(self) -> list[Event]:
        units = dict(self.state.units)
        events = []
        for uid, unit in units.items():
            if unit.readiness == Readiness.TRANSITIONING and unit.readiness_turns_remaining > 0:
                left = unit.readiness_turns_remaining - 1
                units[uid] = unit.model_copy(
                    update={
                        "readiness_turns_remaining": left,
                        "readiness": Readiness.CONDITION_I
                        if left == 0
                        else Readiness.TRANSITIONING,
                    }
                )
        self.state = self.state.model_copy(update={"units": units, "winner": self._winner(units)})
        if self.state.winner:
            events.append(
                self._event("victory", (self.state.winner,), (), {"winner": self.state.winner})
            )
        return events

    def _winner(self, units: dict[str, UnitState]) -> str | None:
        if self.state.scenario_id == "ryukyu-shockwave-2025":
            runway_ids = [
                facility_id
                for facility_id, definition in self.ground_facilities.items()
                if definition.facility_type == "runway"
            ]
            disabled = sum(
                not self.state.ground_facilities[facility_id].operational
                for facility_id in runway_ids
            )
            if disabled >= 3:
                return "china_tactical"
            if self.state.turn >= 239:
                return "usa_tactical"
            return None
        if self.state.scenario_id != "tattletail-1991-strict":
            return None
        soviet = units["smyshlennyy-mod-kashin"]
        poolster = units["poolster-a835"]
        nato_crippled = sum(u.crippled for u in units.values() if u.side_id == "nato")
        if soviet.sunk:
            return (
                "nato_decisive"
                if nato_crippled <= 1
                else "nato_tactical"
                if nato_crippled <= 2
                else "nato"
            )
        if poolster.crippled and nato_crippled >= 3:
            return "soviet_decisive"
        if poolster.crippled and nato_crippled >= 2:
            return "soviet_tactical"
        return None

    def _nato_may_fire(self) -> bool:
        attacked = any(
            e.event_type == "weapon_launched"
            and e.actors
            and self.state.units[e.actors[0]].side_id == "soviet"
            for e in self.events
        )
        word_received = any(
            e.event_type == "attack_word_check" and bool(e.payload.get("received"))
            for e in self.events
        )
        return attacked or word_received

    def _event(
        self,
        event_type: str,
        actors: tuple[str, ...],
        rule_ids: tuple[str, ...],
        payload: dict[str, Any],
        random_record: dict[str, Any] | None = None,
    ) -> Event:
        self._event_sequence += 1
        return Event(
            event_id=f"event-{self._event_sequence:08d}",
            turn=self.state.turn,
            phase=self.state.phase,
            event_type=event_type,
            actors=actors,
            rule_ids=rule_ids,
            payload=payload,
            random_record=random_record,
        )

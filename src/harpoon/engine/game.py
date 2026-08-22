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


class GameEngine:
    """Authoritative single-process engine. Controllers only use this public facade."""

    def __init__(
        self,
        state: WorldState,
        platforms: dict[str, Any],
        weapons: dict[str, WeaponDefinition],
        objectives: dict[str, tuple[str, ...]],
    ) -> None:
        self.state = state
        self.platforms = platforms
        self.weapons = weapons
        self.objectives = objectives
        self.events: list[Event] = []
        self._commands: dict[str, Command] = {}
        self._rng = RecordedRandom(state.seed)
        self._event_sequence = 0

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
            alive = not unit.sunk
            planning = self.state.phase == Phase.PLOTTING and side_id not in self.state.locked_sides
            base_ok = alive and planning
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
                    unavailable_reason=None if base_ok else "仅存活单位可在未锁定的计划阶段机动",
                    rule=explain("H5-3.1-MOVE"),
                )
            )
            targets = tuple(
                c.target_unit_id
                for c in self.state.contacts.get(side_id, ())
                if c.quality == "good_fcs" and c.target_unit_id
            )
            for weapon_id in self.platforms[unit.platform_id].weapons:
                ammo = unit.ammunition.get(weapon_id, 0)
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
                    unavailable_reason=None if base_ok else "仅计划阶段可等待",
                    rule=explain("H5-2.4-PLOT"),
                )
            )
        for kind, rid in (
            ("air_mission", "H5-4-AIR"),
            ("air_attack", "H5-9-AIRWAR"),
            ("submarine_attack", "H5-10-SUBWAR"),
            ("land_strike", "H5-11-STRIKE"),
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
        if prefix == "fire" and command.parameters.get("target_id") not in option.candidate_targets:
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
        weapon = self.weapons[weapon_entity.weapon_id]
        payload: dict[str, Any] = {
            "hit": hit,
            "target_id": weapon_entity.target_id,
            "adjudication_basis": adjudication_basis,
            "hit_table_status": "blocked_pending_authoritative_table_verification",
        }
        random_record = None
        units = dict(self.state.units)
        if hit:
            die, random_record = self._rng.d6(self.state.random_draw_index)
            damage = weapon.base_damage_dp + die // weapon.damage_divisor
            target = units[weapon_entity.target_id]
            new_dp = min(target.damage_dp + damage, self.platforms[target.platform_id].maximum_dp)
            maximum = self.platforms[target.platform_id].maximum_dp
            units[target.unit_id] = target.model_copy(
                update={
                    "damage_dp": new_dp,
                    "crippled": new_dp * 2 >= maximum,
                    "sunk": new_dp >= maximum,
                }
            )
            payload["damage_dp"] = damage
            payload["calculation"] = {
                "formula": "base_damage_dp + floor(D6 / damage_divisor)",
                "base_damage_dp": weapon.base_damage_dp,
                "die_roll": die,
                "damage_divisor": weapon.damage_divisor,
                "result_damage_dp": damage,
                "target_previous_damage_dp": target.damage_dp,
                "target_new_damage_dp": new_dp,
                "crippled_threshold_dp": math.ceil(maximum / 2),
                "sunk_threshold_dp": maximum,
            }
        remaining = tuple(x for x in self.state.in_flight_weapons if x.entity_id != entity_id)
        self.state = self.state.model_copy(
            update={
                "units": units,
                "in_flight_weapons": remaining,
                "version": self.state.version + 1,
                "random_draw_index": self.state.random_draw_index + (1 if hit else 0),
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

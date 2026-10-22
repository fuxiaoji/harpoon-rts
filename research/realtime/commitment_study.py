"""M3b R2/R3 finite commitment decisions over the frozen navigation engine.

All geometry is fictional.  This is a small finite-action diagnostic, not a
general fleet planner.  Reference search pauses simulated time; its CPU and
wall time are reported separately.  Only RealNav is used by the research CLI.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import time

VERSION = "m3b-commitment-v1"
Z_VALUES = ("none", "B", "C")
PRIOR = {"none": .5, "B": .25, "C": .25}
CHECKPOINT = 240
HORIZON = 1200
RADIUS = .12
SCHEMA = {"type": "object", "properties": {
    "choice": {"type": "string", "enum": ["wait", "A", "D"]},
    "brief": {"type": "string"}}, "required": ["choice", "brief"],
    "additionalProperties": False}


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def digest(value):
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def native_cpu_s(nav):
    """Read native child CPU on Windows; unsupported hosts report unknown."""
    if os.name != "nt" or not hasattr(nav, "process"):
        return None
    import ctypes
    from ctypes import wintypes
    create, exit_time, kernel, user = (wintypes.FILETIME() for _ in range(4))
    ok = ctypes.windll.kernel32.GetProcessTimes(wintypes.HANDLE(int(nav.process._handle)),
        ctypes.byref(create), ctypes.byref(exit_time), ctypes.byref(kernel), ctypes.byref(user))
    if not ok:
        return None
    return (((kernel.dwHighDateTime << 32) + kernel.dwLowDateTime) +
            ((user.dwHighDateTime << 32) + user.dwLowDateTime)) / 10_000_000


def native_cpu_delta(nav, before):
    after = native_cpu_s(nav)
    return None if before is None or after is None else max(0., after - before)


def likelihood(signal, latent, reliability):
    if signal not in Z_VALUES or latent not in Z_VALUES:
        raise ValueError("Unknown categorical signal")
    return reliability if signal == latent else (1 - reliability) / 2


def posterior(s1, s2=None):
    weights = {z: PRIOR[z] * likelihood(s1, z, .6) *
               (likelihood(s2, z, .9) if s2 is not None else 1.) for z in Z_VALUES}
    total = sum(weights.values())
    return {z: weights[z] / total for z in Z_VALUES}


def latent_branches(case):
    """Conditional enumeration uses only the evidence in public_input(case)."""
    observed = case["information"] == "seen"
    belief = posterior(case["s1"], case["s2"] if observed else None)
    rows = []
    for z in Z_VALUES:
        for signal in ([case["s2"]] if observed else Z_VALUES):
            for arrival in ([0] if observed else [120, 180]):
                probability = belief[z] * (1 if observed else likelihood(signal, z, .9) * .5)
                rows.append({"z": z, "s2": signal, "arrival": arrival, "p": probability})
    assert abs(sum(row["p"] for row in rows) - 1) < 1e-12
    return rows


class Ledger:
    """Conserved kits: available + reserved + consumed == initial stock."""
    def __init__(self, total=3):
        self.total = total
        self.consumed = 0
        self.reservations = {}
        self.active_tranches = {}

    @property
    def available(self):
        return self.total - self.consumed - sum(self.reservations.values())

    def check(self):
        assert self.available >= 0 and self.consumed >= 0
        assert all(v >= 0 for v in self.reservations.values())
        assert self.available + self.consumed + sum(self.reservations.values()) == self.total
        return self.snapshot()

    def snapshot(self):
        return {"total": self.total, "available": self.available,
                "consumed": self.consumed, "reserved": dict(self.reservations),
                "refundable_unfinished_start": dict(self.active_tranches)}

    def reserve_many(self, requests):
        """Joint acceptance is atomic, not individually legal double booking."""
        if len(requests) != len(set(requests)) or any(k in self.reservations for k in requests):
            raise ValueError("Duplicate reservation")
        if any(type(v) is not int or v <= 0 for v in requests.values()):
            raise ValueError("Reservation must be a positive integer")
        if sum(requests.values()) > self.available:
            raise ValueError("Insufficient uncommitted kits")
        self.reservations.update(requests)
        self.check()

    def start(self, job):
        if job in self.active_tranches or self.reservations.get(job, 0) < 1:
            raise ValueError("Invalid service start")
        self.reservations[job] -= 1
        self.consumed += 1
        self.active_tranches[job] = 1
        self.check()

    def finish(self, job, leg_cost):
        if self.active_tranches.get(job) != 1:
            raise ValueError("Completion requires actual service start")
        rest = leg_cost - 1
        if rest < 0 or self.reservations.get(job, -1) < rest:
            raise ValueError("Completion exceeds reservation")
        self.reservations[job] -= rest
        self.consumed += rest
        self.active_tranches.pop(job)
        if not self.reservations[job]:
            self.reservations.pop(job)
        self.check()

    def cancel(self, job, reversible):
        released = self.reservations.pop(job, 0)
        start_cost = self.active_tranches.pop(job, 0)
        refund = start_cost if reversible else 0
        self.consumed -= refund
        self.check()
        return {"released": released, "refund": refund, "sunk": start_cost - refund}


def distance(a, b):
    return math.hypot(a["x"] - b["x"], a["y"] - b["y"])


def _leg(name, x, y, service, cost, reward, kind="certain"):
    return {"id": name, "goal": {"x": x, "y": y}, "service_s": service,
            "cost": cost, "reward": reward, "reward_condition": kind}


def build_cases(nav):
    """Freeze 24 cells using native navigation calibration, before model calls.

    One predefined evidence realization per structure is a development fixture,
    not an independent randomly sampled population. Hidden realized Z is used
    only for the supplementary realized score, never the expected objective.
    """
    units = [{"id": "u1", "home": {"x": 0., "y": 0.}},
             {"id": "u2", "home": {"x": 0., "y": .55}},
             {"id": "u3", "home": {"x": 0., "y": -.55}}]
    for unit in units:
        unit["standing_goal"] = {"x": .8, "y": unit["home"]["y"]}
    states = {u["id"]: nav.initial(u["home"], course=90) for u in units}
    worlds = {u: nav.dump(state) for u, state in states.items()}
    platform = worlds["u1"]["db"]["platforms"]["ddg_aegis"]
    public_physics = {k: platform[k] for k in
                      ("id", "max_speed_kts", "turn_rate_deg_s", "accel_kts_per_s")}
    public_physics.update(commanded_speed_kts=18., tick_seconds=1,
        decel_kts_per_s=platform["accel_kts_per_s"] * 3,
        approach_distance_nm=.5, approach_speed_cap_kts=platform["max_speed_kts"] * .5,
        native_waypoint_pop_radius_nm=.1, executor_arrival_radius_nm=RADIUS,
        step_order=["commands apply", "waypoint desired bearing and approach cap",
                    "bounded turn", "bounded acceleration/deceleration", "integrate position using updated course/speed"],
        waypoint_logic="Before movement, distance <0.1 pops native waypoint; below 0.5 caps ordered speed at half maximum. Executor detects distance <=0.12 after movement and issues actual clear-route/stop on the next tick.",
        coordinates="nautical miles; x east, y north; course 0 north, 90 east",
        obstacles="none in fictional navigation laboratory",
        service="continuous inside 0.12 nm, including physical braking; full native frame checks")
    homes = {u["id"]: u["home"] for u in units}
    templates = {
        "branch": {"A": {"unit": "u1", "legs": [_leg("A1", .30, -.15, 300, 2, 8)]}},
        "chain": {"A": {"unit": "u1", "legs": [
            _leg("A1", .25, -.15, 45, 1, 2), _leg("A2", .65, -.10, 220, 1, 6)]}},
        "shared": {
            "A": {"unit": "u1", "legs": [_leg("A1", .30, -.15, 300, 2, 8)]},
            "D": {"unit": "u2", "legs": [_leg("D1", .30, .65, 300, 1, 5)]}},
    }
    # Fixed before querying models; identical S1/S2/latent fixture across pairs.
    signal_samples = {"branch": ("B", "none", "none", 120),
                      "chain": ("none", "B", "B", 180),
                      "shared": ("C", "C", "C", 120)}
    cases = []
    for family, raw in templates.items():
        jobs = copy.deepcopy(raw)
        jobs["B"] = {"unit": "u2" if family == "shared" else "u1",
                     "legs": [_leg("B1", .70, .35, 120, 3, 16, "B")]}
        jobs["C"] = {"unit": "u3" if family == "shared" else "u1",
                     "legs": [_leg("C1", .70, -.35, 120, 3, 16, "C")]}
        # Physical feasibility calibration uses the same initial world for all arms.
        for job_id, job in jobs.items():
            state = copy.deepcopy(states[job["unit"]])
            for leg in job["legs"]:
                state = nav.advance(state, leg["goal"], seconds=HORIZON)
                if distance(state["pos"], leg["goal"]) > RADIUS + 1e-9:
                    raise AssertionError("Native direct calibration failed")
                leg["direct_arrival_s"] = state["tick"]
                state = nav.advance(state, seconds=leg["service_s"], stop=True,
                                    until_arrival=False)
                leg["direct_completion_s"] = state["tick"]
            job["current"] = job_id not in ("B", "C")
            job["released_at"] = 0 if job["current"] else CHECKPOINT
            job["direct_return_s"] = nav.advance(state, homes[job["unit"]],
                                                  seconds=HORIZON)["tick"]
        s1, s2, z, arrival = signal_samples[family]
        for information, slack, reversible in itertools.product(
                ("seen", "pending"), (120, 240), (False, True)):
            case_jobs = copy.deepcopy(jobs)
            for job in case_jobs.values():
                for leg in job["legs"]:
                    leg["deadline"] = job["released_at"] + leg["direct_completion_s"] + slack
                assert max(l["deadline"] for l in job["legs"]) < HORIZON
                assert job["direct_return_s"] + job["released_at"] < HORIZON
            case = {"version": VERSION,
                    "case_id": f"{family}-{information}-s{slack}-r{int(reversible)}",
                    "structure": family, "information": information, "slack_s": slack,
                    "reversible": reversible, "s1": s1, "s2": s2,
                    "judge_realized_z": z, "judge_s2_arrival": arrival,
                    "units": copy.deepcopy(units), "jobs": case_jobs,
                    "initial_worlds": worlds, "checkpoint": CHECKPOINT, "horizon": HORIZON,
                    "public_physics": public_physics,
                    "initial_resource": 3, "initial_navigation": {
                        u: {k: v for k, v in state.items() if k in
                            ("tick", "pos", "course_deg", "speed_kts")}
                        for u, state in states.items()}}
            case["configuration_sha256"] = digest(case)
            cases.append(case)
    return cases


def actions(case):
    return ["wait"] + [k for k, v in case["jobs"].items() if v["current"]]


def public_input(case):
    """Whitelist observation projection. Never copy an entire judge object."""
    jobs = {}
    for job_id, job in case["jobs"].items():
        jobs[job_id] = {"unit": job["unit"], "current": job["current"],
                       "released_at": job["released_at"], "legs": [
            {k: copy.deepcopy(leg[k]) for k in ("id", "goal", "service_s", "cost",
               "reward", "reward_condition", "deadline")} for leg in job["legs"]]}
        jobs[job_id]["isolated_timely_navigation_reference"] = {
            "meaning": "Native execution starting from initial home/heading at release; initially stationary until release. This is a public per-job physical calibration, not a future policy value; actual old-route delays change position and arrival times.",
            "legs": [{"id": leg["id"],
                      "arrival": job["released_at"] + leg["direct_arrival_s"],
                      "completion": job["released_at"] + leg["direct_completion_s"]}
                     for leg in job["legs"]],
            "return_home": job["released_at"] + job["direct_return_s"]}
    seen = case["information"] == "seen"
    return {"time": 0, "units": copy.deepcopy(case["units"]),
            "navigation": copy.deepcopy(case["initial_navigation"]), "jobs": jobs,
            "resource": {"available": 3, "reserved": 0, "consumed": 0},
            "cancellation_refunds_unfinished_start": case["reversible"],
            "prior": PRIOR.copy(), "evidence": {"S1": case["s1"],
                "S2": case["s2"] if seen else None},
            "evidence_model": {"labels": list(Z_VALUES), "conditional_independence": True,
                "S1_correct": .6, "S2_correct": .9,
                "incorrect_other_label_probability": "(1 - correct) / 2",
                "S2_arrival_if_unseen": [{"time": 120, "p": .5}, {"time": 180, "p": .5}]},
            "allowed_choices": actions(case), "checkpoint": case["checkpoint"],
            "horizon": case["horizon"], "arrival_radius_nm": RADIUS,
            "navigation_execution": copy.deepcopy(case["public_physics"]),
            "rules": [
                "Choose one currently released job or wait. B/C cannot be chosen before 240.",
                "Reserve all remaining job cost before assignment. Start each leg consumes one kit; completion consumes the remainder.",
                "Legs must be physically reached and serviced sequentially by their assigned ship. Deadline is inclusive; completed costs and rewards never roll back.",
                "Cancel before service releases reservation. Cancel unfinished service refunds its start kit only when cancellation_refunds_unfinished_start is true.",
                "B/C are contingent inspections always available at 240. They pay 16 only when latent Z matches their label, otherwise zero; they consume kits in all latent states.",
                "Old standing navigation continues while waiting and during response delay. An on-time choice replaces its assigned ship's route.",
                "At 240, the same public-belief finite planner chooses continue, cancel/return, or cancel/start one job; then no further decisions. Its search uses only visible evidence.",
                "Replies at or after 240 do not override the shared continuation. All completed rewards are gated by the earning ship returning within 0.12 nm of home at the horizon.",
                "Maximize posterior expected cumulative reward over all latent states and unseen signal/arrival branches. Reference/continuation search pauses simulated time and reports measured CPU separately.",
                "Three response-delay diagnostic replays use 0, 60 and 180 seconds; the actual response is also replayed with measured wall time times 5. Do not assume zero delay."]}


def validate(answer, observation):
    if not isinstance(answer, dict) or set(answer) != {"choice", "brief"}:
        raise ValueError("Output must contain exactly choice and brief")
    if answer["choice"] not in observation["allowed_choices"]:
        raise ValueError("Choice is not currently allowed")
    if not isinstance(answer["brief"], str):
        raise ValueError("brief must be a string")
    return answer["choice"]


class Episode:
    """Event stepping of task service over actual native navigation segments."""
    def __init__(self, nav, case):
        self.nav = nav
        self.case = case
        self.tick = 0
        self.states = {u["id"]: nav.load(case["initial_worlds"][u["id"]])
                       for u in case["units"]}
        self.goals = {u["id"]: copy.deepcopy(u["standing_goal"]) for u in case["units"]}
        self.homes = {u["id"]: u["home"] for u in case["units"]}
        self.ledger = Ledger(case["initial_resource"])
        self.completed = {j: [] for j in case["jobs"]}
        self.raw_rewards = {u: {z: 0. for z in Z_VALUES} for u in self.states}
        self.active = None
        self.events = []
        self.service_checks = []
        self.final_phase = False

    def clone(self):
        obj = copy.copy(self)
        for k in ("states", "goals", "ledger", "completed", "raw_rewards", "active", "events", "service_checks"):
            setattr(obj, k, copy.deepcopy(getattr(self, k)))
        return obj

    def record(self, kind, **extra):
        self.events.append({"tick": self.tick, "kind": kind, **extra,
                            "resource": self.ledger.check()})

    def _move(self, unit, seconds, goal=None, stopped=False):
        if not seconds:
            return
        state = self.states[unit]
        target = state["tick"] + seconds
        if stopped or goal is None or distance(state["pos"], goal) <= RADIUS:
            state = self.nav.advance(state, seconds=seconds, stop=True, until_arrival=False)
        else:
            state = self.nav.advance(state, goal=goal, seconds=seconds)
            remaining = target - state["tick"]
            if remaining:
                state = self.nav.advance(state, seconds=remaining, stop=True, until_arrival=False)
        self.states[unit] = state

    def _advance_other(self, seconds, exclude=None):
        for unit in self.states:
            if unit != exclude:
                self._move(unit, seconds, self.goals[unit])

    def assign(self, job_id):
        if self.active:
            raise ValueError("Must cancel/finish active job before assigning")
        job = self.case["jobs"][job_id]
        if self.tick < job["released_at"]:
            raise ValueError("Job is not released")
        unfinished = [leg for leg in job["legs"] if leg["id"] not in self.completed[job_id]]
        if not unfinished:
            raise ValueError("Job already completed")
        self.ledger.reserve_many({job_id: sum(l["cost"] for l in unfinished)})
        self.active = {"job": job_id, "index": len(self.completed[job_id]), "phase": "travel"}
        self.goals[job["unit"]] = unfinished[0]["goal"]
        self.record("assigned", job=job_id, unit=job["unit"])

    def cancel(self):
        if self.active:
            job_id = self.active["job"]
            unit = self.case["jobs"][job_id]["unit"]
            detail = self.ledger.cancel(job_id, self.case["reversible"])
            self.active = None
            self.goals[unit] = self.homes[unit]
            self.record("cancelled", job=job_id, **detail)

    def _service_start(self):
        active = self.active
        job = self.case["jobs"][active["job"]]
        leg = job["legs"][active["index"]]
        assert self.completed[active["job"]] == [l["id"] for l in job["legs"][:active["index"]]]
        assert distance(self.states[job["unit"]]["pos"], leg["goal"]) <= RADIUS + 1e-9
        self.ledger.start(active["job"])
        active.update(phase="service", service_end=self.tick + leg["service_s"])
        self.record("service_started", job=active["job"], leg=leg["id"], unit=job["unit"],
                    pos=copy.deepcopy(self.states[job["unit"]]["pos"]))

    def _service_finish(self):
        active = self.active
        job_id = active["job"]
        job = self.case["jobs"][job_id]
        leg = job["legs"][active["index"]]
        self.ledger.finish(job_id, leg["cost"])
        self.completed[job_id].append(leg["id"])
        on_time = self.tick <= leg["deadline"]
        if on_time:
            for z in Z_VALUES:
                if leg["reward_condition"] in ("certain", z):
                    self.raw_rewards[job["unit"]][z] += leg["reward"]
        self.record("service_completed", job=job_id, leg=leg["id"], unit=job["unit"],
                    on_time=on_time, deadline=leg["deadline"])
        if active["index"] + 1 < len(job["legs"]):
            active.update(index=active["index"] + 1, phase="travel")
            active.pop("service_end", None)
            self.goals[job["unit"]] = job["legs"][active["index"]]["goal"]
        else:
            self.active = None
            self.goals[job["unit"]] = self.homes[job["unit"]]

    def advance(self, target):
        if type(target) is not int or target < self.tick:
            raise ValueError("Time must advance to an integer target")
        while self.tick < target:
            if not self.active:
                self._advance_other(target - self.tick)
                self.tick = target
                break
            active = self.active
            job = self.case["jobs"][active["job"]]
            unit = job["unit"]
            leg = job["legs"][active["index"]]
            if active["phase"] == "travel":
                state = self.states[unit]
                if distance(state["pos"], leg["goal"]) <= RADIUS:
                    self._service_start()
                    continue
                end = self.nav.advance(state, leg["goal"], seconds=target - self.tick)
                elapsed = end["tick"] - self.tick
                if elapsed <= 0:
                    raise AssertionError("Non-progressing native navigation")
                self._advance_other(elapsed, unit)
                self.states[unit] = end
                self.tick = end["tick"]
                if distance(end["pos"], leg["goal"]) <= RADIUS + 1e-9:
                    self._service_start()
            else:
                end = min(active["service_end"], target)
                elapsed = end - self.tick
                segment = self.nav.segment(self.states[unit], seconds=elapsed, stop=True,
                                           until_arrival=False)
                radii = [distance(frame["pos"], leg["goal"]) for frame in segment]
                if max(radii) > RADIUS + 1e-9:
                    raise AssertionError("Continuous service left station radius; calibration is invalid")
                self.service_checks.append({"job": active["job"], "leg": leg["id"],
                    "unit": unit, "from": self.tick, "to": end,
                    "native_frames": len(segment), "max_radius_nm": max(radii)})
                self.states[unit] = segment[-1]
                self._advance_other(elapsed, unit)
                self.tick = end
                if self.tick == active["service_end"]:
                    self._service_finish()
        assert all(s["tick"] == self.tick for s in self.states.values())

    def continue_with(self, choice):
        self.final_phase = True
        busy_unit = self.case["jobs"][self.active["job"]]["unit"] if self.active else None
        for unit in self.goals:
            if unit != busy_unit:
                self.goals[unit] = self.homes[unit]
        if choice == "continue":
            return
        self.cancel()
        if choice != "return":
            self.assign(choice)

    def outcome(self):
        returned = {u: distance(s["pos"], self.homes[u]) <= RADIUS + 1e-9
                    for u, s in self.states.items()}
        rewards = {z: sum(values[z] for u, values in self.raw_rewards.items() if returned[u])
                   for z in Z_VALUES}
        state_public = {u: {k: v for k, v in s.items() if k in
                        ("tick", "pos", "course_deg", "speed_kts")}
                        for u, s in self.states.items()}
        payload = {"rewards_by_z": rewards, "raw_rewards_by_unit": self.raw_rewards,
                   "returned": returned, "final_navigation": state_public,
                   "final_world_hashes": {u: self.nav.fingerprint(s) for u, s in self.states.items()},
                   "completed": self.completed, "resource": self.ledger.check(),
                   "events": self.events, "continuous_service_checks": self.service_checks}
        payload["trajectory_sha256"] = digest(payload)
        return payload


class Evaluator:
    def __init__(self, nav):
        self.nav = nav
        self.physical_cache = {}

    def _physical(self, case, action, delay):
        if action not in actions(case):
            raise ValueError("Invalid initial action")
        if not isinstance(delay, (int, float)) or not math.isfinite(delay) or delay < 0:
            raise ValueError("Delay must be finite and nonnegative")
        delay = math.ceil(delay)
        # Evidence/latent state never influences this branch-independent simulation.
        key = digest({"jobs": case["jobs"], "units": case["units"],
                      "initial_worlds": case["initial_worlds"], "reversible": case["reversible"],
                      "horizon": case["horizon"], "checkpoint": case["checkpoint"],
                      "action": action, "delay": min(delay, case["checkpoint"])})
        cached = key in self.physical_cache
        if not cached:
            wall, cpu = time.perf_counter(), time.process_time()
            native_cpu = native_cpu_s(self.nav)
            episode = Episode(self.nav, case)
            episode.advance(min(delay, case["checkpoint"]))
            if delay < case["checkpoint"]:
                if action != "wait":
                    episode.assign(action)
                else:
                    episode.record("wait_chosen")
            else:
                episode.record("initial_reply_late")
            episode.advance(case["checkpoint"])
            checkpoint = {"tick": episode.tick, "active": copy.deepcopy(episode.active),
                          "completed": copy.deepcopy(episode.completed),
                          "resource": episode.ledger.check(),
                          "events": copy.deepcopy(episode.events)}
            candidates, invalid = {}, {}
            for choice in ["continue", "return"] + list(case["jobs"]):
                if episode.active and choice == episode.active["job"]:
                    continue
                branch = episode.clone()
                try:
                    branch.continue_with(choice)
                except ValueError as exc:
                    invalid[choice] = str(exc)
                    continue
                branch.record("shared_continuation", choice=choice)
                branch.advance(case["horizon"])
                candidates[choice] = branch.outcome()
            result = {"checkpoint_state": checkpoint, "candidates": candidates,
                      "invalid_candidates": invalid,
                      "construction_timing": {"wall_s": time.perf_counter() - wall,
                                               "cpu_s": time.process_time() - cpu,
                                               "native_cpu_s": native_cpu_delta(self.nav, native_cpu),
                                               "attempted_candidates": len(candidates) + len(invalid),
                                               "valid_candidates": len(candidates)}}
            self.physical_cache[key] = result
        return self.physical_cache[key], cached, delay

    def evaluate(self, case, action, delay=0, clairvoyant=False):
        wall, cpu = time.perf_counter(), time.process_time()
        native_cpu = native_cpu_s(self.nav)
        if isinstance(action, dict):
            action = validate(action, public_input(case))
        physical, cached, delay = self._physical(case, action, delay)
        branches = []
        continuation_cache = {}
        selection_count = 0
        for branch in latent_branches(case):
            belief = ({z: float(z == branch["z"]) for z in Z_VALUES} if clairvoyant else
                      posterior(case["s1"], branch["s2"]))
            belief_key = canonical(belief)
            if belief_key not in continuation_cache:
                values = {choice: sum(belief[z] * out["rewards_by_z"][z] for z in Z_VALUES)
                          for choice, out in physical["candidates"].items()}
                choice = max(values, key=values.get)  # insertion-order tie break is frozen
                continuation_cache[belief_key] = (choice, values)
                selection_count += len(values)
            choice, values = continuation_cache[belief_key]
            out = physical["candidates"][choice]
            branches.append({**branch, "continuation": choice, "public_belief": belief,
                "value": out["rewards_by_z"][branch["z"]],
                "continuation_action_values": values,
                "trajectory_sha256": out["trajectory_sha256"],
                "evidence_events": [{"tick": 0, "signal": "S1", "value": case["s1"]},
                    {"tick": branch["arrival"], "signal": "S2", "value": branch["s2"]}]})
        expected = sum(b["p"] * b["value"] for b in branches)
        result = {"case_id": case["case_id"], "action": action, "delay_s": delay,
                  "late": delay >= case["checkpoint"], "expected_value": expected,
                  "branches": branches, "checkpoint_state": physical["checkpoint_state"],
                  "candidate_outcomes": physical["candidates"],
                  "invalid_candidates": physical["invalid_candidates"],
                  "information_policy": "judge_clairvoyant" if clairvoyant else "public_posterior",
                  "reference_clock": "paused_quality_reference",
                  "timing": {"wall_s": time.perf_counter() - wall,
                    "cpu_s": time.process_time() - cpu, "physical_cache_hit": cached,
                    "native_cpu_s": native_cpu_delta(self.nav, native_cpu),
                    "continuation_selection_candidates": selection_count,
                    "physical_construction": physical["construction_timing"]}}
        return result

    def optimum(self, case, delay=0, clairvoyant=False):
        wall, cpu = time.perf_counter(), time.process_time()
        native_cpu = native_cpu_s(self.nav)
        evaluations = {a: self.evaluate(case, a, delay, clairvoyant) for a in actions(case)}
        values = {a: e["expected_value"] for a, e in evaluations.items()}
        best = max(values, key=values.get)
        return {"choice": best, "expected_value": values[best], "action_values": values,
                "evaluations": evaluations, "timing": {"wall_s": time.perf_counter() - wall,
                    "cpu_s": time.process_time() - cpu, "first_action_candidates": len(values),
                    "native_cpu_s": native_cpu_delta(self.nav, native_cpu),
                    "continuation_selection_candidates": sum(e["timing"][
                        "continuation_selection_candidates"] for e in evaluations.values())},
                "reference_clock": "paused_quality_reference"}


def _evaluator(nav):
    if not hasattr(nav, "_commitment_evaluator"):
        nav._commitment_evaluator = Evaluator(nav)
    return nav._commitment_evaluator


def evaluate(nav, case, action, delay=0):
    return _evaluator(nav).evaluate(case, action, delay)


def same_delay_optimum(nav, case, delay=0):
    return _evaluator(nav).optimum(case, delay)


def information_value(nav, case, delay=0):
    """Preposterior value of observing S2 early; no new instances/model requests.

    The original paired cell has one fixed S2 fixture and is not itself an
    unbiased information-value estimate. This diagnostic integrates all S2.
    """
    wall, cpu, native = time.perf_counter(), time.process_time(), native_cpu_s(nav)
    pending = copy.deepcopy(case)
    pending["information"] = "pending"
    baseline = same_delay_optimum(nav, pending, delay)
    s1_belief = posterior(case["s1"])
    rows = []
    for signal in Z_VALUES:
        informed = copy.deepcopy(case)
        informed.update(information="seen", s2=signal)
        p = sum(s1_belief[z] * likelihood(signal, z, .9) for z in Z_VALUES)
        optimum = same_delay_optimum(nav, informed, delay)
        rows.append({"s2": signal, "p": p, "choice": optimum["choice"],
                     "expected_value": optimum["expected_value"],
                     "action_values": optimum["action_values"], "timing": optimum["timing"]})
    early_value = sum(row["p"] * row["expected_value"] for row in rows)
    assert early_value >= baseline["expected_value"] - 1e-10
    return {"structure": case["structure"], "slack_s": case["slack_s"],
            "reversible": case["reversible"], "delay_s": delay,
            "pending_optimal_value": baseline["expected_value"],
            "early_information_value": early_value,
            "gain": early_value - baseline["expected_value"], "early_signal_rows": rows,
            "pending_search_timing": baseline["timing"],
            "timing": {"wall_s": time.perf_counter() - wall, "cpu_s": time.process_time() - cpu,
                       "native_cpu_s": native_cpu_delta(nav, native)},
            "scope": "preposterior finite-policy diagnostic, not extra independent cases"}


def realized_score(result, case):
    """Supplementary fixture score; never used to choose a public policy."""
    for b in result["branches"]:
        if b["z"] == case["judge_realized_z"] and b["s2"] == case["s2"] and (
                case["information"] == "seen" or b["arrival"] == case["judge_s2_arrival"]):
            return b["value"]
    raise AssertionError("Realized fixture not covered")


def audit_outcome(case, outcome):
    """Independent event-accounting check; does not call Ledger or Episode."""
    reserved, tranches = {}, {}
    spent = 0
    completed = {job: [] for job in case["jobs"]}
    rewards = {u["id"]: {z: 0. for z in Z_VALUES} for u in case["units"]}
    started = {}
    previous_tick = -1
    for event in outcome["events"]:
        tick, kind = event["tick"], event["kind"]
        assert tick >= previous_tick
        previous_tick = tick
        job_id = event.get("job")
        job = case["jobs"].get(job_id)
        if kind == "assigned":
            assert tick >= job["released_at"]
            assert event["unit"] == job["unit"] and job_id not in reserved
            request = sum(l["cost"] for l in job["legs"] if l["id"] not in completed[job_id])
            assert request > 0 and request <= case["initial_resource"] - spent - sum(reserved.values())
            reserved[job_id] = request
        elif kind == "service_started":
            index = len(completed[job_id])
            leg = job["legs"][index]
            assert event["leg"] == leg["id"] and event["unit"] == job["unit"]
            assert distance(event["pos"], leg["goal"]) <= RADIUS + 1e-9
            assert job_id not in tranches and reserved[job_id] >= 1
            reserved[job_id] -= 1
            spent += 1
            tranches[job_id] = 1
            started[(job_id, leg["id"])] = tick
        elif kind == "service_completed":
            leg = job["legs"][len(completed[job_id])]
            assert event["leg"] == leg["id"] and event["unit"] == job["unit"]
            start = started[(job_id, leg["id"])]
            assert tick - start == leg["service_s"]
            intervals = [c for c in outcome["continuous_service_checks"]
                         if c["job"] == job_id and c["leg"] == leg["id"] and
                         c["from"] >= start and c["to"] <= tick]
            assert sum(c["to"] - c["from"] for c in intervals) == leg["service_s"]
            assert tranches.pop(job_id) == 1
            reserved[job_id] -= leg["cost"] - 1
            spent += leg["cost"] - 1
            if reserved[job_id] == 0:
                del reserved[job_id]
            completed[job_id].append(leg["id"])
            assert event["on_time"] == (tick <= leg["deadline"])
            if tick <= leg["deadline"]:
                for z in Z_VALUES:
                    if leg["reward_condition"] in ("certain", z):
                        rewards[job["unit"]][z] += leg["reward"]
        elif kind == "cancelled":
            released = reserved.pop(job_id, 0)
            tranche = tranches.pop(job_id, 0)
            refund = tranche if case["reversible"] else 0
            spent -= refund
            assert (event["released"], event["refund"], event["sunk"]) == (released, refund, tranche-refund)
        expected_ledger = {"total": case["initial_resource"],
            "available": case["initial_resource"] - spent - sum(reserved.values()),
            "consumed": spent, "reserved": reserved.copy(),
            "refundable_unfinished_start": tranches.copy()}
        assert expected_ledger == event["resource"]
        assert expected_ledger["available"] >= 0 and spent >= 0 and all(v >= 0 for v in reserved.values())
    assert expected_ledger == outcome["resource"]
    assert completed == outcome["completed"] and rewards == outcome["raw_rewards_by_unit"]
    for check in outcome["continuous_service_checks"]:
        assert check["max_radius_nm"] <= RADIUS + 1e-9
        assert check["native_frames"] == check["to"] - check["from"] + 1
    returned = {u["id"]: distance(outcome["final_navigation"][u["id"]]["pos"], u["home"]) <= RADIUS + 1e-9
                for u in case["units"]}
    assert returned == outcome["returned"]
    values = {z: sum(r[z] for unit, r in rewards.items() if returned[unit]) for z in Z_VALUES}
    assert values == outcome["rewards_by_z"]
    payload = {k: v for k, v in outcome.items() if k != "trajectory_sha256"}
    assert digest(payload) == outcome["trajectory_sha256"]
    return {"events": len(outcome["events"]), "service_intervals": len(outcome["continuous_service_checks"]),
            "native_service_frames": sum(c["native_frames"] for c in outcome["continuous_service_checks"]),
            "max_service_radius_nm": max((c["max_radius_nm"] for c in outcome["continuous_service_checks"]), default=0),
            "independent_resource_reward_return_recompute": True}


def audit_result(case, result):
    checks = [audit_outcome(case, outcome) for outcome in result["candidate_outcomes"].values()]
    assert abs(sum(b["p"] for b in result["branches"]) - 1.) < 1e-12
    assert abs(sum(b["p"] * b["value"] for b in result["branches"]) - result["expected_value"]) < 1e-12
    if result["information_policy"] == "public_posterior":
        for signal in Z_VALUES:
            choices = {b["continuation"] for b in result["branches"] if b["s2"] == signal}
            assert len(choices) <= 1
    return {"candidate_count": len(checks), "checks": checks, "posterior_normalized": True,
            "expected_score_recomputed": True}


def audit_timely(nav, case):
    """Each job physically feasible when sent at its release from common initial state."""
    rows = []
    for job_id, job in case["jobs"].items():
        state = nav.load(case["initial_worlds"][job["unit"]])
        state = nav.advance(state, seconds=job["released_at"], stop=True, until_arrival=False)
        for leg in job["legs"]:
            state = nav.advance(state, leg["goal"], seconds=case["horizon"] - state["tick"])
            assert distance(state["pos"], leg["goal"]) <= RADIUS + 1e-9
            path = nav.segment(state, seconds=leg["service_s"], stop=True, until_arrival=False)
            assert all(distance(s["pos"], leg["goal"]) <= RADIUS + 1e-9 for s in path)
            state = path[-1]
            assert state["tick"] <= leg["deadline"]
        home = next(u["home"] for u in case["units"] if u["id"] == job["unit"])
        end = nav.advance(state, home, seconds=case["horizon"] - state["tick"])
        assert distance(end["pos"], home) <= RADIUS + 1e-9
        rows.append({"job": job_id, "completion": state["tick"], "return": end["tick"],
                     "scope": "isolated physical feasibility from common initial state at release"})
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--smoke", action="store_true", help="Only first four cells")
    args = parser.parse_args()
    from real_nav import RealNav
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=False)
    with RealNav(args.binary) as nav:
        cases = build_cases(nav)
        if args.smoke:
            cases = cases[:4]
        (out / "cases.json").write_text(json.dumps(cases, ensure_ascii=False, indent=2), encoding="utf-8")
        (out / "public-inputs.json").write_text(json.dumps([
            {"case_id": c["case_id"], "input": public_input(c)} for c in cases],
            ensure_ascii=False, indent=2), encoding="utf-8")
        rows = []
        audits = []
        for case in cases:
            optimum = same_delay_optimum(nav, case)
            evaluations = optimum.pop("evaluations")
            # Full-future diagnostic can choose initial actions separately for each
            # latent branch. It is explicitly outside public observation permission.
            judge = {a: _evaluator(nav).evaluate(case, a, 0, True) for a in actions(case)}
            future_upper = sum(branch["p"] * max(judge[a]["branches"][i]["value"]
                for a in actions(case)) for i, branch in enumerate(next(iter(judge.values()))["branches"]))
            row = {"case_id": case["case_id"], "public_reference": optimum,
                   "hold_resources": evaluations["wait"]["expected_value"],
                   "immediate_A": evaluations["A"]["expected_value"],
                   "judge_future_upper": future_upper,
                   "realized_scores": {a: realized_score(r, case) for a, r in evaluations.items()}}
            standing = Episode(nav, case)
            standing.record("standing_maintained")
            standing.advance(case["horizon"])
            standing_outcome = standing.outcome()
            audit_outcome(case, standing_outcome)
            row["maintain_standing_without_continuation"] = 0.
            audits.append({"case_id": case["case_id"], "timely": audit_timely(nav, case),
                           "actions": {a: audit_result(case, result) for a, result in evaluations.items()},
                           "judge_actions": {a: audit_result(case, result) for a, result in judge.items()}})
            (out / (case["case_id"] + ".json")).write_text(json.dumps({
                "summary": row, "evaluations": evaluations, "judge_evaluations": judge,
                "standing_outcome": standing_outcome},
                ensure_ascii=False, indent=2), encoding="utf-8")
            rows.append(row)
            print(case["case_id"], optimum["choice"], optimum["action_values"], flush=True)
        # Every first-action value and selected trajectory is restored in a fresh
        # native process; CPU counters are intentionally excluded from identity.
        with RealNav(args.binary) as replay_nav:
            replay_checks = []
            for case in cases:
                for action in actions(case):
                    first = evaluate(nav, case, action)
                    replay = evaluate(replay_nav, case, action)
                    assert first["expected_value"] == replay["expected_value"]
                    assert first["branches"] == replay["branches"]
                    assert first["candidate_outcomes"] == replay["candidate_outcomes"]
                    replay_checks.append({"case_id": case["case_id"], "action": action,
                                          "exact_world_trajectory_match": True})
        (out / "audit.json").write_text(json.dumps({"cells": audits, "replay": replay_checks,
            "all_passed": True, "cpu_clock_scope": "cpu_s=Python process; native_cpu_s=Windows child CPU; wall_s includes IPC",
            "reference_clock": "paused_quality_reference"}, ensure_ascii=False, indent=2), encoding="utf-8")
        information_checks = [information_value(nav, case) for case in cases if case["information"] == "seen"]
        (out / "information-value.json").write_text(json.dumps(information_checks,
            ensure_ascii=False, indent=2), encoding="utf-8")
        manifest = {"version": VERSION, "case_count": len(cases), "rows": rows,
                    "engine_binary": str(Path(args.binary).resolve()),
                    "source_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                    "native_requests": nav.requests, "native_wall_s": nav.wall_s,
                    "scoring": "posterior expectation; all native candidate paths retained",
                    "reference_clock": "paused_quality_reference",
                    "scope": "finite action diagnostic; three development templates, not independent scenes"}
        (out / "summary.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()

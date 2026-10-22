"""M3b R1: frozen, scripted wake scheduling under replace/append semantics.

Task/service rules in this module are fictional experimental assumptions. All
movement comes from the injected navigation adapter; the CLI requires RealNav.
The public planner sees only tasks already released. It projects the shared
standing executor through the declared response duration, then exhaustively
chooses a route over the remaining public tasks. No future release is passed to
that planner. Scripted response time is not an LLM reasoning-budget treatment.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import itertools
import json
import math
import time
from collections import defaultdict, deque
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any


VERSION = "m3b-r1-v1"
RADIUS = 0.12
SERVICE_SECONDS = 30
HORIZON = 2400
ARMS = ("GF60", "GF300", "GF600", "EH", "Q30", "Q-S", "Public", "hold")


def stable_hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=False).encode()).hexdigest()


def public_state(state: dict) -> dict:
    return {key: copy.deepcopy(state[key]) for key in
            ("tick", "pos", "course_deg", "speed_kts") if key in state}


def distance(state: dict, goal: dict) -> float:
    return math.hypot(state["pos"]["x"] - goal["x"], state["pos"]["y"] - goal["y"])


@dataclass
class Task:
    id: str
    release: int
    deadline: int
    goal: dict
    service_s: int = SERVICE_SECONDS
    weight: float = 1.0
    revision: int = 1
    status: str = "active"
    service_progress: int = 0
    completed_at: int | None = None
    canceled_at: int | None = None
    expired_at: int | None = None


class TaskBook:
    """Public task ledger: cancellation never subtracts completed reward."""
    def __init__(self, semantics: str):
        if semantics not in ("replace", "append"):
            raise ValueError("unknown task semantics")
        self.semantics = semantics
        self.tasks: dict[str, Task] = {}

    def publish(self, specification: dict, now: int) -> list[dict]:
        if specification["release"] != now:
            raise ValueError("release must occur at its exact public timestamp")
        if specification["id"] in self.tasks:
            raise ValueError("task identity cannot be reused")
        changes = []
        if self.semantics == "replace":
            for task in self.tasks.values():
                if task.status == "active":
                    task.status = "canceled"
                    task.canceled_at = now
                    changes.append({"kind": "canceled", "task": task.id, "tick": now})
        task = Task(**copy.deepcopy(specification))
        self.tasks[task.id] = task
        changes.append({"kind": "released", "task": task.id, "tick": now})
        return changes

    def expire(self, now: int) -> list[dict]:
        changes = []
        for task in self.tasks.values():
            # Completion exactly at the deadline is legal.
            if task.status == "active" and now > task.deadline:
                task.status = "expired"
                task.expired_at = now
                changes.append({"kind": "expired", "task": task.id, "tick": now})
        return changes

    def active(self) -> list[Task]:
        return [t for t in self.tasks.values() if t.status == "active"]

    def score(self) -> float:
        return sum(t.weight for t in self.tasks.values() if t.status == "completed")

    def observation(self) -> list[dict]:
        return [asdict(self.tasks[k]) for k in sorted(self.tasks)]

    def validate_reply(self, plan: list[str], revisions: dict[str, int]) -> tuple[list[str], list[dict]]:
        """Validate one sequential single-vessel route and commit it atomically.

        IDs are examined in a fixed sorted order for membership/duplicate
        validation, then the selected route order is retained. These tasks have
        one vessel and no consumable resource or prerequisite conflicts.
        """
        accepted, rejected = set(), []
        for task_id in sorted(set(plan)):
            task = self.tasks.get(task_id)
            reason = None
            if plan.count(task_id) != 1:
                reason = "duplicate_task"
            elif task is None or task_id not in revisions:
                reason = "unknown_dependency"
            elif task.revision != revisions[task_id]:
                reason = "task_revision_changed"
            elif task.status != "active":
                reason = "task_" + task.status
            if reason:
                rejected.append({"task": task_id, "reason": reason})
            else:
                accepted.add(task_id)
        return [task_id for task_id in plan if task_id in accepted], rejected


class Executor:
    """Shared low-level route executor using exact native navigation segments.

    Service needs consecutive stopped-command ticks inside the circle. Actual
    velocity/turning are native engine outcomes. Leaving the circle resets the
    service clock and triggers the same low-level return-to-goal controller for
    every arm. No task is chosen by this executor.
    """
    def __init__(self, nav, state: dict, book: TaskBook, horizon: int = HORIZON):
        self.nav, self.state, self.book, self.horizon = nav, state, book, horizon
        self.route: list[str] = []
        self.buffer: deque = deque()
        self.buffer_key = None
        self.commands: list[dict] = []

    def set_route(self, route: list[str]) -> None:
        old_head = self.route[0] if self.route else None
        new_head = route[0] if route else None
        self.route = list(route)
        if old_head != new_head:
            self.buffer.clear()
            self.buffer_key = None
            if old_head in self.book.tasks:
                self.book.tasks[old_head].service_progress = 0

    def step(self) -> tuple[dict | None, dict | None]:
        now = int(self.state["tick"])
        while self.route and self.book.tasks[self.route[0]].status != "active":
            self.route.pop(0)
        task = self.book.tasks[self.route[0]] if self.route else None
        serving = bool(task and distance(self.state, task.goal) <= RADIUS)
        mode = "service" if serving else ("travel" if task else "idle")
        key = (task.id if task else None, mode)
        command = None
        if key != self.buffer_key or not self.buffer:
            if mode == "travel":
                states = self.nav.segment(self.state, goal=task.goal,
                                          seconds=self.horizon-now, until_arrival=True)
                command = {"tick": now, "effective_tick": now + 1,
                           "kind": "goto", "task": task.id, "goal": task.goal}
            else:
                length = (task.service_s - task.service_progress) if task else self.horizon-now
                states = self.nav.segment(self.state, seconds=max(1, length), stop=True)
                command = {"tick": now, "effective_tick": now + 1,
                           "kind": "stop", "task": task.id if task else None}
            if len(states) < 2:
                raise RuntimeError("native segment failed to advance one tick")
            self.buffer = deque(states[1:])
            self.buffer_key = key
            self.commands.append(command)
        self.state = self.buffer.popleft()
        if int(self.state["tick"]) != now + 1:
            raise AssertionError("navigation segment skips a simulation second")
        service = None
        if task:
            inside = distance(self.state, task.goal) <= RADIUS
            if serving and inside:
                task.service_progress += 1
                service = {"task": task.id, "progress": task.service_progress,
                           "inside": True, "stop_command_active": True}
                if task.service_progress >= task.service_s and self.state["tick"] <= task.deadline:
                    task.status = "completed"
                    task.completed_at = int(self.state["tick"])
                    service["completed"] = True
            elif not inside:
                task.service_progress = 0
        return service, command


def clone_public_book(book: TaskBook) -> TaskBook:
    result = TaskBook(book.semantics)
    result.tasks = copy.deepcopy(book.tasks)
    return result


def project_standing(nav, state: dict, book: TaskBook, route: list[str], delay: int,
                     horizon: int) -> tuple[dict, TaskBook, list[str]]:
    projected = clone_public_book(book)
    executor = Executor(nav, state, projected, horizon)
    executor.route = list(route)
    end = min(horizon, int(state["tick"]) + delay)
    while executor.state["tick"] < end:
        projected.expire(int(executor.state["tick"]))
        executor.step()
    projected.expire(int(executor.state["tick"]))
    return executor.state, projected, executor.route


def evaluate_route(nav, state: dict, book: TaskBook, route: list[str], horizon: int,
                   *, standing_route: list[str] | None = None) -> dict:
    simulated = clone_public_book(book)
    executor = Executor(nav, state, simulated, horizon)
    # Candidate submission must use exactly the same interruption transition as
    # an actual reply. Retaining a task's partial service after selecting another
    # head would otherwise give that candidate free work upon returning later.
    executor.route = list(standing_route or [])
    executor.set_route(route)
    base_score = simulated.score()
    last_completion = int(state["tick"])
    while executor.state["tick"] < horizon and any(
            simulated.tasks[task_id].status == "active" for task_id in route):
        simulated.expire(int(executor.state["tick"]))
        service, _ = executor.step()
        if service and service.get("completed"):
            last_completion = int(executor.state["tick"])
    return {"value": simulated.score() - base_score,
            "last_completion": last_completion,
            "completions": {t.id: t.completed_at for t in simulated.tasks.values()
                            if t.completed_at is not None and t.id in route}}


def plan_current(nav, state: dict, book: TaskBook, standing_route: list[str], delay: int,
                 horizon: int = HORIZON) -> dict:
    """Finite public optimum with known latency; no future-event argument exists."""
    started = time.perf_counter()
    projected_state, projected_book, projected_route = project_standing(
        nav, state, book, standing_route, delay, horizon)
    ids = sorted(t.id for t in projected_book.active())
    candidates = []
    for size in range(len(ids) + 1):
        for route in itertools.permutations(ids, size):
            result = evaluate_route(nav, projected_state, projected_book, list(route), horizon,
                                    standing_route=projected_route)
            candidates.append({"route": list(route), **result})
    # Reward first, earlier completion next, then fewer commitments and stable IDs.
    best = min(candidates, key=lambda c: (-c["value"], c["last_completion"],
                                          len(c["route"]), tuple(c["route"])))
    return {**best, "examined": len(candidates), "cpu_seconds": time.perf_counter() - started,
            "projected_tick": projected_state["tick"],
            "projection_score": projected_book.score(),
            "dependencies": {task.id: task.revision for task in book.active()}}


def service_reference(nav, state: dict, specification: dict, horizon: int = HORIZON) -> int:
    task = {**copy.deepcopy(specification), "release": int(state["tick"]), "deadline": horizon}
    book = TaskBook("append")
    book.publish(task, int(state["tick"]))
    result = evaluate_route(nav, state, book, [task["id"]], horizon)
    if task["id"] not in result["completions"]:
        raise RuntimeError("zero-inference physical reference is infeasible")
    return result["completions"][task["id"]] - int(state["tick"])


def quiet_bound(slack: float, lref: int = 100) -> int:
    return max(0, min(30, math.floor(slack - lref - 1)))


class WakeSchedule:
    def __init__(self, arm: str, lref: int = 100):
        if arm not in ARMS:
            raise ValueError("unknown arm")
        self.arm, self.lref = arm, lref
        self.pending = False
        self.due: int | None = None
        self.safe_cap: int | None = None
        self.grid_latched = False
        self.notes: list[dict] = []

    def changed(self, now: int, slacks: list[float]) -> None:
        self.pending = True
        if self.arm == "Q30":
            self.due = now + 30
        elif self.arm == "Q-S":
            bound = min((quiet_bound(s, self.lref) for s in slacks), default=0)
            proposed_cap = now + bound
            self.safe_cap = proposed_cap if self.safe_cap is None else min(self.safe_cap, proposed_cap)
            self.due = min(now + 30, self.safe_cap)
        else:
            self.due = now
        self.notes.append({"tick": now, "kind": "public_change", "due": self.due,
                           "safe_cap": self.safe_cap, "slacks": slacks})

    def should_call(self, now: int, dirty: bool, busy: bool) -> bool:
        if self.arm == "hold":
            return False
        if self.arm.startswith("GF"):
            period = int(self.arm[2:])
            if not dirty:
                # A pending grid tick covered by the returned plan is consumed;
                # it must not turn a later off-grid event into an immediate wake.
                self.grid_latched = False
            if now > 0 and now % period == 0 and dirty:
                self.grid_latched = True
                self.notes.append({"tick": now, "kind": "grid", "busy": busy})
            return dirty and self.grid_latched and not busy
        return dirty and self.pending and self.due is not None and now >= self.due and not busy

    def sent(self, now: int) -> None:
        self.pending = False
        self.due = None
        self.safe_cap = None
        self.grid_latched = False
        self.notes.append({"tick": now, "kind": "sent"})


def public_slacks(nav, state: dict, book: TaskBook, horizon: int,
                  standing_route: list[str] | None = None) -> list[float]:
    now = int(state["tick"])
    slacks = []
    for task in book.active():
        result = evaluate_route(nav, state, book, [task.id], horizon,
                                standing_route=standing_route)
        complete = result["completions"].get(task.id)
        if complete is not None:
            slacks.append(task.deadline - complete)
        else:
            # The expiry gate is common; an already infeasible public task has
            # no safe waiting margin. This does not inspect hidden future tasks.
            slacks.append(-1.0)
    return slacks


def make_cases(nav) -> tuple[list[dict], dict]:
    initial = nav.initial({"x": 0.0, "y": 0.0}, course=90)
    goals = ({"x": 1.5, "y": 0.6}, {"x": 1.65, "y": -0.6})
    references = [service_reference(nav, initial, {"id": f"T{i+1}", "goal": goal})
                  for i, goal in enumerate(goals)]
    cases = []
    for semantics, gap, margin, phase in itertools.product(
            ("replace", "append"), (30, 180), (120, 240), (17, 83, 251, 419)):
        tasks = [{"id": f"T{i+1}", "release": phase + i * gap,
                  "deadline": phase + i * gap + references[i] + margin,
                  "goal": goal, "service_s": SERVICE_SECONDS, "weight": 1.0}
                 for i, goal in enumerate(goals)]
        assert all(t["deadline"] < HORIZON for t in tasks)
        cases.append({"id": f"{semantics}-g{gap}-m{margin}-p{phase}",
                      "semantics": semantics, "gap": gap, "margin": margin, "phase": phase,
                      "horizon": HORIZON, "tasks": tasks,
                      "physical_reference_s": references,
                      "first_completable_before_second": references[0] < gap,
                      "first_completable_at_or_before_second": references[0] <= gap})
    return cases, {"initial": public_state(initial), "reference_seconds": references,
                   "initial_world_hash": nav.fingerprint(initial)}


def run_episode(nav, case: dict, arm: str, response_s: int = 100, *,
                backend: str = "scripted", lref: int = 100) -> dict:
    if backend != "scripted":
        raise ValueError("R1 only supports scripted latency; real providers are forbidden")
    if response_s < 0:
        raise ValueError("negative latency")
    if arm not in ARMS:
        raise ValueError("unknown arm")
    horizon = case.get("horizon", HORIZON)
    if any(t["deadline"] >= horizon for t in case["tasks"]):
        raise ValueError("all deadlines must precede the common horizon")
    state = nav.initial({"x": 0.0, "y": 0.0}, course=90)
    book = TaskBook(case["semantics"])
    executor = Executor(nav, state, book, horizon)
    scheduler = WakeSchedule(arm, lref)
    reviewed: dict[str, int] = {}
    requests, events, trace = [], [], []
    inflight = None
    online_count, local_decisions = 0, 0
    service_from_previous = None
    command_from_previous = None
    scheduler_cpu = 0.0
    # This private index belongs only to the environment loop.
    releases = defaultdict(list)
    for task in case["tasks"]:
        releases[task["release"]].append(task)
    while True:
        now = int(executor.state["tick"])
        if service_from_previous and service_from_previous.get("completed"):
            events.append({"tick": now, "kind": "completed", "task": service_from_previous["task"]})
        changed = False
        for specification in releases.get(now, []):
            events.extend(book.publish(specification, now))
            changed = True
        events.extend(book.expire(now))
        if changed:
            started = time.perf_counter()
            slacks = public_slacks(nav, executor.state, book, horizon,
                                  executor.route) if arm == "Q-S" else []
            scheduler.changed(now, slacks)
            scheduler_cpu += time.perf_counter() - started

        def consume_reply(request: dict) -> None:
            nonlocal inflight
            accepted, rejected = book.validate_reply(request["plan"]["route"], request["plan"]["dependencies"])
            executor.set_route(accepted)
            for task_id, revision in request["plan"]["dependencies"].items():
                if task_id in book.tasks and book.tasks[task_id].revision == revision:
                    reviewed[task_id] = revision
            request.update({"reply_tick": now, "commit_tick": now,
                            "command_effective_tick": now + 1, "accepted": accepted,
                            "rejected": rejected, "latest_public_tasks": book.observation()})
            events.append({"tick": now, "kind": "reply", "request": request["id"],
                           "accepted": accepted, "rejected": rejected})
            inflight = None

        if inflight is not None and now == inflight["scheduled_reply_tick"]:
            consume_reply(inflight)
        dirty = any(reviewed.get(task.id) != task.revision for task in book.active())
        should_call = scheduler.should_call(now, dirty, inflight is not None)
        within_budget = arm == "Public" or online_count < 5
        if should_call and within_budget and now < horizon:
            snapshot = {"tick": now, "vessel": public_state(executor.state),
                        "tasks": book.observation(), "standing_route": list(executor.route),
                        "response_s": 0 if arm == "Public" else response_s}
            duration = snapshot["response_s"]
            plan = plan_current(nav, executor.state, book, executor.route, duration, horizon)
            request = {"id": len(requests) + 1, "start_tick": now,
                       "scheduled_reply_tick": now + duration, "snapshot": snapshot,
                       "snapshot_sha256": stable_hash(snapshot), "plan": plan,
                       "kind": "local_public" if arm == "Public" else "scripted"}
            requests.append(request)
            if arm == "Public":
                local_decisions += 1
            else:
                online_count += 1
            scheduler.sent(now)
            inflight = request
            if duration == 0:
                consume_reply(request)
        trace.append({"tick": now, "vessel": public_state(executor.state),
                      "route": list(executor.route), "service": service_from_previous,
                      "command_from_previous": command_from_previous,
                      "tasks": {k: {"status": t.status, "progress": t.service_progress}
                                for k, t in sorted(book.tasks.items())},
                      "score": book.score()})
        if now == horizon:
            break
        service_from_previous, command_from_previous = executor.step()
    total_weight = sum(t["weight"] for t in case["tasks"])
    result = {"version": VERSION, "case": copy.deepcopy(case), "arm": arm,
              "response_s": response_s, "lref": lref, "backend": "scripted",
              "score": book.score(), "normalized_score": book.score() / total_weight,
              "published_weight": total_weight, "online_calls": online_count,
              "logical_initialization_slots": 1, "total_logical_slots": online_count + 1,
              "local_public_decisions": local_decisions,
              "planner_cpu_seconds": sum(r["plan"]["cpu_seconds"] for r in requests),
              "scheduler_cpu_seconds": scheduler_cpu, "tasks": book.observation(),
              "requests": requests, "events": events, "scheduler": scheduler.notes,
              "commands": executor.commands, "trace": trace,
              "final_world_hash": nav.fingerprint(executor.state),
              "final_state": public_state(executor.state)}
    result["audit"] = audit_episode(result)
    if not result["audit"]["passed"]:
        raise AssertionError(result["audit"])
    return result


def audit_episode(result: dict) -> dict:
    """Recompute lifetimes and cumulative rewards from poses and service marks.

    Does not use TaskBook, executor completion flags, recorded scores, or planner
    output to decide whether reward was earned. Native kinematics are separately
    checked by replay/hash; this audit checks the experimental task layer.
    """
    errors = []
    specs = {t["id"]: t for t in result["case"]["tasks"]}
    records: dict[str, dict] = {}
    scored = 0.0
    trace = result["trace"]
    expected_horizon = result["case"].get("horizon", HORIZON)
    if [row["tick"] for row in trace] != list(range(expected_horizon + 1)):
        errors.append("trace does not cover the exact common horizon")
    active_command = None
    previous_vessel = None
    for row in trace:
        now = row["tick"]
        if row["command_from_previous"] is not None:
            active_command = row["command_from_previous"]
            if active_command["effective_tick"] != now or active_command["tick"] != now - 1:
                errors.append(f"command boundary mismatch at {now}")
        # A service tick ending at T is adjudicated before a release at T.
        for task_id, record in records.items():
            if record["status"] != "active":
                continue
            task = specs[task_id]
            matches = bool(active_command and active_command["kind"] == "stop" and
                           active_command["task"] == task_id and previous_vessel is not None and
                           distance(previous_vessel, task["goal"]) <= RADIUS and
                           distance(row["vessel"], task["goal"]) <= RADIUS)
            record["progress"] = record["progress"] + 1 if matches else 0
            if record["progress"] >= task.get("service_s", SERVICE_SECONDS) and now <= task["deadline"]:
                record["status"] = "completed"
                record["completed_at"] = now
                scored += task.get("weight", 1.0)
        for task in specs.values():
            if task["release"] == now:
                if result["case"]["semantics"] == "replace":
                    for record in records.values():
                        if record["status"] == "active":
                            record["status"] = "canceled"
                records[task["id"]] = {"status": "active", "progress": 0, "completed_at": None}
        for task_id, record in records.items():
            if record["status"] == "active" and now > specs[task_id]["deadline"]:
                record["status"] = "expired"
        if abs(scored - row["score"]) > 1e-9:
            errors.append(f"score mismatch at {now}")
        if {k: v["status"] for k, v in records.items()} != {k: v["status"] for k, v in row["tasks"].items()}:
            errors.append(f"task status mismatch at {now}")
        previous_vessel = row["vessel"]
    if scored != result["score"]:
        errors.append("final score mismatch")
    final = {task["id"]: task for task in result["tasks"]}
    for task_id, record in records.items():
        if final[task_id]["completed_at"] != record["completed_at"]:
            errors.append("completion timestamp mismatch: " + task_id)
    last_due = -1
    for request in result["requests"]:
        if request["start_tick"] < last_due:
            errors.append("more than one request in flight")
        last_due = request["scheduled_reply_tick"]
        if stable_hash(request["snapshot"]) != request["snapshot_sha256"]:
            errors.append("snapshot hash mismatch")
        if any(t["release"] > request["start_tick"] for t in request["snapshot"]["tasks"]):
            errors.append("future task leaked into request")
        if request.get("reply_tick") != request["scheduled_reply_tick"]:
            errors.append("request did not finish at declared latency")
        if request.get("command_effective_tick") != request.get("reply_tick", -2) + 1:
            errors.append("execution boundary mismatch")
    if result["online_calls"] > 5 or result["total_logical_slots"] > 6:
        errors.append("request budget exceeded")
    return {"passed": not errors, "errors": errors, "recomputed_score": scored,
            "recomputed_tasks": records, "checked_ticks": len(trace),
            "checked_requests": len(result["requests"])}


def deterministic_view(result: dict) -> dict:
    result = copy.deepcopy(result)
    result.pop("planner_cpu_seconds", None)
    result.pop("scheduler_cpu_seconds", None)
    for request in result["requests"]:
        request["plan"].pop("cpu_seconds", None)
    return result


def summarize(results: list[dict]) -> dict:
    groups = defaultdict(list)
    for result in results:
        key = (result["response_s"], result["case"]["semantics"], result["arm"])
        groups[key].append(result)
    summary = []
    for (response, semantics, arm), rows in sorted(groups.items()):
        summary.append({"response_s": response, "semantics": semantics, "arm": arm,
                        "episodes": len(rows), "score": sum(r["score"] for r in rows),
                        "normalized_score_mean": sum(r["normalized_score"] for r in rows) / len(rows),
                        "calls": sum(r["online_calls"] for r in rows),
                        "public_decisions": sum(r["local_public_decisions"] for r in rows)})
    lookup = {(r["case"]["id"], r["response_s"], r["arm"]): r for r in results}
    paired = []
    for result in results:
        if result["arm"] in ("Q30", "Q-S"):
            other = lookup[(result["case"]["id"], result["response_s"], "EH")]
            paired.append({"case": result["case"]["id"], "semantics": result["case"]["semantics"],
                           "gap": result["case"]["gap"], "margin": result["case"]["margin"],
                           "phase": result["case"]["phase"], "response_s": result["response_s"],
                           "contrast": result["arm"] + "-EH",
                           "normalized_score_difference": result["normalized_score"] - other["normalized_score"],
                           "call_difference": result["online_calls"] - other["online_calls"]})
    return {"version": VERSION, "episodes": len(results), "groups": summary, "paired": paired,
            "all_audits_passed": all(r["audit"]["passed"] for r in results),
            "no_llm_calls": True}


def run_matrix(nav, out: Path, *, replay: bool = True) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    if (out / "manifest.json").exists():
        raise FileExistsError("refusing to overwrite a frozen R1 batch")
    cases, calibration = make_cases(nav)
    schedule = [(case, arm, 100) for case in cases for arm in ARMS]
    schedule.extend((case, arm, latency) for case in cases if case["phase"] == 83
                    for latency in (30, 180) for arm in ("EH", "Q30", "Q-S"))
    assert len(schedule) == 304
    manifest = {"version": VERSION, "calibration": calibration, "cases": cases,
                "service": "30 consecutive stop-command ticks inside radius 0.12; reset outside",
                "scripted_responses": [100, 30, 180], "q_s_lref": 100,
                "latency_controls": "Lref stays 100: 30/180 include deliberate prediction mismatch",
                "planner": "finite current-public-task optimum after standing-plan latency projection",
                "tie_break": "max reward, earlier last completion, fewer tasks, lexical route",
                "event_order": "movement/service completion, public release, expiry, reply, wake, next tick",
                "initialization": "same ready stationary state; one logical slot; zero model tokens",
                "source_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                "schedule": [{"case": c["id"], "arm": a, "response_s": l} for c, a, l in schedule]}
    (out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    episode_dir = out / "episodes"
    episode_dir.mkdir()
    results, replay_checks = [], []
    for i, (case, arm, latency) in enumerate(schedule):
        result = run_episode(nav, case, arm, latency)
        name = f"{i:03d}-{case['id']}-{arm}-l{latency}.json"
        if replay:
            repeated = run_episode(nav, case, arm, latency)
            matches = deterministic_view(result) == deterministic_view(repeated)
            replay_checks.append({"episode": name, "passed": matches,
                                  "first_world": result["final_world_hash"],
                                  "replay_world": repeated["final_world_hash"]})
            if not matches:
                raise AssertionError("complete replay diverged: " + name)
        (episode_dir / name).write_text(json.dumps(result, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
        results.append({key: result[key] for key in (
            "case", "arm", "response_s", "score", "normalized_score", "online_calls",
            "local_public_decisions", "audit")})
        # Retain full per-episode evidence on disk, avoid growing memory across 304 traces.
        if i % 16 == 0:
            print(json.dumps({"completed": i + 1, "total": len(schedule), "case": case["id"],
                              "arm": arm, "score": result["score"]}), flush=True)
        # Each episode starts a fresh native World. Keeping Python copies of all
        # earlier segments cannot accelerate it and grows memory linearly.
        cache = getattr(nav, "cache", None)
        if isinstance(cache, dict):
            cache.clear()
    summary = summarize(results)
    summary["replays"] = replay_checks
    summary["all_replays_passed"] = bool(replay_checks) and all(r["passed"] for r in replay_checks)
    summary["manifest_sha256"] = hashlib.sha256((out / "manifest.json").read_bytes()).hexdigest()
    (out / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    return summary


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--backend", choices=("scripted",), default="scripted")
    parser.add_argument("--skip-replay", action="store_true", help="development only; acceptance requires replay")
    args = parser.parse_args()
    from real_nav import RealNav
    nav = RealNav(args.binary)
    try:
        summary = run_matrix(nav, args.out, replay=not args.skip_replay)
        print(json.dumps({"episodes": summary["episodes"], "all_audits_passed": summary["all_audits_passed"],
                          "all_replays_passed": summary["all_replays_passed"]}), flush=True)
    finally:
        close = getattr(nav, "close", None)
        if close:
            close()


if __name__ == "__main__":
    main()

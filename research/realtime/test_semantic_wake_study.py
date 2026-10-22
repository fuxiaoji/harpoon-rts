"""Task/scheduler tests. ToyNav is an explicit test double, never research physics."""
import copy
import math
import unittest

import semantic_wake_study as study


class ToyNav:
    """Deterministic test-only movement: one distance unit per simulation second."""
    def initial(self, pos, course=90):
        return {"tick": 0, "pos": dict(pos), "course_deg": course, "speed_kts": 0,
                "goal": None}

    def segment(self, state, goal=None, seconds=2400, stop=False, until_arrival=True):
        current = copy.deepcopy(state)
        result = [copy.deepcopy(current)]
        if goal is not None:
            current["goal"] = copy.deepcopy(goal)
        if stop:
            current["goal"] = None
        for _ in range(seconds):
            current = copy.deepcopy(current)
            current["tick"] += 1
            target = current.get("goal")
            if target is not None:
                dx = target["x"] - current["pos"]["x"]
                dy = target["y"] - current["pos"]["y"]
                length = math.hypot(dx, dy)
                if length:
                    current["pos"]["x"] += dx / length * min(1, length)
                    current["pos"]["y"] += dy / length * min(1, length)
                current["speed_kts"] = 1
            else:
                current["speed_kts"] = 0
            result.append(copy.deepcopy(current))
            if goal is not None and until_arrival and study.distance(current, goal) <= study.RADIUS:
                break
        return result

    def fingerprint(self, state):
        return study.stable_hash(state)


def spec(task_id="A", release=1, deadline=40, x=2, service_s=2, weight=1):
    return {"id": task_id, "release": release, "deadline": deadline,
            "goal": {"x": x, "y": 0}, "service_s": service_s, "weight": weight}


def case(semantics="append", tasks=None, horizon=100):
    return {"id": "test", "semantics": semantics,
            "tasks": tasks or [spec(), spec("B", release=3, deadline=60, x=6)],
            "horizon": horizon}


class TaskSemanticsTests(unittest.TestCase):
    def test_candidate_route_switch_resets_interrupted_service_like_executor(self):
        nav = ToyNav()
        state = nav.initial({"x": 0, "y": 0})
        book = study.TaskBook("append")
        book.publish(spec("A", release=0, deadline=4, x=0, service_s=4), 0)
        book.publish(spec("B", release=0, deadline=10, x=1, service_s=1), 0)
        book.tasks["A"].service_progress = 3
        predicted = study.evaluate_route(nav, state, book, ["B", "A"], 20,
                                         standing_route=["A"])
        executed_book = study.clone_public_book(book)
        executor = study.Executor(nav, state, executed_book, 20)
        executor.route = ["A"]
        executor.set_route(["B", "A"])
        while executor.state["tick"] < 20:
            executed_book.expire(executor.state["tick"])
            executor.step()
        self.assertEqual(executed_book.score(), 1)
        self.assertEqual(predicted["value"], executed_book.score())
        continued = study.evaluate_route(nav, state, book, ["A", "B"], 20,
                                         standing_route=["A"])
        self.assertEqual(continued["value"], 2)
        self.assertEqual(continued["completions"]["A"], 1)

    def test_service_drift_resets_clock_and_shared_executor_returns_to_goal(self):
        class OneDriftNav(ToyNav):
            def segment(self, state, goal=None, seconds=2400, stop=False, until_arrival=True):
                result = super().segment(state, goal, seconds, stop, until_arrival)
                if stop and not state.get("drifted"):
                    result[1]["pos"]["x"] += 1
                    result[1]["drifted"] = True
                    return result[:2]
                return result
        nav = OneDriftNav()
        book = study.TaskBook("append")
        book.publish(spec(release=0, service_s=3), 0)
        book.tasks["A"].service_progress = 2
        executor = study.Executor(nav, nav.initial({"x": 2, "y": 0}), book, 100)
        executor.route = ["A"]
        executor.step()
        self.assertEqual(book.tasks["A"].service_progress, 0)
        self.assertEqual(book.tasks["A"].status, "active")
        executor.step()
        self.assertEqual(executor.commands[-1]["kind"], "goto")
        for _ in range(3):
            executor.step()
        self.assertEqual(book.tasks["A"].completed_at, 5)

    def test_replace_preserves_earned_reward(self):
        result = study.run_episode(ToyNav(), case("replace", [spec(deadline=10),
                                  spec("B", release=8, deadline=30, x=4)]), "Public")
        self.assertEqual(result["score"], 2)
        self.assertEqual([t["status"] for t in result["tasks"]], ["completed", "completed"])
        self.assertTrue(result["audit"]["passed"])

    def test_append_accepts_old_task_reply_and_recomputes_new_task(self):
        result = study.run_episode(ToyNav(), case(), "EH", response_s=5)
        first = result["requests"][0]
        self.assertEqual(first["accepted"], ["A"])
        self.assertEqual(first["rejected"], [])
        self.assertEqual([t["id"] for t in first["snapshot"]["tasks"]], ["A"])
        self.assertEqual([t["id"] for t in first["latest_public_tasks"]], ["A", "B"])
        self.assertEqual(result["online_calls"], 2)

    def test_replace_rejects_canceled_dependency_without_rejecting_new_plan(self):
        result = study.run_episode(ToyNav(), case("replace"), "EH", response_s=5)
        first, second = result["requests"]
        self.assertEqual(first["accepted"], [])
        self.assertEqual(first["rejected"], [{"task": "A", "reason": "task_canceled"}])
        self.assertEqual(second["accepted"], ["B"])
        self.assertEqual(result["score"], 1)

    def test_completion_on_deadline_is_inclusive(self):
        # Released T1, travel T2/T3, service T4/T5.
        result = study.run_episode(ToyNav(), case(tasks=[spec(deadline=5)]), "Public")
        self.assertEqual(result["score"], 1)
        self.assertEqual(result["tasks"][0]["completed_at"], 5)
        failed = study.run_episode(ToyNav(), case(tasks=[spec(deadline=4)]), "Public")
        self.assertEqual(failed["score"], 0)
        self.assertEqual(failed["tasks"][0]["status"], "expired")

    def test_duplicate_route_task_is_not_accepted_twice(self):
        book = study.TaskBook("append")
        book.publish(spec(), 1)
        accepted, rejected = book.validate_reply(["A", "A"], {"A": 1})
        self.assertEqual(accepted, [])
        self.assertEqual(rejected[0]["reason"], "duplicate_task")

    def test_partial_reply_preserves_legal_appended_task(self):
        book = study.TaskBook("append")
        book.publish(spec(), 1)
        book.publish(spec("B", release=3), 3)
        book.tasks["A"].status = "expired"
        accepted, rejected = book.validate_reply(["B", "A"], {"A": 1, "B": 1})
        self.assertEqual(accepted, ["B"])
        self.assertEqual(rejected, [{"task": "A", "reason": "task_expired"}])

    def test_common_horizon_kept_after_success_and_hold(self):
        for arm in ("Public", "hold"):
            result = study.run_episode(ToyNav(), case(tasks=[spec()]), arm)
            self.assertEqual(len(result["trace"]), 101)
            self.assertEqual(result["final_state"]["tick"], 100)

    def test_no_future_task_can_enter_first_request(self):
        first_case = case(tasks=[spec(), spec("B", release=20, deadline=80, x=8)])
        second_case = copy.deepcopy(first_case)
        second_case["tasks"][1]["goal"] = {"x": 30, "y": -10}
        left = study.run_episode(ToyNav(), first_case, "EH", response_s=5)["requests"][0]
        right = study.run_episode(ToyNav(), second_case, "EH", response_s=5)["requests"][0]
        self.assertEqual(left["snapshot"], right["snapshot"])
        self.assertEqual(left["plan"]["route"], right["plan"]["route"])


class SchedulingTests(unittest.TestCase):
    def test_fixed_phase_grid_is_absolute_and_gate_prevents_idle_calls(self):
        tasks = [spec(release=17, deadline=180)]
        result = study.run_episode(ToyNav(), case(tasks=tasks, horizon=240), "GF60", response_s=5)
        self.assertEqual([r["start_tick"] for r in result["requests"]], [60])
        self.assertEqual(result["online_calls"], 1)

    def test_quiet30_coalesces_exact_boundary_event(self):
        tasks = [spec(release=17, deadline=180), spec("B", release=47, deadline=200, x=4)]
        result = study.run_episode(ToyNav(), case(tasks=tasks, horizon=240), "Q30", response_s=5)
        self.assertEqual([r["start_tick"] for r in result["requests"]], [77])
        self.assertEqual(len(result["requests"][0]["snapshot"]["tasks"]), 2)

    def test_safe_quiet_cap_is_not_extended_by_later_event(self):
        schedule = study.WakeSchedule("Q-S", 100)
        schedule.changed(10, [120])
        self.assertEqual(schedule.due, 29)
        schedule.changed(20, [200])
        self.assertEqual(schedule.due, 29)
        self.assertFalse(schedule.should_call(28, True, False))
        self.assertTrue(schedule.should_call(29, True, False))

    def test_no_negative_wait_and_latency_mismatch_is_explicit(self):
        self.assertEqual(study.quiet_bound(80, 100), 0)
        self.assertEqual(study.quiet_bound(120, 100), 19)
        self.assertEqual(study.quiet_bound(240, 100), 30)

    def test_fixed_grid_event_while_busy_is_coalesced_once(self):
        schedule = study.WakeSchedule("GF60")
        self.assertFalse(schedule.should_call(60, True, True))
        self.assertTrue(schedule.should_call(83, True, False))
        schedule.sent(83)
        self.assertFalse(schedule.should_call(84, True, False))

    def test_covered_busy_grid_cannot_wake_a_future_off_grid_event(self):
        schedule = study.WakeSchedule("GF60")
        self.assertFalse(schedule.should_call(120, True, True))
        self.assertFalse(schedule.should_call(160, False, False))
        self.assertFalse(schedule.should_call(177, True, False))
        self.assertTrue(schedule.should_call(180, True, False))

    def test_provider_rejected_before_any_navigation(self):
        with self.assertRaisesRegex(ValueError, "providers are forbidden"):
            study.run_episode(None, None, "EH", backend="codex")


class AuditTests(unittest.TestCase):
    def test_audit_detects_forged_score_without_executor_flags(self):
        result = study.run_episode(ToyNav(), case(tasks=[spec()]), "Public")
        result["trace"][-1]["score"] = 0
        self.assertFalse(study.audit_episode(result)["passed"])

    def test_audit_detects_geometrically_invalid_service(self):
        result = study.run_episode(ToyNav(), case(tasks=[spec()]), "Public")
        result["trace"][5]["vessel"]["pos"] = {"x": 999, "y": 999}
        self.assertFalse(study.audit_episode(result)["passed"])

    def test_audit_recomputes_service_without_trusting_completion_markers(self):
        result = study.run_episode(ToyNav(), case(tasks=[spec()]), "Public")
        for row in result["trace"]:
            row["service"] = None
        self.assertTrue(study.audit_episode(result)["passed"])
        result["trace"][4]["command_from_previous"]["kind"] = "goto"
        self.assertFalse(study.audit_episode(result)["passed"])

    def test_complete_replay_ignores_only_measured_cpu(self):
        args = (case(), "EH", 5)
        first = study.run_episode(ToyNav(), *args)
        second = study.run_episode(ToyNav(), *args)
        self.assertEqual(study.deterministic_view(first), study.deterministic_view(second))


if __name__ == "__main__":
    unittest.main()

"""State-machine tests only: no model calls, services, or naval World access."""
import copy
import json
import unittest

import naval_scheduler as sched


def features(tick=0, **changes):
    value = {"tick": tick, "contact_revision": 0, "contacts_count": 0,
             "incoming_count": 0, "incoming_revision": 0, "own_loss_count": 0,
             "task_revision": 0, "plan_age_s": tick}
    value.update(changes)
    return value


def clause(field, op="gt", value=0):
    return {"all": [{"field": field, "op": op, "value": value}]}


class SchedulerTests(unittest.TestCase):
    def initial_request(self, policy, **options):
        state, audit = sched.observe(sched.new_state(policy, **options), features())
        self.assertEqual([r["reason"] for r in audit], ["initialization"])
        state, record = sched.start_request(state, "r0", features())
        self.assertEqual(record["requested_tick"], 0)
        self.assertTrue(record["initialization"])
        return state

    def initialized(self, policy, conditions=None, **options):
        state = self.initial_request(policy, **options)
        state, _ = sched.finish_request(state, "r0", features(), success=True,
                                        wake_conditions=conditions or [])
        return state

    def test_all_seven_arms_charge_one_common_t0_request(self):
        for policy in sched.POLICIES:
            with self.subTest(policy=policy):
                state = self.initial_request(policy)
                self.assertFalse(sched.is_ready(state))
                self.assertEqual(state["request_count"], 1)
                with self.assertRaises(ValueError):
                    sched.start_request(state, "r1", features())
                self.assertEqual(sched.restore_state(json.loads(json.dumps(state))), state)

    def test_absolute_fixed_phase_survives_events_and_reply_times(self):
        state = self.initial_request("F60", phase_s=17)
        state, _ = sched.finish_request(state, "r0", features(10), success=True)
        state, audit = sched.observe(state, features(16, contact_revision=1))
        self.assertEqual(audit, [])
        state, audit = sched.observe(state, features(17, contact_revision=1))
        self.assertEqual(audit[0]["triggered_tick"], 17)
        state, _ = sched.start_request(state, "r1", features(17, contact_revision=1))
        state, _ = sched.finish_request(state, "r1", features(30, contact_revision=1), success=True)
        self.assertEqual(state["next_grid_tick"], 77)
        state, audit = sched.observe(state, features(76, contact_revision=2))
        self.assertFalse(sched.is_ready(state))
        self.assertEqual(audit, [])
        state, audit = sched.observe(state, features(77, contact_revision=2))
        self.assertEqual(audit[0]["triggered_tick"], 77)

    def test_missed_fixed_grids_coalesce_while_one_request_is_inflight(self):
        state = self.initial_request("F60")
        state, audit = sched.observe(state, features(190))
        self.assertEqual((audit[0]["triggered_tick"], audit[0]["last_triggered_tick"],
                          audit[0]["represented_event_count"], audit[0]["observed_tick"]),
                         (60, 180, 3, 190))
        self.assertFalse(sched.is_ready(state))
        state, _ = sched.finish_request(state, "r0", features(195), success=True)
        self.assertTrue(sched.is_ready(state))
        state, record = sched.start_request(state, "r1", features(195))
        self.assertEqual((record["triggered_tick"], record["last_triggered_tick"],
                          record["event_count"], record["requested_tick"]), (60, 180, 3, 195))
        self.assertEqual(state["next_grid_tick"], 240)

    def test_hybrid_event_does_not_reset_600_second_grid(self):
        state = self.initialized("HH600")
        event = features(590, contact_revision=1)
        state, _ = sched.observe(state, event)
        state, _ = sched.start_request(state, "r1", event)
        state, audit = sched.observe(state, features(600, contact_revision=1))
        self.assertEqual([r["reason"] for r in audit], ["fallback_grid"])
        state, _ = sched.finish_request(state, "r1", features(620, contact_revision=1), success=True)
        self.assertTrue(sched.is_ready(state))
        self.assertEqual(state["pending"]["triggered_tick"], 600)
        self.assertEqual(state["next_grid_tick"], 1200)

    def test_dense_new_event_identities_are_latched_even_while_predicate_stays_true(self):
        state = self.initial_request("EH")
        for tick in range(1, 101):
            f = features(tick, contact_revision=tick, contacts_count=1)
            state, audit = sched.observe(state, f)
            self.assertEqual(audit[0]["clause_ids"], [0])
            self.assertTrue(audit[0]["busy"])
            repeated, rows = sched.observe(state, f)
            self.assertEqual(rows, [])
            self.assertEqual(repeated, state)
        self.assertEqual(state["pending"]["event_count"], 100)
        self.assertEqual(state["pending"]["triggered_tick"], 1)
        self.assertEqual(state["pending"]["last_triggered_tick"], 100)
        self.assertEqual(state["pending"]["clause_ids"], [0])
        self.assertLess(len(json.dumps(state["pending"])), 900)
        state, _ = sched.finish_request(state, "r0", f, success=True)
        state, record = sched.start_request(state, "r1", f)
        self.assertEqual(record["event_count"], 100)
        state, _ = sched.finish_request(state, "r1", f, success=True)
        state, audit = sched.observe(state, features(101, contact_revision=100, contacts_count=1))
        self.assertEqual(audit, [])
        self.assertFalse(sched.is_ready(state))

    def test_human_four_increment_causes_use_the_common_dsl(self):
        state = self.initialized("EH")
        self.assertEqual(sched.validate_conditions(sched.human_conditions(features())),
                         state["conditions"])
        f = features(5, contact_revision=1, task_revision=1,
                     own_loss_count=1, incoming_revision=1, incoming_count=1)
        state, audit = sched.observe(state, f)
        self.assertEqual(audit[0]["clause_ids"], [0, 1, 2, 3])
        self.assertEqual(len(audit), 1)
        self.assertEqual(state["pending"]["event_count"], 1)

    def test_same_incoming_count_new_public_identity_is_not_lost(self):
        state = self.initialized("EH")
        f = features(5, incoming_count=1, incoming_revision=1)
        state, _ = sched.observe(state, f)
        state, _ = sched.start_request(state, "r1", f)
        f = features(6, incoming_count=1, incoming_revision=2)
        state, audit = sched.observe(state, f)
        self.assertEqual(audit[0]["clause_ids"], [3])
        self.assertEqual(state["pending"]["triggered_tick"], 6)

    def test_count_only_fallback_cannot_treat_decrease_as_new_incoming(self):
        f = features()
        del f["incoming_revision"]
        state, _ = sched.observe(sched.new_state("EH"), f)
        state, _ = sched.start_request(state, "r0", f)
        f.update(tick=1, incoming_count=2)
        state, audit = sched.observe(state, f)
        self.assertEqual(audit[0]["clause_ids"], [3])
        f.update(tick=2, incoming_count=1)
        state, audit = sched.observe(state, f)
        self.assertEqual(audit, [])

    def test_model_installation_sees_public_change_during_initialization(self):
        state = self.initial_request("EM")
        f = features(20, contact_revision=1, contacts_count=1)
        state, audit = sched.observe(state, f)
        self.assertEqual(audit, [])  # No monitor exists before it is installed.
        state, audit = sched.finish_request(state, "r0", f, success=True,
                                            wake_conditions=[clause("contact_revision")])
        wake = [r for r in audit if r["kind"] == "wake"]
        self.assertEqual(len(wake), 1)
        self.assertTrue(wake[0]["on_installation"])
        self.assertEqual(wake[0]["triggered_tick"], 20)
        self.assertTrue(sched.is_ready(state))
        self.assertEqual(state["conditions_source"], "model")

    def test_empty_model_watch_has_no_hidden_timer_or_recovery(self):
        state = self.initialized("EM")
        state, audit = sched.observe(state, features(10000, contact_revision=3))
        self.assertEqual(audit, [])
        self.assertFalse(sched.is_ready(state))
        self.assertTrue(state["initialized"])
        self.assertIsNone(state["retry_due_tick"])
        self.assertEqual(state["conditions"], [])

    def test_empty_hybrid_model_watch_retains_only_explicit_absolute_fallback(self):
        state = self.initialized("HM600")
        state, audit = sched.observe(state, features(599, contact_revision=3))
        self.assertEqual(audit, [])
        state, audit = sched.observe(state, features(600, contact_revision=3))
        self.assertEqual([r["reason"] for r in audit], ["fallback_grid"])

    def test_clock_only_watch_fires_one_edge_not_once_per_second(self):
        state = self.initialized("EM", [clause("tick", "ge", 5)])
        for tick in range(1, 20):
            state, audit = sched.observe(state, features(tick))
            self.assertEqual(len(audit), int(tick == 5))
        self.assertEqual(state["pending"]["event_count"], 1)

    def test_and_within_or_between_and_event_identity_rearming(self):
        conditions = [{"all": [{"field": "contact_revision", "op": "gt", "value": 0},
                               {"field": "incoming_count", "op": "ge", "value": 2}]},
                      clause("own_loss_count")]
        state = self.initialized("EM", conditions)
        state, audit = sched.observe(state, features(1, contact_revision=1, incoming_count=1))
        self.assertEqual(audit, [])
        state, audit = sched.observe(state, features(2, contact_revision=2, incoming_count=2))
        self.assertEqual(audit[0]["clause_ids"], [0])
        state, audit = sched.observe(state, features(3, contact_revision=3, incoming_count=2,
                                                    own_loss_count=1))
        self.assertEqual(audit[0]["clause_ids"], [0, 1])

    def test_past_clock_condition_intentionally_wakes_on_installation(self):
        state = self.initial_request("EM")
        f = features(20)
        state, audit = sched.finish_request(state, "r0", f, success=True,
                                            wake_conditions=[clause("tick", "ge", 1)])
        self.assertTrue(sched.is_ready(state))
        self.assertTrue(audit[-1]["on_installation"])
        state, _ = sched.start_request(state, "r1", f)
        state, _ = sched.finish_request(state, "r1", features(21), success=True,
                                        wake_conditions=[])
        self.assertFalse(sched.is_ready(state))

    def test_initialization_failure_recovery_is_receipt_based_and_bounded(self):
        state = self.initial_request("EM", max_initialization_retries=1,
                                     initialization_retry_delay_s=30)
        state, audit = sched.finish_request(state, "r0", features(5), success=False)
        self.assertEqual(state["retry_due_tick"], 35)
        self.assertEqual(audit[-1]["kind"], "recovery_scheduled")
        state, audit = sched.observe(state, features(34, contact_revision=1))
        self.assertEqual(audit, [])
        state, audit = sched.observe(state, features(35, contact_revision=1))
        self.assertEqual(audit[0]["reason"], "initialization_recovery")
        self.assertEqual(state["request_count"], 1)  # Scheduling is not a call.
        state, _ = sched.start_request(state, "r1", features(35, contact_revision=1))
        state, _ = sched.finish_request(state, "r1", features(40, contact_revision=1), success=False)
        self.assertIsNone(state["retry_due_tick"])
        state, audit = sched.observe(state, features(10000, contact_revision=2))
        self.assertEqual(audit, [])
        self.assertEqual(state["request_count"], 2)

    def test_zero_recovery_allowance_and_no_recovery_for_initialized_watch_failure(self):
        state = self.initial_request("EM", max_initialization_retries=0)
        state, _ = sched.finish_request(state, "r0", features(5), success=False)
        self.assertIsNone(state["retry_due_tick"])
        state = self.initialized("EM", [clause("contact_revision")])
        f = features(5, contact_revision=1)
        state, _ = sched.observe(state, f)
        state, _ = sched.start_request(state, "r1", f)
        state, _ = sched.finish_request(state, "r1", features(10, contact_revision=1), success=False)
        self.assertIsNone(state["retry_due_tick"])
        state, audit = sched.observe(state, features(50, contact_revision=2))
        self.assertEqual(audit[0]["reason"], "model_condition")

    def test_hybrid_grid_request_supersedes_scheduled_bootstrap_retry(self):
        state = self.initial_request("HM600", initialization_retry_delay_s=700)
        state, _ = sched.finish_request(state, "r0", features(5), success=False)
        state, _ = sched.observe(state, features(600))
        state, _ = sched.start_request(state, "r1", features(600))
        self.assertIsNone(state["retry_due_tick"])
        state, audit = sched.observe(state, features(705))
        self.assertEqual(audit, [])
        state, _ = sched.finish_request(state, "r1", features(710), success=True,
                                        wake_conditions=[])
        self.assertFalse(sched.is_ready(state))

    def test_stale_duplicate_receipt_cannot_install_or_consume_active_request(self):
        state = self.initial_request("EM")
        previous = copy.deepcopy(state)
        result, audit = sched.finish_request(state, "old", features(), success=True,
                                             wake_conditions=[clause("tick", "ge", 0)])
        self.assertEqual(audit[0]["kind"], "response_ignored")
        self.assertEqual(result["inflight"], previous["inflight"])
        self.assertFalse(result["conditions_installed"])
        self.assertEqual(state, previous)
        state, _ = sched.finish_request(state, "r0", features(), success=True, wake_conditions=[])
        duplicate, audit = sched.finish_request(state, "r0", features(), success=True,
                                                wake_conditions=[clause("tick", "ge", 0)])
        self.assertEqual(duplicate["conditions"], [])
        self.assertEqual(duplicate["finished_count"], 1)

    def test_invalid_or_stale_action_cannot_install_even_valid_watch(self):
        state = self.initial_request("EM", max_initialization_retries=0)
        state, _ = sched.finish_request(state, "r0", features(5), success=False,
                                        wake_conditions=[clause("tick", "ge", 0)],
                                        failure_reason="stale_action")
        self.assertFalse(state["conditions_installed"])
        self.assertFalse(state["initialized"])

    def test_invalid_conditions_raise_before_receipt_is_consumed(self):
        state = self.initial_request("EM")
        original = copy.deepcopy(state)
        with self.assertRaises(ValueError):
            sched.finish_request(state, "r0", features(5), success=True,
                                 wake_conditions=[clause("enemy_world")])
        self.assertEqual(state, original)

    def test_restore_retains_inflight_lock_pending_identity_and_exact_continuation(self):
        state = self.initial_request("EH")
        state, _ = sched.observe(state, features(10, contact_revision=1))
        restored = sched.restore_state(json.loads(json.dumps(sched.dump_state(state))))
        self.assertFalse(sched.is_ready(restored))
        with self.assertRaises(ValueError):
            sched.start_request(restored, "duplicate-process-call", restored["last_features"])
        for f in (features(11, contact_revision=1), features(12, contact_revision=2)):
            state, rows = sched.observe(state, f)
            restored, restored_rows = sched.observe(restored, f)
            self.assertEqual((restored, restored_rows), (state, rows))
        f = features(20, contact_revision=2)
        state, rows = sched.finish_request(state, "r0", f, success=True)
        restored, restored_rows = sched.finish_request(restored, "r0", f, success=True)
        self.assertEqual((restored, restored_rows), (state, rows))
        self.assertTrue(sched.is_ready(restored))

    def test_checkpoint_rejects_grid_ledger_retry_and_hidden_feature_corruption(self):
        original = self.initialized("HM600")
        mutations = [lambda s: s.update(next_grid_tick=601),
                     lambda s: s.update(finished_count=2),
                     lambda s: s.update(retry_used=2),
                     lambda s: s["last_features"].update(enemy_truth=1),
                     lambda s: s.update(conditions_installed=False)]
        for mutate in mutations:
            state = copy.deepcopy(original)
            mutate(state)
            with self.subTest(state=state):
                with self.assertRaises(ValueError):
                    sched.restore_state(state)

    def test_private_fields_invalid_dsl_nonfinite_and_excess_size_fail_closed(self):
        for f in (features(enemy_hp=1), features(tick=True), features(tick=-1),
                  features(contact_revision=float("nan"))):
            with self.assertRaises(ValueError):
                sched.validate_features(f)
        for conditions in ([clause("__import__('os')")], [clause("tick", "eval", 0)],
                           [clause("tick", value=float("nan"))], [clause("tick", value=True)],
                           [clause("tick", value=float("inf"))], [clause("tick")] * 5,
                           [{"all": []}], [{"all": [clause("tick")["all"][0]] * 4}],
                           [{"all": [{"field": "tick", "op": "gt", "value": 0,
                                      "program": "print(1)"}]}]):
            with self.subTest(conditions=conditions):
                with self.assertRaises(ValueError):
                    sched.validate_conditions(conditions)
        f = features()
        del f["incoming_revision"]
        with self.assertRaises(ValueError):
            sched.validate_conditions([clause("incoming_revision")], f)

    def test_new_observation_required_at_dispatch_and_time_never_rewinds(self):
        state, _ = sched.observe(sched.new_state("F60"), features())
        with self.assertRaises(ValueError):
            sched.start_request(state, "r0", features(1))
        state, _ = sched.observe(state, features(10, contact_revision=1))
        for f in (features(9, contact_revision=1), features(11, contact_revision=0)):
            with self.assertRaises(ValueError):
                sched.observe(state, f)
        with self.assertRaises(ValueError):
            sched.observe(sched.new_state("EH"), features(1))

    def test_input_and_return_objects_are_isolated_and_request_ids_unique(self):
        original = sched.new_state("EH")
        saved = copy.deepcopy(original)
        f = features()
        state, audit = sched.observe(original, f)
        self.assertEqual(original, saved)
        f["contact_revision"] = 9
        audit[0]["features"]["contact_revision"] = 9
        self.assertEqual(state["last_features"]["contact_revision"], 0)
        started, record = sched.start_request(state, "r0", features())
        record["features"]["contact_revision"] = 9
        self.assertEqual(started["inflight"]["features"]["contact_revision"], 0)
        started, _ = sched.finish_request(started, "r0", features(), success=True)
        started, _ = sched.observe(started, features(1, contact_revision=1))
        with self.assertRaises(ValueError):
            sched.start_request(started, "r0", features(1, contact_revision=1))


if __name__ == "__main__":
    unittest.main()

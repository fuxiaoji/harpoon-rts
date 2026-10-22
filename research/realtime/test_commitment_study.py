"""Resource semantics and actual frozen-core R2/R3 diagnostic checks."""
import copy
import math
import os
from pathlib import Path
import unittest

import commitment_study as study
from real_nav import RealNav


class LedgerTests(unittest.TestCase):
    def test_atomic_shared_reservation_rejects_combined_overdraw(self):
        ledger = study.Ledger()
        before = ledger.snapshot()
        with self.assertRaises(ValueError):
            ledger.reserve_many({"u1_A": 2, "u2_B": 3})
        self.assertEqual(before, ledger.snapshot())
        ledger.reserve_many({"u1_A": 2, "u2_D": 1})
        self.assertEqual(ledger.available, 0)
        ledger.check()

    def test_cancel_before_start_refunds_everything(self):
        for reversible in (False, True):
            ledger = study.Ledger()
            ledger.reserve_many({"A": 2})
            self.assertEqual(ledger.cancel("A", reversible), {"released": 2, "refund": 0, "sunk": 0})
            self.assertEqual(ledger.available, 3)

    def test_cancel_during_service_refunds_only_unfinished_start(self):
        for reversible in (False, True):
            ledger = study.Ledger()
            ledger.reserve_many({"A": 2})
            ledger.start("A")
            ledger.cancel("A", reversible)
            self.assertEqual(ledger.consumed, 0 if reversible else 1)
            self.assertEqual(ledger.available, 3 if reversible else 2)
            ledger.check()

    def test_completed_chain_stage_never_refunded(self):
        ledger = study.Ledger()
        ledger.reserve_many({"A": 2})
        ledger.start("A")
        ledger.finish("A", 1)
        ledger.start("A")
        ledger.cancel("A", True)
        self.assertEqual(ledger.consumed, 1)
        self.assertEqual(ledger.available, 2)

    def test_cannot_complete_without_service_start(self):
        ledger = study.Ledger()
        ledger.reserve_many({"A": 2})
        with self.assertRaises(ValueError):
            ledger.finish("A", 2)


class BeliefTests(unittest.TestCase):
    def test_all_posteriors_normalize(self):
        for s1 in study.Z_VALUES:
            for s2 in (None,) + study.Z_VALUES:
                p = study.posterior(s1, s2)
                self.assertAlmostEqual(sum(p.values()), 1)
                self.assertTrue(all(0 <= v <= 1 for v in p.values()))
        self.assertAlmostEqual(study.posterior("B")["B"], .5)
        self.assertAlmostEqual(study.posterior("none")["none"], .75)

    def test_branch_weights_equal_bayesian_evidence(self):
        case = {"information": "pending", "s1": "B", "s2": "C"}
        branches = study.latent_branches(case)
        self.assertEqual(len(branches), 18)
        for z in study.Z_VALUES:
            self.assertAlmostEqual(sum(b["p"] for b in branches if b["z"] == z),
                                   study.posterior("B")[z])
        for arrival in (120, 180):
            self.assertAlmostEqual(sum(b["p"] for b in branches if b["arrival"] == arrival), .5)


BINARY = Path(os.environ.get("HARPOON_NAV_BINARY", str(Path(__file__).resolve().parents[2] /
              "tmp/research-m3b-20261022/target/release/examples/realtime_nav_service.exe")))


@unittest.skipUnless(BINARY.is_file(), "Frozen native navigation binary is required")
class NativeCommitmentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.nav = RealNav(BINARY)
        cls.cases = study.build_cases(cls.nav)

    @classmethod
    def tearDownClass(cls):
        cls.nav.close()

    def case(self, family="branch", information="pending", slack=120, reversible=False):
        return copy.deepcopy(next(c for c in self.cases if c["structure"] == family and
            c["information"] == information and c["slack_s"] == slack and
            c["reversible"] == reversible))

    def test_exact_24_cell_factor_grid_and_physical_feasibility(self):
        self.assertEqual(len(self.cases), 24)
        self.assertEqual(len({c["case_id"] for c in self.cases}), 24)
        for case in self.cases:
            for job in case["jobs"].values():
                self.assertLess(job["direct_return_s"] + job["released_at"], case["horizon"])
                self.assertTrue(all(l["direct_completion_s"] > 0 for l in job["legs"]))

    def test_all_24_cells_independently_reconcile_every_candidate(self):
        for case in self.cases:
            study.audit_timely(self.nav, case)
            for action in study.actions(case):
                result = study.evaluate(self.nav, case, action)
                audit = study.audit_result(case, result)
                self.assertTrue(audit["expected_score_recomputed"])

    def test_independent_audit_rejects_corrupt_reward_or_resource(self):
        case = self.case()
        result = study.evaluate(self.nav, case, "A")
        for field in ("rewards_by_z", "resource"):
            outcome = copy.deepcopy(result["candidate_outcomes"]["continue"])
            if field == "rewards_by_z":
                outcome[field]["none"] += 1
            else:
                outcome[field]["available"] += 1
            with self.assertRaises(AssertionError):
                study.audit_outcome(case, outcome)

    def test_public_physics_and_individual_timely_reference_are_exposed(self):
        public = study.public_input(self.case())
        physics = public["navigation_execution"]
        self.assertEqual(physics["commanded_speed_kts"], 18)
        self.assertEqual(physics["tick_seconds"], 1)
        self.assertEqual(physics["turn_rate_deg_s"], 1.8)
        self.assertAlmostEqual(physics["decel_kts_per_s"], 1.2)
        self.assertIn("isolated_timely_navigation_reference", public["jobs"]["A"])

    def test_public_projection_and_scores_ignore_unseen_realization(self):
        case = self.case()
        altered = copy.deepcopy(case)
        altered.update(s2="C", judge_realized_z="C", judge_s2_arrival=180)
        self.assertEqual(study.public_input(case), study.public_input(altered))
        self.assertEqual(study.latent_branches(case), study.latent_branches(altered))
        a = study.evaluate(self.nav, case, "wait")
        b = study.evaluate(self.nav, altered, "wait")
        self.assertEqual(a["expected_value"], b["expected_value"])
        self.assertEqual(a["branches"], b["branches"])
        public = study.canonical(study.public_input(case))
        for forbidden in ("judge_realized_z", "initial_worlds", "configuration_sha256",
                          "direct_completion_s", "structure", "case_id"):
            self.assertNotIn(forbidden, public)

    def test_public_continuation_identical_for_same_visible_signal(self):
        result = study.evaluate(self.nav, self.case(), "wait")
        for s2 in study.Z_VALUES:
            selected = {b["continuation"] for b in result["branches"] if b["s2"] == s2}
            beliefs = {study.canonical(b["public_belief"]) for b in result["branches"] if b["s2"] == s2}
            self.assertEqual(len(selected), 1)
            self.assertEqual(len(beliefs), 1)

    def test_pending_signal_arrival_has_no_extra_decision_before_checkpoint(self):
        result = study.evaluate(self.nav, self.case(), "A")
        for z in study.Z_VALUES:
            for s2 in study.Z_VALUES:
                pair = [b for b in result["branches"] if b["z"] == z and b["s2"] == s2]
                self.assertEqual(pair[0]["value"], pair[1]["value"])
                self.assertEqual(pair[0]["trajectory_sha256"], pair[1]["trajectory_sha256"])

    def test_actual_old_route_moves_while_waiting(self):
        ep = study.Episode(self.nav, self.case())
        ep.advance(60)
        self.assertEqual(ep.tick, 60)
        self.assertGreater(ep.states["u1"]["pos"]["x"], .01)
        self.assertGreater(ep.states["u1"]["speed_kts"], 0)

    def test_exact_checkpoint_and_later_replies_never_apply(self):
        case = self.case()
        at = study.evaluate(self.nav, case, "A", 240)
        later = study.evaluate(self.nav, case, "A", 400)
        self.assertTrue(at["late"])
        self.assertTrue(later["late"])
        self.assertEqual(at["expected_value"], later["expected_value"])
        self.assertEqual(at["candidate_outcomes"], later["candidate_outcomes"])
        self.assertFalse(any(e["kind"] == "assigned" for e in at["checkpoint_state"]["events"]))
        before = study.evaluate(self.nav, case, "A", 239)
        self.assertFalse(before["late"])
        self.assertTrue(any(e["kind"] == "assigned" and e["tick"] == 239
                            for e in before["checkpoint_state"]["events"]))

    def test_chain_service_order_and_cumulative_completed_rewards(self):
        result = study.evaluate(self.nav, self.case("chain"), "A")
        outcome = result["candidate_outcomes"]["continue"]
        events = outcome["events"]
        completed = [e for e in events if e["kind"] == "service_completed"]
        self.assertEqual([e["leg"] for e in completed], ["A1", "A2"])
        starts = [e for e in events if e["kind"] == "service_started"]
        self.assertGreaterEqual(starts[1]["tick"], completed[0]["tick"])
        self.assertEqual({e["unit"] for e in starts}, {"u1"})
        self.assertEqual(outcome["rewards_by_z"]["none"], 8)
        cancelled = result["candidate_outcomes"]["return"]
        self.assertEqual(cancelled["rewards_by_z"]["none"], 2)
        self.assertGreaterEqual(cancelled["resource"]["consumed"], 1)

    def test_return_constraint_gates_even_on_time_rewards(self):
        case = self.case()
        ep = study.Episode(self.nav, case)
        ep.assign("A")
        ep.advance(case["jobs"]["A"]["legs"][0]["direct_completion_s"])
        outcome = ep.outcome()
        self.assertEqual(outcome["raw_rewards_by_unit"]["u1"]["none"], 8)
        self.assertFalse(outcome["returned"]["u1"])
        self.assertEqual(outcome["rewards_by_z"]["none"], 0)

    def test_deadline_is_inclusive_and_completed_reward_not_rolled_back(self):
        case = self.case()
        end = case["jobs"]["A"]["legs"][0]["direct_completion_s"]
        for margin, expected in ((0, 8), (-1, 0)):
            instance = copy.deepcopy(case)
            instance["jobs"]["A"]["legs"][0]["deadline"] = end + margin
            ep = study.Episode(self.nav, instance)
            ep.assign("A")
            ep.advance(instance["horizon"])
            ep.cancel()
            self.assertEqual(ep.outcome()["rewards_by_z"]["none"], expected)

    def test_reversible_and_irreversible_start_changes_future_feasibility(self):
        irreversible = study.evaluate(self.nav, self.case(reversible=False), "A")
        reversible = study.evaluate(self.nav, self.case(reversible=True), "A")
        self.assertEqual(irreversible["checkpoint_state"]["resource"]["consumed"], 1)
        self.assertNotIn("B", irreversible["candidate_outcomes"])
        self.assertIn("B", reversible["candidate_outcomes"])
        self.assertEqual(reversible["candidate_outcomes"]["return"]["resource"]["available"], 3)

    def test_shared_jobs_have_distinct_actual_vessels(self):
        case = self.case("shared")
        self.assertEqual(study.actions(case), ["wait", "A", "D"])
        a = study.evaluate(self.nav, case, "A")
        d = study.evaluate(self.nav, case, "D")
        self.assertEqual(next(e["unit"] for e in a["checkpoint_state"]["events"] if e["kind"] == "assigned"), "u1")
        self.assertEqual(next(e["unit"] for e in d["checkpoint_state"]["events"] if e["kind"] == "assigned"), "u2")

    def test_finite_optimum_covers_all_first_actions(self):
        case = self.case("shared")
        result = study.same_delay_optimum(self.nav, case, 60)
        self.assertEqual(set(result["action_values"]), set(study.actions(case)))
        self.assertEqual(result["expected_value"], max(result["action_values"].values()))
        self.assertGreater(result["timing"]["continuation_selection_candidates"], 0)
        self.assertGreaterEqual(result["timing"]["cpu_s"], 0)

    def test_preposterior_information_value_is_nonnegative(self):
        for case in self.cases:
            if case["information"] == "seen":
                result = study.information_value(self.nav, case)
                self.assertGreaterEqual(result["gain"], -1e-10)
                self.assertAlmostEqual(sum(row["p"] for row in result["early_signal_rows"]), 1)

    def test_cross_process_restoration_and_trajectory_digest(self):
        case = self.case()
        before = study.evaluate(self.nav, case, "A", 60)
        with RealNav(BINARY) as other:
            after = study.evaluate(other, case, "A", 60)
        self.assertEqual(before["expected_value"], after["expected_value"])
        self.assertEqual(before["branches"], after["branches"])
        self.assertEqual(before["candidate_outcomes"], after["candidate_outcomes"])

    def test_schema_rejects_future_choice_and_extra_fields(self):
        public = study.public_input(self.case())
        self.assertEqual(study.validate({"choice": "A", "brief": "reason"}, public), "A")
        for bad in ({"choice": "B", "brief": ""}, {"choice": "A"},
                    {"choice": "A", "brief": "", "program": []}):
            with self.assertRaises(ValueError):
                study.validate(bad, public)


if __name__ == "__main__":
    unittest.main()

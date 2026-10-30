"""Posthoc v2 service interruption regression and frozen-result non-regression.

These tests read original frozen evidence and execute native navigation; no
model calls or edits to original inputs/results are performed.
"""
import contextlib
import copy
import importlib.util
import json
import math
import os
from pathlib import Path
import unittest

import commitment_study_v2 as v2
from real_nav import RealNav

STAGE = Path(os.environ.get("HARPOON_M3B_STAGE", str(Path(__file__).resolve().parents[2] /
             "tmp/research-m3b-20261022")))
BINARY = STAGE / "target/release/examples/realtime_nav_service.exe"


@contextlib.contextmanager
def native():
    nav = RealNav(BINARY)
    try:
        yield nav
    finally:
        nav.close()
        nav.process.stdout.close()
        nav.process.stderr.close()
        nav.cache.clear()
        for attr in ("_commitment_evaluator", "_commitment_evaluator_v2"):
            if hasattr(nav, attr):
                getattr(nav, attr).physical_cache.clear()
                delattr(nav, attr)


@unittest.skipUnless(BINARY.is_file() and (STAGE / "r3/references.json").is_file(),
                     "Requires frozen native engine and M3b evidence")
class ContinuousServiceV2Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cases = {case["case_id"]: case for case in json.loads((STAGE / "r2/cases.json").read_text())}
        cls.references = json.loads((STAGE / "r3/references.json").read_text())
        spec = importlib.util.spec_from_file_location("commitment_frozen_v1_for_test",
                  STAGE / "frozen-source/research/realtime/commitment_study.py")
        cls.v1 = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.v1)

    def case(self):
        return copy.deepcopy(self.cases["branch-seen-s240-r0"])

    def compare_evaluation(self, actual, frozen, label):
        for field in ("expected_value", "branches", "candidate_outcomes", "checkpoint_state", "invalid_candidates"):
            self.assertEqual(v2.digest(actual[field]), v2.digest(frozen[field]), f"{label}: {field}")

    def drift_episode(self, nav, reversible=False):
        case = self.case()
        case["reversible"] = reversible
        ep = v2.Episode(nav, case)
        ep.advance(151)
        ep.assign("A")
        ep.advance(240)
        ep.continue_with("C")
        return case, ep

    def test_slot55_exact_original_failure_and_all_corrected_candidates(self):
        case = self.case()
        result = json.loads((STAGE / "r3/calls/055/result.json").read_text())
        delay = math.ceil(result["elapsed_s"] * 5)
        self.assertEqual(delay, 151)
        with native() as nav:
            with self.assertRaisesRegex(AssertionError, "Continuous service left station radius"):
                self.v1.same_delay_optimum(nav, case, delay)
            corrected = v2.same_delay_optimum(nav, case, delay)
            for evaluation in corrected["evaluations"].values():
                v2.audit_result(case, evaluation)
        self.assertEqual(corrected["action_values"], {"wait": 1.1999999999999995, "A": 8.})
        drift = corrected["evaluations"]["A"]["candidate_outcomes"]["C"]
        events = drift["events"]
        starts = [e for e in events if e["kind"] == "service_started"]
        interruptions = [e for e in events if e["kind"] == "service_interrupted"]
        resumptions = [e for e in events if e["kind"] == "service_resumed"]
        completions = [e for e in events if e["kind"] == "service_completed"]
        self.assertEqual([e["tick"] for e in starts], [240])
        self.assertEqual([e["tick"] for e in interruptions], [243])
        self.assertEqual([e["tick"] for e in resumptions], [372])
        self.assertEqual([e["tick"] for e in completions], [492])
        self.assertEqual(interruptions[0]["discarded_service_s"], 2)
        self.assertEqual(interruptions[0]["resource"], resumptions[0]["resource"])
        self.assertEqual(drift["resource"]["consumed"], 3)
        self.assertEqual(drift["rewards_by_z"]["C"], 16)
        self.assertTrue(all(b["continuation"] == "continue" for b in corrected["evaluations"]["A"]["branches"]))

    def test_cancel_after_interruption_refunds_only_once(self):
        for reversible in (False, True):
            with native() as nav:
                case, ep = self.drift_episode(nav, reversible)
                ep.advance(243)
                self.assertEqual(ep.active["phase"], "travel")
                self.assertEqual(ep.ledger.consumed, 1)
                self.assertEqual(ep.ledger.reservations, {"C": 2})
                ep.cancel()
                self.assertEqual(ep.ledger.consumed, 0 if reversible else 1)
                self.assertEqual(ep.ledger.available, 3 if reversible else 2)
                ep.advance(case["horizon"])
                v2.audit_outcome(case, ep.outcome())

    def test_reentry_resets_clock_and_service_is_continuous_until_completion(self):
        with native() as nav:
            case, ep = self.drift_episode(nav)
            ep.advance(372)
            self.assertEqual(ep.active["phase"], "service")
            self.assertEqual(ep.active["service_end"], 492)
            self.assertEqual(ep.ledger.consumed, 1)
            ep.advance(491)
            self.assertEqual(ep.completed["C"], [])
            ep.advance(492)
            self.assertEqual(ep.completed["C"], ["C1"])
            self.assertEqual(ep.ledger.consumed, 3)
            self.assertEqual(ep.ledger.active_tranches, {})
            ep.advance(case["horizon"])
            audit = v2.audit_outcome(case, ep.outcome())
            self.assertEqual((audit["service_interruptions"], audit["service_resumptions"]), (1, 1))

    def test_completed_chain_leg_clears_start_marker_and_survives_later_cancel(self):
        # Controlled native fixture: first leg is the known tangent-entry C
        # location, then another one-kit leg. Only this local test copy changes.
        case = self.case()
        first = copy.deepcopy(case["jobs"]["C"]["legs"][0])
        first.update(id="A1", cost=1, reward=2, service_s=20, reward_condition="certain", deadline=900)
        second = copy.deepcopy(first)
        second.update(id="A2", goal={"x": .9, "y": -.35}, reward=6, service_s=120)
        case["jobs"]["X"] = {"unit": "u1", "current": True, "released_at": 240,
                              "legs": [first, second]}
        with native() as nav:
            ep = v2.Episode(nav, case)
            ep.advance(151)
            ep.assign("A")
            ep.advance(240)
            ep.continue_with("X")
            ep.advance(392)
            self.assertEqual(ep.completed["X"], ["A1"])
            self.assertNotIn("X", ep.ledger.active_tranches)
            # Execute until the second service start; finite native 1 s stepping.
            while ep.active and ep.active["phase"] != "service" and ep.tick < 900:
                ep.advance(ep.tick + 1)
            self.assertEqual(ep.ledger.active_tranches, {"X": 1})
            self.assertEqual(ep.ledger.consumed, 2)
            self.assertEqual(sum(e["kind"] == "service_started" and e.get("job") == "X" for e in ep.events), 2)
            self.assertEqual(sum(e["kind"] == "service_resumed" for e in ep.events), 1)
            ep.case["reversible"] = True
            ep.cancel()
            self.assertEqual(ep.ledger.consumed, 1)
            self.assertEqual(ep.completed["X"], ["A1"])
            ep.advance(case["horizon"])
            self.assertEqual(ep.outcome()["rewards_by_z"]["none"], 2)
            v2.audit_outcome(case, ep.outcome())

    def test_independent_audit_rejects_clock_or_resource_tampering(self):
        with native() as nav:
            case, ep = self.drift_episode(nav)
            ep.advance(case["horizon"])
            original = ep.outcome()
        for kind, field in (("service_interrupted", "discarded_service_s"),
                            ("service_resumed", "tick")):
            changed = copy.deepcopy(original)
            event = next(e for e in changed["events"] if e["kind"] == kind)
            event[field] += 1
            changed["trajectory_sha256"] = v2.digest({k: v for k, v in changed.items() if k != "trajectory_sha256"})
            with self.assertRaises(AssertionError):
                v2.audit_outcome(case, changed)
        changed = copy.deepcopy(original)
        resumed = next(e for e in changed["events"] if e["kind"] == "service_resumed")
        resumed["resource"]["consumed"] += 1
        with self.assertRaises(AssertionError):
            v2.audit_outcome(case, changed)

    def test_all_12_snapshots_common_delays_exactly_match_frozen_outcomes(self):
        count = 0
        for case_id, reference in self.references.items():
            case = self.cases[case_id]
            with native() as nav:
                for delay in (0, 60, 180):
                    old = reference["by_delay"][str(delay)]
                    new = v2.same_delay_optimum(nav, case, delay)
                    self.assertEqual(new["action_values"], old["action_values"])
                    for action, evaluation in new["evaluations"].items():
                        self.compare_evaluation(evaluation, old["evaluations"][action], f"{case_id}/{delay}/{action}")
                        v2.audit_result(case, evaluation)
                        count += 1
        self.assertEqual(count, 84)
        print(f"V2 COMMON NONREGRESSION: 12 snapshots x 3 delays, {count} first-action evaluations; all candidate trajectories identical", flush=True)

    def test_original_slots_0_to_54_actual_all_candidates_exactly_match(self):
        old_dir = STAGE / "r3-analysis-20261023/replays"
        if not old_dir.is_dir():
            self.skipTest("Original partial actual-delay analysis unavailable")
        count, unreturned = 0, []
        for index in range(55):
            path = old_dir / f"{index:03d}.json"
            self.assertTrue(path.is_file(), str(path))
            old = json.loads(path.read_text())
            summary = old["summary"]
            case = self.cases[summary["case_id"]]
            with native() as nav:
                new = v2.same_delay_optimum(nav, case, summary["actual_delay_s"])
                previous = old["references"]["actual"]
                self.assertEqual(new["action_values"], previous["action_values"])
                for action, evaluation in new["evaluations"].items():
                    self.compare_evaluation(evaluation, previous["evaluations"][action], f"slot{index}/{action}")
                    v2.audit_result(case, evaluation)
                    count += 1
                    for candidate, outcome in evaluation["candidate_outcomes"].items():
                        for unit, rewards in outcome["raw_rewards_by_unit"].items():
                            if max(rewards.values()) > 0 and not outcome["returned"][unit]:
                                unreturned.append({"index": index, "action": action,
                                    "candidate": candidate, "unit": unit,
                                    "raw_rewards": rewards, "final_position": outcome["final_navigation"][unit]["pos"]})
        print(f"V2 ACTUAL NONREGRESSION: slots 0..54, {count} first-action evaluations; all candidate trajectories identical", flush=True)
        print("V2 RETURN AUDIT: " + json.dumps(unreturned), flush=True)


if __name__ == "__main__":
    unittest.main()

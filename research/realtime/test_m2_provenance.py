import copy
import importlib.util
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location("m2_analysis_checks",Path(__file__).resolve().parents[2]/"scripts/analyze-realtime-m2.py")
analysis=importlib.util.module_from_spec(spec);spec.loader.exec_module(analysis)


class ProvenanceTests(unittest.TestCase):
    def fixture(self):
        case={"backend":"scripted","clock":"injected","parameters":{"gap_s":30},"seed":0,"family":"d5","budget":6,"delay":100}
        run={"backend":"scripted","clock_mode":"injected","case_parameters":{"gap_s":30},"seed":0,"family":"d5","max_calls":6,"injected_delay_sim_s":100,"initialization_retries_limit":1}
        return run,case

    def test_matching_protocol_and_actual_trace_are_accepted(self):
        analysis.verify_case(*self.fixture())

    def test_real_call_cannot_be_relabelled_as_scripted(self):
        run,case=self.fixture();run["backend"]="codex"
        with self.assertRaises(AssertionError):analysis.verify_case(run,case)

    def test_changed_clock_or_scene_cannot_be_pooled(self):
        run,case=self.fixture()
        for field,value in (("clock_mode","wall"),("case_parameters",{"gap_s":180})):
            wrong=copy.deepcopy(run);wrong[field]=value
            with self.assertRaises(AssertionError):analysis.verify_case(wrong,case)


if __name__=="__main__":unittest.main()

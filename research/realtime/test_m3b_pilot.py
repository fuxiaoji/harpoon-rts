import itertools
import unittest
import run_m3b_pilot as pilot


class PilotGateTests(unittest.TestCase):
    def schedule(self):
        return [{"index":i,"case_id":c,"effort":e,"repeat":r} for i,(c,e,r) in
                enumerate(itertools.product(range(12),pilot.EFFORTS,range(2)))]

    def test_exact_balanced_schedule(self):
        pilot.validate_schedule(self.schedule(), range(12))

    def test_duplicate_and_excess_requests_rejected(self):
        for rows in (self.schedule()+[self.schedule()[0]], self.schedule()[:-1]):
            with self.assertRaises(ValueError):
                pilot.validate_schedule(rows,range(12))
        rows=self.schedule()
        rows[1]={**rows[0],"index":1}
        with self.assertRaises(ValueError):
            pilot.validate_schedule(rows,range(12))

    def test_missing_explicit_provider_authorization_rejected(self):
        with self.assertRaisesRegex(ValueError,"allow-real-model"):
            pilot.run(None,None,False)


if __name__=="__main__":
    unittest.main()

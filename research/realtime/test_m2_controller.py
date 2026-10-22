import unittest
import m2_controller as m2


class RecoveryTests(unittest.TestCase):
    def test_real_backend_is_rejected_before_injected_run_starts(self):
        with self.assertRaises(ValueError):m2.require_backend_clock("codex","injected")
        with self.assertRaises(ValueError):
            m2.run_episode(None,None,None,None,None,None,None,None,backend="codex",clock_mode="injected")
        m2.require_backend_clock("codex","wall")
        m2.require_backend_clock("scripted","injected")
        m2.require_backend_clock("public","injected")

    def test_stale_initialization_gets_only_one_extra_attempt(self):
        recovery=m2.BootstrapRecovery(1)
        self.assertTrue(recovery.claim(False,"stale_response",100,500,1080))
        self.assertFalse(recovery.claim(False,"stale_response",200,500,1080))
        self.assertEqual(recovery.used,1)

    def test_recovery_is_not_a_periodic_or_human_event_fallback(self):
        recovery=m2.BootstrapRecovery(1)
        for status in ("applied","queued","public_task_changed","timer"):
            self.assertFalse(recovery.claim(False,status,100,500,1080))
        self.assertFalse(recovery.claim(True,"stale_response",100,500,1080))
        self.assertEqual(recovery.used,0)

    def test_no_retry_after_public_deadline_or_horizon(self):
        recovery=m2.BootstrapRecovery(1)
        self.assertFalse(recovery.claim(False,"model_failed",500,500,1080))
        self.assertFalse(recovery.claim(False,"model_failed",1080,1200,1080))
        self.assertEqual(recovery.used,0)

    def test_valid_plan_gate_does_not_suppress_rule_initialization(self):
        state={"standing_revision":0,"public_task":{"revision":0}}
        self.assertTrue(m2.can_skip_current_plan(state,"f300",False))
        self.assertFalse(m2.can_skip_current_plan(state,"em",False))
        self.assertTrue(m2.can_skip_current_plan(state,"em",True))
        state["public_task"]["revision"]=1
        self.assertFalse(m2.can_skip_current_plan(state,"f300",False))


if __name__=="__main__":unittest.main()

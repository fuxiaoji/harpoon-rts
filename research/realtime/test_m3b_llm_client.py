"""No network: validate isolation and failure accounting of the M3b adapter."""
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import m3b_llm_client as client


class ClientTests(unittest.TestCase):
    def simulate(self, returncode=0, violation=False):
        events = [{"type": "item.completed", "item": {"type": "agent_message", "text": '{"choice":"wait"}'}},
                  {"type": "turn.completed", "usage": {"input_tokens": 3, "output_tokens": 2}}]
        if violation:
            events.insert(0, {"type":"item.started", "item":{"type":"command_execution"}})
        return SimpleNamespace(returncode=returncode, stderr="", stdout="\n".join(map(json.dumps, events)))

    def test_isolation_and_backend_effort(self):
        with tempfile.TemporaryDirectory() as base:
            with patch.object(client.shutil, "which", return_value="codex.exe"), patch.object(client.subprocess, "run", return_value=self.simulate()) as run:
                r = client.call({"public": 1}, "medium", Path(base)/"receipt", schema_override={"type":"object"},
                                instructions_override="test", validator=lambda a, i: None)
            args = run.call_args
            self.assertIn('model_reasoning_effort="medium"', args.args[0])
            self.assertIn('project_doc_max_bytes=0', args.args[0])
            self.assertNotIn("receipt", str(args.kwargs["cwd"]))
            self.assertEqual(r["status"], "ok")
            self.assertIsNone(r["max_output_tokens"])
            self.assertTrue(r["cwd_empty_after_call"])

    def test_tool_use_and_nonzero_exit_are_failures(self):
        for code, violation in [(0, True), (1, False)]:
            with tempfile.TemporaryDirectory() as base:
                with patch.object(client.shutil, "which", return_value="codex.exe"), patch.object(client.subprocess, "run", return_value=self.simulate(code, violation)):
                    r=client.call({}, "low", Path(base)/"receipt", validator=lambda a,i:None)
                self.assertEqual(r["status"], "failed")
                self.assertIsNone(r["answer"])

    def test_reasoning_is_redacted_and_usage_retained(self):
        raw='\n'.join([json.dumps({"type":"item.completed","item":{"type":"reasoning","text":"private"}}),
                       json.dumps({"type":"turn.completed","usage":{"reasoning_tokens":7}})])
        events, usage, _, violations=client.clean_events(raw)
        self.assertNotIn("private", json.dumps(events))
        self.assertEqual(usage["reasoning_tokens"],7)
        self.assertEqual(violations,[])


if __name__ == "__main__":
    unittest.main()

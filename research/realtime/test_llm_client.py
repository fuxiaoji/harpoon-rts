import json
import unittest
import llm_client


class ClientAuditTests(unittest.TestCase):
    def test_transport_fallback_is_not_a_tool_call(self):
        events = [{"type": "item.completed", "item": {"type": "error", "message": "transport fallback"}},
                  {"type": "turn.completed", "usage": {"output_tokens": 1}}]
        _, usage, _, violations = llm_client.clean_events("\n".join(map(json.dumps, events)))
        self.assertEqual(violations, [])
        self.assertEqual(usage["output_tokens"], 1)

    def test_real_tool_use_invalidates_sample(self):
        _, _, _, violations = llm_client.clean_events(json.dumps(
            {"type": "item.started", "item": {"type": "command_execution", "command": "read"}}))
        self.assertEqual(violations, ["command_execution"])

    def test_reasoning_text_is_not_archived(self):
        cleaned, _, _, _ = llm_client.clean_events(json.dumps(
            {"type": "item.completed", "item": {"type": "reasoning", "text": "private sample text"}}))
        self.assertNotIn("private sample text", json.dumps(cleaned))

    def task(self):
        return {"public_task": {"revision": 1, "goal": {"x": 1, "y": 2}},
                "observation": {"units": [{"id": 1}]}}

    def answer(self):
        return {"action": "goto", "task_revision": 1, "unit": 1,
                "waypoint": {"x": 1, "y": 2}, "brief": "Navigate."}

    def test_stale_revision_is_rejected(self):
        answer = self.answer()
        answer["task_revision"] = 0
        with self.assertRaises(ValueError):
            llm_client.validate_action(answer, self.task())

    def test_enemy_unit_is_rejected(self):
        answer = self.answer()
        answer["unit"] = 99
        with self.assertRaises(ValueError):
            llm_client.validate_action(answer, self.task())

    def test_nonfinite_waypoint_is_rejected(self):
        answer = self.answer()
        answer["waypoint"]["x"] = float("nan")
        with self.assertRaises(ValueError):
            llm_client.validate_action(answer, self.task())


if __name__ == "__main__":
    unittest.main()

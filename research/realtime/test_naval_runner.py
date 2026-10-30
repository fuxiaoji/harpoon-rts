"""Runner integration regression without model calls or the native process."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import naval_runner


class PublicNativeFixture:
    """Keep observations constant so only acceptance can change plan age."""
    def __init__(self, stage):
        self.obs = {'tick': 0, 'your_side': 'Blue', 'units': [],
                    'contacts': [], 'victory': {'own_losses': 0}, 'over': False}

    def ask(self, op, **request):
        if op == 'script_plan':
            return {'commands': []}
        if op == 'validate':
            return {'decision_valid': True, 'accepted': [], 'rejected': []}
        if op == 'advance':
            self.obs['tick'] += request['seconds']
        if op in ('init', 'advance'):
            return {'observation': copy.deepcopy(self.obs),
                    'public_db': {'scope': 'fixture'}, 'feedback': {}}
        if op == 'dump':
            return {'fixture': True}
        if op == 'replay':
            return {'passed': True}
        raise AssertionError(op)

    def close(self):
        pass


class TestNavalRunner(unittest.TestCase):
    def test_new_age_watch_starts_from_accepted_decision(self):
        # A response received at T60 installs "60s since accepted plan".
        # It must wait until T120 instead of immediately reusing the old age.
        age_watch = [{'all': [{'field': 'plan_age_s', 'op': 'ge', 'value': 60}]}]
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'run'
            with patch.object(naval_runner, 'Native', PublicNativeFixture), \
                    patch.object(naval_runner, 'script_conditions', return_value=age_watch):
                summary = naval_runner.run(Path(directory), output, 'EM', 'script',
                                           horizon=122, delay=60, cap=2)
            requests = json.loads((output / 'requests.json').read_text(encoding='utf-8'))
        self.assertEqual(summary['requests'], 2)
        self.assertEqual([r['request_tick'] for r in requests], [0, 120])
        self.assertEqual(requests[0]['received_tick'], 60)


if __name__ == '__main__':
    unittest.main()

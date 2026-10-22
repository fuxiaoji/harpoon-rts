"""Mutating QA: use a dedicated disposable local server, never a player game."""
import argparse
import concurrent.futures
import json
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("url")
p.add_argument("output", type=Path)
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
checks = []


def request(path, body=None):
    req = urllib.request.Request(a.url + path, data=None if body is None else json.dumps(body).encode(),
                                 headers={"Content-Type":"application/json"})
    try:
        with urllib.request.urlopen(req, timeout=120) as response:
            return response.status, json.load(response)
    except urllib.error.HTTPError as error:
        return error.code, error.read().decode()


def api(path, body=None):
    status, value = request(path, body)
    assert status == 200, (path, status, value)
    return value


def observe():
    return api('/api/state?side=Blue&max_events=500')


def plan(identity, obs, side='Blue'):
    return dict(side=side, ticks=1, commands=[], plan_id=identity,
                expected_tick=obs['tick'], session_revision=obs['session_revision'])


def check(name, predicate):
    assert predicate, name
    checks.append(name)


api('/api/game', dict(scenario='dual_carrier_duel', seed=20261021, speed=0, automated_side='None'))
initial = observe()
for name, mutation, code in [
    ('missing identity', lambda q: {'side':'Blue','commands':[],'ticks':1}, 400),
    ('stale observation', lambda q: dict(q, expected_tick=1), 409),
    ('stale session', lambda q: dict(q, session_revision=0), 409),
    ('malformed plan id', lambda q: dict(q, plan_id='../old'), 400),
]:
    status, detail = request('/api/advance', mutation(plan('invalid', initial)))
    check(name + ' rejects without any observation/event change', status == code and observe() == initial)

with concurrent.futures.ThreadPoolExecutor(2) as pool:
    results = list(pool.map(lambda _: request('/api/advance', plan('concurrent-once', initial)), range(2)))
check('two concurrent submissions advance exactly once', sorted(s for s, _ in results) == [200,409]
      and observe()['tick'] == 1)
accepted = next(v for s, v in results if s == 200)
check('receipt preserves session and distinguishes accepted from execution',
      accepted['observation']['session_revision'] == initial['session_revision'] and accepted['accepted'] == []
      and accepted['plan_id'] == 'concurrent-once' and accepted['end_tick'] == 1)
before = observe()
status, _ = request('/api/advance', plan('concurrent-once', before))
check('consumed id is rejected even with refreshed tick', status == 409 and observe() == before)
api('/api/advance', plan('concurrent-once', before, side='Red'))
check('plan ids are scoped to each side', observe()['tick'] == 2)

# Exercise the first-party authoring client and retain a real request/receipt.
client = Path(__file__).with_name('side-player.py')
player_dir = a.output / 'side-client'
command = [sys.executable, '-X', 'utf8', str(client), '--url', a.url, '--side', 'Blue', '--output', str(player_dir)]
created=subprocess.run(command + ['observe'], check=True, capture_output=True, text=True)
template=Path(json.loads(created.stdout)['template'])
authored = json.loads(template.read_text(encoding='utf-8'))
authored['ticks'] = 1
authored['brief'] = dict(evidence='QA observation', options='hold', choice='hold', risk='none', check='receipt')
template.write_text(json.dumps(authored), encoding='utf-8')
subprocess.run(command + ['advance', str(template)], check=True, capture_output=True)
check('side-only client submits an explicitly authored identity', observe()['tick'] == 3)
retry = subprocess.run(command + ['advance', str(template)], capture_output=True, text=True)
check('client never refreshes a stale plan or replays it', retry.returncode == 1 and observe()['tick'] == 3)

saved = api('/api/saves', {'label':'plan identity QA'})
before_load = observe()
api('/api/advance', plan('before-load', before_load))
api('/api/saves/' + saved['id'] + '/load', {})
after_load = observe()
status, _ = request('/api/advance', plan('not-yet-submitted', before_load))
check('loading same tick invalidates the old session', after_load['tick'] == before_load['tick']
      and after_load['session_revision'] != before_load['session_revision'] and status == 409 and observe() == after_load)
verification = api('/api/replay/verify', {})
check('accepted/rejected transport requests preserve full command replay', verification['verified'])

result = dict(passed=checks, verification=verification, concurrent_statuses=[s for s, _ in results])
(a.output / 'plan-validation.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(result, ensure_ascii=False))

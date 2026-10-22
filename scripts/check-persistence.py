"""Destructive QA only: pass the URL of a dedicated test server, never a player game."""
import argparse
import json
from pathlib import Path
import urllib.request
import urllib.error

p = argparse.ArgumentParser()
p.add_argument("url")
p.add_argument("save_dir", type=Path)
p.add_argument("evidence", type=Path)
a = p.parse_args()
a.evidence.mkdir(parents=True, exist_ok=True)
checks = []
plan_seq = 0

def api(path, body=None, expected=200, keep_revision=False):
    global plan_seq
    if path == '/api/advance' and body is not None:
        observed = api('/api/state?side=' + body['side'] + '&max_events=0', keep_revision=True)
        plan_seq += 1
        body = dict(body, expected_tick=observed['tick'], session_revision=observed['session_revision'],
                    plan_id=f'persistence-qa-{plan_seq}')
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(a.url + path, data=data, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            status, raw = r.status, r.read()
    except urllib.error.HTTPError as e:
        status, raw = e.code, e.read()
    assert status == expected, (path, status, raw[:300])
    value=json.loads(raw) if status==200 else raw.decode()
    if isinstance(value,dict) and not keep_revision: value.pop('session_revision',None)
    return value

def check(name, result):
    assert result, name
    checks.append(name)

api('/api/game', {"scenario":"csg_grand_battle","seed":20261020,"speed":0,"automated_side":"None"})
api('/api/tactical/config', {"side":"Red","enabled":True,"step_now":True})
obs = api('/api/state?side=Blue&max_events=200')
unit = next(u['id'] for u in obs['units'] if u['kind'] == 'Ship')
bad = {"side":"Blue","commands":[{"cmd":"stop","unit":unit}]*17,"ticks":300}
api('/api/advance',bad,400)
check('oversized command batch rejected without advancing',api('/api/time')['tick']==0)
api('/api/advance',{"side":"Blue","commands":[],"intent":"QA synchronized recording","ticks":300})
api('/api/command', {"side":"Blue","command":{"cmd":"set_speed","unit":unit,"speed_kts":12.345678901234567}})
save = api('/api/saves', {"label":"QA queue and FSM checkpoint"})
before = [api('/api/state?side='+side+'&max_events=200') for side in ['Blue','Red']]
api('/api/replay/current',expected=409)
api('/api/advance',{"side":"Blue","ticks":300})
expected = [api('/api/state?side='+side+'&max_events=200') for side in ['Blue','Red']]
verify1 = api('/api/replay/verify',{})
api('/api/saves/'+save['id']+'/load',{})
check('load restores both observations and pauses',before==[api('/api/state?side='+side+'&max_events=200') for side in ['Blue','Red']] and api('/api/time')['speed']==0)
api('/api/advance',{"side":"Blue","ticks":300})
check('queued order and FSM continuation match exactly',expected==[api('/api/state?side='+side+'&max_events=200') for side in ['Blue','Red']])
verify2=api('/api/replay/verify',{})
check('command replay matches full World',verify1==verify2 and verify1['verified'])
original=json.loads((a.save_dir/(save['id']+'.json')).read_text(encoding='utf-8'))
original['payload']['world']['tick']+=1
(a.save_dir/'corrupt.json').write_text(json.dumps(original,ensure_ascii=False),encoding='utf-8')
api('/api/saves/corrupt/load',{},400)
original['version']=999
(a.save_dir/'future.json').write_text(json.dumps(original,ensure_ascii=False),encoding='utf-8')
api('/api/saves/future/load',{},400)
check('corruption and future version leave current battle intact',api('/api/time')['tick']==600)
check('invalid files are visible as unavailable',len([s for s in api('/api/saves')['saves'] if s.get('invalid')])==2)
# A short real scenario verifies final automatic archiving and old viewer compatibility.
api('/api/game',{"scenario":"dual_carrier_duel","seed":20261020,"speed":0,"automated_side":"None"})
api('/api/advance',{"side":"Blue","ticks":300})
archive=api('/api/replay/current')
check('completed battle automatically persists',any(s.get('over') and not s.get('invalid') for s in api('/api/saves')['saves']))
check('archive includes all sequential events',all(x['seq']==i+1 for i,x in enumerate(archive['events'])))
check('both sides are sampled synchronously every 30 seconds',[f['tick'] for f in archive['frames']['Blue']]==list(range(0,301,30))==[f['tick'] for f in archive['frames']['Red']])
check('archive never embeds enemy truth in player contacts',all('about' not in c and 'offset' not in c for side in ['Blue','Red'] for f in archive['frames'][side] for c in f['contacts']))
(a.evidence/'qa-replay.json').write_text(json.dumps(archive,ensure_ascii=False),encoding='utf-8')
(a.evidence/'restart-reference.json').write_text(json.dumps({'save':save,'before':before,'continued':expected,'verify':verify2},ensure_ascii=False),encoding='utf-8')
(a.evidence/'http-validation.json').write_text(json.dumps({'passed':checks,'verify':verify2},ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps({'passed':len(checks),'save_id':save['id'],'verify':verify2},ensure_ascii=False))

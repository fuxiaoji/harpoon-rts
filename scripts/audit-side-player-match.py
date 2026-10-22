"""Post-game audit of side-player decisions and the completed referee archive."""
import argparse
import collections
import hashlib
import json
from pathlib import Path

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('save',type=Path);p.add_argument('player',type=Path);p.add_argument('output',type=Path)
p.add_argument('--referee',type=Path,required=True)
a=p.parse_args()
payload=json.loads(a.save.read_text(encoding='utf-8-sig'))['payload']
world=payload['world'];assert world['over'] and world['tick']==10800
events=payload['recording']['events']
assert all(e['seq']==i+1 for i,e in enumerate(events))
referee=json.loads(a.referee.read_text(encoding='utf-8-sig'));assert referee['verified'] and referee['events_verified']==len(events)
key=lambda e:tuple(e[k] for k in ('tick','side','shooter','weapon','count'))
assert collections.Counter(map(key,referee['releases']))==collections.Counter(key(e) for e in events if e['event']=='weapons_release')
source_kind={key(e):e['kind'] for e in referee['releases']}
windows=[];seen=set();last=0;submitted=accepted=0;rejects=[];commands=collections.Counter()
folders=sorted((x for x in a.player.iterdir() if x.is_dir() and (x/'request.json').exists()),key=lambda d:json.loads((d/'request.json').read_text(encoding='utf-8'))['expected_tick'])
for folder in folders:
    request=json.loads((folder/'request.json').read_text(encoding='utf-8'))
    before=json.loads((folder/'before.json').read_text(encoding='utf-8'))
    receipt=json.loads((folder/'response.json').read_text(encoding='utf-8'))
    brief=json.loads((folder/'brief.json').read_text(encoding='utf-8'))
    assert request['side']=='Blue' and request['plan_id'] not in seen
    seen.add(request['plan_id'])
    assert before['tick']==request['expected_tick']==receipt['start_tick']==last
    assert before['session_revision']==request['session_revision']
    assert 1<=request['ticks']<=300 and len(request['commands'])<=16
    assert all(isinstance(brief.get(k),str) for k in ('evidence','options','choice','risk','check'))
    assert all(u['side']=='Blue' for o in (before,receipt['observation']) for u in o['units'])
    assert all('about' not in c and 'offset' not in c for o in (before,receipt['observation']) for c in o['contacts'])
    last=receipt['end_tick'];assert last==before['tick']+request['ticks']
    submitted+=len(request['commands']);accepted+=len(receipt['accepted'])
    commands.update(c['cmd'] for c in request['commands'])
    rejects.extend(dict(tick=before['tick'],detail=r) for r in receipt['rejected'])
    windows.append(dict(plan_id=request['plan_id'],start=before['tick'],end=last,hold=not request['commands'],brief=brief))
assert last==10800
weapons={v['name']:v for v in world['db']['weapons'].values()}
by_side={}
for side in ('Blue','Red'):
    release=[e for e in events if e['event']=='weapons_release' and e.get('side')==side]
    by_side[side]=dict(weapons_rounds=dict(collections.Counter({name:sum(e['count'] for e in release if e['weapon']==name) for name in sorted({e['weapon'] for e in release})})),
        air_asm_rounds=sum(e['count'] for e in release if weapons[e['weapon']]['kind']=='Ascm' and source_kind[key(e)]=='Aircraft'),
        launches_by_mission=dict(collections.Counter({m:sum(e['count'] for e in events if e['event']=='air_launch' and e.get('side')==side and e['mission']==m) for m in sorted({e['mission'] for e in events if e['event']=='air_launch' and e.get('side')==side})})),
        recovered=sum(e['count'] for e in events if e['event']=='air_recovered' and e.get('side')==side),
        losses=[e for e in events if e['event'] in ('sunk','air_shot_down','ditched') and e.get('side')==side])
result=dict(model='gpt-6.1-sol',interface='authorized player subagent; side-player transport, no scripted strategy',
    tick=world['tick'],result=world['result'],decision_windows=len(windows),hold_windows=sum(w['hold'] for w in windows),
    submitted=submitted,accepted=accepted,allocation_rejections=rejects,submitted_commands=dict(commands),
    continuous_observation_bound_windows=True,blue_observation_structure_verified=True,
    save_sha256=hashlib.sha256(a.save.read_bytes()).hexdigest(),events=len(events),sides=by_side,
    exact_replay_world_fnv64=referee['world_fnv64'],terminal_entries=referee['terminal_entries'],
    public_decision_windows=windows,
    limits=['Observation structure and recorded request identity are audited; this is not a cryptographic sandbox of the player tools.',
        'Public decision briefs are reviewable justifications, not private chain of thought.',
        'Frozen LLM game preceded the last controller fixes. Later unit and native evidence must be kept separate.'])
a.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:result[k] for k in ('tick','result','decision_windows','hold_windows','submitted','accepted','events')},ensure_ascii=False))

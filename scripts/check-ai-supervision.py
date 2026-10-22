"""Mutating regression: use only a dedicated disposable server."""
import argparse,json,urllib.request
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('url');p.add_argument('output',type=Path);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
seq=0;checks=[]
def api(path,body=None):
    request=urllib.request.Request(a.url+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(request,timeout=120) as r:return json.load(r)
def obs(side='Blue'):return api('/api/state?side='+side+'&max_events=500')
def advance(side,ticks,commands=[]):
    global seq
    seq+=1;o=obs(side)
    return api('/api/advance',dict(side=side,ticks=ticks,commands=commands,plan_id=f'supervision-{seq}',expected_tick=o['tick'],session_revision=o['session_revision']))
def check(name,value):
    assert value,name
    checks.append(name)
api('/api/game',dict(scenario='csg_grand_battle',seed=20261020,speed=0,automated_side='None'))
o=obs();existing={u['id'] for u in o['units']};host=next(u for u in o['units'] if u['deck'] and any(c=='cn_j15' and n>=16 for c,n in u['deck']['ready']));hid=host['id']
receipt=advance('Blue',1,[dict(cmd='launch_aircraft',unit=hid,**{'class':'cn_j15'},count=16,mission=dict(mission='cap',station=host['pos'],radius_nm=12,follow=True))])
o=obs();air=[u for u in o['units'] if u['kind']=='Aircraft' and u['id'] not in existing]
check('large CAP request is reduced before actual launch',len(air)==2)
check('newborn CAP already has constrained engagement policy',all(u['engagement_policy']['air_only'] and u['engagement_policy']['max_in_flight']==2 for u in air))
advance('Red',300);o=obs();check('newborn AI aircraft acquire task leases without a model turn',all(str(u['id']) in o['ai_supervision']['tasks'] for u in air))
advance('Blue',100,[dict(cmd='goto',unit=hid,waypoints=[dict(x=host['pos']['x']+5,y=host['pos']['y'])])])
before=obs()['ai_supervision'];saved=api('/api/saves',dict(label='AI accounting and leases'))
advance('Red',300);api('/api/saves/'+saved['id']+'/load',{})
check('save/load restores shared task and resource accounts',obs()['ai_supervision']==before)
api('/api/command',dict(side='Blue',command=dict(cmd='set_speed',unit=hid,speed_kts=8)))
check('manual unit command releases AI ownership',str(hid) not in obs()['ai_supervision']['tasks'])
advance('Red',300);advance('Red',300);o=obs();ids={str(u['id']) for u in air}
check('unreviewed air tasks expire while the other side advances time',not ids.intersection(o['ai_supervision']['tasks']))
check('expiry produces a recorded executor review',any(e.get('event')=='rules_event' and e.get('rule')=='task_review' for e in o['events']))
check('enemy state does not expose Blue task leases',not ids.intersection(obs('Red')['ai_supervision']['tasks']))
verification=api('/api/replay/verify',{});check('review and manual takeover replay exactly',verification['verified'])
(a.output/'supervision-http.json').write_text(json.dumps(dict(passed=checks,verification=verification,launch_receipt=receipt,save_id=saved['id']),ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(dict(passed=len(checks),verification=verification)))

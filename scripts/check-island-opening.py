"""Full hot-opening QA. Only pass a dedicated disposable local test service."""
import argparse
import json
import time
import urllib.request
from pathlib import Path

p=argparse.ArgumentParser();p.add_argument('url');p.add_argument('evidence',type=Path);a=p.parse_args()
a.evidence.mkdir(parents=True,exist_ok=True)
def api(path,body=None):
    req=urllib.request.Request(a.url+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(req,timeout=180) as r:return json.load(r)
api('/api/game',{'scenario':'taiwan_island_opening','speed':0})
for side in ('Blue','Red'):api('/api/tactical/config',{'side':side,'enabled':True,'step_now':False})
b=api('/api/state?side=Blue&max_events=0');r=api('/api/state?side=Red&max_events=0')
counts={kind:sum(u['kind']==kind for u in b['units']) for kind in ('Ship','Sub','Aircraft','Base','Missile')}
assert counts['Aircraft']==64 and counts['Missile']==64 and r['exercise']['locked']
assert len(b['exercise']['opening_flights'])==48 and not r['exercise'].get('opening_flights')
initial=api('/api/saves',{'label':'Hot opening64 aircraft64 missiles / dual FSM QA initial'})
samples=[];started=time.monotonic();api('/api/time',{'speed':300})
while b['tick']<21600 and not b['over']:
    time.sleep(2)
    b=api('/api/state?side=Blue&max_events=0');samples.append(b['tick'])
    print('HTTP hot opening tick',b['tick'],flush=True)
    if time.monotonic()-started>900:
        api('/api/time',{'speed':0});raise TimeoutError('QA wall time exceeded; game paused')
final=api('/api/saves',{'label':'Hot opening six-hour dual FSM QA final'})
archive=a.evidence/'island-hot-http-replay.json'
with urllib.request.urlopen(a.url+'/api/saves/'+final['id']+'/replay',timeout=180) as response,archive.open('wb') as dest:
    while data:=response.read(1024*1024):dest.write(data)
record=json.loads(archive.read_bytes())
summary={'initial':initial,'final':final,'initial_counts':counts,'result':b.get('result'),
    'archive_bytes':archive.stat().st_size,'frames':{s:len(record['frames'][s]) for s in ('Blue','Red')},
    'events':len(record['events']),'orders':len(record.get('orders',[])),'sampled_ticks':samples}
(a.evidence/'island-hot-http-summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(summary,ensure_ascii=True),flush=True)

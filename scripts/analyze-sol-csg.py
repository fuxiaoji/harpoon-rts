"""Post-battle referee audit. Never give its saved World input to a player agent."""
import argparse, collections, hashlib, json, urllib.request
from pathlib import Path

p=argparse.ArgumentParser()
p.add_argument('url');p.add_argument('save',type=Path);p.add_argument('output',type=Path)
a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
save=json.loads(a.save.read_text(encoding='utf-8'));payload=save['payload'];world=payload['world']
assert world['over'] and world['tick']==10800, 'completed three-hour battle required'
with urllib.request.urlopen(a.url+'/api/replay/current',timeout=120) as r:archive=json.load(r)
assert archive['seed']==world['seed']
events=archive['events'];assert [e['seq'] for e in events]==list(range(1,world['event_seq']+1))
assert len(archive['frames']['Blue'])==len(archive['frames']['Red'])==361
assert [f['tick'] for f in archive['frames']['Blue']]==[f['tick'] for f in archive['frames']['Red']]
assert all(u['side']==s for s in ['Blue','Red'] for f in archive['frames'][s] for u in f['units'])
assert all('about' not in c and 'offset' not in c for s in ['Blue','Red'] for f in archive['frames'][s] for c in f['contacts'])
db=world['db'];weapons={w['name']:w for w in db['weapons'].values()}
initial=payload['recording']['initial']['entities'];units={u['id']:u for u in initial.values()}
for s in ['Blue','Red']:
 for f in archive['frames'][s]:
  for u in f['units']:units.setdefault(u['id'],u)

result={'model':'gpt-6.1-sol','side':'Blue','opponent':'tactical_fsm','scenario':world['scenario_id'],'seed':world['seed'],
        'tick':world['tick'],'result':world['result'],'events':len(events),'frames_per_side':361,
        'source':archive['source'],'sides':{},'sunk':[],'recovery_compatibility_issues':[]}
for side in ['Blue','Red']:
 release=collections.Counter();launch=collections.Counter();recover=collections.Counter();notes=collections.Counter()
 for e in events:
  if e.get('side')!=side:continue
  if e['event']=='weapons_release':release[e['weapon']]+=e['count']
  if e['event']=='air_launch':launch[e['mission']]+=e['count']
  if e['event']=='air_recovered':
   recover[e['class']]+=e['count']
   receiver=units.get(e['carrier'],{})
   if ('歼-15' in e['class'] or 'J-15' in e['class']) and not receiver.get('name','').startswith('CV'):
    result['recovery_compatibility_issues'].append({'tick':e['tick'],'carrier':e['carrier'],'receiver':receiver.get('name'),'class':e['class'],'count':e['count']})
  if e['event']=='rules_event' and e.get('domain')=='tactical_ai':notes[e['rule']]+=1
  if e['event']=='sunk':result['sunk'].append({'tick':e['tick'],'side':side,'name':e['unit_name']})
 result['sides'][side]={'weapon_release_rounds':dict(release),'aircraft_launched_by_mission':dict(launch),'aircraft_recovered_by_class':dict(recover),'fsm_notes':dict(notes),
  'commands':sum(1 for o in archive.get('orders',[]) if o['side']==side),
  'final_carriers':[{'id':u['id'],'name':u['name'],'hp':u['hp'],'hp_max':u['hp_max'],'pos':u['pos']} for u in world['entities'].values() if u['side']==side and u['alive'] and u['name'].startswith('CV')]}
requests=sorted((a.output/'blue-player').glob('window-*-request.json'))
responses=sorted((a.output/'blue-player').glob('window-*-response.json'))
result['llm_windows']=len(requests)
result['llm_raw_commands']=sum(len(json.loads(x.read_text(encoding='utf-8')).get('commands',[])) for x in requests)
result['llm_accepted_commands']=sum(len(json.loads(x.read_text(encoding='utf-8')).get('applied',[])) for x in responses)
result['llm_rejected']=sum(len(json.loads(x.read_text(encoding='utf-8')).get('rejected',[])) for x in responses)
result['llm_intent_events']=sum(e.get('domain')=='llm_ai' for e in events)
result['private_ai_notes']=all(e['vis']=={'Side':e['side']} for e in events if e.get('domain') in ['llm_ai','tactical_ai'])
result['timeline']=[{'tick':f['tick'],'blue':f.get('victory',{}).get('own_kill_points'),'red':archive['frames']['Red'][i].get('victory',{}).get('own_kill_points')} for i,f in enumerate(archive['frames']['Blue']) if f['tick']%600==0]
for name,value in [('battle-metrics.json',result),('sol-vs-fsm-csg.json',archive)]:
 (a.output/name).write_text(json.dumps(value,ensure_ascii=False,indent=2 if name.startswith('battle') else None),encoding='utf-8')
print(json.dumps({k:result[k] for k in ['result','events','frames_per_side','llm_windows','llm_raw_commands','llm_accepted_commands','llm_rejected','llm_intent_events','private_ai_notes','recovery_compatibility_issues','sunk']},ensure_ascii=False))

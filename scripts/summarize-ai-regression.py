"""Retrospective game diagnostics. Never feeds referee measurements to an AI."""
import argparse,collections,json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('matches',nargs='+',type=Path);a=p.parse_args()
root=Path(__file__).resolve().parents[1];db=json.loads((root/'database/consolidated.json').read_text(encoding='utf-8'))
for path in sorted((root/'database/volumes').glob('*.json')):
    if not path.name.startswith('_'):
        v=json.loads(path.read_text(encoding='utf-8'));db['weapons'].update(v.get('weapons',{}))
weapons={v['name']:v for v in db['weapons'].values()};results=[]
for path in a.matches:
    j=json.loads(path.read_text(encoding='utf-8'));events=j['events']
    assert all(e['seq']==i+1 for i,e in enumerate(events)),path
    sides={}
    for side in ['Blue','Red']:
        own=[e for e in events if e.get('side')==side]
        releases=[r for r in j['releases'] if r['side']==side]
        asm=[r for r in releases if weapons.get(r['weapon'],{}).get('kind')=='Ascm' and r['kind']=='Aircraft']
        trace=[r for r in j['air_trace'] if r['side']==side]
        airframes={r['unit'] for r in trace};shooters={r['shooter'] for r in asm}
        bins=collections.defaultdict(list)
        for r in j['terminal_entries']:
            if r['side']==side:bins[(r['target'],r['tick']//60)].append(r)
        mixed=[v for v in bins.values() if len({r['shooter_kind'] for r in v})>1]
        sides[side]=dict(actual_air_asm_rounds=sum(r['count'] for r in asm),air_release_altitudes_m=sorted({r['altitude_m'] for r in asm}),
            actual_launched_airframes=sum(e['count'] for e in own if e['event']=='air_launch'),actual_strike_departures=sum(e['count'] for e in own if e['event']=='air_launch' and e['mission']=='strike'),
            actual_recovered_airframes=sum(e['count'] for e in own if e['event']=='air_recovered'),confirmed_air_shot_down=sum(e['event']=='air_shot_down' for e in own),
            observed_strike_airframes_30s=len(airframes),observed_strike_airframes_with_release=len(shooters&airframes),
            fresh_track_samples_30s=sum(r['track_fresh'] for r in trace),strike_samples_30s=len(trace),phases_30s=dict(collections.Counter(r['phase'] for r in trace)),
            terminal_entries=sum(len(v) for v in bins.values()),mixed_source_target_minute_bins=len(mixed),max_mixed_source_entries=max(map(len,mixed),default=0),
            audit_actions=dict(collections.Counter(e['rule'] for e in own if e['event']=='rules_event' and e.get('domain')=='tactical_ai')),
            actual_offensive_rounds_by_source_kind=dict(collections.Counter({kind:sum(r['count'] for r in releases if r['kind']==kind and weapons.get(r['weapon'],{}).get('kind') in ['Ascm','Slcm','Torpedo','Lwt','AswRocket','Bomb','Rocket']) for kind in ['Base','Ship','Sub','Aircraft']})))
    results.append(dict(file=path.name,seed=j['seed'],tick=j['tick'],result=j['result'],world_fnv64=j['world_fnv64'],events=len(events),prealert_commands=j['prealert_commands'],sea_position_checks=j['sea_position_checks'],sides=sides))
output=dict(matches=results,limits=['Air task trace samples every30s; fresh samples are not continuous-track proof','Recovery receipts identify carrier/class/count, not a unique airframe; no invented per-mission recovery rate','Terminal entry is first entry within10nm of the live referee target, not an impact or guaranteed pressure; missiles destroyed earlier are excluded','Minute bins depend on their fixed boundaries; they do not prove deliberate coordination','Authored opening launches are included in T0 events and must not be credited to AI planning'])
a.output.write_text(json.dumps(output,ensure_ascii=False,indent=2),encoding='utf-8');print(json.dumps([dict(file=r['file'],world=r['world_fnv64'],air_asm={s:v['actual_air_asm_rounds'] for s,v in r['sides'].items()}) for r in results]))

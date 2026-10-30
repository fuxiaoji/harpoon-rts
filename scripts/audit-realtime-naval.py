"""Independently inspect referee archives and replay in fresh native processes."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import sys
import time
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'research/realtime'))
from naval_runner import Native,save

def main():
    p=argparse.ArgumentParser();p.add_argument('--stage',type=Path,required=True)
    p.add_argument('--batch',default='calibration');p.add_argument('--wait-for-completion',action='store_true')
    a=p.parse_args();stage=a.stage.resolve()
    rows=[];unresolved=0
    for directory in sorted((stage/a.batch).iterdir()):
        if a.wait_for_completion and (directory/'protocol.json').exists():
            while not (directory/'summary.json').exists():
                if (directory/'failure.json').exists():raise ValueError('run failed: '+directory.name)
                time.sleep(5)
        if not (directory/'summary.json').exists():continue
        summary=json.loads((directory/'summary.json').read_text(encoding='utf-8'))
        checkpoint=json.loads((directory/'checkpoint.json').read_text(encoding='utf-8'))
        world=checkpoint['state']['world'];events=checkpoint['all_events']
        assert world['tick']==summary['tick']
        assert summary['over']==world['over']
        assert len(checkpoint['steps'])==world['tick']
        assert all(e['seq']==i+1 for i,e in enumerate(events))
        assert world['rules']['profile']=='hv2026-rts-v2'
        unresolved+=len(world['rules'].get('pending_adjudications',{}))
        for path in (directory/'inputs').glob('*.json'):
            inp=json.loads(path.read_text(encoding='utf-8'))
            obs=inp['observation'];side=obs['your_side']
            assert all(u['side']==side for u in obs['units'])
            assert not any(k in inp for k in ('world','seed','policy','checkpoint'))
            assert not any('about' in c or 'offset' in c for c in obs['contacts'])
        core_counts=Counter(e.get('event') for e in events)
        sources={'player':sum(len(s['player_orders']) for s in checkpoint['steps']),
                 'maintenance':sum(len(s['maintenance_orders']) for s in checkpoint['steps']),
                 'fsm':sum(len(s['fsm_orders']) for s in checkpoint['steps'])}
        if summary['protocol']['backend']!='fsm':
            side=summary['protocol']['player_side']
            assert all(owner!=side for s in checkpoint['steps'] for owner,_ in s['fsm_orders'])
        checkpoint_sha=hashlib.sha256((directory/'checkpoint.json').read_bytes()).hexdigest()
        cache=directory/'independent-process-replay.json'
        if cache.exists():
            prior=json.loads(cache.read_text(encoding='utf-8'))
            assert prior['checkpoint_sha256']==checkpoint_sha
            replay=prior['result']
        else:
            native=Native(stage)
            try:replay=native.ask(op='replay',checkpoint_path=str(directory/'checkpoint.json'))
            finally:native.close()
            save(cache,{'checkpoint_sha256':checkpoint_sha,'result':replay})
        assert replay['passed']
        row={'run':directory.name,'tick':world['tick'],'over':world['over'],
             'requests':summary['requests'],'result':world['result'],'events':len(events),
             'core_events':dict(core_counts),'command_sources':sources,
             'pending_adjudications':len(world['rules'].get('pending_adjudications',{})),
             'fresh_process_replay':replay,
             'checkpoint_sha256':checkpoint_sha}
        rows.append(row)
        print(json.dumps({'audited':directory.name,'events':len(events),'passed':True}),flush=True)
    expected=11 if a.batch=='calibration' else 7
    result={'passed':len(rows)==expected,'runs':rows,'expected_runs':expected,
            'unresolved_adjudications':unresolved,'independent_process_replays':len(rows)}
    if a.batch=='calibration':
        assert all(r['tick']==10800 and r['over'] for r in rows)
    save(stage/f'{a.batch}-validation.json',result)
    print(json.dumps({'passed':result['passed'],'runs':len(rows),'unresolved':unresolved}),flush=True)
if __name__=='__main__':main()

"""Seal N1 evidence, sources and this-stage catalog entries after inspection."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import shutil

ROOT=Path(__file__).resolve().parents[1]
REPO=ROOT.parent
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,x):p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')

def verified_batch(stage, name, expected, tick, terminal):
    validation=json.loads((stage/f'{name}-validation.json').read_text(encoding='utf-8'))
    rows=validation['runs']
    if (not validation['passed'] or validation['expected_runs']!=expected
            or len(rows)!=expected or validation['independent_process_replays']!=expected
            or validation['unresolved_adjudications']!=0
            or len({row['run'] for row in rows})!=expected):
        raise ValueError(f'{name}: incomplete independent validation')
    policies=[]; starts=0
    for row in rows:
        directory=stage/name/row['run']
        summary=json.loads((directory/'summary.json').read_text(encoding='utf-8'))
        protocol=summary['protocol']
        if (row['tick']!=tick or row['over'] is not terminal
                or summary['tick']!=tick or summary['over'] is not terminal
                or not row['fresh_process_replay']['passed']
                or sha(directory/'checkpoint.json')!=row['checkpoint_sha256']):
            raise ValueError(f'{name}: invalid replay or endpoint for {row["run"]}')
        if name=='live-pilot':
            if (protocol['backend']!='model' or protocol['endpoint_is_full_game']
                    or protocol['horizon_s']!=1200 or summary['result'] is not None
                    or summary['max_clock_lag_s']>2):
                raise ValueError('live pilot must contain verified real-clock model prefixes')
            requests=json.loads((directory/'requests.json').read_text(encoding='utf-8'))
            dispatches=json.loads((directory/'dispatch-ledger.json').read_text(encoding='utf-8'))
            if (len(dispatches)!=len(requests) or len(requests)!=summary['requests']
                    or len(requests)>3 or len({r['id'] for r in dispatches})!=len(requests)
                    or sum(r['status']=='ok' for r in requests)!=summary['valid_receipts']
                    or sum(r['usable'] for r in requests)!=summary['usable_receipts']
                    or sum(r['usage'] is None for r in requests)!=summary['usage_unknown']
                    or sum(bool(r.get('terminal_inflight')) for r in requests)!=summary['terminal_inflight']
                    or [r['id'] for r in dispatches]!=[r['id'] for r in requests]):
                raise ValueError('live pilot request ledger is inconsistent')
            starts+=len(dispatches);policies.append(protocol['policy'])
    if name=='live-pilot' and (starts>21 or sorted(policies)!=sorted(
            ['F60','F300','F600','EH','EM','HH600','HM600'])):
        raise ValueError('live pilot must respect the original seven-arm call ceiling')
    return validation


def verify_execution_sources(stage):
    for batch in ('calibration','live-pilot'):
        for manifest in (stage/batch).glob('*source-manifest.json'):
            source_dir='resume-execution-source' if manifest.name.startswith('resume-') else 'execution-source'
            for name,digest in json.loads(manifest.read_text(encoding='utf-8')).items():
                if sha(stage/batch/source_dir/name)!=digest:
                    raise ValueError('execution source changed: '+name)


def main():
    p=argparse.ArgumentParser();p.add_argument('--stage',type=Path,required=True);a=p.parse_args()
    stage=a.stage.resolve()
    if (stage/'artifact-manifest.json').exists():raise ValueError('sealed archive already exists')
    validation=verified_batch(stage,'calibration',11,10800,True)
    live=verified_batch(stage,'live-pilot',7,1200,False)
    verify_execution_sources(stage)
    audit=json.loads((stage/'naval-independent-all11-audit.json').read_text(encoding='utf-8'))
    if not audit['passed'] or audit['runs_audited']!=11:
        raise ValueError('independent full calibration audit missing')
    age_audit=json.loads((stage/'naval-plan-age-differential-audit.json').read_text(encoding='utf-8'))
    if (not age_audit['passed'] or len(age_audit['runs'])!=2
            or any(not r['dispatch_times_equal'] or not r['all_dispatch_features_equal_saved_model_inputs']
                   for r in age_audit['runs'])):
        raise ValueError('plan-age defect impact has not been resolved for these saved prefixes')
    for row in age_audit['sources']:
        if sha(stage/row['path'])!=row['sha256']:
            raise ValueError('plan-age differential source changed: '+row['path'])
    frozen=json.loads((stage/'source-manifest.json').read_text(encoding='utf-8'))
    for row in frozen['files']:
        if sha(stage/'frozen-source'/row['path'])!=row['sha256']:
            raise ValueError('frozen source changed: '+row['path'])
    binary=json.loads((stage/'binary.json').read_text(encoding='utf-8'))
    if sha(stage/'target/release/examples/realtime_naval_service.exe')!=binary['sha256']:
        raise ValueError('frozen binary changed')
    report=ROOT/'docs/research-naval-n1-20261023.md'
    if '真实先导进行中' in report.read_text(encoding='utf-8'):
        raise ValueError('final report still has unfinished pilot placeholder')
    catalog=REPO/'resources/catalog/inventory.csv'
    with catalog.open('r',encoding='utf-8-sig',newline='') as handle:
        previous=list(csv.DictReader(handle))
    key='research;version=2026-10-23-n1;purpose=full-naval-wake-calibration'
    if any(r['category']==key for r in previous):raise ValueError('stage catalog already recorded')
    sources=[ROOT/'crates/harpoon-server/examples/realtime_naval_service.rs']
    sources.extend((ROOT/'research/realtime').glob('*naval*.py'))
    sources.extend((ROOT/'research/realtime').glob('naval-n1*.md'))
    sources.append(ROOT/'research/realtime/naval-n2-next-work-package.md')
    sources.extend((ROOT/'scripts').glob('*realtime*naval*.py'))
    sources.extend([report,ROOT/'research/realtime/README.md',REPO/'plan.md',REPO/'havedone.md'])
    for src in sources:
        dst=stage/'final-analysis-source'/src.relative_to(REPO)
        dst.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src,dst)
    checks={'passed':True,'scope':'instrumentation and recorded native replay; not model ranking',
            'frozen_files_verified':len(frozen['files']),
            'calibration_full_runs':len(validation['runs']),
            'calibration_new_process_replays':validation['independent_process_replays'],
            'unresolved_adjudications':validation['unresolved_adjudications']}
    checks['live_pilot']=live
    checks['postpilot_plan_age_impact']='dispatch/input times unchanged in saved EM/HM; installation attribution corrected separately'
    save(stage/'stage-validation.json',checks)
    files=[{'path':f.relative_to(stage).as_posix(),'bytes':f.stat().st_size,'sha256':sha(f)}
           for f in sorted(stage.rglob('*')) if f.is_file() and '__pycache__' not in f.parts
           and f.name!='artifact-manifest.pending.json'
           and (not f.relative_to(stage).as_posix().startswith('target/') or f.suffix=='.exe')]
    save(stage/'artifact-manifest.pending.json',{'version':'2026-10-23-n1','files':files,
        'exclusions':['build cache','Python bytecode'],'bytes':sum(f['bytes'] for f in files)})
    sources.extend([stage/'artifact-manifest.pending.json',stage/'source-manifest.json',stage/'stage-validation.json',
                    stage/'calibration-validation.json',stage/'target/release/examples/realtime_naval_service.exe'])
    rows=[]
    for src in sorted(set(sources)):
        relative=src.relative_to(REPO).as_posix().replace('artifact-manifest.pending.json','artifact-manifest.json')
        rows.append({'relative_path':relative,'category':key,
                     'extension':src.suffix,'bytes':src.stat().st_size,'sha256':sha(src),'pdf_pages':''})
    with catalog.open('a',encoding='utf-8',newline='') as handle:
        csv.DictWriter(handle,fieldnames=list(previous[0]),lineterminator='\n').writerows(rows)
    (stage/'artifact-manifest.pending.json').rename(stage/'artifact-manifest.json')
    for row in rows:assert sha(REPO/row['relative_path'])==row['sha256']
    print(json.dumps({'files':len(files),'bytes':sum(f['bytes'] for f in files),
                      'manifest_sha256':sha(stage/'artifact-manifest.json'),'catalog_records':len(rows)}),flush=True)
if __name__=='__main__':main()

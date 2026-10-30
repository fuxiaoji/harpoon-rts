"""Run the predeclared full-naval instrumentation calibration/pilot."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'research/realtime'))
from naval_runner import run,save,POLICIES,Native

def main():
    p=argparse.ArgumentParser();p.add_argument('--stage',type=Path,required=True)
    p.add_argument('--mode',choices=['calibration','model'],required=True)
    p.add_argument('--allow-real-model',action='store_true')
    p.add_argument('--resume-unsent',action='store_true');a=p.parse_args()
    if a.mode=='model' and not a.allow_real_model:p.error('real model disabled without explicit flag')
    stage=a.stage.resolve();out=stage/('calibration' if a.mode=='calibration' else 'live-pilot')
    if a.resume_unsent and a.mode!='model':p.error('resume is only for unsent live arms')
    if a.resume_unsent:
        if not (out/'stopped.json').exists():raise ValueError('no preserved stopped batch')
    else:out.mkdir(exist_ok=False)
    source_folder='resume-execution-source' if a.resume_unsent else 'execution-source'
    sources={}
    for name in ['naval_runner.py','naval_inputs.py','naval_scheduler.py','m3b_llm_client.py','naval-n1-protocol.md']:
        source=ROOT/'research/realtime'/name;dest=out/source_folder/name
        dest.parent.mkdir(exist_ok=True,parents=True);shutil.copy2(source,dest)
        sources[name]=hashlib.sha256(dest.read_bytes()).hexdigest()
    driver=Path(__file__).resolve();shutil.copy2(driver,out/source_folder/driver.name)
    sources[driver.name]=hashlib.sha256(driver.read_bytes()).hexdigest()
    save(out/('resume-source-manifest.json' if a.resume_unsent else 'execution-source-manifest.json'),sources)
    rows=[]
    if a.mode=='calibration':
        jobs=[(policy,'script',1800) for policy in POLICIES]+[('EH','hold',1800),('EH','fsm',1800),
              ('EH','script',600),('F600','script',600)]
        save(out/'job-manifest.json',{'jobs':jobs,'horizon':10800,'seed':20261023,
            'side':'Blue','delay':100,'cap':32,'model_calls':0})
        for i,(policy,backend,lease) in enumerate(jobs):
            row=run(stage,out/f'{i:02d}-{policy}-{backend}-lease{lease}',policy,backend,
                    horizon=10800,delay=100,cap=32,task_lease_s=lease)
            rows.append(row);save(out/'summary.json',rows)
            print(json.dumps({'completed':i+1,'jobs':len(jobs),'policy':policy,
                              'tick':row['tick'],'result':row['result']}),flush=True)
    else:
        previous=json.loads((stage/'calibration-validation.json').read_text(encoding='utf-8'))
        if not previous['passed'] or previous['unresolved_adjudications']:
            raise ValueError('calibration gate has not passed')
        jobs=['F300','EM','F60','HH600','F600','EH','HM600']
        job_manifest={'jobs':jobs,'horizon':1200,'seed':20261023,
            'side':'Blue','speed':10,'cap':3,'max_model_requests':21,
            'purpose':'bounded real-time interface pilot; not terminal win-rate comparison'}
        if a.resume_unsent:
            assert json.loads((out/'job-manifest.json').read_text(encoding='utf-8'))==job_manifest
            save(out/'resume-intent.json',{'only_unsent_arms':True,
                'preserved_stopped':json.loads((out/'stopped.json').read_text(encoding='utf-8')),
                'external_change':'normal user process environment after restricted-token home lookup failed',
                'old_failed_attempts_are_not_reissued':True,'new_failure_block_starts_at_zero':True})
        else:save(out/'job-manifest.json',job_manifest)
        failures=0
        for i,policy in enumerate(jobs):
            directory=out/f'{i:02d}-{policy}'
            if a.resume_unsent and directory.exists():
                if not (directory/'summary.json').exists():raise ValueError('cannot reissue an incomplete arm')
                rows.append(json.loads((directory/'summary.json').read_text(encoding='utf-8')))
                continue
            row=run(stage,directory,policy,'model',horizon=1200,speed=10,
                    cap=3,task_lease_s=1800,initial_failures=failures)
            failures=row['consecutive_failure_tail']
            rows.append(row);save(out/'summary.json',rows)
            print(json.dumps({'completed':i+1,'jobs':7,'policy':policy,
                              'tick':row['tick'],'requests':row['requests']}),flush=True)
            if row['stop_after_three_failures'] or (row['max_clock_lag_s'] or 0)>2:
                save(out/'stopped.json',{'after_policy':policy,'reason':'failure or clock gate'})
                break
    print(json.dumps({'finished':a.mode,'runs':len(rows),'requests':sum(x['requests'] for x in rows)}),flush=True)
if __name__=='__main__':main()

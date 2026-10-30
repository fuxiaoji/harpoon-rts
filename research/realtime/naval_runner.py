"""Whole-engine naval calibration and bounded live model pilot.

Scripted calls are native FSM plans with declared injected delay, never LLM
samples. Live calls run on a worker while the foreground advances the full
engine at one-second ticks. Referee checkpoints never enter model inputs.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess
import time

import naval_scheduler as scheduler
from naval_inputs import (PublicFeatures, project_input, script_conditions,
                          validate_answer, SCHEMA, INSTRUCTIONS)
import m3b_llm_client

POLICIES = ('F60', 'F300', 'F600', 'EH', 'EM', 'HH600', 'HM600')


def save(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')


class Native:
    def __init__(self, stage):
        self.stage = Path(stage).resolve()
        binary = self.stage/'target/release/examples/realtime_naval_service.exe'
        info = json.loads((self.stage/'binary.json').read_text(encoding='utf-8'))
        if hashlib.sha256(binary.read_bytes()).hexdigest() != info['sha256']:
            raise ValueError('binary changed')
        self.process = subprocess.Popen([str(binary)], cwd=self.stage/'frozen-source',
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, encoding='utf-8', bufsize=1)

    def ask(self, **request):
        self.process.stdin.write(json.dumps(request, allow_nan=False)+'\n')
        self.process.stdin.flush()
        line=self.process.stdout.readline()
        if not line:
            raise RuntimeError('native service closed: '+self.process.stderr.read()[-2000:])
        reply=json.loads(line)
        if 'error' in reply:raise RuntimeError(reply['error'])
        return reply

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:self.process.terminate();self.process.wait(timeout=10)


def public_database(stage, obs):
    db={}
    for path in sorted((Path(stage)/'frozen-source/database/volumes').glob('*.json')):
        if path.name.startswith('_'):continue
        payload=json.loads(path.read_text(encoding='utf-8-sig'))
        for kind in ('platforms','weapons','sensors'):
            values=payload.get(kind,{})
            if isinstance(values,list):values={v.get('id',v.get('name')):v for v in values}
            db.setdefault(kind,{}).update(values)
    weapons={w['weapon'] for u in obs['units'] for w in u.get('weapons',[])}
    classes={u['class'] for u in obs['units']}
    classes.update(x[0] for u in obs['units'] for x in (u.get('deck') or {}).get('ready',[]))
    platforms={k:v for k,v in db.get('platforms',{}).items() if k in classes}
    # Definitions are public; readiness and deployments come only from obs.
    for v in platforms.values():
        for mount in v.get('weapons',[]):
            if isinstance(mount,dict):weapons.add(mount.get('weapon',''))
    return {'platforms':platforms,
            'weapons':{k:v for k,v in db.get('weapons',{}).items() if k in weapons},
            'scope':'public definitions for own observed and own ready classes only'}


def response_observation(response):
    return response['observation']


def run(stage, output, policy, backend, horizon=10800, delay=100, speed=10,
        seed=20261023, side='Blue', cap=32, task_lease_s=1800, initial_failures=0):
    output=Path(output).resolve();output.mkdir(parents=True,exist_ok=False)
    protocol={'version':'naval-n1-v1','policy':policy,'backend':backend,
        'horizon_s':horizon,'script_delay_s':delay if backend=='script' else None,
        'sim_seconds_per_wall_second':speed if backend=='model' else None,
        'seed':seed,'player_side':side,'max_calls':cap,'effort':'low',
        'task_lease_s':task_lease_s,'observation_ttl_s':600,
        'full_scenario_duration_s':10800,'scenario':'csg_littoral',
        'rules_profile':'hv2026-rts-v2',
        'endpoint_is_full_game':horizon==10800,
        'cost_matching':'call ceiling only; token/money not matched',
        'output_token_hard_cap':None,'model':m3b_llm_client.MODEL if backend=='model' else None}
    save(output/'protocol.json',protocol)
    native=Native(stage); tracker=PublicFeatures(); state=scheduler.new_state(policy)
    records=[]; audit=[]; traces=[]; feedback=[]; future=None; inflight=None; last_decision=None
    last_command_feedback=None
    consecutive_failures=initial_failures; stop_requests=False; step_durations=[]; drifts=[]
    dispatches=[]
    pool=ThreadPoolExecutor(max_workers=1)
    start=time.perf_counter()
    try:
        current=native.ask(op='init',scenario_path='scenarios/csg_littoral.json',
            seed=seed,player_side=side,allow_script_plan=backend=='script',
            fsm_self=backend=='fsm',task_lease_s=task_lease_s,
            rules_profile='hv2026-rts-v2')
        obs=response_observation(current); save(output/'initial-observation.json',obs)
        db=current.get('public_db') or public_database(stage,obs)
        start=time.perf_counter()  # Pre-T0 native initialization is a separate setup cost.
        while obs['tick']<horizon and not obs['over']:
            tick=obs['tick']; features=tracker.update(obs)
            state,events=scheduler.observe(state,features);audit.extend(events)
            apply=[]; decision=False; receipt=None
            if inflight is not None:
                if backend=='script' and tick>=inflight['ready_tick']:
                    receipt=inflight['receipt']
                elif backend=='model' and future.done():receipt=future.result()
            if receipt is not None:
                usable=receipt['status']=='ok' and tick-inflight['tick']<=600
                answer=receipt.get('answer') or {}
                command_validation=None
                proposed=[]
                if usable:
                    proposed=json.loads(answer['commands_json'])
                    command_validation=native.ask(op='validate',commands=proposed)
                    usable=command_validation['decision_valid']
                    apply=command_validation['accepted'] if usable else []
                    last_command_feedback={'validation':command_validation,'execution':None,
                                           'observation_tick':inflight['tick']}
                if usable:
                    decision=True
                    last_decision={'observation_tick':inflight['tick'],'accepted_tick':tick,
                                   'brief':answer['brief']}
                    tracker.accepted(tick)
                    # The newly accepted plan has age zero. Install its watch
                    # against that state, while preserving already latched events.
                    features=tracker.update(obs)
                state,events=scheduler.finish_request(state,inflight['id'],features,
                    success=usable,wake_conditions=answer.get('wake_conditions') if usable else None)
                audit.extend(events)
                records.append({'id':inflight['id'],'request_tick':inflight['tick'],
                    'received_tick':tick,'apply_tick':tick+1 if usable else None,
                    'usable':usable,'status':receipt['status'],'elapsed_s':receipt.get('elapsed_s'),
                    'usage':receipt.get('usage'),'reason':inflight['reason'],
                    'commands_proposed':len(proposed),'commands_passed_validation':len(apply),
                    'brief':answer.get('brief'),
                    'command_validation':command_validation,
                    'wake_conditions':answer.get('wake_conditions')})
                if backend=='model':
                    consecutive_failures=0 if receipt['status']=='ok' else consecutive_failures+1
                    if consecutive_failures>=3:stop_requests=True
                inflight=None;future=None
                save(output/'requests-progress.json',records)
            if backend in ('script','model') and inflight is None and not stop_requests and len(records)<cap and scheduler.is_ready(state):
                ident=f'request-{len(records):03d}'
                # Current snapshot after a just-accepted plan has updated age.
                features=tracker.update(obs)
                state,events=scheduler.observe(state,features);audit.extend(events)
                state,request=scheduler.start_request(state,ident,features)
                model_input=project_input(obs,features,db,last_decision,{
                    'supervision':current.get('supervision'),
                    'last_command_feedback':last_command_feedback})
                model_input['controller_contract']['task_review_lease_s']=task_lease_s
                save(output/'inputs'/f'{ident}.json',model_input)
                reason=request.get('reason',request.get('reasons'))
                inflight={'id':ident,'tick':tick,'reason':reason}
                dispatches.append({'id':ident,'tick':tick,'reason':reason,'backend':backend,
                                   'dispatched_unix_ns':time.time_ns()})
                save(output/'dispatch-ledger.json',dispatches)
                if backend=='script':
                    proposed=native.ask(op='script_plan')
                    commands=proposed['commands'][:16]
                    answer={'observation_tick':tick,'commands_json':json.dumps(commands),
                        'wake_conditions':script_conditions(features),
                        'brief':'Native FSM planned from this own observation; scripted calibration.'}
                    receipt={'status':'ok','answer':answer,'elapsed_s':None,'usage':None}
                    inflight.update(ready_tick=tick+delay,receipt=receipt)
                    save(output/'script-replies'/f'{ident}.json',receipt)
                else:
                    future=pool.submit(m3b_llm_client.call,model_input,'low',
                        output/'calls'/ident,180,SCHEMA,INSTRUCTIONS,validate_answer)
            if backend=='model':
                deadline=start+(tick+1)/speed
                wait=deadline-time.perf_counter()
                if wait>0:time.sleep(wait)
                drifts.append(max(0.,time.perf_counter()-deadline))
            before=time.perf_counter()
            current=native.ask(op='advance',seconds=1,commands=apply,decision=decision)
            if decision:
                last_command_feedback['execution']=current.get('feedback')
                feedback.append({'tick_before':tick,'request_id':records[-1]['id'],
                                 'feedback':current.get('feedback')})
            step_durations.append(time.perf_counter()-before)
            obs=response_observation(current)
            traces.append({'tick':tick,'features':features,'inflight':None if inflight is None else inflight['id'],
                           'decision':decision,'submitted':len(apply),
                           'own_units':len(obs['units']),'contacts':len(obs['contacts'])})
            if obs['tick']%600==0:
                save(output/'observations'/f'{obs["tick"]:05d}.json',obs)
                print(json.dumps({'policy':policy,'backend':backend,'tick':obs['tick'],
                                  'calls_completed':len(records),'inflight':inflight is not None}),flush=True)
        if inflight is not None:
            receipt=future.result() if future else inflight['receipt']
            records.append({'id':inflight['id'],'request_tick':inflight['tick'],
                'received_tick':None,'apply_tick':None,'usable':False,
                'status':receipt['status'],'terminal_inflight':True,
                'elapsed_s':receipt.get('elapsed_s'),'usage':receipt.get('usage'),
                'reason':inflight['reason'],'commands_proposed':0})
        checkpoint=native.ask(op='dump',path=str(output/'checkpoint.json'))
        save(output/'checkpoint-metadata.json',checkpoint)
        replay=native.ask(op='replay')
        save(output/'replay-validation.json',replay)
        save(output/'final-observation.json',obs)
        save(output/'scheduler-final.json',state)
        save(output/'feature-final.json',tracker.state)
        save(output/'requests.json',records)
        save(output/'decision-feedback.json',feedback)
        (output/'scheduler-events.jsonl').write_text(''.join(json.dumps(x,ensure_ascii=False)+'\n' for x in audit),encoding='utf-8')
        (output/'timing.jsonl').write_text(''.join(json.dumps(x,ensure_ascii=False)+'\n' for x in traces),encoding='utf-8')
        summary={'protocol':protocol,'tick':obs['tick'],'over':obs['over'],'result':obs.get('result'),
            'wall_s':time.perf_counter()-start,'requests':len(records),
            'valid_receipts':sum(r['status']=='ok' for r in records),
            'usable_receipts':sum(r['usable'] for r in records),
            'terminal_inflight':sum(r.get('terminal_inflight',False) for r in records),
            'own_victory':obs.get('victory'),'replay':replay,
            'stop_after_three_failures':stop_requests,
            'consecutive_failure_tail':consecutive_failures,
            'max_clock_lag_s':max(drifts,default=None),
            'p95_clock_lag_s':sorted(drifts)[int(.95*(len(drifts)-1))] if drifts else None,
            'max_step_wall_s':max(step_durations,default=0.),
            'call_cap_reached':len(records)>=cap and backend in ('script','model'),
            'call_cap_reached_at':records[cap-1].get('received_tick') if cap and len(records)>=cap else None,
            'usage_unknown':sum(r['usage'] is None for r in records) if backend=='model' else None}
        save(output/'summary.json',summary)
        return summary
    except BaseException as e:
        save(output/'failure.json',{'error':repr(e),'requests_completed':records,
            'tick':locals().get('obs',{}).get('tick'),
            'inflight':inflight,'inflight_usage_unknown':inflight is not None and backend=='model',
            'scheduler':state,'feature_tracker':tracker.state,'dispatches':dispatches,
            'limits':'API process/connection resume is not implemented; never silently reissue a charged request'})
        save(output/'requests-progress.json',records)
        save(output/'scheduler-events-partial.json',audit)
        save(output/'timing-partial.json',traces)
        raise
    finally:
        pool.shutdown(wait=True)
        native.close()


def main():
    p=argparse.ArgumentParser();p.add_argument('--stage',type=Path,required=True)
    p.add_argument('--out',type=Path,required=True);p.add_argument('--policy',choices=POLICIES,default='EH')
    p.add_argument('--backend',choices=['script','model','hold','fsm'],required=True)
    p.add_argument('--horizon',type=int,default=10800);p.add_argument('--delay',type=int,default=100)
    p.add_argument('--speed',type=float,default=10);p.add_argument('--seed',type=int,default=20261023)
    p.add_argument('--side',choices=['Blue','Red'],default='Blue');p.add_argument('--cap',type=int,default=32)
    p.add_argument('--task-lease',type=int,default=1800);p.add_argument('--allow-real-model',action='store_true')
    a=p.parse_args()
    if a.backend=='model' and not a.allow_real_model:p.error('real model requires --allow-real-model')
    if not 1<=a.horizon<=10800 or not 0<a.speed<=60 or a.cap<0 or a.delay<0:p.error('invalid limits')
    x=run(a.stage,a.out,a.policy,a.backend,a.horizon,a.delay,a.speed,a.seed,a.side,a.cap,a.task_lease)
    print(json.dumps({'tick':x['tick'],'requests':x['requests'],'result':x['result']}),flush=True)
if __name__=='__main__':main()

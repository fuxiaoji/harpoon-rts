"""Public-only scheduling and nonblocking real-model calls for the judge bridge."""
from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import copy
import json
import math
from pathlib import Path
import subprocess
import time

import llm_client

FIELDS = ("tick", "task_revision", "standing_revision", "task_mismatch", "distance_nm",
          "deadline_remaining_s", "plan_age_s", "notice_seq")
OPS = {"eq": lambda x,y:x==y, "ne":lambda x,y:x!=y, "lt":lambda x,y:x<y,
       "le":lambda x,y:x<=y, "gt":lambda x,y:x>y, "ge":lambda x,y:x>=y}
ATOM = {"type":"object", "properties":{"field":{"type":"string","enum":list(FIELDS)},
        "op":{"type":"string","enum":list(OPS)}, "value":{"type":"number"}},
        "required":["field","op","value"],"additionalProperties":False}
SCHEMA = copy.deepcopy(llm_client.SCHEMA)
SCHEMA["properties"]["wake_conditions"] = {"type":"array","maxItems":3,"items":{
    "type":"object","properties":{"all":{"type":"array","minItems":1,"maxItems":3,"items":ATOM}},
    "required":["all"],"additionalProperties":False}}
SCHEMA["required"].append("wake_conditions")
INSTRUCTIONS = """You control a vessel in a simulated navigation task that may be replaced.
Use only this own observation and public task. Do not use tools, files, internet,
external information or code execution. Return the requested JSON, with a brief
explanation of at most one short sentence and no reasoning trace.
The standing order initially visits revision 0. Choose hold or goto for the own
vessel. Goto can only target the current public goal; the engine follows it at
18 knots. Hold keeps the existing order and does not complete a changed task.
The world continues moving during your response. Task score is earned by entering
radius_nm by deadline_tick. Use the revision of the supplied public task.
Also write future wake_conditions using only the listed numerical wake_features.
Each condition is an AND list of field/op/value comparisons; the list of conditions
is OR. A condition wakes on its false-to-true edge, including if already true when
installed. Conditions are replaced only when a valid response is applied. Empty
conditions request no event wakes. A new task may arrive at an unknown future time;
there is no guaranteed timer fallback in the event-only method. Do not predict
future events from scenario labels. Account for the finite remaining call budget.
Features: tick=current simulated second; task_revision=current public revision;
standing_revision=revision currently being followed; task_mismatch=1 iff they differ;
distance_nm=own distance to public goal; deadline_remaining_s=deadline minus tick;
plan_age_s=seconds since last accepted decision (initially since tick 0);
notice_seq=public notification count, which also increases for routine repeats.
The supplied watch_semantics states whether new task revisions rearm a condition
that remains true; it applies equally to human and model conditions.
"""


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def validate(answer, model_input):
    if set(answer) != set(SCHEMA["required"]):
        raise ValueError("invalid extended plan keys")
    llm_client.validate_action({k:v for k,v in answer.items() if k!="wake_conditions"}, model_input)
    conditions = answer["wake_conditions"]
    if not isinstance(conditions,list) or len(conditions)>3:
        raise ValueError("invalid wake condition count")
    for clause in conditions:
        if set(clause)!={"all"} or not 1<=len(clause["all"])<=3:
            raise ValueError("invalid AND clause")
        for atom in clause["all"]:
            if set(atom)!={"field","op","value"} or atom["field"] not in FIELDS or atom["op"] not in OPS:
                raise ValueError("invalid condition atom")
            if type(atom["value"]) not in (int,float) or not math.isfinite(atom["value"]) or abs(atom["value"])>1e9:
                raise ValueError("invalid numeric threshold")


class Watch:
    def __init__(self, conditions=(), revision_keyed=False):
        self.conditions=list(conditions)
        self.previous=[False]*len(self.conditions)
        self.keys=[None]*len(self.conditions)
        self.revision_keyed=revision_keyed

    def poll(self, features):
        truth=[all(OPS[a["op"]](features[a["field"]],a["value"]) for a in c["all"]) for c in self.conditions]
        hit=False
        for index,(current,old) in enumerate(zip(truth,self.previous)):
            if current and (not old or self.revision_keyed and self.keys[index]!=features["task_revision"]):
                hit=True
                self.keys[index]=features.get("task_revision")
        self.previous=truth
        return hit


def features(model_input, last_apply):
    obs,task=model_input["observation"],model_input["public_task"]
    pos=obs["units"][0]["pos"]
    return {"tick":obs["tick"], "task_revision":task["revision"],
            "standing_revision":model_input["standing_revision"],
            "task_mismatch":int(task["revision"]!=model_input["standing_revision"]),
            "distance_nm":math.hypot(pos["x"]-task["goal"]["x"],pos["y"]-task["goal"]["y"]),
            "deadline_remaining_s":task["deadline_tick"]-obs["tick"],
            "plan_age_s":obs["tick"]-last_apply,"notice_seq":model_input.get("public_notice_seq",0)}


def run_episode(binary, cwd, output, calls_root, tag, family, policy, seed,
                tick_ms=100, tight_window=300, max_calls=6, effort="low", backend="codex",
                revision_keyed=False, skip_expired=False, script_delay_ms=6):
    output.mkdir(parents=True,exist_ok=False)
    stderr=(output/"bridge-stderr.log").open("w",encoding="utf-8")
    process=subprocess.Popen([str(binary),"--case",family,"--seed",str(seed),"--tight-window-s",str(tight_window)],
        cwd=cwd,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=stderr,text=True,encoding="utf-8",bufsize=1)

    def receive():
        line=process.stdout.readline()
        if not line:
            raise RuntimeError("judge bridge ended; see bridge-stderr.log")
        return json.loads(line)

    def exchange(message):
        process.stdin.write(json.dumps(message)+"\n")
        process.stdin.flush()
        return receive()

    initial=receive()["input"]
    horizon=initial["public_task"]["deadline_tick"]
    model_input=initial
    requests,clock_log,triggers=[],[],[]
    monitor_ns=0
    executor=ThreadPoolExecutor(max_workers=1)
    pending=None
    queued=None
    latched=None
    last_apply=0
    human=Watch([{"all":[{"field":"task_mismatch","op":"eq","value":1}]}],revision_keyed)
    model_watch=Watch(revision_keyed=revision_keyed)
    skipped=[]
    started=time.perf_counter_ns()
    wall_us=lambda:(time.perf_counter_ns()-started)//1000

    def worker(model_input, call_dir):
        began=wall_us()
        if backend=="codex":
            record=llm_client.call(model_input,effort,call_dir,120,SCHEMA,INSTRUCTIONS,validate)
        else:
            call_dir.mkdir(parents=True,exist_ok=False)
            simulated_start=time.perf_counter()
            time.sleep(script_delay_ms/1000)
            task=model_input["public_task"]
            answer={"action":"hold" if model_input["standing_revision"]==task["revision"] else "goto",
                    "unit":model_input["observation"]["units"][0]["id"],"waypoint":task["goal"],
                    "task_revision":task["revision"],"brief":"Scripted instrumentation control.",
                    "wake_conditions":[{"all":[{"field":"task_mismatch","op":"eq","value":1}]}]}
            validate(answer,model_input)
            record={"status":"ok","answer":answer,"usage":None,"elapsed_s":time.perf_counter()-simulated_start,
                    "requested_sleep_s":script_delay_ms/1000,"money_cost":None,
                    "requested_model":None,"backend":"scripted_not_llm"}
            save(call_dir/"input.json",model_input);save(call_dir/"result.json",record)
        return record,began,wall_us()

    def collect(now, after_horizon=False):
        nonlocal pending,queued
        future,index=pending
        try:
            record,began,ended=future.result()
        except Exception as error:
            record={"status":"failed","usage":None,"answer":None,"worker_exception":repr(error),"money_cost":None}
            began,ended=requests[index]["requested_wall_us"],wall_us()
        row=requests[index]
        row.update(record=record,worker_started_wall_us=began,response_wall_us=ended,
                   response_tick=now,validated_wall_us=wall_us(),validated_tick=now)
        pending=None
        if after_horizon:
            row["status"]="in_flight_at_horizon"
        elif record["status"]!="ok":
            row["status"]="model_failed"
        elif row["input"]["public_task"]["revision"]!=model_input["public_task"]["revision"]:
            row["status"]="stale_response"
        elif now+1>model_input["public_task"]["deadline_tick"]:
            row["status"]="expired_response"
        elif now==horizon:
            row["status"]="after_horizon"
        else:
            row["status"]="queued"
            queued=(index,record["answer"])

    try:
        for now in range(horizon+1):
            due=started+now*tick_ms*1_000_000
            remaining=(due-time.perf_counter_ns())/1e9
            if remaining>0:time.sleep(remaining)
            if now:
                payload=None
                if queued:
                    index,answer=queued
                    payload={"request_id":index,**{k:v for k,v in answer.items() if k!="wake_conditions"}}
                packet=exchange({"kind":"step","tick":now,"plan":payload})
                model_input=packet["input"]
                if queued:
                    index,answer=queued
                    application=packet["application"]
                    requests[index]["status"]=application["status"]
                    if application["status"]=="applied":
                        last_apply=now
                        requests[index].update(applied_tick=now,applied_wall_us=wall_us(),commands=application["commands"])
                        if policy in ("em","hm600"):
                            model_watch=Watch(answer["wake_conditions"],revision_keyed)
                    queued=None
            observed_wall_us=wall_us()
            clock_log.append({"tick":now,"observed_wall_us":observed_wall_us,
                              "lag_us":observed_wall_us-now*tick_ms*1000})
            if pending and pending[0].done():collect(now)
            monitor_started=time.perf_counter_ns()
            public_features=features(model_input,last_apply)
            reason=None
            if policy in ("eh","hh600") and human.poll(public_features):reason="human_condition"
            if policy in ("em","hm600"):
                if now==0:reason="model_watch_bootstrap"
                elif model_watch.poll(public_features):reason="model_condition"
            period={"f60":60,"f300":300,"f600":600,"hh600":600,"hm600":600}.get(policy)
            if period and now>0 and now%period==0:reason=(reason+"+fixed_grid") if reason else "fixed_grid"
            monitor_ns+=time.perf_counter_ns()-monitor_started
            if reason:
                triggers.append({"tick":now,"wall_us":wall_us(),"reason":reason})
                if latched is None:latched=(now,wall_us(),reason)
            if skip_expired and latched and now>model_input["public_task"]["deadline_tick"]:
                skipped.append({"tick":now,"trigger":latched,"reason":"public_task_expired"})
                latched=None
            if latched and pending is None and queued is None and now<horizon and len(requests)<max_calls:
                trigger_tick,trigger_wall,reason=latched
                latched=None
                index=len(requests)
                snapshot=copy.deepcopy(model_input)
                snapshot["wake_features"]=public_features
                snapshot["watch_semantics"]="false-to-true or true on a new task revision" if revision_keyed else "false-to-true only"
                snapshot["controller_budget"]={"calls_remaining_after_this":max_calls-index-1,
                    "simulation_seconds_per_wall_second":1000/tick_ms}
                row={"id":index,"input":snapshot,"triggered_tick":trigger_tick,"triggered_wall_us":trigger_wall,
                     "trigger":reason,"observed_tick":now,"observed_wall_us":observed_wall_us,
                     "requested_tick":now,"requested_wall_us":wall_us(),"status":"in_flight",
                     "response_tick":None,"applied_tick":None}
                requests.append(row)
                call_dir=calls_root/f"{tag}-{index:02d}"
                row["call_directory"]=str(call_dir)
                pending=(executor.submit(worker,snapshot,call_dir),index)
                save(output/"requests-progress.json",requests)
            if now>0 and now%300==0:
                print(json.dumps({"episode":tag,"tick":now,"requests":len(requests),
                                  "in_flight":pending is not None,"wall_s":wall_us()/1e6}),flush=True)
        episode_wall_us=wall_us()
        engine=exchange({"kind":"finish"})
        process.wait(timeout=10)
        if process.returncode:raise RuntimeError("judge bridge failed")
        save(output/"engine.json",engine)
        if pending:collect(horizon,True)
    finally:
        executor.shutdown(wait=True)
        if process.poll() is None:process.terminate();process.wait(timeout=10)
        stderr.close()
    usage={k:sum((r.get("record",{}).get("usage") or {}).get(k,0) for r in requests)
           for k in ("input_tokens","output_tokens","reasoning_output_tokens","cached_input_tokens")}
    result={"stage":"exploratory live navigation wake study", "backend":backend,"family":family,
            "controller_version":2,"revision_keyed":revision_keyed,"skip_expired":skip_expired,
            "script_delay_ms":script_delay_ms,"skipped_triggers":skipped,
            "policy":policy,"seed":seed,"tick_ms":tick_ms,"tight_window_s":tight_window,"max_calls":max_calls,
            "effort":effort,"model":llm_client.MODEL if backend=="codex" else None,
            "success":engine["success"],"reached_tick":engine["reached_tick"],"requests":requests,
            "calls":len(requests),"usage":usage if backend=="codex" else None,"money_cost":None,
            "episode_wall_us":episode_wall_us,"clock":clock_log,"triggers":triggers,
            "monitor_checks":horizon+1,"monitor_wall_ns":monitor_ns,"world_hash":engine["world_hash"],
            "replay_verified":engine["replay_verified"],"latched_at_end":latched,
            "budget_exhausted":len(requests)>=max_calls}
    save(output/"run.json",result)
    return result

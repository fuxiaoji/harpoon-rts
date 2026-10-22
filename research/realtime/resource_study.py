"""Public finite resource diagnostic: generation, exact combination, real calls.

Offline catalog construction is measured and uses only the same public case.
No provider is reachable from prepare/control/analyze modes.
"""
from __future__ import annotations
import argparse
import copy
import csv
import functools
import hashlib
import itertools
import json
import math
from pathlib import Path
import random
import shutil
import subprocess
import sys
import time


def read(p): return json.loads(Path(p).read_text(encoding="utf-8"))
def save(p,v): Path(p).write_text(json.dumps(v,ensure_ascii=False,indent=2),encoding="utf-8")
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def verify_stage(stage):
    for row in read(stage/"source-manifest.json"):
        assert sha(stage/"frozen-source"/row["path"])==row["sha256"]
    for name,digest in read(stage/"prepare-validation.json")["binaries"].items():
        assert sha(stage/f"target/release/examples/{name}.exe")==digest

SCHEMA={"type":"object","properties":{
    "routes":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"object","properties":{
        "unit":{"type":"integer"},"task_ids":{"type":"array","maxItems":4,"items":{"type":"integer"}}},
        "required":["unit","task_ids"],"additionalProperties":False}},
    "brief":{"type":"string"}},"required":["routes","brief"],"additionalProperties":False}
INSTRUCTIONS="""Allocate three fictional exercise vessels to public service tasks.
Use only this input. No tools, files, code execution or internet. Return JSON only,
with one route per own unit and at most four ordered tasks per unit. Tasks may be
omitted; each task may appear at most once fleet-wide. Maximize actual total reward.
Task service completes after navigating within 0.12 nautical miles of its goal
and holding there for service_s. Reward requires service completion <= deadline.
Late tasks still consume travel/service/resources. No automatic skipping occurs.
Every assigned task requires the vessel's skill and consumes its load capacity.
A predecessor must be earlier in the SAME vessel's route (physical completion,
even if late); different vessels cannot share a predecessor. All vessels must
return within 0.12 nm of their own home by horizon. Any skill, predecessor, load
or return violation makes that entire vessel route score zero. Shared kits are
the sum for ALL assigned tasks; exceeding shared_kits makes the whole fleet score
zero. Empty routes stay at home. There is no refueling or resource refund.
Navigation uses 18 knots, gradual turning (1.8 degrees/second), approach cap
15 knots within 0.5 nm, waypoint pop radius 0.1 nm. Initial course east, stopped.
Service stops/clears navigation; the next departure retains the actual heading
and position. Consider follow-up rewards, resource opportunity costs and return.
Give a single short public explanation, not a reasoning trace.
"""

def validate(plan,case):
    if set(plan)!={"routes","brief"} or not isinstance(plan["brief"],str) or len(plan["routes"])!=3:
        raise ValueError("invalid response shape")
    units=set(); tasks=set()
    for r in plan["routes"]:
        if set(r)!={"unit","task_ids"} or type(r["unit"]) is not int or r["unit"] not in (1,2,3) or r["unit"] in units:
            raise ValueError("invalid own unit")
        units.add(r["unit"])
        if not isinstance(r["task_ids"],list) or len(r["task_ids"])>4: raise ValueError("route too long")
        for t in r["task_ids"]:
            if type(t) is not int or not 0<=t<len(case["tasks"]) or t in tasks: raise ValueError("invalid/duplicate task")
            tasks.add(t)
    # Semantic mistakes remain valid actions and receive their real zero score.

def generate(seed,complexity):
    rng=random.Random(seed)
    # Locations jittered independently of phase/urgency/budget, never on results.
    reflect=-1 if seed%2 else 1
    def pos(x,y): return {"x":round(x+rng.uniform(-.12,.12),3),"y":round(reflect*y+rng.uniform(-.12,.12),3)}
    vessels=[{"id":1,"pos":{"x":0,"y":-1},"home":{"x":0,"y":-1},"capacity":5,"skills":["common","survey"]},
        {"id":2,"pos":{"x":0,"y":1},"home":{"x":0,"y":1},"capacity":5,"skills":["common","recovery"]},
        {"id":3,"pos":{"x":0,"y":0},"home":{"x":0,"y":0},"capacity":5,"skills":["common","survey","recovery"]}]
    specs=[(1.0,-.65,560,2,45,1,1,"survey",None),
        (1.85,-.8,1000,18,75,2,2,"recovery",0),
        (1.05,.6,580,3,40,1,1,"recovery",None),
        (1.75,.9,1030,17,85,2,2,"survey",2),
        (.7,-1.15,560,9,65,2,1,"common",None),
        (.65,1.2,590,8,60,2,1,"recovery",None),
        (2.25,-.15,1000,13,80,3,2,"survey",None),
        (1.95,.05,920,12,100,2,2,"common",None),
        (.5,.1,380,6,50,1,1,"common",None)]
    if complexity=="small":
        specs=[(.7,-1.15,560,9,65,2,1,"common",None),(.65,1.2,590,8,60,2,1,"recovery",None),
            (1.25,-.15,800,13,80,3,2,"survey",None),(.5,.1,380,6,50,1,1,"common",None)]
    tasks=[dict(id=i,goal=pos(x,y),deadline=d,reward=r,service_s=s,load=l,kits=k,skill=skill,predecessor=pre)
        for i,(x,y,d,r,s,l,k,skill,pre) in enumerate(specs)]
    return {"vessels":vessels,"tasks":tasks,"horizon":1400,"shared_kits":6 if complexity=="complex" else 5}

def invoke(binary,mode,case,output,plan=None,delay=0):
    cmd=[str(binary),mode,str(case)]
    if plan is not None: cmd.extend([str(plan),str(delay)])
    start=time.perf_counter()
    p=subprocess.run(cmd,capture_output=True,text=True,encoding="utf-8",errors="replace")
    if p.returncode: raise RuntimeError(p.stderr[-3000:])
    Path(output).write_text(p.stdout,encoding="utf-8")
    return json.loads(p.stdout),time.perf_counter()-start

def masks(catalog):
    tables={u:{} for u in (1,2,3)}
    for row in catalog:
        if not row["feasible"]:continue
        u,m=row["unit"],row["mask"]
        old=tables[u].get(m)
        key=lambda r:(r["score"],-r["return_tick"],tuple(-i for i in r["task_ids"]))
        if old is None or key(row)>key(old):tables[u][m]=row
    return tables

def solve(case,catalog):
    tables=masks(catalog)
    kit=lambda m:sum(t["kits"] for t in case["tasks"] if m&(1<<t["id"]))
    dp={0:(0,[])}
    for u in (1,2,3):
        nxt={}
        for used,(score,paths) in dp.items():
            for mask,row in tables[u].items():
                union=used|mask
                if used&mask or kit(union)>case["shared_kits"]:continue
                value=score+row["score"]
                if union not in nxt or value>nxt[union][0]:nxt[union]=(value,paths+[row])
        dp=nxt
    score,paths=max(dp.values(),key=lambda p:p[0])
    # Independently structured exact subset recurrence; no shared DP states.
    @functools.lru_cache(None)
    def check(u,remaining):
        if u==0:return 0
        best=0;subset=remaining
        while True:
            row=tables[u].get(subset)
            if row is not None:best=max(best,row["score"]+check(u-1,remaining^subset))
            if subset==0:break
            subset=(subset-1)&remaining
        return best
    bound=max(check(3,m) for m in range(1<<len(case["tasks"])) if kit(m)<=case["shared_kits"])
    assert score==bound
    return {"routes":[{"unit":r["unit"],"task_ids":r["task_ids"]} for r in paths],"brief":"Measured public exact finite planner."},score

def greedy(case,catalog):
    legal={(r["unit"],tuple(r["task_ids"])):r for r in catalog if r["feasible"]}
    routes={u:[] for u in (1,2,3)};used=set();kits=0
    while True:
        choices=[]
        for u,ids in routes.items():
            if len(ids)==4:continue
            old=legal[(u,tuple(ids))]
            for t in case["tasks"]:
                if t["id"] in used or kits+t["kits"]>case["shared_kits"]:continue
                r=legal.get((u,tuple(ids+[t["id"]])))
                if r is not None and r["score"]>old["score"]:
                    choices.append((r["score"]-old["score"],-r["return_tick"],-u,-t["id"]))
        if not choices:break
        _,_,nu,nt=max(choices);u,t=-nu,-nt
        routes[u].append(t);used.add(t);kits+=case["tasks"][t]["kits"]
    return {"routes":[{"unit":u,"task_ids":ids} for u,ids in routes.items()],"brief":"Immediate marginal reward greedy; actual route feasibility checked."}

def audit_evaluation(case,plan,result):
    validate(plan,case)
    kits=sum(case["tasks"][t]["kits"] for r in plan["routes"] for t in r["task_ids"])
    assert kits==result["kits"] and result["shared_budget_ok"]==(kits<=case["shared_kits"])
    total=0
    for route,row in zip(plan["routes"],result["routes"]):
        assert route["unit"]==row["unit"] and route["task_ids"]==row["task_ids"]
        v=next(v for v in case["vessels"] if v["id"]==row["unit"])
        seen=set();structural=True;load=0
        for i in route["task_ids"]:
            t=case["tasks"][i];load+=t["load"]
            structural &= t["skill"] in v["skills"] and (t["predecessor"] is None or t["predecessor"] in seen) and load<=v["capacity"]
            seen.add(i)
        if not structural:assert not row["feasible"]
        if row["feasible"]:assert structural and row["return_tick"]<=case["horizon"] and row["reason"]=="ok"
        score=0;previous=result["delay_s"]
        for visit in row["visits"]:
            t=case["tasks"][visit["task"]]
            assert previous<=visit["arrived"] and visit["completed"]==visit["arrived"]+t["service_s"]
            assert visit["on_time"]==(visit["completed"]<=t["deadline"])
            assert math.hypot(visit["pos"]["x"]-t["goal"]["x"],visit["pos"]["y"]-t["goal"]["y"])<=.120001
            previous=visit["completed"]
            if visit["on_time"]:score+=t["reward"]
        assert row["score"]==(score if row["feasible"] else 0)
        total+=row["score"]
    assert result["earned"]==(total if result["shared_budget_ok"] else 0)

def prepare(stage):
    frozen=stage/"frozen-source";binary=stage/"target/release/examples/realtime_resource.exe"
    data=stage/"quality";data.mkdir(exist_ok=False)
    rows=[]
    for complexity,seed in itertools.product(("small","complex"),(11,23,37)):
        tag=f"{complexity}-{seed}";folder=data/tag;folder.mkdir()
        case=generate(seed,complexity);save(folder/"input.json",case)
        catalog,elapsed=invoke(binary,"--catalog",folder/"input.json",folder/"catalog.json")
        began=time.perf_counter();plan,opt=solve(case,catalog["catalog"]);combination=time.perf_counter()-began
        save(folder/"reference-plan.json",plan)
        actual,_=invoke(binary,"--evaluate",folder/"input.json",folder/"reference-score.json",folder/"reference-plan.json")
        audit_evaluation(case,plan,actual);assert actual["earned"]==opt and opt>0
        cheap=greedy(case,catalog["catalog"]);save(folder/"greedy-plan.json",cheap)
        gs,_=invoke(binary,"--evaluate",folder/"input.json",folder/"greedy-score.json",folder/"greedy-plan.json")
        audit_evaluation(case,cheap,gs)
        rows.append({"tag":tag,"complexity":complexity,"seed":seed,"optimum":opt,"greedy":gs["earned"],
            "routes_enumerated":len(catalog["catalog"]),"catalog_wall_s":elapsed,"combination_wall_s":combination,"input_sha256":sha(folder/"input.json")})
        print(json.dumps(rows[-1]),flush=True)
    save(data/"offline-summary.json",rows)
    order=[]
    block=list(rows);random.Random(20261022).shuffle(block)
    for i,row in enumerate(block):
        efforts=["low","medium","high"];random.Random(20261022+i).shuffle(efforts)
        for effort in efforts:order.append({"tag":row["tag"],"effort":effort})
    save(data/"intent.json",{"stage":"M3a frozen fixed-snapshot exploratory quality diagnostic","order":order,
        "max_requests":18,"retries":0,"model":"gpt-6.1-sol","clock":"paused fixed snapshots, not real-time",
        "instructions_sha256":hashlib.sha256(INSTRUCTIONS.encode()).hexdigest(),"money":None})
    save(data/"prepare-validation.json",{"status":"passed","cases":6,"positive_optima":True,
        "exact_solvers_agree":True,"large_greedy_gaps":[r["optimum"]-r["greedy"] for r in rows if r["complexity"]=="complex"]})

def run(stage):
    # This function is the only provider entry; no defaults infer this mode.
    import llm_client
    data=stage/"quality";intent=read(data/"intent.json")
    assert len(intent["order"])==intent["max_requests"]==18
    calls=data/"calls";calls.mkdir(exist_ok=False)
    rows=[];refs={r["tag"]:r for r in read(data/"offline-summary.json")}
    for index,item in enumerate(intent["order"]):
        tag=item["tag"];case=read(data/tag/"input.json")
        assert sha(data/tag/"input.json")==refs[tag]["input_sha256"]
        print(json.dumps({"request":index,"case":tag,"effort":item["effort"]}),flush=True)
        record=llm_client.call(case,item["effort"],calls/f"r{index:03d}",180,SCHEMA,INSTRUCTIONS,validate)
        rows.append({**item,"index":index,"call_dir":str(calls/f"r{index:03d}"),"status":record["status"]})
        save(data/"request-ledger.json",rows)
    save(data/"calls-complete.json",{"requests":len(rows),"successful":sum(r["status"]=="ok" for r in rows)})

def analyze(stage):
    data=stage/"quality";binary=stage/"target/release/examples/realtime_resource.exe"
    refs={r["tag"]:r for r in read(data/"offline-summary.json")};rows=[]
    for item in read(data/"request-ledger.json"):
        directory=Path(item["call_dir"]);record=read(directory/"result.json");case=read(data/item["tag"]/"input.json")
        assert read(directory/"input.json")==case
        assert record["input_sha256"]==hashlib.sha256(json.dumps(case,sort_keys=True).encode()).hexdigest()
        assert record["effort"]==item["effort"] and record["requested_model"]=="gpt-6.1-sol" and not record["tool_violations"]
        row={**item,"latency_s":record["elapsed_s"],"usage":record["usage"],"money":None,"optimum":refs[item["tag"]]["optimum"],"scores":{}}
        if record["status"]=="ok":
            save(directory/"plan.json",record["answer"])
            for urgency in ("wide","tight"):
                c=copy.deepcopy(case)
                if urgency=="tight":
                    c["horizon"]-=180
                    for t in c["tasks"]:t["deadline"]-=180
                save(directory/f"{urgency}-case.json",c)
                for delay in sorted({0,60,180,math.ceil(record["elapsed_s"]*5)}):
                    result,_=invoke(binary,"--evaluate",directory/f"{urgency}-case.json",directory/f"{urgency}-{delay}.json",directory/"plan.json",delay)
                    audit_evaluation(c,record["answer"],result)
                    row["scores"][f"{urgency}-{delay}"]=result["earned"]
                    if urgency=="wide" and delay==0:row["feasible"]=result["shared_budget_ok"] and all(r["feasible"] for r in result["routes"])
            # Independent fresh-process deterministic replay of the primary score.
            repeat,_=invoke(binary,"--evaluate",data/item["tag"]/"input.json",directory/"repeat.json",directory/"plan.json",0)
            assert repeat==read(directory/"wide-0.json")
        else:row["scores"]["wide-0"]=0;row["feasible"]=False
        row["normalized"]=row["scores"]["wide-0"]/row["optimum"]
        rows.append(row)
    byeffort={e:[r for r in rows if r["effort"]==e] for e in ("low","medium","high")}
    summary={e:{"n":len(rs),"raw_score":sum(r["scores"]["wide-0"] for r in rs),
        "mean_normalized":sum(r["normalized"] for r in rs)/len(rs),"feasible":sum(r["feasible"] for r in rs),
        "latencies_s":[r["latency_s"] for r in rs],
        "usage":{k:sum((r["usage"] or {}).get(k,0) for r in rs) for k in ("input_tokens","output_tokens","reasoning_output_tokens","cached_input_tokens")}}
        for e,rs in byeffort.items()}
    pairs=[]
    for tag in refs:
        these={r["effort"]:r for r in rows if r["tag"]==tag}
        pairs.append({"tag":tag,"optimum":refs[tag]["optimum"],"greedy":refs[tag]["greedy"],
            "scores":{e:r["scores"]["wide-0"] for e,r in these.items()},
            "high_minus_low":these["high"]["scores"]["wide-0"]-these["low"]["scores"]["wide-0"]})
    result={"rows":rows,"summary":summary,"pairs":pairs,"money":None,"independent_sample_units":6,"calls":len(rows)}
    save(data/"analysis.json",result)
    with (data/"summary.csv").open("w",newline="",encoding="utf-8-sig") as f:
        writer=csv.DictWriter(f,fieldnames=["tag","optimum","greedy","low","medium","high","high_minus_low"]);writer.writeheader()
        for p in pairs:writer.writerow({k:v for k,v in p.items() if k!="scores"}|p["scores"])
    print(json.dumps({"summary":summary,"pairs":pairs}),flush=True)

if __name__=="__main__":
    p=argparse.ArgumentParser();p.add_argument("mode",choices=["prepare","run","analyze"]);p.add_argument("--stage",type=Path,required=True);a=p.parse_args()
    verify_stage(a.stage)
    {"prepare":prepare,"run":run,"analyze":analyze}[a.mode](a.stage)

"""Provider-free startup/geometry/phase controls, separately frozen from M2a."""
import argparse
import hashlib
import importlib
import itertools
import json
from pathlib import Path
import subprocess
import sys

def read(p):return json.loads(p.read_text(encoding="utf-8"))
def save(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2),encoding="utf-8")
def main():
    p=argparse.ArgumentParser();p.add_argument("--stage",type=Path,required=True);a=p.parse_args();stage=a.stage
    frozen=stage/"frozen-source";sys.path.insert(0,str(frozen/"research/realtime"))
    live=importlib.import_module("m2b_controller");audit=importlib.import_module("m2b_audit").audit
    def forbid(*args,**kwargs):raise AssertionError("M2b must never invoke provider")
    live.llm_client.call=forbid
    for r in read(stage/"source-manifest.json"):assert hashlib.sha256((frozen/r["path"]).read_bytes()).hexdigest()==r["sha256"]
    binary=stage/"target/release/examples/realtime_controlled.exe"
    assert hashlib.sha256(binary.read_bytes()).hexdigest()==read(stage/"prepare-validation.json")["binaries"]["realtime_controlled"]
    folder=stage/"timing";folder.mkdir(exist_ok=False);(folder/"calls").mkdir()
    order=[]
    for phase,gap,window,method in itertools.product((17,83,251,419),(30,180),(420,600),("gf60","gf300","gf600","eh","em","ehq30","emcold")):
        order.append({"phase":phase,"gap":gap,"window":window,"method":method,"ready":method!="emcold"})
    save(folder/"intent.json",{"order":order,"backend":"scripted","clock":"injected","delay":100,"startup_cost":"ready arms one common scripted preflight outside clock; online cap5; cold EM online cap6"})
    rows=[]
    for i,c in enumerate(order):
        tag=f"r{i:03d}";directory=folder/tag
        cp={"first_event_tick":c["phase"],"gap_s":c["gap"],"event_window_s":c["window"],"episode_s":1400,"notice_every_s":30,"second_goal_dx_nm":.15,"controlled_geometry":1}
        method=c["method"];policy={"gf60":"f60","gf300":"f300","gf600":"f600","eh":"eh","em":"em","ehq30":"eh","emcold":"em"}[method]
        result=live.run_episode(binary,frozen,directory,folder/"calls",tag,"d5",policy,7,backend="scripted",clock_mode="injected",delay_sim_s=100,
            max_calls=5 if c["ready"] else 6,case_parameters=cp,ready_watch=c["ready"],event_quiet_s=30 if method=="ehq30" else 0)
        engine=read(directory/"engine.json");check=audit(result,engine)
        assert all(r["record"]["requested_model"] is None for r in result["requests"])
        assert engine["updates"][0]["goal"]=={"x":1.5,"y":.6} and engine["updates"][1]["goal"]=={"x":1.65,"y":-.6}
        assert all(p["pos"]=={"x":0.,"y":0.} for p in engine["positions"][:c["phase"]])
        proc=subprocess.run([str(binary),"--replay",str(directory/"engine.json")],cwd=frozen,capture_output=True,text=True)
        assert proc.returncode==0;(directory/"replay.log").write_text(proc.stdout+proc.stderr,encoding="utf-8")
        row={"case":c,"audit":check,"calls_total":result["calls"]+int(c["ready"]),
            **{k:result[k] for k in ("success","calls","reached_tick","world_hash","initialization_retries_used")}}
        assert row["calls_total"]<=6;rows.append(row);save(folder/"summary.json",rows)
        if i%14==0:print(json.dumps({"done":i+1,"of":len(order)}),flush=True)
    for phase,gap,window in itertools.product((17,83,251,419),(30,180),(420,600)):
        matched=[r for r in rows if (r["case"]["phase"],r["case"]["gap"],r["case"]["window"])==(phase,gap,window)]
        these={r["case"]["method"]:r for r in matched}
        assert these["eh"]["world_hash"]==these["em"]["world_hash"] and these["eh"]["calls"]==these["em"]["calls"]
    summary={m:{"n":len(rs),"success":sum(r["success"] for r in rs),"calls_online":sum(r["calls"] for r in rs),"calls_total":sum(r["calls_total"] for r in rs)}
        for m in ("gf60","gf300","gf600","eh","em","ehq30","emcold") for rs in [[r for r in rows if r["case"]["method"]==m]]}
    save(folder/"validation.json",{"status":"passed","episodes":len(rows),"real_model_calls":0,"paired_same_rule_worlds":16,"summary":summary})
    print(json.dumps(read(folder/"validation.json")),flush=True)
if __name__=="__main__":main()

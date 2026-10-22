"""Posthoc fixed-goal geometry sensitivity; frozen source, no provider calls."""
import argparse
import hashlib
import importlib
import itertools
import json
from pathlib import Path
import subprocess
import sys


def read(path):return json.loads(path.read_text(encoding="utf-8"))
def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding="utf-8")


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument("--stage",type=Path,required=True);args=p.parse_args();root=args.stage
    frozen=root/"frozen-source";sys.path.insert(0,str(frozen/"research/realtime"))
    for item in read(root/"source-manifest.json"):assert hashlib.sha256((frozen/item["path"]).read_bytes()).hexdigest()==item["sha256"]
    live=importlib.import_module("m2_controller");audit=importlib.import_module("m2_audit").audit
    def forbid(*args,**kwargs):raise AssertionError("geometry control forbids provider calls")
    live.llm_client.call=forbid
    binary=root/"target/release/examples/realtime_live.exe"
    assert hashlib.sha256(binary.read_bytes()).hexdigest()==read(root/"prepare-validation.json")["binary_sha256"]
    batch=root/"geometry";batch.mkdir(exist_ok=False);(batch/"calls").mkdir()
    order=[]
    for phase,gap,window,method in itertools.product((59,239),(30,180),(420,600),("gf300","eh","em","public")):
        order.append({"parameters":{"first_event_tick":phase,"gap_s":gap,"event_window_s":window,"episode_s":1080,"notice_every_s":30,"second_goal_dx_nm":0.15},
            "seed":0,"method":method,"budget":6,"delay":100,"backend":"public" if method=="public" else "scripted","clock":"injected","family":"d5"})
    save(batch/"protocol.json",{"stage":"M2a posthoc fixed-goal sensitivity","order":order,
         "analysis":"scripted control only; all target locations fixed within phase across gaps; compare with original counterfactual-trajectory-normalized goal convention",
         "independent_variable":"gap no longer changes second target coordinates; first goal remains normalized to publication position",
         "boundary":"phase59/239 are congruent modulo60; this does not test all 60s cadence phases"})
    rows=[];methods={"gf300":"f300","eh":"eh","em":"em","public":"eh"}
    for i,c in enumerate(order):
        tag=f"r{i:04d}";directory=batch/"episodes"/tag
        result=live.run_episode(binary,frozen,directory,batch/"calls",tag,"d5",methods[c["method"]],0,backend=c["backend"],clock_mode="injected",delay_sim_s=100,
                               case_parameters=c["parameters"],max_calls=6)
        assert result["backend"]==c["backend"] and all(r["record"]["requested_model"] is None for r in result["requests"])
        engine=read(directory/"engine.json");check=audit(result,engine)
        proc=subprocess.run([str(binary),"--replay",str(directory/"engine.json")],cwd=frozen,capture_output=True,text=True)
        assert proc.returncode==0;(directory/"replay.log").write_text(proc.stdout+proc.stderr,encoding="utf-8")
        rows.append({"index":i,"case":c,"trace":str(directory/"run.json"),"audit":check,
            **{k:result[k] for k in ("success","reached_tick","terminal_tick","calls","usage","initialization_retries_used","watch_initialized","world_hash")}})
        save(batch/"summary.json",rows)
    assert all(r["success"] for r in rows if r["case"]["method"]=="public")
    save(batch/"validation.json",{"status":"passed","episodes":len(rows),"independent_audits":len(rows),"cross_process_replays":len(rows),"real_model_requests":0})
    print(json.dumps(read(batch/"validation.json")),flush=True)


if __name__=="__main__":main()

"""Post-discovery scripted condition-removal controls; no real model calls."""
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
    frozen=root/"frozen-source"
    for item in read(root/"source-manifest.json"):
        assert hashlib.sha256((frozen/item["path"]).read_bytes()).hexdigest()==item["sha256"]
    sys.path.insert(0,str(frozen/"research/realtime"))
    live=importlib.import_module("m2_controller");audit=importlib.import_module("m2_audit").audit
    def no_provider_calls(*args,**kwargs):raise AssertionError("this control runner forbids all provider calls")
    live.llm_client.call=no_provider_calls
    binary=root/"target/release/examples/realtime_live.exe"
    assert hashlib.sha256(binary.read_bytes()).hexdigest()==read(root/"prepare-validation.json")["binary_sha256"]
    batch=root/"mechanism-fixed-v2";batch.mkdir(exist_ok=False);(batch/"calls").mkdir()
    order=[]
    for phase,gap,window,empty in itertools.product((59,239),(30,180),(420,600),(False,True)):
        order.append({"parameters":{"first_event_tick":phase,"gap_s":gap,"event_window_s":window,"episode_s":1080,"notice_every_s":30},
                      "seed":0,"method":"em","budget":6,"delay":100,"backend":"scripted","clock":"injected","family":"d5",
                      "label":"empty" if empty else "rules","scripted_conditions":[] if empty else None})
    save(batch/"protocol.json",{"stage":"M2a post-discovery mechanism control","mode":"mechanism","order":order,
        "analysis":"exploratory scripted removal of wake rules after observing initialization-only successes; never a new model sample",
        "controlled":"same current-goal planner, fixed 100s availability, case, seed, budget, recovery and controller; only wake condition list changes",
        "intent":"test whether any learned-condition event is necessary in early dense cases, and whether later changes still require conditions"})
    rows=[]
    for i,c in enumerate(order):
        tag=f"r{i:04d}";directory=batch/"episodes"/tag
        assert c["backend"]=="scripted"
        result=live.run_episode(binary,frozen,directory,batch/"calls",tag,"d5","em",0,backend="scripted",clock_mode="injected",delay_sim_s=100,
            case_parameters=c["parameters"],scripted_conditions=c["scripted_conditions"])
        engine=read(directory/"engine.json");check=audit(result,engine)
        assert result["backend"]=="scripted" and all(r["record"]["requested_model"] is None for r in result["requests"])
        proc=subprocess.run([str(binary),"--replay",str(directory/"engine.json")],cwd=frozen,capture_output=True,text=True)
        assert proc.returncode==0
        (directory/"replay.log").write_text(proc.stdout+proc.stderr,encoding="utf-8")
        rows.append({"index":i,"case":c,"trace":str(directory/"run.json"),"audit":check,
                     **{k:result[k] for k in ("success","reached_tick","terminal_tick","calls","usage","initialization_retries_used","watch_initialized","world_hash")}})
        save(batch/"summary.json",rows)
    identical_success_pairs=[];condition_effect_pairs=[]
    for a,b in zip(rows[::2],rows[1::2]):
        if a["case"]["parameters"]["first_event_tick"]==59 and a["case"]["parameters"]["gap_s"]==30:
            assert a["success"] and b["success"] and a["world_hash"]==b["world_hash"]
            assert a["calls"]==b["calls"]==2 and a["reached_tick"]==b["reached_tick"]==414
            identical_success_pairs.append([a["index"],b["index"]])
        if a["success"] and not b["success"]:condition_effect_pairs.append([a["index"],b["index"]])
    assert len(identical_success_pairs)==2 and len(condition_effect_pairs)==4
    save(batch/"validation.json",{"status":"passed","episodes":len(rows),"independent_audits":len(rows),"cross_process_replays":len(rows),
        "identical_success_after_rule_removal":identical_success_pairs,"rule_removal_changes_success":condition_effect_pairs,"real_model_requests":0})
    print(json.dumps(read(batch/"validation.json")),flush=True)


if __name__=="__main__":main()

"""Measured public planners on the same snapshots; no provider or judge state."""
import argparse
import importlib
import importlib.util
import json
import math
from pathlib import Path
import sys
import time

def save(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2),encoding="utf-8")
def main():
    p=argparse.ArgumentParser();p.add_argument("--stage",type=Path,required=True);p.add_argument("--guarded-only",action="store_true");p.add_argument("--output",type=Path);a=p.parse_args();stage=a.stage
    sys.path.insert(0,str(stage/"frozen-source/research/realtime"));r=importlib.import_module("resource_study");r.verify_stage(stage)
    binary=stage/"target/release/examples/realtime_resource.exe";folder=a.output or stage/("public-cost-guard30" if a.guarded_only else "public-cost-controls");folder.mkdir(exist_ok=False);rows=[]
    for case in r.read(stage/"quality/offline-summary.json"):
        for algorithm in (("exact_guard30",) if a.guarded_only else ("exact","catalog_greedy")):
            output=folder/f"{case['tag']}-{algorithm}";output.mkdir()
            # Only public input crosses this planner boundary. Do not read the
            # existing offline score, catalog, reference solution or model calls.
            started=time.perf_counter_ns()
            model_input=r.read(stage/"quality"/case["tag"]/"input.json");save(output/"input.json",model_input)
            catalog,_=r.invoke(binary,"--catalog",output/"input.json",output/"catalog.json")
            if algorithm=="exact_guard30":
                spec=importlib.util.spec_from_file_location("resource_frontier",Path(__file__).with_name("frontier-realtime-resource.py"))
                frontier=importlib.util.module_from_spec(spec);spec.loader.exec_module(frontier)
                plan=r.solve(model_input,frontier.shifted(model_input,catalog["catalog"],30))[0]
            else:plan=r.solve(model_input,catalog["catalog"])[0] if algorithm=="exact" else r.greedy(model_input,catalog["catalog"])
            save(output/"plan.json",plan)
            elapsed=(time.perf_counter_ns()-started)/1e9;delay=math.ceil(elapsed*5)
            if algorithm=="exact_guard30":assert delay<=30,"reserved execution window exceeded; do not silently score as guarded"
            actual,_=r.invoke(binary,"--evaluate",output/"input.json",output/"score.json",output/"plan.json",delay)
            r.audit_evaluation(model_input,plan,actual)
            rows.append({"tag":case["tag"],"algorithm":algorithm,"planner_wall_s":elapsed,"delay_sim_s":delay,
                "score":actual["earned"],"reserved_delay_s":30 if algorithm=="exact_guard30" else None,"model_calls":0,"model_tokens":None,"money":None})
    save(folder/"summary.json",rows);print(json.dumps(rows),flush=True)
if __name__=="__main__":main()

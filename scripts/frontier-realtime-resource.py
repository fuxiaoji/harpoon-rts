"""Exact finite latency frontier from stationary-start navigation catalogs.

Every selected bound is verified by fresh production navigation, including
completion-time/position translation; these are hindsight bounds, not policies.
"""
import argparse
import copy
import importlib
import json
from pathlib import Path
import sys

def save(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2),encoding="utf-8")
def shifted(case,catalog,delay):
    rows=copy.deepcopy(catalog)
    for row in rows:
        row["return_tick"]+=delay
        row["feasible"]=row["feasible"] and row["return_tick"]<=case["horizon"]
        row["score"]=sum(case["tasks"][v["task"]]["reward"] for v in row["visits"]
            if v["completed"]+delay<=case["tasks"][v["task"]]["deadline"]) if row["feasible"] else 0
    return rows
def main():
    p=argparse.ArgumentParser();p.add_argument("--stage",type=Path,required=True);a=p.parse_args();stage=a.stage
    sys.path.insert(0,str(stage/"frozen-source/research/realtime"));r=importlib.import_module("resource_study");r.verify_stage(stage)
    folder=stage/"delay-frontier";folder.mkdir(exist_ok=False);binary=stage/"target/release/examples/realtime_resource.exe";rows=[]
    for summary in r.read(stage/"quality/offline-summary.json"):
        tag=summary["tag"];source=stage/"quality"/tag;case=r.read(source/"input.json");catalog=r.read(source/"catalog.json")["catalog"]
        for delay in (0,1,30,60,180,360):
            plan,score=r.solve(case,shifted(case,catalog,delay));out=folder/f"{tag}-{delay}";out.mkdir();save(out/"plan.json",plan)
            actual,_=r.invoke(binary,"--evaluate",source/"input.json",out/"score.json",out/"plan.json",delay)
            r.audit_evaluation(case,plan,actual);assert actual["earned"]==score
            for route,row in zip(plan["routes"],actual["routes"]):
                original=next(z for z in catalog if z["unit"]==route["unit"] and z["task_ids"]==route["task_ids"])
                assert original["return_tick"]+delay==row["return_tick"]
                assert len(original["visits"])==len(row["visits"])
                for old,new in zip(original["visits"],row["visits"]):
                    assert old["arrived"]+delay==new["arrived"] and old["completed"]+delay==new["completed"] and old["pos"]==new["pos"]
            rows.append({"tag":tag,"delay":delay,"finite_optimum":score})
    save(folder/"summary.json",rows);print(json.dumps(rows),flush=True)
if __name__=="__main__":main()

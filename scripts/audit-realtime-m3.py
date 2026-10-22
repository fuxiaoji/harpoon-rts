"""Post-run mechanism/constraint audit and reproducible descriptive statistics."""
import argparse
from collections import Counter
import importlib
import json
from pathlib import Path
import statistics
import sys

def read(p):return json.loads(p.read_text(encoding="utf-8"))
def save(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2),encoding="utf-8")
def main():
    p=argparse.ArgumentParser();p.add_argument("--stage",type=Path,required=True);p.add_argument("--offline-only",action="store_true");p.add_argument("--summary-only",action="store_true");a=p.parse_args();stage=a.stage
    sys.path.insert(0,str(stage/"frozen-source/research/realtime"));r=importlib.import_module("resource_study");r.verify_stage(stage)
    if a.summary_only:
        describe(stage);return
    binary=stage/"target/release/examples/realtime_resource.exe"
    controls=stage/"constraint-controls";controls.mkdir(exist_ok=False)
    rows=[]
    for seed in (11,23,37):
        tag=f"complex-{seed}";original=read(stage/"quality"/tag/"input.json")
        for variant in ("original","more_shared_kits","no_predecessors","all_capabilities","more_capacity","long_return_horizon"):
            case=read(stage/"quality"/tag/"input.json")
            if variant=="more_shared_kits":case["shared_kits"]=100
            elif variant=="no_predecessors":
                for task in case["tasks"]:task["predecessor"]=None
            elif variant=="all_capabilities":
                for v in case["vessels"]:v["skills"]=["common","survey","recovery"]
            elif variant=="more_capacity":
                for v in case["vessels"]:v["capacity"]=100
            elif variant=="long_return_horizon":case["horizon"]=4000
            folder=controls/f"{tag}-{variant}";folder.mkdir();save(folder/"input.json",case)
            catalog,elapsed=r.invoke(binary,"--catalog",folder/"input.json",folder/"catalog.json")
            plan,score=r.solve(case,catalog["catalog"]);save(folder/"plan.json",plan)
            actual,_=r.invoke(binary,"--evaluate",folder/"input.json",folder/"score.json",folder/"plan.json");r.audit_evaluation(case,plan,actual);assert actual["earned"]==score
            # All route candidates, not just selected winning plans, get an
            # independent mission-score/constraint check from serialized visits.
            for unit in (1,2,3):
                for row in catalog["catalog"]:
                    if row["unit"]!=unit:continue
                    routes=[row if u==unit else next(z for z in catalog["catalog"] if z["unit"]==u and z["task_ids"]==[]) for u in (1,2,3)]
                    candidate={"routes":[{"unit":z["unit"],"task_ids":z["task_ids"]} for z in routes],"brief":"catalog audit"}
                    kits=sum(case["tasks"][t]["kits"] for route in candidate["routes"] for t in route["task_ids"])
                    result={"routes":routes,"earned":sum(z["score"] for z in routes) if kits<=case["shared_kits"] else 0,"kits":kits,"shared_budget_ok":kits<=case["shared_kits"],"delay_s":0}
                    r.audit_evaluation(case,candidate,result)
            rows.append({"tag":tag,"variant":variant,"score":score,"routes":len(catalog["catalog"]),"enumeration_wall_s":elapsed})
            print(json.dumps(rows[-1]),flush=True)
    save(controls/"summary.json",rows)
    timing=read(stage/"timing/summary.json");pairs=[]
    for row in timing:
        if row["case"]["method"]!="eh":continue
        context={k:v for k,v in row["case"].items() if k not in ("method","ready")}
        other={x["case"]["method"]:x for x in timing if all(x["case"][k]==v for k,v in context.items())}
        pairs.append({**context,"ready_success":row["success"],"cold_success":other["emcold"]["success"],
            "quiet_success":other["ehq30"]["success"],"ready_calls":row["calls"],"cold_calls":other["emcold"]["calls"],"quiet_calls":other["ehq30"]["calls"]})
    assert all(p["ready_success"]==p["quiet_success"] for p in pairs)
    save(stage/"timing-mechanisms.json",{"pairs":pairs,"phase_residues":{str(p):{str(period):p%period for period in (60,300,600)} for p in (17,83,251,419)},
        "note":"scripted condition execution only; one logical preflight slot in ready arms is not an actual LLM request or monetary charge"})
    if a.offline_only:return
    describe(stage)

def describe(stage):
    analysis=read(stage/"quality/analysis.json")
    summary={}
    for effort in ("low","medium","high"):
        rs=[x for x in analysis["rows"] if x["effort"]==effort]
        summary[effort]={"latency_median_s":statistics.median(x["latency_s"] for x in rs),
            "latency_min_s":min(x["latency_s"] for x in rs),"latency_max_s":max(x["latency_s"] for x in rs),
            "small_score":sum(x["scores"]["wide-0"] for x in rs if x["tag"].startswith("small")),
            "complex_score":sum(x["scores"]["wide-0"] for x in rs if x["tag"].startswith("complex")),
            "small_nominal_optimum":108,"complex_nominal_optimum":129,
            "actual_delay_wide_score":sum(x["scores"].get(f"wide-{__import__('math').ceil(x['latency_s']*5)}",0) for x in rs),
            "tight_zero_delay_score":sum(x["scores"].get("tight-0",0) for x in rs),
            "failed":sum(x["status"]!="ok" for x in rs)}
    failed_reasons=Counter()
    for row in analysis["rows"]:
        call=Path(row["call_dir"])
        if (call/"wide-0.json").exists():
            result=read(call/"wide-0.json")
            if not result["shared_budget_ok"]:failed_reasons["shared_kits"]+=1
            for route in result["routes"]:
                if not route["feasible"]:failed_reasons[route["reason"]]+=1
    save(stage/"descriptive-results.json",{"by_effort":summary,"semantic_failures":failed_reasons,
        "unknown_usage_receipts":sum(x["usage"] is None for x in analysis["rows"]),"score_pairs":analysis["pairs"],
        "scope":"six fixed development snapshots, two templates; not independent domain samples or confirmatory budget effect"})
    print(json.dumps(read(stage/"descriptive-results.json")),flush=True)
if __name__=="__main__":main()

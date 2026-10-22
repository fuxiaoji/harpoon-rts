"""Validate the completed stage, freeze sources/docs, inventory and hash evidence."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys

ROOT=Path(__file__).resolve().parents[1]
PROJECT=ROOT.parent
def read(p):return json.loads(p.read_text(encoding="utf-8"))
def save(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2),encoding="utf-8")
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser();p.add_argument("--stage",type=Path,required=True);p.add_argument("--validate-only",action="store_true");a=p.parse_args();stage=a.stage
    sys.path.insert(0,str(stage/"frozen-source/research/realtime"));import resource_study as r;r.verify_stage(stage)
    complete=read(stage/"quality/calls-complete.json");ledger=read(stage/"quality/request-ledger.json");analysis=read(stage/"quality/analysis.json")
    assert complete["requests"]==len(ledger)==len(analysis["rows"])==18
    intent=read(stage/"quality/intent.json");assert [{k:x[k] for k in ("tag","effort")} for x in ledger]==intent["order"]
    records=[read(Path(x["call_dir"])/"result.json") for x in ledger]
    assert len(list((stage/"quality/calls").glob("r*/result.json")))==18
    for entry,record in zip(ledger,records):
        inp=read(Path(entry["call_dir"])/"input.json")
        assert record["instructions_sha256"]==intent["instructions_sha256"]
        assert record["effort"]==entry["effort"] and record["requested_model"]==intent["model"] and not record["tool_violations"]
        assert inp==read(stage/"quality"/entry["tag"]/"input.json")
        assert not {"seed","effort","family","future","catalog","optimum"}.intersection(inp)
    for row in analysis["rows"]:
        if row["status"]=="ok":assert row["scores"]["wide-180"]==row["scores"]["tight-0"]
    timing=read(stage/"timing/validation.json");assert timing["status"]=="passed" and timing["episodes"]==112 and timing["real_model_calls"]==0
    controls=read(stage/"constraint-controls/summary.json");assert len(controls)==18
    frontier=read(stage/"delay-frontier/summary.json");assert len(frontier)==36
    public=read(stage/"public-cost-guard30-verified/summary.json");assert len(public)==6
    refs={x["tag"]:x for x in read(stage/"quality/offline-summary.json")}
    assert all(x["score"]==refs[x["tag"]]["optimum"] and x["delay_sim_s"]<=30 for x in public)
    catalogs=[stage/"quality"/tag for tag in refs]+[p for p in (stage/"constraint-controls").iterdir() if p.is_dir()]
    catalog_routes=0
    for directory in catalogs:
        case=read(directory/"input.json");catalog=read(directory/"catalog.json")["catalog"]
        actual={(row["unit"],tuple(row["task_ids"])) for row in catalog};expected=set()
        assert len(actual)==len(catalog)
        for vessel in case["vessels"]:
            def visit(ids,load,kits):
                expected.add((vessel["id"],tuple(ids)))
                if len(ids)==4:return
                for task in case["tasks"]:
                    if task["id"] in ids or task["skill"] not in vessel["skills"]:continue
                    if task["predecessor"] is not None and task["predecessor"] not in ids:continue
                    if load+task["load"]>vessel["capacity"] or kits+task["kits"]>case["shared_kits"]:continue
                    visit(ids+[task["id"]],load+task["load"],kits+task["kits"])
            visit([],0,0)
        assert actual==expected
        empties={u:next(z for z in catalog if z["unit"]==u and not z["task_ids"]) for u in (1,2,3)}
        for row in catalog:
            visited=[z["task"] for z in row["visits"]]
            assert visited==row["task_ids"][:len(visited)] and row["mask"]==sum(1<<i for i in visited)
            if row["feasible"]:assert visited==row["task_ids"]
            routes=[row if u==row["unit"] else empties[u] for u in (1,2,3)]
            plan={"routes":[{"unit":z["unit"],"task_ids":z["task_ids"]} for z in routes],"brief":"Independent finite-space audit"}
            kits=sum(case["tasks"][t]["kits"] for z in routes for t in z["task_ids"])
            result={"routes":routes,"earned":sum(z["score"] for z in routes) if kits<=case["shared_kits"] else 0,
                "kits":kits,"shared_budget_ok":kits<=case["shared_kits"],"delay_s":0}
            r.audit_evaluation(case,plan,result)
        catalog_routes+=len(catalog)
    tests={}
    for name in ("realtime_controlled","realtime_resource"):
        log=(stage/f"{name}-test.log").read_text(encoding="utf-8")
        tests[name]=int(re.search(r"test result: ok\. (\d+) passed",log)[1])
    assert tests=={"realtime_controlled":17,"realtime_resource":5}
    assert "Ran 4 tests" in (stage/"python-resource-tests.log").read_text(encoding="utf-8")
    # All archived M2a artifacts remain byte-identical, including the files that
    # record its excluded accidental calls. Nothing is silently pruned.
    old=Path("E:/CodexBuilds/harpoon-research-m2a-20261022");old_manifest=read(old/"artifact-manifest.json")
    for item in old_manifest["files"]:
        path=old/item["path"];assert path.stat().st_size==item["bytes"] and sha(path)==item["sha256"]
    core=list((stage/"frozen-source/crates/harpoon-core/src").rglob("*.rs"))
    for path in core:
        relative=path.relative_to(stage/"frozen-source")
        assert sha(path)==sha(old/"goal-control-v2/frozen-source"/relative)
    failures=[]
    for suffix,reason in (("","cargo executable not on PATH"),("-v2","Option<Vec<Vec2>> compilation mismatch"),("-v3","Rust mutable-borrow compilation error"),("-v4","cold Watch(None) scripting error after six timing controls")):
        path=Path("E:/CodexBuilds/harpoon-research-m3a-20261022"+suffix)
        assert path.is_dir() and not list(path.glob("quality/calls/r*/result.json"))
        failures.append({"path":str(path),"reason":reason,"excluded_from_primary_stage":True,"real_model_calls":0})
    usage={k:sum(rec["usage"][k] for rec in records if rec["usage"] is not None and rec["usage"].get(k) is not None)
        for k in ("input_tokens","output_tokens","reasoning_output_tokens","cached_input_tokens")}
    validation={"status":"passed","stage":"M2b timing controls + M3a fixed-snapshot development gate","model_requests":18,
        "successful_response_receipts":sum(x["status"]=="ok" for x in records),"failed_response_receipts":sum(x["status"]!="ok" for x in records),
        "unknown_usage_receipts":sum(x["usage"] is None for x in records),"known_usage_totals":usage,"money":None,
        "timing_episodes":112,"same_rule_paired_worlds":16,"quality_snapshots":6,"constraint_cases":18,"delay_frontier_branches":36,
        "public_costed_runs":24,"catalogs_with_independent_completeness_and_score_checks":len(catalogs),"catalog_routes_independently_audited":catalog_routes,
        "rust_tests":tests,"python_resource_tests":4,"legacy_m2a_files_unchanged":len(old_manifest["files"]),
        "frozen_core_files_unchanged":len(core),"earlier_build_or_control_attempts":failures,
        "scope":"Exploratory, same model, two templates; model repeats/cross-model/closed-loop M3/M4/holdout not completed"}
    save(stage/"stage-validation.json",validation)
    if a.validate_only:
        print(json.dumps(validation),flush=True);return
    output=stage/"final-source";output.mkdir(exist_ok=False)
    paths=["crates/harpoon-core/examples/realtime_resource.rs","research/realtime/resource_study.py","research/realtime/test_resource_study.py","research/realtime/m3-protocol.md",
        "scripts/prepare-realtime-m3.py","scripts/control-realtime-m2b.py","scripts/audit-realtime-m3.py","scripts/plot-realtime-m3.py","scripts/frontier-realtime-resource.py",
        "scripts/cost-realtime-resource-planners.py","scripts/finalize-realtime-m3.py","docs/research-realtime-m3a-20261022.md","research/realtime/README.md"]
    for rel in paths:
        target=output/rel;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/rel,target)
    for rel in ("agent.md","plan.md","havedone.md"):
        (output/"repository-governance").mkdir(exist_ok=True);shutil.copy2(PROJECT/rel,output/"repository-governance"/rel)
    files=[]
    for path in sorted(stage.rglob("*")):
        if not path.is_file():continue
        rel=path.relative_to(stage).as_posix()
        if "__pycache__" in path.parts or rel in ("artifact-manifest.json","archive-validation.json","finalize-console.log"):continue
        if rel.startswith("target/") and rel not in ("target/release/examples/realtime_resource.exe","target/release/examples/realtime_controlled.exe"):continue
        files.append({"path":rel,"bytes":path.stat().st_size,"sha256":sha(path)})
    save(stage/"artifact-manifest.json",{"scope":"stage evidence, raw receipts, catalogs, final/frozen sources and two release binaries; build/cache and self-validation excluded",
        "file_count":len(files),"total_bytes":sum(x["bytes"] for x in files),"files":files})
    for item in files:assert sha(stage/item["path"])==item["sha256"]
    inventory=PROJECT/"resources/catalog/inventory.csv"
    with inventory.open(encoding="utf-8-sig",newline="") as f:existing={(x["relative_path"],x["sha256"]) for x in csv.DictReader(f)}
    materials=[ROOT/rel for rel in paths]+[stage/name for name in ("stage-validation.json","artifact-manifest.json","quality/intent.json","quality/analysis.json","quality/summary.csv",
        "timing/validation.json","timing-mechanisms.json","constraint-controls/summary.json","delay-frontier/summary.json","descriptive-results.json",
        "public-cost-guard30-verified/summary.json","figures/stage-overview.png","figures/stage-overview.svg","figures/quality-latency.png","figures/quality-latency.svg","figures/common-delay.png","figures/common-delay.svg")]
    additions=[]
    for path in materials:
        try:name=path.relative_to(PROJECT).as_posix()
        except ValueError:name=path.as_posix()
        if (name,sha(path)) in existing:continue
        additions.append([name,"research-live-source-or-evidence;version=2026-10-22-m3a-v1;use=controlled-startup-and-finite-resource-quality-development",path.suffix,path.stat().st_size,sha(path),""])
    with inventory.open("a",encoding="utf-8",newline="") as f:csv.writer(f).writerows(additions)
    save(stage/"archive-validation.json",{"status":"passed","files_verified":len(files),"total_bytes":sum(x["bytes"] for x in files),"inventory_added":len(additions),"money":None})
    print(json.dumps({**validation,"archive":read(stage/"archive-validation.json")}),flush=True)
if __name__=="__main__":main()

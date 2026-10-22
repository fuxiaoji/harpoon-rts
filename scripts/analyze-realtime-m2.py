"""Audit/describe M2a saved episodes; no inferential population claims or calls."""
from __future__ import annotations
import argparse
from collections import Counter,defaultdict
import hashlib
import importlib
import json
import math
from pathlib import Path
import statistics
import sys


def read(path):return json.loads(path.read_text(encoding="utf-8"))
def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding="utf-8")
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_case(run,case):
    assert run["backend"]==case["backend"],"actual backend differs from frozen case label"
    assert run["clock_mode"]==case["clock"]
    assert run["case_parameters"]==case["parameters"] and run["seed"]==case["seed"] and run["family"]==case["family"]
    assert run["max_calls"]==case["budget"]
    assert run["initialization_retries_limit"]==case.get("initialization_retries",1)
    if case["clock"]=="injected":assert run["injected_delay_sim_s"]==case["delay"]


def summarize(rows):
    groups=defaultdict(list)
    for row in rows:groups[row["method"]].append(row)
    return [{"method":method,"episodes":len(rs),"successes":sum(r["success"] for r in rs),
             "requests":sum(r["requests"] for r in rs),"model_calls":sum(r["model_calls"] for r in rs),
             "stale_responses":sum(r["statuses"].get("stale_response",0)+r["statuses"].get("stale_before_application",0) for r in rs),
             "initialization_retries":sum(r["initialization_retries"] for r in rs),
             "usage":{key:sum(r["usage"][key] for r in rs) for key in ("input_tokens","output_tokens","reasoning_output_tokens","cached_input_tokens")}}
            for method,rs in groups.items()]


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--stage",required=True,type=Path)
    p.add_argument("--batch",choices=("checks","scripted","live","mechanism-fixed-v2","live-recovery-v2","geometry","recovery-cap-controls"),required=True)
    p.add_argument("--allow-prefix",action="store_true")
    args=p.parse_args();root=args.stage;batch=root/args.batch
    sys.path.insert(0,str(root/"frozen-source/research/realtime"))
    audit=importlib.import_module("m2_audit").audit
    protocol=read(batch/"protocol.json");summaries=read(batch/"summary.json")
    if args.allow_prefix:assert 0<len(summaries)<=len(protocol["order"])
    else:assert len(summaries)==len(protocol["order"])
    rows=[];calls=[];scenarios={};core=[]
    for manifest in read(root/"source-manifest.json"):
        source=root/"frozen-source"/manifest["path"]
        assert sha(source)==manifest["sha256"] and source.stat().st_size==manifest["bytes"]
        if manifest["path"].startswith("crates/harpoon-core/src/"):core.append(manifest)
    assert sha(root/"target/release/examples/realtime_live.exe")==read(root/"prepare-validation.json")["binary_sha256"]
    for summary,c in zip(summaries,protocol["order"]):
        assert summary["case"]==c
        path=Path(summary["trace"]);run=read(path);engine=read(path.parent/"engine.json")
        verify_case(run,c)
        audit(run,engine)
        assert read(path.parent/"replay.log")["replay_verified"]
        key=json.dumps([c["parameters"],c["seed"],c["family"]],sort_keys=True)
        signature=json.dumps([engine["initial_scenario"],engine["updates"],engine["notice_ticks"]],sort_keys=True)
        if key in scenarios:assert scenarios[key]==signature
        else:scenarios[key]=signature
        request_records=[]
        for r in run["requests"]:
            record=r["record"]
            call={"episode":summary["index"],"method":c["method"],"parameters":c["parameters"],"seed":c["seed"],"backend":c["backend"],
                  "request_id":r["id"],"start_tick":r["requested_tick"],"response_tick":r["response_tick"],"apply_tick":r["applied_tick"],
                  "trigger_tick":r["triggered_tick"],"status":r["status"],"usage":record.get("usage"),
                  "client_elapsed_s":record.get("elapsed_s"),"call_directory":r["call_directory"],
                  "worker_elapsed_s":(r["response_wall_us"]-r["worker_started_wall_us"])/1e6,
                  "answer":record.get("answer"),"initializing_watch":r["initializing_watch"]}
            if c["backend"]!="codex":assert record["requested_model"] is None and record["usage"] is None
            calls.append(call);request_records.append(call)
        usages=[r["usage"] for r in request_records if r["usage"] is not None]
        lags=sorted(row["lag_us"] for row in run["clock"] if row["lag_us"] is not None)
        watch_tick=min((r["applied_tick"] for r in run["requests"] if r["applied_tick"] is not None),default=None) if run["policy"] in ("em","hm600") else None
        rows.append({"index":summary["index"],"method":c["method"],"seed":c["seed"],"parameters":c["parameters"],
                     "check_label":c.get("label"),
                     "budget":c["budget"],"delay":c["delay"] if c["clock"]=="injected" else None,
                     "backend":c["backend"],"success":run["success"],"requests":run["calls"],
                     "model_calls":run["calls"] if c["backend"]=="codex" else 0,
                     "statuses":dict(Counter(r["status"] for r in run["requests"])),
                     "initialization_retries":run["initialization_retries_used"],"reached_tick":run["reached_tick"],"terminal_tick":run["terminal_tick"],
                     "model_condition_events":sum("model_condition" in t["reason"] for t in run["triggers"]),
                     "model_condition_requests":sum("model_condition" in r["trigger"] for r in run["requests"]),
                     "first_watch_installed_tick":watch_tick,
                     "task_updates_after_watch_installed":sum(u["tick"]>watch_tick for u in engine["updates"]) if watch_tick is not None else None,
                     "deadline_tick":engine["final_task"]["deadline_tick"],
                     "plan_gate_skips":sum(s["reason"]=="standing_plan_still_valid" for s in run["skipped_triggers"]),
                     "usage":{key:sum(u.get(key,0) for u in usages) for key in ("input_tokens","output_tokens","reasoning_output_tokens","cached_input_tokens")},
                     "model_usage_missing":sum(r["usage"] is None for r in request_records) if c["backend"]=="codex" else 0,
                     "clock_lag_p95_ms":lags[math.ceil(.95*len(lags))-1]/1000 if lags else None,
                     "clock_lag_max_ms":max(lags)/1000 if lags else None,"trace":str(path)})
    public=[row for row in rows if row["backend"]=="public"]
    assert all(row["success"] for row in public)
    # Explicit paired temporal cells, not independent-call pseudo-replication.
    cells=defaultdict(dict)
    for row in rows:
        k=json.dumps([row["parameters"],row["seed"],row["budget"],row["delay"],row["check_label"]],sort_keys=True)
        assert row["method"] not in cells[k]
        cells[k][row["method"]]={"success":row["success"],"requests":row["requests"],"index":row["index"]}
    model_calls=[c for c in calls if c["backend"]=="codex"]
    if args.batch in ("live","live-recovery-v2"):
        workspaces=list((batch/"calls/controller-workspaces").iterdir())
        assert len(workspaces)>=len(model_calls) if args.allow_prefix else len(workspaces)==len(model_calls)
        import re
        assert all(re.fullmatch("[0-9a-f]{32}",d.name) and not list(d.iterdir()) for d in workspaces)
    complete=len(summaries)==len(protocol["order"])
    output={"scope":protocol["analysis"],"status":"passed" if complete else "passed_completed_prefix",
            "episodes":rows,"method_summary":summarize(rows),"paired_cells":dict(cells),"calls":calls,
            "real_model_calls":len(model_calls),"public_controls":len(public),"physical_controls_all_passed":True,
            "usage_total":{key:sum((c["usage"] or {}).get(key,0) for c in model_calls) for key in ("input_tokens","output_tokens","reasoning_output_tokens","cached_input_tokens")},
            "model_usage_missing":sum(c["usage"] is None for c in model_calls),"money_cost":None,
            "successful_em_without_any_model_condition_event":[r["index"] for r in rows if r["method"]=="em" and r["success"] and r["model_condition_events"]==0],
            "validation":{"episodes":len(rows),"planned":len(protocol["order"]),"scenario_signatures":len(scenarios),"core_files":len(core),
                          "independent_audits":len(rows),"verified_replay_logs":len(rows),"source_hashes_verified":True,"binary_hash_verified":True}}
    save(batch/"analysis.json",output)
    print(json.dumps({"status":output["status"],"method_summary":output["method_summary"],"usage":output["usage_total"],"validation":output["validation"]}),flush=True)


if __name__=="__main__":main()

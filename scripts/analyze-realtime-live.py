"""Reconstruct public trigger edges, audit live receipts/clocks, and describe pilot.

Does not issue model requests or produce confirmatory significance claims.
"""
from __future__ import annotations
import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import statistics


def read(path):return json.loads(path.read_text(encoding="utf-8"))
def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding="utf-8")
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def compare(a,op,b):
    if op=="eq":return a==b
    if op=="ne":return a!=b
    if op=="lt":return a<b
    if op=="le":return a<=b
    if op=="gt":return a>b
    if op=="ge":return a>=b
    raise AssertionError("unknown predicate")


def reconstruct_triggers(run,engine):
    standing,last_apply=0,0
    old_human=False
    human_key=None
    conditions=[];previous=[];keys=[]
    keyed=run.get("revision_keyed",False)
    updates=engine.get("updates") or ([engine["update"]] if engine["update"] else [])
    predicted=[]
    applications={r["applied_tick"]:r for r in run["requests"] if r["applied_tick"] is not None}
    for point in engine["positions"]:
        now=point["tick"]
        r=applications.get(now)
        if r:
            answer=r["record"]["answer"]
            if answer["action"]=="goto":standing=r["input"]["public_task"]["revision"]
            last_apply=now
            if run["policy"] in ("em","hm600"):
                conditions=answer["wake_conditions"];previous=[False]*len(conditions);keys=[None]*len(conditions)
        published=[u for u in updates if u["tick"]<=now]
        revision=len(published)
        goal=published[-1]["goal"] if published else engine["initial_scenario"]["units"][0]["waypoints"][0]
        deadline=published[-1]["deadline_tick"] if published else engine["initial_scenario"]["duration_s"]
        assert point["task_revision"]==revision
        f={"tick":now,"task_revision":revision,"standing_revision":standing,"task_mismatch":int(revision!=standing),
           "distance_nm":math.hypot(point["pos"]["x"]-goal["x"],point["pos"]["y"]-goal["y"]),
           "deadline_remaining_s":deadline-now,"plan_age_s":now-last_apply}
        if run.get("controller_version",1)>=2:
            notices=sum(t<=now for t in engine.get("notice_ticks",[]))
            assert point.get("public_notice_seq",0)==notices
            f["notice_seq"]=notices
        hit=False
        if run["policy"] in ("eh","hh600"):
            active=bool(f["task_mismatch"])
            hit=active and (not old_human or keyed and human_key!=revision)
            if hit:human_key=revision
            old_human=active
        if run["policy"] in ("em","hm600"):
            if now==0:hit=True
            else:
                current=[all(compare(f[a["field"]],a["op"],a["value"]) for a in c["all"]) for c in conditions]
                for i,(a,b) in enumerate(zip(current,previous)):
                    if a and (not b or keyed and keys[i]!=revision):
                        hit=True;keys[i]=revision
                previous=current
        period={"f60":60,"f300":300,"f600":600,"hh600":600,"hm600":600}.get(run["policy"])
        if period and now>0 and now%period==0:hit=True
        if hit:predicted.append(now)
        for request in run["requests"]:
            if request["requested_tick"]==now:
                assert request["input"]["wake_features"]==f
                assert request["input"]["public_task"]["revision"]==revision
                assert request["input"]["public_task"]["goal"]==goal
                assert request["input"]["public_task"]["deadline_tick"]==deadline
                assert not {"updates","update","notice_ticks"}.intersection(request["input"])
                if run.get("skip_expired",False):assert now<=deadline
        for skipped in run.get("skipped_triggers",[]):
            if skipped["tick"]==now:
                assert run["skip_expired"] and now>deadline and skipped["trigger"][0]<=now
    assert predicted==[t["tick"] for t in run["triggers"]]
    return len(predicted)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--batch",required=True,type=Path)
    p.add_argument("--allow-incomplete",action="store_true",help="audit only a completed prefix and mark it partial")
    args=p.parse_args()
    root=args.batch
    protocol=read(root/"protocol.json")
    episodes=read(root/"summary.json")
    if args.allow_incomplete:
        assert 0<len(episodes)<=len(protocol["order"])
    else:
        assert len(episodes)==len(protocol["order"])
    import importlib.util
    spec=importlib.util.spec_from_file_location("live_runner",root/"frozen-source/scripts/run-realtime-live.py")
    runner=importlib.util.module_from_spec(spec);spec.loader.exec_module(runner)
    rows=[];calls=[];trigger_count=0
    for row,(family,policy,seed) in zip(episodes,protocol["order"]):
        assert (row["family"],row["policy"],row["seed"])==(family,policy,seed)
        path=Path(row["trace"])
        run,engine=read(path),read(path.parent/"engine.json")
        runner.audit(run,engine)
        trigger_count+=reconstruct_triggers(run,engine)
        changed=engine["update"]
        new=[r for r in run["requests"] if r["input"]["public_task"]["revision"]>0]
        applied=[r for r in new if r["applied_tick"] is not None and r["record"]["answer"]["action"]=="goto"]
        latencies=[];in_motion=0
        for request in run["requests"]:
            directory=Path(request["call_directory"])
            record=read(directory/"result.json")
            if protocol["backend"]=="codex":
                assert record==request["record"]
                assert read(directory/"input.json")==request["input"]
                assert record["requested_model"]=="gpt-6.1-sol" and record["effort"]==protocol["effort"]
                assert not record["tool_violations"] and record["money_cost"] is None
                assert record["input_sha256"]==hashlib.sha256(json.dumps(request["input"],sort_keys=True).encode()).hexdigest()
            latencies.append((request["response_wall_us"]-request["worker_started_wall_us"])/1e6)
            a=engine["positions"][request["requested_tick"]]["pos"]
            b=engine["positions"][request["response_tick"]]["pos"]
            in_motion+=a!=b
            calls.append({"family":family,"policy":policy,"seed":seed,"status":request["status"],
                "start_tick":request["requested_tick"],"reply_tick":request["response_tick"],"apply_tick":request["applied_tick"],
                "latency_s":latencies[-1],"usage":record.get("usage"),"answer":record.get("answer")})
        lags=sorted(p["lag_us"] for p in run["clock"])
        task=engine["final_task"]
        arrivals=[p["tick"] for p in engine["positions"] if p["tick"]>=task["visible_since_tick"] and
            math.hypot(p["pos"]["x"]-task["goal"]["x"],p["pos"]["y"]-task["goal"]["y"])<=task["radius_nm"]]
        rows.append({"family":family,"policy":policy,"seed":seed,"success":run["success"],"calls":run["calls"],
            "usage":run["usage"],"reached_tick":run["reached_tick"],"task_deadline":engine["final_task"]["deadline_tick"],
            "first_new_snapshot":min((r["requested_tick"] for r in new),default=None),
            "first_new_goto_apply":min((r["applied_tick"] for r in applied),default=None),
            "first_arrival_even_if_late":min(arrivals,default=None),
            "statuses":dict(Counter(r["status"] for r in run["requests"])),
            "calls_started_after_public_deadline":sum(r["requested_tick"]>r["input"]["public_task"]["deadline_tick"] for r in run["requests"]),
            "skipped_expired_triggers":len(run.get("skipped_triggers",[])),
            "final_revision":task["revision"],
            "routine_notifications":len(engine.get("notice_ticks",[])),
            "first_final_revision_snapshot":min((r["requested_tick"] for r in new if r["input"]["public_task"]["revision"]==task["revision"]),default=None),
            "calls_while_following_current_revision":sum(r["input"]["standing_revision"]==r["input"]["public_task"]["revision"] for r in run["requests"]),
            "latency_median_s":statistics.median(latencies) if latencies else None,
            "clock_lag_p95_ms":lags[math.ceil(.95*len(lags))-1]/1000,"clock_lag_max_ms":max(lags)/1000,
            "monitor_wall_ms":run.get("monitor_wall_ns",run.get("monitor_cpu_ns",0))/1e6,
            "responses_with_navigation_motion":in_motion,"budget_exhausted":run["budget_exhausted"]})
    manifest=read(root/"source-manifest.json")
    for item in manifest:
        source=root/"frozen-source"/item["path"]
        assert sha(source)==item["sha256"] and source.stat().st_size==item["bytes"]
    if protocol["backend"]=="codex":
        workspaces=list((root/"calls/controller-workspaces").iterdir())
        assert len(workspaces)>=len(calls) if args.allow_incomplete else len(workspaces)==len(calls)
        import re
        assert all(re.fullmatch("[0-9a-f]{32}",w.name) and not list(w.iterdir()) for w in workspaces)
    output={"scope":protocol["analysis"],"episodes":rows,"calls":calls,"real_model_calls":len(calls) if protocol["backend"]=="codex" else 0,
            "usage_total":{k:sum((c["usage"] or {}).get(k,0) for c in calls) for k in ("input_tokens","output_tokens","reasoning_output_tokens","cached_input_tokens")},
            "validation":{"status":"passed_completed_prefix" if len(episodes)<len(protocol["order"]) else "passed",
                          "planned_episodes":len(protocol["order"]),"trigger_events_reconstructed":trigger_count,"episodes":len(rows),
                          "frozen_source_hashes":len(manifest),"call_receipts":len(calls),"money_cost":None},
            "timer_note":"legacy monitor_cpu_ns is perf_counter wall elapsed, not CPU process time"}
    save(root/"analysis.json",output)
    print(json.dumps({"episodes":rows,"validation":output["validation"]}),flush=True)


if __name__=="__main__":main()

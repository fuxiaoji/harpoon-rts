"""Independent score/public-feature/watch reconstruction for M2a traces."""
from collections import Counter
import hashlib
import json
import math


def read(path):return json.loads(path.read_text(encoding="utf-8"))


def compare(a,op,b):
    return {"eq":a==b,"ne":a!=b,"lt":a<b,"le":a<=b,"gt":a>b,"ge":a>=b}[op]


def audit(run,engine):
    task=engine["final_task"]
    updates=engine["updates"]
    points=engine["positions"]
    assert len(points)==len(run["clock"])==run["terminal_tick"]+1
    assert run["calls"]==len(run["requests"])<=run["max_calls"]
    assert run["world_hash"]==engine["world_hash"] and engine["replay_verified"]
    reached=[p["tick"] for p in points if task["visible_since_tick"]<=p["tick"]<=task["deadline_tick"] and
             math.hypot(p["pos"]["x"]-task["goal"]["x"],p["pos"]["y"]-task["goal"]["y"])<=task["radius_nm"]]
    assert run["success"]==bool(reached) and run["reached_tick"]==(min(reached) if reached else None)
    assert all(u["tick"]<=run["terminal_tick"] for u in updates)
    assert run["terminal_tick"]==(run["reached_tick"] if run["success"] else task["deadline_tick"])
    if run["clock_mode"]=="wall":assert run["episode_wall_us"]>=run["terminal_tick"]*run["tick_ms"]*1000
    cp=run["case_parameters"] or {}
    if run["family"]=="d5":
        assert len(updates)==2
        assert [u["tick"] for u in updates]==[cp["first_event_tick"],cp["first_event_tick"]+cp["gap_s"]]
        assert all(u["deadline_tick"]-u["tick"]==cp["event_window_s"] for u in updates)
        assert abs(updates[1]["goal"]["x"]-updates[0]["goal"]["x"]-cp.get("second_goal_dx_nm",.005*cp["gap_s"]))<1e-9
        assert updates[1]["goal"]["y"]==-updates[0]["goal"]["y"]
    apps={r["applied_tick"]:r for r in run["requests"] if r["applied_tick"] is not None}
    standing,last_apply=0,0
    previous=[];keys=[];conditions=[];human_previous=False;human_key=None;initialized=False
    predicted=[]
    for point in points:
        now=point["tick"]
        request=apps.get(now)
        if request:
            answer=request["record"]["answer"]
            if answer["action"]=="goto":standing=answer["task_revision"]
            last_apply=now
            if run["policy"] in ("em","hm600"):
                initialized=True;conditions=answer["wake_conditions"];previous=[False]*len(conditions);keys=[None]*len(conditions)
        published=[u for u in updates if u["tick"]<=now]
        revision=len(published)
        goal=published[-1]["goal"] if published else engine["initial_scenario"]["units"][0]["waypoints"][0]
        deadline=published[-1]["deadline_tick"] if published else engine["initial_scenario"]["duration_s"]
        notices=sum(t<=now for t in engine["notice_ticks"])
        assert point["task_revision"]==revision and point["public_notice_seq"]==notices
        f={"tick":now,"task_revision":revision,"standing_revision":standing,"task_mismatch":int(standing!=revision),
           "distance_nm":math.hypot(point["pos"]["x"]-goal["x"],point["pos"]["y"]-goal["y"]),
           "deadline_remaining_s":deadline-now,"plan_age_s":now-last_apply,"notice_seq":notices}
        if now<run["terminal_tick"]:
            reason=None
            if run["policy"] in ("eh","hh600"):
                active=bool(f["task_mismatch"])
                if active and (not human_previous or human_key!=revision):reason="human_condition";human_key=revision
                human_previous=active
            if run["policy"] in ("em","hm600"):
                if now==0:reason="model_watch_bootstrap"
                else:
                    current=[all(compare(f[a["field"]],a["op"],a["value"]) for a in c["all"]) for c in conditions]
                    hit=False
                    for i,(a,b) in enumerate(zip(current,previous)):
                        if a and (not b or keys[i]!=revision):hit=True;keys[i]=revision
                    previous=current
                    if hit:reason="model_condition"
            period={"f60":60,"f300":300,"f600":600,"hh600":600,"hm600":600}.get(run["policy"])
            if period and now>0 and now%period==0:reason=(reason+"+fixed_grid") if reason else "fixed_grid"
            if reason:predicted.append((now,reason))
        for r in run["requests"]:
            if r["requested_tick"]!=now:continue
            assert r["input"]["wake_features"]==f
            assert r["input"]["public_task"]["revision"]==revision
            assert r["input"]["public_task"]["goal"]==goal
            assert r["input"]["public_task"]["deadline_tick"]==deadline
            assert not {"updates","case_parameters","seed","family","gap_s","first_event_tick"}.intersection(r["input"])
            assert all(u["side"]=="Blue" for u in r["input"]["observation"]["units"]) and not r["input"]["observation"]["contacts"]
            if run["skip_expired"]:assert now<=deadline
            if run["gate_current"] and not (run["policy"] in ("em","hm600") and not initialized):assert standing!=revision
        for skipped in run["skipped_triggers"]:
            if skipped["tick"]!=now:continue
            if skipped["reason"]=="public_task_expired":assert now>deadline
            else:
                assert skipped["reason"]=="standing_plan_still_valid" and standing==revision
                assert run["gate_current"] and (run["policy"] not in ("em","hm600") or initialized)
    recoveries=run["initialization_recovery_events"]
    assert len(recoveries)==run["initialization_retries_used"]<=run["initialization_retries_limit"]
    for event in recoveries:
        r=run["requests"][event["source_request"]]
        assert r["initializing_watch"] and event["status"]==r["status"]
        assert event["status"] in ("model_failed","stale_response","expired_response","stale_before_application","expired_before_application")
        assert event["tick"]==r["response_tick"] or event["tick"]==r["response_tick"]+1
        predicted.append((event["tick"],"initialization_recovery"))
    assert Counter(predicted)==Counter((t["tick"],t["reason"]) for t in run["triggers"])
    for r in run["requests"]:
        assert r["triggered_tick"]<=r["requested_tick"]==r["input"]["observation"]["tick"]<=r["response_tick"]
        assert r["triggered_wall_us"]<=r["requested_wall_us"]<=r["worker_started_wall_us"]<=r["response_wall_us"]<=r["validated_wall_us"]
        if r["applied_tick"] is not None:
            assert r["applied_tick"]==r["response_tick"]+1 and r["applied_wall_us"]>=r["validated_wall_us"]
        if run["clock_mode"]=="injected" and run["backend"]=="scripted" and r["status"]!="in_flight_at_terminal":
            assert r["response_tick"]-r["requested_tick"]==max(1,run["injected_delay_sim_s"])
        if run["backend"]=="codex":
            from pathlib import Path
            directory=Path(r["call_directory"])
            record=read(directory/"result.json")
            assert record==r["record"] and read(directory/"input.json")==r["input"]
            assert record["requested_model"]=="gpt-6.1-sol" and record["effort"]==run["effort"] and not record["tool_violations"]
            assert record["input_sha256"]==hashlib.sha256(json.dumps(r["input"],sort_keys=True).encode()).hexdigest()
    return {"independent_score":True,"public_features_and_triggers":True,"requests":run["calls"],"terminal_tick":run["terminal_tick"]}

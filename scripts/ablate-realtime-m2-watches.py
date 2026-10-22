"""Prefix-preserving intervention on pure EM watch execution; no provider calls.

Keep original initialization actions/times until the first watch would be installed,
then install no conditions. No later request is introduced or reused off-policy.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess


def read(path):return json.loads(path.read_text(encoding="utf-8"))
def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding="utf-8")


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument("--stage",required=True,type=Path)
    p.add_argument("--verify-prefixes-only",action="store_true")
    args=p.parse_args();root=args.stage
    clean=read(root/"clean-analysis.json");assert clean["status"]=="passed"
    binary=root/"target/release/examples/realtime_live.exe"
    assert hashlib.sha256(binary.read_bytes()).hexdigest()==read(root/"prepare-validation.json")["binary_sha256"]
    if args.verify_prefixes_only:
        verified=[]
        for row in clean["episodes"]:
            if row["method"]!="em":continue
            original=read(Path(row["trace"]))
            retained={r["requested_tick"]:r for r in original["requests"] if r["initializing_watch"]}
            first=min((r["applied_tick"] for r in original["requests"] if r["applied_tick"] is not None),default=None)
            assert first is None or max(retained)<first
            command=[str(binary),"--case","d5","--seed",str(row["seed"])]
            for key,value in row["parameters"].items():command.extend(["--"+key.replace("_","-"),str(value)])
            process=subprocess.Popen(command,cwd=root/"frozen-source",stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding="utf-8")
            public=json.loads(process.stdout.readline())["input"]
            for now in range(max(retained)+1):
                if now:
                    process.stdin.write(json.dumps({"kind":"step","tick":now,"plan":None})+"\n");process.stdin.flush()
                    public=json.loads(process.stdout.readline())["input"]
                if now not in retained:continue
                r=retained[now];obs=public["observation"];task=public["public_task"];pos=obs["units"][0]["pos"]
                snapshot={**public,"wake_features":{"tick":now,"task_revision":task["revision"],"standing_revision":public["standing_revision"],
                    "task_mismatch":int(task["revision"]!=public["standing_revision"]),"distance_nm":math.hypot(pos["x"]-task["goal"]["x"],pos["y"]-task["goal"]["y"]),
                    "deadline_remaining_s":task["deadline_tick"]-now,"plan_age_s":now,"notice_seq":public["public_notice_seq"]},
                    "watch_semantics":"false-to-true or true on a new task revision",
                    "controller_budget":{"calls_remaining_after_this":original["max_calls"]-r["id"]-1,"simulation_seconds_per_wall_second":1000/original["tick_ms"],"clock_mode":"wall"}}
                assert snapshot==r["input"]
                digest=hashlib.sha256(json.dumps(snapshot,sort_keys=True).encode()).hexdigest()
                assert digest==r["record"]["input_sha256"]
                verified.append({"original_index":row["index"],"request_id":r["id"],"input_sha256":digest})
            process.stdin.close();process.wait(timeout=10);assert process.returncode==0
        save(root/"watch-ablation/prefix-input-verification.json",{"status":"passed","replayed_prefixes":8,"verified_model_inputs":len(verified),"inputs":verified,"new_model_calls":0})
        print(json.dumps({"status":"passed","verified_model_inputs":len(verified)}),flush=True)
        return
    batch=root/"watch-ablation";batch.mkdir(exist_ok=False)
    rows=[r for r in clean["episodes"] if r["method"]=="em"];assert len(rows)==8
    save(batch/"protocol.json",{"stage":"posthoc watch-execution causal diagnostic","source":"../clean-analysis.json","episodes":[r["index"] for r in rows],
        "intervention":"after first accepted initialization, install empty conditions; no new model calls; retain observed initialization responses and availability/application times",
        "identification":"all retained calls precede any possible watch effect and retain identical public inputs; later pure-event calls are removed, never reused on different observations",
        "world":"fresh production-bridge simulation of all ticks and private updates until natural termination",
        "cost":"saved prefix usage is a counterfactual ledger, not new API consumption; prompt/output were not shortened"})
    results=[]
    for row in rows:
        original=read(Path(row["trace"]));old=read(Path(row["trace"]).parent/"engine.json")
        first=min((r["applied_tick"] for r in original["requests"] if r["applied_tick"] is not None),default=None)
        retained=[r for r in original["requests"] if r["initializing_watch"]]
        assert not any(r["applied_tick"] is not None and r["applied_tick"]>first for r in retained) if first is not None else True
        assert all(r["requested_tick"]<(first or original["terminal_tick"]+1) for r in retained)
        applications={r["applied_tick"]:r for r in retained if r["applied_tick"] is not None}
        directory=batch/f"e{row['index']:04d}";directory.mkdir()
        args_list=[str(binary),"--case","d5","--seed",str(row["seed"])]
        for key,value in row["parameters"].items():args_list.extend(["--"+key.replace("_","-"),str(value)])
        with (directory/"bridge-stderr.log").open("w",encoding="utf-8") as err:
            process=subprocess.Popen(args_list,cwd=root/"frozen-source",stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,text=True,encoding="utf-8")
            def receive():
                line=process.stdout.readline();assert line,"bridge ended";return json.loads(line)
            def exchange(value):
                process.stdin.write(json.dumps(value)+"\n");process.stdin.flush();return receive()
            receive()
            for now in range(1,1081):
                request=applications.get(now)
                plan={"request_id":request["id"],**{k:v for k,v in request["record"]["answer"].items() if k!="wake_conditions"}} if request else None
                packet=exchange({"kind":"step","tick":now,"plan":plan})
                if request:assert packet["application"]["status"]=="applied"
                if packet["terminal"]:break
            trace=exchange({"kind":"finish","allow_terminal":True});process.wait(timeout=10);assert process.returncode==0
        save(directory/"engine.json",trace)
        prefix=first if first is not None else min(len(trace["positions"]),len(old["positions"]))-1
        assert trace["positions"][:prefix+1]==old["positions"][:prefix+1]
        assert trace["updates"]==old["updates"] and trace["initial_scenario"]==old["initial_scenario"]
        task=trace["final_task"]
        hits=[p["tick"] for p in trace["positions"] if task["visible_since_tick"]<=p["tick"]<=task["deadline_tick"] and
              math.hypot(p["pos"]["x"]-task["goal"]["x"],p["pos"]["y"]-task["goal"]["y"])<=task["radius_nm"]]
        assert trace["success"]==bool(hits) and trace["reached_tick"]==(min(hits) if hits else None)
        replay=subprocess.run([str(binary),"--replay",str(directory/"engine.json")],cwd=root/"frozen-source",capture_output=True,text=True)
        assert replay.returncode==0;(directory/"replay.log").write_text(replay.stdout+replay.stderr,encoding="utf-8")
        if row["model_condition_events"]==0:assert trace["world_hash"]==old["world_hash"]
        results.append({"original_index":row["index"],"source_trace":row["trace"],"parameters":row["parameters"],
            "actual_success":row["success"],"without_watch_execution_success":trace["success"],
            "actual_calls":original["calls"],"retained_prefix_calls":len(retained),"first_install_tick":first,
            "model_condition_events":row["model_condition_events"],"world_unchanged":trace["world_hash"]==old["world_hash"],
            "actual_world_hash":old["world_hash"],"counterfactual_world_hash":trace["world_hash"],"replay_verified":True})
        save(batch/"summary.json",results)
    assert sum(r["actual_success"] for r in results)==3
    assert sum(r["without_watch_execution_success"] for r in results)==1
    assert sum(r["world_unchanged"] for r in results)==4
    save(batch/"validation.json",{"status":"passed","counterfactual_replays":8,"new_model_calls":0,
        "actual_successes":3,"without_condition_execution_successes":1,"identical_worlds":4,
        "scope":"finite saved EM trajectories with unchanged initialization inputs; does not compare self-generation to human rules or reuse later outputs under new observations"})
    print(json.dumps(read(batch/"validation.json")),flush=True)


if __name__=="__main__":main()

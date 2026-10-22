"""Freeze/run real continuously paced LLM wake episodes. New output directory only.

The default pilot is seven D2 scheduling arms plus D0/D1 event controls, one seed.
This is an interface/exploratory study, not confirmatory method ranking.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import random
import re
import shutil
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]


def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding="utf-8")
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def audit(run,engine):
    assert engine["replay_verified"] and run["world_hash"]==engine["world_hash"]
    task=engine["final_task"]
    eligible=[p for p in engine["positions"] if task["visible_since_tick"]<=p["tick"]<=task["deadline_tick"]]
    import math
    reached=[p["tick"] for p in eligible if math.hypot(p["pos"]["x"]-task["goal"]["x"],p["pos"]["y"]-task["goal"]["y"])<=task["radius_nm"]]
    assert run["success"]==bool(reached) and run["reached_tick"]==(min(reached) if reached else None)
    assert len(run["clock"])==len(engine["positions"])==1201
    assert run["episode_wall_us"]>=1200*run["tick_ms"]*1000
    for r in run["requests"]:
        assert r["triggered_tick"]<=r["observed_tick"]==r["requested_tick"]==r["input"]["observation"]["tick"]
        assert r["triggered_wall_us"]<=r["requested_wall_us"]<=r["worker_started_wall_us"]<=r["response_wall_us"]
        assert all(u["side"]=="Blue" for u in r["input"]["observation"]["units"])
        assert not r["input"]["observation"]["contacts"]
        if r["applied_tick"] is not None:
            assert r["applied_tick"]>r["response_tick"]>=r["requested_tick"]
            assert r["applied_wall_us"]>=r["response_wall_us"]
        if r["record"].get("tool_violations"):assert r["status"]=="model_failed"
    # Any normal response taking multiple ticks must have continued navigation.
    moving=[]
    for r in run["requests"]:
        if r["response_tick"]>r["requested_tick"]+1:
            a=engine["positions"][r["requested_tick"]]["pos"]
            b=engine["positions"][min(r["response_tick"],1200)]["pos"]
            moving.append(a!=b)
    return {"requests":len(run["requests"]),"responses_with_motion":sum(moving),
            "replay_verified":True,"independent_score_verified":True}


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out",required=True,type=Path)
    p.add_argument("--base-source",type=Path,default=Path("E:/CodexBuilds/harpoon-research-m1b-20261021/assignment-v2/frozen-source"))
    p.add_argument("--backend",choices=("codex","scripted"),default="codex")
    p.add_argument("--tick-ms",type=int,default=100)
    p.add_argument("--seeds",default="7")
    p.add_argument("--effort",choices=("low","medium","high"),default="low")
    p.add_argument("--max-calls",type=int,default=6)
    p.add_argument("--episode",action="append",help="optional FAMILY:POLICY subset for a separate sensitivity batch")
    p.add_argument("--revision-keyed",action="store_true")
    p.add_argument("--skip-expired",action="store_true")
    p.add_argument("--script-delay-ms",type=int,default=6)
    args=p.parse_args()
    if args.out.exists():p.error("use a new output directory")
    if args.tick_ms<1 or args.max_calls<1 or args.script_delay_ms<0:p.error("positive clock/call budget and nonnegative script delay required")
    args.out.mkdir(parents=True)
    frozen=args.out/"frozen-source"
    shutil.copytree(args.base_source,frozen,ignore=shutil.ignore_patterns("__pycache__"))
    overlays=["crates/harpoon-core/examples/realtime_live.rs", "research/realtime/llm_client.py",
              "research/realtime/live_controller.py","research/realtime/test_live_controller.py",
              "scripts/analyze-realtime-live.py",f"scripts/{Path(__file__).name}"]
    for relative in overlays:
        dest=frozen/relative;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/relative,dest)
    manifest=[{"path":f.relative_to(frozen).as_posix(),"bytes":f.stat().st_size,"sha256":sha(f)}
              for f in sorted(frozen.rglob("*")) if f.is_file()]
    save(args.out/"source-manifest.json",manifest)
    seeds=[int(s) for s in args.seeds.split(",")]
    matrix=[]
    for seed in seeds:
        if args.episode:
            block=[]
            for item in args.episode:
                family,policy=item.split(":")
                if family not in ("d0","d1","d2","d5") or policy not in ("f60","f300","f600","eh","em","hh600","hm600"):
                    p.error("invalid FAMILY:POLICY")
                block.append((family,policy,seed))
        else:
            block=[("d2",policy,seed) for policy in ("f60","f300","f600","eh","em","hh600","hm600")]
        random.Random(20261021+seed).shuffle(block)
        matrix.extend(block if args.episode else block+[("d0","eh",seed),("d1","eh",seed)])
    protocol={"stage":"M1d event-identity and expiry controls" if args.revision_keyed or args.skip_expired or any(e[0]=="d5" for e in matrix) else "M1c interface-v2 pilot","backend":args.backend,
              "controller_version":2,"revision_keyed":args.revision_keyed,"skip_expired":args.skip_expired,
              "script_delay_ms":args.script_delay_ms,"dense_task_gap_s":30,"dense_second_lifetime_s":900,
              "order":matrix,"seeds":seeds,"tick_ms":args.tick_ms,"tight_window_s":300,
              "horizon_s":1200,"effort":args.effort,"max_calls_including_bootstrap":args.max_calls,
              "clock":"fixed wall-clock pace; world continues during model subprocess; full horizon retained",
              "tight_window_reason":"separate pilot from old 180s fixture: prior simple-model 9-11s *10 plus about 140s navigation requires about 250s; round to 300s before calls",
              "fixed_grid":"T0-based 60/300/600, not reset by events; no initial call",
              "human_event":"task_mismatch==1; same DSL and revision-rearming mode as model watches",
              "watch_semantics":"false-to-true or true on a new task revision" if args.revision_keyed else "false-to-true only",
              "expiry_gate":"drop latched triggers after current public deadline; do not cancel or erase in-flight calls" if args.skip_expired else "none",
              "model_event":"one T0 bootstrap call counted in same budget; replace watches only on applied response",
              "hybrid":"event or T0-based 600s grid, reported separately from pure events",
              "queue":"one in-flight; first trigger latched, coalesced; response applied next tick; stale/expired refused",
              "cost":"same schema and instructions all calls; bootstraps/failures/after-deadline calls count; money unknown",
              "analysis":"report individual episodes, score/call/token/lag; exploratory diagnostic, no general win-rate or significance claim",
              "subset":args.episode,"sensitivity_note":"explicit subset is post-pilot exploration; do not pool with original matrix",
              "scope":"public navigation tasks; no damage or adversarial win rate; not evidence LLM needed for navigation"}
    save(args.out/"protocol.json",protocol)
    cargo=shutil.which("cargo") or str(Path.home()/".cargo/bin/cargo.exe")
    target=args.out/"target"
    for action,extra in (("test",[]),("build",["--release"])):
        proc=subprocess.run([cargo,action,"--manifest-path",str(frozen/"Cargo.toml"),"-p","harpoon-core",
            "--example","realtime_live","--locked","--target-dir",str(target),*extra],cwd=frozen,capture_output=True,text=True,encoding="utf-8",errors="replace")
        (args.out/f"{action}.log").write_text(proc.stdout+proc.stderr,encoding="utf-8")
        if proc.returncode:raise RuntimeError(f"{action} failed; see log")
    sys.path.insert(0,str(frozen/"research/realtime"))
    import live_controller as live
    binary=target/"release/examples/realtime_live.exe"
    calls=args.out/"calls";calls.mkdir()
    rows=[];checks=[]
    for index,(family,policy,seed) in enumerate(matrix):
        tag=f"e{index:02d}-{family}-{policy}-{seed}"
        directory=args.out/"episodes"/tag
        print(json.dumps({"starting":tag}),flush=True)
        run=live.run_episode(binary,frozen,directory,calls,tag,family,policy,seed,args.tick_ms,300,args.max_calls,args.effort,args.backend,
                             args.revision_keyed,args.skip_expired,args.script_delay_ms)
        engine=json.loads((directory/"engine.json").read_text(encoding="utf-8"))
        checks.append(audit(run,engine))
        proc=subprocess.run([str(binary),"--replay",str(directory/"engine.json")],cwd=frozen,capture_output=True,text=True,encoding="utf-8")
        (directory/"replay.log").write_text(proc.stdout+proc.stderr,encoding="utf-8")
        if proc.returncode:raise RuntimeError("cross-process replay failed")
        row={k:run[k] for k in ("family","policy","seed","success","reached_tick","calls","usage","world_hash","budget_exhausted")}
        row["trace"]=str(directory/"run.json")
        rows.append(row);save(args.out/"summary.json",rows)
        print(json.dumps({"completed":row}),flush=True)
    tests=int(re.search(r"test result: ok\. (\d+) passed;",(args.out/"test.log").read_text()).group(1))
    save(args.out/"validation.json",{"status":"passed","backend":args.backend,"episodes":len(rows),
        "unit_tests":tests,"checks":checks,"cross_process_replays":len(rows),"binary_sha256":sha(binary),
        "model_calls":sum(r["calls"] for r in rows) if args.backend=="codex" else 0,"money_cost":None})
    print(json.dumps({"status":"passed","episodes":len(rows),"out":str(args.out)}),flush=True)


if __name__=="__main__":main()

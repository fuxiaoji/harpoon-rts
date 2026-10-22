"""Freeze, validate, and run the bounded M2a wake-scheduling exploration.

prepare builds once; checks/scripted/live use that immutable source and binary.
The real stage is a development pilot, not a confirmatory seven-arm benchmark.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib
import itertools
import json
from pathlib import Path
import random
import re
import shutil
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
METHODS={"gf60":("f60",True),"gf300":("f300",True),"gf600":("f600",True),
         "eh":("eh",True),"em":("em",True),"hh600":("hh600",True),"hm600":("hm600",True),
         "rawf60":("f60",False),"rawf300":("f300",False),"rawf600":("f600",False),
         "public":("eh",True)}


def read(path):return json.loads(path.read_text(encoding="utf-8"))
def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding="utf-8")
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def parameters(phase,gap,window):return {"first_event_tick":phase,"gap_s":gap,"event_window_s":window,"episode_s":1080,"notice_every_s":30}


def case(phase,gap,window,seed,method,budget=6,delay=100,backend="scripted",clock="injected"):
    return {"parameters":parameters(phase,gap,window),"seed":seed,"method":method,"budget":budget,
            "delay":delay,"backend":backend,"clock":clock,"family":"d5"}


def prepare(root,base):
    frozen=root/"frozen-source"
    if frozen.exists():raise ValueError("frozen source already exists")
    root.mkdir(parents=True,exist_ok=True)
    shutil.copytree(base,frozen,ignore=shutil.ignore_patterns("__pycache__"))
    sources=["crates/harpoon-core/examples/realtime_live.rs","research/realtime/llm_client.py",
             "research/realtime/m2_controller.py","research/realtime/m2_audit.py","research/realtime/test_m2_controller.py",
             "scripts/run-realtime-m2.py"]
    for source in sources:
        target=frozen/source;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/source,target)
    manifest=[{"path":p.relative_to(frozen).as_posix(),"bytes":p.stat().st_size,"sha256":sha(p)} for p in sorted(frozen.rglob("*")) if p.is_file()]
    save(root/"source-manifest.json",manifest)
    cargo=shutil.which("cargo") or str(Path.home()/".cargo/bin/cargo.exe")
    for action,extra in (("test",[]),("build",["--release"])):
        result=subprocess.run([cargo,action,"--manifest-path",str(frozen/"Cargo.toml"),"-p","harpoon-core","--example","realtime_live",
                               "--locked","--target-dir",str(root/"target"),*extra],cwd=frozen,capture_output=True,text=True,encoding="utf-8",errors="replace")
        (root/f"{action}.log").write_text(result.stdout+result.stderr,encoding="utf-8")
        if result.returncode:raise RuntimeError(f"{action} failed")
    tests=int(re.search(r"test result: ok\. (\d+) passed;",(root/"test.log").read_text()).group(1))
    save(root/"prepare-validation.json",{"status":"passed","rust_tests":tests,"source_files":len(manifest),
                                         "binary_sha256":sha(root/"target/release/examples/realtime_live.exe")})
    print(json.dumps({"prepared":str(root),"rust_tests":tests}),flush=True)


def matrix(mode):
    if mode=="checks":
        rows=[]
        for retries in (0,1):
            r=case(59,30,420,7,"em");r.update(label=f"stale-init-retries-{retries}",initialization_retries=retries);rows.append(r)
        r=case(59,30,420,7,"em");r.update(label="two-faults-bounded",fault_calls=[0,1]);rows.append(r)
        r=case(59,30,420,7,"em",delay=10);r.update(label="empty-watch-no-hidden-fallback",scripted_conditions=[]);rows.append(r)
        r=case(11,30,420,7,"em",delay=10);r.update(label="stale-at-application-boundary");rows.append(r)
        r=case(59,30,420,7,"em",budget=1);r.update(label="retry-cannot-exceed-budget");rows.append(r)
        r=case(59,30,420,7,"em",delay=1000);r.update(label="in-flight-at-terminal-no-retry");rows.append(r)
        r=case(59,30,420,7,"eh");r.update(label="D0-zero-call",family="d0",parameters={"episode_s":1080});rows.append(r)
        return rows
    rows=[]
    if mode=="scripted":
        for phase,gap,window,seed in itertools.product((59,239),(30,180),(420,600),(0,1)):
            rows.append(case(phase,gap,window,seed,"public",backend="public",delay=0))
            for method,budget,delay in itertools.product([m for m in METHODS if m!="public"],(2,6),(100,150)):
                rows.append(case(phase,gap,window,seed,method,budget,delay))
    else:
        # One development seed, 8 factorial contexts, 3 real arms + public control.
        # Cases form time blocks; all four arms within each block are shuffled.
        blocks=list(itertools.product((59,239),(30,180),(420,600)))
        random.Random(20261022).shuffle(blocks)
        for index,(phase,gap,window) in enumerate(blocks):
            block=[case(phase,gap,window,7,method,backend="public" if method=="public" else "codex",clock="wall")
                   for method in ("gf300","eh","em","public")]
            random.Random(20261022+index).shuffle(block);rows.extend(block)
    return rows


def run(root,mode):
    frozen=root/"frozen-source"
    for item in read(root/"source-manifest.json"):
        path=frozen/item["path"];assert sha(path)==item["sha256"] and path.stat().st_size==item["bytes"]
    batch=root/mode
    batch.mkdir(exist_ok=False)
    order=matrix(mode)
    save(batch/"protocol.json",{"stage":"M2a development exploration","mode":mode,"order":order,"model":"gpt-6.1-sol",
        "effort":"low","tick_ms":100,"world":"continues during real requests; scripted injected latency explicit",
        "termination":"judge ends only after all private updates published and current task reached or deadline passed; horizon cap1080",
        "visibility":"constant initial horizon1080; no phase/gap/window/future list/seed/method tag in model input",
        "recovery":"at most one extra call after a failed/rejected initialization receipt, before any watch accepted; no periodic or human fallback",
        "gating":"current navigation plan revision equals task revision => skip, except generating initial model watch; raw fixed arms retain no-plan gate",
        "cost":"all calls including initialization/stale/failure/retry count; invocation caps are not token/dollar matching; money unknown",
        "inference":"fixed low; no inference-budget adaptation in this stage",
        "analysis":"paired development grid, descriptive interactions; not a population win rate or confirmatory significance test",
        "real_scope":"GF300/EH/EM across 8 contexts plus zero-LLM control; other four strong arms and raw arms scripted only"})
    sys.path.insert(0,str(frozen/"research/realtime"))
    live=importlib.import_module("m2_controller");audit=importlib.import_module("m2_audit").audit
    binary=root/"target/release/examples/realtime_live.exe"
    calls=batch/"calls";calls.mkdir()
    summaries=[]
    for index,c in enumerate(order):
        tag=f"r{index:04d}"
        policy,gate=METHODS[c["method"]]
        directory=batch/"episodes"/tag
        if mode!="scripted" or index%40==0:print(json.dumps({"starting":tag,"mode":mode,"case":c}),flush=True)
        result=live.run_episode(binary,frozen,directory,calls,tag,c["family"],policy,c["seed"],
            tick_ms=100,max_calls=c["budget"],backend=c["backend"],case_parameters=c["parameters"],
            clock_mode=c["clock"],delay_sim_s=c["delay"],initialization_retries=c.get("initialization_retries",1),
            gate_current=gate,fault_calls=c.get("fault_calls",()),scripted_conditions=c.get("scripted_conditions"))
        engine=read(directory/"engine.json")
        check=audit(result,engine)
        replay=subprocess.run([str(binary),"--replay",str(directory/"engine.json")],cwd=frozen,capture_output=True,text=True,encoding="utf-8")
        (directory/"replay.log").write_text(replay.stdout+replay.stderr,encoding="utf-8")
        assert replay.returncode==0,replay.stderr
        row={"index":index,"case":c,"trace":str(directory/"run.json"),"audit":check,
             **{k:result[k] for k in ("success","reached_tick","terminal_tick","calls","usage","initialization_retries_used","watch_initialized","world_hash")}}
        summaries.append(row);save(batch/"summary.json",summaries)
        if mode!="scripted" or index%40==0:print(json.dumps({"completed":index+1,"total":len(order),"success":row["success"],"calls":row["calls"],"terminal_tick":row["terminal_tick"]}),flush=True)
    if mode=="checks":
        by={r["case"]["label"]:r for r in summaries}
        assert not by["stale-init-retries-0"]["success"] and by["stale-init-retries-0"]["calls"]==1
        assert by["stale-init-retries-1"]["success"] and by["stale-init-retries-1"]["calls"]==2
        assert not by["two-faults-bounded"]["success"] and by["two-faults-bounded"]["calls"]==2
        assert not by["empty-watch-no-hidden-fallback"]["success"] and by["empty-watch-no-hidden-fallback"]["calls"]==1
        assert by["stale-at-application-boundary"]["success"] and by["stale-at-application-boundary"]["calls"]==3
        assert by["retry-cannot-exceed-budget"]["calls"]==1 and by["retry-cannot-exceed-budget"]["initialization_retries_used"]==0
        assert by["in-flight-at-terminal-no-retry"]["calls"]==1 and by["in-flight-at-terminal-no-retry"]["initialization_retries_used"]==0
        assert by["D0-zero-call"]["success"] and by["D0-zero-call"]["calls"]==0
    for row in summaries:
        if row["case"]["backend"]=="public":assert row["success"],"physical feasibility control failed"
    save(batch/"validation.json",{"status":"passed","episodes":len(summaries),"independent_audits":len(summaries),"cross_process_replays":len(summaries),
                                  "real_model_requests":sum(r["calls"] for r in summaries if r["case"]["backend"]=="codex")})
    print(json.dumps({"status":"passed","mode":mode,"episodes":len(summaries)}),flush=True)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage",required=True,type=Path)
    parser.add_argument("--mode",required=True,choices=("prepare","checks","scripted","live"))
    parser.add_argument("--base-source",type=Path,default=Path("E:/CodexBuilds/harpoon-research-m1d-20261022/live-keyed-v2/frozen-source"))
    args=parser.parse_args()
    if args.mode=="prepare":prepare(args.stage,args.base_source)
    else:run(args.stage,args.mode)


if __name__=="__main__":main()

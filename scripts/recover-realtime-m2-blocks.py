"""Rerun whole pre-identified temporal blocks after the primary batch completes."""
import argparse
import hashlib
import importlib
import json
from pathlib import Path
import subprocess
import sys


def read(path):return json.loads(path.read_text(encoding="utf-8"))
def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding="utf-8")


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument("--stage",type=Path,required=True);args=p.parse_args();root=args.stage
    assert read(root/"live/validation.json")["status"]=="passed","finish the primary batch first; no parallel provider calls"
    intent=read(root/"recovery-intent.json");order=intent["order"]
    assert intent["blocks"]==sorted({c["original_index"]//4 for c in order})
    frozen=root/"frozen-source"
    for item in read(root/"source-manifest.json"):
        assert hashlib.sha256((frozen/item["path"]).read_bytes()).hexdigest()==item["sha256"]
    binary=root/"target/release/examples/realtime_live.exe"
    assert hashlib.sha256(binary.read_bytes()).hexdigest()==read(root/"prepare-validation.json")["binary_sha256"]
    sys.path.insert(0,str(frozen/"research/realtime"))
    live=importlib.import_module("m2_controller");audit=importlib.import_module("m2_audit").audit
    batch=root/"live-recovery-v2";batch.mkdir(exist_ok=False);(batch/"calls").mkdir()
    save(batch/"protocol.json",{"stage":"M2a whole-block incident replacement","order":order,
        "selection":intent["reason"],"blocks":intent["blocks"],"incident_audit":"../backend-incident-audit.json",
        "analysis":"post-incident full temporal block replacements; preserve and report original outcomes and all resource use",
        "clock":"same 100ms tick, natural termination, low, cap6, frozen controller/binary as primary",
        "sequence":"run only after the primary batch; no other experiment provider calls in parallel"})
    methods={"gf300":"f300","eh":"eh","em":"em","public":"eh"};rows=[]
    for i,c in enumerate(order):
        assert c["clock"]=="wall" and c["backend"] in ("codex","public") and c["method"] in methods
        tag=f"r{i:04d}";directory=batch/"episodes"/tag
        print(json.dumps({"starting_replacement":i,"original_index":c["original_index"],"case":c}),flush=True)
        result=live.run_episode(binary,frozen,directory,batch/"calls",tag,c["family"],methods[c["method"]],c["seed"],
            tick_ms=100,max_calls=c["budget"],effort="low",backend=c["backend"],clock_mode="wall",case_parameters=c["parameters"],
            revision_keyed=True,skip_expired=True,initialization_retries=1,gate_current=True)
        assert result["backend"]==c["backend"] and result["clock_mode"]=="wall"
        engine=read(directory/"engine.json");check=audit(result,engine)
        replay=subprocess.run([str(binary),"--replay",str(directory/"engine.json")],cwd=frozen,capture_output=True,text=True)
        assert replay.returncode==0
        (directory/"replay.log").write_text(replay.stdout+replay.stderr,encoding="utf-8")
        row={"index":i,"case":c,"trace":str(directory/"run.json"),"audit":check,
             **{k:result[k] for k in ("success","reached_tick","terminal_tick","calls","usage","initialization_retries_used","watch_initialized","world_hash")}}
        rows.append(row);save(batch/"summary.json",rows)
        print(json.dumps({"completed_replacement":i+1,"total":len(order),"success":row["success"],"calls":row["calls"]}),flush=True)
    assert all(r["success"] for r in rows if r["case"]["backend"]=="public")
    save(batch/"validation.json",{"status":"passed","episodes":len(rows),"independent_audits":len(rows),"cross_process_replays":len(rows),
                                   "real_model_requests":sum(r["calls"] for r in rows if r["case"]["backend"]=="codex")})
    print(json.dumps({"status":"passed","replacements":len(rows)}),flush=True)


if __name__=="__main__":main()

"""Resume only unsent frozen M3b slots after verified service availability.

No failed slot is resent. Original schedule, frozen inputs and first receipts
stay immutable. The recovery time block and changed CLI patch are disclosed.
"""
import argparse
import hashlib
import importlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import time


def save(p, v):
    p.write_text(json.dumps(v, ensure_ascii=False, indent=2), encoding="utf-8")


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--stage",type=Path,required=True)
    p.add_argument("--allow-real-model",action="store_true")
    a=p.parse_args()
    if not a.allow_real_model:
        raise ValueError("Explicit --allow-real-model required")
    stage=a.stage.resolve()
    source=stage/"frozen-source/research/realtime"
    sys.path.insert(0,str(source))
    pilot=importlib.import_module("run_m3b_pilot")
    binary=stage/"target/release/examples/realtime_nav_service.exe"
    pilot.check_gate(stage,str(binary))
    out=stage/"r3"
    block=out/"recovery-block-20261023"
    block.mkdir(exist_ok=False)
    schedule=pilot.read(out/"schedule.json")
    previous=[]
    for slot in schedule:
        d=out/"calls"/f'{slot["index"]:03d}'
        if d.exists():
            if not (d/"result.json").exists():
                raise ValueError("Uncertain prior request; cannot resume")
            previous.append(slot["index"])
    if previous != list(range(len(previous))):
        raise ValueError("Previous calls must be a complete prefix")
    pending=schedule[len(previous):]
    version=subprocess.run([shutil.which("codex"),"--version"],capture_output=True,text=True,
                           encoding="utf-8",errors="replace").stdout.strip()
    save(block/"manifest.json",{"created_unix_ns":time.time_ns(),"previous_slots":previous,
        "slots":pending,"max_additional_calls":len(pending),"total_attempt_cap":72,
        "cli_version":version,"ordinary_usage_allowed_preflight":True,
        "preflight_source":"Codex get_usage_limits 2026-10-23 returned ordinaryUsageAllowed=true and 0% used",
        "source_sha256":hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "frozen_manifest_sha256":pilot.sha(out/"manifest.json"),
        "policy":"Never resend failed slots; no change to prompt/model/effort/action class. Stop immediately on renewed quota refusal, otherwise after three consecutive failures."})
    snapshots={s["id"]:s for s in pilot.read(out/"snapshots.json")}
    failures=0
    sent=[]
    for slot in pending:
        receipt=pilot.client.call(snapshots[slot["case_id"]]["input"],slot["effort"],
            out/"calls"/f'{slot["index"]:03d}',timeout_s=180,schema_override=pilot.study.SCHEMA,
            instructions_override=pilot.INSTRUCTIONS,validator=pilot.study.validate)
        sent.append(slot["index"])
        failures=failures+1 if receipt["status"]!="ok" else 0
        save(block/"progress.json",{"sent":sent,"last_slot":slot,"status":receipt["status"]})
        print(json.dumps({**slot,"status":receipt["status"],"elapsed_s":receipt["elapsed_s"],
            "choice":(receipt["answer"] or {}).get("choice"),"usage":receipt["usage"]}),flush=True)
        quota=any("usage limit" in str(e).lower() for e in receipt["transport_events"])
        if quota or failures>=3:
            save(block/"stopped.json",{"quota_refusal":quota,"consecutive_failed":failures,"after":slot["index"]})
            break
    save(block/"completed.json",{"end_unix_ns":time.time_ns(),"sent":sent,"count":len(sent)})


if __name__=="__main__":
    main()

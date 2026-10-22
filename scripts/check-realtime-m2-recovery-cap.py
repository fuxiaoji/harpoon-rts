"""Three no-provider controls for repeated initialization invalidation."""
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
    p=argparse.ArgumentParser(description=__doc__);p.add_argument("--stage",required=True,type=Path);args=p.parse_args();root=args.stage
    frozen=root/"frozen-source";sys.path.insert(0,str(frozen/"research/realtime"))
    for item in read(root/"source-manifest.json"):assert hashlib.sha256((frozen/item["path"]).read_bytes()).hexdigest()==item["sha256"]
    live=importlib.import_module("m2_controller");audit=importlib.import_module("m2_audit").audit
    def forbid(*args,**kwargs):raise AssertionError("recovery control forbids provider calls")
    live.llm_client.call=forbid
    binary=root/"target/release/examples/realtime_live.exe"
    assert hashlib.sha256(binary.read_bytes()).hexdigest()==read(root/"prepare-validation.json")["binary_sha256"]
    batch=root/"recovery-cap-controls";batch.mkdir(exist_ok=False);(batch/"calls").mkdir()
    order=[{"parameters":{"first_event_tick":59,"gap_s":180,"event_window_s":600,"episode_s":1080,"notice_every_s":30},
            "seed":0,"method":"em","budget":budget,"delay":150,"backend":"scripted","clock":"injected","family":"d5",
            "initialization_retries":limit,"label":label} for limit,budget,label in ((1,6,"one-extra-retry"),(5,6,"retry-within-call-cap"),(5,2,"call-cap-two"))]
    save(batch/"protocol.json",{"stage":"posthoc bounded-retry sensitivity","order":order,
         "analysis":"scripted fixed150s response; two successive public revisions may invalidate two initialization attempts; compare retry limits with same total call cap",
         "boundary":"main real pilot remains the frozen one-extra-retry point; no real model sample added"})
    rows=[]
    for i,c in enumerate(order):
        tag=f"r{i:04d}";directory=batch/"episodes"/tag
        result=live.run_episode(binary,frozen,directory,batch/"calls",tag,"d5","em",0,backend="scripted",clock_mode="injected",delay_sim_s=150,
                               case_parameters=c["parameters"],max_calls=c["budget"],initialization_retries=c["initialization_retries"])
        assert result["backend"]=="scripted" and all(r["record"]["requested_model"] is None for r in result["requests"])
        engine=read(directory/"engine.json");check=audit(result,engine)
        proc=subprocess.run([str(binary),"--replay",str(directory/"engine.json")],cwd=frozen,capture_output=True,text=True)
        assert proc.returncode==0;(directory/"replay.log").write_text(proc.stdout+proc.stderr,encoding="utf-8")
        rows.append({"index":i,"case":c,"trace":str(directory/"run.json"),"audit":check,
            **{k:result[k] for k in ("success","reached_tick","terminal_tick","calls","usage","initialization_retries_used","watch_initialized","world_hash")}})
        save(batch/"summary.json",rows)
    assert not rows[0]["success"] and rows[0]["calls"]==2
    assert rows[1]["success"] and rows[1]["calls"]==3 and rows[1]["initialization_retries_used"]==2
    assert not rows[2]["success"] and rows[2]["calls"]==2
    save(batch/"validation.json",{"status":"passed","episodes":3,"independent_audits":3,"cross_process_replays":3,"real_model_requests":0,
        "bounded_retry_success":rows[1]["success"],"no_budget_overrun":True})
    print(json.dumps(read(batch/"validation.json")),flush=True)


if __name__=="__main__":main()

"""Post-pilot physical delay envelope; scripted own-observation commands, no LLM.

Uses the completed live batch's exact binary and maps no wall clock. Responses
are injected at a grid of simulated delays to diagnose that task's tolerance.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding="utf-8")


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--batch",required=True,type=Path)
    p.add_argument("--out",required=True,type=Path)
    args=p.parse_args()
    args.out.mkdir(parents=True,exist_ok=False)
    binary=args.batch/"target/release/examples/realtime_live.exe"
    protocol={"scope":"post-pilot scripted physical calibration, not model or clock comparison",
        "case":"d2","seed":7,"tight_window_s":300,"injected_response_delay_grid":list(range(0,151,5)),
        "application":"first public change plus delay plus one tick", "model_calls":0,
        "binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest()}
    save(args.out/"protocol.json",protocol)
    rows=[]
    for delay in protocol["injected_response_delay_grid"]:
        with (args.out/f"stderr-{delay}.log").open("w",encoding="utf-8") as err:
            process=subprocess.Popen([str(binary),"--case","d2","--seed","7","--tight-window-s","300"],
                cwd=args.batch/"frozen-source",stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,
                text=True,encoding="utf-8",bufsize=1)
            read=lambda:json.loads(process.stdout.readline())
            def exchange(value):
                process.stdin.write(json.dumps(value)+"\n");process.stdin.flush();return read()
            current=read()["input"]
            publication=None
            applied=None
            for now in range(1,1201):
                plan=None
                if publication is not None and now==publication+delay+1:
                    plan={"request_id":0,"action":"goto","unit":current["observation"]["units"][0]["id"],
                          "task_revision":current["public_task"]["revision"],"waypoint":current["public_task"]["goal"]}
                    applied=now
                packet=exchange({"kind":"step","tick":now,"plan":plan})
                current=packet["input"]
                if current["public_task"]["revision"]==1 and publication is None:publication=now
            trace=exchange({"kind":"finish"})
            process.wait(timeout=10)
            assert process.returncode==0 and trace["replay_verified"]
            save(args.out/f"engine-{delay}.json",trace)
            rows.append({"response_delay_s":delay,"apply_tick":applied,"success":trace["success"],
                         "reached_tick":trace["reached_tick"],"world_hash":trace["world_hash"]})
    save(args.out/"summary.json",rows)
    save(args.out/"validation.json",{"status":"passed","branches":len(rows),"model_calls":0,
        "largest_successful_grid_delay_s":max((r["response_delay_s"] for r in rows if r["success"]),default=None),
        "grid_resolution_s":5,"scope":protocol["scope"]})
    print(json.dumps({"rows":rows}),flush=True)


if __name__=="__main__":main()

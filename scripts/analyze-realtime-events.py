"""Summarize the M1d event-rearming/expiry diagnostic, without model calls.

Run analyze-realtime-live.py for each complete batch first. The assertions below
are engineering acceptance checks, not tests of population-level superiority.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path


def read(path):return json.loads(path.read_text(encoding="utf-8"))
def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding="utf-8")
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage",required=True,type=Path)
    parser.add_argument("--scripted-only",action="store_true")
    args=parser.parse_args()
    root=args.stage
    batches=["scripted-legacy-v1","scripted-keyed-v2"]
    if not args.scripted_only:batches.append("live-keyed-v2")
    analyses={name:read(root/name/"analysis.json") for name in batches}
    for name,analysis in analyses.items():
        assert analysis["validation"]["status"]=="passed"
        assert read(root/name/"validation.json")["status"]=="passed"
    old,new=(analyses[n]["episodes"] for n in batches[:2])
    assert len(old)==len(new)==20
    assert [(r["family"],r["policy"],r["seed"]) for r in old]==[(r["family"],r["policy"],r["seed"]) for r in new]
    pairs=[]
    for a,b in zip(old,new):
        family,policy,seed=a["family"],a["policy"],a["seed"]
        if family=="d5":
            # No expiry gate fires here: rearming is the active change.
            assert not a["success"] and b["success"]
            assert a["first_final_revision_snapshot"] is None and b["first_final_revision_snapshot"] is not None
            assert a["statuses"].get("stale_response")==b["statuses"].get("stale_response")==1
            assert a["skipped_expired_triggers"]==b["skipped_expired_triggers"]==0
            assert b["calls"]==a["calls"]+1
            assert a["routine_notifications"]==b["routine_notifications"]==39
        elif family=="d2":
            # These fixed-grid arms never consult either event Watch.
            assert policy in ("f300","f600") and a["success"]==b["success"]==False
            assert b["calls_started_after_public_deadline"]==0
            assert a["calls"]-b["calls"]==a["calls_started_after_public_deadline"]
        elif family=="d0":
            assert a["success"] and b["success"] and a["calls"]==b["calls"]==0
        pairs.append({"family":family,"policy":policy,"seed":seed,
                      "old_success":a["success"],"new_success":b["success"],
                      "old_calls":a["calls"],"new_calls":b["calls"]})
    core_hashes={}
    for name in batches:
        manifest=read(root/name/"source-manifest.json")
        core={m["path"]:m["sha256"] for m in manifest if m["path"].startswith("crates/harpoon-core/src/")}
        for path,value in core.items():assert sha(root/name/"frozen-source"/path)==value
        core_hashes[name]=core
    assert all(core==core_hashes[batches[0]] for core in core_hashes.values())
    output={"scope":"M1d exploratory engineering diagnostic, not a new method or general win rate",
            "scripted_episodes":40,"paired_diagnostics":pairs,
            "dense_old_successes":sum(p["old_success"] for p in pairs if p["family"]=="d5"),
            "dense_new_successes":sum(p["new_success"] for p in pairs if p["family"]=="d5"),
            "expiry_old_calls":sum(p["old_calls"] for p in pairs if p["family"]=="d2"),
            "expiry_new_calls":sum(p["new_calls"] for p in pairs if p["family"]=="d2"),
            "real_episodes":analyses.get("live-keyed-v2",{}).get("episodes",[]),
            "real_usage":analyses.get("live-keyed-v2",{}).get("usage_total"),
            "real_model_calls":analyses.get("live-keyed-v2",{}).get("real_model_calls",0),
            "money_cost":None,"core_source_files_unchanged_across_batches":len(core_hashes[batches[0]]),
            "validation":{"status":"passed_scripted_only" if args.scripted_only else "passed","batches":batches}}
    save(root/("scripted-analysis.json" if args.scripted_only else "stage-analysis.json"),output)
    print(json.dumps({k:v for k,v in output.items() if k!="paired_diagnostics"},ensure_ascii=False))


if __name__=="__main__":main()

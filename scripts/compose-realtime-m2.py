"""Compose audited clean replacements, sensitivity results, and all usage."""
import argparse
from collections import Counter,defaultdict
import csv
import json
from pathlib import Path


def read(path):return json.loads(path.read_text(encoding="utf-8"))
def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2),encoding="utf-8")
def totals(calls):return {key:sum((c.get("usage") or {}).get(key,0) for c in calls) for key in ("input_tokens","output_tokens","reasoning_output_tokens","cached_input_tokens")}


def summary(rows):
    groups=defaultdict(list)
    for row in rows:groups[row["method"]].append(row)
    return [{"method":m,"episodes":len(rs),"successes":sum(r["success"] for r in rs),"model_calls":sum(r["model_calls"] for r in rs),
             "decision_requests":sum(r["requests"] for r in rs),"input_tokens":sum(r["usage"]["input_tokens"] for r in rs),
             "output_tokens":sum(r["usage"]["output_tokens"] for r in rs),"initialization_retries":sum(r["initialization_retries"] for r in rs)} for m,rs in groups.items()]


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument("--stage",required=True,type=Path);args=p.parse_args();root=args.stage
    batches=("checks","scripted","mechanism-fixed-v2","live","live-recovery-v2")
    analyses={name:read(root/name/"analysis.json") for name in batches}
    assert all(a["status"]=="passed" for a in analyses.values())
    intent=read(root/"recovery-intent.json");incident=read(root/"backend-incident-audit.json")
    old=analyses["live"];new=analyses["live-recovery-v2"]
    replacement_order=read(root/"live-recovery-v2/protocol.json")["order"]
    assert replacement_order==intent["order"]
    mapping={case["original_index"]:row for case,row in zip(replacement_order,new["episodes"])}
    assert sorted(mapping)==[i for block in intent["blocks"] for i in range(block*4,block*4+4)]
    clean=[];clean_calls=[];changed=[]
    for row in old["episodes"]:
        original=row["index"];r=mapping.get(original,row)
        assert r["method"]==row["method"] and r["parameters"]==row["parameters"] and r["seed"]==row["seed"]
        batch="live-recovery-v2" if original in mapping else "live"
        source=analyses[batch]
        clean.append({**r,"source_index":r["index"],"source_batch":batch,"index":original})
        clean_calls.extend({**c,"source_index":c["episode"],"source_batch":batch,"episode":original} for c in source["calls"] if c["episode"]==r["index"])
        if original in mapping:changed.append({"original_index":original,"method":r["method"],"old_success":row["success"],"replacement_success":r["success"],"old_calls":row["model_calls"],"replacement_calls":r["model_calls"]})
    assert len(clean)==32 and Counter(r["method"] for r in clean)=={"gf300":8,"eh":8,"em":8,"public":8}
    assert all(r["success"] for r in clean if r["method"]=="public")
    lo,hi=incident["conservative_possible_overlap_window_unix_ns"]
    overlaps=[]
    for c in old["calls"]:
        if c["backend"]!="codex":continue
        raw=read(Path(c["call_directory"])/"result.json")
        start=raw["requested_unix_ns"];end=start+int(raw["elapsed_s"]*1e9)
        if start<=hi and end>=lo:overlaps.append(c["episode"])
    assert sorted({i//4 for i in overlaps})==intent["blocks"]
    main_end=max(read(Path(c["call_directory"])/"result.json")["requested_unix_ns"]+int(c["client_elapsed_s"]*1e9) for c in old["calls"] if c["backend"]=="codex")
    replacement_start=min(read(Path(c["call_directory"])/"result.json")["requested_unix_ns"] for c in new["calls"] if c["backend"]=="codex")
    assert replacement_start>main_end
    all_calls=[];resource_batches={}
    for name in ("live","live-recovery-v2","mechanism"):
        records=[]
        for path in sorted((root/name/"calls").glob("r*/result.json")):
            record=read(path)
            if record.get("requested_model") is None:continue
            records.append({"path":str(path),"usage":record.get("usage"),"status":record["status"],"batch":name})
        resource_batches[name]={"saved_receipts":len(records),"usage":totals(records),"usage_missing":sum(c["usage"] is None for c in records)}
        all_calls.extend(records)
    unknown=incident["unfinished_client_requests_usage_unknown"]
    for directory in unknown:assert (root/directory/"input.json").exists() and not (root/directory/"result.json").exists()
    clean_model_calls=[c for c in clean_calls if c["backend"]=="codex"]
    output={"scope":"M2a exploratory paired timing grid, one geometry seed; not a confirmatory or population ranking",
        "status":"passed","episodes":clean,"calls":clean_calls,"method_summary":summary(clean),"original_primary_summary":summary(old["episodes"]),
        "whole_block_replacements":changed,"clean_model_calls":len(clean_model_calls),"clean_usage":totals(clean_model_calls),
        "all_resource_batches":resource_batches,"all_saved_real_model_receipts":len(all_calls),"all_saved_usage":totals(all_calls),
        "additional_started_client_requests_usage_unknown":len(unknown),"unknown_request_directories":unknown,"money_cost":None,
        "successful_em_without_any_model_condition_event":[r["index"] for r in clean if r["method"]=="em" and r["success"] and r["model_condition_events"]==0],
        "validated_episodes_including_replacements":sum(len(a["episodes"]) for a in analyses.values()),
        "unique_clean_comparison_episodes":32,"scientific_exclusion":"mechanism accidental backend batch excluded completely; original possibly overlapping blocks retained for sensitivity and resource accounting"}
    save(root/"clean-analysis.json",output)
    fields=("index","source_batch","source_index","method","seed","phase_s","gap_s","window_s","budget","success","requests","model_calls","reached_tick","terminal_tick","model_condition_events","initialization_retries","input_tokens","output_tokens")
    with (root/"clean-summary.csv").open("w",encoding="utf-8-sig",newline="") as f:
        writer=csv.DictWriter(f,fieldnames=fields);writer.writeheader()
        for r in clean:
            flat={**r,"phase_s":r["parameters"]["first_event_tick"],"gap_s":r["parameters"]["gap_s"],"window_s":r["parameters"]["event_window_s"],
                  "input_tokens":r["usage"]["input_tokens"],"output_tokens":r["usage"]["output_tokens"]}
            writer.writerow({k:flat[k] for k in fields})
    print(json.dumps({k:output[k] for k in ("status","method_summary","original_primary_summary","clean_model_calls","all_saved_real_model_receipts","all_saved_usage","additional_started_client_requests_usage_unknown","validated_episodes_including_replacements")}),flush=True)


if __name__=="__main__":main()

"""Verify, archive without build caches, and inventory M3b stage evidence."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import shutil

ROOT=Path(__file__).resolve().parents[2]
PROJECT=ROOT/"harpoon-rts"


def read(p): return json.loads(p.read_text(encoding="utf-8"))
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,v): p.write_text(json.dumps(v,ensure_ascii=False,indent=2),encoding="utf-8")


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--stage",type=Path,required=True)
    p.add_argument("--archive",type=Path,required=True)
    a=p.parse_args()
    stage=a.stage.resolve(); archive=a.archive.resolve()
    archive.relative_to(ROOT)  # Catalog paths stay inside this repository.
    if archive.exists(): raise FileExistsError("Archive must be a new directory")
    r1=read(stage/"r1-cross-process-validation.json")
    r2=read(stage/"r2-validation.json")
    r3=read(stage/"r3-analysis-v2-20261023/validation.json")
    if (r1["status"]!="passed" or r1["episodes_expected"]!=304 or
        not r2["passed"] or r2["cells"]!=24 or not r3["passed"] or
        r3["smoke_only"] or not r3["cross_process_replay_passed"]):
        raise ValueError("Independent checks must pass before archiving")
    if not read(stage/"prior-evidence-validation.json")["passed"]:
        raise ValueError("Prior frozen evidence changed")
    active=list((stage/"r3/calls").glob("*"))
    if len(active)!=r3["requests"] or not 0<len(active)<=72:
        raise ValueError("Final model receipt count mismatch")
    if any(not (d/"result.json").exists() for d in active):
        raise ValueError("An uncertain or in-flight model call remains")
    if not read(stage/"m3c-calibration/audit.json")["all_passed"]:
        raise ValueError("Deadline calibration audit failed")
    timed=read(stage/"timed-execution-diagnostic-v2-20261023/validation.json")
    if not timed["passed"] or not timed["cross_process_replay_passed"]:
        raise ValueError("Timed execution diagnostic failed")
    archive.mkdir(parents=True)
    for f in sorted(stage.rglob("*")):
        if not f.is_file(): continue
        rel=f.relative_to(stage)
        if "__pycache__" in rel.parts or f.suffix==".pyc": continue
        if rel.parts[0]=="target" and rel.as_posix()!="target/release/examples/realtime_nav_service.exe": continue
        dest=archive/rel
        dest.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(f,dest)
        if sha(f)!=sha(dest): raise ValueError("Archive copy mismatch: "+str(rel))
    extra=["scripts/prepare-realtime-m3b.py","scripts/resume-realtime-m3b.py",
           "scripts/audit-realtime-m3b-r1.py","scripts/analyze-realtime-m3b.py",
           "scripts/analyze-realtime-m3b-v2.py",
           "scripts/plot-realtime-m3b.py","scripts/finalize-realtime-m3b.py",
           "scripts/compare-realtime-m3b-timed.py",
           "scripts/compare-realtime-m3b-timed-v2.py",
           "research/realtime/commitment_study_v2.py",
           "research/realtime/test_commitment_study_v2.py",
           "research/realtime/m3c-next-work-package.md"]
    for name in extra:
        src=PROJECT/name
        dest=archive/"analysis-source"/name
        dest.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(src,dest)
    calibration=ROOT/"scripts/calibrate-realtime-m3c.py"
    calibration_dest=archive/"analysis-source/repository-root/scripts/calibrate-realtime-m3c.py"
    calibration_dest.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(calibration,calibration_dest)
    rows=[{"path":f.relative_to(archive).as_posix(),"bytes":f.stat().st_size,"sha256":sha(f)}
          for f in sorted(archive.rglob("*")) if f.is_file()]
    save(archive/"artifact-manifest.json",{"version":"m3b-20261023","origin":str(stage),
         "files":rows,"excluded":"Rust build cache and Python bytecode; navigation executable retained",
         "bytes":sum(r["bytes"] for r in rows)})
    save(archive/"archive-validation.json",{"passed":True,"files":len(rows),
         "all_copies_sha256_equal":True,"manifest_sha256":sha(archive/"artifact-manifest.json")})
    local=["plan.md","scripts/calibrate-realtime-m3c.py"]
    local += ["harpoon-rts/"+n for n in extra]
    local += ["harpoon-rts/crates/harpoon-core/examples/realtime_nav_service.rs",
              "harpoon-rts/docs/research-realtime-m3b-20261023.md"]
    local += ["harpoon-rts/research/realtime/"+n for n in (
        "real_nav.py","m3b_llm_client.py","test_m3b_llm_client.py","run_m3b_pilot.py",
        "test_m3b_pilot.py","semantic_wake_study.py","test_semantic_wake_study.py",
        "commitment_study.py","test_commitment_study.py","m3b-protocol.md",
        "m3b-execution-addendum.md","README.md")]
    evidence=["artifact-manifest.json","archive-validation.json","r1/summary.json",
        "r1-cross-process-validation.json","r2/summary.json","r2/audit.json",
        "r2/information-value.json","r2-validation.json","r3/manifest.json",
        "r3/recovery-block-20261023/manifest.json","r3-analysis-v2-20261023/summary.json",
        "r3-analysis-v2-20261023/validation.json","m3c-calibration/summary.json",
        "m3c-calibration/audit.json","timed-execution-diagnostic-v2-20261023/summary.json",
        "timed-execution-diagnostic-v2-20261023/validation.json"]
    evidence += ["figures-final/"+stem+"."+ext for stem in
        ("wake-semantics","information-value","reasoning-pilot") for ext in ("png","svg")]
    inventory=ROOT/"resources/catalog/inventory.csv"
    catalog_files=[ROOT/n for n in local]+[archive/n for n in evidence]
    missing=[str(f) for f in catalog_files if not f.is_file()]
    if missing: raise FileNotFoundError(missing)
    with inventory.open("a",encoding="utf-8",newline="") as stream:
        writer=csv.writer(stream)
        for f in catalog_files:
            writer.writerow([f.relative_to(ROOT).as_posix(),
                "research;version=2026-10-23-m3b;use=wake-commitment-reasoning-pilot",
                f.suffix,f.stat().st_size,sha(f),""])
    print(json.dumps({"archive":str(archive),"files":len(rows),"bytes":sum(r["bytes"] for r in rows)}),flush=True)


if __name__=="__main__": main()

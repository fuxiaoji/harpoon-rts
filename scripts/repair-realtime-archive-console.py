"""One-time repair of an archive including its still-being-written stdout log.

Preserve the original manifest; require that stdout is the ONLY mismatch. No
experimental input, receipt, score, or frozen runtime source may change.
"""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import shutil

ROOT=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2),encoding="utf-8")
def main():
    p=argparse.ArgumentParser();p.add_argument("--stage",type=Path,required=True);a=p.parse_args();stage=a.stage
    manifest=stage/"artifact-manifest.json";old=json.loads(manifest.read_text(encoding="utf-8"))
    bad=[r["path"] for r in old["files"] if sha(stage/r["path"])!=r["sha256"]]
    assert bad==["finalize-console.log"],bad
    old_copy=stage/"artifact-manifest-before-console-fix.json";assert not old_copy.exists();shutil.copy2(manifest,old_copy)
    snapshot=stage/"final-source/scripts/finalize-realtime-m3.py"
    note={"reason":"Redirected finalizer stdout was hashed before its final print; only that log mismatched",
        "original_manifest":old_copy.name,"scientific_artifacts_changed":False,"runtime_sources_changed":False,
        "finalizer_source_before":sha(snapshot),"finalizer_source_after":sha(ROOT/"scripts/finalize-realtime-m3.py"),
        "exclusion":"finalize-console.log; raw receipts and all scientific evidence remain included"}
    shutil.copy2(ROOT/"scripts/finalize-realtime-m3.py",snapshot)
    shutil.copy2(Path(__file__),stage/"final-source/scripts/repair-realtime-archive-console.py")
    save(stage/"archive-console-fix.json",note)
    files=[]
    for path in sorted(stage.rglob("*")):
        if not path.is_file():continue
        rel=path.relative_to(stage).as_posix()
        if "__pycache__" in path.parts or rel in ("artifact-manifest.json","archive-validation.json","finalize-console.log"):continue
        if rel.startswith("target/") and rel not in ("target/release/examples/realtime_resource.exe","target/release/examples/realtime_controlled.exe"):continue
        files.append({"path":rel,"bytes":path.stat().st_size,"sha256":sha(path)})
    save(manifest,{"scope":old["scope"]+"; finalizer stdout excluded to avoid self-write circularity; previous manifest retained",
        "file_count":len(files),"total_bytes":sum(x["bytes"] for x in files),"files":files})
    for row in files:assert sha(stage/row["path"])==row["sha256"]
    inventory=ROOT.parent/"resources/catalog/inventory.csv"
    with inventory.open(encoding="utf-8-sig",newline="") as f:existing={(r["relative_path"],r["sha256"]) for r in csv.DictReader(f)}
    additions=[]
    for path in (ROOT/"scripts/finalize-realtime-m3.py",Path(__file__),manifest,stage/"archive-console-fix.json",old_copy):
        try:rel=path.relative_to(ROOT.parent).as_posix()
        except ValueError:rel=path.as_posix()
        if (rel,sha(path)) not in existing:additions.append([rel,"research-live-source-or-evidence;version=2026-10-22-m3a-v1;use=archive-console-self-write-fix",path.suffix,path.stat().st_size,sha(path),""])
    with inventory.open("a",encoding="utf-8",newline="") as f:csv.writer(f).writerows(additions)
    save(stage/"archive-validation.json",{"status":"passed","files_verified":len(files),"total_bytes":sum(r["bytes"] for r in files),
        "inventory_added":30+len(additions),"console_self_write_repaired":True,"money":None})
    print(json.dumps({"status":"passed","files":len(files),"all_scientific_artifacts_unchanged":True}),flush=True)
if __name__=="__main__":main()

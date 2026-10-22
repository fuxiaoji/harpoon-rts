"""Freeze M2b/M3a from the previously audited M2a core, never the moving core."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]
def save(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2),encoding="utf-8")
def replace(text,old,new):
    assert text.count(old)==1,old[:100]
    return text.replace(old,new)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser();p.add_argument("--stage",type=Path,required=True);a=p.parse_args();stage=a.stage
    stage.mkdir(exist_ok=False)
    base=Path("E:/CodexBuilds/harpoon-research-m2a-20261022/goal-control-v2/frozen-source")
    assert base.is_dir()
    frozen=stage/"frozen-source";shutil.copytree(base,frozen,ignore=shutil.ignore_patterns("__pycache__"))
    for path in ["crates/harpoon-core/examples/realtime_resource.rs","research/realtime/resource_study.py","research/realtime/test_resource_study.py","research/realtime/m3-protocol.md","scripts/prepare-realtime-m3.py","scripts/control-realtime-m2b.py"]:
        (frozen/path).parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/path,frozen/path)
    rust=(base/"crates/harpoon-core/examples/realtime_live.rs").read_text(encoding="utf-8")
    rust=replace(rust,'    let mut judge = Judge::new(template, seed)?;', '''    // Controlled diagnostic: identical stationary geometry before first event.
    let controlled=value("--controlled-geometry").is_some();
    if controlled {
        template.initial_goal=Vec2::new(0.,0.);
        template.scenario.units[0].pos=Vec2::new(0.,0.);
        template.scenario.units[0].speed_kts=0.;
        template.scenario.units[0].waypoints=Some(vec![Vec2::new(0.,0.)]);
    }
    let mut judge = Judge::new(template, seed)?;''')
    rust=replace(rust,'    let mut output = io::stdout().lock();', '''    if controlled {
        if judge.updates.len()!=2 {return Err("controlled geometry requires d5".into());}
        judge.updates[0].goal=Vec2::new(1.5,0.6);
        judge.updates[1].goal=Vec2::new(1.65,-0.6);
        judge.update=Some(judge.updates[0].clone());
    }
    let mut output = io::stdout().lock();''')
    (frozen/"crates/harpoon-core/examples/realtime_controlled.rs").write_text(rust,encoding="utf-8")
    source=(ROOT/"research/realtime/m2_controller.py").read_text(encoding="utf-8")
    source=replace(source,'initialization_retries=1, gate_current=True, fault_calls=(), scripted_conditions=None):',
        'initialization_retries=1, gate_current=True, fault_calls=(), scripted_conditions=None, ready_watch=False, event_quiet_s=0):')
    source=replace(source,'    require_backend_clock(backend,clock_mode)', '''    require_backend_clock(backend,clock_mode)
    if backend not in ("scripted","public"):
        raise ValueError("M2b control batch forbids provider calls")''')
    source=replace(source,'    watch_initialized=False','    watch_initialized=ready_watch')
    source=replace(source,'    model_watch=Watch(revision_keyed=revision_keyed)',
        '    model_watch=Watch([{"all":[{"field":"task_mismatch","op":"eq","value":1}]}] if ready_watch else [],revision_keyed=revision_keyed)')
    source=replace(source,'if policy in ("eh","hh600") and human.poll(public_features):reason="human_condition"',
        'if policy in ("eh","hh600") and now>=model_input["public_task"]["visible_since_tick"]+event_quiet_s and human.poll(public_features):reason="human_condition"')
    source=replace(source,'if now==0:reason="model_watch_bootstrap"','if now==0 and not ready_watch:reason="model_watch_bootstrap"')
    source=replace(source,'            "controller_version":4,','            "ready_watch":ready_watch,"event_quiet_s":event_quiet_s,"controller_version":4,')
    (frozen/"research/realtime/m2b_controller.py").write_text(source,encoding="utf-8")
    audit=(ROOT/"research/realtime/m2_audit.py").read_text(encoding="utf-8")
    audit=replace(audit,'    previous=[];keys=[];conditions=[];human_previous=False;human_key=None;initialized=False', '''    conditions=[{"all":[{"field":"task_mismatch","op":"eq","value":1}]}] if run["ready_watch"] else []
    previous=[False]*len(conditions);keys=[None]*len(conditions);human_previous=False;human_key=None;initialized=run["ready_watch"]''')
    audit=replace(audit,'            if run["policy"] in ("eh","hh600"):',
        '            if run["policy"] in ("eh","hh600") and now>=(published[-1]["tick"] if published else 0)+run["event_quiet_s"]:')
    audit=replace(audit,'if now==0:reason="model_watch_bootstrap"','if now==0 and not run["ready_watch"]:reason="model_watch_bootstrap"')
    (frozen/"research/realtime/m2b_audit.py").write_text(audit,encoding="utf-8")
    manifest=[{"path":f.relative_to(frozen).as_posix(),"bytes":f.stat().st_size,"sha256":sha(f)} for f in sorted(frozen.rglob("*")) if f.is_file()]
    save(stage/"source-manifest.json",manifest)
    for example in ("realtime_controlled","realtime_resource"):
        for action,extra in (("test",[]),("build",["--release"])):
            cargo=shutil.which("cargo") or str(Path.home()/".cargo/bin/cargo.exe")
            command=[cargo,action,"--manifest-path",str(frozen/"Cargo.toml"),"-p","harpoon-core","--example",example,"--locked","--target-dir",str(stage/"target"),*extra]
            proc=subprocess.run(command,cwd=frozen,capture_output=True,text=True,encoding="utf-8",errors="replace")
            (stage/f"{example}-{action}.log").write_text(proc.stdout+proc.stderr,encoding="utf-8")
            if proc.returncode:raise RuntimeError(f"{example}-{action} failed")
    save(stage/"prepare-validation.json",{"status":"passed","core_baseline":str(base),"source_files":len(manifest),
        "binaries":{name:sha(stage/f"target/release/examples/{name}.exe") for name in ("realtime_resource","realtime_controlled")}})
    print(json.dumps({"prepared":str(stage)}),flush=True)
if __name__=="__main__":main()

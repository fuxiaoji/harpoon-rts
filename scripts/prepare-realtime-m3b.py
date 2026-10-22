"""Freeze the audited M3a core and build a separate exact navigation service."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,x):p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')

def main():
    p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True)
    a=p.parse_args();stage=a.out.resolve();stage.mkdir(parents=True,exist_ok=False)
    base=Path('E:/CodexBuilds/harpoon-research-m3a-20261022-v5')
    manifest=json.loads((base/'artifact-manifest.json').read_text(encoding='utf-8'))
    checked=[]
    for r in manifest['files']:
        if r['path'].startswith('frozen-source/'):
            f=base/r['path'];assert sha(f)==r['sha256'];checked.append(r)
    frozen=stage/'frozen-source'
    shutil.copytree(base/'frozen-source',frozen,ignore=shutil.ignore_patterns('__pycache__'))
    for name in ['crates/harpoon-core/examples/realtime_nav_service.rs','research/realtime/real_nav.py',
                 'research/realtime/m3b-protocol.md','scripts/prepare-realtime-m3b.py']:
        (frozen/name).parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/name,frozen/name)
    cargo=shutil.which('cargo') or str(Path.home()/'.cargo/bin/cargo.exe')
    for action,extra in [('test',[]),('build',['--release'])]:
        cmd=[cargo,action,'--offline','--locked','--manifest-path',str(frozen/'Cargo.toml'),
             '-p','harpoon-core','--example','realtime_nav_service','--target-dir',str(stage/'target'),*extra]
        result=subprocess.run(cmd,cwd=frozen,capture_output=True,text=True,encoding='utf-8',errors='replace')
        (stage/f'nav-{action}.log').write_text(result.stdout+result.stderr,encoding='utf-8')
        if result.returncode:raise RuntimeError(f'nav {action} failed; log saved')
    binary=stage/'target/release/examples/realtime_nav_service.exe'
    core=[{'path':f.relative_to(frozen).as_posix(),'sha256':sha(f)}
          for f in sorted((frozen/'crates/harpoon-core/src').rglob('*.rs'))]
    save(stage/'prepare-validation.json',{'status':'passed','base':str(base),
          'base_frozen_files_verified':len(checked),'navigation_binary':str(binary),
          'navigation_binary_sha256':sha(binary),'unchanged_core_sources':core,
          'note':'Research task sources will be frozen separately before controls or calls.'})
    print(json.dumps({'stage':str(stage),'binary':str(binary),'core_files':len(core)}),flush=True)

if __name__=='__main__':main()

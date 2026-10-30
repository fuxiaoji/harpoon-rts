"""Freeze existing full naval engine/data and separate research-only adapters."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True)
    p.add_argument('--build',action='store_true');p.add_argument('--build-cache',type=Path);a=p.parse_args()
    stage=a.out.resolve();stage.mkdir(parents=True,exist_ok=False)
    frozen=stage/'frozen-source';frozen.mkdir()
    paths=[ROOT/'Cargo.toml',ROOT/'Cargo.lock']
    for name in ['crates','data/rules','database','scenarios','research/realtime']:
        paths.extend(x for x in (ROOT/name).rglob('*') if x.is_file()
                     and x.suffix in ('.rs','.toml','.json','.py','.md')
                     and '__pycache__' not in x.parts)
    paths.extend(ROOT/'scenarios'/n for n in ['csg_littoral.json','csg_grand_battle.json'])
    paths.append(Path(__file__).resolve())
    paths.extend((ROOT/'scripts').glob('*realtime*naval*.py'))
    records=[]
    for src in sorted(set(paths)):
        rel=src.relative_to(ROOT);dest=frozen/rel;dest.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(src,dest)
        assert sha(src)==sha(dest)
        records.append({'path':rel.as_posix(),'sha256':sha(dest),'bytes':dest.stat().st_size})
    (stage/'source-manifest.json').write_text(json.dumps({'status':'frozen',
        'source':'current workspace including disclosed uncommitted engineering changes',
        'files':records},ensure_ascii=False,indent=2),encoding='utf-8')
    if a.build:
        cargo=shutil.which('cargo') or str(Path.home()/'.cargo/bin/cargo.exe')
        cache=(a.build_cache or stage/'target').resolve()
        for action,extra in [('test',[]),('build',['--release'])]:
            command=[cargo,action,'--offline','--locked','--manifest-path',str(frozen/'Cargo.toml'),
                     '-p','harpoon-server','--example','realtime_naval_service',
                     '--target-dir',str(cache),*extra]
            result=subprocess.run(command,cwd=frozen,capture_output=True,text=True,encoding='utf-8',errors='replace')
            (stage/f'naval-{action}.log').write_text(result.stdout+result.stderr,encoding='utf-8')
            if result.returncode:raise RuntimeError(f'{action} failed; preserved log')
        binary=stage/'target/release/examples/realtime_naval_service.exe'
        built=cache/'release/examples/realtime_naval_service.exe'
        if built!=binary:
            binary.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(built,binary)
        (stage/'binary.json').write_text(json.dumps({'path':str(binary),'sha256':sha(binary)},indent=2),encoding='utf-8')
    print(json.dumps({'stage':str(stage),'frozen_files':len(records),'build':a.build}),flush=True)
if __name__=='__main__':main()

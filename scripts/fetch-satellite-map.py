"""Recover the frozen, attributed NASA regional image; never overwrite a changed asset."""
import hashlib
import json
import urllib.request
from pathlib import Path

folder=Path(__file__).resolve().parents[1]/'web/maps'
meta=json.loads((folder/'west-pacific-blue-marble.source.json').read_text(encoding='utf-8'))
target=folder/meta['file']
data=target.read_bytes() if target.exists() else urllib.request.urlopen(meta['request'],timeout=45).read()
if hashlib.sha256(data).hexdigest()!=meta['sha256'] or len(data)!=meta['bytes']:
    raise SystemExit('Image differs from the frozen source manifest; review the source before updating.')
if not target.exists(): target.write_bytes(data)
print(f'NASA image verified: {target} ({len(data)} bytes)')

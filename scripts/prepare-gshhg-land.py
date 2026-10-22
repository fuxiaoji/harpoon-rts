"""Derive a frozen regional game land mask from the official GSHHG binary ZIP.
Keep the immutable source archive and its LGPL/README beside the derived file.
"""
import argparse
import hashlib
import json
import struct
import zipfile
from pathlib import Path
from shapely.geometry import Polygon, box, mapping
from shapely.ops import unary_union

p=argparse.ArgumentParser();p.add_argument('archive',type=Path);p.add_argument('output',type=Path);p.add_argument('--bbox',type=float,nargs=4,default=[116,16,136,32]);a=p.parse_args()
w,s,e,north_limit=a.bbox
assert -180<=w<e<=180 and -90<=s<north_limit<=90,'invalid region bounds'
region=box(*a.bbox)
with zipfile.ZipFile(a.archive) as z:
    assert z.testzip() is None,'source ZIP CRC failure'
    names=z.namelist();member=next(n for n in names if n.endswith('gshhs_h.b'))
    raw=z.read(member)
    for name in names:
        if Path(name).name.lower() in ('readme.txt','license.txt') or Path(name).name.lower().startswith('copying'):
            (a.output.parent/('GSHHG-'+Path(name).name)).write_bytes(z.read(name))
levels={k:[] for k in range(1,5)};offset=0;selected=0
while offset<len(raw):
    ident,n,flag,west,east,south,north,area,area_full,container,ancestor=struct.unpack_from('>11i',raw,offset)
    offset+=44;size=n*8;data=memoryview(raw)[offset:offset+size];offset+=size
    assert n>=0 and offset<=len(raw),'invalid GSHHG record length'
    level=flag&255
    if level not in levels or west/1e6>e or east/1e6<w or south/1e6>north_limit or north/1e6<s:continue
    points=[((x/1e6 if x<=180000000 else x/1e6-360),y/1e6) for x,y in struct.iter_unpack('>ii',data)]
    if len(points)<3:continue
    g=Polygon(points)
    if not g.is_valid:g=g.buffer(0)
    g=g.intersection(region)
    if not g.is_empty:levels[level].append(g);selected+=1
assert offset==len(raw)
land=unary_union(levels[1])
for level in (2,3,4):
    rings=unary_union(levels[level]);land=land.union(rings) if level==3 else land.difference(rings)
land=land.simplify(0.001,preserve_topology=True)
assert not land.is_empty and land.is_valid
a.output.write_text(json.dumps(mapping(land),separators=(',',':'))+'\n',encoding='utf-8')
meta=dict(dataset='GSHHG',version='2.3.7',source='https://ftp.soest.hawaii.edu/gshhg/gshhg-bin-2.3.7.zip',
    license='LGPL; upstream license retained beside source',source_sha256=hashlib.sha256(a.archive.read_bytes()).hexdigest(),
    selected_member=member,bbox=a.bbox,selected_records=selected,
    generalization_degrees=0.001,preserve_topology=True,coordinate_system='WGS84',
    derived_sha256=hashlib.sha256(a.output.read_bytes()).hexdigest(),
    note='Regional generalized game land mask; not harbor navigation, bathymetry, elevation or a contemporary shoreline survey')
a.output.with_suffix('.source.json').write_text(json.dumps(meta,indent=2)+'\n',encoding='utf-8')
print(json.dumps(meta))

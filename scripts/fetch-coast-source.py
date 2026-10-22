"""Freeze Natural Earth 5.1.1 land for every authored western-Pacific theater.
Only the derived subset is rewritten. The original archive is hash checked.
"""
import argparse, hashlib, json, struct, urllib.request, zipfile
from pathlib import Path
from shapely.geometry import Polygon, box, mapping
from shapely.ops import unary_union
p=argparse.ArgumentParser();p.add_argument('folder',type=Path);args=p.parse_args();args.folder.mkdir(exist_ok=True)
url='https://naturalearth.s3.amazonaws.com/10m_physical/ne_10m_land.zip';archive=args.folder/'ne_10m_land-5.1.1.zip'
if not archive.exists():archive.write_bytes(urllib.request.urlopen(url,timeout=60).read())
digest=hashlib.sha256(archive.read_bytes()).hexdigest()
assert digest=='e547d749445eaa0964aba76738090ec88f5e63c4585122170f98c67a7ea922dc','source changed; review version before using'
raw=zipfile.ZipFile(archive).read('ne_10m_land.shp');off=100;items=[];region=box(116,10,150,34)
while off<len(raw):
    _,n=struct.unpack_from('>ii',raw,off);data=raw[off+8:off+8+n*2];off+=8+n*2
    if struct.unpack_from('<i',data)[0]!=5:continue
    x0,y0,x1,y1=struct.unpack_from('<dddd',data,4)
    if x1<116 or x0>150 or y1<10 or y0>34:continue
    parts,points=struct.unpack_from('<ii',data,36);ix=list(struct.unpack_from('<'+'i'*parts,data,44))+[points]
    xy=[struct.unpack_from('<dd',data,44+4*parts+16*j) for j in range(points)];shells=[];holes=[]
    for i in range(parts):
        ring=xy[ix[i]:ix[i+1]];poly=Polygon(ring)
        if not poly.is_valid:poly=poly.buffer(0)
        if poly.is_empty:continue
        signed=sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(ring,ring[1:]))
        (shells if signed<0 else holes).append(poly)
    for outer in shells:
        relevant=[h for h in holes if outer.covers(h.representative_point())]
        g=outer.difference(unary_union(relevant)).intersection(region)
        if not g.is_empty:items.append(g)
g=unary_union(items);dest=args.folder/'west-pacific-land.geojson';dest.write_text(json.dumps(mapping(g),separators=(',',':')),encoding='utf-8')
meta=dict(dataset='Natural Earth 1:10 million land',version='5.1.1',source=url,license='public domain',license_url='https://www.naturalearthdata.com/about/terms-of-use/',sha256=digest,bbox=[116,10,150,34],note='1:10 million cartographic scale, not 10 metre resolution; no bathymetry or elevation')
(args.folder/'land-source.json').write_text(json.dumps(meta,indent=2),encoding='utf-8');print(len(g.geoms),'land polygons;',dest.stat().st_size,'bytes')

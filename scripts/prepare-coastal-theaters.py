"""Build public coast charts in each theater's exact game projection.
Run with the project's Python 3.13 + Shapely. Original downloads are immutable.
"""
import json, math, hashlib, argparse, urllib.request
from pathlib import Path
from shapely.geometry import shape, box, Point
from shapely.ops import transform

ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser();parser.add_argument('--source',type=Path,required=True);args=parser.parse_args()
land=shape(json.loads(args.source.read_text(encoding='utf-8')))
db=json.loads((ROOT/'database/consolidated.json').read_text(encoding='utf-8'))
sc=json.loads((ROOT/'scenarios/csg_grand_battle.json').read_text(encoding='utf-8'))
coastal=json.loads(json.dumps(sc));coastal['id']='csg_littoral';coastal['name']='Coastal CSG Battle: Philippine Sea';coastal['description']='Three-hour fictional CSG game east of Taiwan. Same combat forces and 160nm separation as csg_grand_battle; every initial position and mission point translated +100nm east/+100nm north on the 118E/18N,24N chart. Charted land blocks movement; navigation routes avoid islands. Satellite imagery is an August 2004 composite, not live intelligence. Weighted kills: ship10/sub5/air1.'
coastal['context']['theater']={'west':118,'east':134,'south':18,'north':30,'reference_lat':24,'image':'maps/west-pacific-blue-marble.jpg','attribution':'NASA GIBS / MODIS / NASA Earth Observatory','source_url':'https://gibs.earthdata.nasa.gov/layer-metadata/v1.0/BlueMarble_NextGeneration.json','imagery_date':'2004-08'}
ford=coastal['context']['air_wings']['CVN-78 FORD']
for fit in ford:
    if fit['class']=='us_f18e':fit['count']-=2
ford.append({'class':'us_ea18g','count':2})
coastal['description']+=' Coastal variant replaces two Ford F/A-18E with two EA-18G ALQ-99J fits to exercise electronic support; total ready aircraft count is unchanged. J-15D and its pod fit retain source-estimated status.'
def shift(p):
    if isinstance(p,list):return [p[0]+100,p[1]+100]
    return {'x':p['x']+100,'y':p['y']+100}
for u in coastal['units']:
    u['pos']=shift(u['pos'])
    if u.get('waypoints'):u['waypoints']=[shift(p) for p in u['waypoints']]
for m in coastal['context'].get('air_missions',{}).values():
    for key in ['station','area','point']:
        if key in m:m[key]=shift(m[key])
(ROOT/'scenarios/csg_littoral.json').write_text(json.dumps(coastal,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
catalog={}
for path in sorted((ROOT/'scenarios').glob('*.json')):
    name=path.stem;s=json.loads(path.read_text(encoding='utf-8'));t=s.get('context',{}).get('theater')
    if not t:continue
    if name in ['csg_grand_battle','csg_littoral']:s['context']['radar_environment']={'sea_state':3,'precipitation':'None'}
    bbox=[t['west'],t['south'],t['east'],t['north']];key=','.join(f'{v:g}' for v in bbox)
    if bbox!=[118,18,134,30]:
        asset='blue-marble-'+key.replace(',','-')+'.jpg';target=ROOT/'web/maps'/asset
        if name=='csg_grand_battle':asset='csg-126-138-blue-marble.jpg';target=ROOT/'web/maps'/asset
        url=f'https://gibs.earthdata.nasa.gov/wms/epsg4326/best/wms.cgi?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=BlueMarble_NextGeneration&STYLES=&SRS=EPSG:4326&BBOX={key}&WIDTH=2400&HEIGHT=1600&FORMAT=image/jpeg'
        if not target.exists():target.write_bytes(urllib.request.urlopen(url,timeout=60).read())
        t['image']='maps/'+asset
        meta=dict(dataset='NASA Blue Marble Next Generation',imagery_date='2004-08',request=url,bbox_west_south_east_north=bbox,file=asset,sha256=hashlib.sha256(target.read_bytes()).hexdigest(),bytes=target.stat().st_size,purpose='Correctly georeferenced game theater basemap',attribution=t['attribution'])
        (ROOT/'web/maps'/Path(asset).with_suffix('.source.json')).write_text(json.dumps(meta,indent=2)+'\n',encoding='utf-8')
    else:t['image']='maps/west-pacific-blue-marble.jpg'
    catalog[key]=t['image']
    clipped=land.intersection(box(t['west']-2,t['south']-2,t['east']+2,t['north']+2))
    project=lambda x,y,z=None:((x-t['west'])*60*math.cos(math.radians(t['reference_lat'])),(y-t['south'])*60)
    local=transform(project,clipped)
    ships=[u for u in s['units'] if db['platforms'][u['class']]['kind'] in ['Ship','Sub']]
    positions=[tuple(u['pos']) if isinstance(u['pos'],list) else (u['pos']['x'],u['pos']['y']) for u in ships]
    if any(local.covers(Point(x,y)) for x,y in positions):
        candidates=sorted(((x,y) for x in range(-80,81,2) for y in range(-80,81,2)),key=lambda p:(p[0]**2+p[1]**2,p[0],p[1]))
        adjustment=next(((dx,dy) for dx,dy in candidates if all(not local.covers(Point(x+dx,y+dy)) for x,y in positions)),None)
        if adjustment is None:raise RuntimeError(name+': no common sea-safe deployment shift')
        dx,dy=adjustment
        def move(p):return [p[0]+dx,p[1]+dy] if isinstance(p,list) else {'x':p['x']+dx,'y':p['y']+dy}
        for u in s['units']:
            u['pos']=move(u['pos'])
            if u.get('waypoints'):u['waypoints']=[move(p) for p in u['waypoints']]
        for m in s['context'].get('air_missions',{}).values():
            for key in ['station','area','point']:
                if key in m:m[key]=move(m[key])
        if s['context'].get('area_denial'):s['context']['area_denial']['center']=move(s['context']['area_denial']['center'])
        s['description']+=f' Coastline legality correction (2026-10-20): all forces, routes and mission points translated together {dx}nm east/{dy}nm north; relative geometry unchanged.'
        print(name,'coastline legality correction',adjustment)
    polygons=[]
    for p in ([local] if local.geom_type=='Polygon' else local.geoms):
        if p.geom_type!='Polygon' or p.is_empty:continue
        polygons.append(dict(outer=[{'x':round(x,6),'y':round(y,6)} for x,y in p.exterior.coords],holes=[[{'x':round(x,6),'y':round(y,6)} for x,y in r.coords] for r in p.interiors]))
    s['context']['terrain']=dict(source='Natural Earth land 5.1.1, 1:10 million, public domain; chart guard only, no bathymetry/elevation',polygons=polygons)
    for u in s['units']:
        if db['platforms'][u['class']]['kind'] not in ['Ship','Sub']:continue
        p=u['pos'];x,y=p if isinstance(p,list) else (p['x'],p['y'])
        if local.covers(Point(x,y)):raise RuntimeError(f'{name}: land spawn {u.get("name")}')
    path.write_text(json.dumps(s,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(name,len(polygons),'polygons; all ship/sub spawns at sea; image',t['image'])
(ROOT/'web/basemap-index.js').write_text('/* Generated from frozen NASA source manifests; geographic display only. */\n(function(r){const maps='+json.dumps(catalog,separators=(',',':'))+';r.HarpoonBasemaps={path(t){if(!t||t.synthetic)return null;const key=[t.west,t.south,t.east,t.north].join(",");return maps[key]||((t.image||"").endsWith("west-pacific-blue-marble.jpg")?null:t.image);}};})(typeof window!=="undefined"?window:globalThis);\n',encoding='utf-8')

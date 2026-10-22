"""Author a fictional exercise on a real regional background, never a real OOB.

Only writes the new scenario and explicitly prefixed exercise asset overlays.
No real military facility layouts, basing, readiness or attack routes are used.
"""
import argparse
import copy
import hashlib
import json
import math
from pathlib import Path
from shapely.geometry import Point, Polygon, LineString, shape
from shapely.ops import unary_union, transform
from shapely.prepared import prep

ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--land',type=Path,required=True);p.add_argument('--hot-start',action='store_true');p.add_argument('--joint',action='store_true');a=p.parse_args()
if a.joint:a.hot_start=True
original=json.loads((ROOT/'scenarios/csg_littoral.json').read_text(encoding='utf-8'))
theater=copy.deepcopy(original['context']['theater'])
theater.update(north=32,reference_lat=25,image='maps/island-118-18-134-32-blue-marble.jpg')
if a.hot_start:
    theater.update(west=114,south=16,east=138,north=34,image='maps/island-wide-114-16-138-34-blue-marble.jpg',tile_layer='eox-s2cloudless-2016')
def xy(lon,lat):return ((lon-theater['west'])*60*math.cos(math.radians(theater['reference_lat'])),(lat-theater['south'])*60)
terrain=copy.deepcopy(original['context']['terrain'])
if a.land:
    theater['attribution']+=' | Coast: GSHHG 2.3.7 LGPL, generalized 0.001 degrees'
    geographic=shape(json.loads(a.land.read_text(encoding='utf-8')))
    land=transform(lambda lon,lat,z=None:xy(lon,lat),geographic)
    terrain={'source':'GSHHG 2.3.7 high shoreline; LGPL; regional game generalization only, no bathymetry/elevation',
        'polygons':[{'outer':[dict(x=round(x,6),y=round(y,6)) for x,y in g.exterior.coords],
            'holes':[[dict(x=round(x,6),y=round(y,6)) for x,y in ring.coords] for ring in g.interiors]}
            for g in ([land] if land.geom_type=='Polygon' else land.geoms) if g.geom_type=='Polygon' and not g.is_empty]}
land=unary_union([Polygon([(q['x'],q['y']) for q in g['outer']],
    [[(q['x'],q['y']) for q in ring] for ring in g.get('holes',[])]) for g in terrain['polygons']])
prepared=prep(land)
units=[];wings={};missions={};used=[];departures={};naval_groups=[]

def sea(point,clearance=0.2):
    return not prepared.covers(Point(point)) and land.distance(Point(point))>=clearance
def choose(point,on_land,spacing=0.0):
    for radius in [0]+[n*0.5 for n in range(1,101)]:
        for deg in range(0,360,10):
            candidate=(point[0]+math.sin(math.radians(deg))*radius,point[1]+math.cos(math.radians(deg))*radius)
            valid=prepared.contains(Point(candidate)) if on_land else sea(candidate)
            if valid and all(math.dist(candidate,q)>=spacing for q in used):
                return candidate
    raise ValueError('no suitable chart point near fictional deployment')
def add(side,cls,name,pos,speed=0,course=90,**extra):
    units.append(dict(side=side,**{'class':cls},name=name,pos=[round(pos[0],6),round(pos[1],6)],speed_kts=speed,course_deg=course,fire_on=False,**extra))
    if cls not in ('exercise_airbase','exercise_port','exercise_cruise_battery'):used.append(pos)

db=json.loads((ROOT/'database/consolidated.json').read_text(encoding='utf-8'))
for f in sorted((ROOT/'database/volumes').glob('*.json')):
    if not f.name.startswith('_') and f.name!='scenario_exercise_assets.json':
        v=json.loads(f.read_text(encoding='utf-8'))
        for key in ('platforms','weapons','sensors'):db[key].update(v.get(key,{}))
assets={};provenance=[]
for new,parent,label in [('exercise_airbase','airfield','Fictional Air Station (generic RTS airfield proxy)'),
    ('exercise_port','navsta','Fictional Port Complex (generic RTS installation proxy)'),
    ('exercise_cruise_battery','navsta','Fictional Rocket Force Cruise Battery (exercise loadout)')]:
    v=copy.deepcopy(db['platforms'][parent]);v.update(id=new,name=label)
    if new=='exercise_airbase':v['deck']['capacity']=24
    if new=='exercise_cruise_battery':
        v['weapons']=[dict(weapon='cn_dh10',mounts=1,ammo=32)];v['sensors']=['esm_std'];v['deck']=None
    assets[new]=v
    provenance.append(dict(id=new,parent=parent,parent_source='database/consolidated.json',
        assumption='Generic existing RTS installation proxy; no real facility layout, DP or force strength asserted',
        changes=('Fictional 24-aircraft capacity; inherited RTS cycle, HP and defences' if new=='exercise_airbase' else 'Identity only' if new!='exercise_cruise_battery' else 'Fictional 32-round DH-10 exercise loadout; weapon performance inherited unchanged')))
(ROOT/'database/volumes/scenario_exercise_assets.json').write_text(json.dumps(dict(platforms=assets,weapons={},sensors={}),indent=2)+'\n',encoding='utf-8')
(ROOT/'database/volumes/_exercise_asset_sources.json').write_text(json.dumps(dict(version='2026-10-21',assets=provenance,
    weapon_source='Existing cn_dh10 in consolidated.json; no new ballistic-missile model or parameters'),indent=2)+'\n',encoding='utf-8')

# Broad regional points are author choices, not actual military sites.
blue_air=[('BLUE WEST AIR STATION',(119.15,25.6),[('cn_j11b',16),('cn_jh7a',8)]),
    ('BLUE COAST AIR STATION',(119.4,26.2),[('cn_j11b',12),('cn_jh7a',12)]),
    ('BLUE BOMBER AIR STATION',(118.8,25.0),[('cn_j11b',6),('cn_h6j',18)]),
    ('BLUE SUPPORT AIR STATION',(118.6,25.4),[('cn_y8q',6),('cn_z18j',6),('cn_j15d',6),('cn_jh7a',6)]),
    ('BLUE ZHEJIANG SECTOR AIR STATION',(120.4,29.4),[('cn_h6j',12),('cn_j11b',8),('cn_z18j',4)]),
    ('BLUE SHANGHAI SECTOR AIR STATION',(121.1,31.2),[('cn_j11b',12),('cn_jh7a',8),('cn_j15d',4)])]
for name,geo,wing in blue_air:
    point=choose(xy(*geo),True);add('Blue','exercise_airbase',name,point)
    wings[name]=[dict(**{'class':c},count=n) for c,n in wing]
for i,geo in enumerate([(118.9,25.5),(119.2,26.0),(118.7,24.9),(120.7,30.4),(120.1,29.5),(121.0,31.2)],1):
    add('Blue','exercise_cruise_battery',f'BLUE ROCKET GROUP {i}',choose(xy(*geo),True))

blue_centers=[choose(xy(125.8,27.8),False),choose(xy(125.5,23.4),False)]
old_blue=[u for u in original['units'] if u['side']=='Blue' and u['class'] in ('cn_001','cn_002')]
assert len(old_blue)==2
for k,(parent,center) in enumerate(zip(old_blue,blue_centers),1):
    name=f'BLUE CARRIER {k}';add('Blue',parent['class'],name,center,speed=12)
    wings[name]=copy.deepcopy(original['context']['air_wings'][parent['name']])
    for j,cls in enumerate(['cn_055']*3+['cn_052d']*3+['cn_054a']*2):
        if cls not in db['platforms']:raise ValueError(cls)
        angle=(j*45+k*10)*math.pi/180;point=choose((center[0]+math.cos(angle)*9,center[1]+math.sin(angle)*9),False,1.0)
        add('Blue',cls,f'BLUE ESCORT {k}-{j+1}',point,speed=12)
    naval_groups.append(dict(side='Blue',name=f'BLUE CARRIER GROUP {k}',leader=name,members=[name]+[f'BLUE ESCORT {k}-{j+1}' for j in range(8)]))

for label,geo in [('NORTH',(125.6,30.2)),('SOUTH',(124.5,21.5))]:
    center=choose(xy(*geo),False);members=[]
    for j,cls in enumerate(['cn_055','cn_052d','cn_052d','cn_054a']):
        name=f'BLUE {label} SURFACE {j+1}';members.append(name)
        point=choose((center[0]+j*3,center[1]+j%2*3),False,1.0)
        route=choose((point[0]+12,point[1]),False)
        assert not land.intersects(LineString([point,route])), 'fictional surface patrol leg must remain at sea'
        add('Blue',cls,name,point,speed=12,waypoints=[list(route)])
    naval_groups.append(dict(side='Blue',name=f'BLUE {label} SURFACE GROUP',leader=members[0],members=members))

port_names=[];red_carriers=[]
moored_escorts=[]
for k,geo in enumerate([(127.9,26.55),(129.6,28.3)],1):
    land_point=choose(xy(*geo),True);port=f'RED FICTIONAL PORT {k}';port_names.append(port)
    add('Red','exercise_port',port,land_point)
    berth=choose(land_point,False,1.0)
    carrier=f'RED CARRIER {k}';red_carriers.append(carrier)
    parent=next(u for u in original['units'] if u['side']=='Red' and u['class']==('us_ford' if k==1 else 'us_nimitz'))
    direction=math.atan2(berth[0]-land_point[0],berth[1]-land_point[1]);course=math.degrees(direction)%360
    exit_point=(berth[0]+math.sin(direction)*12,berth[1]+math.cos(direction)*12)
    if land.intersects(LineString([berth,exit_point])):
        exit_point=next((q for d in range(0,360,10) if sea(q:=(berth[0]+math.sin(math.radians(d))*8,berth[1]+math.cos(math.radians(d))*8))
            and not land.intersects(LineString([berth,q]))),berth)
    add('Red',parent['class'],carrier,berth,course=course,waypoints=[list(exit_point)])
    departures[carrier]=list(exit_point)
    wings[carrier]=copy.deepcopy(original['context']['air_wings'][parent['name']])
    for j,cls in enumerate(['us_ticonderoga']*2+['us_burke_2a']*3+['us_burke_3']):
        point=choose((berth[0]+(j%3-1)*2,berth[1]+(j//3-0.5)*3),False,0.7)
        add('Red',cls,f'RED MOORED ESCORT {k}-{j+1}',point,course=course)
        moored_escorts.append(f'RED MOORED ESCORT {k}-{j+1}')
    naval_groups.append(dict(side='Red',name=f'RED PORT GROUP {k}',leader=carrier,members=[carrier]+[f'RED MOORED ESCORT {k}-{j+1}' for j in range(6)]))
    station=choose((berth[0]-25,berth[1]+5),False)
    for j,cls in enumerate(['us_e2d','us_f18e','us_f18e']):
        name=f'RED ROUTINE {k}-{j+1}'
        add('Red',cls,name,(station[0],station[1]+j*3),speed=250,course=270,altitude_m=3000)
        missions[name]=dict(mission='recon',point=dict(x=station[0],y=station[1])) if j==0 else dict(mission='cap',station=dict(x=station[0],y=station[1]),radius_nm=10,follow=False)

red_air=[]
for k,geo in enumerate([(127.95,26.6),(129.65,28.25),(124.2,24.4)],1):
    name=f'RED FICTIONAL AIR STATION {k}';red_air.append(name)
    add('Red','exercise_airbase',name,choose(xy(*geo),True))
    wings[name]=[dict(**{'class':'us_f18e'},count=6),dict(**{'class':'us_f18e_asm'},count=4),dict(**{'class':'us_e2d'},count=2)]

sub_specs={'Blue':[(125,25.8),(126,23),(128,22),(128,24),(124.4,28.7),(126.5,30.2),(130.8,22.5),(132,24.5)],
    'Red':[(130,24),(129,23),(129,27),(131,26),(132.5,28),(132,30)]}
for side,positions in sub_specs.items():
    hostile=[u['pos'] for u in units if u['side']!=side and db['platforms'].get(u['class'],{}).get('kind')=='Ship']
    for i,geo in enumerate(positions,1):
        point=choose(xy(*geo),False,2.0)
        assert min(math.dist(point,q) for q in hostile)>=80,'submarine initial separation must be at least 80nm'
        add(side,'cn_093a' if side=='Blue' else 'us_virginia',f'{side.upper()} OUTER SUB {i}',point,course=90 if side=='Blue' else 270,depth_m=150)

protected=[dict(name=n,group='carrier',points=30) for n in red_carriers]
for group in naval_groups:
    if group['side']!='Blue':continue
    leader=next(u for u in units if u['name']==group['leader'])
    for u in units:
        if u['name'] in group['members'] and u['name']!=leader['name']:
            dx=u['pos'][0]-leader['pos'][0];dy=u['pos'][1]-leader['pos'][1]
            u.pop('waypoints',None)
            u['follow']=dict(leader_name=leader['name'],bearing_rel_deg=(math.degrees(math.atan2(dx,dy))-leader['course_deg'])%360,distance_nm=math.hypot(dx,dy))
protected += [dict(name=n,group='infrastructure',points=8) for n in port_names+red_air]
counter=[dict(name=f'BLUE CARRIER {k}',group='counter',points=7) for k in (1,2)]
counter += [dict(name=n,group='counter',points=1) for n,_,_ in blue_air]
description=('Fictional six-hour island exercise on a real Taiwan/Ryukyu regional basemap. All bases, ports, deployments, force sizes and alert conditions are invented game assumptions, not real sites or readiness. '
    'Blue has two carrier groups plus two independent surface groups on separate patrol sectors, eight nuclear submarines, six fictional shore air stations including Zhejiang/Shanghai sector support, 144 shore aircraft including30 H-6J naval bomber proxies, and six generic Rocket Force cruise batteries with192 rounds. Red has six distant nuclear submarines. These are authored exercise forces, not a real OOB or a calculated attack requirement. H-6K/N and ballistic missiles are not modeled. Red has two initially stationary carrier groups at fictional anchorages, three air stations, two port complexes and routine airborne patrols. '
    'Only Red active orders are locked until its own sensors observe a hostile missile or a hostile hit confirms attack; routine flight/sensors and system self-defence continue. At each Red air host, two AAM-capable aircraft are immediately ready; the rest finish an authored360-second initial preparation interval, not a real readiness estimate. Blue receives one initial targeting-quality snapshot of all moored ships and the named facilities, not continuous satellite truth. BeiDou is navigation/timing background, not an imaging sensor. '
    'Blue scores carrier damage (60 maximum) and installation damage (40); Red scores preserved value plus up to20 counterstrike points. Blue must earn at least30 carrier and20 infrastructure points and exceed Red at six hours; otherwise Red wins. Aircraft kills do not directly score. '
    'Air stations and ports use labelled generic RTS proxies; no detailed runway, harbor bathymetry, ballistic missiles, satellite orbits or real facility layouts are modeled. Submarines start at least80nm from hostile ships. Shore collision uses charted polygons only.')
scenario=dict(id='taiwan_island_exercise',name='台海题材·东岛警报（虚构设施演习）',description=description,duration_s=21600,seed=20261021,
    weather=dict(visibility_nm=40,jam=[0,0]),day_night='Day',victory=dict(target_vp=0,kill_count_at_end=False),
    context=dict(theater=theater,terrain=terrain,radar_environment=dict(sea_state=3,precipitation='None'),air_wings=wings,air_missions=missions,
        island_exercise=dict(locked_side='Red',briefing_side='Blue',briefing_targets=red_carriers+moored_escorts+port_names+red_air,
            protected_targets=protected,counter_targets=counter,required_carrier_points=30,required_infrastructure_points=20,
            naval_groups=naval_groups,defender_air_preparation_s=360)),units=units)
for u in units:
    if u['class'] in assets:assert prepared.covers(Point(u['pos'])),u['name']
    elif db['platforms'][u['class']]['kind'] in ('Ship','Sub'):assert not prepared.covers(Point(u['pos'])),u['name']
if a.hot_start:
    scenario.update(id='taiwan_island_opening',name='台海题材·东岛警报：空中开场（虚构设施演习）')
    opening=[]
    for k,home in enumerate([blue_air[2][0],blue_air[4][0]]):
        for j in range(2):opening.append(dict(home=home,**{'class':'cn_h6j'},count=6,target=red_carriers[k],progress=0.35+j*0.10,altitude_m=50,escort=False))
    for k,home in enumerate([blue_air[0][0],blue_air[1][0]]):
        opening.append(dict(home=home,**{'class':'cn_jh7a'},count=8,target=red_carriers[k],progress=0.40,altitude_m=50,escort=False))
        opening.append(dict(home=home,**{'class':'cn_j11b'},count=8,target=red_carriers[k],progress=0.35,altitude_m=1500,escort=True))
    for k in (1,2):
        strike=next(w['class'] for w in wings[f'BLUE CARRIER {k}'] if w['count']>=4 and any(db['weapons'][m['weapon']]['kind']=='Ascm' for m in db['platforms'][w['class']]['weapons']))
        opening.append(dict(home=f'BLUE CARRIER {k}',**{'class':strike},count=4,target=red_carriers[k-1],progress=0.25,altitude_m=50,escort=False))
    salvos=[dict(launcher=f'BLUE ROCKET GROUP {i+1}',weapon='cn_dh10',count=8,target=(port_names+red_air)[i%5],progress=0.30+i*0.025) for i in range(6)]
    salvos += [dict(launcher=f'BLUE ESCORT 1-{k}',weapon='cn_yj18',count=8,target=red_carriers[0],progress=0.30) for k in (1,2)]
    scenario['context']['island_exercise'].update(opening_flights=opening,opening_salvos=salvos)
    scenario['description']+=' Hot opening: 64 Blue aircraft (48 strike and16 escort) and64 missiles are already airborne, representing authored pre-game launches. Aircraft and missiles are deducted from the above inventories, and elapsed flight fuel/time are charged. These counts and positions are game author choices, not a reconstruction or an attack recommendation.'
if a.joint:
    scenario.update(id='island_joint_exercise',name='冲绳地域背景·东岛联合演习（虚构设施部署）')
    groups=[]
    for side in ('Blue','Red'):
        buckets={}
        for u in units:
            if u['side']!=side:continue
            role='shore_missile' if u['class']=='exercise_cruise_battery' else 'shore_aviation' if u['class']=='exercise_airbase' else 'infrastructure' if u['class']=='exercise_port' else 'patrol' if db['platforms'][u['class']]['kind']=='Aircraft' else 'naval'
            buckets.setdefault(role,[]).append(u['name'])
        for role,members in buckets.items():groups.append(dict(side=side,name=f'{side.upper()} {role.upper()}',role=role,members=members))
    scenario['context']['island_exercise']['joint_groups']=groups
    scenario['description']+=' Joint service groups expose friendly task ownership for shore cruise batteries, shore aviation and naval forces. Their control shares ammunition accounting, replacement reserves and observed-feedback reviews. It is a fictional game exercise, not a model of actual Okinawa facilities or a real joint-strike plan. The opening is authored; later launches and combat remain ordinary engine actions. No extra detection/damage bonus or automatic force replenishment.'
dest=ROOT/f"scenarios/{scenario['id']}.json";dest.write_text(json.dumps(scenario,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
chart=dict(source=terrain['source'],scenario_id=scenario['id'],polygons=[dict(
    bounds=[min(p['x'] for p in g['outer']),min(p['y'] for p in g['outer']),max(p['x'] for p in g['outer']),max(p['y'] for p in g['outer'])],
    rings=[[[p['x'],p['y']] for p in ring] for ring in [g['outer'],*g.get('holes',[])]]) for g in terrain['polygons']])
if not a.joint:(ROOT/('web/maps/taiwan-island-opening-land.json' if a.hot_start else 'web/maps/taiwan-island-land.json')).write_text(json.dumps(chart,separators=(',',':'))+'\n',encoding='utf-8')
manifest=dict(version='2026-10-21',scenario=dest.name,sha256=hashlib.sha256(dest.read_bytes()).hexdigest(),
    fictional_facilities=True,real_deployments_used=False,imagery=theater,terrain_source=terrain['source'],
    minimum_submarine_separation_nm=80,departure_waypoints=departures,asset_sources='database/volumes/_exercise_asset_sources.json',
    boundaries=['Generic Base HP and deck cycles are inherited RTS exercise proxies','No ballistic missile or satellite imaging model','Initial briefing snapshot is not live enemy truth'])
manifest.update(author_assumptions=dict(defender_air_preparation_s=360,immediate_cap_per_host=2,shore_aircraft=144,cruise_rounds=192),naval_groups=naval_groups)
if a.joint:manifest['joint_groups']=groups;manifest['coast_chart']='web/maps/taiwan-island-opening-land.json (same frozen geometry/projection)'
(ROOT/('docs/island-joint-provenance.json' if a.joint else 'docs/island-opening-provenance.json' if a.hot_start else 'docs/island-exercise-provenance.json')).write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(dict(scenario=str(dest),initial_units=len(units),air_hosts=len(wings),terrain=terrain['source']),ensure_ascii=False))

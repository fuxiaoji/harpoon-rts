"""Generate the fictional exercise OOB; never changes volume platform values."""
import json
import copy
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# A separate, cited mission fit; keep the source volume's bomb-equipped entry intact.
us_volume=json.loads((ROOT/'database/volumes/us_aircraft.json').read_text(encoding='utf-8'))
anti_ship=copy.deepcopy(us_volume['platforms']['us_f18e'])
anti_ship['id']='us_f18e_asm'
anti_ship['name']='F/A-18E/F Super Hornet (anti-ship fit)'
anti_ship['weapons']=[dict(weapon='us_aim9x',mounts=1,ammo=2),dict(weapon='us_aim120',mounts=1,ammo=2),dict(weapon='us_agm84',mounts=1,ammo=2)]
(ROOT/'database/volumes/us_aircraft_mission_fits.json').write_text(json.dumps(dict(platforms={'us_f18e_asm':anti_ship},weapons={},sensors={}),indent=2)+'\n',encoding='utf-8')
units = []
def spawn(side, cls, name, x, y, **extra):
    unit = dict(side=side, **{'class': cls}, name=name, pos=[x,y], course_deg=90 if side=='Blue' else 270, speed_kts=18, fire_on=side=='Red')
    unit.update(extra)
    units.append(unit)

# All stations, routes, group composition and availability are scenario assumptions.
for number, cls, label, x, y in [(16,'cn_001','LIAONING',310,350),(17,'cn_002','SHANDONG',325,270)]:
    carrier = f'CV-{number} {label}'
    spawn('Blue',cls,carrier,x,y,waypoints=[[x+10,y+5],[x+12,y-5],[x,y]])
    for suffix, ship, dx, dy, bearing in [('CG','cn_055',18,0,0),('DDG-A','cn_052d',8,12,300),('DDG-B','cn_052d',8,-12,60),('FFG','cn_054a',-8,-6,150)]:
        spawn('Blue',ship,f'{label} {suffix}',x+dx,y+dy,follow=dict(leader_name=carrier,bearing_rel_deg=bearing,distance_nm=18 if suffix=='CG' else 14))
    spawn('Blue','cn_093a',f'{label} SSN',x+45,y-15,speed_kts=6,depth_m=150,waypoints=[[x+50,y+10],[x+35,y-10]])
    spawn('Blue','cn_z18j',f'{label} AEW',x+40,y+5,speed_kts=130,altitude_m=2800,waypoints=[[x+45,y+15],[x+55,y-15],[x+30,y-5]])
    for n in range(2):
        spawn('Blue','cn_j15',f'{label} CAP-{n+1}',x+30+n*2,y+5+n*2,speed_kts=485,altitude_m=8000,fire_on=False,waypoints=[[x+45,y+20],[x+45,y-20],[x+20,y]])

for number, cls, label, x, y in [(78,'us_ford','FORD',460,350),(68,'us_nimitz','NIMITZ',475,275)]:
    carrier=f'CVN-{number} {label}'
    spawn('Red',cls,carrier,x,y,speed_kts=24,waypoints=[[390,320],[387,323],[392,318]])
    for suffix, ship, dx, dy, bearing in [('CG','us_ticonderoga',-18,0,0),('DDG-A','us_burke_2a',-8,12,60),('DDG-B','us_burke_3',-8,-12,300)]:
        spawn('Red',ship,f'{label} {suffix}',x+dx,y+dy,speed_kts=24,follow=dict(leader_name=carrier,bearing_rel_deg=bearing,distance_nm=18 if suffix=='CG' else 14))
    spawn('Red','us_virginia',f'{label} SSN',x-40,y-15,speed_kts=7,depth_m=160,waypoints=[[x-65,y],[x-55,y-20]])
    # Forward AEW stations are pre-briefed exercise deployments, not spawned contacts.
    spawn('Red','us_e2d',f'{label} AEW',365 if label=='FORD' else 380,y,speed_kts=255,altitude_m=9000)
    for n in range(2):
        spawn('Red','us_f18e',f'{label} CAP-{n+1}',x-30-n*2,y+5+n*2,speed_kts=480,altitude_m=8000,waypoints=[[x-45,y+20],[x-45,y-20],[x-20,y]])

wing=lambda **classes:[dict(**{'class':cls},count=count) for cls,count in classes.items()]
air_missions={}
for unit in units:
    if ' AEW' in unit['name'] or ' CAP-' in unit['name']:
        air_missions[unit['name']]=dict(mission='cap',station=unit['pos'],radius_nm=15 if 'CAP' in unit['name'] else 8)
        unit.pop('waypoints',None)
context=dict(
    theater=dict(west=118,east=134,south=18,north=30,reference_lat=24,image='maps/west-pacific-blue-marble.jpg',attribution='NASA GIBS / MODIS / NASA Earth Observatory',source_url='https://gibs.earthdata.nasa.gov/layer-metadata/v1.0/BlueMarble_NextGeneration.json',imagery_date='2004-08'),
    area_denial=dict(defender='Blue',center=[390,320],radius_nm=45,hold_s=180,functional_hp_fraction=0.5,protected_names=['CV-16 LIAONING','CV-17 SHANDONG'],intruder_names=['CVN-78 FORD','CVN-68 NIMITZ']),
    air_wings={
        'CV-16 LIAONING':wing(cn_j15=22,cn_z18f=4,cn_z18j=3,cn_z9c=2),
        'CV-17 SHANDONG':wing(cn_j15=22,cn_j15d=2,cn_z18f=4,cn_z18j=3,cn_z9c=2),
        'CVN-78 FORD':wing(us_f18e=24,us_f18e_asm=8,us_f35c=10,us_e2d=3,us_mh60r=5),
        'CVN-68 NIMITZ':wing(us_f18e=24,us_f18e_asm=8,us_e2d=3,us_mh60r=5)},
    air_missions=air_missions,automated_side='Red')
scenario=dict(id='dual_carrier_denial',name='Twin Shields: Dual-carrier Area Denial',description='Fictional two-hour exercise: Liaoning and Shandong defend a 45-nm circle against Ford and Nimitz. US carriers must hold it for 180 continuous seconds. China must deny entry and retain both carriers above 50% hull HP. RTS adaptations and database estimates apply.',context=context,duration_s=7200,weather=dict(visibility_nm=16,jam=[0,0],overcast=0.2,rain=0),day_night='Day',seed=20261019,victory=dict(target_vp=0),units=units)
path=ROOT/'scenarios/dual_carrier_denial.json'
path.write_text(json.dumps(scenario,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(f'{path}: {len(units)} initial units; four carrier groups')

# A separate hotseat lesson: neither side has an automatic staff. Carrier
# movement is deliberately left to the two human-controlled command seats.
lesson=copy.deepcopy(scenario)
lesson.update(id='dual_carrier_hotseat',name='Twin Shields: Six-hour Hotseat Exercise',
    description='Fictional six-hour hotseat exercise. Both sides are manually commanded; no automated staff. Carriers start holding position. Existing patrol, formation, self-defense and standing engagement orders still execute.',
    duration_s=21600)
lesson['context']['automated_side']=None
for unit in lesson['units']:
    if unit['name'] in context['area_denial']['protected_names']+context['area_denial']['intruder_names']:
        unit['speed_kts']=0
        unit.pop('waypoints',None)
    # Both seats explicitly authorize their own automatic target selection.
    unit['fire_on']=False
lesson_path=ROOT/'scenarios/dual_carrier_hotseat.json'
lesson_path.write_text(json.dumps(lesson,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(f'{lesson_path}: six hours; both command seats manual; carriers hold')

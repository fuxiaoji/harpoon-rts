from pathlib import Path
import json
repo=Path(__file__).resolve().parents[1]
tables={
'pre1955':[(30,'weapon',True),(45,'sensor',False),(50,'cic',False),(60,'engineering',True),(75,'flooding',True),(90,'fire',False),(95,'bridge',True),(100,'rudder',True)],
'surface':[(20,'weapon',False),(35,'sensor',False),(45,'cic',False),(60,'engineering',True),(75,'flooding',True),(90,'fire',False),(95,'bridge',False),(100,'rudder',False)],
'missile':[(10,'weapon',False),(25,'sensor',False),(40,'cic',False),(65,'engineering',True),(80,'flooding',True),(95,'fire',False),(100,'bridge',False)],
'small_combatant':[(20,'weapon',False),(30,'sensor',False),(35,'cic',False),(50,'engineering',True),(60,'personnel',False),(80,'flotation',False),(95,'fire',False),(100,'bridge',False)],
'aviation':[(5,'weapon',False),(25,'flight_deck',True),(40,'hangar',True),(45,'ammo_fuel',True),(50,'sensor',False),(55,'cic',False),(65,'engineering',True),(75,'flooding',True),(90,'fire',False),(95,'bridge_airplot',False),(100,'rudder',False)],
'underwater':[(15,'weapon',False),(25,'sensor',False),(30,'cic',False),(50,'engineering',True),(80,'flooding',True),(90,'fire',False),(95,'bridge_airplot',False),(100,'rudder',False)],
'airburst':[(20,'weapon',False),(55,'sensor',False),(60,'engineering',True),(90,'weapon_flightdeck',True),(95,'flight_deck',True),(100,'bridge_airplot',False)],
'merchant':[(10,'weapon_cargo',False),(15,'sensor',False),(25,'engineering',True),(30,'cic_cargo',False),(60,'cargo',False),(75,'flooding',False),(90,'fire',False),(95,'bridge',False),(100,'rudder',False)],
'small_cargo':[(10,'weapon_cargo',False),(15,'sensor',False),(25,'engineering',False),(35,'personnel',False),(60,'cargo',False),(80,'flotation',False),(95,'fire',False),(100,'bridge',False)],
'sub_major':[(15,'weapon',False),(20,'sensor',False),(35,'hull_penetration',False),(40,'battery',False),(45,'battery_engineering',False),(55,'engineering',False),(70,'flooding',False),(85,'fire',False),(90,'sensor',False),(95,'control_major',False),(100,'rudder',False)],
'sub_minor':[(15,'weapon',False),(20,'sensor',False),(35,'hull_deformation',False),(40,'battery',False),(45,'battery_engineering',False),(55,'engineering',False),(70,'half_flooding',False),(85,'half_fire',False),(90,'sensor',False),(95,'control_minor',False),(100,'rudder',False)]}
data={'schema_version':1,'rule_profile':'hv2026-rts-v2','source':{'core':'14-2.HpnV_Rules_1_Jan_26.pdf','version':'2026-01-01','pdf_pages':[127,128,129,130,131,132,133,134,135,136,137],'expanded':'3-2.（扩展损伤手册26版）Harpoon_V_Expanded_Critical_Hit_Tables_(Chinese_Language).pdf','expanded_pdf_pages':list(range(2,14)),'visual_evidence':'E:/CodexBuilds/harpoon-damage-rules-20261021'},'adaptations':{'RTS-02-01':'Continuous1s tick replaces plotting phases. Hits collected inside each tick, primary damage and ordinary critical ratio resolved together; attacks already eligible during that tick remain eligible.','RTS-14-01':'For heterogeneous hits in a tick, ordinary criticals are assigned to contributing hits weighted by primary DP; independent automatic and airframe criticals retain their own source.','RTS-14-02':'No unprinted +/-15% hull variance or blanket per-hit critical probability. Source annex profiles determine target-specific DP.'},'critical_ratio':[[0,0,0,0,0,1],[0,0,0,0,1,2],[0,0,0,1,2,3],[0,0,1,2,3,4],[0,1,2,3,4,5],[1,2,3,4,5,6],[2,3,4,5,6,7],[3,4,5,6,7,8],[4,5,6,7,8,9],[5,6,7,8,9,10],[6,7,8,9,10,11]],'critical_columns':{k:[{'through':n,'effect':e,'armored':s}for n,e,s in v]for k,v in tables.items()},'platforms':{'us_ford':{'size':'A','role':'aviation','year':2017,'belt_cm':0,'deck_cm':5,'tps':315,'source':'America2025 PDF7/A4','dp_breaks':[0,417,834,1250,1500,1667],'speed_steps':[32,24,16,8,0,0]}} ,'weapons':{
'cn_yj12':{'penetration_cm':13,'extra':'D6','profile':'high','source':'China2025 PDF71/D1'},
'cn_yj12_air':{'penetration_cm':9,'extra':'D6','profile':'high','source':'China2025 PDF82/H4, high cruise/remarkD'},
'cn_yj12_surface_skim':{'penetration_cm':13,'extra':'D6','profile':'skim','source':'China2025 PDF71/D1, skim row'},
'cn_yj12_air_skim':{'penetration_cm':9,'extra':'D6','profile':'skim','source':'China2025 PDF82/H4, skim row'},
'cn_yj18':{'penetration_cm':13,'extra':'D6/2','profile':'skim','source':'China2025 PDF71/D1'},
'cn_yj62':{'penetration_cm':15,'extra':'D6/2','profile':'skim','source':'China2025 PDF71/D1'},
'cn_yj82':{'penetration_cm':9,'extra':'D6/2','profile':'skim','source':'China2025 PDF71/D1'},
'cn_yj83':{'penetration_cm':9,'extra':'D6/2','profile':'skim','source':'China2025 PDF71/D1'},
'cn_yj83k':{'penetration_cm':9,'extra':'D6/2','profile':'skim','source':'China2025 PDF82/H4'},
'cn_yj91':{'penetration_cm':1,'airburst':'D6+2','first_sensor':True,'profile':'high','source':'China2025 PDF82/H4;core7.4.2'},
'cn_yu6':{'ship_dp':141,'sub_dp':100,'fuze':'influence','diameter_mm':533,'source':'China2025 PDF76/F1'},
'cn_yu10':{'ship_dp':141,'sub_dp':100,'fuze':'influence','diameter_mm':533,'source':'China2025 PDF76/F1'},
'cn_yu9':{'ship_dp':128,'sub_dp':91,'fuze':'influence','diameter_mm':533,'source':'China2025 PDF76/F1'},
}}
data['severity_limits']={'A':[10,15,17],'B':[10,15,17],'C':[8,12,14],'D':[8,12,14],'E':[6,10,12],'F':[4,7,9],'G':[4,7,9]}
data['dc_reduction']=[[-2,-2,-2,-1],[-2,-2,-1,-1],[-2,-1,-1,-1],[-1,-1,-1,0],[-1,-1,0,0],[-1,0,0,1],[0,0,1,1],[0,1,1,1],[1,1,1,2],[1,1,2,2]]
data['narrow_aspect_deg']=20
data['platforms'].update({
 'cn_001':dict(size='A',role='aviation',year=2012,belt_cm=0,deck_cm=0,tps=0,dp_breaks=[0,252,505,758,909,1010],speed_steps=[29,21,15,7,0,0],source='China2025 PDF10/A3'),
 'cn_002':dict(size='A',role='aviation',year=2019,belt_cm=0,deck_cm=0,tps=0,dp_breaks=[0,256,512,767,921,1023],speed_steps=[29,22,15,7,0,0],source='China2025 PDF10/A3'),
 'cn_003':dict(size='A',role='aviation',year=2025,belt_cm=0,deck_cm=0,tps=0,dp_breaks=[0,320,639,959,1150,1278],speed_steps=[31,23,16,8,0,0],source='China2025 PDF10/A3'),
 'cn_075':dict(size='A',role='aviation',year=2021,belt_cm=0,deck_cm=0,tps=0,dp_breaks=[0,137,274,411,493,548],speed_steps=[23,17,12,6,0,0],source='China2025 PDF29/A22'),
 'us_nimitz':dict(size='A',role='aviation',year=1975,belt_cm=0,deck_cm=5,tps=315,nuclear=True,source='America2025 PDF8/A5; current generic fit DP kept separate pending dated variant')})
data['weapons']['cn_yj12_air'].update(profile='vhi',min_launch_altitude_m=7501,terminal_nm=27,terminal_speed_kts=1585)
data['weapons']['cn_yj12_air_skim']['min_launch_altitude_m']=7501
for key,label,rng,minrng,speed,dp in [
 ('cn_yj12','YJ-12A/B high cruise',162,10.8,1719,48),
 ('cn_yj12_surface_skim','YJ-12A/B sea-skimming cruise',54,10.8,1320,46),
 ('cn_yj12_air','YJ-12 air-launched high cruise',216,21.6,1719,48),
 ('cn_yj12_air_skim','YJ-12 air-launched sea-skimming cruise',110,21.6,1320,46)]:
 data['weapons'][key].update(display_name=label,range_nm=rng,min_range_nm=minrng,speed_kts=speed,ship_dp=dp,terminal_profile='skim')
data['weapons']['cn_yj12_air_skim'].update(terminal_nm=27,terminal_speed_kts=1585)
data['weapons']['cn_yj18'].update(terminal_nm=10.8,terminal_speed_kts=1980,range_nm=165.5,speed_kts=528)
data['weapons']['cn_yj62'].update(extra='D6',ship_dp=44)
data['weapons']['cn_yj91'].update(prh_generation=3)
data['adaptations']['RTS-07-01']='PRH gen3 can retune to another active radar on the same target. Silent targets are approached at the last emitter position. Its shutdown hit modifier is not inferred from gen2: a terminal event requiring this unresolved value is saved as pending adjudication, without rolling a hit/miss. See R-027.'
data['weapons'].update({
 'cn_yj9':dict(ship_dp=20,penetration_cm=5,extra='D6/3',profile='skim',min_launch_altitude_m=21,source='China2025 PDF82/H4 remarkG rail-launched'),
 'cn_dh10':dict(ship_dp=52,penetration_cm=20,extra='D6',profile='skim',source='China2025 PDF71/D1'),
 'us_harpoon':dict(ship_dp=40,penetration_cm=9,extra='D6/2',profile='skim',source='AmericaNavy2025 PDF77/D1 HarpoonIC'),
 'us_agm84':dict(ship_dp=40,penetration_cm=9,extra='D6/2',profile='skim',source='AmericaAircraft2025 PDF99/H2-1 AGM84D'),
 'us_nsm':dict(ship_dp=34,penetration_cm=18,extra='D6/2',profile='skim',source='AmericaNavy2025 PDF77/D1 NSM'),
 'us_tomahawk':dict(ship_dp=51,penetration_cm=14,extra='D6',profile='skim',source='AmericaNavy2025 PDF77/D1 TacticalTomahawkIV'),
 'us_mk48':dict(ship_dp=175,sub_dp=124,fuze='influence',diameter_mm=533,source='AmericaNavy2025 PDF84/F2 Mk48Mod7'),
 'us_mk54':dict(sub_dp=52,fuze='contact',diameter_mm=324,range_nm=6,speed_kts=45,source='AmericaNavy2025 PDF84/F2 Mk54 fast setting45kt/6nm; economy30kt/8nm separate')})
data['platforms']['us_ford'].update(nuclear=True,mount_locations={k:'topside' for k in ['us_essm','us_ram','ciws_30mm','gun_30mm']})
data['platforms'].update({
 'us_america_lha':dict(size='A',role='aviation',year=2014,dp_breaks=[0,145,290,435,522,580],speed_steps=[23,17,12,6,0,0],critical_armor={'flight_deck':2},source='AmericaNavy2025 PDF49/A46; hull/special damage modifiers require separate implementation'),
 'us_wasp_lhd':dict(size='A',role='aviation',year=1989,dp_breaks=[0,135,269,404,484,538],speed_steps=[22,18,11,6,0,0],critical_armor={'flight_deck':2},source='AmericaNavy2025 PDF50/A47 Wasp not LHD8; hull/special damage modifiers require separate implementation')})
data['adaptations']['RTS-14-03']='Aggregate damage-control reduction is distributed proportionally across individual fire/flood records. Each retains its original first-damage deadline, so a later casualty cannot drain HP early or reset an older fire.'
data['adaptations']['RTS-04-01']='A high/very-high missile profile starts at a representative altitude inside the printed band; airborne movement is continuous and terminal transition uses the annex distance/speed. Airframe altitude above the launching aircraft is missile flight, not aircraft ceiling.'
data['time']={'tactical_s':180,'intermediate_s':1800,'surface_s':180,'sinking_die_s':600}
data['critical_columns']['underwater'][-2]['armored']=True
data['critical_columns']['merchant'][2]['armored']=False
data['source']['critical_table_authority']='English2026 PDF128/14-2; D20 rows expanded into equivalent D100 bands. English takes precedence over2021 expanded-table star differences.'
data['tps_belt_reduction']=[[5,0],[10,0.10],[20,0.25],[30,0.40]]
data['tps_areas_per_side']=3
data['pressure_hull_chances']=[80,60,70,50]
data['shaped_charge_roll_modifier']=20
data['adaptations']['RTS-14-04']='For point-platform torpedo collisions, a seeded D3 chooses fore/mid/aft void area independently on each side. A supplied impact_area overrides it. Each area protects once; total side absorption and per-hit R/3 caps remain as printed. This is an explicit spatial abstraction, not a table from the manual.'
data['adaptations']['RTS-14-05']='TPS overflow flooding uses twice the remaining protection as the full-flood threshold, following the worked Enterprise 97DP/24DP example in English14-5/14-6; the surrounding wording is registered in R-025.'
data['adaptations']['RTS-14-06']='Bridge casualty delays a new navigation order (including its route/station) once by two tactical turns. Continuous steering inside the accepted route is not re-delayed each second. Disabled rudder reduces the continuous angular rate to one-third, preserving the printed45-to15-degree ratio.'
data['adaptations']['RTS-14-07']='Secondary fire/flood DP enters the same tick ledger and produces normal ratio criticals. Fire uses the target surface/aviation column; flooding uses the underwater column (submerged submarines use major). Causal immediate flooding is resolved after its initiating hit inside the same tick.'
data['adaptations']['RTS-14-08']='The current instant-spawn deck model stores ready and cycling inventory in the hangar until launch. Hangar hits draw uniformly from this stored inventory, never airborne aircraft. There are no parked-flight-deck locations yet; armed/closely-parked aircraft optional chains are not enabled.'
data['adaptations']['RTS-14-09']='A weapon fit containing multiple physical mounts loses only the selected mount. Remaining pooled ammunition is distributed evenly at the time of the hit; a hit on an already destroyed mount is ignored. Explosion outcome requires explicit topside/belowdecks metadata, otherwise the missing-location adjudication is logged as blocked.'
data['adaptations']['RTS-02-02']='Within a tick, preserve each side command order, apply navigation/emission orders before manual Fire, and collect Blue then Red deterministically. Primary damage remains simultaneous. This removes cross-side input interleaving from projectile allocation and RNG.'
data['adaptations']['RTS-14-10']='Small-craft twice-DP sinking threshold uses a rolling180-second tactical-duration window, including primary and secondary damage. Hull DP does not accumulate outside this window. This preserves the printed tactical duration without introducing arbitrary fixed-turn boundary exploits.'
path=repo/'data/rules/damage-runtime-v2.json'
path.write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print('wrote',path)

//! Source acceptance for the next explicit rule profile, never a legacy replay rewrite.
use harpoon_core::{air, AirMission, Database, EventKind, Scenario, World, Vec2};
use harpoon_core::types::PlatformKind;
use serde_json::json;
use harpoon_core::rules::{operations as ops,damage_runtime as damage_rules};

fn fixture() -> World {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"air-source-2026","name":"Air source acceptance","duration_s":100000,"seed":20261022,
        "context":{"rules_profile":"hv2026-rts-v3","flight_environment":{"wind_from_deg":0,"wind_kts":0},
          "radar_environment":{"sea_state":3,"precipitation":"None"},"air_wings":{"CV":[{"class":"us_f18e","count":8}]}},
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[{"side":"Blue","class":"us_ford","name":"CV","pos":[0,0],"speed_kts":32}]
    })).unwrap();
    harpoon_core::load_scenario(sc, db).unwrap()
}

#[test]
fn catapult_launch_is_not_an_instant_eight_plane_spawn() {
    let mut w=fixture(); let cv=*w.entities.keys().next().unwrap();
    air::launch_aircraft(&mut w,cv,"us_f18e",8,&AirMission::Recon{point:Vec2::new(40.0,0.0)}).unwrap();
    assert_eq!(w.entities.values().filter(|e|e.alive&&e.kind==PlatformKind::Aircraft).count(),0,
        "HV4.6.4.1: each catapult must complete its two minute launch sequence");
    for t in [299,300,419,420] {
        w.tick=t;air::update_air_ops(&mut w);
        assert_eq!(w.entities.values().filter(|e|e.alive&&e.kind==PlatformKind::Aircraft).count(),match t{299=>0,300|419=>4,_=>8});
    }
}

#[test]
fn an_aircraft_at_high_altitude_cannot_instantly_recover() {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();
    air::launch_aircraft(&mut w,cv,"us_f18e",1,&AirMission::Recon{point:Vec2::zero()}).unwrap();
    w.tick=300;air::update_air_ops(&mut w);
    let id=w.entities.values().find(|e|e.kind==PlatformKind::Aircraft).unwrap().id;
    let a=w.entity_mut(id).unwrap(); a.altitude_m=10000.0;a.speed_kts=600.0;
    a.mission=Some(harpoon_core::world::MissionState::Rtb);
    air::update_air_ops(&mut w);
    assert!(!w.events.iter().any(|e|matches!(e.kind,EventKind::AirRecovered{..})),
        "HV4.6.4.1: landing needs Low altitude and quarter full-power speed");
}

#[test]
fn launch_reservations_prevent_duplicate_orders_without_removing_hangar_inventory() {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();
    let m=AirMission::Recon{point:Vec2::zero()};
    air::launch_aircraft(&mut w,cv,"us_f18e",6,&m).unwrap();
    assert_eq!(w.entity(cv).unwrap().deck.as_ref().unwrap().ready[0].1,8,"queued aircraft remain physically in hangar");
    assert!(air::launch_aircraft(&mut w,cv,"us_f18e",3,&m).is_err());
    assert!(air::launch_aircraft(&mut w,cv,"us_f18e",0,&m).is_err());
    assert_eq!(ops::reserved(&w,cv,"us_f18e"),6);
}

#[test]
fn wind_over_deck_is_actual_speed_plus_aligned_wind() {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();let m=AirMission::Rtb;
    w.entity_mut(cv).unwrap().speed_kts=20.0;
    w.context.flight_environment.as_mut().unwrap().wind_kts=10.0;
    air::launch_aircraft(&mut w,cv,"us_f18e",1,&m).unwrap();
    w.entity_mut(cv).unwrap().course_deg=90.0;
    assert!(air::launch_aircraft(&mut w,cv,"us_f18e",1,&m).is_err());
    w.context.flight_environment=None;
    assert!(air::launch_aircraft(&mut w,cv,"us_f18e",1,&m).is_err());
}

#[test]
fn damaged_catapult_restarts_sequence_and_does_not_release_two_planes_in_same_lane() {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();
    air::launch_aircraft(&mut w,cv,"us_f18e",8,&AirMission::Recon{point:Vec2::zero()}).unwrap();
    w.tick=250;w.entity_mut(cv).unwrap().ext.insert("damage.deck_launch".into(),1.0);air::update_air_ops(&mut w);
    w.tick=300;air::update_air_ops(&mut w);
    w.entity_mut(cv).unwrap().ext.insert("damage.deck_launch".into(),0.0);
    w.tick=420;air::update_air_ops(&mut w);
    assert_eq!(w.entities.values().filter(|e|e.kind==PlatformKind::Aircraft).count(),0);
    w.tick=421;air::update_air_ops(&mut w);
    assert_eq!(w.entities.values().filter(|e|e.kind==PlatformKind::Aircraft).count(),4);
    w.tick=540;air::update_air_ops(&mut w);
    assert_eq!(w.entities.values().filter(|e|e.kind==PlatformKind::Aircraft).count(),4);
    w.tick=541;air::update_air_ops(&mut w);
    assert_eq!(w.entities.values().filter(|e|e.kind==PlatformKind::Aircraft).count(),8);
}

#[test]
fn source_ready_times_are_size_and_load_specific_and_fuel_parallel_with_arm() {
    assert_eq!(ops::ready_seconds(1,"guided",false).unwrap(),90*60);
    assert_eq!(ops::ready_seconds(2,"guided",false).unwrap(),100*60);
    assert_eq!(ops::ready_seconds(3,"guided",false).unwrap(),360*60);
    assert_eq!(ops::ready_seconds(1,"asw",true).unwrap(),40*60);
    assert_eq!(ops::ready_seconds(2,"none",true).unwrap(),50*60);
    assert!(ops::ready_seconds(0,"asw",true).is_err());
    assert!(ops::ready_seconds(4,"guided",false).is_err());
}

#[test]
fn recovery_rate_is_one_per_minute_then_actual_guided_weapon_turnaround() {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();
    air::launch_aircraft(&mut w,cv,"us_f18e",2,&AirMission::Recon{point:Vec2::zero()}).unwrap();
    w.tick=300;air::update_air_ops(&mut w);
    let ids:Vec<_>=w.entities.values().filter(|e|e.kind==PlatformKind::Aircraft).map(|e|e.id).collect();
    for id in &ids {let a=w.entity_mut(*id).unwrap();a.altitude_m=1000.0;a.speed_kts=585.0/4.0;a.course_deg=0.0;}
    w.tick=400;
    assert_eq!(ops::recovery_ready(&mut w,ids[0],cv),Some(400+90*60));
    assert!(ops::recovery_ready(&mut w,ids[1],cv).is_none());
    assert_eq!(w.rules.operations.as_ref().unwrap().recoveries.len(),1);
    w.tick=459;assert!(ops::recovery_ready(&mut w,ids[1],cv).is_none());
    w.tick=460;assert_eq!(ops::recovery_ready(&mut w,ids[1],cv),Some(460+90*60));
    assert_eq!(w.rules.operations.as_ref().unwrap().recoveries[&ids[1]].due,520);
}

#[test]
fn explicit_v3_table_identity_and_queue_survive_roundtrip() {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();
    air::launch_aircraft(&mut w,cv,"us_f18e",8,&AirMission::Recon{point:Vec2::new(30.0,0.0)}).unwrap();
    w.tick=299;air::update_air_ops(&mut w);
    let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    restored.rules.validate_tables().unwrap();
    for t in 300..=600 {w.tick=t;restored.tick=t;air::update_air_ops(&mut w);air::update_air_ops(&mut restored);}
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&restored).unwrap());
    restored.rules.table_fingerprint=Some(damage_rules::table_fingerprint());assert!(restored.rules.validate_tables().is_err());
}

#[test]
fn old_v2_profile_retains_its_instant_launch_and_wire_shape() {
    let mut w=fixture();w.rules.profile=damage_rules::RuleProfile::Rts2026;w.rules.operations=None;
    let cv=*w.entities.keys().next().unwrap();air::launch_aircraft(&mut w,cv,"us_f18e",8,&AirMission::Rtb).unwrap();
    assert_eq!(w.entities.values().filter(|e|e.kind==PlatformKind::Aircraft).count(),8);
    assert!(serde_json::to_value(&w.rules).unwrap().get("operations").is_none());
    assert_eq!(damage_rules::fingerprint_for(damage_rules::RuleProfile::Rts2026),damage_rules::table_fingerprint());
}

#[test]
fn radar_clutter_matches_sps10_jungle_example_and_largest_source_rule() {
    assert_eq!(ops::clutter_modifier(2,true,false,3,"None",Some(2),true).unwrap(),0.5);
    assert_eq!(ops::clutter_modifier(2,true,false,9,"Heavy",Some(2),true).unwrap(),0.1);
    assert_eq!(ops::clutter_modifier(6,false,false,9,"Torrential",None,true).unwrap(),0.6);
    assert!(ops::clutter_modifier(6,false,true,9,"Torrential",None,true).is_err(),"no invented off-scale clutter cell");
    assert_eq!(ops::clutter_modifier(1,true,false,0,"Drizzle",None,true).unwrap(),1.0);
    assert!(ops::clutter_modifier(0,true,false,0,"None",None,true).is_err());
    assert!(ops::clutter_modifier(6,false,false,10,"None",None,true).is_err());
    assert!(ops::clutter_modifier(6,false,false,0,"Unknown",None,true).is_err());
}

#[test]
fn airborne_radar_altitude_windows_and_ship_las_ceiling_use_printed_bands() {
    assert_eq!(ops::altitude_modifier("AI",true,5000.0,1000.0),0.5);
    assert_eq!(ops::altitude_modifier("AI",true,5000.0,50.0),0.0);
    assert_eq!(ops::altitude_modifier("LD/SD",true,5000.0,50.0),1.0);
    assert_eq!(ops::altitude_modifier("AS",true,5000.0,10000.0),1.0);
    assert_eq!(ops::altitude_modifier("AS",true,5000.0,1000.0),0.5);
    assert_eq!(ops::altitude_modifier("LAS",false,20.0,7500.0),1.0);
    assert_eq!(ops::altitude_modifier("LAS",false,20.0,7501.0),0.0);
    assert_eq!(ops::altitude_modifier("SS",false,20.0,60.0),0.1);
    assert_eq!(ops::altitude_modifier("SS",false,20.0,61.0),0.0);
    assert_eq!(ops::altitude_modifier("HF",false,20.0,5000.0),0.0);
}

#[test]
fn reaction_table_examples_and_human_auto_columns_are_distinct() {
    assert_eq!(ops::reaction(3,false,3).unwrap(),(4,3));
    assert_eq!(ops::reaction(4,false,4).unwrap(),(2,2));
    assert_eq!(ops::reaction(5,false,2).unwrap(),(4,1));
    assert_eq!(ops::reaction(5,true,2).unwrap(),(3,0));
    assert_eq!(ops::reaction(6,true,2).unwrap(),(2,0));
    assert_eq!(ops::reaction(6,true,10).unwrap(),(-2,0));
    assert!(ops::reaction(0,true,6).is_err());assert!(ops::reaction(6,true,13).is_err());
}

#[test]
fn three_second_table_uses_actual_caliber_and_speed_boundaries() {
    for (cal,expected) in [(20.0,[0.1,0.5,0.9]),(30.0,[0.1,0.5,0.9]),(60.0,[0.0,0.4,0.7]),(61.0,[0.0,0.1,0.2])] {
        for (speed,p) in [(750.0,expected[0]),(751.0,expected[1]),(1525.0,expected[1]),(1526.0,expected[2])] {
            assert_eq!(ops::fragment_probability(Some(cal),false,speed).unwrap(),p);
        }
    }
    assert_eq!(ops::fragment_probability(None,true,750.0).unwrap(),0.0);
    assert_eq!(ops::fragment_probability(None,true,1525.0).unwrap(),0.1);
    assert_eq!(ops::fragment_probability(None,true,1526.0).unwrap(),0.3);
    assert!(ops::fragment_probability(None,false,2000.0).is_err());
}

#[test]
fn sam_with_min_range_over_2_5_cannot_engage_in_point_defense_band() {
    let w=fixture();let mut sam=w.db.weapons["us_essm"].clone();
    sam.min_range_nm=2.51;
    assert!(!ops::sam_pd_legal(&sam,4.0));assert!(ops::sam_pd_legal(&sam,4.0001));
    sam.min_range_nm=2.5;assert!(ops::sam_pd_legal(&sam,4.0));
}

fn radar_fixture()->(World,harpoon_core::EntityId,harpoon_core::EntityId) {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();
    air::launch_aircraft(&mut w,cv,"us_f18e",1,&AirMission::Recon{point:Vec2::new(30.0,0.0)}).unwrap();
    w.tick=300;air::update_air_ops(&mut w);
    let id=w.entities.values().find(|e|e.kind==PlatformKind::Aircraft).unwrap().id;
    for e in w.entities.values_mut(){e.sensors.clear();}
    let a=w.entity_mut(id).unwrap();a.pos=Vec2::zero();a.altitude_m=1000.0;a.course_deg=90.0;
    a.sensors.push(harpoon_core::world::SensorInstance{sensor:"radar_air_med".into(),active:true});
    let red=w.allocate_id();let mut target=w.entity(cv).unwrap().clone();target.id=red;target.side=harpoon_core::Side::Red;
    target.pos=Vec2::new(10.0,0.0);target.course_deg=0.0;target.speed_kts=20.0;target.name="Radar target".into();w.entities.insert(red,target);
    (w,id,red)
}

#[test]
fn ordinary_radar_does_not_reveal_ship_class_or_instant_true_motion() {
    let (mut w,_,_)=radar_fixture();
    for t in 301..=305 {w.tick=t;harpoon_core::sensors::update_sensors(&mut w);}
    let obs=harpoon_core::observe(&w,harpoon_core::Side::Blue,0,0);assert_eq!(obs.contacts.len(),1);
    assert_eq!(obs.contacts[0].class_name,None,"5.2.3: a radar blip is not a ship-class identification");
    assert_eq!(obs.contacts[0].motion_known,Some(false),"one Tactical Turn of positional observations is needed");
    assert_eq!(obs.contacts[0].est_speed_kts,0.0);
}

#[test]
fn radar_motion_is_derived_from_observed_positions_after_180_seconds() {
    let (mut w,_,target)=radar_fixture();
    for t in 301..=481 {
        w.tick=t;let e=w.entity_mut(target).unwrap();e.pos=Vec2::new(10.0,(t-301) as f64*20.0/3600.0);
        e.course_deg=270.0;e.speed_kts=999.0; // truth velocity deliberately contradicts measured positions
        harpoon_core::sensors::update_sensors(&mut w);
    }
    let obs=harpoon_core::observe(&w,harpoon_core::Side::Blue,0,0);
    assert_eq!(obs.contacts[0].motion_known,Some(true));
    assert!((obs.contacts[0].est_speed_kts-20.0).abs()<1e-8);
    assert_eq!(obs.contacts[0].est_course_deg,0.0);
    assert!(obs.contacts[0].class_name.is_none());
}

#[test]
fn radar_uses_source_signature_range_arc_and_both_physical_heights() {
    let (mut w,observer,target)=radar_fixture();
    let def=w.db.sensors["radar_air_med"].clone();
    assert_eq!(ops::radar_height(w.entity(target).unwrap()).unwrap(),40.0);
    let h=harpoon_core::geom::horizon_nm(1000.0,40.0);
    assert_eq!(ops::radar_range(&w,w.entity(observer).unwrap(),&def,w.entity(target).unwrap()).unwrap(),h);
    w.context.radar_environment.as_mut().unwrap().sea_state=9;
    // Clutter applies to the annex range first; the horizon is the final limit.
    assert_eq!(ops::radar_range(&w,w.entity(observer).unwrap(),&def,w.entity(target).unwrap()).unwrap(),h);
    w.entity_mut(observer).unwrap().course_deg=270.0;
    assert_eq!(ops::radar_range(&w,w.entity(observer).unwrap(),&def,w.entity(target).unwrap()).unwrap(),0.0);
    w.entity_mut(observer).unwrap().course_deg=90.0;
    w.entity_mut(target).unwrap().class="unverified_signature".into();
    assert!(ops::radar_range(&w,w.entity(observer).unwrap(),&def,w.entity(target).unwrap()).is_err());
}

#[test]
fn fifth_and_sixth_generation_reaction_runs_in_sensor_chain_and_is_private_until_recognition() {
    let (base,_,target)=radar_fixture();
    let mut chosen=None;
    for seed in 1..100 {
        let mut w=base.clone();w.rng=harpoon_core::Rng::new(seed);w.seed=seed;w.tick=1;
        for e in w.entities.values_mut(){e.sensors.clear();}
        let cv=*w.entities.keys().next().unwrap();
        w.entity_mut(cv).unwrap().sensors.push(harpoon_core::world::SensorInstance{sensor:"radar_air_long".into(),active:true});
        let t=w.entity_mut(target).unwrap();t.kind=PlatformKind::Aircraft;t.class="us_f18e".into();t.altitude_m=1000.0;t.rule_data=None;t.deck=None;
        harpoon_core::sensors::update_sensors(&mut w);
        if let Some(track)=w.rules.operations.as_ref().unwrap().tracks.values().next().cloned() {
            if track.recognized>1 {chosen=Some((w,track));break;}
        }
    }
    let (mut w,track)=chosen.expect("seed range must exercise a positive printed reaction delay");
    assert!(harpoon_core::observe(&w,harpoon_core::Side::Blue,0,0).contacts.is_empty());
    assert!(!harpoon_core::observe(&w,harpoon_core::Side::Blue,0,1000).events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="5.2.1.reaction_clock")));
    for tick in 2..track.recognized {w.tick=tick;harpoon_core::sensors::update_sensors(&mut w);assert!(w.sides[0].contacts.is_empty());}
    w.tick=track.recognized;harpoon_core::sensors::update_sensors(&mut w);assert_eq!(w.sides[0].contacts.len(),1);
    let count=w.events.iter().filter(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="5.2.1.reaction_clock")).count();
    assert_eq!(count,1,"one independent2D6 roll per continuous acquisition, not per second");
    w.entity_mut(target).unwrap().pos=Vec2::new(500.0,0.0);w.tick+=1;harpoon_core::sensors::update_sensors(&mut w);
    assert!(w.rules.operations.as_ref().unwrap().tracks.is_empty());
}

#[test]
fn land_clutter_uses_sector_intersection_and_the_largest_source() {
    let region=ops::LandClutterRegion{center:Vec2::new(10.0,0.0),radius_nm:0.1,terrain:2};
    assert!(region.affects(Vec2::zero(),90.0,20.0));
    assert!(!region.affects(Vec2::zero(),90.0,9.0));
    assert!(!region.affects(Vec2::zero(),0.0,20.0));
    let grazing=ops::LandClutterRegion{center:Vec2::from_bearing(96.0).scaled(10.0),radius_nm:0.3,terrain:2};
    assert!(grazing.affects(Vec2::zero(),90.0,20.0));
    let (mut w,o,t)=radar_fixture();w.context.radar_land_clutter=Some(vec![region]);
    let def=w.db.sensors["radar_air_med"].clone();assert!(ops::radar_range(&w,w.entity(o).unwrap(),&def,w.entity(t).unwrap()).is_ok());
}

#[test]
fn pad_uses_three_minute_shared_launch_and_landing_capacity() {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();
    {let c=w.entity_mut(cv).unwrap();c.class="us_burke_2a".into();c.speed_kts=0.0;c.deck.as_mut().unwrap().ready=vec![("us_mh60r".into(),2)];}
    air::launch_aircraft(&mut w,cv,"us_mh60r",2,&AirMission::Recon{point:Vec2::new(30.0,0.0)}).unwrap();
    w.tick=300;air::update_air_ops(&mut w);
    let id=w.entities.values().find(|e|e.kind==PlatformKind::Aircraft).unwrap().id;
    let a=w.entity_mut(id).unwrap();a.altitude_m=1000.0;a.speed_kts=35.0;a.course_deg=0.0;
    assert!(ops::recovery_ready(&mut w,id,cv).is_none(),"same spot is reserved for the next queued helicopter");
    w.tick=479;air::update_air_ops(&mut w);assert_eq!(w.entities.values().filter(|e|e.kind==PlatformKind::Aircraft).count(),1);
    w.tick=480;air::update_air_ops(&mut w);assert_eq!(w.entities.values().filter(|e|e.kind==PlatformKind::Aircraft).count(),2);
    assert!(ops::recovery_ready(&mut w,id,cv).is_none(),"last takeoff and landing cannot share a spot in the same instant");
    w.tick=660;assert_eq!(ops::recovery_ready(&mut w,id,cv),Some(660+40*60));
}

#[test]
fn safe_sea_state_uses_actual_size_stabilizer_speed_and_recovery_system() {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();
    w.entity_mut(cv).unwrap().class="us_zumwalt".into();w.entity_mut(cv).unwrap().speed_kts=8.0;
    w.context.radar_environment.as_mut().unwrap().sea_state=8;
    assert_eq!(ops::flight_risk(&w,cv,"us_mh60r",false,true).unwrap(),0.0);
    assert!((ops::flight_risk(&w,cv,"us_mh60r",false,false).unwrap()-0.2).abs()<1e-9);
    w.entity_mut(cv).unwrap().speed_kts=7.99;
    assert!((ops::flight_risk(&w,cv,"us_mh60r",false,true).unwrap()-0.4).abs()<1e-9);
}

#[test]
fn sea_state_crashes_are_own_losses_without_invented_enemy_kills() {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();w.context.radar_environment.as_mut().unwrap().sea_state=9;
    air::launch_aircraft(&mut w,cv,"us_f18e",8,&AirMission::Recon{point:Vec2::new(50.0,0.0)}).unwrap();
    for tick in 1..=420 {w.tick=tick;air::update_air_ops(&mut w);}
    let crashes=w.events.iter().filter(|e|matches!(e.kind,EventKind::Ditched{..})).count();
    assert!(crashes>0);assert_eq!(w.losses[0],crashes as u32);
    assert_eq!(w.sides[1].confirmed_kills,0);
}

#[test]
fn fragment_hook_reaches_the_correct_missile_target_and_uses_source_table_not_distance_curve() {
    for distance in [0.2,3.9] {
        let mut w=fixture();let cv=*w.entities.keys().next().unwrap();
        let def=w.db.weapons["cn_yj83k"].clone();
        let incoming=harpoon_core::weapons::launch(&mut w,cv,&def,Some(cv),None).unwrap();
        {let m=w.entity_mut(incoming).unwrap();m.pos=Vec2::new(distance,0.0);m.speed_kts=2000.0;}
        let ciws=w.db.weapons["ciws_30mm"].clone();let before=w.entity(cv).unwrap().hp;
        ops::intercept(&mut w,incoming,cv,&ciws);
        let detail=w.events.iter().find_map(|e|match &e.kind{EventKind::RulesEvent{rule,detail,..}if rule=="8.1.5.fragment_roll"=>Some(serde_json::from_str::<serde_json::Value>(detail).unwrap()),_=>None}).unwrap();
        assert_eq!(detail["probability"],0.9);assert_eq!(detail["caliber_mm"],20.0,"Ford's generic30mm alias is actually a20mm Phalanx");
        assert_eq!(w.entity(cv).unwrap().hp,before,"armored carrier receives no fragment DP");
        assert!(!w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="air_burst")),"legacy interpolation must not also run");
    }
}

#[test]
fn unknown_fragment_caliber_is_preserved_as_pending_adjudication() {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();let def=w.db.weapons["cn_yj83k"].clone();
    let incoming=harpoon_core::weapons::launch(&mut w,cv,&def,Some(cv),None).unwrap();w.entity_mut(incoming).unwrap().pos=Vec2::new(0.2,0.0);
    let gun=w.db.weapons["gun_30mm"].clone();ops::intercept(&mut w,incoming,cv,&gun);
    assert_eq!(w.rules.pending_adjudications[&incoming].rule,"8.1.5.fragments");
    assert!(w.rules.pending_adjudications[&incoming].projectile.projectile.is_some());
}

#[test]
fn queued_inventory_is_only_exposed_to_its_own_side_and_is_not_still_available() {
    let mut w=fixture();let cv=*w.entities.keys().next().unwrap();
    air::launch_aircraft(&mut w,cv,"us_f18e",6,&AirMission::Recon{point:Vec2::new(20.0,0.0)}).unwrap();
    let blue=harpoon_core::observe(&w,harpoon_core::Side::Blue,0,0);let deck=blue.units[0].deck.as_ref().unwrap();
    assert_eq!(deck.ready[0].1,2);assert_eq!(deck.queued.len(),6);
    let red=harpoon_core::observe(&w,harpoon_core::Side::Red,0,0);assert!(red.units.is_empty());assert!(red.events.is_empty());
}

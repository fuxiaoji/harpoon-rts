use harpoon_core::{commands::{AirMission,Command}, observe, scenario_from_json, load_scenario, Database, EntityId, Side, Vec2, EventKind};

fn setup() -> harpoon_core::World {
    load_scenario(scenario_from_json(r#"{"id":"control","name":"control","duration_s":3600,"seed":91,
      "context":{"air_wings":{"CV":[{"class":"us_f18e","count":4}]}},
      "weather":{"visibility_nm":1,"jam":[0,0]},"units":[
      {"side":"Blue","class":"us_ford","name":"CV","pos":[0,0]},
      {"side":"Blue","class":"cn_052d","name":"HELO DECK","pos":[15,0]},
      {"side":"Red","class":"cn_055","pos":[30,0]}]}"#).unwrap(),
      Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()).unwrap()
}

#[test]
fn failed_fire_has_private_execution_receipt_and_consumes_no_ammunition() {
    let mut w=setup(); let before=w.entities[&EntityId(1)].weapons.clone();
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::Fire {unit:EntityId(1),contact:harpoon_core::ContactId(999),weapon:"not-equipped".into(),rounds:1})]);
    assert_eq!(w.entities[&EntityId(1)].weapons[0].ammo,before[0].ammo);
    let has=|side| observe(&w,side,0,1000).events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent {domain,rule,..} if domain=="command" && rule=="rejected"));
    assert!(has(Side::Blue));assert!(!has(Side::Red));
}

#[test]
fn explicit_retask_releases_autopilot_without_refuelling_and_recall_still_works() {
    let mut w=setup();
    harpoon_core::air::launch_aircraft(&mut w,EntityId(1),"us_f18e",1,&AirMission::Recon {point:Vec2::new(-30.0,0.0)}).unwrap();
    let id=*w.entities.keys().last().unwrap();let fuel=w.entities[&id].endurance_s.unwrap();
    harpoon_core::sim::tick(&mut w,&[(Side::Red,Command::RetaskAircraft {unit:id,mission:AirMission::Rtb,manual:false})]);
    assert_eq!(w.entities[&id].mission.as_ref().unwrap().tag(),"recon");
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::RetaskAircraft {unit:id,mission:AirMission::Recon {point:Vec2::zero()},manual:true}),
      (Side::Blue,Command::Goto {unit:id,waypoints:vec![Vec2::new(30.0,0.0)]})]);
    assert!((w.entities[&id].ordered_course_deg-90.0).abs()<5.0);
    assert!(w.entities[&id].endurance_s.unwrap()<fuel);
    w.entity_mut(id).unwrap().pos=Vec2::new(15.0,0.0);
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::ReturnToBase {unit:id})]);
    assert!(w.entities[&id].alive,"fixed wing must not recover at the nearest destroyer");
    assert!(!harpoon_core::air::compatible_recovery(&w,id,EntityId(2)));
    assert!(harpoon_core::air::compatible_recovery(&w,id,EntityId(1)));
}

#[test]
fn radar_emcon_changes_detection_and_is_serializable_and_owned() {
    let mut w=setup();let id=EntityId(1);
    harpoon_core::sim::tick(&mut w,&[(Side::Red,Command::SetRadar {unit:id,active:false})]);
    assert!(harpoon_core::sensors::radar_emitting(&w.entities[&id]));
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::SetRadar {unit:id,active:false})]);
    assert!(!harpoon_core::sensors::radar_emitting(&w.entities[&id]));
    let obs=observe(&w,Side::Blue,0,1000);
    for s in &obs.units.iter().find(|u|u.id==id).unwrap().sensors {
      if w.db.sensors[&s.sensor].kind==harpoon_core::db::SensorKind::Radar {assert!(!s.active);assert_eq!(s.vs_air,None);}
    }
    let clone:harpoon_core::World=serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    assert!(!harpoon_core::sensors::radar_emitting(&clone.entities[&id]));
}

#[test]
fn source_flight_costs_low_altitude_and_rejects_vlow_reheat() {
    let mut w=setup();harpoon_core::air::launch_aircraft(&mut w,EntityId(1),"us_f18e",1,&AirMission::Recon {point:Vec2::new(50.0,0.0)}).unwrap();
    let id=*w.entities.keys().last().unwrap();let e=w.entities.get_mut(&id).unwrap();e.altitude_m=50.0;e.speed_kts=460.0;
    assert_eq!(harpoon_core::rules::flight_profiles::fuel_rate(e,460.0),2.0);
    assert_eq!(harpoon_core::rules::flight_profiles::speed_cap(e),Some(585.0));
    assert!(harpoon_core::rules::flight_profiles::configure(&mut w,id,"reheat").is_err());
    w.entities.get_mut(&id).unwrap().altitude_m=3000.0;
    assert!(harpoon_core::rules::flight_profiles::configure(&mut w,id,"reheat").is_ok());
    assert_eq!(w.entities[&id].ordered_speed_kts,815.0);
}

#[test]
fn automatic_air_policy_never_spends_anti_ship_rounds() {
    let mut w=setup();seed_target(&mut w);let id=EntityId(2);let before=w.entities[&id].weapons.clone();
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::SetEngagementPolicy {unit:id,air_only:true,max_in_flight:2}), (Side::Blue,Command::SetWeaponsFree {unit:id,free:true})]);
    for _ in 0..60 {harpoon_core::sim::tick(&mut w,&[]);}
    for m in before {if w.db.weapons[&m.weapon].kind==harpoon_core::db::WeaponKind::Ascm {assert_eq!(w.entities[&id].weapons.iter().find(|x|x.weapon==m.weapon).unwrap().ammo,m.ammo);}}
    let mut legacy=setup();seed_target(&mut legacy);harpoon_core::sim::tick(&mut legacy,&[(Side::Blue,Command::SetWeaponsFree {unit:id,free:true})]);
    assert!(legacy.events.iter().any(|e|matches!(&e.kind,EventKind::WeaponsRelease {shooter,..} if *shooter==id)),"positive control must actually have an engageable surface contact");
}

fn seed_target(w:&mut harpoon_core::World) {
    w.sides[0].contacts.insert(harpoon_core::ContactId(1),harpoon_core::world::Contact {id:harpoon_core::ContactId(1),about:EntityId(3),category:w.entities[&EntityId(3)].kind,class_name:None,estimate:w.entities[&EntityId(3)].pos,est_course_deg:0.0,est_speed_kts:0.0,quality:1.0,bearing_only:false,last_update_tick:0,first_tick:0,stale:false,sensors:vec!["radar_air_long".into()],offset:Vec2::zero(),announced:false});
}

#[test]
fn cooperating_ships_reserve_a_shared_air_intercept_budget() {
    let mut w=setup();w.entities.get_mut(&EntityId(3)).unwrap().kind=harpoon_core::types::PlatformKind::Aircraft;
    w.entities.get_mut(&EntityId(3)).unwrap().pos=Vec2::new(20.0,0.0);w.entities.get_mut(&EntityId(3)).unwrap().altitude_m=3000.0;
    seed_target(&mut w);let mut commands=vec![];
    for id in [EntityId(1),EntityId(2)] {commands.push((Side::Blue,Command::SetEngagementPolicy {unit:id,air_only:true,max_in_flight:2}));commands.push((Side::Blue,Command::SetWeaponsFree {unit:id,free:true}));}
    harpoon_core::sim::tick(&mut w,&commands);
    assert_eq!(w.entities.values().filter(|e|e.alive && e.projectile.as_ref().is_some_and(|p|p.launched_by_side==Side::Blue && p.target==Some(EntityId(3)))).count(),2,"the second escort must see the first escort's reservations before firing");
}

#[test]
fn manual_fire_must_obey_the_same_target_compatibility_as_automatic_fire() {
    let mut w=setup();w.entities.get_mut(&EntityId(3)).unwrap().pos=Vec2::new(2.0,0.0);seed_target(&mut w);
    harpoon_core::air::launch_aircraft(&mut w,EntityId(1),"us_f18e",1,&AirMission::Recon {point:Vec2::new(50.0,0.0)}).unwrap();
    let plane=*w.entities.keys().last().unwrap();w.entities.get_mut(&plane).unwrap().fire_on=false;
    let mount=w.entities[&plane].weapons.iter().find(|m|w.db.weapons[&m.weapon].kind==harpoon_core::db::WeaponKind::Aam).unwrap().clone();
    assert!(!w.db.weapons[&mount.weapon].targets.surface);
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::Fire {unit:plane,contact:harpoon_core::ContactId(1),weapon:mount.weapon.clone(),rounds:1})]);
    assert_eq!(w.entities[&plane].weapons.iter().find(|m|m.weapon==mount.weapon).unwrap().ammo,mount.ammo,"an incompatible manual command must not fire or consume ammunition");
}

#[test]
fn aircraft_radar_horizon_uses_actual_altitude_not_the_sensor_nominal_mount_height() {
    let mut w=setup();harpoon_core::air::launch_aircraft(&mut w,EntityId(1),"us_f18e",1,&AirMission::Recon {point:Vec2::new(50.0,0.0)}).unwrap();
    let id=*w.entities.keys().last().unwrap();let mut plane=w.entities[&id].clone();let sensor=plane.sensors.iter().find(|s|w.db.sensors[&s.sensor].kind==harpoon_core::db::SensorKind::Radar).unwrap().clone();
    plane.altitude_m=3000.0;let high=harpoon_core::sensors::nominal_ranges(&w.db,&plane,&sensor).1.unwrap();
    plane.altitude_m=50.0;let low=harpoon_core::sensors::nominal_ranges(&w.db,&plane,&sensor).1.unwrap();
    assert!(high>low*1.5,"lowering the actual aircraft must shorten its horizon, subject to the nominal range cap: {high} vs {low}");
}

use harpoon_core::commands::{AirMission, Command};
use harpoon_core::geom::Vec2;
use harpoon_core::scenario::{load_scenario, scenario_from_json};
use harpoon_core::types::Side;
use harpoon_core::Database;

fn world() -> harpoon_core::World {
    let src = r#"{
      "id":"air-mission-test","name":"Air mission test","duration_s":3600,"seed":7,
      "context":{"air_wings":{"CVN":[{"class":"us_f18e","count":6}]}},
      "weather":{"visibility_nm":40,"jam":[0,0]},
      "units":[
        {"side":"Blue","class":"us_ford","name":"CVN","pos":[0,0]},
        {"side":"Red","class":"cn_055","name":"TARGET","pos":[40,0]}]}"#;
    let db: Database = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    load_scenario(scenario_from_json(src).unwrap(), db).unwrap()
}

fn carrier_id(w: &harpoon_core::World) -> harpoon_core::EntityId {
    w.entities.values().find(|e| e.name == "CVN").unwrap().id
}

#[test]fn a_plotted_departure_route_exists_at_birth_and_bad_routes_do_not_consume_aircraft() {
    let mut w=world();let carrier=carrier_id(&w);let point=Vec2::new(15.0,8.0);
    let profile=harpoon_core::commands::LaunchProfile {waypoints:vec![point],altitude_m:1500.0,weapons_free:false,manual_strike:true,engagement:None};
    harpoon_core::air::launch_planned_aircraft(&mut w,carrier,"us_f18e",1,&AirMission::Strike{contact:harpoon_core::ContactId(99)},Some(&profile)).unwrap();
    let air=w.entities.values().find(|u|u.kind==harpoon_core::types::PlatformKind::Aircraft).unwrap().id;
    assert_eq!(w.entity(air).unwrap().waypoints.front(),Some(&point));
    let before=w.entity(carrier).unwrap().deck.as_ref().unwrap().ready.clone();
    let mut bad=profile.clone();bad.waypoints[0].x=f64::NAN;
    assert!(harpoon_core::air::launch_planned_aircraft(&mut w,carrier,"us_f18e",1,&AirMission::Rtb,Some(&bad)).is_err());
    assert_eq!(w.entity(carrier).unwrap().deck.as_ref().unwrap().ready,before);
    let distance=w.entity(air).unwrap().pos.range_to(point);
    harpoon_core::sim::tick(&mut w,&[]);assert!(w.entity(air).unwrap().pos.range_to(point)<distance);
}

fn tick(world: &mut harpoon_core::World, cmds: &[(Side, Command)]) {
    harpoon_core::sim::tick(world, cmds);
}

#[test]
fn plotted_profile_holds_release_at_launch_and_individual_recall_leaves_cap_on_station() {
    let mut w=world();let carrier=carrier_id(&w);
    w.entity_mut(carrier).unwrap().deck.as_mut().unwrap().ready.push(("us_f18e_asm".into(),2));
    let command=Command::LaunchAircraft {unit:carrier,class:"us_f18e_asm".into(),count:1,
        mission:AirMission::Strike {contact:harpoon_core::ContactId(999)},
        profile:Some(harpoon_core::commands::LaunchProfile {waypoints:Vec::new(),altitude_m:50.0,weapons_free:false,manual_strike:true,engagement:None})};
    tick(&mut w,&[(Side::Blue,command)]);
    let air=w.entities.values().find(|e|e.kind==harpoon_core::types::PlatformKind::Aircraft).unwrap().id;
    let u=w.entity(air).unwrap();
    assert_eq!(u.altitude_m,50.0);assert_eq!(u.ordered_altitude_m,50.0);assert!(!u.fire_on);
    assert!(matches!(u.mission,Some(harpoon_core::world::MissionState::Strike {..})),"manual flight must not resolve a hidden target");
    assert!(!w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::WeaponsRelease {shooter,..} if *shooter==air)));
    harpoon_core::air::launch_aircraft(&mut w,carrier,"us_f18e",1,&AirMission::Cap {station:Vec2::zero(),radius_nm:15.0,follow:true}).unwrap();
    let cap=w.entities.values().find(|e|e.kind==harpoon_core::types::PlatformKind::Aircraft && e.id!=air).unwrap().id;
    // Old API retains its exact initial spawn/ordered altitude and ROE.
    assert_eq!(w.entity(cap).unwrap().altitude_m,6000.0);
    assert_eq!(w.entity(cap).unwrap().ordered_altitude_m,8000.0);
    assert!(w.entity(cap).unwrap().fire_on);
    tick(&mut w,&[(Side::Red,Command::ReturnToBase {unit:air})]);
    assert!(matches!(w.entity(air).unwrap().mission,Some(harpoon_core::world::MissionState::Strike {..})));
    w.entity_mut(air).unwrap().pos=Vec2::new(10.0,0.0);
    tick(&mut w,&[(Side::Blue,Command::ReturnToBase {unit:air})]);
    assert!(matches!(w.entity(air).unwrap().mission,Some(harpoon_core::world::MissionState::Rtb)));
    assert!(matches!(w.entity(cap).unwrap().mission,Some(harpoon_core::world::MissionState::Cap {..})));
    assert!(w.entity(air).unwrap().waypoints.is_empty());
}

#[test]
fn launch_profile_is_optional_on_wire_and_invalid_profile_cannot_consume_deck_inventory() {
    let old:Command=serde_json::from_str(r#"{"cmd":"launch_aircraft","unit":1,"class":"fighter","count":1,"mission":{"mission":"rtb"}}"#).unwrap();
    assert!(matches!(old,Command::LaunchAircraft {profile:None,..}));
    let mut w=world();let carrier=carrier_id(&w);let before=w.entity(carrier).unwrap().deck.clone().unwrap().ready;
    let invalid=harpoon_core::commands::LaunchProfile {waypoints:Vec::new(),altitude_m:f64::NAN,weapons_free:false,manual_strike:false,engagement:None};
    assert!(harpoon_core::air::launch_planned_aircraft(&mut w,carrier,"us_f18e",1,&AirMission::Rtb,Some(&invalid)).is_err());
    assert_eq!(w.entity(carrier).unwrap().deck.as_ref().unwrap().ready,before);
}

#[test]
fn launch_policy_applies_before_any_automatic_weapon_selection() {
    let mut restricted=world();let cv=carrier_id(&restricted);
    restricted.entity_mut(cv).unwrap().deck.as_mut().unwrap().ready.push(("us_f18e_asm".into(),2));
    let profile=harpoon_core::commands::LaunchProfile {waypoints:Vec::new(),altitude_m:3000.0,weapons_free:true,manual_strike:false,
        engagement:Some(harpoon_core::commands::LaunchEngagement {air_only:true,max_in_flight:2})};
    harpoon_core::air::launch_planned_aircraft(&mut restricted,cv,"us_f18e_asm",1,
        &AirMission::Cap {station:Vec2::new(30.0,0.0),radius_nm:5.0,follow:false},Some(&profile)).unwrap();
    let air=restricted.entities.values().find(|e|e.kind==harpoon_core::types::PlatformKind::Aircraft).unwrap().id;
    // A valid fixture track isolates launch policy from sensor acquisition.
    let target=restricted.entities.values().find(|e|e.side==Side::Red).unwrap().clone();
    let contact=harpoon_core::ContactId(1);
    restricted.sides[0].next_contact=1;
    restricted.sides[0].contacts.insert(contact,harpoon_core::world::Contact {id:contact,about:target.id,
        category:target.kind,class_name:None,estimate:target.pos,est_course_deg:0.0,est_speed_kts:0.0,
        bearing_only:false,quality:1.0,last_update_tick:0,first_tick:0,stale:false,sensors:vec!["fixture".into()],
        offset:Vec2::zero(),announced:false});
    let mut unrestricted=restricted.clone();unrestricted.entity_mut(air).unwrap().ext.insert("doctrine.air_only".into(),0.0);
    for _ in 0..60 {tick(&mut restricted,&[]);tick(&mut unrestricted,&[]);}
    let releases=|w:&harpoon_core::World|w.events.iter().filter(|e|matches!(&e.kind,
        harpoon_core::EventKind::WeaponsRelease{shooter,weapon,..} if *shooter==air && *weapon=="AGM-84D Harpoon IC")).count();
    assert!(releases(&unrestricted)>0,"control CAP must actually release an ASM; weapons={:?}, contacts={:?}",unrestricted.entity(air).unwrap().weapons,unrestricted.sides[0].contacts);
    assert_eq!(releases(&restricted),0,"restricted CAP must retain its ASMs from the first tick");
    let before=restricted.entity(cv).unwrap().deck.as_ref().unwrap().ready.clone();
    let mut invalid=profile;invalid.engagement.as_mut().unwrap().max_in_flight=33;
    assert!(harpoon_core::air::launch_planned_aircraft(&mut restricted,cv,"us_f18e_asm",1,&AirMission::Rtb,Some(&invalid)).is_err());
    assert_eq!(restricted.entity(cv).unwrap().deck.as_ref().unwrap().ready,before);
}

#[test]
fn follow_cap_tracks_the_moving_carrier() {
    let mut w = world();
    let carrier = carrier_id(&w);
    let cap = AirMission::Cap {
        station: Vec2::new(0.0, 0.0),
        radius_nm: 25.0,
        follow: true,
    };
    tick(&mut w, &[(Side::Blue, Command::LaunchAircraft { profile: None,
        unit: carrier, class: "us_f18e".into(), count: 1, mission: cap,
    })]);
    // Let the aircraft get airborne and settle on the station.
    for _ in 0..30 { tick(&mut w, &[]); }

    // Steam the carrier 60 nm east; the follow CAP must come along.
    for _ in 0..240 {
        tick(&mut w, &[(Side::Blue, Command::SetCourse { unit: carrier, course_deg: 90.0 })]);
    }
    let carrier_pos = w.entity(carrier).unwrap().pos;
    let cap_pos = w
        .entities
        .values()
        .find(|e| e.kind == harpoon_core::types::PlatformKind::Aircraft)
        .expect("CAP airborne")
        .pos;
    assert!(
        cap_pos.range_to(carrier_pos) < 40.0,
        "follow CAP must shadow the carrier (carrier {carrier_pos:?}, cap {cap_pos:?})"
    );
}

#[test]
fn static_cap_stays_on_its_station_when_follow_is_absent() {
    let mut w = world();
    let carrier = carrier_id(&w);
    let cap = AirMission::Cap {
        station: Vec2::new(0.0, 0.0),
        radius_nm: 25.0,
        follow: false,
    };
    tick(&mut w, &[(Side::Blue, Command::LaunchAircraft { profile: None,
        unit: carrier, class: "us_f18e".into(), count: 1, mission: cap,
    })]);
    for _ in 0..30 { tick(&mut w, &[]); }
    for _ in 0..240 {
        tick(&mut w, &[(Side::Blue, Command::SetCourse { unit: carrier, course_deg: 90.0 })]);
    }
    let cap_pos = w
        .entities
        .values()
        .find(|e| e.kind == harpoon_core::types::PlatformKind::Aircraft)
        .expect("CAP airborne")
        .pos;
    assert!(cap_pos.x < 45.0, "a station CAP must hold its original station, saw {cap_pos:?}");
}

#[test]
fn refused_launch_reports_the_reason_to_the_ordering_side() {
    let mut w = world();
    let carrier = carrier_id(&w);
    tick(&mut w, &[(Side::Blue, Command::LaunchAircraft { profile: None,
        unit: carrier,
        class: "us_f18e".into(),
        count: 99,
        mission: AirMission::Cap {
            station: Vec2::zero(), radius_nm: 25.0, follow: true,
        },
    })]);
    let refused = w
        .events
        .iter()
        .find(|e| matches!(e.kind, harpoon_core::events::EventKind::DeckBusy { .. }))
        .expect("refusal event");
    match &refused.kind {
        harpoon_core::events::EventKind::DeckBusy { reason, side, .. } => {
            assert_eq!(*side, Side::Blue);
            assert!(reason.contains("ready"), "reason must say what is available, got {reason:?}");
        }
        _ => unreachable!(),
    }
}

#[test]
fn strike_without_anti_ship_ordnance_returns_instead_of_orbiting() {
    let mut w = world();
    let carrier = carrier_id(&w);
    // A CAP detachment re-tasked as a strike: no ASM in the loadout.
    tick(&mut w, &[(Side::Blue, Command::LaunchAircraft { profile: None,
        unit: carrier, class: "us_f18e".into(), count: 1,
        mission: AirMission::Cap { station: Vec2::zero(), radius_nm: 25.0, follow: false },
    })]);
    for _ in 0..30 { tick(&mut w, &[]); }
    let air = w
        .entities
        .values()
        .find(|e| e.kind == harpoon_core::types::PlatformKind::Aircraft)
        .expect("airborne")
        .id;
    let _ = air;
    // Re-task as a strike, then strip its ASMs to simulate a bomber that
    // cannot release: it must head home instead of orbiting the target.
    let air = w
        .entities
        .values()
        .find(|e| e.kind == harpoon_core::types::PlatformKind::Aircraft)
        .expect("airborne")
        .id;
    w.entity_mut(air).unwrap().weapons.clear();
    w.entity_mut(air).unwrap().mission = Some(harpoon_core::world::MissionState::Strike {
        contact: harpoon_core::types::ContactId(1),
        released: false,
    });
    // Seed contact 1 onto Blue's picture about the Red ship.
    {
        let state = &mut w.sides[Side::Blue.index()];
        state.next_contact += 1;
        let cid = harpoon_core::types::ContactId(state.next_contact);
        let about = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
        state.contacts.insert(cid, harpoon_core::world::Contact {
            id: cid, about, category: harpoon_core::types::PlatformKind::Ship,
            class_name: None, estimate: Vec2::new(40.0, 0.0), est_course_deg: 0.0,
            est_speed_kts: 0.0, bearing_only: false, quality: 1.0,
            last_update_tick: w.tick, first_tick: w.tick, stale: false,
            sensors: vec!["test".into()], offset: Vec2::zero(), announced: true,
        });
    }
    for _ in 0..600 { tick(&mut w, &[]); }
    let mission = w
        .entities
        .get(&air)
        .filter(|e| e.alive)
        .map(|e| e.mission.clone())
        .flatten();
    let still_orbiting = matches!(
        mission,
        Some(harpoon_core::world::MissionState::Strike { released: false, .. })
    );
    assert!(
        !still_orbiting,
        "a strike with no ASMs must head home, not orbit; saw {mission:?}"
    );
}

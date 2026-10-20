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

fn tick(world: &mut harpoon_core::World, cmds: &[(Side, Command)]) {
    harpoon_core::sim::tick(world, cmds);
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
    tick(&mut w, &[(Side::Blue, Command::LaunchAircraft {
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
    tick(&mut w, &[(Side::Blue, Command::LaunchAircraft {
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
    tick(&mut w, &[(Side::Blue, Command::LaunchAircraft {
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
    tick(&mut w, &[(Side::Blue, Command::LaunchAircraft {
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

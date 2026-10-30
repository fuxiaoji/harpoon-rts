//! Independent shared-entry checks; authored launch fit/knowledge are explicit.
use harpoon_core::{Command, ContactId, Database, EntityId, Scenario, Side, Vec2, World};
use serde_json::json;
const EXACT: &str = "us_rgm84d_ic";
fn fixture(bound: bool) -> (World, EntityId, EntityId) {
    let mut db = Database::built_in();
    let mut hull = db.platforms["ffg"].clone();
    hull.id = "scenario_assumption:ic_entry_launcher".into();
    hull.sensors.clear();
    hull.deck = None;
    hull.weapons.truncate(1);
    hull.weapons[0].weapon = EXACT.into();
    hull.weapons[0].ammo = 4;
    db.platforms.insert(hull.id.clone(), hull);
    let config = if bound {
        json!({
            "source":"scenario_assumption: isolated entry/knowledge fixture",
            "launchers":[{"actor_name":"OWN","weapon":EXACT,"exact_variant":"RGM-84D Harpoon IC surface launch","max_rounds_per_turn":4,"source":"scenario_assumption: explicit test mount and per-turn release capacity"}],
            "author_fcs":[{"actor_name":"OWN","contact":71,"weapon":EXACT,"quality":"Good","source":"scenario_assumption: explicit own Good solution"}]
        })
    } else {
        json!({})
    };
    let sc:Scenario=serde_json::from_value(json!({
        "id":"ascm-entry-boundary","name":"scenario_assumption: source entry boundary","duration_s":2000,"seed":123,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},"radar_land_clutter":[],"antiship_2026":config},
        "units":[{"side":"Blue","class":"scenario_assumption:ic_entry_launcher","name":"OWN","pos":[0,0],"speed_kts":0},
            {"side":"Red","class":"ffg","name":"SECRET","pos":[0,60],"speed_kts":0},
            {"side":"Red","class":"ffg","name":"scenario_assumption: live reserve","pos":[1000,1000],"speed_kts":0}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let own = w.entities.values().find(|e| e.name == "OWN").unwrap().id;
    let secret = w.entities.values().find(|e| e.name == "SECRET").unwrap().id;
    for e in w.entities.values_mut() {
        e.sensors.clear();
        if e.id != own {
            e.weapons.clear();
        }
    }
    w.sides[0].contacts.insert(
        ContactId(71),
        harpoon_core::world::Contact {
            id: ContactId(71),
            about: secret,
            category: harpoon_core::types::PlatformKind::Ship,
            class_name: None,
            estimate: Vec2::new(0., 60.),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec!["scenario_assumption: located ship report".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", secret.0),
        harpoon_core::types::PlatformKind::Ship,
    );
    (w, own, secret)
}
fn plot(w: &mut World, own: EntityId) {
    let command:Command=serde_json::from_value(json!({"cmd":"antiship","unit":own,"action":{"type":"plot","contact":71,"weapon":EXACT,"course_deg":0,"speed_kts":561,"enable_run_nm":40,"waypoints":[],"popup":false}})).unwrap();
    harpoon_core::sim::tick(w, &[(Side::Blue, command)]);
}
fn fire(own: EntityId) -> Command {
    Command::Fire {
        unit: own,
        contact: ContactId(71),
        weapon: EXACT.into(),
        rounds: 1,
    }
}

#[test]
fn a_naked_v4_launch_cannot_create_an_unregistered_source_missile() {
    let (mut w, own, secret) = fixture(true);
    let def = w.db.weapons[EXACT].clone();
    assert!(harpoon_core::weapons::launch(&mut w, own, &def, Some(secret), None).is_none());
    assert_eq!(w.stats.projectiles_launched, 0);
    assert_eq!(w.entity(own).unwrap().weapons[0].ammo, 4);
}
#[test]
fn unbound_v4_manual_assigned_free_and_air_strike_hold_ammunition() {
    let (base, own, _) = fixture(false);
    for route in ["manual", "assigned", "free", "air"] {
        let mut w = base.clone();
        match route {
            "manual" => harpoon_core::sim::tick(&mut w, &[(Side::Blue, fire(own))]),
            "assigned" => {
                w.entity_mut(own).unwrap().assigned = Some(ContactId(71));
                harpoon_core::weapons::fire_control_scan(&mut w);
            }
            "free" => {
                w.entity_mut(own).unwrap().fire_on = true;
                harpoon_core::weapons::fire_control_scan(&mut w);
            }
            "air" => {
                let e = w.entity_mut(own).unwrap();
                e.kind = harpoon_core::types::PlatformKind::Aircraft;
                e.altitude_m = 500.;
                e.mission = Some(harpoon_core::world::MissionState::Strike {
                    contact: ContactId(71),
                    released: false,
                });
                harpoon_core::air::update_air_ops(&mut w);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            w.stats.projectiles_launched, 0,
            "unbound route {route} must not use legacy pk"
        );
        assert_eq!(
            w.entity(own).unwrap().weapons[0].ammo,
            4,
            "unbound route {route} must not clear the mount"
        );
    }
}
#[test]
fn a_valid_own_release_is_independent_of_the_unobserved_target_body() {
    let (base, own, secret) = fixture(true);
    let mut outcomes = vec![];
    for hidden_changed in [false, true] {
        let mut w = base.clone();
        if hidden_changed {
            let t = w.entity_mut(secret).unwrap();
            t.alive = false;
            t.kind = harpoon_core::types::PlatformKind::Aircraft;
            t.pos = Vec2::new(999., 999.);
            t.class = "scenario_assumption: secret unavailable source metadata".into();
        }
        plot(&mut w, own);
        harpoon_core::sim::tick(&mut w, &[(Side::Blue, fire(own))]);
        assert_eq!(
            w.stats.projectiles_launched, 1,
            "own report/FCS permits release before physical search"
        );
        let observation = harpoon_core::observe(&w, Side::Blue, 0, 0);
        outcomes.push((
            serde_json::to_value(&observation.units).unwrap(),
            serde_json::to_value(&observation.events).unwrap(),
        ));
    }
    assert_eq!(
        outcomes[0], outcomes[1],
        "secret body cannot change immediate own receipts or orders"
    );
}
#[test]
fn a_fresh_duplicate_report_does_not_repair_a_stale_selected_contact() {
    let (mut w, own, _) = fixture(true);
    let mut duplicate = w.contact(Side::Blue, ContactId(71)).unwrap().clone();
    duplicate.id = ContactId(72);
    w.sides[0].contacts.insert(ContactId(72), duplicate);
    w.sides[0].contacts.get_mut(&ContactId(71)).unwrap().stale = true;
    plot(&mut w, own);
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, fire(own))]);
    assert_eq!(w.stats.projectiles_launched, 0);
    assert_eq!(w.entity(own).unwrap().weapons[0].ammo, 4);
}

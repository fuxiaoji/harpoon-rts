//! Shared game-AI task and resource accounting. Only receives a player's
//! observation and public database. It never adjudicates weapons or adds intel.
use super::*;
use harpoon_core::{geom::Vec2, observe::ContactObs};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Supervisor {
    #[serde(default)]
    pub tasks: BTreeMap<EntityId, Task>,
    #[serde(default)]
    pub attacks: BTreeMap<ContactId, Attack>,
    #[serde(default)]
    ammo: BTreeMap<String, Ammo>,
    #[serde(default)]
    intents: BTreeMap<String, ContactId>,
    #[serde(default)]
    previous: BTreeMap<EntityId, Seen>,
    #[serde(default)]
    unavailable: Vec<Unavailable>,
    #[serde(default)]
    pub ew: BTreeMap<EntityId, Support>,
    #[serde(default)]
    pub asw: BTreeMap<EntityId, Search>,
    #[serde(default)]
    last_tick: Option<i64>,
    #[serde(default)]
    initial_sam: BTreeMap<EntityId, u32>,
    #[serde(default)]
    notices: BTreeMap<String, i64>,
    #[serde(default)]
    unattributed_rounds: BTreeMap<String, u32>,
    #[serde(default)]
    flights: Vec<FlightOrder>,
    #[serde(default)]
    origins: BTreeMap<EntityId, EntityId>,
    #[serde(default)]
    expenditure_by_unit: BTreeMap<EntityId, u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    lost_tracks: BTreeMap<EntityId, i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    scheduled: Vec<QueuedRelease>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    auto_defence: BTreeMap<EntityId, AutoDefence>,
}
#[derive(Clone,Serialize,Deserialize)]
struct AutoDefence {start_stock:u32,allowance:u32,until:i64}
fn sam_stock(u:&UnitObs,db:&Database)->u32 {
    u.weapons.iter().filter(|m|db.weapons.get(&m.weapon).is_some_and(|w|w.kind==WeaponKind::Sam)).map(|m|m.ammo as u32).sum()
}
fn immediate_air_threat(obs:&Observation,u:&UnitObs)->bool {
    obs.contacts.iter().any(|c|fresh(c)&&matches!(c.category,PlatformKind::Missile|PlatformKind::Aircraft)&&u.pos.range_to(c.estimate)<30.0)
}
#[derive(Clone, Serialize, Deserialize)]
struct QueuedRelease {
    unit: EntityId,
    contact: ContactId,
    weapon: String,
    rounds: u8,
    fire_at: i64,
    deadline: i64,
}
#[derive(Clone, Serialize, Deserialize)]
struct FlightOrder {
    home: EntityId,
    class: String,
    target: Option<ContactId>,
    until: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_existing_cap_enabling_free_fire_cannot_dump_anti_ship_weapons() {
        let (db,mut w,_)=setup();
        {let target=w.entities.get_mut(&EntityId(2)).unwrap();target.side=Side::Red;target.pos=Vec2::new(40.0,0.0);}
        w.sides[0].contacts.insert(ContactId(1),serde_json::from_value(json!({"id":1,"about":2,"category":"Ship","class_name":"Type 055","estimate":{"x":40,"y":0},"est_course_deg":0,"est_speed_kts":0,"quality":1,"bearing_only":false,"last_update_tick":0,"first_tick":0,"stale":false,"sensors":[],"offset":{"x":0,"y":0},"announced":true})).unwrap());
        harpoon_core::air::launch_aircraft(&mut w,EntityId(1),"us_f18e_asm",1,&AirMission::Cap{station:Vec2::zero(),radius_nm:10.0,follow:true}).unwrap();
        let aid=w.entities.values().find(|u|u.kind==PlatformKind::Aircraft).unwrap().id;
        let mut baseline=w.clone();
        let o=harpoon_core::observe(&w,Side::Blue,0,256);let mut p=Supervisor::default();p.observe(&o,&db);
        let requested=Command::SetWeaponsFree{unit:aid,free:true};
        let orders=p.guard(&o,&db,vec![requested.clone()]).0;
        for i in 0..30 {
            harpoon_core::sim::tick(&mut w,&if i==0 {orders.iter().cloned().map(|c|(Side::Blue,c)).collect::<Vec<_>>()}else{vec![]});
            harpoon_core::sim::tick(&mut baseline,&if i==0 {vec![(Side::Blue,requested.clone())]}else{vec![]});
        }
        let released=|w:&harpoon_core::World|w.events.iter().filter_map(|e|match &e.kind{harpoon_core::EventKind::WeaponsRelease{shooter,weapon,count,..}if *shooter==aid&&weapon.contains("Harpoon")=>Some(*count as u32),_=>None}).sum::<u32>();
        assert!(released(&baseline)>0,"unprotected pre-existing CAP reproduces unintended anti-ship fire");
        assert_eq!(released(&w),0,"accepted free-fire command must install CAP policy first");
    }
    #[test]
    fn automatic_defence_budget_is_enforced_between_model_decisions_and_survives_reload() {
        let (db,_,mut o)=setup();let id=EntityId(2);let mut p=Supervisor::default();p.observe(&o,&db);
        p.guard(&o,&db,vec![Command::SetWeaponsFree{unit:id,free:true},Command::Goto{unit:id,waypoints:vec![Vec2::new(10.0,0.0)]}]);
        let quota=p.auto_defence[&id].allowance;let mut remaining=quota;
        for m in &mut o.units[1].weapons {if db.weapons[&m.weapon].kind==WeaponKind::Sam {let n=(m.ammo as u32).min(remaining);m.ammo-=n as u16;remaining-=n;}}
        assert_eq!(remaining,0);o.units[1].fire_on=true;o.units[1].assigned=Some(ContactId(1));o.contacts[0].category=PlatformKind::Aircraft;o.contacts[0].estimate=Vec2::new(80.0,0.0);o.tick=30;
        let mut restored:Supervisor=serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        let a=p.maintenance(&o,&db).0;let b=restored.maintenance(&o,&db).0;
        assert_eq!(serde_json::to_value(&a).unwrap(),serde_json::to_value(&b).unwrap());
        assert!(a.iter().any(|c|matches!(c,Command::SetWeaponsFree{unit,free:false}if *unit==id)));
        assert!(a.iter().any(|c|matches!(c,Command::ClearTarget{unit}if *unit==id)),"an assigned target also causes repeated automatic fire");
        assert!(p.guard(&o,&db,vec![Command::AssignTarget{unit:id,contact:ContactId(1)}]).0.is_empty());
        p.review_current(&o,&db);assert_eq!(p.tasks[&id].expires_at,630,"a fire budget must not accidentally expire navigation");
        o.contacts[0].category=PlatformKind::Missile;o.contacts[0].estimate=Vec2::new(10.0,0.0);
        assert!(!p.maintenance(&o,&db).0.iter().any(|c|matches!(c,Command::SetWeaponsFree{unit,free:false}if *unit==id)));
        p.release_manual(id);assert!(!p.auto_defence.contains_key(&id));
    }
    #[test]
    fn mixed_source_batch_waits_for_the_window_and_survives_reload_with_actual_terminal_measurements(
    ) {
        let db =
            Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
        let sc=harpoon_core::scenario_from_json(r#"{"id":"timed-batch","name":"abstract fixture","duration_s":2000,"seed":10,"weather":{"visibility_nm":1,"jam":[0,0]},"context":{"air_wings":{"HOME":[{"class":"us_f18e_asm","count":1}]}},"units":[{"side":"Blue","class":"us_ford","name":"HOME","pos":[10,0]},{"side":"Blue","class":"cn_055","pos":[0,0]},{"side":"Red","class":"us_ford","pos":[75,0]}]}"#).unwrap();
        let mut w = harpoon_core::load_scenario(sc, db.clone()).unwrap();
        let db=w.db.clone();
        w.sides[0].contacts.insert(
            ContactId(1),
            harpoon_core::world::Contact {
                id: ContactId(1),
                about: EntityId(3),
                category: PlatformKind::Ship,
                class_name: None,
                estimate: Vec2::new(75.0, 0.0),
                est_course_deg: 0.0,
                est_speed_kts: 0.0,
                quality: 1.0,
                bearing_only: false,
                last_update_tick: 0,
                first_tick: 0,
                stale: false,
                sensors: vec!["fixture".into()],
                offset: Vec2::zero(),
                announced: true,
            },
        );
        harpoon_core::air::launch_planned_aircraft(
            &mut w,
            EntityId(1),
            "us_f18e_asm",
            1,
            &AirMission::Strike {
                contact: ContactId(1),
            },
            Some(&harpoon_core::commands::LaunchProfile {
                altitude_m: 81.0,
                weapons_free: false,
                manual_strike: true,
                engagement: None,
                waypoints: vec![],
            }),
        )
        .unwrap();
        for u in w.entities.values_mut() {
            u.fire_on = false;
            if u.side == Side::Red {
                u.chaff = 0;
                u.decoys = 0;
                for m in &mut u.weapons {
                    m.ammo = 0;
                }
            }
        }
        let aid = w
            .entities
            .values()
            .find(|u| u.kind == PlatformKind::Aircraft)
            .unwrap()
            .id;
        let asm = db.platforms["us_f18e_asm"]
            .weapons
            .iter()
            .find(|m| db.weapons[&m.weapon].kind == WeaponKind::Ascm)
            .unwrap()
            .weapon
            .clone();
        let mut p = Supervisor::default();
        let o = harpoon_core::observe(&w, Side::Blue, 0, 256);
        p.observe(&o, &db);
        let (orders, _) = p.guard(
            &o,
            &db,
            vec![
                Command::Fire {
                    unit: EntityId(2),
                    contact: ContactId(1),
                    weapon: "cn_yj18".into(),
                    rounds: 2,
                },
                Command::Fire {
                    unit: aid,
                    contact: ContactId(1),
                    weapon: asm.clone(),
                    rounds: 2,
                },
            ],
        );
        assert_eq!(orders.len(), 1);
        assert_eq!(p.scheduled.len(), 1);
        assert!(p.attacks.is_empty(), "queuing is not expenditure");
        let mut restored: Supervisor =
            serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        assert_eq!(p.summary(&o), restored.summary(&o));
        let mut start = orders;
        let mut terminal: BTreeMap<String, i64> = BTreeMap::new();
        let mut actual: BTreeMap<String, u32> = BTreeMap::new();
        let mut seq = w.event_seq;
        for _ in 0..1100 {
            let now = w.tick;
            let c = w.sides[0].contacts.get_mut(&ContactId(1)).unwrap();
            c.quality = 1.0;
            c.last_update_tick = now;
            c.stale = false;
            let o = harpoon_core::observe(&w, Side::Blue, 0, 256);
            p.observe(&o, &db);
            restored.observe(&o, &db);
            if now % 30 == 0 {
                let (a, _) = p.maintenance(&o, &db);
                let (b, _) = restored.maintenance(&o, &db);
                assert_eq!(
                    serde_json::to_value(&a).unwrap(),
                    serde_json::to_value(&b).unwrap()
                );
                start.extend(a);
            }
            harpoon_core::sim::tick(
                &mut w,
                &std::mem::take(&mut start)
                    .into_iter()
                    .map(|c| (Side::Blue, c))
                    .collect::<Vec<_>>(),
            );
            for e in w.events.iter().filter(|e| e.seq > seq) {
                if let harpoon_core::EventKind::WeaponsRelease { weapon, count, .. } = &e.kind {
                    *actual.entry(weapon.clone()).or_default() += *count as u32;
                }
            }
            seq = w.event_seq;
            for u in w
                .entities
                .values()
                .filter(|u| u.alive && u.kind == PlatformKind::Missile)
            {
                if let Some(projectile) = &u.projectile {
                    if ["cn_yj18", asm.as_str()].contains(&projectile.weapon.as_str())
                        && u.pos.range_to(Vec2::new(75.0, 0.0)) < 10.0
                    {
                        terminal.entry(projectile.weapon.clone()).or_insert(w.tick);
                    }
                }
            }
        }
        assert_eq!(actual[&db.weapons["cn_yj18"].name], 2);
        assert_eq!(actual[&db.weapons[&asm].name], 2);
        assert_eq!(terminal.len(), 2);
        assert!((terminal["cn_yj18"]-terminal[&asm]).abs()<=60,"actual mixed-source terminal entries must meet within the documented60s window: {terminal:?}");
        let mut stale = o.clone();
        stale.tick = 1000;
        stale.contacts[0].stale = true;
        p.scheduled.push(QueuedRelease {
            unit: EntityId(2),
            contact: ContactId(1),
            weapon: "cn_yj18".into(),
            rounds: 2,
            fire_at: 900,
            deadline: 950,
        });
        assert!(p
            .maintenance(&stale, &db)
            .0
            .iter()
            .all(|c| !matches!(c, Command::Fire { .. })));
        assert!(p.scheduled.is_empty());
    }
    #[test]
    fn temporary_air_self_defence_does_not_replace_the_strike_task() {
        let (db, _, mut o) = setup();
        o.units[1].kind = PlatformKind::Aircraft;
        o.units[1].mission = Some("strike".into());
        let mut air = o.contacts[0].clone();
        air.id = ContactId(2);
        air.category = PlatformKind::Aircraft;
        o.contacts.push(air);
        let mut p = Supervisor::default();
        p.bind_strike(EntityId(2), Some(ContactId(1)), 0);
        p.guard(
            &o,
            &db,
            vec![
                Command::ClearTarget { unit: EntityId(2) },
                Command::AssignTarget {
                    unit: EntityId(2),
                    contact: ContactId(2),
                },
            ],
        );
        assert_eq!(p.tasks[&EntityId(2)].contact, Some(ContactId(1)));
    }
    #[test]
    fn actual_release_receipt_of_a_lost_shooter_is_counted_once() {
        let (db, mut w, mut o) = setup();
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        p.guard(
            &o,
            &db,
            vec![Command::Fire {
                unit: EntityId(2),
                contact: ContactId(1),
                weapon: "cn_yj18".into(),
                rounds: 2,
            }],
        );
        w.tick = 1;
        w.emit(
            harpoon_core::EventKind::WeaponsRelease {
                side: Side::Blue,
                shooter: EntityId(2),
                shooter_name: "fixture".into(),
                weapon: db.weapons["cn_yj18"].name.clone(),
                count: 2,
            },
            harpoon_core::Visibility::Side(Side::Blue),
        );
        o.events = harpoon_core::observe(&w, Side::Blue, 0, 256).events;
        o.tick = 30;
        o.units.retain(|u| u.id != EntityId(2));
        p.observe(&o, &db);
        assert_eq!(p.attacks[&ContactId(1)].rounds, 2);
        o.tick = 60;
        p.observe(&o, &db);
        assert_eq!(p.attacks[&ContactId(1)].rounds, 2);
    }
    #[test]
    fn receipt_and_magazine_delta_are_one_expenditure_and_evasion_is_not_overwritten() {
        let (db, mut w, mut o) = setup();
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        p.guard(
            &o,
            &db,
            vec![Command::Fire {
                unit: EntityId(2),
                contact: ContactId(1),
                weapon: "cn_yj18".into(),
                rounds: 2,
            }],
        );
        w.tick = 1;
        w.emit(
            harpoon_core::EventKind::WeaponsRelease {
                side: Side::Blue,
                shooter: EntityId(2),
                shooter_name: "fixture".into(),
                weapon: db.weapons["cn_yj18"].name.clone(),
                count: 2,
            },
            harpoon_core::Visibility::Side(Side::Blue),
        );
        o.events = harpoon_core::observe(&w, Side::Blue, 0, 256).events;
        o.tick = 30;
        o.units[1]
            .weapons
            .iter_mut()
            .find(|m| m.weapon == "cn_yj18")
            .unwrap()
            .ammo -= 2;
        p.observe(&o, &db);
        assert_eq!(p.attacks[&ContactId(1)].rounds, 2);
        o.units[1].kind = PlatformKind::Aircraft;
        o.units[1].deck = None;
        o.contacts[0].category = PlatformKind::Missile;
        o.contacts[0].estimate = o.units[1].pos;
        p.asw.insert(
            EntityId(2),
            Search {
                contact: None,
                area: Vec2::new(12.0, 0.0),
                until: 300,
            },
        );
        let (orders, _) = p.guard(
            &o,
            &db,
            vec![Command::RetaskAircraft {
                unit: EntityId(2),
                mission: AirMission::Asw {
                    area: Vec2::zero(),
                    radius_nm: 1.0,
                },
                manual: false,
            }],
        );
        assert!(
            matches!(&orders[0],Command::RetaskAircraft{mission:AirMission::Asw{area,..},..} if *area==Vec2::zero())
        );
    }
    fn setup() -> (Database, harpoon_core::World, Observation) {
        let db =
            Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
        let sc=harpoon_core::scenario_from_json(r#"{"id":"supervision-test","name":"test","duration_s":2000,"seed":21,"weather":{"visibility_nm":1,"jam":[0,0]},"context":{"air_wings":{"HOME":[{"class":"us_f18e","count":20},{"class":"us_f18e_asm","count":12}]}},"units":[{"side":"Blue","class":"us_ford","name":"HOME","pos":[0,0]},{"side":"Blue","class":"cn_055","name":"ESCORT","pos":[5,0]}]}"#).unwrap();
        let w = harpoon_core::load_scenario(sc, db.clone()).unwrap();
        let mut o = harpoon_core::observe(&w, Side::Blue, 0, 0);
        o.contacts.push(serde_json::from_value(json!({"id":1,"category":"Ship","estimate":{"x":40,"y":0},"est_course_deg":0,"est_speed_kts":0,"quality":0.8,"bearing_only":false,"last_seen_s":0,"stale":false,"sensors":[]})).unwrap());
        (db, w, o)
    }
    #[test]
    fn batch_budget_caps_cap_at_birth_and_does_not_double_book_a_deck() {
        let (db, _, o) = setup();
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        let launch = Command::LaunchAircraft {
            unit: EntityId(1),
            class: "us_f18e".into(),
            count: 16,
            mission: AirMission::Cap {
                station: Vec2::zero(),
                radius_nm: 10.0,
                follow: true,
            },
            profile: None,
        };
        let (orders, notes) = p.guard(&o, &db, vec![launch.clone(), launch]);
        assert_eq!(orders.len(), 1);
        let Command::LaunchAircraft { count, profile, .. } = &orders[0] else {
            panic!()
        };
        assert_eq!(*count, 2);
        assert!(
            profile
                .as_ref()
                .unwrap()
                .engagement
                .as_ref()
                .unwrap()
                .air_only
        );
        assert!(notes.iter().any(|n| n.action == "resource_hold"));
    }
    #[test]
    fn spending_is_actual_ammunition_change_and_unconfirmed_bda_pauses_identical_attacks() {
        let (db, _, mut o) = setup();
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        let fire = Command::Fire {
            unit: EntityId(2),
            contact: ContactId(1),
            weapon: "cn_yj18".into(),
            rounds: 2,
        };
        assert_eq!(p.guard(&o, &db, vec![fire.clone()]).0.len(), 1);
        assert!(p.attacks.is_empty(), "orders are not launches");
        o.tick = 30;
        o.units[1]
            .weapons
            .iter_mut()
            .find(|w| w.weapon == "cn_yj18")
            .unwrap()
            .ammo -= 2;
        p.observe(&o, &db);
        assert_eq!(p.attacks[&ContactId(1)].rounds, 2);
        o.tick = 240;
        p.observe(&o, &db);
        assert!(!p.target_allowed(ContactId(1), o.tick));
        assert!(p.guard(&o, &db, vec![fire]).0.is_empty());
        assert_eq!(p.attacks[&ContactId(1)].observed_damage, 0.0);
        assert_eq!(p.attacks[&ContactId(1)].unconfirmed_batches, 1);
    }
    #[test]
    fn same_observation_damage_is_credited_and_missing_shooter_is_not_counted_as_a_release() {
        let (db, _, mut o) = setup();
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        p.guard(
            &o,
            &db,
            vec![Command::Fire {
                unit: EntityId(2),
                contact: ContactId(1),
                weapon: "cn_yj18".into(),
                rounds: 2,
            }],
        );
        o.tick = 30;
        o.units[1]
            .weapons
            .iter_mut()
            .find(|w| w.weapon == "cn_yj18")
            .unwrap()
            .ammo -= 2;
        o.contacts[0].bda_damage = 52.0;
        o.contacts[0].bda_hits = 1;
        p.observe(&o, &db);
        o.tick = 240;
        p.observe(&o, &db);
        assert!(p.target_allowed(ContactId(1), o.tick));
        assert_eq!(p.attacks[&ContactId(1)].observed_hits, 1);
        let total = p.attacks[&ContactId(1)].rounds;
        o.tick = 270;
        o.units.retain(|u| u.id != EntityId(2));
        p.observe(&o, &db);
        assert_eq!(p.attacks[&ContactId(1)].rounds, total);
    }
    #[test]
    fn late_confirmed_damage_releases_a_hold_without_rewriting_the_original_expenditure() {
        let (db, _, mut o) = setup();
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        p.guard(
            &o,
            &db,
            vec![Command::Fire {
                unit: EntityId(2),
                contact: ContactId(1),
                weapon: "cn_yj18".into(),
                rounds: 2,
            }],
        );
        o.tick = 30;
        o.units[1]
            .weapons
            .iter_mut()
            .find(|w| w.weapon == "cn_yj18")
            .unwrap()
            .ammo -= 2;
        p.observe(&o, &db);
        o.tick = 240;
        p.observe(&o, &db);
        assert!(!p.target_allowed(ContactId(1), 240));
        o.tick = 270;
        o.contacts[0].bda_damage = 20.0;
        p.observe(&o, &db);
        assert!(p.target_allowed(ContactId(1), 270));
        assert_eq!(p.attacks[&ContactId(1)].rounds, 2);
    }
    #[test]
    fn one_recovery_receipt_does_not_excuse_multiple_disappearing_support_aircraft() {
        let (db, mut w, o) = setup();
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        for id in [91, 92] {
            p.previous.insert(
                EntityId(id),
                Seen {
                    class: "us_mh60r".into(),
                    pos: Vec2::zero(),
                    home: Some(EntityId(1)),
                    support: true,
                    rtb: false,
                },
            );
        }
        w.tick = 1;
        w.emit(
            harpoon_core::EventKind::AirRecovered {
                side: Side::Blue,
                carrier: EntityId(1),
                class: db.platforms["us_mh60r"].name.clone(),
                count: 1,
            },
            harpoon_core::Visibility::Side(Side::Blue),
        );
        p.observe(&harpoon_core::observe(&w, Side::Blue, 0, 256), &db);
        assert_eq!(p.unavailable.len(), 1);
    }
    #[test]
    fn shared_inflight_allocation_counts_the_entire_batch() {
        let (db, _, mut o) = setup();
        o.contacts[0].own_in_flight = 6;
        let mut p = Supervisor::default();
        let fire = Command::Fire {
            unit: EntityId(2),
            contact: ContactId(1),
            weapon: "cn_yj18".into(),
            rounds: 4,
        };
        let (orders, _) = p.guard(&o, &db, vec![fire.clone(), fire]);
        assert_eq!(orders.len(), 1);
        assert!(matches!(orders[0], Command::Fire { rounds: 2, .. }));
    }
    #[test]
    fn expiry_cancels_navigation_and_manual_takeover_clears_owned_lease() {
        let (db, _, mut o) = setup();
        let mut p = Supervisor::default();
        p.guard(
            &o,
            &db,
            vec![Command::Goto {
                unit: EntityId(2),
                waypoints: vec![Vec2::new(1.0, 2.0)],
            }],
        );
        o.tick = 601;
        let (c, _) = p.maintenance(&o, &db);
        assert!(c
            .iter()
            .any(|c| matches!(c,Command::Goto{waypoints,..} if waypoints.is_empty())));
        assert!(p.tasks.is_empty());
        p.guard(
            &o,
            &db,
            vec![Command::AssignTarget {
                unit: EntityId(2),
                contact: ContactId(1),
            }],
        );
        p.release_manual(EntityId(2));
        assert!(p.tasks.is_empty());
    }
    #[test]
    fn stale_target_cancels_owned_task_and_cap_is_not_misclassified_as_empty_offensive_load() {
        let (db, _, mut o) = setup();
        let mut p = Supervisor::default();
        p.guard(
            &o,
            &db,
            vec![Command::AssignTarget {
                unit: EntityId(2),
                contact: ContactId(1),
            }],
        );
        o.tick = 30;
        o.contacts[0].stale = true;
        assert!(!p.maintenance(&o, &db).0.is_empty());
        o.contacts[0].stale = false;
        o.contacts[0].category = PlatformKind::Aircraft;
        o.units[1].mission = Some("cap".into());
        o.units[1].assigned = Some(ContactId(1));
        for w in &mut o.units[1].weapons {
            w.ammo = 0;
        }
        p.guard(
            &o,
            &db,
            vec![Command::AssignTarget {
                unit: EntityId(2),
                contact: ContactId(1),
            }],
        );
        o.tick = 60;
        assert!(p.maintenance(&o, &db).0.is_empty());
    }
    #[test]
    fn reserve_holds_discretionary_sam_fire_but_reacts_to_observed_immediate_threat() {
        let (db, _, mut o) = setup();
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        for w in &mut o.units[1].weapons {
            if db.weapons[&w.weapon].kind == WeaponKind::Sam {
                w.ammo = 1;
            }
        }
        o.tick = 30;
        p.observe(&o, &db);
        let cmd = Command::SetWeaponsFree {
            unit: EntityId(2),
            free: true,
        };
        assert!(matches!(
            p.guard(&o, &db, vec![cmd.clone()]).0[0],
            Command::SetWeaponsFree { free: false, .. }
        ));
        o.contacts[0].category = PlatformKind::Missile;
        o.contacts[0].estimate = Vec2::new(10.0, 0.0);
        assert!(matches!(
            p.guard(&o, &db, vec![cmd]).0[0],
            Command::SetWeaponsFree { free: true, .. }
        ));
    }
    #[test]
    fn support_binding_ends_and_asw_searchers_do_not_all_follow_the_same_old_track() {
        let (db, mut w, _) = setup();
        harpoon_core::air::launch_aircraft(
            &mut w,
            EntityId(1),
            "us_f18e_asm",
            2,
            &AirMission::Strike {
                contact: ContactId(1),
            },
        )
        .unwrap();
        let mut o = harpoon_core::observe(&w, Side::Blue, 0, 0);
        let mut ew = o
            .units
            .iter()
            .find(|u| u.kind == PlatformKind::Aircraft)
            .unwrap()
            .clone();
        ew.id = EntityId(90);
        ew.class = "cn_j15d".into();
        ew.mission = Some("recon".into());
        o.units.push(ew);
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        assert_eq!(p.ew[&EntityId(90)].members.len(), 2);
        let command = Command::SetJamming {
            unit: EntityId(90),
            mode: "escort".into(),
            contacts: vec![],
        };
        assert!(
            matches!(&p.guard(&o,&db,vec![command.clone()]).0[0],Command::SetJamming{mode,..} if mode=="escort")
        );
        o.tick = 30;
        for u in &mut o.units {
            if u.mission.as_deref() == Some("strike") {
                u.mission = Some("rtb".into());
            }
        }
        p.observe(&o, &db);
        assert!(p.ew.is_empty());
        assert!(
            matches!(&p.guard(&o,&db,vec![command]).0[0],Command::SetJamming{mode,..} if mode=="off")
        );
        let mut helo = o
            .units
            .iter()
            .find(|u| u.kind == PlatformKind::Aircraft)
            .unwrap()
            .clone();
        helo.class = "us_mh60r".into();
        helo.mission = Some("asw".into());
        helo.id = EntityId(91);
        o.units.push(helo.clone());
        helo.id = EntityId(92);
        o.units.push(helo);
        o.contacts.push(serde_json::from_value(json!({"id":8,"category":"Sub","estimate":{"x":12,"y":0},"est_course_deg":0,"est_speed_kts":0,"quality":0.8,"bearing_only":false,"last_seen_s":0,"stale":false,"sensors":[]})).unwrap());
        o.tick = 60;
        p.observe(&o, &db);
        assert_eq!(
            p.asw
                .values()
                .filter(|t| t.contact == Some(ContactId(8)))
                .count(),
            1
        );
        o.tick = 90;
        o.contacts[0].stale = true;
        o.units[0].pos.x = 20.0;
        p.observe(&o, &db);
        assert!(p.asw.values().all(|t| t.contact.is_none()));
        assert!(p.asw.values().any(|t| t.area.x > 20.0));
    }
    #[test]
    fn saves_keep_accounting_leases_and_bindings_and_views_do_not_contain_hidden_unit_data() {
        let (db, _, o) = setup();
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        p.guard(
            &o,
            &db,
            vec![Command::AssignTarget {
                unit: EntityId(2),
                contact: ContactId(1),
            }],
        );
        let text = serde_json::to_string(&p).unwrap();
        let restored: Supervisor = serde_json::from_str(&text).unwrap();
        assert_eq!(p.summary(&o), restored.summary(&o));
        assert!(!p.summary(&o).to_string().contains("about"));
        assert!(!p.summary(&o).to_string().contains("enemy_hp"));
    }
    #[test]
    fn a_recovering_aircraft_does_not_also_receive_a_planned_release() {
        let (db, _, o) = setup();
        let mut p = Supervisor::default();
        let (c, _) = p.guard(
            &o,
            &db,
            vec![
                Command::Fire {
                    unit: EntityId(2),
                    contact: ContactId(1),
                    weapon: "cn_yj18".into(),
                    rounds: 2,
                },
                Command::ReturnToBase { unit: EntityId(2) },
            ],
        );
        assert!(!c.iter().any(|c| matches!(c, Command::Fire { .. })));
    }
    #[test]
    fn a_fresh_hold_reviews_safe_tasks_while_stale_targets_cannot_be_renewed() {
        let (db, _, mut o) = setup();
        let mut p = Supervisor::default();
        p.guard(
            &o,
            &db,
            vec![Command::AssignTarget {
                unit: EntityId(2),
                contact: ContactId(1),
            }],
        );
        o.tick = 300;
        p.review_current(&o, &db);
        assert_eq!(p.tasks[&EntityId(2)].expires_at, 900);
        o.tick = 400;
        o.contacts[0].stale = true;
        p.review_current(&o, &db);
        assert_eq!(p.tasks[&EntityId(2)].expires_at, 900);
        assert!(!p.maintenance(&o, &db).0.is_empty());
        assert!(p.tasks.is_empty());
    }
    #[test]
    fn departure_route_and_task_ownership_are_installed_and_accounted_without_waiting_for_another_model_turn(
    ) {
        let (db, mut w, o) = setup();
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        let launch = Command::LaunchAircraft {
            unit: EntityId(1),
            class: "us_f18e_asm".into(),
            count: 4,
            mission: AirMission::Strike {
                contact: ContactId(1),
            },
            profile: None,
        };
        let (commands, _) = p.guard(&o, &db, vec![launch]);
        harpoon_core::sim::tick(
            &mut w,
            &commands
                .into_iter()
                .map(|c| (Side::Blue, c))
                .collect::<Vec<_>>(),
        );
        let o = harpoon_core::observe(&w, Side::Blue, 0, 256);
        p.observe(&o, &db);
        let air: Vec<_> = w
            .entities
            .values()
            .filter(|u| u.kind == PlatformKind::Aircraft)
            .collect();
        assert_eq!(air.len(), 4);
        assert!(air.iter().all(|u| !u.waypoints.is_empty()));
        assert_eq!(p.origins.len(), 4);
        assert!(air
            .iter()
            .all(|u| p.tasks[&u.id].contact == Some(ContactId(1))));
    }
    #[test]
    fn transient_air_track_loss_allows_bounded_search_but_never_fire_or_lease_renewal() {
        let (db, mut w, _) = setup();
        harpoon_core::air::launch_aircraft(
            &mut w,
            EntityId(1),
            "us_f18e_asm",
            1,
            &AirMission::Strike {
                contact: ContactId(1),
            },
        )
        .unwrap();
        let mut o = harpoon_core::observe(&w, Side::Blue, 0, 0);
        let id = o
            .units
            .iter()
            .find(|u| u.kind == PlatformKind::Aircraft)
            .unwrap()
            .id;
        let (_, _, fixture) = setup();
        o.contacts = fixture.contacts;
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        p.guard(
            &o,
            &db,
            vec![Command::AssignTarget {
                unit: id,
                contact: ContactId(1),
            }],
        );
        o.tick = 30;
        o.contacts[0].stale = true;
        p.observe(&o, &db);
        p.review_current(&o, &db);
        assert_eq!(p.tasks[&id].expires_at, 600);
        assert!(p.maintenance(&o, &db).0.is_empty());
        assert!(p
            .guard(
                &o,
                &db,
                vec![Command::Fire {
                    unit: id,
                    contact: ContactId(1),
                    weapon: "us_harpoon".into(),
                    rounds: 2
                }]
            )
            .0
            .is_empty());
        o.tick = 90;
        o.contacts[0].stale = false;
        p.observe(&o, &db);
        assert!(!p.lost_tracks.contains_key(&id));
        o.tick = 120;
        o.contacts[0].stale = true;
        p.observe(&o, &db);
        o.tick = 300;
        assert!(p
            .maintenance(&o, &db)
            .0
            .iter()
            .any(|c| matches!(c,Command::ReturnToBase{unit} if *unit==id)));
    }
    #[test]
    fn retasking_to_search_clears_the_previous_strike_target_lease() {
        let (db, _, mut o) = setup();
        o.units[1].kind = PlatformKind::Aircraft;
        let mut p = Supervisor::default();
        p.guard(
            &o,
            &db,
            vec![Command::AssignTarget {
                unit: EntityId(2),
                contact: ContactId(1),
            }],
        );
        p.guard(
            &o,
            &db,
            vec![Command::RetaskAircraft {
                unit: EntityId(2),
                mission: AirMission::Recon {
                    point: Vec2::zero(),
                },
                manual: true,
            }],
        );
        o.contacts[0].stale = true;
        o.tick = 300;
        assert!(p.tasks[&EntityId(2)].contact.is_none());
        assert!(p.maintenance(&o, &db).0.is_empty());
    }
    #[test]
    fn later_manual_launches_are_not_automatically_owned_by_ai_task_leases() {
        let (db, mut w, o) = setup();
        let mut p = Supervisor::default();
        p.observe(&o, &db);
        harpoon_core::air::launch_aircraft(
            &mut w,
            EntityId(1),
            "us_f18e",
            2,
            &AirMission::Cap {
                station: Vec2::zero(),
                radius_nm: 10.0,
                follow: true,
            },
        )
        .unwrap();
        w.tick = 30;
        let o = harpoon_core::observe(&w, Side::Blue, 0, 0);
        p.observe(&o, &db);
        assert!(p.tasks.is_empty());
    }
    #[test]
    fn supported_jamming_executes_with_existing_geometry_and_stops_after_the_package_returns() {
        let db =
            Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
        let sc=harpoon_core::scenario_from_json(r#"{"id":"bound-ew","name":"bound-ew","duration_s":600,"seed":3,"weather":{"visibility_nm":1,"jam":[0,0]},"context":{"radar_environment":{"sea_state":0,"precipitation":"None"}},"units":[{"side":"Blue","class":"cn_j15","pos":[0,0],"altitude_m":3000},{"side":"Blue","class":"us_f18e","pos":[20,0],"altitude_m":3000},{"side":"Red","class":"us_ea18g","pos":[10,0],"course_deg":270,"altitude_m":3000},{"side":"Red","class":"us_f18e_asm","pos":[11,0],"altitude_m":3000}]}"#).unwrap();
        let mut w = harpoon_core::load_scenario(sc, db.clone()).unwrap();
        w.entity_mut(EntityId(4)).unwrap().mission = Some(
            harpoon_core::world::MissionState::from_air_mission(&AirMission::Strike {
                contact: ContactId(1),
            }),
        );
        w.sides[1].contacts.insert(
            ContactId(1),
            harpoon_core::world::Contact {
                id: ContactId(1),
                about: EntityId(1),
                category: PlatformKind::Aircraft,
                class_name: None,
                estimate: Vec2::zero(),
                est_course_deg: 0.0,
                est_speed_kts: 0.0,
                quality: 0.5,
                bearing_only: true,
                last_update_tick: 0,
                first_tick: 0,
                stale: false,
                sensors: vec!["esm_std".into()],
                offset: Vec2::zero(),
                announced: false,
            },
        );
        let mut p = Supervisor::default();
        let o = harpoon_core::observe(&w, Side::Red, 0, 256);
        p.observe(&o, &db);
        let jamming = Command::SetJamming {
            unit: EntityId(3),
            mode: "spot".into(),
            contacts: vec![ContactId(1)],
        };
        let (orders, _) = p.guard(&o, &db, vec![jamming.clone()]);
        harpoon_core::sim::tick(
            &mut w,
            &orders
                .into_iter()
                .map(|c| (Side::Red, c))
                .collect::<Vec<_>>(),
        );
        let multiplier = harpoon_core::rules::electronic_warfare::combined_modifier(
            &w,
            w.entity(EntityId(1)).unwrap(),
            &db.sensors["cn_klj4"],
            w.entity(EntityId(2)).unwrap(),
        )
        .unwrap();
        assert!(multiplier < 1.0);
        w.entity_mut(EntityId(4)).unwrap().mission = None;
        let o = harpoon_core::observe(&w, Side::Red, 0, 256);
        p.observe(&o, &db);
        let (orders, _) = p.guard(&o, &db, vec![jamming]);
        harpoon_core::sim::tick(
            &mut w,
            &orders
                .into_iter()
                .map(|c| (Side::Red, c))
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            harpoon_core::rules::electronic_warfare::combined_modifier(
                &w,
                w.entity(EntityId(1)).unwrap(),
                &db.sensors["cn_klj4"],
                w.entity(EntityId(2)).unwrap()
            ),
            Some(1.0)
        );
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Task {
    pub contact: Option<ContactId>,
    pub expires_at: i64,
    pub reviewed_at: i64,
    pub route: bool,
    pub air: bool,
    pub jammer: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Attack {
    pub rounds: u32,
    pub observed_damage: f64,
    pub observed_hits: u32,
    pub last_release: i64,
    pub reviewed_rounds: u32,
    pub unconfirmed_batches: u32,
    pub hold_until: i64,
    pub damage_at_release: f64,
    pub last_review: i64,
}
#[derive(Clone, Serialize, Deserialize)]
struct Ammo {
    unit: EntityId,
    weapon: String,
    amount: u16,
}
#[derive(Clone, Serialize, Deserialize)]
struct Seen {
    class: String,
    pos: Vec2,
    home: Option<EntityId>,
    support: bool,
    rtb: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Unavailable {
    class: String,
    pos: Vec2,
    home: Option<EntityId>,
    until: i64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Support {
    pub members: Vec<EntityId>,
    pub target: Option<ContactId>,
    pub until: i64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Search {
    pub contact: Option<ContactId>,
    pub area: Vec2,
    pub until: i64,
}

fn key(unit: EntityId, weapon: &str) -> String {
    format!("{}:{}", unit.0, weapon)
}
pub fn unit_of(command: &Command) -> EntityId {
    EntityId(
        serde_json::to_value(command)
            .ok()
            .and_then(|v| v["unit"].as_u64())
            .unwrap_or(0) as u32,
    )
}
fn offensive(kind: WeaponKind) -> bool {
    matches!(
        kind,
        WeaponKind::Ascm
            | WeaponKind::Slcm
            | WeaponKind::Bomb
            | WeaponKind::Rocket
            | WeaponKind::Torpedo
            | WeaponKind::Lwt
            | WeaponKind::AswRocket
    )
}
fn support_class(class: &str, db: &Database) -> bool {
    is_aew_class(class)
        || harpoon_core::rules::electronic_warfare::capability(class).is_some()
        || db.platforms.get(class).is_some_and(|p| {
            p.sensors.iter().any(|s| {
                db.sensors
                    .get(s)
                    .is_some_and(|s| s.kind == harpoon_core::db::SensorKind::SonarActive)
            })
        })
}
fn home<'a>(obs: &'a Observation, u: &UnitObs) -> Option<&'a UnitObs> {
    obs.units
        .iter()
        .filter(|h| h.deck.is_some())
        .min_by(|a, b| u.pos.range_to(a.pos).total_cmp(&u.pos.range_to(b.pos)))
}
fn fresh(c: &ContactObs) -> bool {
    !c.stale && !c.bearing_only && c.quality >= 0.4 && c.bda_hp_fraction != Some(0.0)
}
fn notice(
    unit: EntityId,
    contact: Option<ContactId>,
    action: &str,
    zh: String,
    en: String,
) -> StaffNote {
    StaffNote {
        unit,
        contact,
        action: action.into(),
        zh,
        en,
    }
}

impl Supervisor {
    fn track_auto_defence(&mut self,obs:&Observation,db:&Database,u:&UnitObs) {
        let left=sam_stock(u,db);if left==0{return;}
        let initial=self.initial_sam.get(&u.id).copied().unwrap_or(left);
        let b=self.auto_defence.entry(u.id).or_insert(AutoDefence{start_stock:left,allowance:(initial/4).clamp(4,16),until:obs.tick+600});
        if obs.tick>=b.until {*b=AutoDefence{start_stock:left,allowance:(initial/4).clamp(4,16),until:obs.tick+600};}
    }
    fn defence_held(&self,obs:&Observation,db:&Database,u:&UnitObs)->bool {
        self.auto_defence.get(&u.id).is_some_and(|b|{
            let left=sam_stock(u,db);let initial=self.initial_sam.get(&u.id).copied().unwrap_or(b.start_stock);
            !immediate_air_threat(obs,u)&&(obs.tick>=b.until||left<=initial/4||b.start_stock.saturating_sub(left)>=b.allowance)
        })
    }
    fn friendly_home<'a>(&self, obs: &'a Observation, u: &UnitObs) -> Option<&'a UnitObs> {
        self.previous
            .get(&u.id)
            .and_then(|p| p.home)
            .and_then(|id| obs.units.iter().find(|h| h.id == id && h.deck.is_some()))
            .or_else(|| home(obs, u))
    }
    fn dedupe(&mut self, notes: Vec<StaffNote>, tick: i64) -> Vec<StaffNote> {
        self.notices.retain(|_, at| tick - *at <= 1800);
        notes
            .into_iter()
            .filter(|n| {
                if n.action == "attack_account" {
                    return true;
                }
                let k = format!("{}:{}:{:?}:{}", n.unit.0, n.action, n.contact, n.en);
                if self.notices.get(&k).is_some_and(|at| tick - *at < 300) {
                    return false;
                }
                self.notices.insert(k, tick);
                true
            })
            .collect()
    }
    pub fn is_empty(&self) -> bool {
        self.last_tick.is_none()
            && self.tasks.is_empty()
            && self.attacks.is_empty()
            && self.scheduled.is_empty()
            && self.auto_defence.is_empty()
    }
    /// Ammunition deltas are actual own expenditure, not submitted Fire orders.
    /// Missing aircraft are never counted as having fired their remaining load.
    pub fn observe(&mut self, obs: &Observation, db: &Database) -> Vec<StaffNote> {
        if self.last_tick == Some(obs.tick) {
            return vec![];
        }
        let mut notes = Vec::new();
        self.flights.retain(|f| f.until >= obs.tick&&!obs.events.iter().any(|e|e.tick>f.until-60&&e.tick<=f.until-30&&matches!(&e.kind,harpoon_core::EventKind::DeckBusy{carrier,..}if *carrier==f.home)));
        for u in obs
            .units
            .iter()
            .filter(|u| u.kind == PlatformKind::Aircraft && !self.previous.contains_key(&u.id))
        {
            let ordered = self.flights.iter().rev().find(|f| {
                f.class == u.class
                    && obs
                        .units
                        .iter()
                        .any(|h| h.id == f.home && u.pos.range_to(h.pos) < 8.0)
            });
            let contact = ordered
                .and_then(|f| f.target)
                .or_else(|| {
                    obs.exercise.as_ref().and_then(|e| {
                        e.opening_flights
                            .iter()
                            .find(|f| f.unit == u.id)
                            .map(|f| f.target)
                    })
                })
                .or(u.assigned);
            let origin = ordered.map(|f| f.home).or_else(|| {
                obs.exercise.as_ref().and_then(|e| {
                    e.opening_flights
                        .iter()
                        .find(|f| f.unit == u.id)
                        .map(|f| f.home)
                })
            });
            if let Some(origin) = origin {
                self.origins.insert(u.id, origin);
            }
            if u.mission.is_some()
                && (self.last_tick.is_none() || ordered.is_some() || origin.is_some())
            {
                self.tasks.entry(u.id).or_insert(Task {
                    contact,
                    expires_at: obs.tick + 600,
                    reviewed_at: obs.tick,
                    route: true,
                    air: true,
                    jammer: false,
                });
            }
            if let Some(cid) = contact {
                for w in &u.weapons {
                    if db.weapons.get(&w.weapon).is_some_and(|d| offensive(d.kind)) {
                        self.intents.insert(key(u.id, &w.weapon), cid);
                    }
                    if ordered.is_some() {
                        if let Some(mount) = db
                            .platforms
                            .get(&u.class)
                            .and_then(|p| p.weapons.iter().find(|m| m.weapon == w.weapon))
                        {
                            self.ammo.entry(key(u.id, &w.weapon)).or_insert(Ammo {
                                unit: u.id,
                                weapon: w.weapon.clone(),
                                amount: mount.ammo * mount.mounts as u16,
                            });
                        }
                    }
                }
            }
        }
        self.unavailable.retain(|x| x.until > obs.tick);
        // Recovery events identify class and carrier, not a unique airframe.
        // Consume each new event count once instead of reusing it for losses.
        let mut recoveries: BTreeMap<(EntityId, String), u64> = BTreeMap::new();
        for e in obs
            .events
            .iter()
            .filter(|e| self.last_tick.is_none_or(|t| e.tick > t))
        {
            let v = serde_json::to_value(e).unwrap_or_default();
            if v["event"] == "air_recovered" {
                if let (Some(carrier), Some(class)) = (v["carrier"].as_u64(), v["class"].as_str()) {
                    *recoveries
                        .entry((EntityId(carrier as u32), class.into()))
                        .or_default() += v["count"].as_u64().unwrap_or(0);
                }
            }
        }
        for (id, old) in &self.previous {
            if old.support && !old.rtb && !obs.units.iter().any(|u| u.id == *id) {
                let display = db
                    .platforms
                    .get(&old.class)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| old.class.clone());
                let recovered = old
                    .home
                    .and_then(|h| recoveries.get_mut(&(h, display)))
                    .is_some_and(|n| {
                        if *n > 0 {
                            *n -= 1;
                            true
                        } else {
                            false
                        }
                    });
                if !recovered {
                    self.unavailable.push(Unavailable {
                        class: old.class.clone(),
                        pos: old.pos,
                        home: old.home,
                        until: obs.tick + 900,
                    });
                    notes.push(notice(*id,None,"support_reassess","支援机未返回当前观察：原站位暂缓补飞，先复核风险；未把失联直接认定为击落".into(),"Support unavailable in current observation: hold replacement at the old station and reassess; disappearance is not a confirmed kill".into()));
                }
            }
        }
        let mut expenses: BTreeMap<(EntityId, String), u32> = BTreeMap::new();
        // Own release receipts cover an aircraft that fired and was lost
        // between decisions. Ammunition deltas fill any missing event window.
        if let Some(previous_tick) = self.last_tick {
            for e in obs.events.iter().filter(|e| e.tick > previous_tick) {
                if let harpoon_core::EventKind::WeaponsRelease {
                    side,
                    shooter,
                    weapon,
                    count,
                    ..
                } = &e.kind
                {
                    if *side != obs.your_side {
                        continue;
                    }
                    let candidates: Vec<_> = db
                        .weapons
                        .iter()
                        .filter(|(_, d)| d.name == *weapon && offensive(d.kind))
                        .collect();
                    let candidate = if candidates.len() == 1 {
                        Some(candidates[0])
                    } else {
                        candidates.into_iter().find(|(id, _)| {
                            self.ammo.contains_key(&key(*shooter, id))
                                || obs.units.iter().any(|u| {
                                    u.id == *shooter && u.weapons.iter().any(|w| w.weapon == **id)
                                })
                        })
                    };
                    if let Some((id, _)) = candidate {
                        *expenses.entry((*shooter, id.clone())).or_default() += *count as u32;
                    }
                }
            }
        }
        for u in &obs.units {
            let sam = u
                .weapons
                .iter()
                .filter(|w| {
                    db.weapons
                        .get(&w.weapon)
                        .is_some_and(|d| d.kind == WeaponKind::Sam)
                })
                .map(|w| w.ammo as u32)
                .sum();
            self.initial_sam
                .entry(u.id)
                .and_modify(|v| *v = (*v).max(sam))
                .or_insert(sam);
            for w in &u.weapons {
                if let Some(old) = self
                    .ammo
                    .get(&key(u.id, &w.weapon))
                    .filter(|a| a.amount > w.ammo)
                {
                    if db.weapons.get(&w.weapon).is_some_and(|d| offensive(d.kind)) {
                        let n = (old.amount - w.ammo) as u32;
                        expenses
                            .entry((u.id, w.weapon.clone()))
                            .and_modify(|v| *v = (*v).max(n))
                            .or_insert(n);
                    }
                }
            }
        }
        for ((unit, weapon), spent) in expenses {
            *self.expenditure_by_unit.entry(unit).or_default() += spent;
            let task = self.tasks.get(&unit).and_then(|t| t.contact);
            let assigned = obs
                .units
                .iter()
                .find(|u| u.id == unit)
                .and_then(|u| u.assigned);
            if let Some(cid) = self
                .intents
                .remove(&key(unit, &weapon))
                .or(task)
                .or(assigned)
            {
                let a = self.attacks.entry(cid).or_default();
                a.damage_at_release = a.observed_damage;
                a.rounds += spent;
                a.last_release = obs.tick;
                notes.push(notice(unit,Some(cid),"attack_account",format!("实际进攻消耗{}枚，目标C{}；按本方可见回执/弹库核对，等待估损",spent,cid.0),format!("Actual offensive expenditure {} at C{}; own receipt/magazine accounting, BDA pending",spent,cid.0)));
            } else {
                *self.unattributed_rounds.entry(weapon).or_default() += spent;
            }
        }
        for (cid, a) in &mut self.attacks {
            if let Some(c) = obs.contacts.iter().find(|c| c.id == *cid) {
                if c.bda_damage > a.observed_damage || c.bda_hp_fraction == Some(0.0) {
                    a.hold_until = 0;
                }
                a.observed_damage = a.observed_damage.max(c.bda_damage);
                a.observed_hits = a.observed_hits.max(c.bda_hits);
                if a.rounds > a.reviewed_rounds
                    && c.own_in_flight == 0
                    && obs.tick - a.last_release >= 180
                {
                    a.last_review = obs.tick;
                    a.reviewed_rounds = a.rounds;
                    if c.bda_damage <= a.damage_at_release && c.bda_hp_fraction != Some(0.0) {
                        a.unconfirmed_batches += 1;
                        a.hold_until = obs.tick + 600;
                        notes.push(notice(EntityId(0),Some(*cid),"attack_review",format!("C{}本次攻击尚无确认收益，暂停原样补射；重获、改派或人工复核后再决策",cid.0),format!("C{} expenditure has no confirmed benefit yet; pause identical follow-up and reassess tracks or assignment",cid.0)));
                    } else {
                        a.hold_until = 0;
                    }
                }
            }
        }
        self.ammo = obs
            .units
            .iter()
            .flat_map(|u| {
                u.weapons.iter().map(move |w| {
                    (
                        key(u.id, &w.weapon),
                        Ammo {
                            unit: u.id,
                            weapon: w.weapon.clone(),
                            amount: w.ammo,
                        },
                    )
                })
            })
            .collect();
        self.previous = obs
            .units
            .iter()
            .filter(|u| u.kind == PlatformKind::Aircraft)
            .map(|u| {
                (
                    u.id,
                    Seen {
                        class: u.class.clone(),
                        pos: u.pos,
                        home: self.friendly_home(obs, u).map(|h| h.id),
                        support: support_class(&u.class, db)
                            || u.mission.as_deref() == Some("recon"),
                        rtb: u.mission.as_deref() == Some("rtb"),
                    },
                )
            })
            .collect();
        self.intents.retain(|k, _| self.ammo.contains_key(k));
        self.tasks
            .retain(|id, _| obs.units.iter().any(|u| u.id == *id));
        self.lost_tracks.retain(|id, _| self.tasks.contains_key(id));
        for (id, t) in &self.tasks {
            if t.contact
                .is_some_and(|cid| !obs.contacts.iter().any(|c| c.id == cid && fresh(c)))
            {
                self.lost_tracks.entry(*id).or_insert(obs.tick);
            } else {
                self.lost_tracks.remove(id);
            }
        }
        let old = self.ew.clone();
        self.assign_support(obs, db);
        for (id, s) in &self.ew {
            if old
                .get(id)
                .is_none_or(|p| p.until <= obs.tick || p.members != s.members)
            {
                notes.push(notice(*id,s.target,"support_binding",format!("绑定{}架本方突击机，支援复核期限T{}；任务结束或失联则撤收",s.members.len(),s.until),format!("Bind support to {} friendly strike aircraft until T{} review; stop when the package ends",s.members.len(),s.until)));
            }
        }
        self.last_tick = Some(obs.tick);
        self.dedupe(notes, obs.tick)
    }
    pub fn target_allowed(&self, id: ContactId, tick: i64) -> bool {
        self.attacks.get(&id).is_none_or(|a| a.hold_until <= tick)
    }
    pub fn support_launch_allowed(&self, obs: &Observation, host: &UnitObs, class: &str) -> bool {
        !self
            .unavailable
            .iter()
            .any(|p| p.class == class && p.home == Some(host.id) && p.until > obs.tick)
            && !obs.contacts.iter().any(|c| {
                fresh(c)
                    && matches!(c.category, PlatformKind::Aircraft | PlatformKind::Missile)
                    && host.pos.range_to(c.estimate) < 35.0
            })
    }
    pub(super) fn known_origin(&self, unit: EntityId) -> Option<EntityId> {
        self.origins.get(&unit).copied()
    }
    pub(super) fn support_station(
        &self,
        obs: &Observation,
        db: &Database,
        home: &UnitObs,
        desired: Vec2,
    ) -> Vec2 {
        let mut p = super::assessment::station(obs, db, home, desired);
        if let Some(loss) = self
            .unavailable
            .iter()
            .find(|r| r.until > obs.tick && r.pos.range_to(p) < 30.0)
        {
            p = home.pos.advanced(loss.pos.bearing_to(home.pos), 25.0);
        }
        p
    }
    fn assign_support(&mut self, obs: &Observation, db: &Database) {
        let active: Vec<_> = obs
            .units
            .iter()
            .filter(|u| {
                u.mission.as_deref() == Some("strike")
                    && u.weapons.iter().any(|w| {
                        w.ammo > 0 && db.weapons.get(&w.weapon).is_some_and(|d| offensive(d.kind))
                    })
            })
            .collect();
        self.ew.retain(|id, s| {
            s.until > obs.tick
                && obs.units.iter().any(|u| u.id == *id)
                && s.members
                    .iter()
                    .any(|id| active.iter().any(|u| u.id == *id))
        });
        for u in obs
            .units
            .iter()
            .filter(|u| harpoon_core::rules::electronic_warfare::capability(&u.class).is_some())
        {
            if self.ew.contains_key(&u.id) {
                continue;
            }
            if let Some(lead) = active
                .iter()
                .min_by(|a, b| u.pos.range_to(a.pos).total_cmp(&u.pos.range_to(b.pos)))
            {
                let h = self.friendly_home(obs, lead).map(|h| h.id);
                let members = active
                    .iter()
                    .filter(|a| self.friendly_home(obs, a).map(|h| h.id) == h)
                    .map(|a| a.id)
                    .collect();
                let target = self
                    .tasks
                    .get(&lead.id)
                    .and_then(|t| t.contact)
                    .or(lead.assigned)
                    .filter(|id| {
                        obs.contacts.iter().any(|c| {
                            c.id == *id
                                && matches!(c.category, PlatformKind::Ship | PlatformKind::Base)
                        })
                    });
                self.ew.insert(
                    u.id,
                    Support {
                        members,
                        target,
                        until: obs.tick + 600,
                    },
                );
            }
        }
        // One airborne searcher per fresh submarine contact. Remaining assets
        // receive distinct sectors tied to their moving friendly home.
        self.asw.clear();
        let mut claimed = BTreeSet::new();
        let mut rank: BTreeMap<EntityId, u32> = BTreeMap::new();
        for u in obs.units.iter().filter(|u| {
            u.kind == PlatformKind::Aircraft
                && (u.mission.as_deref() == Some("asw")
                    || db.platforms.get(&u.class).is_some_and(|p| {
                        p.weapons.iter().any(|m| {
                            db.weapons
                                .get(&m.weapon)
                                .is_some_and(|d| d.kind == WeaponKind::Lwt)
                        })
                    }))
        }) {
            if u.mission.as_deref() == Some("rtb") {
                continue;
            }
            let Some(h) = self.friendly_home(obs, u) else {
                continue;
            };
            let target = obs
                .contacts
                .iter()
                .filter(|c| {
                    c.category == PlatformKind::Sub
                        && fresh(c)
                        && c.last_seen_s <= 180.0
                        && !claimed.contains(&c.id)
                })
                .filter(|c| {
                    h.pos.range_to(c.estimate) <= 40.0
                        && !obs.contacts.iter().any(|a| {
                            fresh(a)
                                && matches!(
                                    a.category,
                                    PlatformKind::Aircraft | PlatformKind::Missile
                                )
                                && a.estimate.range_to(c.estimate) < 30.0
                        })
                })
                .min_by(|a, b| {
                    u.pos
                        .range_to(a.estimate)
                        .total_cmp(&u.pos.range_to(b.estimate))
                });
            let i = rank.entry(h.id).or_default();
            let offset = *i;
            *i += 1;
            let (contact, area) = if let Some(c) = target {
                claimed.insert(c.id);
                (Some(c.id), c.estimate)
            } else {
                (
                    None,
                    super::assessment::asw_station(obs,db,h,offset).unwrap_or(h.pos),
                )
            };
            self.asw.insert(
                u.id,
                Search {
                    contact,
                    area,
                    until: obs.tick + 180,
                },
            );
        }
    }
    pub fn release_manual(&mut self, unit: EntityId) {
        self.auto_defence.remove(&unit);
        self.scheduled.retain(|p| p.unit != unit);
        self.tasks.remove(&unit);
        self.lost_tracks.remove(&unit);
        self.intents
            .retain(|k, _| !k.starts_with(&format!("{}:", unit.0)));
        self.flights.retain(|f| f.home != unit);
    }
    pub(super) fn bind_strike(&mut self, unit: EntityId, contact: Option<ContactId>, tick: i64) {
        if let Some(t) = self.tasks.get_mut(&unit) {
            if t.contact != contact {
                self.lost_tracks.remove(&unit);
            }
            t.contact = contact;
        } else {
            self.tasks.insert(
                unit,
                Task {
                    contact,
                    expires_at: tick + 600,
                    reviewed_at: tick,
                    route: true,
                    air: true,
                    jammer: false,
                },
            );
        }
    }
    /// A newly accepted observation-bound decision also reviews standing tasks.
    /// Merely ticking time, generating advice or retrying an old plan does not.
    pub fn review_current(&mut self, obs: &Observation, db: &Database) -> Vec<StaffNote> {
        if self.tasks.is_empty() {
            return vec![];
        }
        let mut trial = self.clone();
        for task in trial.tasks.values_mut() {
            task.expires_at = i64::MAX;
        }
        trial.maintenance(obs, db);
        let mut reviewed = 0;
        for (id, task) in &mut self.tasks {
            if trial.tasks.contains_key(id)
                && task
                    .contact
                    .is_none_or(|cid| obs.contacts.iter().any(|c| c.id == cid && fresh(c)))
                && obs
                    .units
                    .iter()
                    .any(|u| u.id == *id && u.mission.as_deref() != Some("rtb"))
            {
                task.expires_at = obs.tick + 600;
                task.reviewed_at = obs.tick;
                reviewed += 1;
            }
        }
        self.dedupe(vec![notice(EntityId(0),None,"standing_review",format!("基于本次观察复核{}项持续任务；失效航迹、缺弹与低续航任务不续期",reviewed),format!("Review {} standing tasks against the current observation; invalid tracks, empty loads and low return reserves are not renewed",reviewed))],obs.tick)
    }
    pub fn maintenance(
        &mut self,
        obs: &Observation,
        db: &Database,
    ) -> (Vec<Command>, Vec<StaffNote>) {
        let mut commands = Vec::new();
        let mut notes = Vec::new();
        let mut remove = Vec::new();
        for (id, t) in &self.tasks {
            let Some(u) = obs.units.iter().find(|u| u.id == *id) else {
                remove.push(*id);
                continue;
            };
            if u.mission.as_deref() == Some("rtb") {
                remove.push(*id);
                continue;
            }
            let invalid = t.contact.is_some_and(|cid| {
                if obs
                    .contacts
                    .iter()
                    .any(|c| c.id == cid && c.bda_hp_fraction == Some(0.0))
                {
                    return true;
                }
                !obs.contacts.iter().any(|c| c.id == cid && fresh(c))
                    && (u.kind != PlatformKind::Aircraft
                        || obs.tick - self.lost_tracks.get(id).copied().unwrap_or(t.reviewed_at)
                            >= 180)
            });
            let offensive_task = u.mission.as_deref() == Some("strike")
                || t.contact.is_some_and(|id| {
                    obs.contacts.iter().any(|c| {
                        c.id == id
                            && matches!(
                                c.category,
                                PlatformKind::Ship | PlatformKind::Sub | PlatformKind::Base
                            )
                    })
                });
            let empty = offensive_task
                && !u.weapons.iter().any(|w| {
                    w.ammo > 0 && db.weapons.get(&w.weapon).is_some_and(|d| offensive(d.kind))
                });
            let low = u.kind == PlatformKind::Aircraft
                && u.endurance_s.is_some_and(|s| {
                    self.friendly_home(obs, u).is_some_and(|h| {
                        let cruise = db
                            .platforms
                            .get(&u.class)
                            .map(|p| p.cruise_speed_kts.max(60.0))
                            .unwrap_or(250.0);
                        s < u.pos.range_to(h.pos) / cruise * 3600.0 * 1.3 + 300.0
                    })
                });
            if t.expires_at > obs.tick && !invalid && !empty && !low {
                continue;
            }
            commands.push(Command::ClearTarget { unit: *id });
            if t.route {
                commands.push(Command::Goto {
                    unit: *id,
                    waypoints: vec![],
                });
            }
            if t.air || low {
                commands.push(Command::ReturnToBase { unit: *id });
            }
            if t.jammer {
                commands.push(Command::SetJamming {
                    unit: *id,
                    mode: "off".into(),
                    contacts: vec![],
                });
            }
            if u.kind == PlatformKind::Sub {
                commands.push(Command::SetSonar {
                    unit: *id,
                    active: false,
                });
                commands.push(Command::SetRadar {
                    unit: *id,
                    active: false,
                });
            }
            notes.push(notice(*id,t.contact,"task_review",format!("持续任务复核：{}，解除旧指派并按任务类型撤收",if invalid{"航迹失效"}else if low{"返航储备不足"}else if empty{"进攻弹药耗尽"}else{"复核期限到达"}),"Standing-task review: expired, invalid track, empty offensive load or return reserve; cancel owned task and recover".into()));
            remove.push(*id);
        }
        for id in remove {
            self.tasks.remove(&id);
        }
        self.auto_defence.retain(|id,_|obs.units.iter().any(|u|u.id==*id));
        for id in self.auto_defence.keys() {
            let u=obs.units.iter().find(|u|u.id==*id).unwrap();
            let assigned_air=u.assigned.is_some_and(|id|obs.contacts.iter().any(|c|c.id==id&&matches!(c.category,PlatformKind::Aircraft|PlatformKind::Missile)));
            if (u.fire_on||assigned_air) && self.defence_held(obs,db,u) {
                if u.fire_on {commands.push(Command::SetWeaponsFree{unit:*id,free:false});}
                if assigned_air {commands.push(Command::ClearTarget{unit:*id});}
                notes.push(notice(*id,None,"defence_budget","自动防空本窗耗弹/保留额或复核期限已到，暂停远距主动交战；当前近程威胁仍由核心自卫处理".into(),"Automatic SAM expenditure/reserve or review deadline reached; hold distant discretionary engagements and retain core self-defence".into()));
            }
        }
        self.dispatch_due(obs, db, &mut commands, &mut notes);
        (commands, notes)
    }
    /// Whole-batch limits apply to both models and FSM. Manual player commands
    /// do not pass through this function. Limits below are commander policies.
    pub fn guard(
        &mut self,
        obs: &Observation,
        db: &Database,
        commands: Vec<Command>,
    ) -> (Vec<Command>, Vec<StaffNote>) {
        let mut out = Vec::new();
        let mut notes = Vec::new();
        let mut reserved = BTreeMap::new();
        let mut launches = BTreeSet::new();
        let mut allocated_weapons = BTreeSet::new();
        let returning: BTreeSet<_> = commands
            .iter()
            .filter_map(|c| {
                if let Command::ReturnToBase { unit } = c {
                    Some(*unit)
                } else {
                    None
                }
            })
            .collect();
        let hosts = obs.units.iter().filter(|u| u.deck.is_some()).count().max(1);
        let mut cap = obs
            .units
            .iter()
            .filter(|u| u.mission.as_deref() == Some("cap"))
            .count();
        let mut strikes = obs
            .units
            .iter()
            .filter(|u| u.mission.as_deref() == Some("strike"))
            .count();
        for mut cmd in commands {
            if obs.units.iter().any(|u|u.id==unit_of(&cmd)&&u.manual_control) {continue;}
            let mut reason = None;
            match &mut cmd {
                Command::LaunchAircraft {
                    unit,
                    class,
                    count,
                    mission,
                    profile,
                } => {
                    let h = obs.units.iter().find(|u| u.id == *unit);
                    if !launches.insert(*unit) {
                        reason = Some("one launch allocation per host and observation");
                    }
                    if let Some(h) = h {
                        let ready = h
                            .deck
                            .as_ref()
                            .and_then(|d| d.ready.iter().find(|(c, _)| c == class))
                            .map(|(_, n)| *n)
                            .unwrap_or(0);
                        let was = *count;
                        let defensive = db.platforms.get(class).is_some_and(|p| {
                            p.weapons.iter().any(|m| {
                                db.weapons
                                    .get(&m.weapon)
                                    .is_some_and(|d| d.kind == WeaponKind::Aam)
                            })
                        });
                        *count =
                            (*count)
                                .min(ready)
                                .min(if matches!(mission, AirMission::Cap { .. }) {
                                    2
                                } else {
                                    4
                                });
                        if matches!(mission, AirMission::Strike { .. }) {
                            if let AirMission::Strike { contact } = mission {
                                if !obs.contacts.iter().any(|c| c.id == *contact && fresh(c)) {
                                    reason = Some(
                                        "strike departure requires an observed current target",
                                    );
                                }
                            }
                            if strikes >= 8 {
                                reason = Some("active strike budget already committed");
                            }
                            if defensive && ready > 2 {
                                *count = (*count).min(ready - 2);
                            }
                            if let AirMission::Strike { contact } = mission {
                                if !self.target_allowed(*contact, obs.tick) {
                                    reason =
                                        Some("unconfirmed attack outcome requires reassessment");
                                }
                            }
                        } else if matches!(mission, AirMission::Cap { .. })
                            && cap >= 2 * hosts.min(4)
                        {
                            reason =
                                Some("CAP budget already committed; retain replacement inventory");
                        }
                        if (support_class(class, db) || matches!(mission, AirMission::Recon { .. }))
                            && !self.support_launch_allowed(obs, h, class)
                        {
                            reason = Some("support replacement waits for station risk review");
                        }
                        if *count == 0 {
                            reason = Some("no unreserved ready aircraft");
                        }
                        if let AirMission::Cap { station, .. } = mission {
                            if super::assessment::danger(obs, db, *station)
                                || (!super::assessment::covered(obs, db, *station)
                                    && h.pos.range_to(*station) > 35.0)
                            {
                                *station = self.support_station(obs, db, h, h.pos);
                                notes.push(notice(*unit,None,"cover_budget","CAP原站位缺乏本方可用掩护或存在已知威胁，改在可复核的舰队防区组织".into(),"CAP station lacks usable friendly cover or has known threats; allocate within the reviewed fleet defence area".into()));
                            }
                        }
                        let p = profile.get_or_insert(harpoon_core::commands::LaunchProfile {
                            waypoints: Vec::new(),
                            altitude_m: 3000.0,
                            weapons_free: false,
                            manual_strike: matches!(mission, AirMission::Strike { .. }),
                            engagement: None,
                        });
                        if p.waypoints.is_empty() {
                            let destination = match mission {
                                AirMission::Strike { contact } => obs
                                    .contacts
                                    .iter()
                                    .find(|c| c.id == *contact)
                                    .map(|c| c.estimate),
                                AirMission::Cap { station, .. } => Some(*station),
                                AirMission::Asw { area, .. } => Some(*area),
                                AirMission::Recon { point } => Some(*point),
                                AirMission::Rtb => None,
                            };
                            if let Some(point) = destination {
                                p.waypoints.push(point);
                            }
                        }
                        p.engagement = Some(harpoon_core::commands::LaunchEngagement {
                            air_only: !matches!(mission, AirMission::Strike { .. }),
                            max_in_flight: 2,
                        });
                        if reason.is_none() {
                            if matches!(mission, AirMission::Cap { .. }) {
                                cap += *count as usize;
                            }
                            if matches!(mission, AirMission::Strike { .. }) {
                                strikes += *count as usize;
                            }
                            self.flights.push(FlightOrder {
                                home: *unit,
                                class: class.clone(),
                                target: if let AirMission::Strike { contact } = mission {
                                    Some(*contact)
                                } else {
                                    None
                                },
                                until: obs.tick + 60,
                            });
                        }
                        if was != *count {
                            notes.push(notice(
                                *unit,
                                None,
                                "resource_budget",
                                format!(
                                    "出击预算将本批{}架调整为{}架，保留后续接替库存",
                                    was, count
                                ),
                                format!(
                                    "Sortie allocation {} -> {}; preserve replacement inventory",
                                    was, count
                                ),
                            ));
                        }
                    }
                }
                Command::Fire {
                    unit,
                    contact,
                    weapon,
                    rounds,
                } => {
                    if returning.contains(unit) {
                        reason = Some("aircraft is ordered to recover in this batch");
                    }
                    if !allocated_weapons.insert(key(*unit, weapon)) {
                        reason = Some("one release allocation per weapon and observation");
                    }
                    if self
                        .scheduled
                        .iter()
                        .any(|p| p.unit == *unit && p.weapon == *weapon)
                    {
                        reason = Some("a scheduled release already owns this weapon allocation");
                    }
                    if let Some(def) = db.weapons.get(weapon) {
                        if let Some(c) = obs.contacts.iter().find(|c| c.id == *contact) {
                            if offensive(def.kind) && !fresh(c) {
                                reason = Some("no current firing track");
                            }
                            if offensive(def.kind) && !self.target_allowed(*contact, obs.tick) {
                                reason = Some("repeat attack held for outcome reassessment");
                            }
                            let pending: u32 = self
                                .scheduled
                                .iter()
                                .filter(|p| p.contact == *contact)
                                .map(|p| p.rounds as u32)
                                .sum();
                            let n = reserved
                                .entry(*contact)
                                .or_insert(c.own_in_flight + pending);
                            let limit: u32 =
                                if matches!(def.kind, WeaponKind::Ascm | WeaponKind::Slcm) {
                                    8
                                } else {
                                    4
                                };
                            if *n >= limit {
                                reason = Some(
                                    "side-wide in-flight and batch ammunition allocation reached",
                                );
                            }
                            *rounds = (*rounds).min(limit.saturating_sub(*n).min(4) as u8);
                            if *rounds == 0 {
                                reason = Some("empty ammunition allocation");
                            }
                            if reason.is_none() {
                                *n += *rounds as u32;
                                self.intents.insert(key(*unit, weapon), *contact);
                            }
                        }
                    }
                }
                Command::RetaskAircraft {
                    unit,
                    mission: AirMission::Asw { area, .. },
                    ..
                } => {
                    if let Some(t) = self.asw.get(unit) {
                        let evading = obs.units.iter().find(|u| u.id == *unit).is_some_and(|u| {
                            super::assessment::danger(obs, db, u.pos)
                                && self
                                    .friendly_home(obs, u)
                                    .is_some_and(|h| area.range_to(h.pos) < 3.0)
                        });
                        if !evading {
                            *area = t.area;
                        }
                        notes.push(notice(*unit,t.contact,"asw_assignment",format!("复核反潜分区：{}；搜索区随本方宿主更新",if t.contact.is_some(){"接替新鲜潜艇接触"}else{"独立搜索扇区"}),"ASW review: assign a fresh contact or distinct moving-home sector; cancel the previous search area".into()));
                    }
                }
                Command::SetJamming {
                    unit,
                    mode,
                    contacts,
                } if mode != "off" => {
                    if self.ew.get(unit).is_none_or(|s| s.until <= obs.tick) {
                        *mode = "off".into();
                        contacts.clear();
                        notes.push(notice(
                            *unit,
                            None,
                            "support_end",
                            "没有仍有效的所属突击任务，关闭进攻干扰".into(),
                            "No live supported strike package; stop offensive jamming".into(),
                        ));
                    }
                }
                Command::RetaskAircraft{unit,mission:AirMission::Cap{..},..} => {
                    out.push(Command::SetEngagementPolicy{unit:*unit,air_only:true,max_in_flight:2});
                }
                Command::AssignTarget{unit,contact} => {
                    if let Some(u)=obs.units.iter().find(|u|u.id==*unit&&sam_stock(u,db)>0) {
                        if obs.contacts.iter().any(|c|c.id==*contact&&matches!(c.category,PlatformKind::Aircraft|PlatformKind::Missile)) {
                            self.track_auto_defence(obs,db,u);
                            if self.defence_held(obs,db,u) {reason=Some("automatic SAM assignment budget reached; reassess distant engagement");}
                        }
                    }
                }
                Command::SetEngagementPolicy{unit,air_only,max_in_flight} => {
                    if obs.units.iter().any(|u|u.id==*unit&&u.mission.as_deref()==Some("cap")) {
                        *air_only=true;*max_in_flight=(*max_in_flight).clamp(1,2);
                    }
                }
                Command::SetWeaponsFree { unit, free } if *free => {
                    if let Some(u) = obs.units.iter().find(|u| u.id == *unit) {
                        if u.mission.as_deref()==Some("cap") {
                            out.push(Command::SetEngagementPolicy{unit:*unit,air_only:true,max_in_flight:2});
                        }
                        self.track_auto_defence(obs,db,u);
                        if self.defence_held(obs,db,u) {
                            *free = false;
                            notes.push(notice(*unit,None,"defence_reserve","远程防空弹药进入保留预算，限制主动远距交战；系统近程自卫仍由核心执行".into(),"SAM reserve reached: hold discretionary long-range engagements; core self-defence still applies".into()));
                        }
                    }
                }
                Command::ClearTarget { unit } => {
                    if !obs
                        .units
                        .iter()
                        .any(|u| u.id == *unit && u.mission.as_deref() == Some("strike"))
                    {
                        if let Some(t) = self.tasks.get_mut(unit) {
                            t.contact = None;
                        }
                    }
                }
                Command::ReturnToBase { unit } | Command::Stop { unit } => {
                    self.release_manual(*unit);
                }
                _ => {}
            }
            let unit = unit_of(&cmd);
            if let Some(reason) = reason {
                notes.push(notice(
                    unit,
                    None,
                    "resource_hold",
                    format!("AI预算暂缓：{}", reason),
                    format!("AI allocation held: {}", reason),
                ));
                continue;
            }
            let (contact, route, mut air, jammer) = match &cmd {
                Command::AssignTarget { unit, contact }
                    if obs
                        .units
                        .iter()
                        .any(|u| u.id == *unit && u.mission.as_deref() == Some("strike"))
                        && obs.contacts.iter().any(|c| {
                            c.id == *contact
                                && matches!(
                                    c.category,
                                    PlatformKind::Aircraft | PlatformKind::Missile
                                )
                        }) =>
                {
                    out.push(cmd);
                    continue;
                }
                Command::AssignTarget { contact, .. } => (Some(*contact), false, false, false),
                Command::Goto { waypoints, .. } if !waypoints.is_empty() => {
                    (None, true, false, false)
                }
                Command::RetaskAircraft { mission, .. } => (
                    if let AirMission::Strike { contact } = mission {
                        Some(*contact)
                    } else {
                        None
                    },
                    true,
                    true,
                    false,
                ),
                Command::SetJamming { mode, .. } if mode != "off" => (None, false, false, true),
                _ => {
                    out.push(cmd);
                    continue;
                }
            };
            air |= obs
                .units
                .iter()
                .any(|u| u.id == unit && u.kind == PlatformKind::Aircraft);
            let t = self.tasks.entry(unit).or_insert(Task {
                contact,
                expires_at: obs.tick + 600,
                reviewed_at: obs.tick,
                route,
                air,
                jammer,
            });
            if matches!(
                cmd,
                Command::AssignTarget { .. } | Command::RetaskAircraft { .. }
            ) {
                let changed = t.contact != contact;
                t.contact = contact;
                if changed
                    || contact
                        .is_none_or(|cid| obs.contacts.iter().any(|c| c.id == cid && fresh(c)))
                {
                    self.lost_tracks.remove(&unit);
                }
            } else {
                t.contact = contact.or(t.contact);
            }
            t.route |= route;
            t.air |= air;
            t.jammer |= jammer;
            t.reviewed_at = obs.tick;
            t.expires_at = obs.tick + 600;
            out.push(cmd);
        }
        let (out, scheduled_notes) = self.coordinate_batch(obs, db, out);
        notes.extend(scheduled_notes);
        (out, self.dedupe(notes, obs.tick))
    }
    fn coordinate_batch(
        &mut self,
        obs: &Observation,
        db: &Database,
        commands: Vec<Command>,
    ) -> (Vec<Command>, Vec<StaffNote>) {
        let mut windows: BTreeMap<ContactId, Vec<(f64, PlatformKind)>> = BTreeMap::new();
        for c in &commands {
            if let Command::Fire {
                unit,
                contact,
                weapon,
                ..
            } = c
            {
                if let (Some(u), Some(t), Some(w)) = (
                    obs.units.iter().find(|u| u.id == *unit),
                    obs.contacts.iter().find(|c| c.id == *contact),
                    db.weapons.get(weapon),
                ) {
                    if matches!(w.kind, WeaponKind::Ascm | WeaponKind::Slcm) {
                        windows.entry(*contact).or_default().push((
                            harpoon_core::rules::damage_runtime::flight_seconds(w,u.pos.range_to(t.estimate)),
                            u.kind,
                        ));
                    }
                }
            }
        }
        let mut out = Vec::new();
        let mut notes = Vec::new();
        for command in commands {
            if let Command::Fire {
                unit,
                contact,
                weapon,
                rounds,
            } = &command
            {
                if let (Some(group), Some(u), Some(t), Some(w)) = (
                    windows.get(contact),
                    obs.units.iter().find(|u| u.id == *unit),
                    obs.contacts.iter().find(|c| c.id == *contact),
                    db.weapons.get(weapon),
                ) {
                    let kinds: BTreeSet<_> =
                        group.iter().map(|(_, kind)| format!("{kind:?}")).collect();
                    if kinds.len() > 1 && matches!(w.kind, WeaponKind::Ascm | WeaponKind::Slcm) {
                        let latest = group.iter().map(|(eta, _)| *eta).fold(0.0, f64::max);
                        let own = harpoon_core::rules::damage_runtime::flight_seconds(w,u.pos.range_to(t.estimate));
                        let delay = (((latest - own).max(0.0) / 30.0).round() as i64) * 30;
                        if (30..=300).contains(&delay) {
                            self.scheduled.push(QueuedRelease {
                                unit: *unit,
                                contact: *contact,
                                weapon: weapon.clone(),
                                rounds: *rounds,
                                fire_at: obs.tick + delay,
                                deadline: obs.tick + delay + 120,
                            });
                            notes.push(notice(*unit,Some(*contact),"timed_release_queued",format!("本批舰空发射预计错时{}秒；T{}复核实际航迹/弹药后才提交，失效即取消",delay,obs.tick+delay),format!("Coordinate mixed-source release with {}s delay; revalidate track and ammunition at T{} or cancel",delay,obs.tick+delay)));
                            continue;
                        }
                    }
                }
            }
            out.push(command);
        }
        (out, notes)
    }
    fn dispatch_due(
        &mut self,
        obs: &Observation,
        db: &Database,
        commands: &mut Vec<Command>,
        notes: &mut Vec<StaffNote>,
    ) {
        let pending = std::mem::take(&mut self.scheduled);
        let mut allocated: BTreeMap<ContactId, u32> = BTreeMap::new();
        for p in pending {
            let u = obs.units.iter().find(|u| u.id == p.unit);
            let cancelled=obs.tick>p.deadline||u.is_none_or(|u|u.mission.as_deref()==Some("rtb")||!u.weapons.iter().any(|m|m.weapon==p.weapon&&m.ammo>=p.rounds as u16))
                ||commands.iter().any(|c|matches!(c,Command::ReturnToBase{unit}|Command::Stop{unit}|Command::ClearTarget{unit}if *unit==p.unit));
            if cancelled {
                notes.push(notice(
                    p.unit,
                    Some(p.contact),
                    "timed_release_cancel",
                    "定时发射取消：任务结束、无弹或超过有效窗口".into(),
                    "Timed release cancelled: task ended, no ammunition or window expired".into(),
                ));
                continue;
            }
            let valid = obs
                .contacts
                .iter()
                .find(|c| c.id == p.contact && fresh(c) && self.target_allowed(c.id, obs.tick));
            let can_fire = valid.is_some_and(|t| {
                let n = allocated.entry(t.id).or_insert(t.own_in_flight);
                let good = *n + p.rounds as u32 <= 8
                    && db
                        .weapons
                        .get(&p.weapon)
                        .is_some_and(|d| u.unwrap().pos.range_to(t.estimate) <= d.range_nm * 0.98)
                    && u.unwrap()
                        .weapons
                        .iter()
                        .any(|m| m.weapon == p.weapon && m.cooldown_s <= 0.0);
                if good && obs.tick >= p.fire_at {
                    *n += p.rounds as u32;
                }
                good
            });
            if obs.tick >= p.fire_at && can_fire {
                self.intents.insert(key(p.unit, &p.weapon), p.contact);
                commands.push(Command::Fire {
                    unit: p.unit,
                    contact: p.contact,
                    weapon: p.weapon,
                    rounds: p.rounds,
                });
                notes.push(notice(
                    p.unit,
                    Some(p.contact),
                    "timed_release_submit",
                    "定时发射复核通过，提交核心；实际放弹以武器回执为准".into(),
                    "Timed release revalidated and submitted; confirm from actual weapon receipts"
                        .into(),
                ));
            } else {
                self.scheduled.push(p);
            }
        }
    }
    pub fn summary(&self, obs: &Observation) -> Value {
        json!({"policy":"game commander allocation, not adjudication","observed_at":self.last_tick.unwrap_or(obs.tick),"current_tick":obs.tick,
            "tasks":self.tasks,"attack_account":self.attacks,"support_bindings":self.ew,"asw_assignments":self.asw,
            "unattributed_actual_expenditure":self.unattributed_rounds,
            "scheduled_releases":self.scheduled,
            "automatic_defence_budgets":self.auto_defence,
            "known_air_origins":self.origins,"actual_offensive_rounds_by_unit":self.expenditure_by_unit,
            "limits":{"cap_per_host":2,"cap_global":2*obs.units.iter().filter(|u|u.deck.is_some()).count().clamp(1,4),"active_strike_aircraft":8,"per_target_offensive_in_flight":8,"task_review_s":600},
            "unknown_damage_is_not_zero":true})
    }
}

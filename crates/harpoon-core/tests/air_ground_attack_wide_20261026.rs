//! Source-only candidates, native Visual and actual source AA only. Not yet run.
use harpoon_core::rules::{aaw2026 as aa, aircombat2026 as ac};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
fn seed() -> u64 {
    (0..1_000_000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(1, 3, "aaw:gun:3:ciws_30mm:0:1:1:1"));
            1 + (r.next_f64() * 100.) as u8 == 100
        })
        .unwrap()
}
fn fixture(dynamic: bool, escort: bool) -> World {
    fixture_seed(dynamic, escort, None)
}
fn fixture_seed(dynamic: bool, escort: bool, selected_seed: Option<u64>) -> World {
    let target = |size| json!({"source":"scenario_assumption: explicit controlled native visual profile","size":size,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    let mut v = json!({"id":"wide-9-6-dynamic","name":"scenario_assumption: native visual bombing and actual defensive fire","seed":selected_seed.unwrap_or_else(seed),"duration_s":1000,"weather":{"visibility_nm":40,"jam":[0,0]},"context":{
  "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
  "aviation_2026":{"source":"US Aircraft2025 B44/H1-3 source load; controlled fuel",
   "no_aa_surface_attack_source":if dynamic{Value::Null}else{json!("scenario_assumption: controlled range condition; actual AA must override if it occurs")},
   "surface_aimpoints":{"range":{"source":"scenario_assumption: published fixed control point","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}},
   "loads":{"us_f18e_mk82":{"payload_kg":6395,"tanks":[{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"}],"external_weapon_rounds":{"us_mk82":6},"source":"US2025 B44/H4-2/H1-3: actual cited Mk82 strike load"}},"parking":{},"initial_fuel_nm":{"STRIKE":1000}},
  "detection_2026":{"bindings":{"us_f18e_mk82:visual":{"type":"visual","source":"scenario_assumption: fitted lookout"},"us_ford:visual":{"type":"visual","source":"scenario_assumption: fitted defensive lookout"}},"targets":{"us_f18e_mk82":target(5),"cn_001":target(0),"us_ford":target(0)},"environment":{"source":"scenario_assumption: clear daylight","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
  "units":[{"side":"Blue","class":"us_f18e_mk82","name":"STRIKE","pos":[0,-460.0/3600.0],"course_deg":0,"speed_kts":460,"altitude_m":1000,"fire_on":false},
   {"side":"Red","class":"cn_001","name":"TARGET","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false}]});
    if escort {
        v["units"].as_array_mut().unwrap().push(json!({"side":"Red","class":"us_ford","name":"ESCORT","pos":[0,1],"course_deg":180,"speed_kts":0,"fire_on":false}));
    }
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    ac::configure_database(&mut db);
    for class in ["us_f18e_mk82", "cn_001", "us_ford"] {
        let p = db.platforms.get_mut(class).unwrap();
        p.sensors = vec!["visual".into()];
        if class == "cn_001" {
            p.weapons.clear();
        }
        if class == "us_ford" {
            p.weapons.retain(|m| m.weapon == "ciws_30mm");
        }
    }
    let mut w =
        harpoon_core::load_scenario(serde_json::from_value::<Scenario>(v).unwrap(), db).unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    w
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn fire(w: &mut World, aimpoint: &str) {
    let a = id(w, "STRIKE");
    let t = id(w, "TARGET");
    let c = w
        .find_contact(Side::Blue, t)
        .expect("actual native visual strike target");
    assert!(harpoon_core::rules::detection2026::known_category(w, Side::Blue, t).is_some());
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::AirCombat {
                unit: a,
                action: ac::Action::SurfaceAttack {
                    contact: c,
                    weapon: "us_mk82".into(),
                    rounds: 6,
                    delivery: ac::Delivery::Glide,
                    aimpoint: aimpoint.into(),
                },
            },
        )],
    );
}
fn detail(w: &World) -> Value {
    w.events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "9.6.source_bomb_attack" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .expect("source dynamic attack must pass admission and resolve")
}
#[test]
fn g01_native_visual_target_does_not_require_a_static_known_range_or_author_no_aa_permission() {
    let mut w = fixture(true, false);
    fire(&mut w, "");
    let d = detail(&w);
    assert_eq!(d["probability"], 0.75);
    assert_eq!(d["total_hits"].is_number(), true);
}
#[test]
fn g02_existing_fixed_no_aa_control_uses_native_contact_and_keeps_source_probability() {
    let mut w = fixture(false, false);
    fire(&mut w, "range");
    assert_eq!(detail(&w)["probability"], 0.75);
}
#[test]
fn g03_actual_source_ciws_fire_at_this_aircraft_replaces_no_aa_with_heavy_aa_modifier() {
    let mut w = fixture(false, true);
    let (a, e) = (id(&w, "STRIKE"), id(&w, "ESCORT"));
    assert!(
        w.find_contact(Side::Red, a).is_some(),
        "actual defensive lookout must see this aircraft"
    );
    let mount = w
        .entity(e)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "ciws_30mm")
        .unwrap();
    assert_eq!(aa::fire_at(&mut w, e, a, mount, 1).unwrap(), 1);
    assert!(
        w.entity(a).unwrap().alive,
        "source-derived ordinary scenario seed selected a real gun miss; no private hit rewrite"
    );
    assert!(w.events.iter().any(
        |event| matches!(&event.kind,EventKind::RulesEvent{rule,..}if rule=="8.1.3.source_gun")
    ));
    fire(&mut w, "range");
    assert_eq!(detail(&w)["probability"],0.65,"A glide35%, stationary+2,Advanced+5,actual HeavyAA-1; an obsolete author NoAA string cannot grant its+1");
}

fn gun_miss_seed(at: i64) -> u64 {
    (0..1_000_000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(
                at,
                3,
                &format!("aaw:gun:3:ciws_30mm:0:{at}:1:1"),
            ));
            1 + (r.next_f64() * 100.) as u8 == 100
        })
        .unwrap()
}
fn bomb_command(w: &World, rounds: u16) -> (Side, Command) {
    let a = id(w, "STRIKE");
    let c = w.find_contact(Side::Blue, id(w, "TARGET")).unwrap();
    (
        Side::Blue,
        Command::AirCombat {
            unit: a,
            action: ac::Action::SurfaceAttack {
                contact: c,
                weapon: "us_mk82".into(),
                rounds,
                delivery: ac::Delivery::Glide,
                aimpoint: "range".into(),
            },
        },
    )
}
#[test]
fn g04_same_tick_native_ciws_and_bomb_clients_both_orders_use_heavy_aa_and_spend_once() {
    for reverse in [false, true] {
        let mut w = fixture_seed(false, true, Some(gun_miss_seed(2)));
        let (a, e) = (id(&w, "STRIKE"), id(&w, "ESCORT"));
        let contact = w
            .find_contact(Side::Red, a)
            .expect("native defensive visual contact");
        let gun = w
            .entity(e)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "ciws_30mm")
            .unwrap()
            .ammo;
        let bomb = bomb_command(&w, 6);
        let defense = (
            Side::Red,
            Command::Fire {
                unit: e,
                contact,
                weapon: "ciws_30mm".into(),
                rounds: 1,
            },
        );
        let commands = if reverse {
            vec![defense, bomb]
        } else {
            vec![bomb, defense]
        };
        harpoon_core::sim::tick(&mut w, &commands);
        assert!(w.entity(a).unwrap().alive);
        assert_eq!(detail(&w)["probability"], 0.65);
        assert_eq!(
            w.entity(e)
                .unwrap()
                .weapons
                .iter()
                .find(|m| m.weapon == "ciws_30mm")
                .unwrap()
                .ammo,
            gun - 1
        );
        assert_eq!(
            w.entity(a)
                .unwrap()
                .weapons
                .iter()
                .find(|m| m.weapon == "us_mk82")
                .unwrap()
                .ammo,
            0
        );
        assert_eq!(w.events.iter().filter(|event|matches!(&event.kind,EventKind::RulesEvent{rule,..}if rule=="9.6.source_bomb_attack")).count(),1);
    }
}
#[test]
fn g05_conflicting_same_tick_bomb_declarations_do_not_buy_a_first_client_pass() {
    for reverse in [false, true] {
        let mut w = fixture(false, false);
        let a = id(&w, "STRIKE");
        let six = bomb_command(&w, 6);
        let four = bomb_command(&w, 4);
        let commands = if reverse {
            vec![four, six]
        } else {
            vec![six, four]
        };
        harpoon_core::sim::tick(&mut w, &commands);
        assert_eq!(
            w.entity(a)
                .unwrap()
                .weapons
                .iter()
                .find(|m| m.weapon == "us_mk82")
                .unwrap()
                .ammo,
            6
        );
        assert!(!w.events.iter().any(|event|matches!(&event.kind,EventKind::RulesEvent{rule,..}if rule=="9.6.source_bomb_attack")));
        assert!(w.events.iter().any(
            |event| matches!(&event.kind,EventKind::RulesEvent{rule,..}if rule=="9.6.pass_conflict")
        ));
    }
}
#[test]
fn g06_cancel_and_declare_clients_both_orders_never_release_stores() {
    for reverse in [false, true] {
        let mut w = fixture(false, false);
        let a = id(&w, "STRIKE");
        let bomb = bomb_command(&w, 6);
        let cancel = (
            Side::Blue,
            Command::AirCombat {
                unit: a,
                action: ac::Action::CancelSurfaceAttack {},
            },
        );
        let commands = if reverse {
            vec![cancel, bomb]
        } else {
            vec![bomb, cancel]
        };
        harpoon_core::sim::tick(&mut w, &commands);
        assert_eq!(
            w.entity(a)
                .unwrap()
                .weapons
                .iter()
                .find(|m| m.weapon == "us_mk82")
                .unwrap()
                .ammo,
            6
        );
        assert!(!w.events.iter().any(|event|matches!(&event.kind,EventKind::RulesEvent{rule,..}if rule=="9.6.source_bomb_attack")));
    }
}

#[test]
fn g07_direct_declarations_are_queued_deduplicated_and_roundtrip_before_the_normal_defense_scan() {
    let mut w = fixture(false, false);
    let a = id(&w, "STRIKE");
    let contact = w.find_contact(Side::Blue, id(&w, "TARGET")).unwrap();
    for _ in 0..2 {
        ac::surface_attack(
            &mut w,
            a,
            contact,
            "us_mk82",
            6,
            ac::Delivery::Glide,
            "range",
        )
        .unwrap();
    }
    assert_eq!(
        w.entity(a)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "us_mk82")
            .unwrap()
            .ammo,
        6,
        "valid direct call declares, it cannot skip defensive phases and release immediately"
    );
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for state in [&mut w, &mut restored] {
        harpoon_core::sim::tick(state, &[]);
        ac::resolve_surface_attacks(state);
        assert_eq!(
            state
                .entity(a)
                .unwrap()
                .weapons
                .iter()
                .find(|m| m.weapon == "us_mk82")
                .unwrap()
                .ammo,
            0
        );
        assert_eq!(state.events.iter().filter(|event|matches!(&event.kind,EventKind::RulesEvent{rule,..}if rule=="9.6.source_bomb_attack")).count(),1);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
#[test]
fn g08_actual_defensive_ciws_kill_before_release_preserves_all_bomb_inventory_in_both_orders() {
    let selected = (0..1_000_000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(2, 3, "aaw:gun:3:ciws_30mm:0:2:1:1"));
            1 + (r.next_f64() * 100.) as u8 == 1
        })
        .unwrap();
    for reverse in [false, true] {
        let mut w = fixture_seed(false, true, Some(selected));
        let (a, e) = (id(&w, "STRIKE"), id(&w, "ESCORT"));
        let contact = w.find_contact(Side::Red, a).unwrap();
        let bomb = bomb_command(&w, 6);
        let defense = (
            Side::Red,
            Command::Fire {
                unit: e,
                contact,
                weapon: "ciws_30mm".into(),
                rounds: 1,
            },
        );
        let commands = if reverse {
            vec![defense, bomb]
        } else {
            vec![bomb, defense]
        };
        harpoon_core::sim::tick(&mut w, &commands);
        assert!(
            !w.entity(a).unwrap().alive,
            "real source gunD100=1 must physically shoot down the aircraft; no HP/hit mutation"
        );
        assert_eq!(
            w.entity(a)
                .unwrap()
                .weapons
                .iter()
                .find(|m| m.weapon == "us_mk82")
                .unwrap()
                .ammo,
            6
        );
        assert!(!w.events.iter().any(|event|matches!(&event.kind,EventKind::RulesEvent{rule,..}if rule=="9.6.source_bomb_attack")));
    }
}

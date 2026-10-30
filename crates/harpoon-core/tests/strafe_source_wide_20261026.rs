//! Artifact-only §9.6.9 source candidates, not registered/executed.
//! Existing SurfaceAttack/Strafe JSON, actual source gun profile; no future enums.
use harpoon_core::rules::aviation2026 as av;
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
const AIRCRAFT: &str = "us_f18e_mk82";
const GUN: &str = "us_m61a2_20mm_source_gun";
fn seed(defense: Option<&str>) -> u64 {
    (0..1_000_000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(2, 1, "9.6.9:strafe"));
            let d100 = 1 + (r.next_f64() * 100.) as u8;
            let d6 = 1 + (r.next_f64() * 6.) as u8;
            d100 <= 20
                && d6 == 6
                && defense.is_none_or(|weapon| {
                    let mut g = Rng::new(*seed).derive(Rng::hash_tag(
                        2,
                        3,
                        &format!("aaw:gun:3:{weapon}:0:2:1:1"),
                    ));
                    1 + (g.next_f64() * 100.) as u8 == 100
                })
        })
        .expect("preconstruction ordinary source seed; no live dice edits")
}
fn input(ground: bool, altitude: f64, escort: Option<&str>, weapon: Option<&str>) -> Value {
    let mut value = json!({"id":"source-strafing","name":"real F18E M61A2 book-abstraction strafing, no invented ammunition","seed":seed(weapon),"duration_s":1000,
    "weather":{"visibility_nm":40,"jam":[0,0]},"context":{
        "rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":0,"precipitation":"None"},
        "strafe_2026":{"source":"English2026 9.6.9/14.1.4; US Aircraft2025 PDF49/B44 actual M61A2 Vulcan20mm(4.2); only source abstract gun pass, ammunition/penetration unresolved","enabled_aircraft":[AIRCRAFT],"ammunition_model":"book_abstract_gun_rating"},
        "aviation_2026":{"source":"existing verified F18E sixMk82 source load retained; integral gun use does not jettison/consume other weapons","loads":{},"parking":{},"initial_fuel_nm":{"STRIKE":800},"no_aa_surface_attack_source":"scenario_assumption: controlled weapons-hold target; actual AA overrides",
            "surface_aimpoints":{"range":{"source":"scenario_assumption: fixed controlled target coordinates allocated Blue","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
        "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: clear daylight native lookouts","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}
    },"units":[
        {"side":"Blue","class":AIRCRAFT,"name":"STRIKE","pos":[0,-460./3600.],"course_deg":0,"speed_kts":460,"altitude_m":altitude,"fire_on":false},
        {"side":"Red","class":if ground{"airfield"}else{"cn_001"},"name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}
    ]});
    value["context"]["aviation_2026"]["loads"][AIRCRAFT] = json!({"payload_kg":6395,"tanks":[{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"}],"external_weapon_rounds":{"us_mk82":6},"source":"existing verified US2025 B44/H1-3/H4-2 Mk82 strike load; source integral gun separately, no fabricated20mm rounds/mass"});
    if ground {
        value["context"]["ground_2026"] = json!({"source":"English2026 11.1 scenario-defined unarmored equipment point; source strafing criticals stop its function without invented DP","facilities":{"TARGET":{"template":null,"custom":{"size":"F","damage_points":8,"vertical_armor":0,"horizontal_armor":0,"effect":"normal","source":"scenario_assumption: one exposed F/8DP unarmored functional point, no real personnel/vehicle casualty count"},"source":"scenario_assumption: controlled ground strafing receiver"}}});
    }
    for class in [AIRCRAFT, "cn_001", "airfield", "us_burke_2a", "us_ford"] {
        value["context"]["detection_2026"]["bindings"][format!("{class}:visual")] =
            json!({"type":"visual","source":"scenario_assumption: fitted native lookout"});
        value["context"]["detection_2026"]["targets"][class] = json!({"source":"scenario_assumption: explicit clear-range native Visual signature; land profileF satisfies5.8.6","size":if class==AIRCRAFT||class=="airfield"{5}else{0},"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    }
    value["context"]["detection_2026"]["camouflage"]["airfield"] = json!(0);
    if let Some(class) = escort {
        value["units"].as_array_mut().unwrap().push(json!({"side":"Red","class":class,"name":"ESCORT","pos":[0,1],"course_deg":180,"speed_kts":0,"fire_on":false}));
    }
    value
}
fn world(ground: bool, altitude: f64, escort: Option<&str>, gun: Option<&str>) -> World {
    let value = input(ground, altitude, escort, gun);
    let parsed: Scenario = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&parsed).unwrap()["context"]["strafe_2026"],
        value["context"]["strafe_2026"],
        "explicit source gun abstraction must survive Scenario parsing"
    );
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    harpoon_core::rules::aaw2026::configure_database(&mut db);
    // The normal registry recreates the strike alias from this base; select
    // the controlled native lookout before that real registration step.
    db.platforms.get_mut("us_f18e").unwrap().sensors = vec!["visual".into()];
    if let (Some(class), Some(gun)) = (escort, gun) {
        db.platforms
            .get_mut(class)
            .unwrap()
            .weapons
            .retain(|m| m.weapon == gun);
    }
    let mut w = harpoon_core::load_scenario(parsed, db).unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    assert!(
        w.entity(named(&w, "STRIKE"))
            .unwrap()
            .pos
            .range_to(harpoon_core::Vec2 { x: 0., y: 0. })
            < 1e-9,
        "real native source cruise arrives at exact overflight anchor"
    );
    w
}
fn named(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn attack(w: &World) -> (Side, Command) {
    let contact = w
        .find_contact(Side::Blue, named(w, "TARGET"))
        .expect("native actual target contact");
    (Side::Blue,serde_json::from_value(json!({"cmd":"air_combat","unit":named(w,"STRIKE"),"action":{"type":"surface_attack","contact":contact,"weapon":GUN,"rounds":1,"delivery":"Strafe","aimpoint":"range"}})).expect("existing SurfaceAttack/Strafe JSON, not a future enum"))
}
fn stocks(w: &World, id: EntityId) -> Vec<(String, u8, u16)> {
    w.entity(id)
        .unwrap()
        .weapons
        .iter()
        .map(|m| (m.weapon.clone(), m.mounts, m.ammo))
        .collect()
}
fn state(w: &World, id: EntityId) -> Value {
    let e = w.entity(id).unwrap();
    let ext: std::collections::BTreeMap<_, _> = e
        .ext
        .iter()
        .filter(|(k, _)| k.starts_with("damage."))
        .collect();
    json!({"hp":e.hp,"systems":e.systems,"damage":ext})
}
fn outcome(w: &World) -> Value {
    w.events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "9.6.9.source_strafe_attack" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .expect("actual integral-gun strafing consumer must execute, not just the Ph helper")
}
fn assert_actual(w: &World, actor: EntityId, target: EntityId, before: &Value, ph: f64) {
    let d = outcome(w);
    assert_eq!(d["probability"].as_f64(), Some(ph));
    assert_eq!(d["salvo"], 1);
    assert_eq!(d["gun_rating"].as_f64(), Some(4.2));
    assert_eq!(d["caliber_mm"].as_f64(), Some(20.0));
    assert_eq!(d["critical_d6"], 6);
    assert_eq!(d["critical_count"], 2);
    let ground = w
        .rules
        .ground
        .as_ref()
        .unwrap()
        .facilities
        .contains_key(&target);
    assert_eq!(
        d["critical_column"],
        if ground {
            "ground_function"
        } else {
            "airburst"
        }
    );
    assert_eq!(d["direct_damage_points"].as_f64(), Some(0.));
    assert_eq!(d["ammunition_model"], "book_abstract_gun_rating");
    assert!(
        d["physical_round_count"].is_null(),
        "unknown rounds per burst must not be invented"
    );
    assert_eq!(
        w.events
            .iter()
            .filter(|e| matches!(&e.kind,EventKind::GunAction{shooter,..}if *shooter==actor))
            .count(),
        1,
        "one actual firing pass, not4.2-round or20mm-stock fabrication"
    );
    if ground {
        let facility = &w.rules.ground.as_ref().unwrap().facilities[&target];
        assert_eq!(
            facility.damage, 0.,
            "source strafing does not invent direct ground DP"
        );
        assert_eq!(facility.critical_hits, 2);
        assert!(facility.disabled);
        assert!(!facility.destroyed);
        assert_eq!(
            w.entity(target).unwrap().hp,
            8.,
            "zero directDP but actual function stops via11.1 critical"
        );
    } else {
        let common_hit = w
            .events
            .iter()
            .any(|e| matches!(&e.kind,EventKind::CriticalHit{unit,..}if *unit==target));
        if !common_hit {
            let traces: Vec<_> = w.events.iter().filter_map(|e| match &e.kind {
                EventKind::RulesEvent { domain, rule, unit: Some(unit), detail, .. }
                    if *unit == target && (domain == "damage" || domain.contains("expanded")) =>
                    Some(json!({"domain":domain,"rule":rule,"detail":serde_json::from_str::<Value>(detail).unwrap_or_else(|_|json!(detail))})),
                _ => None,
            }).collect();
            let expanded = w
                .rules
                .expanded_damage
                .as_ref()
                .and_then(|s| s.units.get(&target))
                .map(|s| json!({"records":s.records,"pending":s.pending,"disabled":s.disabled}));
            let typed_counts: Vec<_> = w
                .rules
                .cluster_critical_counts
                .iter()
                .filter(|(_, c)| c.target == target)
                .map(|(id, c)| json!({"identity":id,"count":c}))
                .collect();
            let pending = w
                .rules
                .pending_adjudications
                .get(&target)
                .map(|p| json!({"at":p.tick,"rule":p.rule,"target":p.target}));
            eprintln!(
                "strafe-common-diagnostic {}",
                json!({"target":target,"outcome":d,"typed_counts":typed_counts,"expanded":expanded,"pending":pending,"traces":traces,"before":before,"after":state(w,target)})
            );
        }
        assert!(
            common_hit,
            "source critical must reach common physical effect"
        );
        assert_ne!(
            state(w, target),
            *before,
            "actual target system/personnel/equipment state changes"
        );
    }
}

#[test]
fn s01_real_twenty_mm_source_gun_strafes_low_vlow_and_ground_with_one_pass_and_no_fake_ammunition()
{
    for (ground, altitude, ph) in [
        (false, 1000., 0.75),
        (false, 50., 0.75),
        (true, 1000., 0.40),
    ] {
        let mut w = world(ground, altitude, None, None);
        let actor = named(&w, "STRIKE");
        let target = named(&w, "TARGET");
        let before = state(&w, target);
        let stores = stocks(&w, actor);
        assert!(
            !stores.iter().any(|s| s.0 == GUN),
            "source gun profile is not a fictional finite mounted20mm store"
        );
        let payload = av::own_view(&w, actor).unwrap()["flight"]["payload_fraction"].clone();
        let cmd = attack(&w);
        let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        let commands = [cmd.clone(), cmd];
        harpoon_core::sim::tick(&mut w, &commands);
        harpoon_core::sim::tick(&mut restored, &commands);
        assert_actual(&w, actor, target, &before, ph);
        assert_eq!(stocks(&w, actor), stores);
        assert_eq!(
            av::own_view(&w, actor).unwrap()["flight"]["payload_fraction"],
            payload
        );
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            serde_json::to_value(&restored).unwrap()
        );
    }
}

#[test]
fn s02_actual_ordinary_and_heavy_defensive_guns_precede_surviving_strafe_in_both_client_orders() {
    for (class, gun, heavy, ph) in [
        ("us_burke_2a", "us_mk45_mod4", false, 0.70),
        ("us_ford", "ciws_30mm", true, 0.65),
    ] {
        for reverse in [false, true] {
            let mut w = world(false, 1000., Some(class), Some(gun));
            let actor = named(&w, "STRIKE");
            let target = named(&w, "TARGET");
            let escort = named(&w, "ESCORT");
            let contact = w
                .find_contact(Side::Red, actor)
                .expect("real defensive lookout contact");
            let ammo = w
                .entity(escort)
                .unwrap()
                .weapons
                .iter()
                .find(|m| m.weapon == gun)
                .unwrap()
                .ammo;
            let before = state(&w, target);
            let stores = stocks(&w, actor);
            let payload = av::own_view(&w, actor).unwrap()["flight"]["payload_fraction"].clone();
            let strike = attack(&w);
            let defense = (
                Side::Red,
                serde_json::from_value(
                    json!({"cmd":"fire","unit":escort,"contact":contact,"weapon":gun,"rounds":1}),
                )
                .unwrap(),
            );
            let commands = if reverse {
                vec![defense, strike]
            } else {
                vec![strike, defense]
            };
            let mut restored: World =
                serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
            harpoon_core::sim::tick(&mut w, &commands);
            harpoon_core::sim::tick(&mut restored, &commands);
            assert!(
                w.entity(actor).unwrap().alive,
                "ordinary master seed selected a real source gun miss"
            );
            assert_actual(&w, actor, target, &before, ph);
            let d = outcome(&w);
            assert_eq!(d["defense"]["no_aa"], false);
            assert_eq!(d["defense"]["heavy_aa"], heavy);
            assert!(w.events.iter().any(
                |e| matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="8.1.3.source_gun")
            ));
            assert_eq!(
                w.entity(escort)
                    .unwrap()
                    .weapons
                    .iter()
                    .find(|m| m.weapon == gun)
                    .unwrap()
                    .ammo,
                ammo - 1
            );
            assert_eq!(stocks(&w, actor), stores);
            assert_eq!(
                av::own_view(&w, actor).unwrap()["flight"]["payload_fraction"],
                payload
            );
            assert_eq!(
                serde_json::to_value(&w).unwrap(),
                serde_json::to_value(&restored).unwrap()
            );
        }
    }
}

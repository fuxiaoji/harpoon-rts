//! Source-red-first ordinary cluster candidates; root owns combined execution.
//! Scenario/Command JSON only: no future Rust Action, State or config types.
use harpoon_core::rules::aviation2026 as av;
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
const FIT: &str = "lab_f18e_cluster200x2";
const BOMB: &str = "lab_cluster200kg";

fn seed(ground: bool) -> u64 {
    (0..100_000)
        .find(|seed| {
            (2..=if ground { 4 } else { 3 }).all(|target| {
                let mut rng = Rng::new(*seed).derive(Rng::hash_tag(
                    2,
                    1,
                    &format!("9.6.6:cluster:{target}:0"),
                ));
                let hit = 1 + (rng.next_f64() * 100.) as u8;
                let count = 1 + (rng.next_f64() * 6.) as u8;
                hit <= 20 && (ground || count == 4)
            })
        })
        .expect("ordinary preconstruction source RNG selection, no World dice changes")
}
fn input(ground: bool, altitude: f64) -> Value {
    let members = if ground {
        vec!["VEHICLE1", "VEHICLE2", "SQUAD"]
    } else {
        vec!["MOORED1", "MOORED2"]
    };
    let mut value = json!({"id":"cluster-source-candidate","name":"scenario laboratory cluster coverage; no real Annex fit/weight claim","seed":seed(ground),"duration_s":1000,
        "weather":{"visibility_nm":40,"jam":[0,0]},"context":{
            "rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":0,"precipitation":"None"},
            "cluster_2026":{"source":"scenario_assumption: named laboratory200kg cluster bomb, author ground4DP per hit and naval0DP plus source criticals; English2026 9.6.6/11.1 general mechanism; not an Annex weapon fit",
                "enabled_laboratory_fits":[FIT],"coverages":{"area":{"source":"scenario_assumption: complete static adjacency group in this one abstract coordinate; no inferred radius or footprint","aimpoint":"range","primary":members[0],"members":members,"geometry":"scenario_declared_static_adjacency","allowed_sides":["Blue"]}}},
            "aviation_2026":{"source":"scenario_assumption: source F18E airframe only; author2*200kg cluster bombs+80kg empty racks, manual experiment release, no Annex fit claim",
                "loads":{},"parking":{},"initial_fuel_nm":{"LAB":800},"no_aa_surface_attack_source":"scenario_assumption: controlled weapons-hold range; real defensive fire overrides",
                "surface_aimpoints":{"range":{"source":"scenario_assumption: published fixed laboratory range allocated Blue","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
            "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: clear daylight actual fitted lookouts","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}
        },"units":[{"side":"Blue","class":FIT,"name":"LAB","pos":[0,-460./3600.],"course_deg":0,"speed_kts":460,"altitude_m":altitude,"fire_on":false}]});
    value["context"]["aviation_2026"]["loads"][FIT] = json!({"payload_kg":480,"tanks":[],"external_weapon_rounds":{"lab_cluster200kg":2},"source":"scenario_assumption: author200kg weapon,80kg empty racks, Manual experiment sight; not Mk20/CBU/Belouga mass"});
    for name in members.iter().copied().chain(std::iter::once("OUTSIDE")) {
        value["units"].as_array_mut().unwrap().push(json!({"side":"Red","class":if ground{"airfield"}else{"cn_001"},"name":name,"pos":if name=="OUTSIDE"{json!([3,3])}else{json!([0,0])},"speed_kts":0,"fire_on":false}));
        if ground {
            let dp = if name.starts_with("VEHICLE") { 2. } else { 8. };
            value["context"]["ground_2026"]["source"]=json!("English2026 11.1 explicitly permits scenario target characteristics; laboratory vehicles/personnel elements, not real casualty statistics");
            value["context"]["ground_2026"]["facilities"][name] = json!({"template":null,"custom":{"size":"F","damage_points":dp,"vertical_armor":0,"horizontal_armor":0,"effect":"normal","source":"scenario_assumption: one unarmored vehicle at2DP or one personnel element at8DP; no body-count conversion"},"source":"scenario_assumption: named vehicle/personnel laboratory point object under11.1"});
        }
    }
    for class in [FIT, "cn_001", "airfield"] {
        value["context"]["detection_2026"]["bindings"][format!("{class}:visual")] =
            json!({"type":"visual","source":"scenario_assumption: fitted native lookout"});
        value["context"]["detection_2026"]["targets"][class] = json!({"source":"scenario_assumption: explicit controlled native Visual signature; land F matches the configured F point objects and satisfies5.8.6 C-H land classes","size":if class==FIT||class=="airfield"{5}else{0},"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    }
    value["context"]["detection_2026"]["camouflage"]["airfield"] = json!(0);
    value
}
fn world(ground: bool, altitude: f64) -> World {
    let value = input(ground, altitude);
    let parsed: Scenario = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&parsed).unwrap()["context"]["cluster_2026"],
        value["context"]["cluster_2026"],
        "new source scope/coverage must not silently disappear during Scenario parsing"
    );
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    harpoon_core::rules::aaw2026::configure_database(&mut db);
    db.platforms.get_mut("us_f18e").unwrap().sensors = vec!["visual".into()];
    // Before construction, only the explicitly controlled target's weapons
    // are removed; source hull/armor/DP are intact, not invented in World.
    db.platforms.get_mut("cn_001").unwrap().weapons.clear();
    let mut w = harpoon_core::load_scenario(parsed, db)
        .expect("named lab fit must be registered before Scenario validation");
    harpoon_core::sim::tick(&mut w, &[]);
    assert!(
        w.entity(named(&w, "LAB"))
            .unwrap()
            .pos
            .range_to(harpoon_core::Vec2 { x: 0., y: 0. })
            < 1e-9,
        "source460kt first native movement actually reaches release point"
    );
    w
}
fn named(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn stock(w: &World) -> u16 {
    w.entity(named(w, "LAB"))
        .unwrap()
        .weapons
        .iter()
        .filter(|m| m.weapon == BOMB)
        .map(|m| m.ammo * m.mounts as u16)
        .sum()
}
fn attack(w: &World, pattern: &str, delivery: &str) -> (Side, Command) {
    let primary = if w.entities.values().any(|e| e.name == "VEHICLE1") {
        "VEHICLE1"
    } else {
        "MOORED1"
    };
    let contact = w
        .find_contact(Side::Blue, named(w, primary))
        .expect("actual native target contact");
    (Side::Blue,serde_json::from_value(json!({"cmd":"air_combat","unit":named(w,"LAB"),"action":{"type":"cluster_attack","contact":contact,"weapon":BOMB,"rounds":1,"delivery":delivery,"pattern":pattern,"coverage":"area"}})).expect("unknown source action is a normal source-red, no future Rust enum"))
}
fn outcome(w: &World) -> Value {
    w.events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "9.6.6.source_cluster_attack" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .expect("actual release and target coverage consumer must execute")
}
fn damage_state(w: &World, id: EntityId) -> Value {
    let entity = w.entity(id).unwrap();
    let damage: std::collections::BTreeMap<_, _> = entity
        .ext
        .iter()
        .filter(|(key, _)| key.starts_with("damage."))
        .collect();
    json!({"hp":entity.hp,"systems":entity.systems,"damage":damage})
}
fn critical_diagnostic(w: &World, id: EntityId, before: &Value) -> Value {
    let typed: Vec<_> = w
        .rules
        .cluster_critical_counts
        .iter()
        .filter(|(_, count)| count.target == id)
        .collect();
    let traces:Vec<_>=w.events.iter().rev().filter_map(|event|match &event.kind{
        EventKind::RulesEvent{domain,rule,unit,detail,..}if *unit==Some(id)&&(domain=="damage"||domain=="expanded_damage2026")=>Some(json!({"domain":domain,"rule":rule,"detail":serde_json::from_str::<Value>(detail).unwrap_or_else(|_|json!(detail))})),
        _=>None,
    }).take(12).collect();
    json!({"target":id,"before":before,"now":damage_state(w,id),"typed_common":typed,"last_common_traces":traces})
}
fn release(w: &mut World, pattern: &str, delivery: &str) -> Value {
    let actor = named(w, "LAB");
    let old = av::own_view(w, actor).unwrap()["flight"]["payload_fraction"]
        .as_f64()
        .unwrap();
    let max = w.rules.aviation.as_ref().unwrap().flights[&actor]
        .spec
        .max_payload_kg;
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap();
    let cmd = attack(w, pattern, delivery);
    harpoon_core::sim::tick(w, &[cmd.clone()]);
    harpoon_core::sim::tick(&mut restored, &[cmd]);
    assert_eq!(
        stock(w),
        1,
        "one actual cluster bomb, no helper-only fake release"
    );
    let after = av::own_view(w, actor).unwrap()["flight"]["payload_fraction"]
        .as_f64()
        .unwrap();
    assert!(
        (old - after - 200. / max).abs() < 1e-10,
        "single200kg physical mass debit, empty80kg rack remains"
    );
    assert!(w.events.iter().any(
        |e| matches!(&e.kind,EventKind::WeaponsRelease{shooter,count:1,..}if *shooter==actor)
    ));
    assert_eq!(
        serde_json::to_value(&*w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    outcome(w)
}

#[test]
fn c01_one_cluster_release_attacks_two_explicitly_adjacent_source_ships_with_real_airburst_criticals(
) {
    for (pattern, ph, criticals) in [("small", 0.60, 6), ("large", 0.70, 3)] {
        let mut w = world(false, 500.);
        let original: Vec<_> = ["MOORED1", "MOORED2", "OUTSIDE"]
            .into_iter()
            .map(|name| (name, damage_state(&w, named(&w, name))))
            .collect();
        let d = release(&mut w, pattern, "Level");
        assert_eq!(d["equipment_source"], "scenario_laboratory");
        let targets = d["targets"].as_array().unwrap();
        assert_eq!(
            targets.len(),
            2,
            "one real pattern covers both static adjacent potential targets"
        );
        for name in ["MOORED1", "MOORED2"] {
            let id = named(&w, name);
            let result = targets.iter().find(|t| t["target"] == json!(id)).unwrap();
            assert_eq!(result["probability"].as_f64(), Some(ph));
            assert_eq!(result["physical_size"], "A");
            assert_eq!(result["critical_count"], criticals);
            assert_eq!(result["critical_column"], "airburst");
            assert_eq!(result["direct_damage_points"].as_f64(), Some(0.));
            assert!(
                w.events
                    .iter()
                    .any(|e| matches!(&e.kind,EventKind::CriticalHit{unit,..}if *unit==id)),
                "cluster criticals must reach existing actual effects; diagnostic={}",
                critical_diagnostic(
                    &w,
                    id,
                    &original.iter().find(|(n, _)| *n == name).unwrap().1
                )
            );
            assert_ne!(
                damage_state(&w, id),
                original.iter().find(|(n, _)| *n == name).unwrap().1,
                "real system/personnel/equipment state must change, not only a probability trace"
            );
        }
        let outside = named(&w, "OUTSIDE");
        assert!(!targets.iter().any(|t| t["target"] == json!(outside)));
        assert_eq!(
            damage_state(&w, outside),
            original.iter().find(|(n, _)| *n == "OUTSIDE").unwrap().1,
            "no invented radius sweeps in unrelated target"
        );
    }
}

#[test]
fn c02_source_land_point_objects_disable_personnel_and_destroy_vehicles_while_vlow_deployment_is_a_spent_dud(
) {
    for altitude in [500., 50.] {
        let mut w = world(true, altitude);
        let d = release(
            &mut w,
            "small",
            if altitude == 50. { "Level" } else { "Glide" },
        );
        if altitude == 50. {
            assert_eq!(d["resolution"], "automatic_dud");
            assert!(d["targets"].as_array().unwrap().is_empty());
            for name in ["VEHICLE1", "VEHICLE2", "SQUAD", "OUTSIDE"] {
                let f = &w.rules.ground.as_ref().unwrap().facilities[&named(&w, name)];
                assert_eq!(f.damage, 0.);
                assert!(!f.disabled);
            }
        } else {
            assert_eq!(
                d["targets"].as_array().unwrap().len(),
                3,
                "static adjacent group contains two vehicles and one personnel element"
            );
            for name in ["VEHICLE1", "VEHICLE2", "SQUAD"] {
                let f = &w.rules.ground.as_ref().unwrap().facilities[&named(&w, name)];
                assert_eq!(
                    f.damage, 4.,
                    "explicit author lab ground4DP reaches actual11.1 state"
                );
                assert!(f.disabled);
                assert_eq!(f.destroyed,name.starts_with("VEHICLE"),"2DP vehicles destroyed;8DP personnel element loses function at50%, without invented casualty counts");
            }
            let outside = &w.rules.ground.as_ref().unwrap().facilities[&named(&w, "OUTSIDE")];
            assert_eq!(outside.damage, 0.);
            assert!(!outside.disabled);
        }
    }
}

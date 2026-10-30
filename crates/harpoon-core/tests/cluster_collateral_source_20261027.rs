//! Artifact-only: two known planned coverage/ship12 source candidates.
//! Not registered, compiled or executed; no future Rust enum/state is used.
use harpoon_core::rules::aviation2026 as av;
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
use std::collections::BTreeSet;

const FIT: &str = "lab_f18e_cluster200x2";
const BOMB: &str = "lab_cluster200kg";

fn named(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn world(n: usize) -> World {
    let mut input = json!({"id":"cluster-planned-secondary-ship12-source","name":"Known assigned two-object coverage at laboratory anchor","seed":20261027,"duration_s":1000,
        "weather":{"visibility_nm":40,"jam":[0,0]},"context":{
            "rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":0,"precipitation":"None"},
            "cluster_2026":{"source":"scenario_assumption: complete planned two-object adjacency known before attacking; source9.6.3 cap and9.6.6 adjacent target also attacked; not unknown incidental collateral",
                "enabled_laboratory_fits":[FIT],"coverages":{"planned":{"source":"scenario_assumption: the only two Red objects form the complete assigned static adjacency group; both must be native contacts before commands","aimpoint":"range","primary":"POINT","members":["POINT","MOORED"],"geometry":"scenario_declared_static_adjacency","allowed_sides":["Blue"]}}},
            "ground_2026":{"source":"English2026 11.1 author point object; experimental characteristics are not a real facility card","facilities":{"POINT":{"template":null,"custom":{"size":"F","damage_points":8,"vertical_armor":0,"horizontal_armor":0,"effect":"normal","source":"scenario_assumption: exposed F/8DP functional equipment point"},"source":"scenario_assumption: explicitly assigned primary at controlled anchor"}}},
            "aviation_2026":{"source":"scenario_assumption: source F18E airframe with registered author2*200kg cluster bombs and80kg racks; no real Annex fit claim","loads":{},"parking":{},"initial_fuel_nm":{},
                "no_aa_surface_attack_source":"scenario_assumption: controlled range, target weapons held; actual source defensive fire would override",
                "surface_aimpoints":{"range":{"source":"scenario_assumption: published fixed laboratory anchor, allocated Blue","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
            "detection_2026":{"bindings":{},"targets":{},"camouflage":{"airfield":0},"environment":{"source":"scenario_assumption: clear daylight fitted native lookouts","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}
        },"units":[]});
    input["context"]["aviation_2026"]["loads"][FIT] = json!({"payload_kg":480,"tanks":[],"external_weapon_rounds":{BOMB:2},"source":"scenario_assumption: author2*200kg bombs and80kg empty racks, not real Annex ordnance"});
    for i in 0..n {
        let name = format!("LAB{i:02}");
        input["context"]["aviation_2026"]["initial_fuel_nm"][&name] = json!(800);
        input["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":FIT,"name":name,"pos":[0,-460./3600.],"course_deg":0,"speed_kts":460,"altitude_m":1000,"fire_on":false}));
    }
    for (name, class) in [("POINT", "airfield"), ("MOORED", "cn_001")] {
        input["units"].as_array_mut().unwrap().push(json!({"side":"Red","class":class,"name":name,"pos":[0,0],"speed_kts":0,"fire_on":false}));
    }
    for class in [FIT, "airfield", "cn_001"] {
        input["context"]["detection_2026"]["bindings"][format!("{class}:visual")] =
            json!({"type":"visual","source":"scenario_assumption: real fitted native lookout"});
        input["context"]["detection_2026"]["targets"][class] = json!({"source":"scenario_assumption: clear-range source Visual profile, configured F land class","size":if class=="cn_001"{0}else{5},"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    }
    let scenario: Scenario = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&scenario).unwrap()["context"]["cluster_2026"],
        input["context"]["cluster_2026"]
    );
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    harpoon_core::rules::aaw2026::configure_database(&mut db);
    db.platforms.get_mut("us_f18e").unwrap().sensors = vec!["visual".into()];
    db.platforms.get_mut("cn_001").unwrap().weapons.clear();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    for name in ["POINT", "MOORED"] {
        assert!(w.find_contact(Side::Blue,named(&w,name)).is_some(),"both deliberately planned objects need genuine native contacts before declaring, no private CID injection");
    }
    for i in 0..n {
        let id = named(&w, &format!("LAB{i:02}"));
        assert!(
            w.entity(id)
                .unwrap()
                .pos
                .range_to(harpoon_core::Vec2 { x: 0., y: 0. })
                < 1e-9
        );
        assert_eq!(stock(&w, id), 2);
    }
    w
}
fn stock(w: &World, id: EntityId) -> u16 {
    w.entity(id)
        .unwrap()
        .weapons
        .iter()
        .filter(|m| m.weapon == BOMB)
        .map(|m| m.ammo * m.mounts as u16)
        .sum()
}
fn physical(w: &World, n: usize) -> Value {
    json!((0..n).map(|i|{let id=named(w,&format!("LAB{i:02}"));json!({"actor":id,"stock":stock(w,id),"payload":av::own_view(w,id).unwrap()["flight"]["payload_fraction"]})}).collect::<Vec<_>>())
}
fn released(w: &World) -> BTreeSet<EntityId> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::WeaponsRelease {
                shooter, count: 1, ..
            } => Some(*shooter),
            _ => None,
        })
        .collect()
}
fn run(n: usize, reverse: bool) -> (BTreeSet<EntityId>, Value, BTreeSet<EntityId>) {
    let mut w = world(n);
    let before = physical(&w, n);
    let target = named(&w, "MOORED");
    let primary = w.find_contact(Side::Blue, named(&w, "POINT")).unwrap();
    let mut commands:Vec<(Side,Command)>=(0..n).map(|i|(Side::Blue,serde_json::from_value(json!({"cmd":"air_combat","unit":named(&w,&format!("LAB{i:02}")),"action":{"type":"cluster_attack","contact":primary,"weapon":BOMB,"rounds":1,"delivery":"Glide","pattern":"small","coverage":"planned"}})).unwrap())).collect();
    if reverse {
        commands.reverse();
    }
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    harpoon_core::sim::tick(&mut w, &commands);
    harpoon_core::sim::tick(&mut restored, &commands);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap(),
        "native full-World restore continues exactly"
    );
    let actual = released(&w);
    let after = physical(&w, n);
    let participants = w
        .rules
        .aircombat
        .as_ref()
        .unwrap()
        .surface_attacks
        .get(&format!("{}:0", target.0))
        .cloned()
        .unwrap_or_default();
    if n == 12 {
        assert_eq!(
            actual.len(),
            12,
            "legitimate twelve-plane control really releases"
        );
        let reports: Vec<Value> = w
            .events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::RulesEvent { rule, detail, .. }
                    if rule == "9.6.6.source_cluster_attack" =>
                {
                    Some(serde_json::from_str(detail).unwrap())
                }
                _ => None,
            })
            .collect();
        assert_eq!(reports.len(), 12);
        for d in reports {
            assert_eq!(d["targets"].as_array().unwrap().len(),2,"both explicitly assigned objects are actually attacked, independent of hit results");
        }
        for i in 0..n {
            let id = named(&w, &format!("LAB{i:02}"));
            assert_eq!(stock(&w, id), 1);
            let max = w.rules.aviation.as_ref().unwrap().flights[&id]
                .spec
                .max_payload_kg;
            assert!(
                (before[i]["payload"].as_f64().unwrap()
                    - after[i]["payload"].as_f64().unwrap()
                    - 200. / max)
                    .abs()
                    < 1e-10,
                "one real200kg debit, no per-secondary duplicate expenditure"
            );
        }
        assert_eq!(participants,actual,"the known secondary ship reserves the same twelve distinct attackers in the common ledger");
    } else {
        assert!(actual.is_empty(),"thirteen planned attacks on the shared known ship must fail the existing all-conflicting-batch budget policy before release, without a client-order prefix");
        assert_eq!(before,after,"over-capacity planned coverage cannot spend bombs or mass and then hide secondary effects");
        assert!(participants.is_empty());
    }
    (actual, after, participants)
}
#[test]
fn cb01_twelve_planned_cluster_coverages_include_known_secondary_ship_in_common_budget() {
    assert_eq!(run(12, false), run(12, true));
}
#[test]
fn cb02_thirteen_planned_cluster_coverages_block_before_expenditure_in_both_client_orders() {
    assert_eq!(run(13, false), run(13, true));
}

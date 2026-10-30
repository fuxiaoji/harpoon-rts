//! Artifact-only author F combatant/G cargo source cases, not real Annex fits.
//! Not registered, compiled or executed. No live effect/HP/die injection.
use harpoon_core::rules::{aircombat2026::budget, aviation2026 as av};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
const AIRCRAFT: &str = "us_f18e_mk82";
const GUN: &str = "us_m61a2_20mm_source_gun";

fn claim() -> String {
    budget::claim_identity(
        budget::AirAttackKind::UnguidedBomb,
        EntityId(1),
        2,
        &json!({"request":{"contact":1,"delivery":"Strafe","aimpoint":"range"},"gun":GUN}),
    )
}
fn short_tag(s: &str) -> String {
    if s.len() <= 96 {
        return s.into();
    }
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in s.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    let mut cut = 64;
    while !s.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}#{hash:016x}", &s[..cut])
}
fn critical_identity() -> String {
    format!("{}:cluster:0", short_tag(&claim()))
}
fn common_die(seed: u64) -> u8 {
    let mut r = Rng::new(seed).derive(Rng::hash_tag(2, 2, &critical_identity()));
    1 + (r.next_f64() * 100.) as u8
}
fn seed() -> u64 {
    (0..1_000_000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(2, 1, "9.6.9:strafe"));
            let hit = 1 + (r.next_f64() * 100.) as u8;
            let d6 = 1 + (r.next_f64() * 6.) as u8;
            hit <= 20 && d6 == 4 && (21..=25).contains(&common_die(*seed))
        })
        .expect("ordinary preconstruction master seed, no live dice edits")
}
fn named(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn stocks(w: &World, id: EntityId) -> Value {
    json!(w
        .entity(id)
        .unwrap()
        .weapons
        .iter()
        .map(|m| json!({"weapon":m.weapon,"ammo":m.ammo,"mounts":m.mounts}))
        .collect::<Vec<_>>())
}
fn world(cargo: bool) -> World {
    let class = if cargo {
        "lab_g_small_cargo_card"
    } else {
        "lab_f_small_combatant_card"
    };
    let role = if cargo {
        "small_cargo"
    } else {
        "small_combatant"
    };
    let size = if cargo { "G" } else { "F" };
    let mut input = json!({"id":"strafe-author-smallcraft-source","name":"explicit author smallcraft card, no real Annex fit claim","seed":seed(),"duration_s":1000,
    "weather":{"visibility_nm":40,"jam":[0,0]},"context":{
        "rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":0,"precipitation":"None"},
        "strafe_2026":{"source":"English2026 9.6.9 actual gun pass against an explicitly authored F/G target game card","enabled_aircraft":[AIRCRAFT],"ammunition_model":"book_abstract_gun_rating"},
        "aviation_2026":{"source":"scenario_assumption: existing source Mk82 aircraft load at controlled range; target boat card is wholly authored","loads":{},"parking":{},"initial_fuel_nm":{"STRIKE":800},
            "no_aa_surface_attack_source":"scenario_assumption: controlled weapons-held range; actual defensive fire overrides",
            "surface_aimpoints":{"range":{"source":"scenario_assumption: published fixed range allocated Blue","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
        "expanded_damage_2026":{"source":"scenario_assumption: complete authored receiver game card; not an Annex variant","units":{"TARGET":{"source":"scenario_assumption: authored physical fit and crew","crew":if cargo{8}else{12},"passengers":0,"components":{},"holds":{}}}},
        "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: clear daylight native lookouts","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}
    },"units":[
        {"side":"Blue","class":AIRCRAFT,"name":"STRIKE","pos":[0,-460./3600.],"course_deg":0,"speed_kts":460,"altitude_m":1000,"fire_on":false},
        {"side":"Red","class":class,"name":"TARGET","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false}
    ]});
    input["context"]["aviation_2026"]["loads"][AIRCRAFT] = json!({"payload_kg":6395,"tanks":[{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"}],"external_weapon_rounds":{"us_mk82":6},"source":"existing verified US2025 B44/H1-3/H4-2 load; no invented20mm ammunition or mass"});
    for target_class in [AIRCRAFT, class] {
        input["context"]["detection_2026"]["bindings"][format!("{target_class}:visual")] =
            json!({"type":"visual","source":"scenario_assumption: actual fitted lookout"});
        input["context"]["detection_2026"]["targets"][target_class] = json!({"source":"scenario_assumption: visual profile matches declared F/G card","size":if target_class==AIRCRAFT||!cargo{5}else{6},"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    }
    input["context"]["expanded_damage_2026"]["units"]["TARGET"]["components"]["lookout"] = json!({"source":"scenario_assumption: one actual visual sensor","kind":"sensor","binding":"visual","physical":0});
    if cargo {
        input["context"]["expanded_damage_2026"]["units"]["TARGET"]["holds"]["general"] = json!({"source":"scenario_assumption:100 tons general cargo, not real vessel capacity","kind":"general","amount":100});
    } else {
        input["context"]["expanded_damage_2026"]["units"]["TARGET"]["components"]["mg"] = json!({"source":"scenario_assumption: one experimental existing12.7mm mount,10 authored rounds, no real boat fit","kind":"weapon_mount","binding":"cw_50cal","physical":0,"mount_index":0,"ammunition":10,"location":"topside"});
    }
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    harpoon_core::rules::aaw2026::configure_database(&mut db);
    db.platforms.get_mut("us_f18e").unwrap().sensors = vec!["visual".into()];
    // Author card is registered before Scenario construction; no live Entity,
    // HP or critical-result field is changed in the resulting World.
    db.platforms.insert(class.into(),serde_json::from_value(json!({
        "id":class,"name":if cargo{"LAB authored G cargo card"}else{"LAB authored F combatant card"},"kind":"Ship","hp":20,"max_speed_kts":20,"cruise_speed_kts":10,"turn_rate_deg_s":10,"accel_kts_per_s":1,"rcs":"VerySmall","noise_db":60,"visual_factor":1,"vp_value":1,"sensors":["visual"],"weapons":if cargo{json!([])}else{json!([{"weapon":"cw_50cal","mounts":1,"ammo":10}])},"chaff":0,"decoys":0,"endurance_s":0,"max_depth_m":0,"dipping_sonar":false,"is_base":false,"deck":null,
        "rule_data":{"source":"scenario_assumption: complete authored smallcraft game card; all body/DP/role/armor/speed values are experimental, no real Annex fit claim","size":size,"role":role,"year":2000,"belt_cm":0,"deck_cm":0,"tps":0,"dp_breaks":[0,5,10,15,18,20],"speed_steps":[20,15,10,5,0,0],"nuclear":false,"double_hull":false,"aip":false,"single_engine":false,"mount_locations":if cargo{json!({})}else{json!({"cw_50cal":"topside"})},"critical_armor":{}}
    })).unwrap());
    let scenario: Scenario = serde_json::from_value(input).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(named(&w, "STRIKE"), EntityId(1));
    assert_eq!(named(&w, "TARGET"), EntityId(2));
    assert_eq!(
        serde_json::to_value(
            w.find_contact(Side::Blue, EntityId(2))
                .expect("actual native smallcraft contact")
        )
        .unwrap(),
        json!(1)
    );
    assert!(
        w.entity(EntityId(1))
            .unwrap()
            .pos
            .range_to(harpoon_core::Vec2 { x: 0., y: 0. })
            < 1e-9
    );
    w
}
fn run(cargo: bool) {
    let mut w = world(cargo);
    let actor = named(&w, "STRIKE");
    let target = named(&w, "TARGET");
    let before = stocks(&w, actor);
    let target_stores = stocks(&w, target);
    let payload = av::own_view(&w, actor).unwrap()["flight"]["payload_fraction"].clone();
    let contact = w.find_contact(Side::Blue, target).unwrap();
    let cmd:Command=serde_json::from_value(json!({"cmd":"air_combat","unit":actor,"action":{"type":"surface_attack","contact":contact,"weapon":GUN,"rounds":1,"delivery":"Strafe","aimpoint":"range"}})).unwrap();
    let commands = [(Side::Blue, cmd)];
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    harpoon_core::sim::tick(&mut w, &commands);
    harpoon_core::sim::tick(&mut restored, &commands);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    let result: Value = w
        .events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "9.6.9.source_strafe_attack" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .expect("actual source smallcraft gun consumer");
    assert_eq!(result["identity"], claim());
    assert_eq!(result["critical_d6"], 4);
    assert_eq!(result["critical_count"], 1);
    assert_eq!(result["salvo"], 1);
    assert_eq!(result["probability"].as_f64(), Some(0.30));
    assert_eq!(result["critical_column"], "small_craft");
    assert_eq!(result["direct_damage_points"].as_f64(), Some(0.));
    assert!(result["physical_round_count"].is_null());
    let traces: Vec<(String, Value)> = w
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                domain,
                rule,
                unit: Some(id),
                detail,
                ..
            } if domain == "damage" && *id == target => {
                Some((rule.clone(), serde_json::from_str(detail).unwrap()))
            }
            _ => None,
        })
        .collect();
    assert!(
        traces.iter().any(|(rule, d)| rule == "hit_resolved"
            && d["column"]
                == if cargo {
                    "small_cargo"
                } else {
                    "small_combatant"
                }
            && d["dp"].as_f64() == Some(0.)),
        "true role subcolumn and0 directDP in common resolution, not Airburst for both cards"
    );
    assert_eq!(
        traces
            .iter()
            .filter(|(rule, d)| rule == "damage_roll"
                && d["purpose"] == "critical_type"
                && d["hit"] == critical_identity())
            .count(),
        1
    );
    assert!(traces.iter().any(|(rule, d)| rule == "damage_roll"
        && d["purpose"] == "critical_type"
        && d["roll"].as_u64() == Some(u64::from(common_die(seed())))));
    assert!(
        w.events
            .iter()
            .any(|e| matches!(&e.kind,EventKind::CriticalHit{unit,..}if *unit==target)),
        "real fitted defender casualty receipt"
    );
    assert_eq!(
        w.events
            .iter()
            .filter(|e| matches!(&e.kind,EventKind::GunAction{shooter,..}if *shooter==actor))
            .count(),
        1
    );
    let typed = w
        .rules
        .cluster_critical_counts
        .values()
        .find(|c| c.target == target)
        .unwrap();
    assert_eq!(
        serde_json::to_value(typed).unwrap()["origin"],
        "aircraft_gun"
    );
    assert_eq!(typed.count, 1);
    if cargo {
        assert!(
            w.entity(target)
                .unwrap()
                .ext
                .get("damage.engine_levels")
                .copied()
                .unwrap_or(0.)
                >= 1.,
            "G cargo roll21-25 is Engineering, whereas Airburst would merely hit a sensor"
        );
    } else {
        assert!(
            harpoon_core::sensors::sensor_damaged(w.entity(target).unwrap(), "visual"),
            "F combatant roll21-25 destroys its actual fitted visual sensor"
        );
    }
    assert_eq!(stocks(&w, actor), before);
    assert_eq!(stocks(&w, target), target_stores);
    assert_eq!(
        av::own_view(&w, actor).unwrap()["flight"]["payload_fraction"],
        payload
    );
}
#[test]
fn sc01_author_f_combatant_uses_true_smallcombatant_column_and_fitted_sensor_effect() {
    run(false);
}
#[test]
fn sc02_author_g_cargo_uses_true_smallcargo_column_and_engineering_effect() {
    run(true);
}

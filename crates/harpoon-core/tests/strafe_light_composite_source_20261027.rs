//! Source9.6.9 ->14.1.3 composite critical-type consumer boundary.
//! Existing normal Strafe command/native Visual/common count; no forced roll.
//! Target is an explicitly authored unarmored non-aviation receiver, not a real Annex claim.
use harpoon_core::rules::aviation2026 as av;
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
const AIRCRAFT: &str = "us_f18e_mk82";
const GUN: &str = "us_m61a2_20mm_source_gun";
fn counted_identity() -> String {
    use harpoon_core::rules::aircombat2026::budget;
    let raw = budget::claim_identity(
        budget::AirAttackKind::UnguidedBomb,
        EntityId(1),
        2,
        &json!({"request":{"contact":1,"delivery":"Strafe","aimpoint":"range"},"gun":GUN}),
    );
    if raw.len() <= 96 {
        return raw;
    }
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in raw.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    let mut cut = 64;
    while !raw.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}#{h:016x}", &raw[..cut])
}
fn seed(_defense: Option<&str>) -> u64 {
    let common = format!("{}:cluster:0", counted_identity());
    (0..1_000_000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(2, 1, "9.6.9:strafe"));
            let d100 = 1 + (r.next_f64() * 100.) as u8;
            let d6 = 1 + (r.next_f64() * 6.) as u8;
            let mut c = Rng::new(*seed).derive(Rng::hash_tag(2, 2, &common));
            let critical = 1 + (c.next_f64() * 100.) as u8;
            let recovery = 1 + (c.next_f64() * 6.) as u8;
            d100 <= 20 && d6 == 6 && critical >= 96 && recovery >= 2
        })
        .expect("pure canonical seed predicate, no live dice/result edits")
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
    let mut value = input(ground, altitude, escort, gun);
    value["units"][1]["class"] = json!("lab_strafe_surface_receiver");
    value["context"]["detection_2026"]["bindings"]["lab_strafe_surface_receiver:visual"] =
        value["context"]["detection_2026"]["bindings"]["cn_001:visual"].clone();
    value["context"]["detection_2026"]["targets"]["lab_strafe_surface_receiver"] =
        value["context"]["detection_2026"]["targets"]["cn_001"].clone();
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
    // Authored receiver metadata exists before Scenario initialization. No live
    // body/type/armor/HP or casualty field is altered after loading.
    let mut receiver = db.platforms["cn_001"].clone();
    receiver.id = "lab_strafe_surface_receiver".into();
    receiver.name = "scenario_assumption: unarmored non-aviation bridge receiver".into();
    receiver.deck = None;
    receiver.weapons.clear();
    receiver.sensors = vec!["visual".into()];
    let mut physical = harpoon_core::rules::damage_runtime::platform("cn_001")
        .unwrap()
        .clone();
    physical.role = "surface".into();
    physical.belt_cm = 0.;
    physical.deck_cm = 0.;
    physical.critical_armor.clear();
    physical.source="scenario_assumption: source-boundary non-aviation unarmored A receiver, not a published vessel variant".into();
    receiver.rule_data = Some(physical);
    db.platforms.insert(receiver.id.clone(), receiver);

    // The normal registry recreates the strike alias from this base; select
    // the controlled native lookout before that real registration step.
    db.platforms.get_mut("us_f18e").unwrap().sensors = vec!["visual".into()];
    db.platforms.get_mut("cn_001").unwrap().weapons.clear();
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

#[test]
fn strafe_light01_real_counted_aircraft_gun_resolves_composite_bridge_before_light_weapon_filter() {
    let mut w = world(false, 1000., None, None);
    let actor = named(&w, "STRIKE");
    let target = named(&w, "TARGET");
    assert_eq!(actor, EntityId(1));
    assert_eq!(target, EntityId(2));
    assert_eq!(
        w.find_contact(Side::Blue, target).unwrap().0,
        1,
        "the canonical seed must match an actual native observed contact"
    );
    assert!(!harpoon_core::rules::damage_runtime::aviation(
        w.entity(target).unwrap()
    ));
    let hp = w.entity(target).unwrap().hp;
    let stock = stocks(&w, actor);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let cmd = attack(&w);
    harpoon_core::sim::tick(&mut w, &[cmd.clone()]);
    harpoon_core::sim::tick(&mut restored, &[cmd]);
    let d = outcome(&w);
    assert_eq!(d["critical_count"], 2);
    assert_eq!(d["critical_d6"], 6);
    assert_eq!(d["caliber_mm"].as_f64(), Some(20.));
    let identity = counted_identity();
    let c = &w.rules.cluster_critical_counts[&format!("{}:{identity}", target.0)];
    assert_eq!(
        c.origin,
        harpoon_core::rules::damage_runtime::CountedCriticalOrigin::AircraftGun
    );
    assert_eq!(c.count, 2);
    let critical = format!("{identity}:cluster:0");
    let die = w
        .events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule,
                unit: Some(unit),
                detail,
                ..
            } if rule == "damage_roll" && *unit == target => {
                let q: Value = serde_json::from_str(detail).unwrap();
                (q["hit"] == critical && q["purpose"] == "critical_type").then_some(q)
            }
            _ => None,
        })
        .expect("the real common first critical must retain its ordinary D100");
    assert!(
        (96..=100).contains(&die["roll"].as_u64().unwrap()),
        "source Airburst row is Bridge/Airplot; this non-aviation target must resolve Bridge"
    );
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::CriticalHit{unit,system,..}if *unit==target&&system=="bridge")),
        "English2026 PDF129/14-3 allows20mm Bridge: map the actual composite row to its final physical system before light-caliber filtering, no synthetic CriticalHit for pending");
    assert_eq!(
        w.entity(target)
            .unwrap()
            .ext
            .get("damage.bridge_order_delay"),
        Some(&360.)
    );
    assert_eq!(
        w.entity(target).unwrap().hp,
        hp,
        "source strafing adds0 ordinary DP"
    );
    assert_eq!(
        stocks(&w, actor),
        stock,
        "book abstract gun pass must not fabricate shell consumption or remove existing bombs"
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

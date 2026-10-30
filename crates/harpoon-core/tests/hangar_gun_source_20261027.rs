//! Artifact-only single-critical native Strafe -> source Hangar inventory case.
//! Not registered, compiled or executed. No injected effect, die or HP change.
use harpoon_core::rules::{aircombat2026::budget, aviation2026 as av, ground2026};
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
// Exact existing common identity protocol, checked against actual outcome.
// This is only preconstruction ordinary master-seed selection, not a World edit.
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
fn ground_identity() -> String {
    format!("{}:ground_gun_criticals", short_tag(&claim()))
}
fn hangar_die(seed: u64) -> u8 {
    let mut r = Rng::new(seed).derive(Rng::hash_tag(
        2,
        2,
        &format!("11.aircraft:{}", ground_identity()),
    ));
    1 + (r.next_f64() * 6.) as u8
}
fn seed() -> u64 {
    (0..1_000_000).find(|seed| {
        let mut r=Rng::new(*seed).derive(Rng::hash_tag(2,1,"9.6.9:strafe"));
        let hit=1+(r.next_f64()*100.) as u8;
        let die=1+(r.next_f64()*6.) as u8;
        hit<=20&&die==4&&hangar_die(*seed)%2==0
    }).expect("ordinary preconstruction seed: one20mm critical and integral Hangar loss, no multiplicity or rounding inference")
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
fn world(seed: u64) -> World {
    let mut input = json!({"id":"hangar-zero-dp-gun-critical-source","name":"one real source gun critical at an unarmored source Medium Hangar","seed":seed,"duration_s":1000,
    "weather":{"visibility_nm":40,"jam":[0,0]},"context":{
        "rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":0,"precipitation":"None"},
        "air_wings":{"HANGAR":[{"class":"us_f18e","count":3}]},
        "strafe_2026":{"source":"English2026 9.6.9 actual20mm source gun; one critical only; no physical shell inventory claim","enabled_aircraft":[AIRCRAFT],"ammunition_model":"book_abstract_gun_rating"},
        "ground_2026":{"source":"English2026 PDF116 section11.1.2 andPDF118 Generic Land Targets; source MediumHangar special Result without a fabricated positiveDP threshold",
            "facilities":{"HANGAR":{"template":"hangar_medium","custom":null,"source":"English2026 PDF118/11-3 Medium Hangar D/100DP/armor0,D6/2 aircraft inside lost","stored_airframes":["2:us_f18e:0","2:us_f18e:1","2:us_f18e:2"]}}},
        "aviation_2026":{"source":"scenario_assumption: existing source Mk82 load and declared parked individual source airframes, controlled range","loads":{},"parking":{"HANGAR":3},"initial_fuel_nm":{"STRIKE":800},
            "no_aa_surface_attack_source":"scenario_assumption: weapons-held controlled range; actual defensive fire overrides",
            "surface_aimpoints":{"range":{"source":"scenario_assumption: published fixed MediumHangar range allocated Blue","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
        "detection_2026":{"bindings":{},"targets":{},"camouflage":{"airfield":0},"environment":{"source":"scenario_assumption: clear daylight fitted native lookout","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}
    },"units":[
        {"side":"Blue","class":AIRCRAFT,"name":"STRIKE","pos":[0,-460./3600.],"course_deg":0,"speed_kts":460,"altitude_m":1000,"fire_on":false},
        {"side":"Red","class":"airfield","name":"HANGAR","pos":[0,0],"speed_kts":0,"fire_on":false}
    ]});
    input["context"]["aviation_2026"]["loads"][AIRCRAFT] = json!({"payload_kg":6395,"tanks":[{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"}],"external_weapon_rounds":{"us_mk82":6},"source":"existing verified US2025 B44/H1-3/H4-2 full Mk82 load; no fabricated integral gun rounds/mass"});
    for class in [AIRCRAFT, "airfield"] {
        input["context"]["detection_2026"]["bindings"][format!("{class}:visual")] =
            json!({"type":"visual","source":"scenario_assumption: fitted native lookout"});
        input["context"]["detection_2026"]["targets"][class] = json!({"source":"scenario_assumption: source groundD and clear native Visual profile, no stealth cover","size":if class=="airfield"{3}else{5},"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    }
    let scenario: Scenario = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&scenario).unwrap()["context"]["ground_2026"]["facilities"]["HANGAR"]
            ["stored_airframes"],
        input["context"]["ground_2026"]["facilities"]["HANGAR"]["stored_airframes"]
    );
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    harpoon_core::rules::aaw2026::configure_database(&mut db);
    db.platforms.get_mut("us_f18e").unwrap().sensors = vec!["visual".into()];
    db.platforms.get_mut("airfield").unwrap().weapons.clear();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(named(&w, "STRIKE"), EntityId(1));
    assert_eq!(named(&w, "HANGAR"), EntityId(2));
    assert_eq!(
        serde_json::to_value(
            w.find_contact(Side::Blue, EntityId(2))
                .expect("real native ground contact, no assigned private CID")
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
    assert_eq!(w.rules.aviation.as_ref().unwrap().airframes.len(), 3);
    w
}
#[test]
fn hg01_zero_dp_one_native_gun_critical_consumes_integral_source_hangar_airframe_losses_once() {
    let seed = seed();
    let mut w = world(seed);
    let actor = named(&w, "STRIKE");
    let target = named(&w, "HANGAR");
    let stock = stocks(&w, actor);
    let payload = av::own_view(&w, actor).unwrap()["flight"]["payload_fraction"].clone();
    let losses_before = w.losses[Side::Red.index()];
    let contact = w.find_contact(Side::Blue, target).unwrap();
    let cmd:Command=serde_json::from_value(json!({"cmd":"air_combat","unit":actor,"action":{"type":"surface_attack","contact":contact,"weapon":GUN,"rounds":1,"delivery":"Strafe","aimpoint":"range"}})).unwrap();
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let commands = [(Side::Blue, cmd)];
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
        .expect("actual native gun pass, not a direct call to a loss helper");
    assert_eq!(
        result["identity"],
        claim(),
        "stable original claim used for ordinary seed selection"
    );
    assert_eq!(result["critical_d6"], 4);
    assert_eq!(
        result["critical_count"], 1,
        "one critical deliberately avoids unresolved multi-critical special-loss multiplicity"
    );
    assert_eq!(result["direct_damage_points"].as_f64(), Some(0.));
    let facility = &w.rules.ground.as_ref().unwrap().facilities[&target];
    assert_eq!(facility.damage, 0.);
    assert_eq!(facility.critical_hits, 1);
    assert!(facility.disabled && !facility.destroyed);
    assert_eq!(w.entity(target).unwrap().hp, 100.);
    assert!(
        facility.pending_aircraft_loss.is_empty(),
        "selected original D6 is integral, no fractional rounding inference"
    );
    let loss = u32::from(hangar_die(seed) / 2);
    assert_eq!(
        facility.loss_count, loss,
        "source zeroDP critical must reach real stored-airframe loss consumer"
    );
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().airframes.len(),
        3 - loss as usize
    );
    let ready: u16 = w
        .entity(target)
        .unwrap()
        .deck
        .as_ref()
        .unwrap()
        .ready
        .iter()
        .filter(|(class, _)| class == "us_f18e")
        .map(|(_, count)| u16::from(*count))
        .sum();
    assert_eq!(
        ready,
        3 - loss as u16,
        "carrier ready ledger follows actual lost individual airframes"
    );
    assert_eq!(w.losses[Side::Red.index()], losses_before + loss);
    assert_eq!(stocks(&w, actor), stock);
    assert_eq!(
        av::own_view(&w, actor).unwrap()["flight"]["payload_fraction"],
        payload
    );
    let before_repeat = serde_json::to_value(&w).unwrap();
    ground2026::apply_damage(&mut w, target, 0., 1, &ground_identity()).unwrap();
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        before_repeat,
        "re-delivering the exact already-adjudicated common receipt is idempotent"
    );
}

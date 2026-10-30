//! New source candidates: explicit laboratory rockets, never a Hydra/LAU61 fit.
//! Unknown Scenario configuration/action is a normal source-red; no future Rust enums.
use harpoon_core::rules::aviation2026 as av;
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
const FIT: &str = "lab_mh60r_he_rocket12";
const ROCKET: &str = "lab_he_rocket20kg";

fn seed(gun: Option<&str>) -> u64 {
    (0..3_000_000)
        .find(|seed| {
            // Explicit independent RTS stream, stable across8/12 experimental arms.
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(2, 1, "9.6.7:salvo"));
            let roll = 1 + (r.next_f64() * 100.) as u8;
            let dice = [1 + (r.next_f64() * 6.) as u8, 1 + (r.next_f64() * 6.) as u8];
            let gun_miss = gun.is_none_or(|weapon| {
                let mut g = Rng::new(*seed).derive(Rng::hash_tag(
                    2,
                    3,
                    &format!("aaw:gun:3:{weapon}:0:2:1:1"),
                ));
                1 + (g.next_f64() * 100.) as u8 == 100
            });
            // Source A target adds2: raw2 becomes the same printed column4.
            roll == 1 && dice[0] + dice[1] == 2 && gun_miss
        })
        .expect("ordinary source seed selection; no World dice/contact/outcome editing")
}
fn scenario(seed: u64, escort: Option<&str>) -> Value {
    let target = |size| json!({"source":"scenario_assumption: clear controlled native visual signature","size":size,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    let mut value = json!({"id":"rocket-laboratory-source","name":"Author laboratory rockets on a verified MH60R airframe; not an Annex real weapon fit","seed":seed,"duration_s":1000,
    "weather":{"visibility_nm":40,"jam":[0,0]},"context":{
        "rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":0,"precipitation":"None"},
        "rocket_2026":{"source":"scenario_assumption: laboratory only; English2026 PDF108/9-7 general rocket algorithm; no Hydra mass or MH60S/LAU61 fit claim","enabled_laboratory_fits":[FIT]},
        "aviation_2026":{"source":"scenario_assumption: experiment uses source MH60R airframe plus author12*20kg rockets and80kg empty racks; no Annex weapon fit claim",
            "no_aa_surface_attack_source":"scenario_assumption: controlled weapons-hold target; real defensive fire overrides",
            "loads":{},"parking":{},"initial_fuel_nm":{"LAB":100},
            "surface_aimpoints":{"range":{"source":"scenario_assumption: fixed exercise coordinates allocated Blue","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
        "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: clear daylight actual native lookouts","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}
    },"units":[
            {"side":"Blue","class":FIT,"name":"LAB","pos":[0,-100./3600.],"course_deg":0,"speed_kts":100,"altitude_m":500,"fire_on":false},
            {"side":"Red","class":"cn_001","name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}
    ]});
    value["context"]["aviation_2026"]["loads"][FIT] = json!({"payload_kg":320,"tanks":[],"external_weapon_rounds":{"lab_he_rocket20kg":12},"source":"scenario_assumption: author20kg/rocket,10DP,0cm penetration,80kg empty racks; Ballistic experiment sight; none are Hydra/LAU61 source values"});
    for class in [FIT, "cn_001", "us_burke_2a", "us_ford"] {
        value["context"]["detection_2026"]["bindings"][format!("{class}:visual")] =
            json!({"type":"visual","source":"scenario_assumption: fitted real experiment lookout"});
        value["context"]["detection_2026"]["targets"][class] = target(if class == FIT {
            5
        } else if class == "cn_001" {
            0
        } else {
            1
        });
    }
    if let Some(class) = escort {
        value["units"].as_array_mut().unwrap().push(json!({"side":"Red","class":class,"name":"ESCORT","pos":[0,1],"course_deg":180,"speed_kts":0,"fire_on":false}));
    }
    value
}
fn world(seed: u64, escort: Option<&str>) -> World {
    let value = scenario(seed, escort);
    let parsed: Scenario = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&parsed).unwrap()["context"]["rocket_2026"],
        value["context"]["rocket_2026"],
        "laboratory source scope must survive Scenario parsing, not silently disappear"
    );
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    harpoon_core::rules::aaw2026::configure_database(&mut db);
    // Explicit laboratory lookout selection before native World construction.
    // No private contacts, live geometry, stocks, gun evidence or FCS are edited.
    db.platforms.get_mut("us_mh60r").unwrap().sensors = vec!["visual".into()];
    // Burke keeps its real SPY-1 AA director and original fitted radars;
    // all defenders already carry the explicitly bound Visual lookout.
    if let Some(class) = escort {
        let weapon = if class == "us_ford" {
            "ciws_30mm"
        } else {
            "us_mk45_mod4"
        };
        // A clearly selected single-AA-weapon exercise fit, with the original
        // source mount ammunition/directors intact and deterministic index0.
        db.platforms
            .get_mut(class)
            .unwrap()
            .weapons
            .retain(|m| m.weapon == weapon);
    }
    let mut w = harpoon_core::load_scenario(parsed, db)
        .expect("source registry supplies named laboratory fit before validation");
    harpoon_core::sim::tick(&mut w, &[]);
    let actor = named(&w, "LAB");
    assert!(
        w.entity(actor)
            .unwrap()
            .pos
            .range_to(harpoon_core::Vec2 { x: 0., y: 0. })
            < 1e-9,
        "valid source100kt cruise must naturally arrive overhead in one actual second"
    );
    w
}
fn named(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn rocket(w: &World, rounds: u16) -> (Side, Command) {
    let actor = named(w, "LAB");
    let target = named(w, "TARGET");
    let contact = w
        .find_contact(Side::Blue, target)
        .expect("actual native Visual contact");
    (Side::Blue,serde_json::from_value(json!({"cmd":"air_combat","unit":actor,"action":{"type":"rocket_attack","weapon":ROCKET,"rounds":rounds,"contact":contact,"aimpoint":"range"}})).expect("9.6.7 action consumer must be registered"))
}
fn stock(w: &World) -> u32 {
    w.entity(named(w, "LAB"))
        .unwrap()
        .weapons
        .iter()
        .filter(|m| m.weapon == ROCKET)
        .map(|m| m.mounts as u32 * m.ammo as u32)
        .sum()
}
fn detail(w: &World) -> Value {
    w.events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "9.6.7.source_rocket_attack" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "actual rocket consumer must resolve; state={:?}; events={:?}",
                w.rules.rocket,
                w.events.iter().rev().take(24).collect::<Vec<_>>()
            )
        })
}

#[test]
fn r01_eight_and_twelve_actual_rockets_share_four_pair_ph_but_use_actual_two_d6_damage_rows() {
    let seed = seed(None);
    let mut probabilities = Vec::new();
    for (rounds, hits) in [(8, 1), (12, 2)] {
        let mut w = world(seed, None);
        let actor = named(&w, "LAB");
        let target = named(&w, "TARGET");
        let target_hp = w.entity(target).unwrap().hp;
        let initial = av::own_view(&w, actor).unwrap()["flight"].clone();
        let max_payload = w.rules.aviation.as_ref().unwrap().flights[&actor]
            .spec
            .max_payload_kg;
        assert_eq!(stock(&w), 12);
        let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        let command = rocket(&w, rounds);
        harpoon_core::sim::tick(&mut w, &[command.clone()]);
        harpoon_core::sim::tick(&mut restored, &[command]);
        let d = detail(&w);
        assert_eq!(d["physical_size"],"A","this positive uses the already registered source A hull; original B target remains source-gap evidence");
        assert_eq!(d["rounds"], rounds);
        assert_eq!(d["pairs_for_ph"], 4);
        assert_eq!(d["d100"], 1);
        assert_eq!(d["modified_2d6"], 4);
        assert_eq!(d["total_hits"], hits);
        assert_eq!(d["defense"]["no_aa"], true);
        assert_eq!(d["defense"]["heavy_aa"], false);
        assert_eq!(stock(&w), 12 - rounds as u32);
        let after = av::own_view(&w, actor).unwrap()["flight"].clone();
        assert_eq!(after["external_weapon_rounds"][ROCKET], 12 - rounds);
        assert!(
            (initial["payload_fraction"].as_f64().unwrap()
                - after["payload_fraction"].as_f64().unwrap()
                - rounds as f64 * 20. / max_payload)
                .abs()
                < 1e-10
        );
        assert_eq!(d["damage_points_per_hit"].as_f64(), Some(10.0));
        assert!(w.entity(target).unwrap().hp<target_hp,"actual source rocket hits must reach common target damage, not only an outcome helper or trace");
        assert_eq!(d["equipment_source"], "scenario_laboratory");
        probabilities.push(d["probability"].clone());
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            serde_json::to_value(&restored).unwrap()
        );
    }
    assert_eq!(
        probabilities[0], probabilities[1],
        "over8 rockets increases damage potential, not chance to hit"
    );
}

#[test]
fn r02_same_tick_actual_ordinary_and_heavy_guns_override_author_noaa_before_rocket_release() {
    for (class, weapon, heavy) in [
        ("us_burke_2a", "us_mk45_mod4", false),
        ("us_ford", "ciws_30mm", true),
    ] {
        for reverse in [false, true] {
            let mut w = world(seed(Some(weapon)), Some(class));
            let (actor, escort) = (named(&w, "LAB"), named(&w, "ESCORT"));
            let contact = w
                .find_contact(Side::Red, actor)
                .expect("native defensive lookout sees actual rocket aircraft");
            let before = w
                .entity(escort)
                .unwrap()
                .weapons
                .iter()
                .find(|m| m.weapon == weapon)
                .unwrap()
                .ammo;
            let attack = rocket(&w, 8);
            let defense=(Side::Red,serde_json::from_value(json!({"cmd":"fire","unit":escort,"contact":contact,"weapon":weapon,"rounds":1})).unwrap());
            let commands = if reverse {
                vec![defense, attack]
            } else {
                vec![attack, defense]
            };
            let mut restored: World =
                serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
            harpoon_core::sim::tick(&mut w, &commands);
            harpoon_core::sim::tick(&mut restored, &commands);
            assert!(
                w.entity(actor).unwrap().alive,
                "ordinary deterministic source gun miss is selected before construction"
            );
            assert!(w.events.iter().any(
                |e| matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="8.1.3.source_gun")
            ));
            let d = detail(&w);
            assert_eq!(d["defense"]["no_aa"], false);
            assert_eq!(d["defense"]["heavy_aa"], heavy);
            assert_eq!(stock(&w), 4);
            assert_eq!(
                w.entity(escort)
                    .unwrap()
                    .weapons
                    .iter()
                    .find(|m| m.weapon == weapon)
                    .unwrap()
                    .ammo,
                before - 1
            );
            assert_eq!(
                serde_json::to_value(&w).unwrap(),
                serde_json::to_value(&restored).unwrap()
            );
        }
    }
}

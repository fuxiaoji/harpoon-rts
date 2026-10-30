//! Existing-public-API source candidates; SALH path is prospectively configured.
//! Real GBU12 row, explicit author F18E mounting. No private Flight/contact/dice edits.
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
const W: &str = "lab_gbu12_salh2";
fn die(seed: u64, actor: u32, at: i64, round: u16) -> u8 {
    let id = format!("9.6.8:{actor}:{at}:1:{round}");
    let mut r = Rng::new(seed).derive(Rng::hash_tag(at, actor, &id));
    1 + (r.next_f64() * 100.) as u8
}
fn placed(seed: u64, actor: u32, at: i64, rounds: u16) -> bool {
    let id = ac::budget::claim_identity(
        ac::budget::AirAttackKind::Pgm,
        EntityId(actor),
        at,
        &json!({"weapon":W,"rounds":rounds,"briefing":"range"}),
    ) + ":rf-placement";
    let mut r = Rng::new(seed).derive(Rng::hash_tag(0, 0, &id));
    i64::from(1 + (r.next_f64() * 10.) as u8)
        <= fc::tables()["placement_quality"]["Good"].as_i64().unwrap()
}
fn seed(smoke: bool) -> u64 {
    (0..1_000_000)
        .find(|s| {
            if smoke {
                placed(*s, 1, 2, 4)
                    && placed(*s, 4, 221, 1)
                    && die(*s, 1, 2, 0) > 70
                    && die(*s, 1, 2, 1) <= 70
                    && (18..=70).contains(&die(*s, 1, 2, 2))
                    && die(*s, 1, 2, 3) <= 17
            } else {
                placed(*s, 1, 2, 1) && (18..=70).contains(&die(*s, 1, 2, 0))
            }
        })
        .unwrap()
}
fn input(smoke: bool) -> Value {
    let mut v = json!({"id":"salh-source","name":"source GBU12 and actual friendly designator lab","seed":seed(smoke),"duration_s":600,
 "weather":{"visibility_nm":40,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},
 "aviation_2026":{"source":"scenario_assumption: author experimental four-GBU12 load on actual F18E, not a claimed Annex loadout; published277kg each",
 "loads":{"us_f18e":{"payload_kg":1108,"tanks":[],"external_weapon_rounds":{(W):4},"source":"scenario_assumption: exact4x277kg experimental GBU12 external load, source weapon row US2025 PDF101/H2-3"}},
 "parking":{},"initial_fuel_nm":{"STRIKE":100,"NEXT":100},"surface_aimpoints":{"range":{"source":"scenario_assumption: prebriefed fixed target coordinates","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
 "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: controlled clear LOS/no moisture; no free FCS from spot","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
 "laser_sensors":{"source":"scenario_assumption: actually installed controlled compatible designator/spot hardware","fits":{"PAINT":{"source":"scenario_assumption: actual installed friendly designator retrofit","device":"designator","installed":true,"range_nm":5,"range_source":"scenario_assumption: explicit controlled designator envelope","bearing_policy":"exact_scene_ray"},"STRIKE":{"source":"scenario_assumption: actual installed spot receiver","device":"lst","installed":true,"spot_range_nm":5,"spot_range_source":"scenario_assumption: controlled spot envelope","bearing_policy":"exact_scene_ray"}},
 "initial_jobs":{"PAINT":{"source":"scenario_assumption: allocated fixed target painting job","enabled":true,"purpose":"designation","known_aimpoint":{"x":0,"y":0},"aimpoint_source":"scenario_assumption: own fixed briefing; no hidden current target tracking"}}}},
 "salh_2026":{"source":"English2026 PDF108/109 plus US Aircraft2025 PDF101/H2-3; native path prospective, not an already-supported SATNAV alias",
 "weapons":{(W):{"source":"US Aircraft2025 PDF101/H2-3 exact GBU12 PavewayII","guidance":"SALH","generation":2,"min_range_nm":0.5,"range_nm":3.5,"speed_kts":528,"flight_path":"Glide","hang_weight_kg":277,"damage_points":35,"penetration_cm":4,"signature":"VSmall"}},
 "fits":{"STRIKE":{"source":"scenario_assumption: author mounting4xGBU12 on realF18E","weapon":W,"rounds":4,"external":true}},
 "briefings":{"range":{"source":"scenario_assumption: allocated fixed target/source-compatible friendly designator","target":"TARGET","aimpoint":"range","known_kind":"Base","known_motion":"stationary","allowed_sides":["Blue"],"delivery":"glide","designator":"PAINT","device":"designator","fcs":{"source":"scenario_assumption: controlled GoodRF quality, not created by laser spot","quality":"Good"}}}},
 "ground_2026":{"source":"English2026 source controlled MediumHangar D/100DP/unarmored","facilities":{"TARGET":{"template":"hangar_medium","custom":null,"source":"English2026 PDF118/11-3"}}}},
 "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-2],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
 {"side":"Blue","class":"ru_ka27","name":"PAINT","pos":[1,-1],"course_deg":0,"speed_kts":0,"altitude_m":1000,"fire_on":false},
 {"side":"Red","class":"airfield","name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}]});
    if smoke {
        v["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":"us_f18e","name":"NEXT","pos":[0,-2.-460.*220./3600.],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false}));
        v["context"]["salh_2026"]["fits"]["NEXT"] =
            v["context"]["salh_2026"]["fits"]["STRIKE"].clone();
        v["context"]["detection_2026"]["laser_sensors"]["fits"]["NEXT"] =
            v["context"]["detection_2026"]["laser_sensors"]["fits"]["STRIKE"].clone();
    }
    v
}
fn world(smoke: bool) -> World {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let def:harpoon_core::db::WeaponDef=serde_json::from_value(json!({"id":W,"name":"GBU12 PavewayII source SALH2 laboratory mounting","kind":"Bomb","targets":{"surface":true,"land":true},"range_nm":3.5,"min_range_nm":0.5,"speed_kts":528,"pk":0,"salvo":1,"reload_s":0,"damage":35,"guidance":"BallisticGun","rule_data":{"source":"US Aircraft2025 PDF101/H2-3 GBU12; pk0 withdraws legacy fallback, native SALH table required","ship_dp":35,"penetration_cm":4,"profile":"glide","bomb_kind":"LDGP"}})).unwrap();
    db.weapons.insert(W.into(), def);
    for class in ["us_f18e", "ru_ka27", "airfield"] {
        let p = db.platforms.get_mut(class).unwrap();
        p.sensors.clear();
        p.weapons.clear();
    }
    db.platforms
        .get_mut("us_f18e")
        .unwrap()
        .weapons
        .push(harpoon_core::db::WeaponFit {
            weapon: W.into(),
            mounts: 1,
            ammo: 4,
        });
    harpoon_core::load_scenario(
        serde_json::from_value::<Scenario>(input(smoke)).unwrap(),
        db,
    )
    .unwrap()
}
fn command(actor: EntityId, rounds: u16) -> (Side, Command) {
    (
        Side::Blue,
        Command::AirCombat {
            unit: actor,
            action: ac::Action::GuidedSurfaceAttack {
                weapon: W.into(),
                rounds,
                briefing: "range".into(),
            },
        },
    )
}
fn terminals(w: &World, actor: EntityId) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule,
                unit: Some(id),
                detail,
                ..
            } if rule == "9.6.8.source_terminal" && *id == actor => {
                let d: Value = serde_json::from_str(detail).unwrap();
                (d["weapon"] == W).then_some(d)
            }
            _ => None,
        })
        .collect()
}
fn prerequisite(w: &World) {
    let laser = &w.rules.detection.as_ref().unwrap().laser;
    assert!(
        laser.beams.values().any(|b| b.purpose
            == harpoon_core::rules::detection2026::laser::Purpose::Designation
            && b.emitter == EntityId(2)
            && b.target == Some(EntityId(3))),
        "actual existing physical friendly designator beam must already reach the fixed target"
    );
    assert!(
        !laser.spots.is_empty(),
        "actual first-class tracker must receive that spot, no synthetic illumination flag"
    );
}
#[test]
fn salh01_real_designator_is_required_until_impact_and_stopping_it_quarters_native_probability() {
    for stop in [false, true] {
        let mut w = world(false);
        harpoon_core::sim::tick(&mut w, &[]);
        prerequisite(&w);
        let c = command(EntityId(1), 1);
        harpoon_core::sim::tick(&mut w, &[c]);
        assert!(w.entities.values().any(|e|e.projectile.as_ref().is_some_and(|p|p.weapon==W)),"the real SALH source registry/admission must release a physical source packet; SATNAV-only or legacy fallback cannot satisfy LOS testing");
        if stop {
            let c = (
                Side::Blue,
                Command::Detection {
                    unit: EntityId(2),
                    action: harpoon_core::rules::detection2026::DetectionAction::Laser {
                        action: harpoon_core::rules::detection2026::laser::Action::Emission {
                            enabled: false,
                        },
                    },
                },
            );
            harpoon_core::sim::tick(&mut w, &[c]);
        } else {
            harpoon_core::sim::tick(&mut w, &[]);
        }
        let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        for _ in 0..35 {
            harpoon_core::sim::tick(&mut w, &[]);
            harpoon_core::sim::tick(&mut saved, &[]);
        }
        let t = terminals(&w, EntityId(1));
        assert_eq!(t.len(), 1);
        assert!(
            (t[0]["probability"].as_f64().unwrap() - if stop { 0.175 } else { 0.70 }).abs() < 1e-9
        );
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            serde_json::to_value(&saved).unwrap()
        );
    }
}
#[test]
fn salh02_one_real_designator_supports_four_same_target_weapons_and_first_miss_still_obscures_third_plus(
) {
    let mut w = world(true);
    harpoon_core::sim::tick(&mut w, &[]);
    prerequisite(&w);
    let c = command(EntityId(1), 4);
    harpoon_core::sim::tick(&mut w, &[c]);
    assert_eq!(w.entities.values().filter(|e|e.projectile.as_ref().is_some_and(|p|p.weapon==W)).count(),4,"one designator is not a two-weapon channel cap; all four legitimately released packets must exist");
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    while w.tick < 40 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    let t = terminals(&w, EntityId(1));
    assert_eq!(t.len(), 4);
    assert!(t[0]["d100"].as_u64().unwrap() > 70);
    assert_eq!(
        t[0]["hit"], false,
        "source-selected original first miss must still make smoke"
    );
    for (i, r) in t.iter().enumerate() {
        assert!(
            (r["probability"].as_f64().unwrap() - if i < 2 { 0.70 } else { 0.175 }).abs() < 1e-9
        );
    }
    while w.tick < 220 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    let c = command(EntityId(4), 1);
    harpoon_core::sim::tick(&mut w, &[c.clone()]);
    harpoon_core::sim::tick(&mut saved, &[c]);
    while w.tick < 250 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    let next = terminals(&w, EntityId(4));
    assert_eq!(next.len(), 1);
    assert!(
        (next[0]["probability"].as_f64().unwrap() - 0.70).abs() < 1e-9,
        "after a complete TacticalTurn of actual smoke time, native source chance must recover"
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}

//! Existing-public-API source candidates; SALH path is prospectively configured.
//! Real GBU12 row, explicit author F18E mounting. No private Flight/contact/dice edits.
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
const W: &str = "lab_agm65e2_maverick";
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
                    && (21..=80).contains(&die(*s, 1, 2, 2))
                    && die(*s, 1, 2, 3) <= 17
            } else {
                placed(*s, 1, 2, 1) && (21..=80).contains(&die(*s, 1, 2, 0))
            }
        })
        .unwrap()
}
fn input(smoke: bool) -> Value {
    let mut v = json!({"id":"salh-source","name":"source GBU12 and actual friendly designator lab","seed":seed(smoke),"duration_s":600,
 "weather":{"visibility_nm":40,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},
 "aviation_2026":{"source":"scenario_assumption: author experimental four-GBU12 load on actual F18E, not a claimed Annex loadout; published277kg each",
 "loads":{"us_f18e":{"payload_kg":300,"tanks":[],"external_weapon_rounds":{(W):1},"source":"scenario_assumption: exact4x277kg experimental GBU12 external load, source weapon row US2025 PDF101/H2-3"}},
 "parking":{},"initial_fuel_nm":{"STRIKE":100,"NEXT":100},"surface_aimpoints":{"range":{"source":"scenario_assumption: prebriefed fixed target coordinates","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
 "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: controlled clear LOS/no moisture; no free FCS from spot","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
 "laser_sensors":{"source":"scenario_assumption: actually installed controlled compatible designator/spot hardware","fits":{"PAINT":{"source":"scenario_assumption: actual installed friendly designator retrofit","device":"designator","installed":true,"range_nm":5,"range_source":"scenario_assumption: explicit controlled designator envelope","bearing_policy":"exact_scene_ray"},"STRIKE":{"source":"scenario_assumption: actual installed spot receiver","device":"lst","installed":true,"spot_range_nm":5,"spot_range_source":"scenario_assumption: controlled spot envelope","bearing_policy":"exact_scene_ray"}},
 "initial_jobs":{"PAINT":{"source":"scenario_assumption: allocated fixed target painting job","enabled":true,"purpose":"designation","known_aimpoint":{"x":0,"y":0},"aimpoint_source":"scenario_assumption: own fixed briefing; no hidden current target tracking"}}}},
 "salh_2026":{"source":"English2026 PDF108/109 plus US Aircraft2025 PDF101/H2-3; native path prospective, not an already-supported SATNAV alias",
 "weapons":{(W):{"source":"US Aircraft2025 PDF101/H2-3 exact GBU12 PavewayII","guidance":"SALH","generation":3,"min_range_nm":1.5,"range_nm":11,"speed_kts":791,"flight_path":"Direct","hang_weight_kg":300,"damage_points":34,"penetration_cm":8,"signature":"VSm"}},
 "fits":{"STRIKE":{"source":"scenario_assumption: author mounting4xGBU12 on realF18E","weapon":W,"rounds":1,"external":true}},
 "briefings":{"range":{"source":"scenario_assumption: allocated fixed target/source-compatible friendly designator","target":"TARGET","aimpoint":"range","known_kind":"Ship","known_motion":"stationary","allowed_sides":["Blue"],"delivery":"glide","designator":"PAINT","device":"designator","fcs":{"source":"scenario_assumption: controlled GoodRF quality, not created by laser spot","quality":"Good"}}}},
 "ground_2026":{"source":"English2026 source controlled MediumHangar D/100DP/unarmored","facilities":{"TARGET":{"template":"hangar_medium","custom":null,"source":"English2026 PDF118/11-3"}}}},
 "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-2],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
 {"side":"Blue","class":"ru_ka27","name":"PAINT","pos":[1,-1],"course_deg":0,"speed_kts":0,"altitude_m":1000,"fire_on":false},
 {"side":"Red","class":"cn_052d","name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}]});
    v["context"].as_object_mut().unwrap().remove("ground_2026");
    v["context"]["pgm_2026"]=json!({"source":"scenario_assumption: target countermeasures","countermeasures":{"TARGET":{"source":"scenario_assumption: no PGM countermeasure","generation":0}}});
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
    for class in ["us_f18e", "ru_ka27", "cn_052d"] {
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
            ammo: 1,
        });
    db.platforms.get_mut("cn_052d").unwrap().rule_data=Some(harpoon_core::rules::damage_runtime::PlatformDamage{source:"scenario_assumption: unarmored C test hull".into(),size:"C".into(),role:"surface".into(),belt_cm:0.,deck_cm:0.,..Default::default()});
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
fn salh3_actual_designation_and_interruption_use_original_dice_and_common_ship_damage() {
    for stop in [false,true] {
        let mut w=world(false); harpoon_core::sim::tick(&mut w,&[]); prerequisite(&w);
        harpoon_core::sim::tick(&mut w,&[command(EntityId(1),1)]);
        assert!(w.entities.values().any(|e|e.projectile.as_ref().is_some_and(|p|p.weapon==W)));
        if stop {harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::Detection{unit:EntityId(2),action:harpoon_core::rules::detection2026::DetectionAction::Laser{action:harpoon_core::rules::detection2026::laser::Action::Emission{enabled:false}}})]);}
        for _ in 0..35 {harpoon_core::sim::tick(&mut w,&[]);}
        let t=terminals(&w,EntityId(1)); assert_eq!(t.len(),1);
        assert_eq!(t[0]["probability"],if stop {0.2} else {0.8});
        if !stop {assert_eq!(t[0]["hit"],true); assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,detail,..} if rule=="damage_roll" && detail.contains("14.1.5.airframe_extra"))));}
    }
}

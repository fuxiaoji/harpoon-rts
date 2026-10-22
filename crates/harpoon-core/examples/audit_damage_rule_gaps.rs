//! Diagnostic of the existing RTS damage approximation, NOT a strict-rules test.
//! Forced hits in an isolated World; never connects to a server or changes saves.
use harpoon_core::{
    air, damage, db::Database, load_scenario, AirMission, EventKind, Rng, Scenario,
};
use serde_json::{json, Value};
use std::path::Path;

fn impacts(w: &harpoon_core::World) -> Vec<f64> {
    w.events
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::Impact { damage, .. } => Some(damage),
            _ => None,
        })
        .collect()
}

fn main() {
    let mut db = Database::built_in();
    let mut paths: Vec<_> = std::fs::read_dir("database/volumes")
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.extension().is_some_and(|v| v == "json")
                && !p.file_name().unwrap().to_string_lossy().starts_with('_')
        })
        .collect();
    paths.sort();
    for p in paths {
        db = db.merged(&Database::from_json(&std::fs::read_to_string(p).unwrap()).unwrap());
    }
    db.validate().unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"damage-gap-diagnostic","name":"Isolated rule audit","seed":20261021,
        "context":{"rules_profile":"rts-legacy-v1"},
        "duration_s":100000,"weather":{"visibility_nm":12,"jam":[0,0],"overcast":0,"rain":0},
        "units":[{"side":"Red","class":"us_ford","name":"Audit target","pos":{"x":0,"y":0}}]
    }))
    .unwrap();
    let pristine = load_scenario(sc, db).unwrap();
    let id = *pristine.entities.keys().next().unwrap();
    let mut rows: Vec<Value> = vec![];
    let mut record = |key: &str, source: &str, expected: &str, actual: Value, gap: bool| {
        rows.push(json!({"id":key,"source":source,"strict_requirement":expected,"observed":actual,"gap_detected":gap}));
    };
    let missile = pristine.db.weapons["cn_yj18"].clone();
    let mut w = pristine.clone();
    w.entity_mut(id).unwrap().weapons.clear();
    w.tick = 10;
    for _ in 0..2 {
        damage::apply_hit_at(&mut w, id, &missile, false, Some(90.0));
    }
    let d = impacts(&w);
    record(
        "same_tick_stream",
        "engineering: stable per-hit random event identity",
        "Distinct hits must not restart the identical damage stream",
        json!(d),
        d.len() == 2 && d[0] == d[1],
    );

    let mut w = pristine.clone();
    w.entity_mut(id).unwrap().hp = 1667.0 - 834.0;
    let speed = w.entity(id).unwrap().max_speed(&w.db);
    record(
        "dp_speed",
        "HV2026 14.1.1; America A-4 PDF7",
        "Ford: at 834 cumulative DP, maximum speed 16 knots",
        json!(speed),
        (speed - 16.0).abs() > 1e-9,
    );

    let mission = AirMission::Recon {
        point: harpoon_core::Vec2::new(0.0, 10.0),
    };
    let mut w = pristine.clone();
    w.entity_mut(id).unwrap().hp = 1667.0 * 0.25;
    let launched = air::launch_aircraft(&mut w, id, "fighter", 1, &mission).is_ok();
    record(
        "massive_damage_launch",
        "HV2026 14.2.1 PDF132",
        "At 25% original DP remaining, a carrier cannot launch or land",
        json!({"launch_succeeded":launched}),
        launched,
    );

    let mut w = pristine.clone();
    w.entity_mut(id)
        .unwrap()
        .ext
        .insert("flight_deck_hit".into(), 1.0);
    let launched = air::launch_aircraft(&mut w, id, "fighter", 1, &mission).is_ok();
    record(
        "deck_flag_unconsumed",
        "HV2026 14.5 Flight Deck PDF135",
        "A damaged launch/recovery location must restrict the corresponding operation",
        json!({"existing_undifferentiated_deck_flag":1,"launch_succeeded":launched}),
        launched,
    );
    let ac = w
        .entities
        .values()
        .find(|e| e.class == "fighter")
        .unwrap()
        .id;
    let recovery = air::compatible_recovery(&w, ac, id);
    record(
        "deck_recovery_unrestricted",
        "HV2026 14.5 Flight Deck PDF135",
        "An aft hit destroys arresting gear and prevents ordinary fixed-wing recovery",
        json!({"existing_undifferentiated_deck_flag":1,"compatible_recovery":recovery}),
        recovery,
    );

    let mut w = pristine.clone();
    w.entity_mut(id).unwrap().burn_s = 60.0;
    damage::update_fires(&mut w);
    let fire_loss = 1667.0 - w.entity(id).unwrap().hp;
    record("instant_fire_drain","HV2026 14.1/14.4 PDF127/133","Ordinary A-E fire secondary DP begins at the third Tactical Turn resolution, after the initial DC roll",json!({"first_second_dp_loss":fire_loss}),fire_loss>0.0);

    let mut w = pristine.clone();
    w.entity_mut(id).unwrap().systems.radar = 0.0;
    w.entity_mut(id)
        .unwrap()
        .ext
        .insert("dc_last_hit_tick".into(), 0.0);
    w.tick = 300;
    harpoon_core::rules::damage_ext::step(&mut w);
    let radar = w.entity(id).unwrap().systems.radar;
    record(
        "five_minute_repair",
        "HV2026 14.6 PDF136",
        "Sensor repairs are post-battle rolls at 6/12/24/48 hours",
        json!({"radar_after_300s":radar}),
        radar > 0.0,
    );

    let gun = pristine.db.weapons["gun_30mm"].clone();
    assert!(gun.damage * 1.15 < 1667.0 * 0.01);
    let mut count = 0;
    let mut labels = std::collections::BTreeSet::new();
    for seed in 0..512 {
        let mut w = pristine.clone();
        w.seed = seed;
        w.rng = Rng::new(seed);
        w.entity_mut(id).unwrap().weapons.clear();
        damage::apply_hit_at(&mut w, id, &gun, false, Some(90.0));
        for ev in &w.events {
            if let EventKind::CriticalHit { system, .. } = &ev.kind {
                count += 1;
                labels.insert(system.clone());
            }
        }
    }
    record(
        "below_one_percent",
        "HV2026 14.1.2/14.1.3 PDF129; expanded PDF2",
        "A single-turn 30mm hit below 1% original DP produces no damage-ratio critical",
        json!({"weapon":gun.id,"scalar_dp":gun.damage,"original_dp":1667,"trials":512,"critical_events":count,"systems":labels}),
        count > 0,
    );

    let mut w = pristine.clone();
    let arm = w.db.weapons["cn_yj91"].clone();
    damage::apply_hit_at(&mut w, id, &arm, false, Some(90.0));
    let d = impacts(&w);
    record("arm_hull_damage","HV2026 7.4.2 PDF77/14.1.4 PDF129-130; China H4 PDF82","YJ-91 PRH airburst: no primary hull DP; D6+2 criticals with the first on the targeted radar",json!({"hull_dp":d,"guidance":arm.guidance}),d.iter().any(|d|*d>0.0));

    let mut ds = vec![];
    for seed in 0..16 {
        let mut w = pristine.clone();
        w.seed = seed;
        w.rng = Rng::new(seed);
        w.entity_mut(id).unwrap().weapons.clear();
        damage::apply_hit_at(&mut w, id, &missile, false, Some(90.0));
        ds.push(impacts(&w)[0]);
    }
    record("scalar_jitter","HV2026 14.1/14.1.5 PDF127/130; China D1 PDF71","YJ-18 entry 33+D6/2 means 33 primary DP plus D6/2 extra criticals, not an invented +/-15% hull multiplier",json!({"primary_dp_samples":ds}),ds.iter().any(|v|(*v-33.0).abs()>1e-9));

    let output = std::env::args()
        .nth(1)
        .expect("provide output JSON outside game saves");
    let p = Path::new(&output);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let detected = rows.iter().filter(|r| r["gap_detected"] == true).count();
    let result = json!({"kind":"baseline_gap_diagnostic_not_rules_compliance","production_modified":false,"cases":rows.len(),"gaps_detected":detected,"rows":rows});
    std::fs::write(p, serde_json::to_string_pretty(&result).unwrap()).unwrap();
    println!(
        "{detected}/{} existing-model discrepancies reproduced -> {output}",
        result["cases"]
    );
    assert_eq!(
        detected,
        result["cases"].as_u64().unwrap() as usize,
        "baseline behavior changed: re-audit before updating expectations"
    );
}

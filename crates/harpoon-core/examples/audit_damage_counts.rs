//! Read-only damage sandbox. Run from harpoon-rts; never opens a game server.
//! All arrivals are forced successful hits, one per simulated second.
use harpoon_core::{
    damage,
    db::{Database, WeaponKind},
    load_scenario, Rng, Scenario,
};
use serde_json::json;
use std::{collections::BTreeSet, path::Path};

fn main() {
    let trials: usize = std::env::args()
        .nth(1)
        .map(|s| s.parse().unwrap())
        .unwrap_or(20_000);
    assert!(trials > 1);
    let mut db = Database::built_in();
    let mut paths: Vec<_> = std::fs::read_dir("database/volumes")
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "json")
                && !p.file_name().unwrap().to_string_lossy().starts_with('_')
        })
        .collect();
    paths.sort();
    for p in paths {
        db = db.merged(&Database::from_json(&std::fs::read_to_string(p).unwrap()).unwrap());
    }
    db.validate().unwrap();
    let cn =
        Database::from_json(&std::fs::read_to_string("database/volumes/cn_navy.json").unwrap())
            .unwrap();
    let ids: BTreeSet<_> = cn
        .weapons
        .keys()
        .cloned()
        .chain(
            cn.platforms
                .values()
                .flat_map(|p| p.weapons.iter().map(|m| m.weapon.clone())),
        )
        .collect();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"damage-sandbox", "name":"Damage sandbox", "seed":20261021,
        "context":{"rules_profile":"rts-legacy-v1"},
        "duration_s":100000, "weather":{"visibility_nm":12,"jam":[0,0],"overcast":0,"rain":0},
        "units":[{"side":"Red","class":"us_ford","name":"Sandbox carrier","pos":{"x":0,"y":0}}]
    }))
    .unwrap();
    let mut world = load_scenario(sc, db).unwrap();
    let id = *world.entities.keys().next().unwrap();
    let pristine = world.entities[&id].clone();
    assert_eq!(pristine.hp, 1667.0);
    assert_eq!(
        harpoon_core::rules::damage_tables::armor_factor("us_ford", WeaponKind::Ascm, Some(90.0)),
        1.0
    );
    assert_eq!(
        harpoon_core::rules::damage_tables::armor_factor(
            "us_ford",
            WeaponKind::Torpedo,
            Some(90.0)
        ),
        1.0
    );
    let mut check_target = pristine.clone();
    check_target.weapons.clear();
    world.entities.insert(id, check_target);
    world.tick = 10;
    let check_weapon = world.db.weapons["cn_yj18"].clone();
    world.events.clear();
    for _ in 0..2 {
        damage::apply_hit_at(&mut world, id, &check_weapon, false, Some(90.0));
    }
    let same_tick_damage: Vec<_> = world
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            harpoon_core::EventKind::Impact { damage, .. } => Some(*damage),
            _ => None,
        })
        .collect();
    assert_eq!(same_tick_damage.len(), 2);
    assert_eq!(same_tick_damage[0], same_tick_damage[1]);
    let mut rows = vec![];
    for wid in ids {
        let weapon = world.db.weapons[&wid].clone();
        if !weapon.targets.surface {
            continue;
        }
        let cn_platforms: Vec<_> = cn
            .platforms
            .values()
            .filter(|p| p.weapons.iter().any(|m| m.weapon == wid))
            .map(|p| p.id.clone())
            .collect();
        let mut counts = Vec::with_capacity(trials);
        let mut magazines = 0usize;
        for trial in 0..trials {
            world.entities.insert(id, pristine.clone());
            world.rng = Rng::new(20261021 + trial as u64);
            world.seed = 20261021 + trial as u64;
            world.tick = 0;
            world.events.clear();
            world.event_seq = 0;
            world.losses = [0, 0];
            world.losses_by_kind.clear();
            let mut count = 0u32;
            while world.entities[&id].alive {
                count += 1;
                assert!(count <= 10_000, "unexpected nontermination");
                world.tick += 1;
                harpoon_core::rules::damage_ext::step(&mut world);
                damage::apply_hit_at(
                    &mut world,
                    id,
                    &weapon,
                    matches!(
                        weapon.kind,
                        WeaponKind::Torpedo | WeaponKind::Lwt | WeaponKind::AswRocket
                    ),
                    Some(90.0),
                );
                damage::update_fires(&mut world);
                if world.events.iter().any(|e| matches!(&e.kind, harpoon_core::EventKind::CriticalHit { system, .. } if system == "magazine_detonation")) {
                    magazines += 1;
                }
                world.events.clear();
            }
            counts.push(count);
        }
        counts.sort_unstable();
        let mean = counts.iter().map(|&x| x as f64).sum::<f64>() / trials as f64;
        let var = counts
            .iter()
            .map(|&x| (x as f64 - mean).powi(2))
            .sum::<f64>()
            / (trials - 1) as f64;
        rows.push(json!({"weapon":wid,"name":weapon.name,"kind":weapon.kind,"damage":weapon.damage,"cn_platforms":cn_platforms,
            "nominal_hp_over_damage":1667.0/weapon.damage,"nominal_integer":(1667.0/weapon.damage).ceil(),
            "mean_hits":mean,"mean_standard_error":(var/trials as f64).sqrt(),
            "median_hits":counts[trials/2],"p90_hits":counts[(trials*9/10).min(trials-1)],
            "magazine_sink_fraction":magazines as f64/trials as f64}));
    }
    let result = json!({"trials_per_weapon":trials,"base_seed":20261021,"target":"us_ford","hp":1667,"same_tick_damage_sample":same_tick_damage,
        "protocol":"Forced successful hits; broadside; one hit per second; production damage/control/fire functions; no defense, targeting, flight, ammunition exhaustion or return fire; fully stocked target mounts retained for critical checks. Paired seeds across weapons.","rows":rows});
    let output = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "damage-counts.json".into());
    if let Some(p) = Path::new(&output).parent() {
        if !p.as_os_str().is_empty() {
            std::fs::create_dir_all(p).unwrap();
        }
    }
    std::fs::write(&output, serde_json::to_string_pretty(&result).unwrap()).unwrap();
    println!(
        "{} weapons x {trials} trials -> {output}",
        result["rows"].as_array().unwrap().len()
    );
}

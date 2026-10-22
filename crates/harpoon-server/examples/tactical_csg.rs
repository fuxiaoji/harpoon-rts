//! Runs the production staff, never an inline approximation.
//! cargo run --release -p harpoon-server --example tactical_csg -- <seed> <output.json>
#[allow(dead_code)]
#[path = "../src/ai.rs"]
mod ai;
#[allow(dead_code)]
#[path = "../src/tactical.rs"]
mod tactical;
#[path="support/flight_audit.rs"]mod flight_audit;

use ai::CommandStaff;
use harpoon_core::{db::Database, observe::observe, scenario::scenario_from_json, types::Side};
use serde_json::json;
use std::collections::BTreeMap;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let seed = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(20261020);
    let mut db = Database::built_in();
    let mut files: Vec<_> = std::fs::read_dir("database/volumes")
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter(|p| !p.file_name().unwrap().to_string_lossy().starts_with('_'))
        .collect();
    files.sort();
    for f in files {
        db = db.merged(&Database::from_json(&std::fs::read_to_string(f).unwrap()).unwrap());
    }
    let text=args.get(4).map(|p|std::fs::read_to_string(p).unwrap());
    let mut sc = scenario_from_json(text.as_deref().unwrap_or(include_str!("../../../scenarios/csg_grand_battle.json"))).unwrap();
    sc.seed = seed;
    let mut w = harpoon_core::load_scenario(sc, db.clone()).unwrap();
    let mut staffs = [
        tactical::TacticalStaff::with_db(Side::Blue, db.clone()),
        tactical::TacticalStaff::with_db(Side::Red, db.clone()),
    ];
    let mut snapshots = Vec::new();
    let mut releases = Vec::new();
    let mut seen_seq = 0;
    let mut all_events = Vec::new();
    let mut flight_audit=flight_audit::Audit::default();
    let mut audit_memory=[serde_json::Value::Null,serde_json::Value::Null];
    let mut sea_position_checks = 0_u64;
    let mut prealert_commands=0_u64;
    let mut command_windows=Vec::new();
    let mut air_trace=Vec::new();let mut terminal_entries=Vec::new();let mut terminal_seen=std::collections::BTreeSet::new();
    let mut initial_units=serde_json::to_value(&w.entities).unwrap();
    // Only diagnostics read referee state. Both controllers below receive
    // their independent observations, never this initial inventory.
    if !initial_units.is_object() {initial_units=serde_json::Value::Null;}
    let start = std::time::Instant::now();
    let max_ticks = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(10800);
    while !w.over && w.tick < max_ticks {
        let mut commands = Vec::new();
        if w.tick % 30 == 0 {
            // Gather both observations before applying either side's orders.
            for (i, staff) in staffs.iter_mut().enumerate() {
                let side = if i == 0 { Side::Blue } else { Side::Red };
                let obs = observe(&w, side, 0, 256);
                let orders=staff.decide(&obs);
                let memory=serde_json::to_value(&*staff).unwrap();
                audit_memory[i]=memory.clone();
                for u in obs.units.iter().filter(|u|u.mission.as_deref()==Some("strike")) {
                    let m=&memory["air_minds"][u.id.0.to_string()];
                    let contact=m["target"].as_u64().and_then(|id|obs.contacts.iter().find(|c|c.id.0 as u64==id));
                    let range=u.weapons.iter().filter(|w|w.ammo>0&&db.weapons[&w.weapon].kind==harpoon_core::db::WeaponKind::Ascm).map(|w|db.weapons[&w.weapon].range_nm).fold(0.0,f64::max);
                    air_trace.push(json!({"tick":w.tick,"side":side,"unit":u.id,"home":m["home"],"class":u.class,"phase":m["phase"],"target":m["target"],"altitude_m":u.altitude_m,"endurance_s":u.endurance_s,
                        "distance_nm":contact.map(|c|u.pos.range_to(c.estimate)),"loaded_range_nm":range,"track_fresh":contact.is_some_and(|c|!c.stale&&!c.bearing_only&&c.quality>=0.4),"track_quality":contact.map(|c|c.quality)}));
                }
                if obs.exercise.as_ref().is_some_and(|e|e.locked) {prealert_commands+=orders.len() as u64;}
                command_windows.push(json!({"tick":w.tick,"side":side,"count":orders.len(),"locked":obs.exercise.as_ref().map(|e|e.locked)}));
                commands.extend(orders.into_iter().map(|c| (side, c)));
                tactical::record_notes(&mut w, side, staff.take_notes());
            }
        }
        harpoon_core::sim::tick(&mut w, &commands);
        // Post-game diagnostics only: exact projectile truth never reaches
        // either staff. A10nm terminal entry is a measuring boundary, not an
        // engine hit, interception, or claimed arrival after destruction.
        for u in w.entities.values().filter(|u|u.alive&&u.kind==harpoon_core::types::PlatformKind::Missile) {
            if let Some(p)=&u.projectile {
                if let Some(target)=p.target.and_then(|id|w.entity(id)).filter(|target|target.alive) {
                    if db.weapons.get(&p.weapon).is_some_and(|d|matches!(d.kind,harpoon_core::db::WeaponKind::Ascm|harpoon_core::db::WeaponKind::Slcm))
                        &&u.pos.range_to(target.pos)<=10.0&&terminal_seen.insert(u.id) {
                        terminal_entries.push(json!({"tick":w.tick,"projectile":u.id,"side":u.side,"target":target.id,"shooter":p.launched_by,"shooter_kind":w.entity(p.launched_by).map(|s|s.kind),"weapon":p.weapon}));
                    }
                }
            }
        }
        if let Some(terrain) = &w.context.terrain {
            for u in w.entities.values().filter(|u| u.alive && matches!(u.kind, harpoon_core::types::PlatformKind::Ship | harpoon_core::types::PlatformKind::Sub)) {
                assert!(!terrain.contains(u.pos), "{} entered charted land at tick {}", u.name, w.tick);
                sea_position_checks += 1;
            }
        }
        for e in w.events.iter().rev().take_while(|e| e.seq > seen_seq) {
            if let harpoon_core::EventKind::WeaponsRelease {
                shooter,
                weapon,
                count,
                side,
                ..
            } = &e.kind
            {
                if let Some(u) = w.entities.get(shooter) {
                    releases.push(json!({"tick":e.tick,"side":side,"shooter":shooter,"class":u.class,"kind":u.kind,
                        "altitude_m":u.altitude_m,"pos":u.pos,"weapon":weapon,"count":count,"mission":u.mission}));
                }
            }
        }
        let mut new_events: Vec<_> = w
            .events
            .iter()
            .rev()
            .take_while(|e| e.seq > seen_seq)
            .cloned()
            .collect();
        new_events.reverse();
        flight_audit.sample(&w,&new_events,&audit_memory);
        all_events.extend(new_events);
        seen_seq = w.event_seq;
        if w.tick % 1800 == 0 {
            eprintln!(
                "seed {seed}: tick {} / {max_ticks}, {} entities, {} events, {:.1}s",
                w.tick,
                w.entities.len(),
                w.events.len(),
                start.elapsed().as_secs_f64()
            );
        }
        if w.tick % 300 == 0 {
            let missiles = w.entities.values().filter(|e| e.kind == harpoon_core::types::PlatformKind::Missile).count();
            eprintln!(
                "DIAG tick={} wall={:.0}s entities={} missiles={} pending={} resolved={} adj={} delayed={} cas={} seqs={}",
                w.tick, start.elapsed().as_secs_f64(), w.entities.len(), missiles,
                w.rules.pending.len(), w.rules.resolved_hits.len(),
                w.rules.pending_adjudications.len(), w.rules.delayed_orders.len(),
                w.rules.casualties.values().map(|v|v.len()).sum::<usize>(),
                w.rules.event_sequences.len()
            );
        }
        if w.tick % 600 == 0 || w.over {
            snapshots.push(json!({"tick":w.tick,"sides":[observe(&w,Side::Blue,w.event_seq,0),observe(&w,Side::Red,w.event_seq,0)]}));
        }
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut timeline = Vec::new();
    for ev in &all_events {
        let v = serde_json::to_value(ev).unwrap();
        let kind = v["event"].as_str().unwrap_or("unknown");
        let side = v["side"].as_str().unwrap_or("public");
        let detail = match kind {
            "weapons_release" | "sam_shot" => v["weapon"].as_str().unwrap_or(""),
            "air_launch" => v["mission"].as_str().unwrap_or(""),
            _ => "",
        };
        *counts.entry(format!("{side}/{kind}/{detail}")).or_default() += 1;
        if matches!(
            kind,
            "sunk" | "air_shot_down" | "scenario_ended" | "air_launch"
        ) {
            timeline.push(v);
        }
    }
    let state = serde_json::to_vec(&json!({"blue":observe(&w,Side::Blue,0,usize::MAX),"red":observe(&w,Side::Red,0,usize::MAX)})).unwrap();
    let digest = format!(
        "{:016x}",
        state
            .iter()
            .fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64)
                .wrapping_mul(0x100000001b3))
    );
    let full_state = serde_json::to_vec(&w).unwrap();
    let world_digest = format!(
        "{:016x}",
        full_state
            .iter()
            .fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64)
                .wrapping_mul(0x100000001b3))
    );
    let mut result = json!({"seed":seed,"tick":w.tick,"result":w.result,"observation_fnv64":digest,"world_fnv64":world_digest,"sea_position_checks":sea_position_checks,"prealert_commands":prealert_commands,"command_windows":command_windows,"initial_units":initial_units,"exercise_state":w.context.island_exercise,"wall_seconds":start.elapsed().as_secs_f64(),"counts":counts,"timeline":timeline,"snapshots":snapshots,"events":all_events,"releases":releases,"air_trace":air_trace,"terminal_entries":terminal_entries,"controllers":staffs});
    result["flight_audit"]=serde_json::to_value(&flight_audit).unwrap();
    println!(
        "{}",
        json!({"seed":seed,"tick":w.tick,"result":w.result,"observation_fnv64":digest,"world_fnv64":world_digest,"counts":counts,"wall_seconds":start.elapsed().as_secs_f64()})
    );
    if let Some(path) = args.get(2) {
        std::fs::write(path, serde_json::to_vec(&result).unwrap()).unwrap();
    }
}

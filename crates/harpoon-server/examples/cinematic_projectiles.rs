//! Postgame referee-only projectile associations. Replays without changing rules.
//! Usage: cinematic_projectiles <completed-save.json> <new-output.json>
use harpoon_core::{Command, Event, Side, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Deserialize)]
struct Input {
    tick: i64,
    commands: Vec<(Side, Command)>,
    injected: Vec<Event>,
}
#[derive(Deserialize)]
struct Recording {
    initial: World,
    inputs: Vec<Input>,
}
fn digest<T: Serialize>(x: &T) -> String {
    let h = serde_json::to_vec(x)
        .unwrap()
        .iter()
        .fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ *b as u64).wrapping_mul(0x100000001b3)
        });
    format!("{h:016x}")
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let save: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let expected: World = serde_json::from_value(save["payload"]["world"].clone()).unwrap();
    assert!(expected.over, "completed games only");
    let r: Recording = serde_json::from_value(save["payload"]["recording"].clone()).unwrap();
    let mut w = r.initial;
    let mut cursor = 0;
    let mut all_events = w.events.clone();
    let mut projectiles: BTreeMap<u32, Value> = BTreeMap::new();
    loop {
        for e in w
            .entities
            .values()
            .filter(|e| e.alive && e.projectile.is_some())
        {
            let d = e.projectile.as_ref().unwrap();
            let record = projectiles.entry(e.id.0).or_insert_with(|| {
                json!({
                    "id":e.id,"weapon":d.weapon,"kind":e.kind,"side":e.side,
                    "shooter":d.launched_by,"target":d.target,"first_tick":w.tick,
                    "initial_projectile":d,"first_pos":e.pos
                })
            });
            record["last_tick"] = json!(w.tick);
            record["last_pos"] = json!(e.pos);
            record["last_projectile"] = json!(d);
        }
        if w.tick >= expected.tick || w.over {
            break;
        }
        let seq = w.event_seq;
        let cap = w.event_cap;
        w.event_cap = usize::MAX;
        if let Some(input) = r.inputs.get(cursor).filter(|i| i.tick == w.tick) {
            for e in &input.injected {
                assert_eq!(e.seq, w.event_seq + 1);
                w.emit(e.kind.clone(), e.vis);
            }
            harpoon_core::sim::tick(&mut w, &input.commands);
            cursor += 1;
        } else {
            harpoon_core::sim::tick(&mut w, &[]);
        }
        all_events.extend(w.events.iter().filter(|e| e.seq > seq).cloned());
        w.event_cap = cap;
        if w.events.len() > cap {
            w.events.drain(0..w.events.len() - cap);
        }
    }
    assert_eq!(digest(&w), digest(&expected), "full World must match");
    let expected_events: Vec<Event> =
        serde_json::from_value(save["payload"]["recording"]["events"].clone()).unwrap();
    assert_eq!(
        digest(&all_events),
        digest(&expected_events),
        "all original events must match"
    );
    let output = json!({"format":"harpoon-director-projectiles-v1","world_fnv64":digest(&w),
        "events":all_events.len(),"commands":cursor,"source_save":args[1],
        "note":"Referee-only associations. No player data or engine changes. Terminal tick is last live tick + 1; use original events to prove outcome.",
        "projectiles":projectiles});
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])
        .unwrap();
    serde_json::to_writer(&mut file, &output).unwrap();
    println!(
        "{} projectiles; {} events; World {}",
        projectiles.len(),
        all_events.len(),
        digest(&w)
    );
}

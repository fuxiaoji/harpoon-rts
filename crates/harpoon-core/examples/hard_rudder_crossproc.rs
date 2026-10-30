//! Hard-rudder cross-process acceptance: phase-1 creates a world with a real
//! natural jam and saves at three checkpoints; phase-2 (separate process)
//! restores each save, replays the same command stream, and compares the
//! complete serialized World byte-for-byte against an in-process continuation.
use harpoon_core::{Command, Database, Scenario, Side};
use serde_json::json;

fn die(seed: u64, purpose: &str) -> u8 {
    let mut rng = harpoon_core::Rng::new(seed).derive(harpoon_core::Rng::hash_tag(0, 1, purpose));
    1 + (rng.next_f64() * 100.) as u8
}
fn identity() -> String {
    "3.1.4.hard:1:1".into()
}
fn seed_for(test: impl Fn(u64) -> bool) -> u64 {
    (1..=2_000_000)
        .find(|seed| test(*seed))
        .expect("pre-World native seed exists")
}

fn fixture(seed: u64) -> harpoon_core::World {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"hard-rudder-crossproc","name":"cross-process hard-rudder acceptance",
        "seed":seed,"duration_s":20000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "navigation_2026":{"source":"scenario_assumption: hard-rudder cross-process mechanism geometry",
                "units":{"SHIP":{"source":"scenario_assumption: explicit mechanism fit","size":"F",
                    "merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,"undamaged_max_kts":30,
                    "propulsion":"NonSteam","readiness":"Underway"}}}},
        "units":[{"side":"Blue","class":"us_burke_2a","name":"SHIP","pos":[0,0],"course_deg":0,"speed_kts":30,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"RETAINER","pos":[500,500],"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    for e in w.entities.values_mut() {
        e.sensors.clear();
        e.fire_on = false;
    }
    w
}

fn save(w: &harpoon_core::World, path: &str) {
    let bytes = serde_json::to_vec(w).unwrap();
    std::fs::write(path, &bytes).unwrap();
}
fn load(path: &str) -> harpoon_core::World {
    let bytes = std::fs::read(path).unwrap();
    let w: harpoon_core::World = serde_json::from_slice(&bytes).unwrap();
    w.rules.validate_tables().unwrap();
    w
}

fn orders() -> Vec<(Side, Command)> {
    vec![(
        Side::Blue,
        Command::Navigation {
            unit: harpoon_core::EntityId(1),
            action: serde_json::from_value(
                json!({"type":"hard_rudder","course_deg":45,"direction":"starboard"}),
            )
            .unwrap(),
        },
    )]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = std::env::var("HARD_RUDDER_DIR").unwrap_or_else(|_| ".".into());
    match args.get(1).map(String::as_str) {
        Some("phase1") => {
            let seed = seed_for(|seed| {
                die(seed, &format!("{}:risk", identity())) == 5
                    && die(seed, &format!("{}:free:0", identity())) > 25
                    && die(seed, &format!("{}:free:1", identity())) <= 25
            });
            let mut w = fixture(seed);
            std::fs::write(format!("{dir}/seed.txt"), format!("{seed}")).unwrap();
            // Order hard rudder with explicit automatic recovery authorization,
            // then stop at pre-Resolution T1, mid-circle T179, and T181 (the
            // first authorized Resolution has consumed closed window0).
            harpoon_core::sim::tick(
                &mut w,
                &[(
                    Side::Blue,
                    Command::Navigation {
                        unit: harpoon_core::EntityId(1),
                        action: serde_json::from_value(
                            json!({"type":"rudder_recovery","automatic":true}),
                        )
                        .unwrap(),
                    },
                )],
            );
            harpoon_core::sim::tick(&mut w, &orders());
            for _ in 1..1 {
                harpoon_core::sim::tick(&mut w, &[]);
            }
            save(&w, &format!("{dir}/save-t1.json"));
            while w.tick < 179 {
                harpoon_core::sim::tick(&mut w, &[]);
            }
            save(&w, &format!("{dir}/save-t179.json"));
            while w.tick < 181 {
                harpoon_core::sim::tick(&mut w, &[]);
            }
            save(&w, &format!("{dir}/save-t181.json"));
            // Continue in-process to T200 for the replay reference.
            while w.tick < 365 {
                harpoon_core::sim::tick(&mut w, &[]);
            }
            save(&w, &format!("{dir}/reference-t365.json"));
            println!("phase1 complete: seed={seed} tick={}", w.tick);
        }
        Some("phase2") => {
            for checkpoint in ["t1", "t179", "t181"] {
                let mut restored = load(&format!("{dir}/save-{checkpoint}.json"));
                while restored.tick < 365 {
                    harpoon_core::sim::tick(&mut restored, &[]);
                }
                let reference =
                    std::fs::read_to_string(format!("{dir}/reference-t365.json")).unwrap();
                let replayed = serde_json::to_string(&restored).unwrap();
                assert_eq!(
                    replayed, reference,
                    "{checkpoint} cross-process continuation must reproduce the complete World"
                );
                let id = harpoon_core::EntityId(1);
                let hard = &restored.rules.navigation.as_ref().unwrap().units[&id].hard_rudder;
                let ep = &hard.episodes[&identity()];
                assert_eq!(ep.risk.d100, 5, "original risk die survives the round trip");
                assert_eq!(ep.risk.jammed, true);
                assert_eq!(
                    ep.phase, "freed",
                    "the authorized second-window25 roll was selected to succeed"
                );
                assert_eq!(
                    ep.checks.len(),
                    2,
                    "closed windows0 and1 each consumed one authorized slot"
                );
                assert!(restored.entity(id).unwrap().alive);
                assert!(restored.entity(id).unwrap().alive);
                println!("{checkpoint}: complete World reproduced, original dice preserved");
            }
            println!("phase2 complete");
        }
        _ => panic!("usage: hard_rudder_crossproc phase1|phase2"),
    }
}

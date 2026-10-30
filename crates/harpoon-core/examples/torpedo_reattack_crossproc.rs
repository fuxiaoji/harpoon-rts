//! 10.2.7 reattack cross-process acceptance: phase-1 runs the full miss ->
//! Past -> circle -> reacquisition -> second-attack chain saving three
//! checkpoints; phase-2 (a separate process) restores each save, replays to
//! the terminal state, and compares the complete serialized World against the
//! in-process continuation.
use harpoon_core::{load_scenario, Command, ContactId, Database, Scenario, Side, Vec2};
use serde_json::json;

const TORP: u32 = 3;

fn roll(seed: u64, entity: u32, purpose: &str, sides: u8) -> u8 {
    let mut r =
        harpoon_core::Rng::new(seed).derive(harpoon_core::Rng::hash_tag(0, entity, purpose));
    1 + (r.next_f64() * sides as f64) as u8
}
fn d100(seed: u64, purpose: &str) -> u8 {
    roll(seed, TORP, purpose, 100)
}
fn d10(seed: u64, purpose: &str) -> u8 {
    roll(seed, 0, purpose, 10)
}
fn seed_for(test: impl Fn(u64) -> bool) -> u64 {
    (1..=2_000_000)
        .find(|seed| test(*seed))
        .expect("pre-World native seed exists")
}

fn fixture(seed: u64) -> harpoon_core::World {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"torpedo-reattack","name":"10.2.7 reattack chain","duration_s":3000,"seed":seed,
        "context":{"rules_profile":"hv2026-rts-v4","torpedo_2026":{"source":"scenario_assumption: reattack chain mechanisms",
        "environment":{"source":"scenario_assumption: open water no layer","layer_m":null,"reverberant":false},
        "targets":{"cn_093":{"source":"scenario_assumption: controlled target","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
        "platforms":{"us_virginia":{"source":"scenario_assumption: loaded tubes","wire_capacity":2,"reload_parallel":2,
            "tubes":[{"bearing_deg":0,"loaded":"us_mk48"},{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}},
        "orders":{"SHOOTER:us_mk48":{"source":"scenario_assumption: declared order","speed_index":0,"depth_m":50,
            "enable_run_nm":0,"bearing_deg":null,"cut_wire":true,"spread_deg":[],
            "solution":{"quality":"Good","source":"scenario_assumption: fixture solution"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,0.5],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut w = load_scenario(sc, db).unwrap();
    let s = w
        .entities
        .values()
        .find(|e| e.name == "SHOOTER")
        .unwrap()
        .id;
    let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    w.entity_mut(t).unwrap().weapons.clear();
    w.sides[0].contacts.insert(
        ContactId(99),
        harpoon_core::world::Contact {
            id: ContactId(99),
            about: t,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 0.5),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: 0,
            stale: false,
            sensors: vec![],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert("0:2".into(), harpoon_core::types::PlatformKind::Sub);
    w
}

fn save(w: &harpoon_core::World, dir: &str, name: &str) {
    std::fs::write(format!("{dir}/{name}"), serde_json::to_vec(w).unwrap()).unwrap();
}
fn load(dir: &str, name: &str) -> harpoon_core::World {
    let bytes = std::fs::read(format!("{dir}/{name}")).unwrap();
    let w: harpoon_core::World = serde_json::from_slice(&bytes).unwrap();
    w.rules.validate_tables().unwrap();
    w
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = std::env::var("REATTACK_DIR").unwrap_or_else(|_| ".".into());
    match args.get(1).map(String::as_str) {
        Some("phase1") => {
            let seed = seed_for(|seed| {
                d10(seed, "10.placement:3:0") <= 7
                    && d100(seed, "10.2.5.attack:0") > 80
                    && d10(seed, "10.placement:3:1") <= 7
            });
            let mut w = fixture(seed);
            std::fs::write(format!("{dir}/seed.txt"), format!("{seed}")).unwrap();
            harpoon_core::sim::tick(
                &mut w,
                &[(
                    Side::Blue,
                    Command::Fire {
                        unit: harpoon_core::EntityId(1),
                        contact: ContactId(99),
                        weapon: "us_mk48".into(),
                        rounds: 1,
                    },
                )],
            );
            // Save on the initial run (before the first attack), during Past,
            // and on the circle before the reacquisition.
            let in_past = |w: &harpoon_core::World| {
                matches!(
                    &w.rules.torpedoes.as_ref().unwrap().flights[&harpoon_core::EntityId(TORP)]
                        .phase,
                    harpoon_core::rules::torpedo2026::Phase::Past { .. }
                )
            };
            let on_circle = |w: &harpoon_core::World| {
                matches!(
                    &w.rules.torpedoes.as_ref().unwrap().flights[&harpoon_core::EntityId(TORP)]
                        .phase,
                    harpoon_core::rules::torpedo2026::Phase::Search { .. }
                )
            };
            while !in_past(&w) && !w.over {
                harpoon_core::sim::tick(&mut w, &[]);
            }
            save(&w, &dir, "save-past.json");
            while !on_circle(&w) && !w.over {
                harpoon_core::sim::tick(&mut w, &[]);
            }
            save(&w, &dir, "save-circle.json");
            let mut reacquired = false;
            while !reacquired && !w.over {
                harpoon_core::sim::tick(&mut w, &[]);
                reacquired = w.events.iter().any(|e| {
                    matches!(&e.kind, harpoon_core::EventKind::RulesEvent { rule, detail, .. }
                        if rule == "10.2.4.placement" && detail.contains("\"pass\":1"))
                });
            }
            save(&w, &dir, "save-reacquired.json");
            let mut refw = load(&dir, "save-reacquired.json");
            while !refw.over
                && refw
                    .rules
                    .torpedoes
                    .as_ref()
                    .unwrap()
                    .flights
                    .contains_key(&harpoon_core::EntityId(TORP))
            {
                harpoon_core::sim::tick(&mut refw, &[]);
            }
            // run to over for a fully settled terminal state
            while !refw.over {
                harpoon_core::sim::tick(&mut refw, &[]);
            }
            save(&refw, &dir, "reference-terminal.json");
            println!("phase1 complete: seed={seed} terminal tick={}", refw.tick);
        }
        Some("phase2") => {
            let reference_bytes = std::fs::read(format!("{dir}/reference-terminal.json")).unwrap();
            for checkpoint in ["save-past.json", "save-circle.json", "save-reacquired.json"] {
                let mut restored = load(&dir, checkpoint);
                while !restored.over {
                    harpoon_core::sim::tick(&mut restored, &[]);
                }
                assert_eq!(
                    serde_json::to_vec(&restored).unwrap(),
                    reference_bytes,
                    "{checkpoint} cross-process continuation must reproduce the terminal World"
                );
                let attacks: Vec<_> = restored
                    .events
                    .iter()
                    .filter(|e| matches!(&e.kind, harpoon_core::EventKind::RulesEvent { rule, .. } if rule == "10.2.5.source_terminal"))
                    .collect();
                assert!(attacks.len() >= 2, "{checkpoint} must contain both attacks");
                println!(
                    "{checkpoint}: terminal World reproduced, {} attacks recorded",
                    attacks.len()
                );
            }
            println!("phase2 complete");
        }
        _ => panic!("usage: torpedo_reattack_crossproc phase1|phase2"),
    }
}

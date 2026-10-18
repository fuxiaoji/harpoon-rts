//! harpoon-core: a deterministic, real-time naval wargame engine inspired by
//! the classic Harpoon tabletop system, reworked for RTS play.
//!
//! Layering:
//! - `geom`/`rng`      — deterministic primitives (cross-platform trig, xoshiro)
//! - `db`/`db_data`    — data-driven platform/weapon/sensor database
//! - `world`           — authoritative mutable state (entities, contacts, decks)
//! - `movement`/`sensors`/`weapons`/`damage`/`air` — subsystems over `&mut World`
//! - `sim`             — the tick loop + command processing + victory
//! - `observe`         — per-side projection (double-blind contract)
//!
//! Everything is serde-serializable; a replay is
//! `(scenario JSON, seed, command log)` and reproduces the world exactly.

pub mod air;
pub mod commands;
pub mod db;
pub mod db_data;
pub mod damage;
pub mod events;
pub mod geom;
pub mod movement;
pub mod observe;
pub mod rules;
pub mod rng;
pub mod scenario;
pub mod sensors;
pub mod sim;
pub mod types;
pub mod world;
pub mod weapons;

pub use commands::{AirMission, Command};
pub use db::Database;
pub use events::{Event, EventKind, Visibility};
pub use geom::Vec2;
pub use observe::{observe, Observation};
pub use rng::Rng;
pub use scenario::{load_scenario, scenario_from_json, Scenario, UnitSpawn};
pub use types::{ContactId, EntityId, Side};
pub use world::World;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DayNight;

    fn mini_scenario() -> Scenario {
        Scenario {
            id: "mini".into(),
            name: "Mini".into(),
            description: String::new(),
            duration_s: 600,
            weather: world::Weather { visibility_nm: 12.0, jam: [0.0, 0.0], overcast: 0.0, rain: 0.0 },
            day_night: DayNight::Day,
            seed: 20251017,
            victory: Default::default(),
            units: vec![
                UnitSpawn {
                    side: Side::Blue,
                    class: "ddg_aegis".into(),
                    name: Some("BLUE DDG".into()),
                    pos: Vec2::new(0.0, 0.0),
                    course_deg: 0.0,
                    speed_kts: 18.0,
                    depth_m: None,
                    altitude_m: None,
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: true,
                },
                UnitSpawn {
                    side: Side::Red,
                    class: "mgb".into(),
                    name: Some("RED MGB".into()),
                    pos: Vec2::new(30.0, 10.0),
                    course_deg: 270.0,
                    speed_kts: 24.0,
                    depth_m: None,
                    altitude_m: None,
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: true,
                },
            ],
        }
    }

    #[test]
    fn determinism_same_seed_same_outcome() {
        let hash = |seed_shift: u64| -> (f64, i64) {
            let mut sc = mini_scenario();
            sc.seed += seed_shift; // 0 both times
            let mut w = load_scenario(sc, Database::built_in()).unwrap();
            let cmds = vec![(Side::Red, Command::Fire {
                unit: crate::types::EntityId(2),
                contact: crate::types::ContactId(1),
                weapon: "ascm_std".into(),
                rounds: 1,
            })];
            // drive red toward blue until in range, fire when tracked
            let mut fired = false;
            for t in 0..3600 {
                if !fired && t == 60 {
                    sim::tick(&mut w, &cmds);
                    fired = true;
                    continue;
                }
                sim::tick(&mut w, &[]);
            }
            let blue = w.entities.values().find(|e| e.side == Side::Blue).unwrap();
            (blue.hp, w.tick)
        };
        let a = hash(0);
        let b = hash(0);
        assert_eq!(a, b, "same seed must reproduce exactly");
    }

    #[test]
    fn observation_never_leaks_enemy_truth() {
        let mut w = load_scenario(mini_scenario(), Database::built_in()).unwrap();
        for _ in 0..300 {
            sim::tick(&mut w, &[]);
        }
        let obs = observe::observe(&w, Side::Blue, 0, 1000);
        // Blue units present, ids are own; contact positions must not equal
        // the red unit's exact position unless quality == 1.0.
        let red_pos = w.entities.values().find(|e| e.side == Side::Red).unwrap().pos;
        for c in &obs.contacts {
            if c.quality < 0.99 {
                assert!(
                    c.estimate.range_to(red_pos) > 1e-6 || c.quality >= 0.99,
                    "contact estimate leaked truth at quality {}",
                    c.quality
                );
            }
        }
        // No enemy unit ids in units list.
        assert!(obs.units.iter().all(|u| u.side == Side::Blue));
    }
}

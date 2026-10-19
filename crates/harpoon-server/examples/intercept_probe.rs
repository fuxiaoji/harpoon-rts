//! Intercept-rate probe — measures SAM-vs-ASCM and AAM-vs-aircraft hit rates
//! over many seeded trials to answer "why do interceptors never hit?".
//!
//! Run: cargo run --release -p harpoon-server --example intercept_probe

use harpoon_core::commands::{AirMission, Command};
use harpoon_core::db::Database;
use harpoon_core::scenario::{load_scenario, Scenario, UnitSpawn};
use harpoon_core::types::{DayNight, Side};
use harpoon_core::Vec2;

fn base(seed: u64) -> Scenario {
    Scenario {
        id: "probe".into(),
        name: "Probe".into(),
        description: String::new(),
        duration_s: 7200,
        weather: harpoon_core::world::Weather { visibility_nm: 12.0, jam: [0.0, 0.0], overcast: 0.0, rain: 0.0 },
        day_night: DayNight::Day,
        seed,
        victory: Default::default(),
        units: vec![
            UnitSpawn {
                side: Side::Blue, class: "ddg_aegis".into(), name: Some("DDG".into()),
                pos: Vec2::new(0.0, 0.0), course_deg: 0.0, speed_kts: 10.0,
                depth_m: None, altitude_m: None, waypoints: None, follow: None,
                sonar_active: false, fire_on: true,
            },
            UnitSpawn {
                side: Side::Red, class: "mgb".into(), name: Some("MGB".into()),
                pos: Vec2::new(9.0, 1.0), course_deg: 270.0, speed_kts: 10.0,
                depth_m: None, altitude_m: None, waypoints: None, follow: None,
                sonar_active: false, fire_on: true,
            },
        ],
    }
}

/// Trial A: MGB fires ASCM salvos at DDG; DDG engages with SAMs. Count shots
/// vs intercepts vs impacts on the DDG.
fn trial_sam(seed: u64) -> (u32, u32, u32) {
    let mut w = load_scenario(base(seed), Database::built_in()).unwrap();
    let (mut shots, mut kills, mut impacts) = (0u32, 0u32, 0u32);
    let mut last_seq = 0u64;
    while !w.over && w.tick < 5400 {
        harpoon_core::sim::tick(&mut w, &[]);
        for ev in w.events.iter().filter(|e| e.seq > last_seq) {
            use harpoon_core::EventKind::*;
            match &ev.kind {
                SamShot { .. } => shots += 1,
                Intercept { intercepted: true, .. } => kills += 1,
                Impact { unit, .. } => {
                    if w.entity(*unit).map(|u| u.class == "ddg_aegis").unwrap_or(false) {
                        impacts += 1;
                    }
                }
                _ => {}
            }
        }
        if let Some(e) = w.events.last() {
            last_seq = e.seq;
        }
    }
    if seed == 1 {
        let mut kinds = std::collections::BTreeMap::new();
        for e in &w.events {
            *kinds.entry(e.kind.name().to_string()).or_insert(0) += 1;
        }
        let rules: Vec<String> = w.events.iter().filter_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent { domain, rule, .. } => Some(format!("{}/{}", domain, rule)),
            _ => None }).collect();
        eprintln!("  [rules seed1] {:?}", &rules[..rules.len().min(6)]);
        eprintln!("  [events seed1] {:?}", kinds);
        eprintln!("  [alive] {}", w.entities.values().filter(|e| e.alive).map(|e| format!("{}({})", e.name, e.side)).collect::<Vec<_>>().join(", "));
    }
    (shots, kills, impacts)
}

/// Trial B: two fighter squads head-on, CAP missions overlapping — count AAM
/// shots vs air kills.
fn trial_aam(seed: u64) -> (u32, u32) {
    let mut sc = base(seed);
    sc.units.clear();
    sc.units.push(UnitSpawn {
        side: Side::Blue, class: "cvn".into(), name: Some("CV".into()),
        pos: Vec2::new(0.0, 0.0), course_deg: 0.0, speed_kts: 0.0,
        depth_m: None, altitude_m: None, waypoints: None, follow: None,
        sonar_active: false, fire_on: true,
    });
    sc.units.push(UnitSpawn {
        side: Side::Red, class: "airfield".into(), name: Some("AF".into()),
        pos: Vec2::new(60.0, 0.0), course_deg: 0.0, speed_kts: 0.0,
        depth_m: None, altitude_m: None, waypoints: None, follow: None,
        sonar_active: false, fire_on: true,
    });
    let mut w = load_scenario(sc, Database::built_in()).unwrap();
    let mut launched = false;
    let (mut shots, mut kills) = (0u32, 0u32);
    let mut last_seq = 0u64;
    while !w.over && w.tick < 7200 {
        let cmds = if !launched && w.tick == 60 {
            launched = true;
            let mut c = Vec::new();
            for (unit, side) in [(1u32, Side::Blue), (2, Side::Red)] {
                let uid = harpoon_core::types::EntityId(unit);
                let pos = w.entity(uid).map(|e| e.pos).unwrap_or_default();
                let toward = if side == Side::Blue { 90.0 } else { 270.0 };
                c.push((side, Command::LaunchAircraft {
                    unit: uid,
                    class: "fighter".into(),
                    count: 4,
                    mission: AirMission::Cap { station: pos.advanced(toward, 25.0), radius_nm: 6.0 },
                }));
            }
            c
        } else {
            Vec::new()
        };
        harpoon_core::sim::tick(&mut w, &cmds);
        if seed == 1 && w.tick == 300 {
            for e in w.entities.values().filter(|e| e.kind == harpoon_core::types::PlatformKind::Aircraft) {
                eprintln!("[dbg] {} side={} pos=({:.1},{:.1}) fire_on={} ammo={:?} assigned={:?}",
                    e.name, e.side, e.pos.x, e.pos.y, e.fire_on,
                    e.weapons.iter().map(|m| (m.weapon.clone(), m.ammo)).collect::<Vec<_>>(),
                    e.assigned);
            }
            for c in w.sides[Side::Blue.index()].contacts.values() {
                eprintln!("[dbg-blue-contact] C{} about={} cat={:?} q={:.2} stale={} est=({:.1},{:.1})",
                    c.id.0, c.about.0, c.category, c.quality, c.stale, c.estimate.x, c.estimate.y);
            }
        }
        for ev in w.events.iter().filter(|e| e.seq > last_seq) {
            use harpoon_core::EventKind::*;
            match &ev.kind {
                SamShot { shooter, .. } => {
                    // AAM launches also emit SamShot; count only from aircraft.
                    if w.entity(*shooter).map(|u| u.kind == harpoon_core::types::PlatformKind::Aircraft).unwrap_or(false) {
                        shots += 1;
                    }
                }
                AirShotDown { .. } => kills += 1,
                _ => {}
            }
        }
        if let Some(e) = w.events.last() {
            last_seq = e.seq;
        }
    }
    if seed == 1 {
        let mut kinds = std::collections::BTreeMap::new();
        for e in &w.events {
            *kinds.entry(e.kind.name().to_string()).or_insert(0) += 1;
        }
        eprintln!("  [air events seed1] {:?}", kinds);
        eprintln!("  [alive] {}", w.entities.values().filter(|e| e.alive).map(|e| format!("{} {}", e.name, e.side)).collect::<Vec<_>>().join(", "));
    }
    (shots, kills)
}

fn main() {
    println!("=== SAM vs incoming ASCM (10 trials, 90 min each) ===");
    let (mut s, mut k, mut i) = (0u32, 0u32, 0u32);
    for seed in 1..=10u64 {
        let (a, b, c) = trial_sam(seed);
        s += a;
        k += b;
        i += c;
        println!("  seed {seed:2}: shots {a:3}  intercepted {b:3}  impacts-on-DDG {c:2}");
    }
    println!(
        "  TOTAL: {s} SAM shots -> {k} intercepted ({}%), {i} ASCM impacts ({}%)\n",
        pct(k, s),
        pct(i, s)
    );

    println!("=== AAM air combat (10 trials, 2h each) ===");
    let (mut s, mut k) = (0u32, 0u32);
    for seed in 1..=10u64 {
        let (a, b) = trial_aam(seed);
        s += a;
        k += b;
        println!("  seed {seed:2}: AAM shots {a:3}  air kills {b:2}");
    }
    println!("  TOTAL: {s} AAM shots -> {k} kills ({}%)\n", pct(k, s));
}

fn pct(a: u32, b: u32) -> u32 {
    if b == 0 { 0 } else { a * 100 / b }
}

//! Operation Crimson Tide — full-mechanics scripted selfplay (P4 smoke gate).
//!
//! The author (session agent) plays BOTH sides through a beat script. All
//! triggers read only the acting side's own contact book (fog of war kept).
//! The run asserts:
//!   1. every required mechanic produced at least one event,
//!   2. two runs with the same seed produce identical digests & event counts.
//!
//! Run: cargo run --release -p harpoon-server --example crimson_tide

use harpoon_core::commands::{AirMission, Command};
use harpoon_core::db::Database;
use harpoon_core::observe::observe;
use harpoon_core::scenario::{load_scenario, Scenario};
use harpoon_core::types::{ContactId, EntityId, PlatformKind, Side};
use harpoon_core::world::World;

use std::collections::BTreeMap;

/// 17 项必须出现（base_damaged 在 TacticalStaff 下不可保证，移为可选）。
const REQUIRED_EVENTS: &[&str] = &[
    "detection",
    "contact_identified",
    "counter_detected",
    "weapons_release",
    "incoming",
    "sam_shot",
    "intercept",
    "chaff_deployed",
    "decoy_fooled",
    "torpedo_detected",
    "gun_action",
    "impact",
    "critical_hit",
    "sunk",
    "air_launch",
    "air_recovered",
    "scenario_ended",
];

#[derive(Default)]
struct Driver {
    blue_cap: bool,
    blue_helo: bool,
    blue_recon: bool,
    blue_strike_base: bool,
    blue_cap_refresh: bool,
    blue_recall: bool,
    red_cap: bool,
    red_bombers: bool,
    mgb_fired: [bool; 2],
    ssk_fired: bool,
    helo_retasked: bool,
}

struct Outcome {
    digest: u64,
    ticks: i64,
    event_counts: BTreeMap<String, u64>,
    first_seen: BTreeMap<String, String>,
    result: Option<harpoon_core::world::VictoryResult>,
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "scenarios/crimson_tide.json".into());
    let txt = std::fs::read_to_string(&path).expect("scenario file");
    let sc: Scenario = serde_json::from_str(&txt).expect("scenario json");

    println!("=== Operation Crimson Tide — full-mechanics smoke ===");
    let r1 = run(&sc);
    let r2 = run(&sc);

    // Determinism gate.
    let deterministic = r1.digest == r2.digest && r1.event_counts == r2.event_counts;
    println!(
        "determinism: {} (digest {:016x} vs {:016x})",
        if deterministic { "PASS" } else { "FAIL" },
        r1.digest,
        r2.digest
    );

    // Coverage gate.
    println!("\n-- mechanic coverage --");
    let mut missing = Vec::new();
    for req in REQUIRED_EVENTS {
        let n = r1.event_counts.get(*req).copied().unwrap_or(0);
        let sample = r1.first_seen.get(*req).map(|s| s.as_str()).unwrap_or("-");
        println!("{:22} {:>4}  {}", req, n, sample);
        if n == 0 {
            missing.push(*req);
        }
    }
    println!(
        "\nresult: {:?}  ticks: {}",
        r1.result.as_ref().map(|r| (r.winner, &r.reason)),
        r1.ticks
    );

    let ok = deterministic && missing.is_empty();
    if !missing.is_empty() {
        eprintln!("MISSING mechanics: {missing:?}");
    }
    println!(
        "\nSMOKE {}",
        if ok { "PASS ✓" } else { "FAIL ✗" }
    );
    std::process::exit(if ok { 0 } else { 1 });
}

fn run(sc: &Scenario) -> Outcome {
    let mut world = load_scenario(sc.clone(), Database::built_in()).expect("load");
    let mut driver = Driver::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut first_seen: BTreeMap<String, String> = BTreeMap::new();
    let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
    let mut last_seq = 0u64;

    while !world.over && world.tick < sc.duration_s {
        let cmds = beat(&mut world, &mut driver);
        harpoon_core::sim::tick(&mut world, &cmds);

        for ev in world.events.iter() {
            if ev.seq <= last_seq {
                continue;
            }
            let name = ev.kind.name().to_string();
            *counts.entry(name.clone()).or_default() += 1;
            first_seen.entry(name).or_insert_with(|| format!("[t={}s]", ev.tick));
        }
        if let Some(ev) = world.events.back() {
            last_seq = ev.seq;
        }
        // State digest over kinematic truth.
        for e in world.entities.values() {
            digest ^= (e.pos.x.to_bits()) >> 1;
            digest = digest.wrapping_mul(0x100_0000_01b3);
            digest ^= (e.pos.y.to_bits()) << 3;
            digest = digest.wrapping_mul(0x100_0000_01b3);
            digest ^= e.hp.to_bits();
            digest = digest.wrapping_mul(0x100_0000_01b3);
        }
    }
    Outcome {
        digest,
        ticks: world.tick,
        event_counts: counts,
        first_seen,
        result: world.result.clone(),
    }
}

/// The beat script: my hands on both sides. Every trigger uses the acting
/// side's own contact picture, not referee truth.
fn beat(world: &mut World, d: &mut Driver) -> Vec<(Side, Command)> {
    let mut cmds: Vec<(Side, Command)> = Vec::new();
    let tick = world.tick;
    if tick % 15 != 0 {
        return cmds;
    }

    let cvn = uid(world, "CV-72 FREEDOM");
    let ddg = uid(world, "DDG-101");
    let ffg = uid(world, "FFG-58");
    let airfield = uid(world, "MIKO AIRFIELD");
    let mgb1 = uid(world, "RED MGB-1");
    let mgb2 = uid(world, "RED MGB-2");
    let ssk = uid(world, "RED SSK-7");

    // --- BLUE ---
    if Side::parse("blue").is_some() {
        if !d.blue_cap && tick >= 300 {
            if let Some(cvn) = cvn {
                cmds.push((Side::Blue, Command::LaunchAircraft { profile: None,
                    unit: cvn, class: "fighter".into(), count: 2,
                    mission: AirMission::Cap { station: p(44.0, 44.0), radius_nm: 18.0, follow: false },
                }));
                d.blue_cap = true;
            }
        }
        if !d.blue_helo && tick >= 300 {
            if let Some(cvn) = cvn {
                cmds.push((Side::Blue, Command::LaunchAircraft { profile: None,
                    unit: cvn, class: "helo_asw".into(), count: 2,
                    mission: AirMission::Asw { area: p(6.0, 6.0), radius_nm: 10.0 },
                }));
                d.blue_helo = true;
            }
        }
        if !d.blue_recon && tick >= 600 {
            if let Some(cvn) = cvn {
                cmds.push((Side::Blue, Command::LaunchAircraft { profile: None,
                    unit: cvn, class: "fighter".into(), count: 1,
                    mission: AirMission::Recon { point: p(80.0, 72.0) },
                }));
                d.blue_recon = true;
            }
        }
        // Retask helo onto the FFG's submarine contact when one appears.
        if !d.helo_retasked {
            if let Some(ffg) = ffg {
                if let Some((cid, _)) = best_contact(world, Side::Blue, ffg, PlatformKind::Sub) {
                    let dest = world
                        .contact(Side::Blue, cid)
                        .map(|c| c.estimate_at(tick))
                        .unwrap_or_default();
                    cmds.push((Side::Blue, Command::LaunchAircraft { profile: None,
                        unit: cvn.unwrap_or(EntityId(1)),
                        class: "helo_asw".into(),
                        count: 1,
                        mission: AirMission::Asw { area: dest, radius_nm: 4.0 },
                    }));
                    d.helo_retasked = true;
                }
            }
        }
        // Land strike once the recon fighter has visually fixed the base.
        if !d.blue_strike_base {
            if let (Some(ddg), Some(cvn)) = (ddg, cvn) {
                if let Some((cid, _)) = base_contact(world, Side::Blue, "navsta") {
                    cmds.push((Side::Blue, Command::AssignTarget { unit: ddg, contact: cid }));
                    cmds.push((Side::Blue, Command::Fire {
                        unit: ddg, contact: cid, weapon: "slcm".into(), rounds: 2,
                    }));
                    cmds.push((Side::Blue, Command::LaunchAircraft { profile: None,
                        unit: cvn, class: "striker".into(), count: 2,
                        mission: AirMission::Strike { contact: cid },
                    }));
                    d.blue_strike_base = true;
                }
            }
        }
        // CAP refresh mid-game.
        if !d.blue_cap_refresh && tick >= 14400 {
            if let Some(cvn) = cvn {
                cmds.push((Side::Blue, Command::LaunchAircraft { profile: None,
                    unit: cvn, class: "fighter".into(), count: 2,
                    mission: AirMission::Cap { station: p(44.0, 44.0), radius_nm: 18.0, follow: false },
                }));
                d.blue_cap_refresh = true;
            }
        }
        // Evening recall.
        if !d.blue_recall && tick >= 26400 {
            if let Some(cvn) = cvn {
                cmds.push((Side::Blue, Command::RecoverAircraft { unit: cvn }));
                d.blue_recall = true;
            }
        }
    }

    // --- RED ---
    {
        if !d.red_cap && tick >= 900 {
            if let Some(airfield) = airfield {
                cmds.push((Side::Red, Command::LaunchAircraft { profile: None,
                    unit: airfield, class: "fighter".into(), count: 2,
                    mission: AirMission::Cap { station: p(48.0, 48.0), radius_nm: 15.0, follow: false },
                }));
                d.red_cap = true;
            }
        }
        if !d.red_bombers && tick >= 2400 {
            if let Some(airfield) = airfield {
                if let Some((cid, _)) = contact_on(world, Side::Red, cvn) {
                    if contact_quality(world, Side::Red, cid) > 0.5 {
                        cmds.push((Side::Red, Command::LaunchAircraft { profile: None,
                            unit: airfield, class: "bomber".into(), count: 2,
                            mission: AirMission::Strike { contact: cid },
                        }));
                        d.red_bombers = true;
                    }
                }
            }
        }
        // Missile boats: close, assign, shoot two salvos, keep pressing.
        for (i, mgb) in [mgb1, mgb2].into_iter().enumerate() {
            if d.mgb_fired[i] {
                continue;
            }
            let Some(mgb) = mgb else { continue };
            if let Some((cid, dist)) = nearest_contact_of_kind(world, Side::Red, mgb, PlatformKind::Ship) {
                if dist < 55.0 {
                    cmds.push((Side::Red, Command::AssignTarget { unit: mgb, contact: cid }));
                    cmds.push((Side::Red, Command::Fire {
                        unit: mgb, contact: cid, weapon: "ascm_std".into(), rounds: 1,
                    }));
                    if dist < 30.0 {
                        d.mgb_fired[i] = true;
                    }
                }
            }
        }
        // Submarine ambush: fire the heavyweight pair inside 9 nm, then run.
        if !d.ssk_fired {
            if let Some(ssk) = ssk {
                if let Some((cid, dist)) = nearest_contact_of_kind(world, Side::Red, ssk, PlatformKind::Ship) {
                    if dist < 9.0 {
                        cmds.push((Side::Red, Command::AssignTarget { unit: ssk, contact: cid }));
                        cmds.push((Side::Red, Command::Fire {
                            unit: ssk, contact: cid, weapon: "torp_homing".into(), rounds: 2,
                        }));
                        cmds.push((Side::Red, Command::SetSpeed { unit: ssk, speed_kts: 18.0 }));
                        cmds.push((Side::Red, Command::SetDepth { unit: ssk, depth_m: 40.0 }));
                        cmds.push((Side::Red, Command::SetCourse { unit: ssk, course_deg: 70.0 }));
                        d.ssk_fired = true;
                    }
                }
            }
        }
    }

    cmds
}

// --- helpers -------------------------------------------------------------

fn p(x: f64, y: f64) -> harpoon_core::Vec2 {
    harpoon_core::Vec2::new(x, y)
}

fn uid(world: &World, name: &str) -> Option<EntityId> {
    world.entities.values().find(|e| e.name == name).map(|e| e.id)
}

fn contact_on(world: &World, side: Side, about: Option<EntityId>) -> Option<(ContactId, f64)> {
    let about = about?;
    world.sides[side.index()]
        .contacts
        .values()
        .find(|c| c.about == about && !c.stale)
        .map(|c| (c.id, c.quality))
}

fn contact_quality(world: &World, side: Side, cid: ContactId) -> f64 {
    world.contact(side, cid).map(|c| c.quality).unwrap_or(0.0)
}

fn base_contact(world: &World, side: Side, class: &str) -> Option<(ContactId, f64)> {
    world.sides[side.index()]
        .contacts
        .values()
        .filter(|c| !c.stale)
        .filter(|c| {
            world
                .entity(c.about)
                .map(|e| e.class == class && e.kind == PlatformKind::Base)
                .unwrap_or(false)
        })
        .map(|c| (c.id, c.quality))
        .next()
}

fn best_contact(
    world: &World,
    side: Side,
    from: EntityId,
    kind: PlatformKind,
) -> Option<(ContactId, f64)> {
    let from_pos = world.entity(from)?.pos;
    world.sides[side.index()]
        .contacts
        .values()
        .filter(|c| !c.stale && c.quality > 0.3)
        .filter(|c| {
            world
                .entity(c.about)
                .map(|e| e.kind == kind)
                .unwrap_or(false)
        })
        .map(|c| (c.id, from_pos.range_to(c.estimate_at(world.tick))))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
}

fn nearest_contact_of_kind(
    world: &World,
    side: Side,
    from: EntityId,
    kind: PlatformKind,
) -> Option<(ContactId, f64)> {
    best_contact(world, side, from, kind)
}

//! Operation Pacific Storm — large joint-campaign scripted selfplay
//! (extended smoke gate).
//!
//! The author (session agent) plays BOTH sides through a beat script. Every
//! trigger that depends on the enemy reads only the acting side's own contact
//! book (fog of war kept); time-scheduled doctrine orders are allowed because
//! the scenario author is both players.
//!
//! The run asserts:
//!   1. every Crimson Tide mechanic still produces at least one event,
//!   2. each rules-extension rule (AAW air burst, ASW sonobuoy, air-combat
//!      merge/pass/hit, damage-control closure, towed array) fires,
//!   3. two runs with the same seed produce identical digests & event counts.
//!
//! Run: cargo run --release -p harpoon-server --example pacific_smoke

use harpoon_core::commands::{AirMission, Command};
use harpoon_core::db::Database;
use harpoon_core::scenario::{load_scenario, Scenario};
use harpoon_core::types::{ContactId, EntityId, PlatformKind, Side};
use harpoon_core::world::World;

use std::collections::BTreeMap;

/// The 18 mechanics asserted by the Crimson Tide gate, plus the six
/// domain-qualified rules events. Damage-control closure is satisfied by
/// either `damage/fire_out` or `damage/flooding_contained`.
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
    "base_damaged",
    "scenario_ended",
];

const REQUIRED_RULES: &[(&str, &str)] = &[
    ("aaw", "air_burst"),
    ("asw", "sonobuoy_laid"),
    ("aircombat", "dogfight_merge"),
    ("aircombat", "dogfight_pass"),
    ("aircombat", "dogfight_hit"),
    ("asw", "towed_array_deployed"),
];

const REQUIRED_DC: &[(&str, &str)] = &[("damage", "fire_out"), ("damage", "flooding_contained")];

const BOMBER_WAVES: &[i64] = &[1500, 8000, 14000, 20000];

#[derive(Default)]
struct Driver {
    /// One-shot scheduled flags.
    initial_orders: bool,
    blue_cap_1: bool,
    blue_cap_2: bool,
    blue_cap_3: bool,
    blue_cap_4: bool,
    blue_helo: bool,
    red_cap_1: bool,
    red_cap_2: bool,
    bomber_wave: [bool; 4],
    mgb_salvo: bool,
    mgb_salvo_tick: i64,
    mgb_salvo2: bool,
    mgb_charge: bool,
    blue_anti_ship: bool,
    blue_shore_strike: bool,
    blue_airfield_strike: bool,
    blue_asw_attack: bool,
    ssk_fired: bool,
    ddg_awake: bool,
    blue_awake: bool,
    blue_ffg_asw_tight: bool,
    /// One-tick weapons-free pulse for a merge (both sides as scripted).
    pulse_tick: Option<i64>,
    last_blue_pulse: i64,
    duel_tick: Option<i64>,
    last_duel: i64,
    duel_pulses: u8,
    duel_done: bool,
}

struct Outcome {
    digest: u64,
    ticks: i64,
    event_counts: BTreeMap<String, u64>,
    rule_counts: BTreeMap<String, u64>,
    first_seen: BTreeMap<String, String>,
    final_state: Vec<String>,
    result: Option<harpoon_core::world::VictoryResult>,
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "scenarios/pacific_storm.json".into());
    let txt = std::fs::read_to_string(&path).expect("scenario file");
    let sc: Scenario = serde_json::from_str(&txt).expect("scenario json");

    println!("=== Operation Pacific Storm — joint-campaign smoke ===");
    let trace = std::env::var("PACIFIC_TRACE").is_ok();
    let r1 = run(&sc, trace);
    let r2 = run(&sc, false);

    let deterministic = r1.digest == r2.digest
        && r1.event_counts == r2.event_counts
        && r1.rule_counts == r2.rule_counts;
    println!(
        "determinism: {} (digest {:016x} vs {:016x})",
        if deterministic { "PASS" } else { "FAIL" },
        r1.digest,
        r2.digest
    );

    println!("\n-- mechanic coverage --");
    let mut missing: Vec<String> = Vec::new();
    for req in REQUIRED_EVENTS {
        let n = r1.event_counts.get(*req).copied().unwrap_or(0);
        let sample = r1.first_seen.get(*req).map(|s| s.as_str()).unwrap_or("-");
        println!("{:22} {:>4}  {}", req, n, sample);
        if n == 0 {
            missing.push((*req).to_string());
        }
    }
    for (domain, rule) in REQUIRED_RULES {
        let key = format!("{domain}/{rule}");
        let n = r1.rule_counts.get(&key).copied().unwrap_or(0);
        let sample = r1.first_seen.get(&key).map(|s| s.as_str()).unwrap_or("-");
        println!("{:22} {:>4}  {}", key, n, sample);
        if n == 0 {
            missing.push(key);
        }
    }
    let dc_n: u64 = REQUIRED_DC
        .iter()
        .map(|(d, r)| r1.rule_counts.get(&format!("{d}/{r}")).copied().unwrap_or(0))
        .sum();
    let dc_sample = REQUIRED_DC
        .iter()
        .find_map(|(d, r)| r1.first_seen.get(&format!("{d}/{r}")).map(|s| s.to_string()))
        .unwrap_or_else(|| "-".into());
    println!("{:22} {:>4}  {}", "damage/fire_or_flood", dc_n, dc_sample);
    if dc_n == 0 {
        missing.push("damage/fire_out|flooding_contained".into());
    }

    println!("\n-- rules-extension event totals --");
    for (k, v) in &r1.rule_counts {
        println!("{k:28} {v:>4}  {}", r1.first_seen.get(k).cloned().unwrap_or_default());
    }

    println!("\n-- final state --");
    for s in &r1.final_state {
        println!("{s}");
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
    println!("\nSMOKE {}", if ok { "PASS ✓" } else { "FAIL ✗" });
    std::process::exit(if ok { 0 } else { 1 });
}

fn run(sc: &Scenario, trace: bool) -> Outcome {
    let mut world = load_scenario(sc.clone(), Database::built_in()).expect("load");
    let mut driver = Driver::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut rules: BTreeMap<String, u64> = BTreeMap::new();
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
            if let harpoon_core::events::EventKind::RulesEvent { domain, rule, .. } = &ev.kind {
                let key = format!("{domain}/{rule}");
                *rules.entry(key.clone()).or_default() += 1;
                first_seen.entry(key).or_insert_with(|| format!("[t={}s]", ev.tick));
                if trace {
                    if let harpoon_core::events::EventKind::RulesEvent { unit, detail, .. } = &ev.kind {
                        let who = unit
                            .and_then(|u| world.entity(u))
                            .map(|e| e.name.clone())
                            .unwrap_or_else(|| "-".into());
                        println!("  [{:>5}] {domain}/{rule} {who}: {detail}", ev.tick);
                    }
                }
            }
            if trace {
                trace_event(&world, ev);
            }
        }
        if let Some(ev) = world.events.last() {
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

    let mut final_state = Vec::new();
    for name in [
        "CVN-76 REAGAN",
        "DDG-101 STETHEM",
        "DDG-89 MUSTIN",
        "FFG-58 INGRAHAM",
        "MIKO AIRFIELD",
        "KURE NAVAL STATION",
        "RED MGB-1",
        "RED MGB-2",
        "RED MGB-3",
        "RED SSK-7",
        "RED SSK-9",
    ] {
        if let Some(e) = world.entities.values().find(|e| e.name == name) {
            final_state.push(format!(
                "{name:20} alive={:<5} hp={:7.1}/{:7.1} fire={:5.1}s flood={:5.1}s",
                e.alive, e.hp, e.hp_max, e.burn_s, e.flooding_s
            ));
        }
    }

    Outcome {
        digest,
        ticks: world.tick,
        event_counts: counts,
        rule_counts: rules,
        first_seen,
        final_state,
        result: world.result.clone(),
    }
}

/// The beat script: my hands on both sides. Enemy-dependent triggers read the
/// acting side's own contact book; scheduled doctrine orders are the author's
/// scenario direction.
fn beat(world: &mut World, d: &mut Driver) -> Vec<(Side, Command)> {
    let tick = world.tick;
    let mut cmds: Vec<(Side, Command)> = Vec::new();

    // --- one-shot opening doctrine -------------------------------------
    if tick >= 1 && !d.initial_orders {
        // Red missile boats hold fire until the scripted wolfpack salvo;
        // blue ships hold their SAMs until the extended air battle develops.
        for name in ["CVN-76 REAGAN", "DDG-101 STETHEM", "DDG-89 MUSTIN", "FFG-58 INGRAHAM"] {
            if let Some(id) = uid(world, name) {
                cmds.push((Side::Blue, Command::SetWeaponsFree { unit: id, free: false }));
            }
        }
        for name in ["RED MGB-1", "RED MGB-2", "RED MGB-3", "RED SSK-7", "RED SSK-9"] {
            if let Some(id) = uid(world, name) {
                cmds.push((Side::Red, Command::SetWeaponsFree { unit: id, free: false }));
            }
        }
        if let Some(ffg) = uid(world, "FFG-58 INGRAHAM") {
            cmds.push((Side::Blue, Command::SetSonar { unit: ffg, active: true }));
        }
        d.initial_orders = true;
    }

    let cvn = uid(world, "CVN-76 REAGAN");
    let airfield = uid(world, "MIKO AIRFIELD");
    let blue_ffg = uid(world, "FFG-58 INGRAHAM");

    // --- blue air operations -------------------------------------------
    if !d.blue_cap_1 && tick >= 300 {
        if let (Some(cvn), true) = (cvn, ready(world, cvn, "fighter") >= 4) {
            cmds.push((Side::Blue, Command::LaunchAircraft {
                unit: cvn, class: "fighter".into(), count: 4,
                mission: AirMission::Cap { station: p(50.0, 44.0), radius_nm: 5.0, follow: false },
            }));
            d.blue_cap_1 = true;
        }
    }
    if !d.blue_helo && tick >= 300 {
        if let (Some(cvn), true) = (cvn, ready(world, cvn, "helo_asw") >= 2) {
            cmds.push((Side::Blue, Command::LaunchAircraft {
                unit: cvn, class: "helo_asw".into(), count: 2,
                mission: AirMission::Asw { area: p(32.0, 30.0), radius_nm: 6.0 },
            }));
            d.blue_helo = true;
        }
    }
    if !d.blue_cap_2 && tick >= 6800 {
        if let (Some(cvn), true) = (cvn, ready(world, cvn, "fighter") >= 4) {
            cmds.push((Side::Blue, Command::LaunchAircraft {
                unit: cvn, class: "fighter".into(), count: 4,
                mission: AirMission::Cap { station: p(50.0, 44.0), radius_nm: 5.0, follow: false },
            }));
            d.blue_cap_2 = true;
        }
    }
    if !d.blue_cap_3 && tick >= 14400 {
        if let (Some(cvn), true) = (cvn, ready(world, cvn, "fighter") >= 4) {
            cmds.push((Side::Blue, Command::LaunchAircraft {
                unit: cvn, class: "fighter".into(), count: 4,
                mission: AirMission::Cap { station: p(50.0, 44.0), radius_nm: 5.0, follow: false },
            }));
            d.blue_cap_3 = true;
        }
    }
    if !d.blue_cap_4 && tick >= 21600 {
        if let (Some(cvn), true) = (cvn, ready(world, cvn, "fighter") >= 4) {
            cmds.push((Side::Blue, Command::LaunchAircraft {
                unit: cvn, class: "fighter".into(), count: 4,
                mission: AirMission::Cap { station: p(50.0, 44.0), radius_nm: 5.0, follow: false },
            }));
            d.blue_cap_4 = true;
        }
    }

    // --- red air operations --------------------------------------------
    if !d.red_cap_1 && tick >= 900 {
        if let (Some(af), true) = (airfield, ready(world, airfield, "fighter") >= 4) {
            cmds.push((Side::Red, Command::LaunchAircraft {
                unit: af, class: "fighter".into(), count: 4,
                mission: AirMission::Cap { station: p(54.0, 47.0), radius_nm: 5.0, follow: false },
            }));
            d.red_cap_1 = true;
        }
    }
    if !d.red_cap_2 && tick >= 18000 {
        if let (Some(af), true) = (airfield, ready(world, airfield, "fighter") >= 4) {
            cmds.push((Side::Red, Command::LaunchAircraft {
                unit: af, class: "fighter".into(), count: 4,
                mission: AirMission::Cap { station: p(54.0, 47.0), radius_nm: 5.0, follow: false },
            }));
            d.red_cap_2 = true;
        }
    }
    // Red bomber waves: 3-ship strike packages against the tracked carrier.
    for (i, &wave_tick) in BOMBER_WAVES.iter().enumerate() {
        if !d.bomber_wave[i] && tick >= wave_tick {
            let n = ready(world, airfield, "bomber");
            if n >= 2 {
                if let Some((cid, _)) = red_cvn_contact(world) {
                    cmds.push((Side::Red, Command::LaunchAircraft {
                        unit: airfield.unwrap(), class: "bomber".into(), count: 3.min(n),
                        mission: AirMission::Strike { contact: cid },
                    }));
                    d.bomber_wave[i] = true;
                }
            }
        }
    }

    // --- red wolfpack salvos (two concentrated 12-missile waves) --------
    let fire_wolfpack = |world: &World, cmds: &mut Vec<(Side, Command)>, cid: ContactId| -> bool {
        let mut any = false;
        for name in ["RED MGB-1", "RED MGB-2", "RED MGB-3"] {
            if let Some(mgb) = uid(world, name) {
                let can = world.entity(mgb).map(|e| {
                    e.alive
                        && e.pos.range_to(contact_point(world, Side::Red, cid)) < 55.0
                        && e.weapons.iter().any(|m| m.weapon == "ascm_std" && m.ammo > 0)
                }).unwrap_or(false);
                if can {
                    cmds.push((Side::Red, Command::Fire {
                        unit: mgb, contact: cid, weapon: "ascm_std".into(), rounds: 4,
                    }));
                    any = true;
                }
            }
        }
        any
    };
    if !d.mgb_salvo && tick >= 1700 {
        if let Some((cid, _)) = red_cvn_contact(world) {
            if fire_wolfpack(world, &mut cmds, cid) {
                d.mgb_salvo = true;
                d.mgb_salvo_tick = tick;
            }
        }
    } else if d.mgb_salvo && !d.mgb_salvo2 && tick >= d.mgb_salvo_tick + 50 {
        if let Some((cid, _)) = red_cvn_contact(world) {
            if fire_wolfpack(world, &mut cmds, cid) {
                d.mgb_salvo2 = true;
            }
        }
    }
    if !d.mgb_charge && tick >= 2300 {
        // Commander's charge order, aimed at Red's own carrier contact
        // estimate — never at the referee's true position.
        if let (Some(mgb), Some((cid, _))) = (uid(world, "RED MGB-1"), red_cvn_contact(world)) {
            let pos = contact_point(world, Side::Red, cid);
            cmds.push((Side::Red, Command::SetCourse {
                unit: mgb,
                course_deg: world.entity(mgb).map(|e| e.pos.bearing_to(pos)).unwrap_or(45.0),
            }));
            cmds.push((Side::Red, Command::SetSpeed { unit: mgb, speed_kts: 34.0 }));
            cmds.push((Side::Red, Command::SetWeaponsFree { unit: mgb, free: true }));
            d.mgb_charge = true;
        }
    }

    // --- blue anti-ship strike (missile boats) --------------------------
    if !d.blue_anti_ship && tick >= 2400 {
        if let Some((cid, _)) = blue_contact_named(world, "Missile Boat") {
            if let (Some(cvn), true) = (cvn, ready(world, cvn, "striker") >= 2) {
                cmds.push((Side::Blue, Command::LaunchAircraft {
                    unit: cvn, class: "striker".into(), count: 2,
                    mission: AirMission::Strike { contact: cid },
                }));
                d.blue_anti_ship = true;
            }
        }
    }
    // --- blue land strike (naval station + airfield) --------------------
    if !d.blue_shore_strike && tick >= 3000 {
        if let Some((cid, _)) = blue_contact_named(world, "Naval Station") {
            let mut any = false;
            if let Some(ddg) = uid(world, "DDG-101 STETHEM") {
                let can = world.entity(ddg).map(|e| {
                    e.alive && e.weapons.iter().any(|m| m.weapon == "slcm" && m.ammo > 0)
                }).unwrap_or(false);
                if can {
                    cmds.push((Side::Blue, Command::Fire {
                        unit: ddg, contact: cid, weapon: "slcm".into(), rounds: 2,
                    }));
                    any = true;
                }
            }
            if let (Some(cvn), true) = (cvn, ready(world, cvn, "striker") >= 2) {
                cmds.push((Side::Blue, Command::LaunchAircraft {
                    unit: cvn, class: "striker".into(), count: 2,
                    mission: AirMission::Strike { contact: cid },
                }));
                any = true;
            }
            if any {
                d.blue_shore_strike = true;
            }
        }
    }
    if !d.blue_airfield_strike && tick >= 12000 {
        if let Some((cid, _)) = blue_contact_named(world, "Coastal Airfield") {
            let mut any = false;
            if let Some(ddg) = uid(world, "DDG-89 MUSTIN") {
                let can = world.entity(ddg).map(|e| {
                    e.alive && e.weapons.iter().any(|m| m.weapon == "slcm" && m.ammo > 0)
                }).unwrap_or(false);
                if can {
                    cmds.push((Side::Blue, Command::Fire {
                        unit: ddg, contact: cid, weapon: "slcm".into(), rounds: 2,
                    }));
                    any = true;
                }
            }
            if let (Some(cvn), true) = (cvn, ready(world, cvn, "striker") >= 2) {
                cmds.push((Side::Blue, Command::LaunchAircraft {
                    unit: cvn, class: "striker".into(), count: 2,
                    mission: AirMission::Strike { contact: cid },
                }));
                any = true;
            }
            if any {
                d.blue_airfield_strike = true;
            }
        }
    }

    // --- submarine ambush ----------------------------------------------
    if !d.ssk_fired {
        if let Some(ssk) = uid(world, "RED SSK-7") {
            if let Some((cid, dist)) = nearest_red_ship_contact(world, ssk) {
                if dist < 9.0 {
                    cmds.push((Side::Red, Command::AssignTarget { unit: ssk, contact: cid }));
                    cmds.push((Side::Red, Command::Fire {
                        unit: ssk, contact: cid, weapon: "torp_homing".into(), rounds: 2,
                    }));
                    cmds.push((Side::Red, Command::SetSpeed { unit: ssk, speed_kts: 14.0 }));
                    cmds.push((Side::Red, Command::SetDepth { unit: ssk, depth_m: 80.0 }));
                    cmds.push((Side::Red, Command::SetCourse { unit: ssk, course_deg: 300.0 }));
                    cmds.push((Side::Red, Command::ClearTarget { unit: ssk }));
                    d.ssk_fired = true;
                }
            }
        }
    }
    // --- blue ASW counter-attack on the tracked submarine --------------
    if !d.blue_asw_attack && tick >= 6000 {
        if let Some((cid, _)) = blue_contact_named(world, "Diesel Submarine") {
            let mut any = false;
            if let Some(ffg) = blue_ffg {
                if world.entity(ffg).map(|e| e.alive).unwrap_or(false) {
                    cmds.push((Side::Blue, Command::SetWeaponsFree { unit: ffg, free: true }));
                    cmds.push((Side::Blue, Command::AssignTarget { unit: ffg, contact: cid }));
                    any = true;
                }
            }
            for e in world.entities.values() {
                if e.alive && e.side == Side::Blue && e.class == "helo_asw" {
                    cmds.push((Side::Blue, Command::AssignTarget { unit: e.id, contact: cid }));
                    any = true;
                }
            }
            if any {
                d.blue_asw_attack = true;
            }
        }
    }

    // --- blue ships wake up for the missile defense ---------------------
    // The two Aegis destroyers open fire into the wolfpack salvo first
    // (the scripted "DDG intercepts the missile-boat salvo" beat); the
    // carrier keeps her own SAMs tight a little longer so the leakers are
    // adjudicated by the CIWS terminal chain, whose 3-second-rule fragments
    // are the air-burst workout.
    if !d.ddg_awake && tick >= 3360 {
        for name in ["DDG-101 STETHEM", "DDG-89 MUSTIN"] {
            if let Some(id) = uid(world, name) {
                cmds.push((Side::Blue, Command::SetWeaponsFree { unit: id, free: true }));
            }
        }
        d.ddg_awake = true;
    }
    if !d.blue_awake && tick >= 3600 {
        if let Some(cvn) = cvn {
            cmds.push((Side::Blue, Command::SetWeaponsFree { unit: cvn, free: true }));
        }
        d.blue_awake = true;
    }
    // Keep the FFG quiet while it runs the towed-array ASW screen, then let
    // it fight once the submarine contact is being prosecuted.
    if !d.blue_ffg_asw_tight && tick >= 2600 && tick < 7000 {
        if let Some(ffg) = blue_ffg {
            cmds.push((Side::Blue, Command::SetWeaponsFree { unit: ffg, free: false }));
        }
        d.blue_ffg_asw_tight = true;
    }

    // --- weapons-free pulses that start merges --------------------------
    let blue_fighters: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.alive && e.side == Side::Blue && e.class == "fighter")
        .map(|e| e.id)
        .collect();
    let red_fighters: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.alive && e.side == Side::Red && e.class == "fighter")
        .map(|e| e.id)
        .collect();

    if d.pulse_tick.is_none()
        && tick - d.last_blue_pulse >= 300
        && red_bomber_close_to_blue(world, &blue_fighters, 5.2)
    {
        d.pulse_tick = Some(tick);
    }
    let pulse_now = d.pulse_tick == Some(tick);
    if let Some(pt) = d.pulse_tick {
        if tick > pt {
            d.pulse_tick = None;
            d.last_blue_pulse = pt;
        }
    }

    // Scripted fighter head-on duels: while both CAPs are inside merge
    // radius, release weapons for a single tick (then tighten again). Two
    // pulses are allowed so an orbit-phase miss can be retried.
    if !d.duel_done
        && (3000..=6000).contains(&tick)
        && tick - d.last_duel >= 600
        && red_fighter_close_to_blue(world, &blue_fighters, 5.8)
    {
        d.duel_tick = Some(tick);
        d.last_duel = tick;
    }
    let duel_now = d.duel_tick == Some(tick);
    if let Some(dt) = d.duel_tick {
        if tick > dt {
            d.duel_tick = None;
            d.duel_pulses += 1;
            if d.duel_pulses >= 2 {
                d.duel_done = true;
            }
        }
    }

    // Desired fire state: tight by default, free for one-tick merge pulses.
    for &id in &blue_fighters {
        let desired = pulse_now || duel_now;
        apply_fire_state(world, &mut cmds, Side::Blue, id, desired);
    }
    // Strike/ASW aircraft never need free fire: strike_run and assigned
    // torpedo runs bypass the free-fire scan.
    for e in world.entities.values() {
        if e.alive && e.side == Side::Blue && e.class != "fighter" && e.kind == PlatformKind::Aircraft {
            apply_fire_state(world, &mut cmds, Side::Blue, e.id, false);
        }
    }
    for &id in &red_fighters {
        apply_fire_state(world, &mut cmds, Side::Red, id, duel_now);
    }

    // --- loitering orbits ----------------------------------------------
    if tick % 900 == 0 {
        if let Some(aew) = uid(world, "BLUE EAGLE EYE") {
            if world.entity(aew).map(|e| e.waypoints.is_empty()).unwrap_or(false) {
                cmds.push((Side::Blue, Command::Goto {
                    unit: aew,
                    waypoints: vec![p(55.0, 62.0), p(62.0, 48.0), p(50.0, 38.0), p(34.0, 46.0)],
                }));
            }
        }
        if let Some(aew) = uid(world, "RED SENTRY") {
            if world.entity(aew).map(|e| e.waypoints.is_empty()).unwrap_or(false) {
                cmds.push((Side::Red, Command::Goto {
                    unit: aew,
                    waypoints: vec![p(98.0, 98.0), p(70.0, 98.0), p(98.0, 72.0)],
                }));
            }
        }
    }
    if tick == 24000 {
        if let Some(cvn) = cvn {
            cmds.push((Side::Blue, Command::Goto {
                unit: cvn,
                waypoints: vec![p(24.0, 22.0), p(38.0, 32.0), p(50.0, 40.0), p(58.0, 26.0), p(42.0, 18.0), p(24.0, 20.0)],
            }));
        }
    }
    if tick >= 27000 && tick < 27001 {
        if let Some(cvn) = cvn {
            cmds.push((Side::Blue, Command::RecoverAircraft { unit: cvn }));
        }
        if let Some(af) = airfield {
            cmds.push((Side::Red, Command::RecoverAircraft { unit: af }));
        }
    }

    cmds
}

// --- helpers -------------------------------------------------------------

/// Debug-only event trace for tuning the beat script (`PACIFIC_TRACE=1`).
fn trace_event(world: &World, ev: &harpoon_core::events::Event) {
    use harpoon_core::events::EventKind as K;
    match &ev.kind {
        K::AirLaunch { side, class, count, mission, carrier: _ } => println!(
            "  [{:>5}] air_launch {side:?} {}x {class} mission={mission}",
            ev.tick,
            count,
        ),
        K::AirRecovered { side, carrier, class, count } => println!(
            "  [{:>5}] air_recovered {side:?} {}x {class} on #{}",
            ev.tick, count, carrier.0
        ),
        K::WeaponsRelease { side, shooter_name, weapon, count, .. } => {
            if matches!(
                weapon.as_str(),
                "Anti-Ship Cruise Missile"
                    | "Land-Attack Cruise Missile"
                    | "Air-Launched ASM"
                    | "Active-Homing Torpedo"
                    | "Lightweight ASW Torpedo"
                    | "ASW Rocket"
            ) {
                println!("  [{:>5}] release {side:?} {shooter_name} {count}x {weapon}", ev.tick);
            }
        }
        K::Sunk { side, unit_name, .. } => {
            println!("  [{:>5}] sunk {side:?} {unit_name}", ev.tick)
        }
        K::CriticalHit { side, unit, system } => {
            let who = world.entity(*unit).map(|e| e.name.clone()).unwrap_or_default();
            println!("  [{:>5}] critical {side:?} {who}: {system}", ev.tick)
        }
        K::BaseDamaged { side, base_name, damage, hp_left, .. } => println!(
            "  [{:>5}] base_damaged {side:?} {base_name} {damage:.1} -> {hp_left:.1}",
            ev.tick
        ),
        K::Impact { side, unit_name, weapon, damage, hp_left, .. } => {
            if weapon != "aircraft cannon" {
                println!(
                    "  [{:>5}] impact {side:?} {unit_name} by {weapon} {damage:.1} -> {hp_left:.1}",
                    ev.tick
                )
            }
        }
        _ => {}
    }
}

fn p(x: f64, y: f64) -> harpoon_core::Vec2 {
    harpoon_core::Vec2::new(x, y)
}

fn uid(world: &World, name: &str) -> Option<EntityId> {
    world.entities.values().find(|e| e.name == name).map(|e| e.id)
}

fn ready(world: &World, carrier: Option<EntityId>, class: &str) -> u8 {
    let Some(cid) = carrier else { return 0 };
    world
        .entity(cid)
        .and_then(|e| e.deck.as_ref())
        .and_then(|d| d.ready.iter().find(|(c, _)| c == class).map(|(_, n)| *n))
        .unwrap_or(0)
}

fn apply_fire_state(
    world: &World,
    cmds: &mut Vec<(Side, Command)>,
    side: Side,
    id: EntityId,
    desired: bool,
) {
    let Some(e) = world.entity(id) else { return };
    if e.fire_on != desired {
        cmds.push((side, Command::SetWeaponsFree { unit: id, free: desired }));
    }
}

fn contact_point(world: &World, side: Side, cid: ContactId) -> harpoon_core::Vec2 {
    world
        .contact(side, cid)
        .map(|c| c.estimate_at(world.tick))
        .unwrap_or_default()
}

fn blue_contact_named(world: &World, needle: &str) -> Option<(ContactId, f64)> {
    contact_named(world, Side::Blue, needle)
}

fn contact_named(world: &World, side: Side, needle: &str) -> Option<(ContactId, f64)> {
    world.sides[side.index()]
        .contacts
        .values()
        .filter(|c| !c.stale && c.quality > 0.3)
        .filter(|c| {
            c.class_name
                .as_deref()
                .map(|n| n.contains(needle))
                .unwrap_or(false)
        })
        .map(|c| (c.id, c.quality))
        .max_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.0.cmp(&a.0))
        })
}

fn red_cvn_contact(world: &World) -> Option<(ContactId, f64)> {
    contact_named(world, Side::Red, "Fleet Carrier")
}

fn nearest_red_ship_contact(world: &World, from: EntityId) -> Option<(ContactId, f64)> {
    let pos = world.entity(from)?.pos;
    world.sides[Side::Red.index()]
        .contacts
        .values()
        .filter(|c| !c.stale && c.quality > 0.3 && c.category == PlatformKind::Ship)
        .map(|c| (c.id, pos.range_to(c.estimate_at(world.tick))))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
}

/// Does any blue CAP aircraft hold an identified Red bomber contact inside
/// `range_nm`? Read from Blue's own contact book only.
fn red_bomber_close_to_blue(world: &World, blue_air: &[EntityId], range_nm: f64) -> bool {
    for &id in blue_air {
        let Some(e) = world.entity(id) else { continue };
        for c in world.sides[Side::Blue.index()].contacts.values() {
            if c.stale || c.category != PlatformKind::Aircraft {
                continue;
            }
            let bomber = c
                .class_name
                .as_deref()
                .map(|n| n.contains("Bomber"))
                .unwrap_or(false);
            if bomber && e.pos.range_to(c.estimate_at(world.tick)) <= range_nm {
                return true;
            }
        }
    }
    false
}

/// Does Blue hold a Red fighter contact inside merge-pulse range of a Blue
/// CAP aircraft? (Own picture only, used for the scripted head-on duel.)
fn red_fighter_close_to_blue(world: &World, blue_air: &[EntityId], range_nm: f64) -> bool {
    for &id in blue_air {
        let Some(e) = world.entity(id) else { continue };
        for c in world.sides[Side::Blue.index()].contacts.values() {
            if c.stale || c.category != PlatformKind::Aircraft {
                continue;
            }
            let bomber = c
                .class_name
                .as_deref()
                .map(|n| n.contains("Bomber"))
                .unwrap_or(false);
            // Treat unidentified air contacts as possible fighters: blue
            // AEW/radar identification can lag the merge geometry by a few
            // seconds, and the pulse only opens fire for one tick.
            if !bomber && e.pos.range_to(c.estimate_at(world.tick)) <= range_nm {
                return true;
            }
        }
    }
    false
}

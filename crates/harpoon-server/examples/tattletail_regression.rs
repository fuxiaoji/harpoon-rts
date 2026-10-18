//! Tattletail (15 May 1991) — official-scenario regression driver.
//!
//! Source: `19.（典型想定）Tattletail_10_Aug_24.pdf`.
//! - Briefing / forces / orders / setup: PDF p.2.
//! - Ship cards: Copeland p.7, Emden p.3, Ariadne p.6, Van Kinsbergen p.4,
//!   Poolster p.5, Smyshlennyy p.8.
//!
//! IMPORTANT SOURCE FACT: the official Russian unit is **Smyshlennyy, a
//! Mod Kashin DDG**, not a submarine. The PDF's "ASW search operations"
//! sentence is background context; no submarine is present in the scenario.
//! The ask's illustrative "submarine contact by towed array" event therefore
//! has no referent in this PDF and is **not** asserted here. Instead the
//! regression asserts the facts the PDF does print and the engine can
//! reproduce: six ships, the printed diamond/aft geometry, radar-active /
//! sonar-passive initial state, both contact books established (double
//! blind), the 0300-0315 attack window, Poolster as the primary target,
//! NATO's self-defense release, and a deterministic two-run replay.
//!
//! Assertions (none are unconditional truth):
//! 1. Six ships exactly, with the official names/sides and the declared
//!    class mapping; no submarine.
//! 2. Initial geometry: one escort in each printed 90-degree arc at
//!    2,000-10,000 yards from Poolster (PDF p.2); Smyshlennyy on 090-270
//!    relative bearing, >= 2,000 yards from every escort and inside the
//!    mapped radar horizon; all course 000 / 12 kt.
//! 3. Initial state: every radar active (sensor present), every sonar
//!    passive, both sides weapons-tight (Condition III / peacetime ROE).
//! 4. By t=60 s both sides hold surface contacts with quality >= 0.30
//!    (the engine's fire-control gate standing in for the printed Good
//!    fire-control solution), detected by radar, and the per-side
//!    observation projection contains no enemy unit IDs (double blind).
//! 5. No Blue weapons release before the Soviet attack; Red's first
//!    anti-ship launch falls inside ticks 600-900 (0300-0315 at 1 s/tick);
//!    every Red missile is aimed at Poolster (the printed primary target).
//! 6. NATO releases in self-defense after the attack, fires anti-ship
//!    missiles back, and the incoming missiles are seen and engaged by SAMs
//!    (incoming + sam_shot + intercept events).
//! 7. No subsurface detection event occurs anywhere in the run (the PDF has
//!    no submarine in this scenario).
//! 8. Two runs with the same seed produce identical event/state digests,
//!    event counts and phase timelines.
//!
//! Engine gaps, declared rather than papered over: Condition III -> GQ D6
//! transitions, the referee's "any D6=6" NATO-release roll, Good/Fair/Poor
//! fire-control grades, the 5% visibility factor (`visibility_nm` is carried
//! but the current visual chain uses day/night + overcast/rain only), sea
//! state/wind/depth bands, and the official crippled-count victory
//! conditions are not modelled. The driver keeps both sides weapons-tight
//! until the scripted 03:10 Soviet launch and then frees the four NATO
//! escorts, standing in for the referee release.
//!
//! Run: cargo run --release -p harpoon-server --example tattletail_regression

use harpoon_core::commands::Command;
use harpoon_core::db::Database;
use harpoon_core::events::EventKind;
use harpoon_core::geom::horizon_nm;
use harpoon_core::observe::observe;
use harpoon_core::scenario::{load_scenario, Scenario};
use harpoon_core::types::{DayNight, EntityId, PlatformKind, Side};
use harpoon_core::world::{Entity, World};

use std::collections::BTreeMap;

/// 03:10 on 15 May 1991; the official attack window is 0300-0315 (PDF p.2),
/// so tick 600 is inside it and tick 900 is the deadline.
const ATTACK_TICK: i64 = 600;
const ATTACK_DEADLINE_TICK: i64 = 900;
/// Second four-missile salvo (P-15M launchers reload in 60 s in this fit).
const SECOND_SALVO_TICK: i64 = ATTACK_TICK + 60;
/// 2,000 and 10,000 yards converted to nautical miles.
const MIN_ESCORT_NM: f64 = 2_000.0 / 2_025.371_828_5;
const MAX_ESCORT_NM: f64 = 10_000.0 / 2_025.371_828_5;

const POOLSTER: &str = "POOLSTER (A 835)";
const SMYSHLENNYY: &str = "SMYSHLENNYY (644)";

/// Official force list (PDF p.2) with the declared built_in mapping.
const OFFICIAL: &[(&str, &str, Side)] = &[
    ("USS COPELAND (FFG-25)", "uk_type45", Side::Blue),
    ("EMDEN (F 210)", "uk_type23", Side::Blue),
    ("HMS ARIADNE (F 72)", "ffg", Side::Blue),
    ("VAN KINSBERGEN (F 809)", "uk_type23", Side::Blue),
    (POOLSTER, "uk_type26", Side::Blue),
    (SMYSHLENNYY, "ru_sovremenny", Side::Red),
];

const NATO_ESCORTS: &[&str] = &[
    "USS COPELAND (FFG-25)",
    "EMDEN (F 210)",
    "HMS ARIADNE (F 72)",
    "VAN KINSBERGEN (F 809)",
];

#[derive(Default)]
struct Driver {
    first_salvo: bool,
    second_salvo: bool,
}

#[derive(Default)]
struct Trace {
    event_counts: BTreeMap<String, u64>,
    rule_counts: BTreeMap<String, u64>,
    first_seen: BTreeMap<String, String>,
    detection_categories: BTreeMap<String, u64>,
    detection_sensors: BTreeMap<String, Vec<String>>,
    red_asm_launch_ticks: Vec<i64>,
    blue_asm_launch_ticks: Vec<i64>,
    blue_sam_shot_ticks: Vec<i64>,
    blue_intercept_ticks: Vec<i64>,
    blue_intercept_hits: u32,
    blue_incoming_ticks: Vec<i64>,
    /// Every Red `rasl_km` projectile seen, mapped to its referee-only target.
    red_missile_targets: BTreeMap<EntityId, Option<EntityId>>,
    scenario_end_tick: Option<i64>,
    contacts_at_60: [usize; 2],
    poolster_track_quality: Option<f64>,
    smyshlennyy_track_quality: Option<f64>,
    digest: u64,
    ticks: i64,
    final_state: Vec<String>,
}

impl Trace {
    fn new() -> Self {
        Self {
            digest: 0xcbf2_9ce4_8422_2325,
            ..Default::default()
        }
    }

    fn observe(&mut self, world: &World, last_seq: &mut u64) {
        for ev in world.events.iter() {
            if ev.seq <= *last_seq {
                continue;
            }
            *last_seq = ev.seq;
            let name = ev.kind.name().to_string();
            *self.event_counts.entry(name.clone()).or_default() += 1;
            self.first_seen
                .entry(name.clone())
                .or_insert_with(|| format!("[t={}s]", ev.tick));

            match &ev.kind {
                EventKind::Detection {
                    side,
                    category,
                    sensor,
                    ..
                } => {
                    *self
                        .detection_categories
                        .entry(category.clone())
                        .or_default() += 1;
                    self.detection_sensors
                        .entry(side.name().to_string())
                        .or_default()
                        .push(sensor.clone());
                }
                EventKind::WeaponsRelease {
                    side, weapon, ..
                } => {
                    if *side == Side::Red && weapon.contains("Supersonic") {
                        self.red_asm_launch_ticks.push(ev.tick);
                    }
                    if *side == Side::Blue && weapon.contains("Anti-Ship") {
                        self.blue_asm_launch_ticks.push(ev.tick);
                    }
                }
                EventKind::SamShot { side, .. } if *side == Side::Blue => {
                    self.blue_sam_shot_ticks.push(ev.tick)
                }
                EventKind::Intercept {
                    side, intercepted, ..
                } if *side == Side::Blue => {
                    self.blue_intercept_ticks.push(ev.tick);
                    if *intercepted {
                        self.blue_intercept_hits += 1;
                    }
                }
                EventKind::Incoming { side, .. } if *side == Side::Blue => {
                    self.blue_incoming_ticks.push(ev.tick)
                }
                EventKind::ScenarioEnded { .. } => self.scenario_end_tick = Some(ev.tick),
                EventKind::RulesEvent { domain, rule, .. } => {
                    *self
                        .rule_counts
                        .entry(format!("{domain}/{rule}"))
                        .or_default() += 1;
                }
                _ => {}
            }

            // Full event payload, including tick/visibility/ids, is hashed so
            // two runs must agree on the exact narrative, not just counts.
            match serde_json::to_string(ev) {
                Ok(s) => self.digest = hash_bytes(self.digest, s.as_bytes()),
                Err(_) => self.digest = hash_bytes(self.digest, name.as_bytes()),
            }
        }

        // Record missile truth targets while they exist (referee side only).
        for e in world
            .entities
            .values()
            .filter(|e| e.alive && e.side == Side::Red && e.kind == PlatformKind::Missile)
        {
            if let Some(p) = &e.projectile {
                if p.weapon == "rasl_km" {
                    self.red_missile_targets.entry(e.id).or_insert(p.target);
                }
            }
        }
    }
}

struct Run {
    trace: Trace,
    initial_failures: Vec<String>,
    contact_failures: Vec<String>,
    event_failures: Vec<String>,
    result: Option<harpoon_core::world::VictoryResult>,
}

impl Run {
    fn failures(&self) -> impl Iterator<Item = &String> {
        self.initial_failures
            .iter()
            .chain(self.contact_failures.iter())
            .chain(self.event_failures.iter())
    }
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "scenarios/tattletail.json".into());
    let txt = std::fs::read_to_string(&path).expect("scenario file");
    let sc: Scenario = serde_json::from_str(&txt).expect("scenario json");

    println!("=== Tattletail (15 May 1991) — official-scenario regression ===");
    println!(
        "scenario: {}  duration: {} s  seed: {}",
        sc.id, sc.duration_s, sc.seed
    );
    println!(
        "source: 19.（典型想定）Tattletail_10_Aug_24.pdf — briefing/setup p.2, ship cards pp.3-8"
    );

    let r1 = run(&sc);
    let r2 = run(&sc);

    print_report(&r1);

    let deterministic = r1.trace.digest == r2.trace.digest
        && r1.trace.event_counts == r2.trace.event_counts
        && r1.trace.rule_counts == r2.trace.rule_counts
        && r1.trace.red_asm_launch_ticks == r2.trace.red_asm_launch_ticks
        && r1.trace.blue_asm_launch_ticks == r2.trace.blue_asm_launch_ticks
        && r1.trace.blue_sam_shot_ticks == r2.trace.blue_sam_shot_ticks
        && r1.trace.blue_intercept_ticks == r2.trace.blue_intercept_ticks
        && r1.trace.red_missile_targets == r2.trace.red_missile_targets
        && r1.trace.contacts_at_60 == r2.trace.contacts_at_60
        && r1.trace.ticks == r2.trace.ticks
        && r1.trace.final_state == r2.trace.final_state;
    println!(
        "determinism ......... {}  (digest {:016x} vs {:016x}, ticks {})",
        mark(deterministic),
        r1.trace.digest,
        r2.trace.digest,
        r1.trace.ticks
    );

    let failure_count = r1.failures().count();
    let ok = deterministic && failure_count == 0;
    if failure_count > 0 {
        eprintln!("\nFAILURES:");
        for f in r1.failures() {
            eprintln!("  - {f}");
        }
    }
    println!(
        "\nRESULT {}",
        if ok { "PASS ✓" } else { "FAIL ✗" }
    );
    std::process::exit(if ok { 0 } else { 1 });
}

fn run(sc: &Scenario) -> Run {
    let mut world = load_scenario(sc.clone(), Database::built_in()).expect("load scenario");
    let mut trace = Trace::new();
    let initial_failures = check_initial(&world);
    let mut driver = Driver::default();
    let mut last_seq = 0u64;

    // Quiet period: build the initial radar picture (all radars active).
    for _ in 0..60 {
        harpoon_core::sim::tick(&mut world, &[]);
        trace.observe(&world, &mut last_seq);
    }
    for side in [Side::Blue, Side::Red] {
        trace.contacts_at_60[side.index()] = world.sides[side.index()]
            .contacts
            .values()
            .filter(|c| !c.stale)
            .count();
    }
    trace.poolster_track_quality = uid(&world, POOLSTER)
        .and_then(|id| world.find_contact(Side::Red, id))
        .and_then(|cid| world.contact(Side::Red, cid))
        .map(|c| c.quality);
    trace.smyshlennyy_track_quality = uid(&world, SMYSHLENNYY)
        .and_then(|id| world.find_contact(Side::Blue, id))
        .and_then(|cid| world.contact(Side::Blue, cid))
        .map(|c| c.quality);
    let contact_failures = check_contact_books(&world, &trace);

    while !world.over && world.tick < sc.duration_s {
        let cmds = beat(&world, &mut driver);
        harpoon_core::sim::tick(&mut world, &cmds);
        trace.observe(&world, &mut last_seq);
    }

    trace.ticks = world.tick;
    trace.final_state = final_state(&world);
    let event_failures = check_events(&world, &trace);
    let result = world.result.clone();

    Run {
        trace,
        initial_failures,
        contact_failures,
        event_failures,
        result,
    }
}

/// Scripted orders. Uses only the acting side's own contact book to select
/// targets; no enemy truth is read for the fire decision.
fn beat(world: &World, d: &mut Driver) -> Vec<(Side, Command)> {
    let mut cmds = Vec::new();
    let next_tick = world.tick + 1;
    let poolster = uid(world, POOLSTER);
    let smyshlennyy = uid(world, SMYSHLENNYY);

    // Red attack any time after 0300 and no later than 0315 (PDF p.2). The
    // driver picks 03:10 (tick 600) and fires the four P-15M in two salvos
    // at Poolster, the printed primary target (PDF p.2).
    if next_tick == ATTACK_TICK && !d.first_salvo {
        if let (Some(shooter), Some(target)) = (smyshlennyy, poolster) {
            if let Some(cid) = world.find_contact(Side::Red, target) {
                cmds.push((
                    Side::Red,
                    Command::Fire {
                        unit: shooter,
                        contact: cid,
                        weapon: "rasl_km".into(),
                        rounds: 2,
                    },
                ));
                d.first_salvo = true;
            }
        }
        // NATO is released for self/task-force defense once the attack is
        // detected (the referee D6 release is not modelled; stand-in here).
        if d.first_salvo {
            for name in NATO_ESCORTS {
                if let Some(id) = uid(world, name) {
                    cmds.push((Side::Blue, Command::SetWeaponsFree { unit: id, free: true }));
                }
            }
        }
    }

    if next_tick == SECOND_SALVO_TICK && d.first_salvo && !d.second_salvo {
        if let (Some(shooter), Some(target)) = (smyshlennyy, poolster) {
            if let Some(cid) = world.find_contact(Side::Red, target) {
                cmds.push((
                    Side::Red,
                    Command::Fire {
                        unit: shooter,
                        contact: cid,
                        weapon: "rasl_km".into(),
                        rounds: 2,
                    },
                ));
                d.second_salvo = true;
            }
        }
    }

    cmds
}

fn check_initial(world: &World) -> Vec<String> {
    let mut f: Vec<String> = Vec::new();
    let ships: Vec<&Entity> = world
        .entities
        .values()
        .filter(|e| e.kind == PlatformKind::Ship)
        .collect();

    if ships.len() != OFFICIAL.len() {
        f.push(format!(
            "expected {} ships (PDF p.2), found {}",
            OFFICIAL.len(),
            ships.len()
        ));
    }
    if world.entities.len() != OFFICIAL.len() {
        f.push(format!(
            "expected exactly {} units at start (no optional Ka-25), found {}",
            OFFICIAL.len(),
            world.entities.len()
        ));
    }
    if world
        .entities
        .values()
        .any(|e| e.alive && e.kind == PlatformKind::Sub)
    {
        f.push("submarine present: PDF p.2 has Smyshlennyy (DDG), no submarine".into());
    }
    if !world.sides_present[0] || !world.sides_present[1] {
        f.push("both sides must be present at scenario start".into());
    }
    if world.tick != 0 {
        f.push(format!("initial tick must be 0, got {}", world.tick));
    }

    for (name, class, side) in OFFICIAL {
        match world.entities.values().find(|e| e.name == *name) {
            None => f.push(format!("missing official unit '{name}'")),
            Some(e) => {
                if e.class != *class {
                    f.push(format!(
                        "{name}: class {} != declared mapping {}",
                        e.class, class
                    ));
                }
                if e.side != *side {
                    f.push(format!("{name}: side {} != {:?}", e.side, side));
                }
            }
        }
    }

    // PDF p.2 setup: course 000 / 12 kt, radars active, sonars passive,
    // Condition III (weapons tight), all units Good fire-control solution.
    for e in &ships {
        if e.course_deg.abs() > 1e-9 {
            f.push(format!("{}: initial course {:.3} != 000", e.name, e.course_deg));
        }
        if (e.speed_kts - 12.0).abs() > 1e-9 {
            f.push(format!("{}: initial speed {:.3} != 12 kt", e.name, e.speed_kts));
        }
        if e.depth_m.abs() > 1e-9 {
            f.push(format!("{}: surface ship has depth {}", e.name, e.depth_m));
        }
        if e.fire_on {
            f.push(format!("{}: weapons free at start (Condition III/ROE violated)", e.name));
        }
        if e.sensors.iter().filter(|s| s.sensor.starts_with("sonar")).any(|s| s.active) {
            f.push(format!("{}: an active sonar is pinging; PDF p.2 says all sonars passive", e.name));
        }
        if !e.sensors.iter().any(|s| s.sensor.starts_with("radar")) {
            f.push(format!("{}: no radar fitted; PDF p.2 says all radars active", e.name));
        }
    }

    // PDF p.2: one frigate in each 315-45 / 45-135 / 135-225 / 225-315 arc,
    // 2,000-10,000 yards from Poolster; the formation is centered on Poolster.
    if let Some(pool) = find(world, POOLSTER) {
        let mut arc_count = [0usize; 4];
        for name in NATO_ESCORTS {
            let Some(e) = find(world, name) else { continue };
            let range = pool.pos.range_to(e.pos);
            if !(MIN_ESCORT_NM..=MAX_ESCORT_NM).contains(&range) {
                f.push(format!(
                    "{name}: {range:.3} nm from Poolster is outside the printed 2,000-10,000 yd arc"
                ));
            }
            let bearing = pool.pos.bearing_to(e.pos);
            match escort_arc(bearing) {
                Some(i) => arc_count[i] += 1,
                None => f.push(format!(
                    "{name}: bearing {bearing:.1} deg is outside the four printed arcs"
                )),
            }
        }
        if arc_count != [1, 1, 1, 1] {
            f.push(format!(
                "escort arcs not one-per-sector: {arc_count:?} (PDF p.2)"
            ));
        }
    } else {
        f.push("missing Poolster (formation center, PDF p.2)".into());
    }

    // PDF p.2: Smyshlennyy is astern (090-270 relative), >= 2,000 yards from
    // every escort, and no farther than her radar horizon.
    if let (Some(pool), Some(sove)) = (find(world, POOLSTER), find(world, SMYSHLENNYY)) {
        let bearing = pool.pos.bearing_to(sove.pos);
        if !(90.0..=270.0).contains(&bearing) {
            f.push(format!(
                "Smyshlennyy bearing {bearing:.1} deg is not in the printed 090-270 deg aft sector"
            ));
        }
        let range = pool.pos.range_to(sove.pos);
        let horizon = horizon_nm(22.0, 0.0);
        if range > horizon {
            f.push(format!(
                "Smyshlennyy {range:.2} nm from Poolster is beyond the {horizon:.2} nm mapped radar horizon"
            ));
        }
        for name in NATO_ESCORTS {
            if let Some(e) = find(world, name) {
                let d = sove.pos.range_to(e.pos);
                if d < MIN_ESCORT_NM {
                    f.push(format!(
                        "Smyshlennyy is {d:.3} nm from {name}, inside the printed 2,000 yd stand-off"
                    ));
                }
            }
        }
    } else {
        f.push("missing Poolster or Smyshlennyy (PDF p.2)".into());
    }

    // PDF p.2 environment: night, light rain, 5% visibility. The engine has no
    // sea-state/wind/depth fields; see scenarios/tattletail.json.
    if world.day_night != DayNight::Night {
        f.push("day_night is not Night (PDF p.2: 0300, nighttime)".into());
    }
    if world.weather.rain <= 0.0 {
        f.push("no rain set (PDF p.2: light rain)".into());
    }
    if world.weather.overcast <= 0.0 {
        f.push("no overcast set (PDF p.2: night/rain conditions)".into());
    }
    if world.weather.visibility_nm <= 0.0 || world.weather.visibility_nm > 1.0 {
        f.push(format!(
            "visibility_nm {:.2} is not the declared 5%-of-clear mapping (<=1.0 nm)",
            world.weather.visibility_nm
        ));
    }

    f
}

fn check_contact_books(world: &World, trace: &Trace) -> Vec<String> {
    let mut f: Vec<String> = Vec::new();
    let poolster = uid(world, POOLSTER);
    let smyshlennyy = uid(world, SMYSHLENNYY);

    for side in [Side::Blue, Side::Red] {
        let live = world.sides[side.index()]
            .contacts
            .values()
            .filter(|c| !c.stale)
            .count();
        if live == 0 {
            f.push(format!(
                "{side} has no live contacts after 60 s; both sides start with radar-active/Good FC (PDF p.2)"
            ));
        }
        let sensors = trace
            .detection_sensors
            .get(side.name())
            .cloned()
            .unwrap_or_default();
        if sensors.is_empty() {
            f.push(format!("{side} produced no Detection event in the first 60 s"));
        } else if !sensors.iter().any(|s| s.starts_with("radar")) {
            f.push(format!(
                "{side} detection sensors {sensors:?} contain no radar entry (PDF p.2: all radars active)"
            ));
        }
    }

    if let Some(pool) = poolster {
        match world.find_contact(Side::Red, pool) {
            None => f.push("Red has no contact on Poolster (primary target, PDF p.2)".into()),
            Some(cid) => {
                let c = world.contact(Side::Red, cid);
                let q = c.map(|c| c.quality).unwrap_or(0.0);
                if q < 0.30 {
                    f.push(format!(
                        "Red's Poolster contact quality {q:.2} < 0.30 after 60 s (Good FC gate)"
                    ));
                }
                if !c
                    .map(|c| c.sensors.iter().any(|s| s.starts_with("radar")))
                    .unwrap_or(false)
                {
                    f.push("Red's Poolster track is not radar-derived (PDF p.2: radars active)".into());
                }
                if c.map(|c| c.class_name.is_none()).unwrap_or(true) {
                    f.push("Red's Poolster contact is not identified after 60 s".into());
                }
                if let Some(c) = c {
                    if (c.est_speed_kts - 12.0).abs() > 0.5 {
                        f.push(format!(
                            "Red estimates Poolster at {:.1} kt, expected 12 kt",
                            c.est_speed_kts
                        ));
                    }
                    if c.est_course_deg.min(360.0 - c.est_course_deg) > 1.0 {
                        f.push(format!(
                            "Red estimates Poolster on course {:.1}, expected 000",
                            c.est_course_deg
                        ));
                    }
                }
            }
        }
    }
    if let Some(sove) = smyshlennyy {
        match world.find_contact(Side::Blue, sove) {
            None => f.push("Blue has no contact on Smyshlennyy after 60 s".into()),
            Some(cid) => {
                let c = world.contact(Side::Blue, cid);
                let q = c.map(|c| c.quality).unwrap_or(0.0);
                if q < 0.30 {
                    f.push(format!(
                        "Blue's Smyshlennyy contact quality {q:.2} < 0.30 after 60 s (Good FC gate)"
                    ));
                }
                if !c
                    .map(|c| c.sensors.iter().any(|s| s.starts_with("radar")))
                    .unwrap_or(false)
                {
                    f.push(
                        "Blue's Smyshlennyy track is not radar-derived (PDF p.2: radars active)"
                            .into(),
                    );
                }
                if c.map(|c| c.class_name.is_none()).unwrap_or(true) {
                    f.push("Blue's Smyshlennyy contact is not identified after 60 s".into());
                }
                if let Some(c) = c {
                    if (c.est_speed_kts - 12.0).abs() > 0.5 {
                        f.push(format!(
                            "Blue estimates Smyshlennyy at {:.1} kt, expected 12 kt",
                            c.est_speed_kts
                        ));
                    }
                    if c.est_course_deg.min(360.0 - c.est_course_deg) > 1.0 {
                        f.push(format!(
                            "Blue estimates Smyshlennyy on course {:.1}, expected 000",
                            c.est_course_deg
                        ));
                    }
                }
            }
        }
    }

    // Double-blind projection: a client sees only its own units, and enemy
    // truth never appears in its unit list. Contact estimates are allowed.
    for side in [Side::Blue, Side::Red] {
        let obs = observe(world, side, 0, 1000);
        if obs.units.iter().any(|u| u.side != side) {
            f.push(format!("{side} observation leaks enemy units"));
        }
        if obs.contacts.is_empty() {
            f.push(format!("{side} observation has no contacts after 60 s"));
        }
        if obs
            .contacts
            .iter()
            .any(|c| c.category == PlatformKind::Sub)
        {
            f.push(format!(
                "{side} observation has a subsurface contact; PDF p.2 has no submarine"
            ));
        }
    }

    f
}

fn check_events(world: &World, trace: &Trace) -> Vec<String> {
    let mut f: Vec<String> = Vec::new();
    // --- 0300-0315 attack window and Poolster priority --------------------
    match trace.red_asm_launch_ticks.first() {
        None => f.push("Red made no anti-ship launch; the PDF orders an attack by 0315".into()),
        Some(t) => {
            if *t < ATTACK_TICK || *t > ATTACK_DEADLINE_TICK {
                f.push(format!(
                    "first Red anti-ship launch at tick {t}, outside the 0300-0315 window ({ATTACK_TICK}-{ATTACK_DEADLINE_TICK})"
                ));
            }
        }
    }
    if trace.red_asm_launch_ticks.len() != 4 {
        f.push(format!(
            "Red fired {} anti-ship missiles; the card carries four P-15M and the driver orders two twin salvos (PDF p.8)",
            trace.red_asm_launch_ticks.len()
        ));
    }
    if trace
        .red_asm_launch_ticks
        .iter()
        .any(|t| *t < ATTACK_TICK || *t > ATTACK_DEADLINE_TICK)
    {
        f.push("an anti-ship launch occurred outside the 0300-0315 window".into());
    }
    if trace.red_missile_targets.is_empty() {
        f.push("no Red missile projectile was observed in flight".into());
    }
    let poolster = uid(world, POOLSTER);
    for (mid, target) in &trace.red_missile_targets {
        if *target != poolster {
            f.push(format!(
                "Red missile #{mid} targeted {target:?}, not Poolster (primary target)"
            ));
        }
    }

    // --- NATO self-defense release ---------------------------------------
    if trace
        .blue_asm_launch_ticks
        .iter()
        .any(|t| *t < ATTACK_TICK)
    {
        f.push("Blue fired anti-ship missiles before the Soviet attack (ROE violation)".into());
    }
    if trace.blue_asm_launch_ticks.is_empty() {
        f.push("Blue never returned anti-ship fire after the Soviet attack".into());
    }
    if trace.blue_sam_shot_ticks.is_empty() {
        f.push("Blue never engaged the incoming missiles with SAMs".into());
    }
    if trace.blue_incoming_ticks.len() < 4 {
        f.push(format!(
            "Blue detected only {} incoming missiles; all four P-15M must show up as incoming",
            trace.blue_incoming_ticks.len()
        ));
    }
    if trace.blue_intercept_ticks.is_empty() {
        f.push("no Blue intercept adjudication occurred".into());
    }

    // --- scenario closure -------------------------------------------------
    if trace.scenario_end_tick.is_none() {
        f.push("no ScenarioEnded event; run did not close".into());
    }
    if trace.event_counts.get("detection").copied().unwrap_or(0) == 0 {
        f.push("event stream contains no detection (empty run)".into());
    }
    if trace.event_counts.get("scenario_started").copied().unwrap_or(0) != 1 {
        f.push("scenario_started count != 1".into());
    }

    // --- source fidelity: no submarine in this PDF ------------------------
    if let Some(n) = trace.detection_categories.get("Subsurface") {
        f.push(format!(
            "{n} subsurface detection event(s); the PDF's Russian unit is the DDG Smyshlennyy, no submarine"
        ));
    }

    f
}

fn print_report(run: &Run) {
    let t = &run.trace;

    println!(
        "initial geometry .... {}  (six ships, printed diamond/aft sectors, 2,000-10,000 yd, course 000/12 kt)",
        mark(run.initial_failures.is_empty())
    );
    println!(
        "contact books ....... {}  (Blue {} / Red {} live contacts at t=60 s; Poolster track q={:.2}, Smyshlennyy track q={:.2}; double blind)",
        mark(run.contact_failures.is_empty()),
        t.contacts_at_60[0],
        t.contacts_at_60[1],
        t.poolster_track_quality.unwrap_or(f64::NAN),
        t.smyshlennyy_track_quality.unwrap_or(f64::NAN)
    );

    println!(
        "attack window ....... {}  (Red ASM launch ticks {:?}; window {}..={})",
        mark(t.red_asm_launch_ticks.first().map(|x| *x >= ATTACK_TICK && *x <= ATTACK_DEADLINE_TICK).unwrap_or(false)
            && t.red_asm_launch_ticks.len() >= 2
            && t.red_asm_launch_ticks.iter().all(|x| *x <= ATTACK_DEADLINE_TICK)),
        t.red_asm_launch_ticks,
        ATTACK_TICK,
        ATTACK_DEADLINE_TICK
    );
    println!(
        "NATO response ....... {}  (Blue ASM launch ticks {:?}; SAM shots {}; intercept adjudications {} ({} hits); incoming {})",
        mark(!t.blue_asm_launch_ticks.is_empty()
            && t.blue_asm_launch_ticks.iter().all(|x| *x >= ATTACK_TICK)
            && !t.blue_sam_shot_ticks.is_empty()
            && !t.blue_intercept_ticks.is_empty()
            && t.blue_incoming_ticks.len() >= 4),
        t.blue_asm_launch_ticks,
        t.blue_sam_shot_ticks.len(),
        t.blue_intercept_ticks.len(),
        t.blue_intercept_hits,
        t.blue_incoming_ticks.len()
    );

    println!("\n-- event coverage --");
    for (name, n) in &t.event_counts {
        println!(
            "{name:24} {:>5}  {}",
            n,
            t.first_seen.get(name).cloned().unwrap_or_default()
        );
    }
    println!("\n-- rules-extension events --");
    if t.rule_counts.is_empty() {
        println!("(none)");
    }
    for (name, n) in &t.rule_counts {
        println!(
            "{name:24} {:>5}  {}",
            n,
            t.first_seen.get(name).cloned().unwrap_or_default()
        );
    }
    let subs = t
        .detection_categories
        .get("Subsurface")
        .copied()
        .unwrap_or(0);
    println!("\nsubsurface detections: {subs} (expected 0; PDF p.2 has no submarine)");
    println!("scenario end tick: {:?} / run ticks: {}", t.scenario_end_tick, t.ticks);
    match &run.result {
        Some(r) => println!(
            "result: {:?} ({}), VP Blue {:.1} / Red {:.1}",
            r.winner, r.reason, r.vp_blue, r.vp_red
        ),
        None => println!("result: none"),
    }

    println!("\n-- final state --");
    for s in &t.final_state {
        println!("{s}");
    }
}

fn final_state(world: &World) -> Vec<String> {
    let mut out = Vec::new();
    for (name, _, _) in OFFICIAL {
        if let Some(e) = find(world, name) {
            out.push(format!(
                "{name:24} hp {:7.1}/{:<7.1} alive {:<5} pos ({:6.2},{:6.2})",
                e.hp, e.hp_max, e.alive, e.pos.x, e.pos.y
            ));
        } else {
            out.push(format!("{name:24} MISSING"));
        }
    }
    out
}

// --- helpers -------------------------------------------------------------

fn uid(world: &World, name: &str) -> Option<EntityId> {
    world.entities.values().find(|e| e.name == name).map(|e| e.id)
}

fn find<'a>(world: &'a World, name: &str) -> Option<&'a Entity> {
    world.entities.values().find(|e| e.name == name)
}

fn escort_arc(bearing: f64) -> Option<usize> {
    if (315.0..360.0).contains(&bearing) || (0.0..=45.0).contains(&bearing) {
        Some(0)
    } else if (45.0..=135.0).contains(&bearing) {
        Some(1)
    } else if (135.0..=225.0).contains(&bearing) {
        Some(2)
    } else if (225.0..=315.0).contains(&bearing) {
        Some(3)
    } else {
        None
    }
}

fn mark(ok: bool) -> &'static str {
    if ok {
        "PASS"
    } else {
        "FAIL"
    }
}

fn hash_bytes(mut h: u64, bytes: &[u8]) -> u64 {
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

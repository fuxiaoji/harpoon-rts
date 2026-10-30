//! Native unique-opportunity cross-process acceptance preparation.
//! Source-derived fixtures are copied below as standalone code, never injected
//! Flight/die/hit states. Phase1 writes references, phase2 must actually run
//! in another process; this source file alone is not an acceptance result.

#[allow(dead_code)]
mod ordinary {
    use harpoon_core::{
        load_scenario, Command, ContactId, Database, EventKind, Scenario, Side, Vec2, World,
    };
    use serde_json::{json, Value};

    const TORPEDO_ID: u32 = 5;

    fn roll(seed: u64, entity: u32, purpose: &str, sides: u8) -> u8 {
        let mut r =
            harpoon_core::Rng::new(seed).derive(harpoon_core::Rng::hash_tag(0, entity, purpose));
        1 + (r.next_f64() * sides as f64) as u8
    }
    fn d100(seed: u64, entity: u32, purpose: &str) -> u8 {
        roll(seed, entity, purpose, 100)
    }
    fn d10(seed: u64, purpose: &str) -> u8 {
        roll(seed, 0, purpose, 10)
    }
    fn attack_purpose(pass: u32) -> String {
        format!("10.2.5.attack:{pass}")
    }
    fn placement_purpose(pass: u32) -> String {
        format!("10.placement:{TORPEDO_ID}:{pass}")
    }
    fn seed_for(test: impl Fn(u64) -> bool) -> u64 {
        (1..=2_000_000)
            .find(|seed| test(*seed))
            .expect("pre-World native seed exists")
    }

    fn fixture(seed: u64) -> (World, harpoon_core::EntityId, harpoon_core::EntityId) {
        let db = Database::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../database/consolidated.json"
        )))
        .unwrap();
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
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,5.0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"OTHER_OPPORTUNITY","pos":[0.8,5.5],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Blue","class":"us_burke_2a","name":"FAR_BLUE_RETAINER","pos":[300,300],"speed_kts":0,"fire_on":false}]})).unwrap();
        let mut w = load_scenario(sc, db).unwrap();
        let s = w
            .entities
            .values()
            .find(|e| e.name == "SHOOTER")
            .unwrap()
            .id;
        let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
        // These author fixtures isolate post-release mechanics. The only injected
        // located contact is the original TARGET; it is not native sonar/FCS.
        // OTHER remains absent from Blue contacts and never creates its own ACM.
        for e in w.entities.values_mut() {
            e.sensors.clear();
            if e.side == Side::Red {
                e.weapons.clear();
            }
        }
        assert_eq!(s.0, 1);
        assert_eq!(t.0, 2);
        w.sides[0].contacts.insert(
            ContactId(99),
            harpoon_core::world::Contact {
                id: ContactId(99),
                about: t,
                category: harpoon_core::types::PlatformKind::Sub,
                class_name: None,
                estimate: Vec2::new(0., 5.0),
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
        (w, s, t)
    }

    fn launch(w: &mut World, s: harpoon_core::EntityId) -> harpoon_core::EntityId {
        harpoon_core::sim::tick(
            w,
            &[(
                Side::Blue,
                Command::Fire {
                    unit: s,
                    contact: ContactId(99),
                    weapon: "us_mk48".into(),
                    rounds: 1,
                },
            )],
        );
        let id = w
            .entities
            .values()
            .find(|e| e.kind == harpoon_core::types::PlatformKind::Torpedo)
            .expect("torpedo released")
            .id;
        assert_eq!(id.0, TORPEDO_ID, "fixture geometry fixes the torpedo id");
        id
    }

    fn details(w: &World, rule: &str) -> Vec<Value> {
        w.events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::RulesEvent {
                    rule: r, detail, ..
                } if r == rule => Some(serde_json::from_str(detail).unwrap()),
                _ => None,
            })
            .collect()
    }
    fn attacks(w: &World) -> Vec<Value> {
        details(w, "10.2.5.source_terminal")
    }
    fn placements(w: &World) -> Vec<Value> {
        details(w, "10.2.4.placement")
    }
    fn flight<'a>(w: &'a World) -> &'a harpoon_core::rules::torpedo2026::Flight {
        w.rules
            .torpedoes
            .as_ref()
            .unwrap()
            .flights
            .get(&harpoon_core::EntityId(TORPEDO_ID))
            .unwrap()
    }
    fn restored(w: &World) -> World {
        let copy: World = serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap();
        copy.rules.validate_tables().unwrap();
        copy
    }

    pub(super) fn starting() -> (
        World,
        harpoon_core::EntityId,
        harpoon_core::EntityId,
        harpoon_core::EntityId,
        u32,
        u64,
    ) {
        let seed = seed_for(|seed| {
            d10(seed, &placement_purpose(0)) <= 7
                && d100(seed, TORPEDO_ID, &attack_purpose(0)) > 80
                && d10(seed, &placement_purpose(1)) > 7
                && d10(seed, "10.placement:5:1:op:3") <= 7
                && d100(seed, TORPEDO_ID, &attack_purpose(1)) <= 80
        });
        let (w, shooter, original) = fixture(seed);
        let object = w
            .entities
            .values()
            .find(|e| e.name == "OTHER_OPPORTUNITY")
            .unwrap()
            .id;
        (w, shooter, original, object, TORPEDO_ID, seed)
    }
}

#[allow(dead_code)]
mod moss {
    use harpoon_core::{
        load_scenario, Command, ContactId, Database, EventKind, Scenario, Side, Vec2, World,
    };
    use serde_json::{json, Value};

    const TORP: u32 = 4;
    const MOSS: u32 = 3;

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
    fn attack_purpose(pass: u32) -> String {
        format!("10.2.5.attack:{pass}")
    }
    fn placement_purpose(pass: u32) -> String {
        format!("10.placement:{TORP}:{pass}")
    }
    fn op_purpose(pass: u32, candidate: u32) -> String {
        format!("10.placement:{TORP}:{pass}:op:{candidate}")
    }
    fn seed_for(test: impl Fn(u64) -> bool) -> u64 {
        (1..=2_000_000)
            .find(|seed| test(*seed))
            .expect("pre-World native seed exists")
    }

    fn fixture(seed: u64, moss: bool) -> (World, harpoon_core::EntityId, harpoon_core::EntityId) {
        let db = Database::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../database/consolidated.json"
        )))
        .unwrap();
        let sc: Scenario = serde_json::from_value(json!({
        "id":"torpedo-opportunity","name":"10.2.7 opportunity chain","duration_s":3000,"seed":seed,
        "context":{"rules_profile":"hv2026-rts-v4",
        "detection_2026":{"source":"scenario_assumption: reattack environment","targets":{"cn_093":{"source":"scenario_assumption: signature the decoy imitates","size":2,"noise":2,"anechoic":true,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":0}},"environment":{"source":"scenario_assumption: open water 400m",
            "sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":400.0,"layer":null,"ice":0,"cloud":null}},
        "torpedo_2026":{"source":"scenario_assumption: opportunity chain mechanisms",
        "environment":{"source":"scenario_assumption: open water no layer","layer_m":null,"reverberant":false},
        "targets":{"cn_093":{"source":"scenario_assumption: controlled target","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
        "platforms":{"us_virginia":{"source":"scenario_assumption: loaded tubes","wire_capacity":2,"reload_parallel":2,
            "tubes":[{"bearing_deg":0,"loaded":"us_mk48"},{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}},
        "orders":{"SHOOTER:us_mk48":{"source":"scenario_assumption: declared order","speed_index":0,"depth_m":50,
            "enable_run_nm":0,"bearing_deg":null,"cut_wire":true,"spread_deg":[],
            "solution":{"quality":"Good","source":"scenario_assumption: fixture solution"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,5.0],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
        let mut w = load_scenario(sc, db).unwrap();
        let s = w
            .entities
            .values()
            .find(|e| e.name == "SHOOTER")
            .unwrap()
            .id;
        let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
        w.entity_mut(t).unwrap().weapons.clear();
        if moss {
            // Deploy a real Red-side mobile decoy through the EW command path.
            use harpoon_core::rules::ew2026 as ew;
            let mut f = ew::Fit {
                source: "scenario_assumption: generic mechanism fixture".into(),
                kind: ew::ObjectKind::MobileDecoy,
                generation: Some(3),
                launcher: Some(ew::Launcher::Internal),
                endurance_s: None,
                deployment_s: None,
                radar_height_m: Some(1.),
            };
            f.generation = Some(3);
            {
                let st = w.rules.ew.as_mut().unwrap();
                st.config.fits.insert("mobile".into(), f.clone());
                st.config
                    .loadouts
                    .entry("cn_093".into())
                    .or_default()
                    .insert("mobile".into(), 2);
            }
            let launcher = t;
            w.entity_mut(launcher).unwrap().kind = harpoon_core::types::PlatformKind::Sub;
            let speed = [7., 10., 12., 15.][2];
            ew::apply(
                &mut w,
                launcher,
                &ew::Action::Deploy {
                    fit: "mobile".into(),
                    at: None,
                    radar_signature: None,
                    course_deg: Some(90.),
                    speed_kts: Some(speed),
                    depth_m: Some(50.),
                },
            )
            .expect("real Red MOSS deployment");
        }
        w.sides[0].contacts.insert(
            ContactId(99),
            harpoon_core::world::Contact {
                id: ContactId(99),
                about: t,
                category: harpoon_core::types::PlatformKind::Sub,
                class_name: None,
                estimate: Vec2::new(0., 5.0),
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
        (w, s, t)
    }

    fn launch(w: &mut World, s: harpoon_core::EntityId) -> harpoon_core::EntityId {
        harpoon_core::sim::tick(
            w,
            &[(
                Side::Blue,
                Command::Fire {
                    unit: s,
                    contact: ContactId(99),
                    weapon: "us_mk48".into(),
                    rounds: 1,
                },
            )],
        );
        w.entities
            .values()
            .find(|e| e.kind == harpoon_core::types::PlatformKind::Torpedo)
            .expect("torpedo released")
            .id
    }

    fn details(w: &World, rule: &str) -> Vec<Value> {
        w.events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::RulesEvent {
                    rule: r, detail, ..
                } if r == rule => Some(serde_json::from_str(detail).unwrap()),
                _ => None,
            })
            .collect()
    }

    pub(super) fn starting() -> (
        World,
        harpoon_core::EntityId,
        harpoon_core::EntityId,
        harpoon_core::EntityId,
        u32,
        u64,
    ) {
        let seed = seed_for(|seed| {
            d10(seed, &placement_purpose(0)) <= 7
                && d100(seed, &attack_purpose(0)) > 64
                && d10(seed, &placement_purpose(1)) > 4
                && matches!(d10(seed, &op_purpose(1, MOSS)), 5 | 6)
                && d100(seed, &attack_purpose(1)) <= 64
        });
        let (w, shooter, original) = fixture(seed, true);
        (
            w,
            shooter,
            original,
            harpoon_core::EntityId(MOSS),
            TORP,
            seed,
        )
    }
}

use harpoon_core::{Command, ContactId, EntityId, EventKind, Side, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Case {
    mode: String,
    remove_launcher: bool,
    capture_tick: i64,
    terminal_tick: i64,
    shooter: EntityId,
    original: EntityId,
    object: EntityId,
    torpedo: EntityId,
    checkpoint: String,
    reference: String,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    schema: String,
    phase1_pid: u32,
    cases: Vec<Case>,
    facts: Vec<Value>,
    assumptions: Vec<String>,
}

fn write_new(dir: &Path, name: &str, bytes: &[u8]) {
    let path = dir.join(name);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .unwrap_or_else(|error| panic!("refuse overwriting evidence {}: {error}", path.display()));
    file.write_all(bytes).unwrap();
}

fn save(dir: &Path, name: &str, world: &World) {
    write_new(dir, name, &serde_json::to_vec(world).unwrap());
}

fn load(dir: &Path, name: &str) -> World {
    let bytes = std::fs::read(dir.join(name)).unwrap();
    let world: World = serde_json::from_slice(&bytes).unwrap();
    world.rules.validate_tables().unwrap();
    world
}

fn fnv64(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |h, byte| {
        (h ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

fn details(world: &World, rule: &str) -> Vec<Value> {
    world
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RulesEvent {
                rule: r, detail, ..
            } if r == rule => Some(serde_json::from_str(detail).unwrap()),
            _ => None,
        })
        .collect()
}

fn check_privacy(world: &World, case: &Case) {
    let rules = [
        "10.2.4.opportunity_placement",
        "10.2.7.opportunity_acquired",
        "10.2.5.source_terminal",
    ];
    for rule in rules {
        let events: Vec<_> = world
            .events
            .iter()
            .filter(|event| matches!(&event.kind, EventKind::RulesEvent {rule:r,..} if r == rule))
            .collect();
        assert!(
            !events.is_empty(),
            "{rule} requires actual nonempty evidence"
        );
        assert!(events
            .iter()
            .all(|event| matches!(event.vis, harpoon_core::Visibility::Referee)));
    }
    assert!(
        world.sides[Side::Blue.index()]
            .contacts
            .values()
            .all(|c| c.about != case.object),
        "capture is private physics and cannot create a free shooter Contact"
    );
    let release =
        harpoon_core::rules::torpedo2026::unreported_releases(world, Side::Blue, ContactId(99))
            .expect("immutable original contact release ledger remains after launcher removal");
    assert_eq!(release["status"], "unknown");
    assert!(release["physical_in_flight"].is_null());
    let known = release["released"].as_array().unwrap();
    assert_eq!(known.len(), 1);
    assert_eq!(known[0]["status"], "unknown");
    assert!(known[0]["physical_alive"].is_null());
    assert!(known[0]["reported_status"].is_null());
    assert!(!release.to_string().contains("opportunity_acquired"));
    assert!(!release.to_string().contains("candidate_placement"));
    if let Some(shooter) = world.entity(case.shooter) {
        assert_eq!(
            shooter.ext.get("control.manual"),
            Some(&1.),
            "the original player takeover must persist to the terminal state"
        );
    }
}

fn advance_one(world: &mut World, case: &Case) {
    assert!(
        !world.over,
        "author retainer prevents an unrelated early game-over"
    );
    if world.tick == case.capture_tick {
        harpoon_core::sim::tick(
            world,
            &[(
                Side::Blue,
                Command::SetUnitAutomation {
                    unit: case.shooter,
                    enabled: false,
                },
            )],
        );
    } else {
        harpoon_core::sim::tick(world, &[]);
    }
    if case.remove_launcher && world.tick == case.capture_tick + 2 {
        let removed = world
            .entities
            .remove(&case.shooter)
            .expect("live launcher exists before lifecycle variant");
        assert_eq!(
            removed.ext.get("control.manual"),
            Some(&1.),
            "manual takeover is actual before removing its entity"
        );
        assert!(world
            .entities
            .values()
            .any(|e| e.alive && e.side == Side::Blue && e.name == "FAR_BLUE_RETAINER"));
    }
    assert!(
        !world
            .rules
            .pending_adjudications
            .contains_key(&case.torpedo),
        "qualified positive source opportunity must execute, not silently freeze"
    );
}

fn terminal_facts(world: &World, case: &Case) -> Value {
    let terminals = details(world, "10.2.5.source_terminal");
    assert_eq!(
        terminals.len(),
        2,
        "initial miss plus exactly one native opportunity attack"
    );
    assert_eq!(terminals[0]["hit"], false);
    assert_eq!(terminals[1]["hit"], true);
    assert_eq!(terminals[1]["target"], case.object.0);
    assert_eq!(terminals[1]["pass"], 1);
    assert!(!world
        .entity(case.torpedo)
        .is_some_and(|entity| entity.alive));
    let flight = &world.rules.torpedoes.as_ref().unwrap().flights[&case.torpedo];
    assert_eq!(flight.target, case.original, "release target is immutable");
    let capture = flight
        .opportunity
        .as_ref()
        .unwrap()
        .capture
        .as_ref()
        .unwrap();
    assert_eq!(
        capture.owner,
        Side::Blue,
        "actual body ownership survives a removed launcher"
    );
    assert_eq!(capture.release_target, case.original);
    assert_eq!(capture.captured_at, case.capture_tick);
    assert!(capture
        .pursuit_started_at
        .is_some_and(|tick| tick > capture.captured_at));
    assert_eq!(
        capture.original_placement.identity,
        flight.placement.as_ref().unwrap().identity
    );
    assert!(!capture.original_placement.placed);
    assert!(capture.candidate_placement.placed);
    let outcome = if case.mode == "entity" {
        let damage: f64 = world
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::Impact { unit, damage, .. } if *unit == case.object => Some(*damage),
                _ => None,
            })
            .sum();
        assert!(
            damage > 0.,
            "ordinary opportunity must cause real production Impact damage"
        );
        if world.entity(case.object).is_none() {
            assert!(
                world.events.iter().any(
                    |event| matches!(&event.kind,EventKind::Sunk{unit,..}if *unit==case.object)
                ),
                "a cleaned-up opportunity body needs an actual native Sunk event"
            );
        }
        json!({"damage":damage,"remaining_hp":world.entity(case.object).map(|e|e.hp),
            "sunk":world.events.iter().any(|event|matches!(&event.kind,EventKind::Sunk{unit,..}if *unit==case.object))})
    } else {
        assert_eq!(terminals[1]["probability"], 0.64);
        assert_eq!(terminals[1]["opportunity_kind"], "mobile_decoy");
        assert_eq!(
            world.rules.ew.as_ref().unwrap().objects[&case.object].status,
            harpoon_core::rules::ew2026::Status::Destroyed
        );
        assert_eq!(details(world, "10.2.7.opportunity_destroyed").len(), 1);
        json!({"mobile_decoy_destroyed_exactly_once":true})
    };
    check_privacy(world, case);
    json!({"mode":case.mode,"removed_launcher":case.remove_launcher,"tick":world.tick,
        "torpedo":case.torpedo,"object":case.object,"original":case.original,
        "capture_tick":case.capture_tick,"attack":terminals[1],"outcome":outcome,
        "world_fnv64":fnv64(&serde_json::to_vec(world).unwrap()),
        "private_capture_preserved":true,"release_status":"unknown"})
}

fn finish(world: &mut World, case: &Case) {
    for _ in 0..2200 {
        if details(world, "10.2.5.source_terminal").len() == 2
            && !world
                .entity(case.torpedo)
                .is_some_and(|entity| entity.alive)
        {
            return;
        }
        advance_one(world, case);
    }
    panic!("native terminal was not reached in a bounded continuation");
}

fn phase1(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    assert!(
        !dir.join("phase1-manifest.json").exists(),
        "use a new acceptance directory"
    );
    let mut cases = Vec::new();
    let mut facts = Vec::new();
    for mode in ["entity", "moss"] {
        let (mut world, shooter, original, object, torpedo_id, seed) = if mode == "entity" {
            ordinary::starting()
        } else {
            moss::starting()
        };
        world.rules.validate_tables().unwrap();
        // This declared author fixture has no sensing: CID99 and Good isolate
        // the post-release mechanics, not native FCS/classification success.
        for entity in world.entities.values_mut() {
            entity.sensors.clear();
        }
        save(dir, &format!("{mode}-initial-author-world.json"), &world);
        harpoon_core::sim::tick(
            &mut world,
            &[(
                Side::Blue,
                Command::Fire {
                    unit: shooter,
                    contact: ContactId(99),
                    weapon: "us_mk48".into(),
                    rounds: 1,
                },
            )],
        );
        let torpedo = EntityId(torpedo_id);
        assert!(world
            .entity(torpedo)
            .is_some_and(|e| e.kind == harpoon_core::types::PlatformKind::Torpedo));
        for _ in 0..2000 {
            if !details(&world, "10.2.7.opportunity_acquired").is_empty() {
                break;
            }
            assert!(!world.over);
            harpoon_core::sim::tick(&mut world, &[]);
            assert!(!world.rules.pending_adjudications.contains_key(&torpedo));
        }
        let acquired = details(&world, "10.2.7.opportunity_acquired");
        assert_eq!(acquired.len(), 1, "actual unique capture is required");
        assert_eq!(acquired[0]["candidate"], object.0);
        let capture_tick = world.tick;
        let capture = &world.rules.torpedoes.as_ref().unwrap().flights[&torpedo]
            .opportunity
            .as_ref()
            .unwrap()
            .capture
            .as_ref()
            .unwrap();
        assert_eq!(capture.captured_at, capture_tick);
        assert_eq!(
            capture.pursuit_started_at, None,
            "no free movement on capture tick"
        );
        assert!(capture.terminal.is_none());
        save(dir, &format!("{mode}-capture.json"), &world);
        for remove_launcher in [false, true] {
            if mode == "moss" && remove_launcher {
                continue;
            }
            let label = if remove_launcher { "removed" } else { "live" };
            let reference_file = format!("{mode}-{label}-reference.json");
            let mut case = Case {
                mode: mode.into(),
                remove_launcher,
                capture_tick,
                terminal_tick: 0,
                shooter,
                original,
                object,
                torpedo,
                checkpoint: format!("{mode}-capture.json"),
                reference: reference_file.clone(),
            };
            let mut mid = world.clone();
            while mid.tick < capture_tick + 5 {
                advance_one(&mut mid, &case);
            }
            let captured = &mid.rules.torpedoes.as_ref().unwrap().flights[&torpedo]
                .opportunity
                .as_ref()
                .unwrap()
                .capture
                .as_ref()
                .unwrap();
            assert!(captured.pursuit_started_at.is_some());
            assert!(
                captured.terminal.is_none(),
                "mid checkpoint must precede the actual attack"
            );
            let pursuit_file = format!("{mode}-{label}-pursuit.json");
            save(dir, &pursuit_file, &mid);
            let mut reference = world.clone();
            finish(&mut reference, &case);
            case.terminal_tick = reference.tick;
            let evidence = terminal_facts(&reference, &case);
            save(dir, &reference_file, &reference);
            facts.push(json!({"seed":seed,"source_facts":evidence}));
            cases.push(case.clone());
            case.checkpoint = pursuit_file;
            cases.push(case);
        }
    }
    let manifest=Manifest {schema:"2026-10-25-opportunity-complete-world-source-v1".into(),
        phase1_pid:std::process::id(),cases,facts,assumptions:vec![
            "Source exact subset: unique qualified non-wire coplanar opportunity first attack; full book remains incomplete".into(),
            "Scenario author Good and original located CID99/classification/signatures isolate post-release physics; no native FCS claim".into(),
            "Ordinary geometry original5nm, opportunity(.8,5.5) at50m; explicit far Blue retainer prevents unrelated early gameover".into(),
            "MOSS is actual EW Deploy Gen3, max12kt/course90/depth50m with source imitated noise; original target5nm".into(),
            "All initial sensors are author disabled; capture itself must not mint an own contact".into(),
            "Manual takeover is a real SetUnitAutomation command at capture+1; removed branch physically removes launcher after capture+2, an explicit lifecycle test rather than combat sinking".into(),
            "No Flight/opportunity/phase/Placement/die/HP success injection or probability alteration".into(),
            "Writing phase1 files does not prove independent process validation; phase2 must actually run".into(),
        ]};
    write_new(
        dir,
        "phase1-manifest.json",
        &serde_json::to_vec_pretty(&manifest).unwrap(),
    );
    println!(
        "phase1 actual native captures and terminal references saved: {} checkpoints, pid {}",
        manifest.cases.len(),
        manifest.phase1_pid
    );
}

fn phase2(dir: &Path) {
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(dir.join("phase1-manifest.json")).unwrap()).unwrap();
    assert_ne!(
        manifest.phase1_pid,
        std::process::id(),
        "phase2 must be an independent process"
    );
    let mut results = Vec::new();
    for case in &manifest.cases {
        let reference = load(dir, &case.reference);
        let reference_bytes = serde_json::to_vec(&reference).unwrap();
        let mut world = load(dir, &case.checkpoint);
        assert!(world.tick <= case.terminal_tick);
        while world.tick < case.terminal_tick {
            advance_one(&mut world, case);
        }
        assert_eq!(world.tick, case.terminal_tick);
        assert_eq!(
            serde_json::to_vec(&world).unwrap(),
            reference_bytes,
            "full World differs after independent restore of {}",
            case.checkpoint
        );
        let facts = terminal_facts(&world, case);
        results.push(
            json!({"checkpoint":case.checkpoint,"reference":case.reference,
            "removed_launcher":case.remove_launcher,"full_world_byte_equal":true,
            "tables_validated":true,"facts":facts}),
        );
    }
    write_new(dir,"phase2-validation.json",&serde_json::to_vec_pretty(&json!({
        "ok":true,"phase1_pid":manifest.phase1_pid,"phase2_pid":std::process::id(),
        "verified_checkpoints":results.len(),"results":results,
        "scope":"Two source mechanism fixtures; not full-book, native FCS, HTTP/MCP or LLM-match acceptance"})).unwrap());
    println!("phase2 independent complete World equality and source terminal checks passed for {} checkpoints",manifest.cases.len());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "usage: torpedo_opportunity_source_crossproc_20261025 phase1|phase2 <new-evidence-dir>"
    );
    let dir = Path::new(&args[2]);
    match args[1].as_str() {
        "phase1" => phase1(dir),
        "phase2" => phase2(dir),
        _ => panic!("phase1 or phase2 required"),
    }
}

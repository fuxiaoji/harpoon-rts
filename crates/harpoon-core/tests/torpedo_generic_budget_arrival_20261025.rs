//! Unregistered native budget/arrival candidates. Author Good, CID99 and
//! declared signatures/geometry isolate physical source mechanics. No native
//! FCS claim and no Flight/phase/fuel/dice/HP success state is injected.

#[allow(dead_code)]
mod rejected_opportunity {
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

    fn source_seed(candidate_places: bool) -> u64 {
        seed_for(|seed| {
            d10(seed, &placement_purpose(0)) <= 7
                && d100(seed, &attack_purpose(0)) > 64
                && d10(seed, &placement_purpose(1)) > 4
                && (d10(seed, &op_purpose(1, MOSS)) <= 6) == candidate_places
                && d100(seed, &attack_purpose(1)) <= 64
        })
    }

    fn await_capture(w: &mut World, torpedo: harpoon_core::EntityId) {
        for _ in 0..2200 {
            harpoon_core::sim::tick(w, &[]);
            if !details(w, "10.2.7.opportunity_acquired").is_empty()
                || w.rules.pending_adjudications.contains_key(&torpedo)
                || !w.entity(torpedo).is_some_and(|e| e.alive)
            {
                break;
            }
        }
        assert!(
            !w.rules.pending_adjudications.contains_key(&torpedo),
            "fully sourced unique MOSS must not need an inferred target decision"
        );
        let acquired = details(w, "10.2.7.opportunity_acquired");
        assert_eq!(
            acquired.len(),
            1,
            "native seed must capture the actual MOSS"
        );
        assert_eq!(acquired[0]["kind"], "mobile_decoy");
        assert_eq!(acquired[0]["candidate"], MOSS);
    }

    fn finish_native_moss_hit(w: &mut World, torpedo: harpoon_core::EntityId) {
        for _ in 0..2200 {
            harpoon_core::sim::tick(w, &[]);
            if !details(w, "10.2.7.opportunity_destroyed").is_empty()
                && !w.entity(torpedo).is_some_and(|e| e.alive)
            {
                break;
            }
            assert!(
                !w.rules.pending_adjudications.contains_key(&torpedo),
                "qualified native pursuit must execute instead of freezing"
            );
        }
        assert_eq!(details(w, "10.2.7.opportunity_destroyed").len(), 1);
        assert!(!w.entity(torpedo).is_some_and(|e| e.alive));
        assert_eq!(
            w.rules.ew.as_ref().unwrap().objects[&harpoon_core::EntityId(MOSS)].status,
            harpoon_core::rules::ew2026::Status::Destroyed
        );
    }

    /// O1 preserves the source ACM-column hit/destruction assertions. Author
    /// geometry puts the launcher truly beyond acquisition range and the MOSS
    /// at the torpedo's own depth; native EW movement supplies its real position.

    pub(super) fn starting() -> (World, harpoon_core::EntityId) {
        let (w, s, _) = fixture(source_seed(false), true);
        (w, s)
    }
}
use harpoon_core::rules::torpedo2026::Phase;
use harpoon_core::{
    load_scenario, Command, ContactId, Database, EntityId, EventKind, Scenario, Side, Vec2, World,
};
use serde_json::{json, Value};

fn roll(seed: u64, entity: u32, purpose: &str, sides: u8) -> u8 {
    let mut rng =
        harpoon_core::Rng::new(seed).derive(harpoon_core::Rng::hash_tag(0, entity, purpose));
    1 + (rng.next_f64() * sides as f64) as u8
}
fn source_seed(placed: bool, hit: bool) -> u64 {
    (1..=100000)
        .find(|seed| {
            (roll(*seed, 0, "10.placement:3:0", 10) <= 7) == placed
                && (roll(*seed, 3, "10.2.5.attack:0", 100) <= 80) == hit
        })
        .unwrap()
}
fn fixture(
    seed: u64,
    distance: f64,
    target_speed: f64,
    speed_index: usize,
    enable: f64,
) -> (World, EntityId) {
    let db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let scenario:Scenario=serde_json::from_value(json!({
  "id":"generic-torpedo-native-budget","name":"Source budget mechanism fixture","seed":seed,"duration_s":4000,
  "context":{"rules_profile":"hv2026-rts-v4","torpedo_2026":{
   "source":"scenario_assumption: author Good, located original contact and controlled two-sub geometry, not native FCS",
   "environment":{"source":"scenario_assumption: no-layer nonreverberant source mechanism conditions","layer_m":null,"reverberant":false},
   "targets":{"cn_093":{"source":"scenario_assumption: controlled C/quiet/coated/nofoil original","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
   "platforms":{"us_virginia":{"source":"scenario_assumption: one mechanism tube, no real complete fit claim","wire_capacity":1,"reload_parallel":1,"tubes":[{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{}}},
   "orders":{"SHOOTER:us_mk48":{"source":"scenario_assumption: declared source setting and fixed depth","speed_index":speed_index,"depth_m":50,"enable_run_nm":enable,"bearing_deg":0,"cut_wire":true,"spread_deg":[],
    "solution":{"quality":"Good","source":"scenario_assumption: explicit author Good isolates physical budget, not sonar/FCS proof"}}}}},
  "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
   {"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":0,"course_deg":0,"depth_m":50,"fire_on":false},
   {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,distance],"speed_kts":target_speed,"course_deg":0,"depth_m":50,"fire_on":false}]
 })).unwrap();
    let mut world = load_scenario(scenario, db).unwrap();
    let shooter = world
        .entities
        .values()
        .find(|e| e.name == "SHOOTER")
        .unwrap()
        .id;
    let target = world
        .entities
        .values()
        .find(|e| e.name == "TARGET")
        .unwrap()
        .id;
    for e in world.entities.values_mut() {
        e.sensors.clear();
        if e.side == Side::Red {
            e.weapons.clear();
        }
    }
    world.sides[0].contacts.insert(
        ContactId(99),
        harpoon_core::world::Contact {
            id: ContactId(99),
            about: target,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., distance),
            est_course_deg: 0.,
            est_speed_kts: target_speed,
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
    world
        .rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert("0:2".into(), harpoon_core::types::PlatformKind::Sub);
    (world, shooter)
}
fn launch(world: &mut World, shooter: EntityId) -> EntityId {
    harpoon_core::sim::tick(
        world,
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
    let id = world
        .entities
        .values()
        .find(|e| e.kind == harpoon_core::types::PlatformKind::Torpedo)
        .expect("real source Fire must release")
        .id;
    id
}
fn details(world: &World, rule: &str) -> Vec<Value> {
    world
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule: r, detail, ..
            } if r == rule => Some(serde_json::from_str(detail).unwrap()),
            _ => None,
        })
        .collect()
}

/// The helper measures source endurance-distance, not planar chord length.
/// It never changes any native Flight, phase, fuel, die or HP.
fn check_last_tick(world: &mut World, torpedo: EntityId, wanted: fn(&Phase) -> bool) -> f64 {
    let mut saw_wanted = false;
    for _ in 0..3500 {
        let flight = &world.rules.torpedoes.as_ref().unwrap().flights[&torpedo];
        saw_wanted |= wanted(&flight.phase);
        let fuel = flight.fuel;
        let run = flight.run_nm;
        let range = flight.weapon.settings[flight.speed_index].range_nm;
        let speed = flight.weapon.settings[flight.speed_index].speed_kts;
        let affordable = (1. - fuel).max(0.) * range;
        let last = affordable < speed / 3600.;
        if last {
            assert!(
                wanted(&flight.phase),
                "final budget case must reach its declared native phase: {:?}",
                flight.phase
            );
        }
        harpoon_core::sim::tick(world, &[]);
        assert!(
            !world.rules.pending_adjudications.contains_key(&torpedo),
            "author source fixture must not hide an unrelated pending decision"
        );
        let after = &world.rules.torpedoes.as_ref().unwrap().flights[&torpedo];
        if last {
            assert!(saw_wanted);
            assert!(
                after.run_nm - run <= affordable + 1e-10,
                "remaining source budget {affordable}nm cannot pay for actual run increment {}nm",
                after.run_nm - run
            );
            assert!(
                (after.fuel - 1.).abs() < 1e-12,
                "spent fraction must stop at1, got {}",
                after.fuel
            );
            assert!(!world.entity(torpedo).is_some_and(|e| e.alive));
            assert_eq!(details(world, "10.2.source_runout").len(), 1);
            return after.fuel;
        }
    }
    panic!("native fixture did not reach a partial final tick");
}

#[test]
fn generic_initial_placement_failure_circle_cannot_overrun_source_endurance() {
    let (mut world, shooter) = fixture(source_seed(false, false), 5., 0., 0, 0.);
    let torpedo = launch(&mut world, shooter);
    check_last_tick(&mut world, torpedo, |p| matches!(p, Phase::Search { .. }));
    assert_eq!(details(&world, "10.2.4.placement").len(), 1);
    assert_eq!(
        details(&world, "10.2.4.placement")[0]["placement"]["placed"],
        false
    );
    assert!(details(&world, "10.2.5.source_terminal").is_empty());
}

#[test]
fn generic_failed_reattack_and_opportunity_circle_cannot_overrun_source_endurance() {
    let (mut world, shooter) = rejected_opportunity::starting();
    let torpedo = launch(&mut world, shooter);
    check_last_tick(&mut world, torpedo, |p| matches!(p, Phase::Search { .. }));
    assert_eq!(details(&world, "10.2.4.opportunity_placement").len(), 1);
    assert_eq!(
        details(&world, "10.2.4.opportunity_placement")[0]["placement"]["placed"],
        false
    );
    assert!(details(&world, "10.2.7.opportunity_acquired").is_empty());
    assert_eq!(details(&world, "10.2.5.source_terminal").len(), 1);
}

#[test]
fn generic_never_enabled_native_run_cannot_overrun_source_endurance() {
    // The real slow setting is40kt/22.6nm. A 20kt target initially22nm
    // away remains outside passive capture throughout the affordable run.
    let (mut world, shooter) = fixture(source_seed(true, false), 22., 20., 1, 22.6);
    let torpedo = launch(&mut world, shooter);
    check_last_tick(&mut world, torpedo, |p| matches!(p, Phase::Run));
    assert!(details(&world, "7.5.7.acquired").is_empty());
    assert!(details(&world, "10.2.5.source_terminal").is_empty());
}

#[test]
fn generic_native_first_miss_past_cannot_overrun_source_endurance() {
    // Legal14.7nm initial shot is within the true fast15nm setting; a
    // natural first miss leaves less range than source1000yd overshoot.
    let (mut world, shooter) = fixture(source_seed(true, false), 14.7, 0., 0, 0.);
    let torpedo = launch(&mut world, shooter);
    check_last_tick(&mut world, torpedo, |p| matches!(p, Phase::Past { .. }));
    let attacks = details(&world, "10.2.5.source_terminal");
    assert_eq!(attacks.len(), 1);
    assert_eq!(attacks[0]["hit"], false);
}

#[test]
fn generic_locked_static_target_requires_actual_arrival_before_native_terminal_die() {
    let distance = 0.1;
    let (mut world, shooter) = fixture(source_seed(true, true), distance, 0., 0, 0.);
    let torpedo = launch(&mut world, shooter);
    for _ in 0..100 {
        if !details(&world, "10.2.5.source_terminal").is_empty() {
            break;
        }
        harpoon_core::sim::tick(&mut world, &[]);
    }
    let attacks = details(&world, "10.2.5.source_terminal");
    assert_eq!(attacks.len(), 1, "actual native terminal must occur");
    assert_eq!(attacks[0]["hit"], true);
    let actual = world.rules.torpedoes.as_ref().unwrap().flights[&torpedo].run_nm;
    assert!(actual+1e-10>=distance,
  "native homing hit cannot consume less run({actual}) than the stationary same-plane separation({distance})");
}

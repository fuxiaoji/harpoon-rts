//! Registered after the prior batch was frozen. Initial four source failures
//! were witnessed at strict projector scenario loading, before combat assertions.
//! The laboratory hardware is explicit; native reports/FCS and source dice are
//! produced by the engine, not written into contacts or weapon results.
use harpoon_core::rules::detection2026;
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};

const ACTIVE: &str = "sonar_a_projector_lab";
const PASSIVE: &str = "projector_lab_passive";
const WEAPON: &str = "cn_rbu1000";

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn run(w: &mut World, until: i64) {
    while w.tick < until {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn fixture(seed: u64, sea: u8, wrong_arc: bool) -> World {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let mount = db.platforms["cn_956em"]
        .weapons
        .iter()
        .find(|m| m.weapon == WEAPON)
        .unwrap()
        .clone();
    assert_eq!((mount.mounts, mount.ammo), (2, 4));
    let mount_index = db.platforms["us_ford"].weapons.len();
    db.platforms.get_mut("us_ford").unwrap().weapons.push(mount);
    for (name, kind) in [
        (ACTIVE, harpoon_core::db::SensorKind::SonarActive),
        (PASSIVE, harpoon_core::db::SensorKind::SonarPassive),
    ] {
        let mut s = db.sensors.values().next().unwrap().clone();
        s.id = name.into();
        s.name = format!("scenario_assumption:{name}");
        s.kind = kind;
        s.air_range_nm = 0.;
        s.surface_range_nm = 0.;
        s.missile_range_nm = 0.;
        s.sub_range_nm = 16.;
        db.sensors.insert(name.into(), s);
    }
    db.platforms.get_mut("us_ford").unwrap().sensors = vec![ACTIVE.into(), PASSIVE.into()];
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    db.validate().unwrap();
    let receiver = |active| json!({"type":"sonar","source":"scenario_assumption: independent laboratory Gen5 LMF16nm Bow receiver, no real Ford fit claim","generation":5,"passive_range_nm":if active{None}else{Some(16.)},"active_range_nm":if active{Some(16.)}else{None},"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false});
    let value = json!({"id":"next-rgb10-pattern","name":"scenario_assumption: original source Ford CS6 hull and experimental RBU fitting","seed":seed,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{
        "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":sea,"precipitation":"None"},
        "asw_weapons_2026":{"source":"scenario_assumption: source RGB10 model, experimental original mount and stock placement","standoffs":{},"projectors":{"cn_rbu1000":{"source":"China2025 PDF75/E1 exact H-only RGB10 row","model":"rbu1000_rgb10","units":{"LAB":{"source":"scenario_assumption: original Ford hull DP/source CS6; original956EM two RBU mounts/aggregate8 stock; experiment1loaded+3reserve each; reload unspecified","mount_index":mount_index,"mounts":[{"physical":0,"arc":"P","loaded_patterns":1,"reserve_patterns":3},{"physical":1,"arc":"S","loaded_patterns":1,"reserve_patterns":3}],"reload":null}}}}},
        "detection_2026":{"bindings":{format!("us_ford:{ACTIVE}"):receiver(true),format!("us_ford:{PASSIVE}"):receiver(false)},"environment":{"source":"scenario_assumption: explicit open-water laboratory","sea_state":sea,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},"targets":{"cn_093":{"source":"scenario_assumption: controlled acoustic signature, original loaded hull DP retained","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
        "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"course_deg":0,"speed_kts":0,"sonar_active":true,"fire_on":false},{"side":"Red","class":"cn_093","name":"TARGET","pos":if wrong_arc{json!([0,0.3])}else{json!([0.3,0])},"course_deg":if wrong_arc{90}else{0},"speed_kts":0,"depth_m":50,"fire_on":false}]});
    let scenario: Scenario = serde_json::from_value(value)
        .expect("next-window source-red checkpoint: strict projector config must become a real engine configuration");
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let actor = id(&w, "LAB");
    let target = id(&w, "TARGET");
    // Acoustic trials cap at90%; a fixed author time cannot guarantee the
    // last epoch succeeded. Wait for actual consecutive native reports rather
    // than injecting success or changing the original deterministic dice.
    while w.tick < 6000 {
        let classified=detection2026::known_category(&w, Side::Blue, target)==Some(harpoon_core::types::PlatformKind::Sub);
        let located=w.sides[0].contacts.values().any(|c|c.about==target&&!c.stale&&!c.bearing_only);
        if classified && located && harpoon_core::rules::firecontrol2026::own_projector_solution(&w,actor,target,ACTIVE).is_ok_and(|s|s.quality==harpoon_core::rules::firecontrol2026::Quality::Good) {break;}
        harpoon_core::sim::tick(&mut w,&[]);
    }
    assert_eq!(
        detection2026::known_category(&w, Side::Blue, target),
        Some(harpoon_core::types::PlatformKind::Sub),
        "actual passive held-contact classification must precede the attack"
    );
    assert!(
        w.rules
            .aaw
            .as_ref()
            .unwrap()
            .firecontrol
            .tracks
            .values()
            .any(|t| {
                t.observer == actor
                    && t.target == target
                    && t.sensor == ACTIVE
                    && t.report
                        .as_ref()
                        .is_some_and(|r| r.measured_position.is_some())
                    && t.motion.as_ref().is_some_and(|m| m.known)
            }),
        "actual measured same-receiver motion is required; no author FCS quality"
    );
    w
}
fn contact(w: &World) -> ContactId {
    let target = id(w, "TARGET");
    w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == target && !c.stale && !c.bearing_only)
        .expect("actual native located CID required")
        .id
}
fn ammo(w: &World) -> u16 {
    w.entity(id(w, "LAB"))
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == WEAPON)
        .unwrap()
        .ammo
}
fn fire(w: &mut World) {
    let unit = id(w, "LAB");
    let contact = contact(w);
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::Fire {
                unit,
                contact,
                weapon: WEAPON.into(),
                rounds: 1,
            },
        )],
    );
}
fn results(w: &World) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "8.5.3.pattern_result" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
#[test]
fn p01_native_one_pattern_consumes_one_stock_token_and_one_original_die() {
    let mut w = fixture(20261025, 0, false);
    let before = ammo(&w);
    fire(&mut w);
    assert_eq!(
        ammo(&w),
        before - 1,
        "one stock token is one declared complete six-charge pattern"
    );
    let rs = results(&w);
    assert_eq!(rs.len(), 1);
    let r = &rs[0];
    assert_eq!(r["bombs"], 6);
    assert!(r["raw_d100"]
        .as_u64()
        .is_some_and(|d| (1..=100).contains(&d)));
    assert!(r["thresholds"]["contact"].is_null());
    assert_eq!(r["thresholds"]["major"], 11);
    assert_eq!(r["thresholds"]["minor"], 20);
    assert!(
        w.entities
            .values()
            .all(|e| e.projectile.as_ref().is_none_or(|p| p.weapon != WEAPON)),
        "no unsourced ballistic carrier"
    );
    let after = ammo(&w);
    fire(&mut w);
    assert_eq!(ammo(&w), after);
    assert_eq!(results(&w).len(), 1, "no same-Tac repeat or free reload");
}
#[test]
fn p02_natural_major_and_minor_use_real_dp_and_distinct_submarine_columns() {
    let mut seen = std::collections::BTreeSet::new();
    for seed in 1..=256 {
        let mut w = fixture(seed, 0, false);
        fire(&mut w);
        let rs = results(&w);
        assert_eq!(rs.len(), 1);
        let result = &rs[0];
        let Some(severity) = result["severity"].as_str() else {
            continue;
        };
        let (expected_dp, column) = match severity {
            "major" => (11., "sub_major"),
            "minor" => (6., "sub_minor"),
            "miss" => continue,
            other => panic!("H-only RGB10 cannot cause {other}"),
        };
        let attack = result["attack_id"]
            .as_str()
            .expect("stable actual pattern identity");
        let target = id(&w, "TARGET");
        assert!(w.events.iter().any(|e| matches!(&e.kind, EventKind::Impact { unit, damage, .. } if *unit == target && *damage == expected_dp)));
        assert!(
            w.events.iter().any(|e| match &e.kind {
                EventKind::RulesEvent { rule, detail, .. } if rule == "hit_resolved" => {
                    let v: Value = serde_json::from_str(detail).unwrap();
                    v["hit"] == attack && v["column"] == column && v["dp"] == expected_dp
                }
                _ => false,
            }),
            "actual shared damage consumer must retain the source severity"
        );
        seen.insert(severity.to_owned());
        if seen.len() == 2 {
            break;
        }
    }
    assert_eq!(
        seen,
        ["major".to_owned(), "minor".to_owned()]
            .into_iter()
            .collect()
    );
}
#[test]
fn p03_sea_seven_and_actual_wrong_source_arc_never_spend_a_pattern() {
    for (sea, wrong_arc) in [(7, false), (0, true)] {
        let mut w = fixture(20261025, sea, wrong_arc);
        let before = ammo(&w);
        fire(&mut w);
        assert_eq!(ammo(&w), before);
        assert!(results(&w).is_empty());
        let lab = id(&w, "LAB");
        assert!(!w.events.iter().any(
            |e| matches!(&e.kind, EventKind::WeaponsRelease { shooter, .. } if *shooter == lab)
        ));
    }
}
#[test]
fn p04_actual_pattern_stock_original_result_and_epoch_restore_exactly() {
    // Pick a naturally adjudicated miss so an unrelated catastrophic victim
    // critical cannot end this storage-only comparison. P02 covers real hits.
    let mut w = (1..=256)
        .find_map(|seed| {
            let mut w = fixture(seed, 0, false);
            fire(&mut w);
            let rs = results(&w);
            assert_eq!(rs.len(), 1, "nonempty source producer checkpoint");
            (rs[0]["severity"] == "miss").then_some(w)
        })
        .expect("a natural full-pattern miss is required; no die is supplied");
    let after = ammo(&w);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let to = w.tick + 400;
    run(&mut w, to);
    run(&mut restored, to);
    fire(&mut w);
    fire(&mut restored);
    assert_eq!(
        ammo(&w),
        after,
        "reserve stock is not automatically loaded without a source reload policy"
    );
    assert_eq!(results(&w).len(), 1);
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
}

#[test]
fn p05_native_projector_fcs_does_not_invent_a_torpedo_seeker() {
    let w = fixture(20261025, 0, false);
    let solution = harpoon_core::rules::firecontrol2026::own_projector_solution(
        &w, id(&w,"LAB"), id(&w,"TARGET"), ACTIVE,
    ).unwrap();
    assert!(!solution.modifiers.contains_key("torpedo_seeker"));
    let ordinary = harpoon_core::rules::firecontrol2026::own_sensor_solution(
        &w, id(&w,"LAB"), id(&w,"TARGET"), ACTIVE, 1, 40.,
    ).unwrap();
    assert_eq!(ordinary.modifiers["torpedo_seeker"], -1);
    assert_eq!(solution.contact_track, ordinary.contact_track + 1);
}

#[test]
fn p06_unsupported_multiple_patterns_reject_read_only_without_manual_takeover() {
    let w = fixture(20261025, 0, false);
    let before = serde_json::to_vec(&w).unwrap();
    let command = Command::Fire {unit:id(&w,"LAB"),contact:contact(&w),weapon:WEAPON.into(),rounds:2};
    assert!(command.validate_source(&w,Side::Blue).is_err());
    assert_eq!(serde_json::to_vec(&w).unwrap(),before);
}

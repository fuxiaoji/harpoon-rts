//! Unregistered candidate12. Native receiver scans/classification, genuine
//! failed entry and opportunity capture, then actual defender reaction orders.
//! Hardware overrides below are explicit scenario assumptions, not Annex fits.
use harpoon_core::rules::{aviation2026 as av, detection2026 as det, torpedo2026 as torp};
use harpoon_core::{
    Command, Database, EntityId, EventKind, Rng, Scenario, Side, Visibility, World,
};
use serde_json::{json, Value};

const RED_SENSOR: &str = "scenario_assumption_red_passive";

fn raw_die(seed: u64, window: i64, entity: u32, label: &str, sides: u8) -> u8 {
    let mut rng = Rng::new(seed).derive(Rng::hash_tag(window, entity, label));
    1 + (rng.next_f64() * f64::from(sides)) as u8
}

fn fixture() -> World {
    // Five initial bodies mean actual source launch6; runtime must verify this.
    // Seed selection is before load_scenario and never changes saved results.
    let seed = (1..=200_000)
        .find(|seed| {
            raw_die(*seed, 0, 0, "10.placement:6:0", 10) == 10
                && (65..=80).contains(&raw_die(*seed, 0, 6, "10.2.5.attack:0", 100))
                && [0, 1].into_iter().all(|window| {
                    ["sonar_p_hull", "sonar_a_hull"].into_iter().all(|sensor| {
                        raw_die(*seed, window, 0, &format!("5.4.6:4:{sensor}:2"), 100) <= 90
                    })
                })
                && [1, 2, 3].into_iter().all(|window| {
                    raw_die(*seed, window, 0, &format!("5.4.6:3:{RED_SENSOR}:6"), 100) <= 90
                })
        })
        .expect("native deterministic seed satisfying source trials exists");
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json"))
        .expect("consolidated source database");
    // Author mechanism hardware only. Do not describe these as real Type093
    // acceleration/turning rates or real fitted sensor performance.
    for hull in db.platforms.values_mut() {
        hull.sensors.clear();
    }
    let mut receiver = db.sensors["sonar_p_hull"].clone();
    receiver.id = RED_SENSOR.into();
    receiver.name = "scenario_assumption aft passive receiver".into();
    db.sensors.insert(receiver.id.clone(), receiver);
    let red = db.platforms.get_mut("cn_093").unwrap();
    red.accel_kts_per_s = 0.5;
    red.turn_rate_deg_s = 10.;
    red.sensors = vec![RED_SENSOR.into()];
    db.platforms.get_mut("us_burke_2a").unwrap().sensors =
        vec!["sonar_p_hull".into(), "sonar_a_hull".into()];
    let source_range = torp::acquisition_nm(4, true, 2, false, false, true).unwrap();
    // Actual20kt northbound movement until Fire tick181 places this candidate
    // just outside entry range. The30kt radial seeker catches it next tick;
    // its subsequent45kt chase lasts beyond the native classification window.
    let opportunity_initial_y = source_range + 0.001 - 20. / 3600. * 181.;
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"a1-native-defender-reaction-candidate12","name":"scenario_assumption native detection / A1 evasion regression",
        "duration_s":1500,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "aviation_2026":{"source":"US Aircraft2025 PDF69/B64 exact four-Mk54 BlockI load; mechanism fixture",
                "loads":{"us_mh60r_mk54x4":{"payload_kg":1140,"tanks":[],"external_weapon_rounds":{"us_mk54":4},
                    "source":"US Aircraft2025 PDF69/B64 and PDF93/E1 noteC"}},"parking":{},"initial_fuel_nm":{"HELO":200}},
            "detection_2026":{"bindings":{
                "us_burke_2a:sonar_p_hull":{"type":"sonar","source":"scenario_assumption: synthetic Gen5 LMF0.2nm bow passive, NOT real Burke fit",
                    "generation":5,"passive_range_nm":0.2,"active_range_nm":0.2,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false},
                "us_burke_2a:sonar_a_hull":{"type":"sonar","source":"scenario_assumption: synthetic Gen5 LMF0.2nm bow active, NOT real Burke fit",
                    "generation":5,"passive_range_nm":0.2,"active_range_nm":0.2,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false},
                "cn_093:scenario_assumption_red_passive":{"type":"sonar","source":"scenario_assumption: synthetic Gen5 LMF40nm aft passive receiver, NOT real Type093 fit",
                    "generation":5,"passive_range_nm":40,"active_range_nm":40,"frequency":"lmf","mount":"aft_digital","active_torpedo_detection":false,"high_speed_tow":false}},
                "targets":{
                    "cn_093":{"source":"scenario_assumption: controlled Quiet untreated submarine signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":null},
                    "us_mk54":{"source":"scenario_assumption: controlled physical size for passive mechanism;7.5.7 ordinary Noisy torpedo acoustic signature","size":7,"noise":1,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":null}},
                "environment":{"source":"scenario_assumption: clear deep no-layer mechanism water","sea_state":0,"precipitation":0,"visibility_percent":100,
                    "shipping":0,"biologics":0,"water_depth_m":1000,"layer":null,"ice":0,"cloud":null}},
            "torpedo_2026":{"source":"scenario_assumption: isolate postcapture reaction; no native airborne FCS claim",
                "environment":{"source":"scenario_assumption: no-layer nonreverberant water","layer_m":null,"reverberant":false},
                "targets":{"cn_093":{"source":"scenario_assumption: controlled Quiet uncoated targets","size":2,"noise":2,"anechoic":false,"foil_borne":false}},
                "orders":{"HELO:us_mk54":{"source":"scenario_assumption: own declared source order","speed_index":1,"depth_m":50,"enable_run_nm":0,
                    "bearing_deg":0,"spread_deg":[],"cut_wire":false,
                    "solution":{"quality":"Good","source":"scenario_assumption: author Good isolates A1 and reaction, not native aircraft FCS"}}}}},
        "units":[
            {"side":"Blue","class":"us_mh60r_mk54x4","name":"HELO","pos":[0,0],"course_deg":0,"speed_kts":0,"altitude_m":1000,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"ORIGINAL","pos":[0,0.05],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"OPPORTUNITY","pos":[0,opportunity_initial_y],"course_deg":0,"speed_kts":20,"depth_m":50,"fire_on":false},
            {"side":"Blue","class":"us_burke_2a","name":"NATIVE_REPORT_HELPER","pos":[0,0],"course_deg":0,"speed_kts":0,"sonar_active":true,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DISTANT_RET AINER","pos":[600,600],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    harpoon_core::load_scenario(scenario, db).unwrap()
}

fn named(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn flight(w: &World, id: EntityId) -> &torp::Flight {
    &w.rules.torpedoes.as_ref().unwrap().flights[&id]
}
fn tick(w: &mut World, orders: &[(Side, Command)]) {
    harpoon_core::sim::tick(w, orders);
}
fn close(a: f64, b: f64, why: &str) {
    assert!((a - b).abs() < 1e-9, "{why}: {a} != {b}");
}
fn native_scan_evidence(w: &World, observer: EntityId, target: EntityId, classified: bool) -> bool {
    w.events.iter().any(|event| match &event.kind {
        EventKind::RulesEvent {
            rule, unit, detail, ..
        } if rule == "5.scan" && *unit == Some(observer) => {
            let detail: Value = serde_json::from_str(detail).unwrap();
            detail["truth_target"] == json!(target)
                && detail["sensor"] == RED_SENSOR
                && detail["result"]["probability"]
                    .as_f64()
                    .is_some_and(|p| p > 0.)
                && (!classified
                    || detail["result"]["classification"]["level"]
                        .as_u64()
                        .is_some_and(|level| level >= 1))
        }
        _ => false,
    })
}

#[test]
fn native_classified_opportunity_reaction_uses_precommand_pose_and_the_source_evasion_column() {
    let mut w = fixture();
    let launcher = named(&w, "HELO");
    let original = named(&w, "ORIGINAL");
    let opportunity = named(&w, "OPPORTUNITY");
    let hardware = w.entity(opportunity).unwrap().class_def(&w.db).unwrap();
    close(
        hardware.accel_kts_per_s,
        0.5,
        "actual loaded author acceleration",
    );
    close(
        hardware.turn_rate_deg_s,
        10.,
        "actual loaded author turn rate",
    );
    assert!(w.sides.iter().all(|side| side.contacts.is_empty()));
    assert!(w.rules.detection.as_ref().unwrap().tracks.is_empty());
    assert!(w.rules.detection.as_ref().unwrap().categories.is_empty());
    tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Aviation {
                unit: launcher,
                action: av::Action::Throttle {
                    setting: av::Throttle::Hover,
                },
            },
        )],
    );
    while w.tick < 180 {
        tick(&mut w, &[]);
    }
    let contact = w.sides[Side::Blue.index()]
        .contacts
        .values()
        .find(|c| c.about == original && !c.stale && !c.bearing_only)
        .expect("native active sonar supplies located ORIGINAL report")
        .id;
    assert_eq!(
        det::known_category(&w, Side::Blue, original),
        Some(harpoon_core::types::PlatformKind::Sub)
    );
    assert!(w.rules.torpedoes.as_ref().unwrap().flights.is_empty());
    tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: launcher,
                contact,
                weapon: "us_mk54".into(),
                rounds: 1,
            },
        )],
    );
    assert_eq!(w.tick, 181);
    let id = EntityId(6);
    assert_eq!(
        w.stats.projectiles_launched, 1,
        "native source Fire released exactly once"
    );
    let p = flight(&w, id).placement.as_ref().unwrap();
    assert_eq!((p.d10, p.placed), (10, false));
    let dice = serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().placements).unwrap();
    assert!(
        !w.rules.pending_adjudications.contains_key(&id),
        "actual entry remains clear water"
    );
    tick(&mut w, &[]);
    let a = flight(&w, id)
        .acquisition
        .as_ref()
        .expect("actual unique coplanar opportunity must capture");
    assert_eq!(
        (a.original_target, a.target, a.owner),
        (original, opportunity, Side::Blue)
    );
    assert!(!w.rules.pending_adjudications.contains_key(&id));
    // Native passive scans must run in one window before classification in the
    // next. Never assign contacts, tracks, categories, trial results or time.
    while det::known_category(&w, Side::Red, id) != Some(harpoon_core::types::PlatformKind::Torpedo)
        && w.tick < 400
    {
        assert!(
            flight(&w, id)
                .acquisition
                .as_ref()
                .unwrap()
                .terminal
                .is_none(),
            "classification must precede real arrival"
        );
        tick(&mut w, &[]);
    }
    assert_eq!(
        det::known_category(&w, Side::Red, id),
        Some(harpoon_core::types::PlatformKind::Torpedo)
    );
    assert!(w.sides[Side::Red.index()]
        .contacts
        .values()
        .any(|c| c.about == id && !c.stale));
    assert!(native_scan_evidence(&w, opportunity, id, false));
    assert!(
        native_scan_evidence(&w, opportunity, id, true),
        "actual defending receiver classification evidence, not contact injection"
    );
    assert!(
        flight(&w, id)
            .acquisition
            .as_ref()
            .unwrap()
            .threat
            .is_none(),
        "first reaction still begins after the sensing phase"
    );
    let baseline = torp::Threat {
        course: w.entity(opportunity).unwrap().course_deg,
        speed: w.entity(opportunity).unwrap().speed_kts,
        depth_band: torp::depth_band(50.).unwrap(),
    };
    close(baseline.course, 0., "detected before reaction course");
    close(baseline.speed, 20., "detected before reaction speed");
    let target_hp = w.entity(opportunity).unwrap().hp;
    let reaction_at = w.tick;
    let before_reaction_world = serde_json::to_value(&w).unwrap();
    tick(
        &mut w,
        &[
            (
                Side::Red,
                Command::SetSpeed {
                    unit: opportunity,
                    speed_kts: 25.,
                },
            ),
            (
                Side::Red,
                Command::SetCourse {
                    unit: opportunity,
                    course_deg: 45.,
                },
            ),
        ],
    );
    assert_eq!(w.tick, reaction_at + 1);
    close(
        w.entity(opportunity).unwrap().speed_kts,
        20.5,
        "first native reaction acceleration",
    );
    close(
        w.entity(opportunity).unwrap().course_deg,
        10.,
        "first native reaction turn",
    );
    let saved_after_first_reaction = serde_json::to_value(
        flight(&w, id)
            .acquisition
            .as_ref()
            .unwrap()
            .threat
            .as_ref()
            .unwrap(),
    )
    .unwrap();
    let after_first_reaction_world = serde_json::to_value(&w).unwrap();
    for _ in 0..9 {
        tick(&mut w, &[]);
    }
    let reacted = w.entity(opportunity).unwrap();
    close(reacted.speed_kts, 25., "actual completed speed declaration");
    close(reacted.course_deg, 45., "actual completed turn declaration");
    close(
        reacted.depth_m,
        50.,
        "the entire native reaction remains coplanar",
    );
    assert!(reacted.speed_kts - baseline.speed >= 5. && reacted.speed_kts >= 20.);
    assert!(harpoon_core::geom::angle_diff(reacted.course_deg, baseline.course).abs() >= 45.);
    assert!(
        torp::evasion(&baseline, reacted, false),
        "both source conditions were actually achieved after detection"
    );
    for _ in 0..350 {
        if flight(&w, id)
            .acquisition
            .as_ref()
            .unwrap()
            .terminal
            .is_some()
        {
            break;
        }
        tick(&mut w, &[]);
    }
    let a = flight(&w, id).acquisition.as_ref().unwrap();
    let terminal = a
        .terminal
        .as_ref()
        .expect("a genuine physical first attack must occur after the completed reaction");
    assert!(
        (65..=80).contains(&terminal.d100),
        "pre-world chosen native source attack die"
    );
    assert_eq!(saved_after_first_reaction,json!({"course":0.,"speed":20.,"depth_band":torp::depth_band(50.).unwrap()}),
        "the actual detected opportunity needs its precommand/pre-Movement baseline; terminal={terminal:?}");
    assert!(terminal.evading);
    close(
        terminal.probability,
        0.64,
        "G4 source evasion-only column, not base0.80",
    );
    assert!(
        !terminal.hit,
        "the same unmodified D10065..80 must miss at0.64"
    );
    assert_eq!(terminal.acm_generation, None);
    close(
        w.entity(opportunity).unwrap().hp,
        target_hp,
        "a valid source evasion must not receive a manufactured base-column hit",
    );
    assert_eq!(
        serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().placements).unwrap(),
        dice
    );
    let saved_terminal = serde_json::to_value(terminal).unwrap();
    for _ in 0..10 {
        tick(&mut w, &[]);
    }
    assert_eq!(
        serde_json::to_value(&flight(&w, id).acquisition.as_ref().unwrap().terminal).unwrap(),
        saved_terminal
    );
    let terminals: Vec<_> = w
        .events
        .iter()
        .filter(
            |e| matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="10.2.5.source_terminal"),
        )
        .collect();
    assert_eq!(terminals.len(), 1);
    assert_eq!(terminals[0].vis, Visibility::Referee);

    // A save on either side of the sensing->reaction boundary must retain the
    // same private baseline, source miss and entire deterministic World.
    let expected = serde_json::to_value(&w).unwrap();
    for (checkpoint, reaction_due) in [
        (before_reaction_world, true),
        (after_first_reaction_world, false),
    ] {
        let mut restored: World = serde_json::from_value(checkpoint).unwrap();
        if reaction_due {
            tick(
                &mut restored,
                &[
                    (
                        Side::Red,
                        Command::SetSpeed {
                            unit: opportunity,
                            speed_kts: 25.,
                        },
                    ),
                    (
                        Side::Red,
                        Command::SetCourse {
                            unit: opportunity,
                            course_deg: 45.,
                        },
                    ),
                ],
            );
        }
        while restored.tick < w.tick {
            tick(&mut restored, &[]);
        }
        assert_eq!(serde_json::to_value(&restored).unwrap(), expected,
            "full World restore around native reaction must not replace the baseline or change the source miss");
    }
}

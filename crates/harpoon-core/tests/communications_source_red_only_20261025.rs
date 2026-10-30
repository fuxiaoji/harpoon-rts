//! source_red_only: candidate integration tests; root has not registered/run these.
//!
//! Authority: English HarpoonV2026 PDF74/6-6 and75/6-7, section6.3.9/10;
//! acoustic cue conditions additionally PDF72/6-4. No60/300s or0.9/0.5
//! source constants are invented here. The historical controls deliberately
//! retain those old RTS values without claiming they implement the source.
//!
//! Full fixtures create contacts and per-observer reports by genuine native
//! sim::tick -> sensors::update_sensors_v4, never by a Contact/FCS/report setter.
//! Lab hull aliases/sensors/signatures/environment are explicit authored fits,
//! not asserted real equipment. Original hull HP and weapon inventories remain.
//! No HP, die, source classification, Flight or communications state is injected.
//! Only Legacy/v3 controls use clearly adjudicated Contacts to isolate history.
use harpoon_core::rules::{damage_runtime::RuleProfile, firectl};
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};

const PASSIVE: &str = "lab_comms_passive_source";
const MARKER_PREFIXED: &str = "radar_air_lab_comms_name_only";
const MARKER_RENAMED: &str = "lab_comms_name_only_radar";
const SENDER: &str = "lab_comms_sender";
const PREFIX_HULL: &str = "lab_comms_marker_receiver";
const PLAIN_HULL: &str = "lab_comms_plain_receiver";
const GHOST_HULL: &str = "lab_comms_remote_unobserving_sonar";

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

fn lab_database(prefixed: bool) -> Database {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let mut passive = db.sensors.values().next().unwrap().clone();
    passive.id = PASSIVE.into();
    passive.name = "scenario_assumption: laboratory passive sonar, not real fit".into();
    passive.kind = harpoon_core::db::SensorKind::SonarPassive;
    passive.air_range_nm = 0.;
    passive.surface_range_nm = 0.;
    passive.sub_range_nm = 4.;
    passive.missile_range_nm = 0.;
    db.sensors.insert(PASSIVE.into(), passive);

    let marker = if prefixed {
        MARKER_PREFIXED
    } else {
        MARKER_RENAMED
    };
    let mut radar = db.sensors.values().next().unwrap().clone();
    radar.id = marker.into();
    radar.name =
        "scenario_assumption: name-only radar marker, zero ranges, no communications fit".into();
    radar.kind = harpoon_core::db::SensorKind::Radar;
    radar.air_range_nm = 0.;
    radar.surface_range_nm = 0.;
    radar.sub_range_nm = 0.;
    radar.missile_range_nm = 0.;
    db.sensors.insert(marker.into(), radar);

    for (key, sensors) in [
        (SENDER, vec![PASSIVE.to_string()]),
        (PREFIX_HULL, vec![marker.to_string()]),
        (PLAIN_HULL, vec![]),
        (GHOST_HULL, vec![PASSIVE.to_string()]),
    ] {
        let mut hull = db.platforms["us_burke_2a"].clone();
        hull.id = key.into();
        hull.name = format!("scenario_assumption: original-DP hull alias {key}");
        hull.sensors = sensors;
        // Original HP/weapons/rule_data are retained. No claim that the
        // synthetic hull carries an actual Annex communications system.
        db.platforms.insert(key.into(), hull);
    }
    db.validate().unwrap();
    db
}

fn world(db: Database, profile: &str, include_ghost: bool, seed: u64) -> World {
    let mut units = vec![
        json!({"side":"Blue","class":SENDER,"name":"SENDER","pos":[0,0],"course_deg":90,"speed_kts":0,"fire_on":false}),
        json!({"side":"Blue","class":PREFIX_HULL,"name":"PREFIX_RECEIVER","pos":[0,-1],"course_deg":90,"speed_kts":0,"fire_on":false}),
        json!({"side":"Blue","class":PLAIN_HULL,"name":"PLAIN_RECEIVER","pos":[0,-2],"course_deg":90,"speed_kts":0,"fire_on":false}),
        json!({"side":"Red","class":"cn_093","name":"TARGET","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}),
    ];
    if include_ghost {
        units.push(json!({"side":"Blue","class":GHOST_HULL,"name":"NO_REPORT_GHOST","pos":[200,0],"course_deg":90,"speed_kts":0,"fire_on":false}));
    }
    let receiver = json!({"type":"sonar","source":"scenario_assumption: Gen5 Bow LMF4nm50percent lab sonar",
        "generation":5,"passive_range_nm":4,"active_range_nm":null,"frequency":"lmf","mount":"bow",
        "active_torpedo_detection":false,"high_speed_tow":false});
    let bindings = [
        (format!("{SENDER}:{PASSIVE}"), receiver.clone()),
        (format!("{GHOST_HULL}:{PASSIVE}"), receiver),
    ]
    .into_iter()
    .collect::<std::collections::BTreeMap<_, _>>();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"communications-source-red-only-laboratory",
        "name":"scenario_assumption: no communications fit, native source detector and name-only receivers",
        "duration_s":10000,"seed":seed,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":profile,"radar_environment":{"sea_state":2,"precipitation":"None"},
            "detection_2026":{
                "environment":{"source":"scenario_assumption: explicit calm LMF open-water laboratory",
                    "sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
                    "water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
                "bindings":bindings,
                "targets":{"cn_093":{"source":"scenario_assumption: controlled target signature, not real full fit",
                    "size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,
                    "stealthy_ir":false,"radar_signature":2}}}},
        "units":units
    })).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let target = id(&w, "TARGET");
    // This is an authored passive target: it makes no observations of its
    // own. Its real source HP and inventory remain exactly scenario-loaded.
    w.entity_mut(target).unwrap().sensors.clear();
    assert!(w.sides[Side::Blue.index()].contacts.is_empty());
    w
}

fn native_fixture(include_ghost: bool, prefixed: bool) -> (World, ContactId, EntityId) {
    let db = lab_database(prefixed);
    // Select a seed only by running a real first native tick. No cached die
    // or hand-written detection/contact/report/classification is installed.
    for seed in 1..=128 {
        let mut w = world(db.clone(), "hv2026-rts-v4", include_ghost, seed);
        let sender = id(&w, "SENDER");
        let target = id(&w, "TARGET");
        let marker = id(&w, "PREFIX_RECEIVER");
        let plain = id(&w, "PLAIN_RECEIVER");
        harpoon_core::sim::tick(
            &mut w,
            &[
                (
                    Side::Blue,
                    Command::SetUnitAutomation {
                        unit: sender,
                        enabled: false,
                    },
                ),
                (
                    Side::Blue,
                    Command::SetUnitAutomation {
                        unit: marker,
                        enabled: false,
                    },
                ),
                (
                    Side::Blue,
                    Command::SetUnitAutomation {
                        unit: plain,
                        enabled: false,
                    },
                ),
            ],
        );
        let Some(contact) = w.sides[0].contacts.values().find(|c| c.about == target) else {
            continue;
        };
        let cid = contact.id;
        assert!(
            contact.bearing_only,
            "native passive geometry before any firectl pass on the new contact"
        );
        let reports = &w.rules.aaw.as_ref().unwrap().firecontrol.tracks;
        let report = reports
            .values()
            .find(|r| r.observer == sender && r.target == target)
            .and_then(|r| r.report.as_ref())
            .expect("genuine successful observer report retained");
        assert_eq!(
            report.sensor_kind,
            harpoon_core::db::SensorKind::SonarPassive
        );
        assert_eq!(report.at, 1);
        assert!(report.measured_position.is_none());
        for receiver in [marker, plain] {
            assert!(!reports
                .values()
                .any(|r| r.observer == receiver && r.target == target));
        }
        if include_ghost {
            let ghost = id(&w, "NO_REPORT_GHOST");
            assert!(
                w.entity(ghost)
                    .unwrap()
                    .pos
                    .range_to(w.entity(target).unwrap().pos)
                    > 199.
            );
            assert!(
                !reports
                    .values()
                    .any(|r| r.observer == ghost && r.target == target),
                "distant duplicate sensor is not a native contributor"
            );
        }
        assert_eq!(w.rules.profile, RuleProfile::Rts2026Full);
        return (w, cid, target);
    }
    panic!("native detector fixture never produced an actual source contact; this is fixture failure, not an accepted rule-red");
}

fn run_to(w: &mut World, at: i64) {
    while w.tick < at {
        assert!(
            !w.over,
            "laboratory must not silently end before the observation checkpoint"
        );
        harpoon_core::sim::tick(w, &[]);
    }
    assert_eq!(w.tick, at);
}

fn shadow_quality(w: &World, owner: EntityId, target: EntityId) -> Option<f64> {
    w.entity(owner)
        .unwrap()
        .ext
        .get(&format!("firectl.shadow.{}.q", target.0))
        .copied()
}

fn inventory_and_manual(w: &World) -> Value {
    json!(w.entities.values().filter(|e| e.side == Side::Blue).map(|e| {
        json!({"unit":e.id,"ammo":e.weapons.iter().map(|m|json!([m.weapon,m.ammo])).collect::<Vec<_>>(),
            "manual":e.ext.get("control.manual"),"fire_on":e.fire_on})
    }).collect::<Vec<_>>())
}

fn crossfix_events(w: &World) -> usize {
    w.events
        .iter()
        .filter(|e| {
            matches!(&e.kind,EventKind::RulesEvent{domain,rule,..}
        if domain=="firectl"&&rule=="cross_fix")
        })
        .count()
}

#[test]
fn a01_full_native_contact_must_not_auto_relay_at60_or_multiply_again_at300() {
    let (mut w, _, target) = native_fixture(false, true);
    let marker = id(&w, "PREFIX_RECEIVER");
    let plain = id(&w, "PLAIN_RECEIVER");
    let protected = inventory_and_manual(&w);
    run_to(&mut w, 59);
    let q_source = w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == target)
        .unwrap()
        .quality;
    run_to(&mut w, 60);
    let q60 = shadow_quality(&w, marker, target);
    run_to(&mut w, 300);
    let q300 = shadow_quality(&w, plain, target);
    assert_eq!(
        inventory_and_manual(&w),
        protected,
        "old sharing must never alter real ammo or native manual reservations"
    );
    // Confirm the obsolete values if this baseline path still executes;
    // after isolation there must simply be no old shadow to inspect.
    if let (Some(first), Some(second)) = (q60, q300) {
        assert!((first - q_source * 0.9).abs() < 1e-9);
        assert!((second - first * 0.5).abs() < 1e-9);
    }
    assert!(q60.is_none() && q300.is_none(),
        "6.3.9/10: no real link fit/message; Full native source q={q_source}, obsolete T60={q60:?}, T300={q300:?}; expected no auto shadow");
}

#[test]
fn a02_full_changing_only_radar_air_name_cannot_grant_a_data_link() {
    let (mut prefixed, _, target_a) = native_fixture(false, true);
    let (mut renamed, _, target_b) = native_fixture(false, false);
    assert_eq!(
        prefixed.seed, renamed.seed,
        "same detector and native trial purposes"
    );
    let receiver_a = id(&prefixed, "PREFIX_RECEIVER");
    let receiver_b = id(&renamed, "PREFIX_RECEIVER");
    assert_eq!(receiver_a, receiver_b);
    assert_eq!(target_a, target_b);
    run_to(&mut prefixed, 60);
    run_to(&mut renamed, 60);
    let a = shadow_quality(&prefixed, receiver_a, target_a);
    let b = shadow_quality(&renamed, receiver_b, target_b);
    assert!(
        b.is_none(),
        "renamed zero-range radar has no link proof either"
    );
    assert_eq!(a, b, "only class radar_air_* name changed; no actual link fit, observer report or communications action exists");
}

#[test]
fn a03_native_full_tick_must_not_credit_a_far_unit_without_its_own_report() {
    let (mut w, cid, target) = native_fixture(true, true);
    let ghost = id(&w, "NO_REPORT_GHOST");
    let protected = inventory_and_manual(&w);
    let marker = format!(
        "firectl.crossfix.{}.{}.{}",
        Side::Blue.index(),
        cid.0,
        w.contact(Side::Blue, cid).unwrap().first_tick
    );
    assert!(!w.entity(target).unwrap().ext.contains_key(&marker));
    harpoon_core::sim::tick(&mut w, &[]);
    assert!(!w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .any(|r| r.observer == ghost && r.target == target && r.report.is_some()));
    assert_eq!(inventory_and_manual(&w), protected);
    // The native scan later in this tick may correctly overwrite transient
    // q/bearing corruption. The bogus cross-fix reward marker persists and
    // proves the actual native rules pass credited a non-observer.
    assert!(!w.entity(target).unwrap().ext.contains_key(&marker),
        "actual native tick recorded a permanent crossfix reward for a distant unobserving duplicate sensor; events={}",crossfix_events(&w));
    assert_eq!(crossfix_events(&w), 0);
}

#[test]
fn a04_full_public_domain_pass_cannot_converge_to_truth_from_a_nonobserver() {
    let (mut w, cid, target) = native_fixture(true, true);
    let before = w.contact(Side::Blue, cid).unwrap().clone();
    assert!(before.bearing_only);
    assert!(
        before.estimate.range_to(w.entity(target).unwrap().pos) > 0.4,
        "native 1nm bearing anchor is not the target's1.5nm true position"
    );
    // Deliberate public domain-call isolation after genuine source scanning.
    // This case is not represented as a second full native sim tick.
    firectl::step(&mut w);
    let after = w.contact(Side::Blue, cid).unwrap();
    assert_eq!(
        (after.quality, after.bearing_only, after.estimate),
        (before.quality, before.bearing_only, before.estimate),
        "a mounted sensorID is not a second observed bearing or source-compliant passive cross-fix"
    );
}

fn adjudicated_contact(w: &mut World, passive: bool, quality: f64) -> (ContactId, EntityId) {
    let target = id(w, "TARGET");
    let cid = ContactId(91);
    let c = serde_json::from_value(
        json!({"id":cid,"about":target,"category":"Sub","class_name":null,
        "estimate":{"x":0,"y":1.0},"est_course_deg":0,"est_speed_kts":0,
        "bearing_only":passive,"quality":quality,"last_update_tick":0,"first_tick":0,"stale":false,
        "sensors":[if passive {PASSIVE} else {"adjudicated_legacy_radio_report"}],
        "offset":{"x":0,"y":0},"announced":true}),
    )
    .unwrap();
    w.sides[0].next_contact = cid.0;
    w.sides[0].contacts.insert(cid, c);
    (cid, target)
}

fn historical_shadow_control(profile: &str) {
    let mut w = world(lab_database(true), profile, false, 1);
    let (_, target) = adjudicated_contact(&mut w, false, 0.60);
    let marker = id(&w, "PREFIX_RECEIVER");
    let plain = id(&w, "PLAIN_RECEIVER");
    let protected = inventory_and_manual(&w);
    // Explicit referee-adjudicated picture, no claimed native report/FCS.
    w.tick = 60;
    firectl::step(&mut w);
    assert!((shadow_quality(&w, marker, target).unwrap() - 0.54).abs() < 1e-9);
    w.tick = 300;
    firectl::step(&mut w);
    assert!((shadow_quality(&w, plain, target).unwrap() - 0.27).abs() < 1e-9);
    assert_eq!(inventory_and_manual(&w), protected);
}

#[test]
fn a05_legacy_adjudicated_shadow_history_keeps_point54_then_point27() {
    historical_shadow_control("rts-legacy-v1");
}

#[test]
fn a06_v3_adjudicated_shadow_history_keeps_point54_then_point27() {
    historical_shadow_control("hv2026-rts-v3");
}

#[test]
fn a07_legacy_and_v3_adjudicated_crossfix_history_is_not_rewritten() {
    for profile in ["rts-legacy-v1", "hv2026-rts-v3"] {
        let mut w = world(lab_database(true), profile, true, 1);
        let (cid, _) = adjudicated_contact(&mut w, true, 0.30);
        w.tick = 1;
        firectl::step(&mut w);
        let c = w.contact(Side::Blue, cid).unwrap();
        assert!((c.quality - 0.40).abs() < 1e-9);
        assert!(!c.bearing_only);
        assert_eq!(crossfix_events(&w), 1);
        // Historical input is explicit adjudication. There are no source
        // observer reports, and this test does not make a source-native claim.
        assert!(w
            .rules
            .aaw
            .as_ref()
            .is_none_or(|a| a.firecontrol.tracks.is_empty()));
    }
}

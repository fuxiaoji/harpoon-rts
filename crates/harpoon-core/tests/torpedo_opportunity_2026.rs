//! Source PDF114/10-5 §10.2.7, source-corrected legacy cases.
//! Author Good and original located CID99 isolate post-release mechanisms;
//! they are not native FCS proof. No implicit IFF, depth teleport, target
//! priority, or original-target coating may be lent to a candidate.
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
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
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
#[test]
fn o1_failed_reacquisition_captures_moss_and_destroys_it_on_hit() {
    let (mut w, shooter, original) = fixture(source_seed(true), true);
    let torpedo = launch(&mut w, shooter);
    assert_eq!(torpedo.0, TORP);
    await_capture(&mut w, torpedo);
    let placements = details(&w, "10.2.4.opportunity_placement");
    assert_eq!(
        placements.len(),
        1,
        "same pass/candidate die is emitted once"
    );
    assert_eq!(placements[0]["candidate"], MOSS);
    assert_eq!(placements[0]["pass"], 1);
    assert_eq!(placements[0]["placement"]["chance"], 6);
    assert_eq!(placements[0]["placement"]["placed"], true);
    assert!(placements[0]["inputs"]["off_axis_deg"].as_f64().unwrap() <= 60.);
    assert_eq!(placements[0]["inputs"]["placement_modifier"], 0);
    assert_eq!(
        w.rules.torpedoes.as_ref().unwrap().flights[&torpedo].target,
        original,
        "capturing the MOSS must not overwrite the original release target"
    );
    finish_native_moss_hit(&mut w, torpedo);
    let terminal = details(&w, "10.2.5.source_terminal");
    assert_eq!(terminal.len(), 2, "initial miss and one native MOSS attack");
    assert_eq!(terminal[0]["hit"], false);
    let moss_attack = &terminal[1];
    assert_eq!(moss_attack["opportunity_kind"], "mobile_decoy");
    assert_eq!(moss_attack["hit"], true);
    assert_eq!(
        moss_attack["probability"], 0.64,
        "Gen4 vs Gen3 ACM-only column"
    );
    let own = harpoon_core::rules::torpedo2026::own_view(&w, Side::Blue, shooter).unwrap();
    assert_eq!(own["releases"][0]["status"], "unknown");
    assert!(own["releases"][0]["physical_alive"].is_null());
}

/// Source correction of legacy O2/O3: the original fixture actually created
/// two MOSS objects, not an ordinary entity. With two physically possible
/// candidates, no rule authorizes nearest/ID priority. Keep both real objects
/// and the torpedo intact, retain pending evidence, and draw zero candidate dice.
#[test]
fn o2_o3_two_real_moss_candidates_remain_pending_without_nearest_or_id_priority() {
    let seed = seed_for(|seed| {
        roll(seed, 0, "10.placement:5:0", 10) <= 7
            && roll(seed, 5, "10.2.5.attack:0", 100) > 64
            && roll(seed, 0, "10.placement:5:1", 10) > 4
    });
    let (mut w, shooter, original) = fixture(seed, true);
    use harpoon_core::rules::ew2026 as ew;
    ew::apply(
        &mut w,
        original,
        &ew::Action::Deploy {
            fit: "mobile".into(),
            at: None,
            radar_signature: None,
            course_deg: Some(100.),
            speed_kts: Some(12.),
            depth_m: Some(50.),
        },
    )
    .expect("second physical source-generation MOSS launch");
    assert_eq!(w.rules.ew.as_ref().unwrap().objects.len(), 2);
    let torpedo = launch(&mut w, shooter);
    assert_eq!(
        torpedo.0, 5,
        "both real decoys allocate IDs before the torpedo"
    );
    for _ in 0..2200 {
        harpoon_core::sim::tick(&mut w, &[]);
        if w.rules.pending_adjudications.contains_key(&torpedo) {
            break;
        }
    }
    assert!(w.rules.pending_adjudications.contains_key(&torpedo));
    let pending = details(&w, "10.2.7.opportunity_pending");
    assert_eq!(
        pending.len(),
        1,
        "real ambiguity must be recorded, not a launch failure"
    );
    assert_eq!(pending[0]["qualified"].as_array().unwrap().len(), 2);
    assert!(pending[0]["unknown"].as_array().unwrap().is_empty());
    let qualified = pending[0]["qualified"].as_array().unwrap();
    assert_ne!(
        qualified[0]["inputs"]["range_nm"], qualified[1]["inputs"]["range_nm"],
        "distinct real geometry must not become an invented nearest-first rule"
    );
    assert!(details(&w, "10.2.4.opportunity_placement").is_empty());
    assert!(details(&w, "10.2.7.opportunity_acquired").is_empty());
    assert!(w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .placements
        .placements
        .keys()
        .all(|identity| !identity.starts_with("10.placement:5:1:op:")));
    assert!(w.entity(torpedo).is_some_and(|e| e.alive));
    assert!(w
        .rules
        .ew
        .as_ref()
        .unwrap()
        .objects
        .values()
        .all(|obj| obj.status == ew::Status::Deployed));
    assert_eq!(
        details(&w, "10.2.5.source_terminal").len(),
        1,
        "only the original native miss"
    );
    for event in &w.events {
        if matches!(&event.kind, EventKind::RulesEvent { rule, .. } if rule == "10.2.7.opportunity_pending")
        {
            assert!(matches!(event.vis, harpoon_core::Visibility::Referee));
        }
    }
}

/// O4 remains a real failed-candidate/runout chain, with a mandatory candidate
/// die and no vacuous all() or scenario-over tautology. MOSS has Fair chance6,
/// never the original coated target's chance4.
#[test]
fn o4_all_candidates_rejected_keep_circling_until_runout() {
    let (mut w, shooter, _) = fixture(source_seed(false), true);
    let torpedo = launch(&mut w, shooter);
    for _ in 0..2200 {
        harpoon_core::sim::tick(&mut w, &[]);
        assert!(
            !w.rules.pending_adjudications.contains_key(&torpedo),
            "a single fully sourced failed candidate must keep searching"
        );
        if !details(&w, "10.2.source_runout").is_empty() {
            break;
        }
    }
    let ops = details(&w, "10.2.4.opportunity_placement");
    assert_eq!(
        ops.len(),
        1,
        "one real candidate was evaluated once, not zero"
    );
    assert_eq!(ops[0]["placement"]["chance"], 6);
    assert_eq!(ops[0]["placement"]["placed"], false);
    assert!(ops[0]["placement"]["d10"].as_u64().unwrap() > 6);
    assert!(details(&w, "10.2.7.opportunity_acquired").is_empty());
    let runout = details(&w, "10.2.source_runout");
    assert_eq!(
        runout.len(),
        1,
        "actual endurance exhaustion must terminate"
    );
    assert!(runout[0]["spent_fraction"].as_f64().unwrap() >= 1.);
    assert!(!w.entity(torpedo).is_some_and(|e| e.alive));
    assert_eq!(details(&w, "10.2.5.source_terminal").len(), 1);
    if let Some(flight) = w.rules.torpedoes.as_ref().unwrap().flights.get(&torpedo) {
        assert!(flight.fuel >= 1.);
        assert!(flight.opportunity.is_none());
    }
}

/// O5 preserves full-World continuation equality and exact-once MOSS
/// destruction at both capture and an actual mid-pursuit checkpoint.
#[test]
fn o5_restore_during_opportunity_pursuit_reproduces_terminal_state() {
    let (mut w, shooter, _) = fixture(source_seed(true), true);
    let torpedo = launch(&mut w, shooter);
    await_capture(&mut w, torpedo);
    let capture: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    capture.rules.validate_tables().unwrap();
    for _ in 0..5 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let mid: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let flight = &mid.rules.torpedoes.as_ref().unwrap().flights[&torpedo];
    let state = flight
        .opportunity
        .as_ref()
        .unwrap()
        .capture
        .as_ref()
        .unwrap();
    assert!(state.pursuit_started_at.is_some());
    assert!(
        state.terminal.is_none(),
        "mid-pursuit checkpoint precedes the terminal attack"
    );
    let mut reference = capture.clone();
    finish_native_moss_hit(&mut reference, torpedo);
    for checkpoint in [capture, mid] {
        let mut restored: World =
            serde_json::from_slice(&serde_json::to_vec(&checkpoint).unwrap()).unwrap();
        restored.rules.validate_tables().unwrap();
        finish_native_moss_hit(&mut restored, torpedo);
        assert_eq!(
            serde_json::to_value(&restored).unwrap(),
            serde_json::to_value(&reference).unwrap(),
            "complete restored World, private capture, original dice and terminal states must match"
        );
        assert_eq!(details(&restored, "10.2.7.opportunity_destroyed").len(), 1);
    }
}

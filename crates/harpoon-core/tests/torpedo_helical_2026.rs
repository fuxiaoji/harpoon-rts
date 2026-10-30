//! H1 source PDF79/7-4 plus parent's explicit radial/clock/depth RTS contract.
//! Actual MH60R BlockI/Mk54 Fire is exercised. The own report, Good solution,
//! water and target signature are authored mechanism inputs, not native Air FCS.
//! This target does not claim complete R054 acquisition/opportunity/reattack.
use harpoon_core::rules::{aviation2026 as av, torpedo2026 as torp};
use harpoon_core::{
    Command, ContactId, Database, EntityId, EventKind, Rng, Scenario, Side, Vec2, World,
};
use serde_json::{json, Value};
use std::sync::OnceLock;

fn fixture(depth: f64, enable: f64, authored_max: Option<f64>) -> (World, EntityId, EntityId) {
    static DB: OnceLock<Database> = OnceLock::new();
    let db = DB
        .get_or_init(|| {
            Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
        })
        .clone();
    let mut weapons = json!({});
    if let Some(maximum) = authored_max {
        // Explicit synthetic boundary profile. Keeping the carried ID isolates
        // search mechanics; this is not a claim about real Mk54's depth limit.
        let mut synthetic = torp::tables()["weapons"]["us_mk54"].clone();
        synthetic["source"] = json!(
            "scenario_assumption: synthetic H1 maximum depth boundary, not real Mk54 performance"
        );
        synthetic["max_depth_m"] = json!(maximum);
        weapons["us_mk54"] = synthetic;
    }
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"h1-actual-air-entry","name":"H1 explicit author mechanism trial","duration_s":2000,"seed":20261024,
        "context":{"rules_profile":"hv2026-rts-v4",
            "aviation_2026":{"source":"US Aircraft2025 PDF69/B64 exact BlockI four-Mk54 carriage; controlled search mechanism",
                "loads":{"us_mh60r_mk54x4":{"payload_kg":1140,"tanks":[],"external_weapon_rounds":{"us_mk54":4},
                    "source":"US Aircraft2025 PDF69/B64 and PDF93/E1 noteC"}},
                "parking":{},"initial_fuel_nm":{"HELO":200}},
            "detection_2026":{"environment":{"source":"scenario_assumption: deep no-layer physical H1 water",
                "sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
                "water_depth_m":1000,"layer":null,"ice":0,"cloud":null}},
            "torpedo_2026":{"source":"scenario_assumption: H1 source mechanism fixture, not a real full acoustic/FCS fit",
                "weapons":weapons,
                "environment":{"source":"scenario_assumption: no layer or reverberation","layer_m":null,"reverberant":false},
                "targets":{"cn_093":{"source":"scenario_assumption: controlled target metadata","size":2,"noise":2,"anechoic":false,"foil_borne":false}},
                "orders":{"HELO:us_mk54":{"source":"scenario_assumption: declared H1 mechanism order only",
                    "speed_index":0,"depth_m":depth,"enable_run_nm":enable,"bearing_deg":0,"spread_deg":[],"cut_wire":false,
                    "solution":{"quality":"Good","source":"scenario_assumption: isolates search after failed entry, not native own FCS"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_mh60r_mk54x4","name":"HELO","pos":[0,0],"speed_kts":0,"altitude_m":1000,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DISTANT_PRIMARY","pos":[0,10],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Blue","class":"us_ford","name":"scenario_assumption_blue_victory_retainer","pos":[500,500],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"scenario_assumption_red_victory_retainer","pos":[600,600],"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let h = w.entities.values().find(|e| e.name == "HELO").unwrap().id;
    let t = w
        .entities
        .values()
        .find(|e| e.name == "DISTANT_PRIMARY")
        .unwrap()
        .id;
    for e in w.entities.values_mut() {
        e.sensors.clear();
        // Explicit remote inert retainers prevent deleting the only launcher
        // or primary from ending the whole game and stopping its clock.
        if e.name.starts_with("scenario_assumption_") {
            e.weapons.clear();
        }
    }
    w.entity_mut(t).unwrap().weapons.clear();
    // The own located report is authored. The physical primary is deliberately
    // remote so a failed entry has no nearby acquisition/attack candidate.
    w.sides[0].contacts.insert(
        ContactId(71),
        harpoon_core::world::Contact {
            id: ContactId(71),
            about: t,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 0.25),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec!["scenario_assumption: located report".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert(format!("0:{}", t.0), harpoon_core::types::PlatformKind::Sub);
    av::apply(
        &mut w,
        h,
        &av::Action::Throttle {
            setting: av::Throttle::Hover,
        },
    )
    .unwrap();
    let id = EntityId(w.next_entity + 1);
    // Select a raw native boundary seed; never overwrite placement or dice.
    let seed = (1..=200_000)
        .find(|seed| {
            let rng = Rng::new(*seed);
            let mut trial = rng.derive(Rng::hash_tag(0, 0, &format!("10.placement:{}:0", id.0)));
            1 + (trial.next_f64() * 10.) as u8 == 10
        })
        .unwrap();
    w.seed = seed;
    w.rng = Rng::new(seed);
    (w, h, t)
}
fn release(w: &mut World, h: EntityId) -> EntityId {
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: h,
                contact: ContactId(71),
                weapon: "us_mk54".into(),
                rounds: 1,
            },
        )],
    );
    assert_eq!(
        w.stats.projectiles_launched, 1,
        "actual source Fire must release, not just construct a Flight"
    );
    let state = w.rules.torpedoes.as_ref().unwrap();
    let id = *state
        .launches
        .iter()
        .find(|(_, l)| l.shooter == h)
        .unwrap()
        .0;
    assert_eq!(
        w.entity(h)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "us_mk54")
            .unwrap()
            .ammo,
        3
    );
    id
}
fn ticks(w: &mut World, n: usize) {
    for _ in 0..n {
        harpoon_core::sim::tick(w, &[]);
    }
}
fn flight(w: &World, id: EntityId) -> &torp::Flight {
    &w.rules.torpedoes.as_ref().unwrap().flights[&id]
}
fn helical(w: &World, id: EntityId) -> Value {
    serde_json::to_value(flight(w, id)).unwrap()["helical"].clone()
}
fn placements(w: &World) -> Value {
    serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().placements).unwrap()
}
fn restore(w: &World) -> World {
    serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap()
}
fn yards(y: f64) -> f64 {
    y * torp::tables()["constants"]["yards_to_nm"].as_f64().unwrap()
}
fn close(a: f64, b: f64, why: &str) {
    assert!((a - b).abs() < 1e-9, "{why}: {a} != {b}");
}
fn assert_no_attack(w: &World) {
    assert!(!w.events.iter().any(
        |e| matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="10.2.5.source_terminal")
    ));
}

#[test]
fn failed_actual_entry_starts_continuous_radial_search_and_retains_native_dice() {
    let (mut w, h, _) = fixture(50., 0., None);
    let id = release(&mut w, h);
    let p = flight(&w, id).placement.as_ref().unwrap();
    assert_eq!((p.tick, p.d10, p.placed), (1, 10, false));
    let dice = placements(&w);
    assert!(
        helical(&w, id).is_object(),
        "failed entry must save an actual H1 state, not a missing-field panic"
    );
    assert!(
        !w.rules.pending_adjudications.contains_key(&id),
        "empty-water H1 cannot be permanently held at entry"
    );
    assert_eq!(
        w.entity(id).unwrap().pos,
        Vec2::zero(),
        "entry tick must not teleport to the 75yd circumference"
    );
    ticks(&mut w, 1);
    let e = w.entity(id).unwrap();
    close(
        e.pos.x,
        0.,
        "radial follows saved declared heading, not failed D6 heading",
    );
    close(e.pos.y, 45. / 3600., "one real second of radial movement");
    close(e.depth_m, 50., "radial maintains declared Shallow depth");
    close(
        flight(&w, id).fuel,
        (45. / 3600.) / 6.,
        "radial endurance charge",
    );
    assert_eq!(flight(&w, id).pass, 0);
    assert_eq!(placements(&w), dice);
    assert_no_attack(&w);
}

#[test]
fn source_circle_uses_500yd_clock_fixed_entry_center_and_two_physical_bands() {
    let (mut w, h, _) = fixture(50., 0., None);
    let id = release(&mut w, h);
    ticks(&mut w, 23);
    let speed = 45. / 3600.;
    let radial_time = yards(75.) / speed;
    let turns = (23. - radial_time) / (yards(500.) / speed);
    assert!(turns > 1. && turns < 1.1);
    let expected = Vec2::zero().advanced(turns * 360., yards(75.));
    let e = w.entity(id).unwrap();
    close(e.pos.x, expected.x, "source centered right circle x");
    close(e.pos.y, expected.y, "source centered right circle y");
    close(
        e.pos.range_to(Vec2::zero()),
        yards(75.),
        "fixed entry radius",
    );
    close(
        e.course_deg,
        (90. + turns * 360.).rem_euclid(360.),
        "right-circle tangent",
    );
    // b=1 at50m; two continuous physical bands per circle. At b in[3,4],
    // interpolate source200..300m, not detection's coarsened Deep indices.
    close(
        e.depth_m,
        200. + (1. + 2. * turns - 3.) * 100.,
        "two-band RTS depth coordinate",
    );
    close(
        flight(&w, id).fuel,
        speed * 23. / 6.,
        "500yd circle endurance clock plus radial",
    );
    close(
        flight(&w, id).run_nm,
        yards(75.) + yards(150.) * std::f64::consts::PI * turns,
        "physical path is distinct from endurance cost",
    );
    assert!(!w.rules.pending_adjudications.contains_key(&id));
    assert_no_attack(&w);
}

#[test]
fn true_source_maximum_reflects_then_first_shallow_return_is_explicit_pending() {
    let (mut w, h, _) = fixture(50., 0., None);
    let id = release(&mut w, h);
    let mut maximum = 50f64;
    let mut last = 50.;
    let mut ascended = false;
    for _ in 0..130 {
        ticks(&mut w, 1);
        let depth = w.entity(id).unwrap().depth_m;
        assert!(
            depth <= 600. + 1e-9 && depth >= 50. - 1e-9,
            "source Mk54 600m ceiling / Shallow return"
        );
        maximum = maximum.max(depth);
        ascended |= maximum > 590. && depth < last;
        last = depth;
    }
    assert!(
        maximum > 590. && ascended,
        "actual down/up physics must run, not remain at Order.depth50"
    );
    close(last, 50., "first upward Shallow boundary");
    let pending = w
        .rules
        .pending_adjudications
        .get(&id)
        .expect("post-Shallow mode remains unimplemented");
    assert!(
        pending.rule.contains("Shallow") || pending.rule.contains("upper"),
        "wrong boundary: {}",
        pending.rule
    );
    assert!(helical(&w, id).is_object());
    assert_no_attack(&w);
}

#[test]
fn explicit_author_mid_band_maximum_reflects_without_rounding_or_free_endurance() {
    let (mut w, h, _) = fixture(50., 0., Some(350.));
    let id = release(&mut w, h);
    let mut peak = 50f64;
    let mut previous = 50.;
    let mut ascending = false;
    for _ in 0..48 {
        ticks(&mut w, 1);
        let depth = w.entity(id).unwrap().depth_m;
        assert!(
            depth <= 350. + 1e-9,
            "authored350m boundary must not round to400m"
        );
        peak = peak.max(depth);
        ascending |= peak > 340. && depth < previous;
        previous = depth;
    }
    assert!(
        peak > 340. && ascending,
        "partial physical band needs a real reflected trajectory"
    );
    close(
        flight(&w, id).fuel,
        (45. / 3600.) * 48. / 6.,
        "reflection preserves source clock/endurance",
    );
    assert!(!w.rules.pending_adjudications.contains_key(&id));
}

#[test]
fn physical_search_is_independent_of_primary_truth_metadata_and_launcher_takeover_or_removal() {
    let (mut baseline, h, t) = fixture(50., 0., None);
    let id = release(&mut baseline, h);
    let mut lost = baseline.clone();
    lost.entities.remove(&t);
    let mut unknown = baseline.clone();
    unknown.entity_mut(t).unwrap().class = "scenario_assumption_unknown_primary".into();
    unknown
        .rules
        .torpedoes
        .as_mut()
        .unwrap()
        .config
        .targets
        .clear();
    let mut removed = baseline.clone();
    removed.entities.remove(&h);
    let mut manual = baseline.clone();
    for step in 0..30 {
        ticks(&mut baseline, 1);
        ticks(&mut lost, 1);
        ticks(&mut unknown, 1);
        ticks(&mut removed, 1);
        if step == 0 {
            harpoon_core::sim::tick(
                &mut manual,
                &[(
                    Side::Blue,
                    Command::SetUnitAutomation {
                        unit: h,
                        enabled: false,
                    },
                )],
            );
        } else {
            ticks(&mut manual, 1);
        }
        for counter in [&lost, &unknown, &removed, &manual] {
            assert!(!counter.rules.pending_adjudications.contains_key(&id),"self-contained search must not require primary metadata/lifecycle or a live automatic launcher");
            assert_eq!(
                counter.entity(id).unwrap().pos,
                baseline.entity(id).unwrap().pos
            );
            close(
                counter.entity(id).unwrap().depth_m,
                baseline.entity(id).unwrap().depth_m,
                "independent depth",
            );
            assert_eq!(
                serde_json::to_value(flight(counter, id)).unwrap(),
                serde_json::to_value(flight(&baseline, id)).unwrap()
            );
        }
    }
    assert!(helical(&baseline, id).is_object());
    assert!(baseline.entity(id).unwrap().pos.range_to(Vec2::zero()) > 0.);
}

#[test]
fn live_search_survives_complete_world_restore_without_new_dice_or_public_physical_status() {
    let (mut w, h, _) = fixture(50., 0., None);
    let id = release(&mut w, h);
    let original = placements(&w);
    ticks(&mut w, 13);
    assert!(helical(&w, id).is_object());
    let checkpoint = w.entity(id).unwrap().pos;
    let mut copy = restore(&w);
    ticks(&mut w, 27);
    ticks(&mut copy, 27);
    assert_ne!(
        w.entity(id).unwrap().pos,
        checkpoint,
        "a saved H1 must actually continue"
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&copy).unwrap()
    );
    assert_eq!(placements(&w), original);
    for side in [Side::Blue, Side::Red] {
        let observed = harpoon_core::observe::observe(&w, side, 0, 1000);
        let public = serde_json::to_string(&observed).unwrap();
        for private in [
            "helical",
            "circle_turns",
            "band_coordinate",
            "radial_run_nm",
        ] {
            assert!(
                !public.contains(private),
                "private search state leaked to {side:?}: {private}"
            );
        }
    }
    let own = torp::own_view(&w, Side::Blue, h).unwrap();
    assert_eq!(own["releases"][0]["status"], "unknown");
    assert!(own["releases"][0]["physical_alive"].is_null());
    assert_no_attack(&w);
}

#[test]
fn unsupported_nonshallow_and_delayed_enable_stay_pending_without_claiming_a_rules_ban() {
    for (depth, enable) in [(100., 0.), (50., 0.25)] {
        let (mut w, h, _) = fixture(depth, enable, None);
        let id = release(&mut w, h);
        assert!(
            w.rules.pending_adjudications.contains_key(&id),
            "unsupported combination must retain a complete referee decision"
        );
        let before = serde_json::to_value(&w.rules.pending_adjudications[&id]).unwrap();
        let dice = placements(&w);
        let pos = w.entity(id).unwrap().pos;
        ticks(&mut w, 20);
        assert_eq!(
            serde_json::to_value(&w.rules.pending_adjudications[&id]).unwrap(),
            before
        );
        assert_eq!(placements(&w), dice);
        assert_eq!(w.entity(id).unwrap().pos, pos);
        assert_no_attack(&w);
    }
}

#[test]
fn an_unreported_eligible_opportunity_keeps_full_referee_pending_instead_of_selecting_or_hitting() {
    let (mut w, h, t) = fixture(50., 0., None);
    let id = release(&mut w, h);
    ticks(&mut w, 4);
    assert!(helical(&w, id).is_object());
    assert!(!w.rules.pending_adjudications.contains_key(&id));
    let dice = placements(&w);
    let body = w.entity(id).unwrap().clone();
    let mut candidate = w.entity(t).unwrap().clone();
    candidate.id = w.allocate_id();
    candidate.name = "scenario_assumption_unreported_opportunity".into();
    candidate.pos = body.pos.advanced(body.course_deg, 0.1);
    candidate.depth_m = body.depth_m;
    let candidate_id = candidate.id;
    let hp = candidate.hp;
    w.entities.insert(candidate_id, candidate);
    ticks(&mut w, 1);
    assert!(
        w.rules.pending_adjudications.contains_key(&id),
        "a physical opportunity requiring unresolved acquisition cannot be ignored"
    );
    assert_eq!(w.entity(candidate_id).unwrap().hp, hp);
    assert_eq!(placements(&w), dice);
    assert_eq!(flight(&w, id).pass, 0);
    assert_no_attack(&w);
    let pending = serde_json::to_value(&w.rules.pending_adjudications[&id]).unwrap();
    assert!(pending["projectile"].is_object());
    assert!(pending["evidence"]
        .as_array()
        .is_some_and(|events| !events.is_empty()));
    for side in [Side::Blue, Side::Red] {
        let visible = harpoon_core::observe::observe(&w, side, 0, 1000);
        assert!(!visible
            .events
            .iter()
            .any(|e| matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="10.pending")));
    }
}

#[test]
fn a_controlled_remaining_budget_runs_out_partway_through_a_circle_without_a_free_tick() {
    let (mut w, h, _) = fixture(50., 0., None);
    let id = release(&mut w, h);
    // Controlled used-endurance boundary, not an asserted80yd Mk54 range.
    w.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .flights
        .get_mut(&id)
        .unwrap()
        .fuel = 1. - yards(80.) / 6.;
    let dice = placements(&w);
    ticks(&mut w, 10);
    assert!(
        w.entity(id).is_none_or(|e| !e.alive),
        "no-candidate H1 must genuinely exhaust its remaining endurance"
    );
    assert!(!w.rules.pending_adjudications.contains_key(&id));
    close(
        flight(&w, id).fuel,
        1.,
        "retire at the budget boundary without overshoot",
    );
    close(
        flight(&w, id).run_nm,
        yards(75.) + yards(5.) * (150. * std::f64::consts::PI / 500.),
        "only affordable radial and partial physical circle",
    );
    assert_eq!(placements(&w), dice);
    assert_no_attack(&w);
}

#[test]
fn an_explicit_legacy_pending_save_never_auto_resumes_or_reinterprets_old_search() {
    let (mut w, h, t) = fixture(50., 0., None);
    let id = release(&mut w, h);
    // Controlled pre-H1 save shape: no helical object, original Search and
    // retained entry-placement evidence. This is not a new source release.
    let mut serialized = serde_json::to_value(&w).unwrap();
    serialized["rules"]["torpedoes"]["flights"][id.0.to_string()]
        .as_object_mut()
        .unwrap()
        .remove("helical");
    let mut old: World = serde_json::from_value(serialized).unwrap();
    old.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .flights
        .get_mut(&id)
        .unwrap()
        .phase = torp::Phase::Search {
        center: Vec2::zero(),
        heading: 0.,
        angle: 0.,
    };
    torp::defer(
        &mut old,
        id,
        t,
        "scenario_assumption: explicit old saved source-search decision, no authorized resume",
    );
    let pending = serde_json::to_value(&old.rules.pending_adjudications[&id]).unwrap();
    let old_flight = serde_json::to_value(flight(&old, id)).unwrap();
    let dice = placements(&old);
    let pos = old.entity(id).unwrap().pos;
    let mut copy = restore(&old);
    ticks(&mut old, 20);
    ticks(&mut copy, 20);
    assert_eq!(
        serde_json::to_value(&old).unwrap(),
        serde_json::to_value(&copy).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&old.rules.pending_adjudications[&id]).unwrap(),
        pending
    );
    assert_eq!(serde_json::to_value(flight(&old, id)).unwrap(), old_flight);
    assert_eq!(placements(&old), dice);
    assert_eq!(old.entity(id).unwrap().pos, pos);
    assert!(helical(&old, id).is_null());
}

#[test]
fn an_explicit_author_high_speed_candidate_cannot_cross_the_search_volume_between_endpoint_checks()
{
    let (mut w, h, t) = fixture(50., 0., None);
    let id = release(&mut w, h);
    let dice = placements(&w);
    let seeker = &flight(&w, id).weapon;
    let outer = torp::acquisition_nm(seeker.generation, true, 0, false, false, true)
        .unwrap()
        .max(torp::acquisition_nm(seeker.generation, false, 0, false, false, true).unwrap());

    // An explicitly fictional 2160kt mechanism body makes the whole swept
    // boundary observable with a one-second engine tick. It is not a claim
    // about a real submarine, Annex performance, or a source navigation fit.
    // A stationary native tick first registers the real starting position.
    // Then actual SetSpeed plus explicitly fictional 2160kt/s acceleration
    // crosses the acquisition outer circle with both endpoints still outside
    // the own-step padding. Never inject State.motion or its history. The
    // previous moving-spawn setup is preserved as a separate source snapshot.
    let class_id = "scenario_assumption_h1_swept_geometry_sub";
    let half_segment_nm = 0.3;
    let synthetic_speed = 2. * half_segment_nm * 3600.;
    let mut synthetic = w.db.platforms["cn_093"].clone();
    synthetic.id = class_id.into();
    synthetic.name = "scenario_assumption: synthetic H1 swept-volume boundary".into();
    synthetic.max_speed_kts = synthetic_speed;
    synthetic.cruise_speed_kts = synthetic_speed;
    synthetic.accel_kts_per_s = synthetic_speed;
    synthetic.sensors.clear();
    synthetic.weapons.clear();
    w.db.platforms.insert(class_id.into(), synthetic);
    let mut candidate = w.entity(t).unwrap().clone();
    candidate.id = w.allocate_id();
    candidate.class = class_id.into();
    candidate.name = "scenario_assumption_h1_high_speed_candidate".into();
    candidate.pos = Vec2::new(-half_segment_nm, outer + 0.01);
    candidate.course_deg = 90.;
    candidate.ordered_course_deg = 90.;
    candidate.speed_kts = 0.;
    candidate.ordered_speed_kts = 0.;
    candidate.depth_m = 50.;
    candidate.ordered_depth_m = 50.;
    candidate.sensors.clear();
    candidate.weapons.clear();
    let candidate_id = candidate.id;
    w.entities.insert(candidate_id, candidate);
    let state = w.rules.torpedoes.as_mut().unwrap();
    let mut profile = state.config.targets["cn_093"].clone();
    profile.source =
        "scenario_assumption: synthetic swept-volume candidate, not real hull performance".into();
    state.config.targets.insert(class_id.into(), profile);

    ticks(&mut w, 1);
    assert!(!w.rules.pending_adjudications.contains_key(&id));
    let candidate_from = w.entity(candidate_id).unwrap().pos;
    close(
        candidate_from.x,
        -half_segment_nm,
        "stationary native candidate history",
    );
    let body_from = w.entity(id).unwrap().pos;
    close(
        body_from.y,
        45. / 3600.,
        "H1 genuinely advanced before the crossing tick",
    );
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Red,
            Command::SetSpeed {
                unit: candidate_id,
                speed_kts: synthetic_speed,
            },
        )],
    );
    close(
        w.entity(candidate_id).unwrap().speed_kts,
        synthetic_speed,
        "actual SetSpeed and explicit author acceleration",
    );

    let state = w.rules.torpedoes.as_ref().unwrap();
    let motion = &state.motion[&candidate_id];
    assert_eq!(motion.tick, w.tick);
    assert_eq!(motion.from, candidate_from);
    assert_eq!(motion.to, w.entity(candidate_id).unwrap().pos);
    close(
        motion.to.x,
        half_segment_nm,
        "second native candidate movement",
    );
    let closest_on_candidate_segment = Vec2::new(body_from.x, motion.from.y);
    assert!(motion.from.x < body_from.x && motion.to.x > body_from.x);
    assert!(closest_on_candidate_segment.range_to(body_from) < outer);
    assert!(motion.to.range_to(body_from) > outer + 45. / 3600.);
    assert!(
        w.rules.pending_adjudications.contains_key(&id),
        "native candidate Motion crosses the possible acquisition volume; an outside endpoint cannot prove this whole H1 tick empty"
    );
    assert_eq!(placements(&w), dice);
    assert_eq!(flight(&w, id).pass, 0);
    assert_no_attack(&w);
}

//! Source-exact MH60R BlockI four-Mk54 fit. US Aircraft2025 PDF69/B64,
//! PDF93/E1 AnnexF noteC distinguishes285kg helo and292kg fixed-wing.
//! Placement quality/water/target properties are explicit controlled scenario
//! assumptions; these tests do not claim a full real MH60R FCS model.
use harpoon_core::rules::{aircombat2026 as ac, aviation2026 as av};
use harpoon_core::{Command, ContactId, Database, EntityId, Scenario, Side, Vec2, World};
use serde_json::json;

fn fixture(class: &str, rounds: u16, payload: f64) -> (World, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc:Scenario=serde_json::from_value(json!({"id":"mh60r-four-mk54","name":"Source exact helicopter external fit","duration_s":1000,"seed":20261023,
        "context":{"rules_profile":"hv2026-rts-v4","aviation_2026":{"source":"US Aircraft2025 B64/E1 exact four-Mk54 helo fit",
            "loads":{class:{"payload_kg":payload,"tanks":[],"external_weapon_rounds":{"us_mk54":rounds},"source":"US Aircraft2025 PDF69/B64 four Mk54, PDF93/E1 noteC285kg helicopter version, no HAAWC/Mod2"}},
            "parking":{},"initial_fuel_nm":{"HELO":200}},
            "torpedo_2026":{"source":"scenario_assumption: controlled placement and water",
                "environment":{"source":"scenario_assumption: no layer or reverberation","layer_m":null,"reverberant":false},
                "targets":{"cn_093":{"source":"scenario_assumption: controlled target","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
                "orders":{"HELO:us_mk54":{"source":"scenario_assumption","speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":null,"spread_deg":[],"cut_wire":true,"solution":{"quality":"Good","source":"scenario_assumption: controlled source mechanism placement quality"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":class,"name":"HELO","pos":[0,0],"speed_kts":0,"altitude_m":1000,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"SUB","pos":[0,1],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let h = w.entities.values().find(|e| e.name == "HELO").unwrap().id;
    let t = w.entities.values().find(|e| e.name == "SUB").unwrap().id;
    w.entity_mut(t).unwrap().weapons.clear();
    w.sides[0].contacts.insert(
        ContactId(71),
        harpoon_core::world::Contact {
            id: ContactId(71),
            about: t,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 1.),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec!["sonar_dipping".into()],
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
    (w, h, t)
}
fn ammo(w: &World, h: EntityId) -> u16 {
    w.entity(h)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_mk54")
        .unwrap()
        .ammo
}
fn payload(w: &World, h: EntityId) -> f64 {
    w.rules.aviation.as_ref().unwrap().flights[&h].payload_fraction * 1480.
}

#[test]
fn real_four_mk54_fit_releases_one_helo_weight_once_then_jettisons_remaining_without_attack_and_replays(
) {
    let (mut w, h, t) = fixture("us_mh60r_mk54x4", 4, 1140.);
    // scenario_assumption: an isolated, noncombat Red reserve keeps this
    // external-store lifecycle trial running after the first immediate hit.
    // kill_count_at_end alone does not disable normal annihilation victory.
    let mut reserve = w.entity(t).unwrap().clone();
    reserve.id = w.allocate_id();
    reserve.name = "scenario_assumption: remote lifecycle-trial reserve".into();
    reserve.pos = Vec2::new(1000., 1000.);
    reserve.sensors.clear();
    reserve.weapons.clear();
    reserve.fire_on = false;
    w.entities.insert(reserve.id, reserve);
    assert!(
        w.rules.aviation.as_ref().unwrap().flights.contains_key(&h),
        "source-exact four285kg torpedoes must initialize the declared1140kg external mass"
    );
    assert_eq!(ammo(&w, h), 4);
    assert!((payload(&w, h) - 1140.).abs() < 1e-9);
    av::apply(
        &mut w,
        h,
        &av::Action::Throttle {
            setting: av::Throttle::Hover,
        },
    )
    .unwrap();
    let mut resumed: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for world in [&mut w, &mut resumed] {
        harpoon_core::sim::tick(
            world,
            &[(
                Side::Blue,
                Command::Fire {
                    unit: h,
                    contact: ContactId(71),
                    weapon: "us_mk54".into(),
                    rounds: 2,
                },
            )],
        );
    }
    assert!(
        !w.over,
        "the authored isolated reserve must keep a real next tick available for jettison"
    );
    assert_eq!(
        w.stats.projectiles_launched, 1,
        "9.7.1 actual two-round request still drops one"
    );
    assert_eq!(ammo(&w, h), 3);
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&h].external_weapon_rounds["us_mk54"],
        3
    );
    assert!(
        (payload(&w, h) - 855.).abs() < 1e-9,
        "one actual helicopter Mk54 release deducts285kg, never fixed-wing292 or warhead45"
    );
    let action = av::Action::Jettison {
        weapon: "us_mk54".into(),
        rounds: 3,
    };
    let mut direct = w.clone();
    let hp = direct.entity(t).unwrap().hp;
    let launched = direct.stats.projectiles_launched;
    av::apply(&mut direct, h, &action).unwrap();
    assert_eq!(direct.entity(t).unwrap().hp, hp);
    assert_eq!(direct.stats.projectiles_launched, launched);
    for world in [&mut w, &mut resumed] {
        harpoon_core::sim::tick(
            world,
            &[(
                Side::Blue,
                Command::Aviation {
                    unit: h,
                    action: action.clone(),
                },
            )],
        );
    }
    assert_eq!(ammo(&w, h), 0);
    assert!(payload(&w, h).abs() < 1e-9);
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&h].external_weapon_rounds["us_mk54"],
        0
    );
    assert_eq!(
        w.stats.projectiles_launched, 1,
        "jettison creates no additional attacking projectile"
    );
    assert_eq!(ac::target_maneuver(&w, h).unwrap(), 2.);
    assert_eq!(
        serde_json::to_value(w).unwrap(),
        serde_json::to_value(resumed).unwrap()
    );
}

#[test]
fn rejected_exact_mk54_release_preserves_ammo_and_external_mass() {
    for high in [true, false] {
        let (mut w, h, _) = fixture("us_mh60r_mk54x4", 4, 1140.);
        assert!(
            w.rules.aviation.as_ref().unwrap().flights.contains_key(&h),
            "exact source payload must initialize before release boundary"
        );
        if high {
            let e = w.entity_mut(h).unwrap();
            e.altitude_m = 2500.;
            e.ordered_altitude_m = 2500.;
        } else {
            av::apply(
                &mut w,
                h,
                &av::Action::Throttle {
                    setting: av::Throttle::Fmp,
                },
            )
            .unwrap();
            w.entity_mut(h).unwrap().speed_kts = 140.;
        }
        harpoon_core::sim::tick(
            &mut w,
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
        assert_eq!(w.stats.projectiles_launched, 0);
        assert_eq!(ammo(&w, h), 4);
        assert!((payload(&w, h) - 1140.).abs() < 1e-9);
        assert_eq!(
            w.rules.aviation.as_ref().unwrap().flights[&h].external_weapon_rounds["us_mk54"],
            4
        );
    }
}

#[test]
fn generic_two_round_host_or_incomplete_external_declaration_does_not_inherit_exact_four_round_fit()
{
    for (class, rounds, mass) in [("us_mh60r", 2, 570.), ("us_mh60r_mk54x4", 3, 855.)] {
        let (w, h, _) = fixture(class, rounds, mass);
        assert!(
            !w.rules.aviation.as_ref().unwrap().flights.contains_key(&h),
            "unknown host fit or mixed/internal mapping must remain blocked"
        );
    }
}

#[test]
fn invalid_external_fit_cannot_bypass_the_blocked_flight_mass_by_using_real_fire() {
    for (class, rounds, mass) in [("us_mh60r", 2, 570.), ("us_mh60r_mk54x4", 3, 855.)] {
        let (mut w, h, _) = fixture(class, rounds, mass);
        assert!(!w.rules.aviation.as_ref().unwrap().flights.contains_key(&h));
        let before = ammo(&w, h);
        harpoon_core::sim::tick(
            &mut w,
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
        assert_eq!(w.stats.projectiles_launched,0,"failed explicit external-fit validation must block the real aerial release, not merely suppress its mass ledger");
        assert_eq!(ammo(&w, h), before);
    }
}

// Real own-contact admission counterfactuals, after the frozen following batch.
// Book source: English2026 PDF109/9-8 and PDF74/6-6; own contact/quality
// are explicit controlled scenario evidence, never inferred from true body.

fn admission_fixture() -> (World, EntityId, EntityId) {
    let (mut w, h, t) = fixture("us_mh60r_mk54x4", 4, 1140.);
    av::apply(
        &mut w,
        h,
        &av::Action::Throttle {
            setting: av::Throttle::Hover,
        },
    )
    .unwrap();
    for e in w.entities.values_mut() {
        e.sensors.clear();
        e.fire_on = false;
    }
    // Keep one unrelated opponent alive in every counterfactual so victory
    // checks do not announce destruction of all hostile platforms.
    let mut survivor = w.entity(t).unwrap().clone();
    survivor.id = w.allocate_id();
    survivor.name = "UNRELATED-SILENT-SUB".into();
    survivor.pos = Vec2::new(1000., 1000.);
    survivor.weapons.clear();
    survivor.sensors.clear();
    w.entities.insert(survivor.id, survivor);
    (w, h, t)
}
fn admission_receipts(w: &World) -> Vec<serde_json::Value> {
    w.events
        .iter()
        .filter(|e| e.vis == harpoon_core::Visibility::Side(Side::Blue))
        .filter_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent { domain, rule, .. }
                if domain == "command" || rule == "9.7.1.torpedo_entered_water" =>
            {
                Some(serde_json::to_value(&e.kind).unwrap())
            }
            harpoon_core::EventKind::WeaponsRelease {
                side: Side::Blue, ..
            } => Some(serde_json::to_value(&e.kind).unwrap()),
            _ => None,
        })
        .collect()
}

fn assert_manual_hidden_admission(hidden_case: &str) {
    let (template, h, t) = admission_fixture();
    let known = serde_json::to_value(&template.sides[0].contacts).unwrap();
    let mut receipts = vec![];
    for hidden in ["live-submerged", hidden_case] {
        let mut w = template.clone();
        let body = w.entity_mut(t).unwrap();
        match hidden {
            "unobserved-death" => {
                body.alive = false;
                body.hp = 0.;
            }
            "unobserved-surfacing" => {
                body.depth_m = 0.;
                body.ordered_depth_m = 0.;
            }
            // Exact class remains unknown to Blue; only the referee acoustic
            // table binding differs. No new target acoustic values are guessed.
            "unknown-acoustic-profile" => {
                body.class = "cn_094".into();
                body.rule_data = None;
            }
            _ => {}
        }
        assert_eq!(known, serde_json::to_value(&w.sides[0].contacts).unwrap());
        let mut resumed: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        let fire = Command::Fire {
            unit: h,
            contact: ContactId(71),
            weapon: "us_mk54".into(),
            rounds: 1,
        };
        for world in [&mut w, &mut resumed] {
            harpoon_core::sim::tick(world, &[(Side::Blue, fire.clone())]);
        }
        assert_eq!(
            w.stats.projectiles_launched, 1,
            "{hidden}: same valid own release cannot query referee body state"
        );
        assert_eq!(ammo(&w, h), 3);
        assert_eq!(
            w.rules.aviation.as_ref().unwrap().flights[&h].external_weapon_rounds["us_mk54"],
            3
        );
        assert!((payload(&w, h) - 855.).abs() < 1e-9);
        receipts.push(admission_receipts(&w));
        assert_eq!(
            serde_json::to_value(w).unwrap(),
            serde_json::to_value(resumed).unwrap()
        );
    }
    assert!(
        receipts.windows(2).all(|p| p[0] == p[1]),
        "own command/release receipts must not disclose hidden target applicability"
    );
}

#[test]
fn real_airborne_fire_does_not_probe_hidden_alive() {
    assert_manual_hidden_admission("unobserved-death");
}
#[test]
fn real_airborne_fire_does_not_probe_hidden_depth() {
    assert_manual_hidden_admission("unobserved-surfacing");
}
#[test]
fn real_airborne_fire_does_not_probe_hidden_target_profile() {
    assert_manual_hidden_admission("unknown-acoustic-profile");
}

#[test]
fn source_airborne_release_still_requires_own_current_located_classified_sub_evidence() {
    for invalid in [
        "missing-contact",
        "stale",
        "bearing-only",
        "unknown-category",
        "wrong-category",
        "distant-report-without-source-fcs",
        "missing-source-fcs",
    ] {
        let (mut w, h, t) = admission_fixture();
        invalidate_own_asw_evidence(&mut w, t, invalid);
        harpoon_core::sim::tick(
            &mut w,
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
            w.stats.projectiles_launched, 0,
            "{invalid}: removing truth probes must preserve own evidence gates"
        );
        assert_eq!(ammo(&w, h), 4);
        assert!((payload(&w, h) - 1140.).abs() < 1e-9);
    }
}

fn invalidate_own_asw_evidence(w: &mut World, t: EntityId, invalid: &str) {
    match invalid {
        "missing-contact" => {
            w.sides[0].contacts.remove(&ContactId(71));
        }
        "stale" => w.sides[0].contacts.get_mut(&ContactId(71)).unwrap().stale = true,
        "bearing-only" => {
            w.sides[0]
                .contacts
                .get_mut(&ContactId(71))
                .unwrap()
                .bearing_only = true
        }
        "unknown-category" => {
            w.rules
                .detection
                .as_mut()
                .unwrap()
                .categories
                .remove(&format!("0:{}", t.0));
        }
        "wrong-category" => {
            w.rules.detection.as_mut().unwrap().categories.insert(
                format!("0:{}", t.0),
                harpoon_core::types::PlatformKind::Ship,
            );
        }
        "distant-report-without-source-fcs" => {
            w.sides[0]
                .contacts
                .get_mut(&ContactId(71))
                .unwrap()
                .estimate = Vec2::new(0., 1000.);
            // Section10.2 limits actual run from the launch datum, not the
            // initial reported target-center distance. This negative fails
            // for missing source FCS, without inventing a range threshold.
            invalidate_own_asw_evidence(w, t, "missing-source-fcs");
        }
        "missing-source-fcs" => {
            w.rules
                .torpedoes
                .as_mut()
                .unwrap()
                .config
                .orders
                .get_mut("HELO:us_mk54")
                .unwrap()
                .solution = None;
            w.rules.aaw.as_mut().unwrap().firecontrol.tracks.clear();
        }
        _ => panic!("unknown own-evidence fixture"),
    }
}
fn change_hidden_asw_body(w: &mut World, t: EntityId, hidden: &str) {
    let body = w.entity_mut(t).unwrap();
    match hidden {
        "unobserved-death" => {
            body.alive = false;
            body.hp = 0.;
        }
        "unobserved-surfacing" => {
            body.depth_m = 0.;
            body.ordered_depth_m = 0.;
        }
        "unknown-acoustic-profile" => {
            body.class = "cn_094".into();
            body.rule_data = None;
        }
        "live-submerged" => {}
        _ => panic!("unknown hidden-body fixture"),
    }
}
fn set_asw_auto_authority(w: &mut World, h: EntityId, mode: &str) {
    let e = w.entity_mut(h).unwrap();
    e.assigned = (mode == "assigned").then_some(ContactId(71));
    e.fire_on = mode == "weapons-free";
}

fn assert_auto_hidden_admission(hidden_case: &str) {
    for mode in ["assigned", "weapons-free"] {
        let (template, h, t) = admission_fixture();
        let mut feedback = vec![];
        for hidden in ["live-submerged", hidden_case] {
            let mut w = template.clone();
            change_hidden_asw_body(&mut w, t, hidden);
            set_asw_auto_authority(&mut w, h, mode);
            let mut resumed: World =
                serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
            for world in [&mut w, &mut resumed] {
                harpoon_core::sim::tick(world, &[]);
            }
            assert_eq!(
                w.stats.projectiles_launched, 1,
                "{mode}/{hidden}: automatic selection must not query true alive/depth/class"
            );
            assert_eq!(ammo(&w, h), 3);
            assert!((payload(&w, h) - 855.).abs() < 1e-9);
            feedback.push(admission_receipts(&w));
            assert_eq!(
                serde_json::to_value(w).unwrap(),
                serde_json::to_value(resumed).unwrap()
            );
        }
        assert!(
            feedback.windows(2).all(|p| p[0] == p[1]),
            "{mode}: same own release cannot disclose hidden applicability"
        );
    }
}

#[test]
fn real_automatic_airborne_drop_does_not_probe_hidden_alive() {
    assert_auto_hidden_admission("unobserved-death");
}
#[test]
fn real_automatic_airborne_drop_does_not_probe_hidden_depth() {
    assert_auto_hidden_admission("unobserved-surfacing");
}
#[test]
fn real_automatic_airborne_drop_does_not_probe_hidden_target_profile() {
    assert_auto_hidden_admission("unknown-acoustic-profile");
}

#[test]
fn automatic_airborne_drop_preserves_own_evidence_fcs_and_fire_authority_gates() {
    for mode in ["assigned", "weapons-free"] {
        for invalid in [
            "missing-contact",
            "stale",
            "bearing-only",
            "unknown-category",
            "wrong-category",
            "distant-report-without-source-fcs",
            "missing-source-fcs",
        ] {
            let (mut w, h, t) = admission_fixture();
            set_asw_auto_authority(&mut w, h, mode);
            invalidate_own_asw_evidence(&mut w, t, invalid);
            harpoon_core::sim::tick(&mut w, &[]);
            assert_eq!(
                w.stats.projectiles_launched, 0,
                "{mode}/{invalid}: source admission must retain own legal constraints"
            );
            assert_eq!(ammo(&w, h), 4);
            assert!((payload(&w, h) - 1140.).abs() < 1e-9);
        }
    }
    let (mut w, h, _) = admission_fixture();
    set_asw_auto_authority(&mut w, h, "hold");
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(w.stats.projectiles_launched, 0);
    assert_eq!(ammo(&w, h), 4);
}

#[test]
fn actual_mk54_acquisition_never_damages_a_hidden_surface_target_or_guesses_unknown_acoustics() {
    for hidden in ["unobserved-surfacing", "unknown-acoustic-profile"] {
        let (mut w, h, t) = admission_fixture();
        change_hidden_asw_body(&mut w, t, hidden);
        // Put physical truth inside the source150yd acquisition region while
        // keeping the same own located Sub report. This tests terminal domain,
        // not only a helper that could miss for unrelated geometry.
        w.entity_mut(t).unwrap().pos = Vec2::new(0., 0.02);
        let hp = w.entity(t).unwrap().hp;
        let mut resumed: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        let order = Command::Fire {
            unit: h,
            contact: ContactId(71),
            weapon: "us_mk54".into(),
            rounds: 1,
        };
        for world in [&mut w, &mut resumed] {
            harpoon_core::sim::tick(world, &[(Side::Blue, order.clone())]);
        }
        assert_eq!(
            w.stats.projectiles_launched, 1,
            "{hidden}: legitimate source release is separate from physical applicability"
        );
        for _ in 0..600 {
            harpoon_core::sim::tick(&mut w, &[]);
            harpoon_core::sim::tick(&mut resumed, &[]);
        }
        assert_eq!(
            w.entity(t).unwrap().hp,
            hp,
            "{hidden}: no Mk54 ship hit or invented target acoustic parameters"
        );
        if hidden == "unknown-acoustic-profile" {
            assert!(
                w.rules
                    .pending_adjudications
                    .values()
                    .any(|p| p.target == t && p.rule.starts_with("10.source_torpedo")),
                "unknown true acoustic profile must retain complete referee pending evidence"
            );
        }
        assert_eq!(
            serde_json::to_value(w).unwrap(),
            serde_json::to_value(resumed).unwrap()
        );
    }
}

// A referee body association does not merge separate player ContactIds. A
// command refers to its selected report, including that report's eligibility,
// measured position and release ledger identity.
fn duplicate_asw_report(w: &mut World, id: ContactId, estimate: Vec2) {
    let mut report = w.sides[0].contacts[&ContactId(71)].clone();
    report.id = id;
    report.estimate = estimate;
    report.stale = false;
    report.bearing_only = false;
    w.sides[0].contacts.insert(id, report);
}
fn selected_asw_fire(h: EntityId) -> (Side, Command) {
    (
        Side::Blue,
        Command::Fire {
            unit: h,
            contact: ContactId(71),
            weapon: "us_mk54".into(),
            rounds: 1,
        },
    )
}
fn assert_selected_report_cannot_borrow_eligibility(invalid: &str, assigned: bool) {
    let (mut w, h, t) = admission_fixture();
    duplicate_asw_report(&mut w, ContactId(72), Vec2::new(0., 1.));
    invalidate_own_asw_evidence(&mut w, t, invalid);
    if assigned {
        let actor = w.entity_mut(h).unwrap();
        actor.assigned = Some(ContactId(71));
        actor.fire_on = false;
    }
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for world in [&mut w, &mut restored] {
        if assigned {
            harpoon_core::sim::tick(world, &[]);
        } else {
            harpoon_core::sim::tick(world, &[selected_asw_fire(h)]);
        }
    }
    assert_eq!(
        w.stats.projectiles_launched, 0,
        "selected {invalid} report A must not borrow located report B through their hidden common body; assigned={assigned}"
    );
    assert_eq!(ammo(&w, h), 4);
    assert!((payload(&w, h) - 1140.).abs() < 1e-9);
    assert_eq!(
        serde_json::to_value(w).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
}
#[test]
fn selected_stale_manual_asw_report_cannot_borrow_a_located_same_body_track() {
    assert_selected_report_cannot_borrow_eligibility("stale", false);
}
#[test]
fn selected_bearing_manual_asw_report_cannot_borrow_a_located_same_body_track() {
    assert_selected_report_cannot_borrow_eligibility("bearing-only", false);
}
#[test]
fn assigned_asw_report_cannot_borrow_another_track_when_free_fire_is_disabled() {
    for invalid in ["stale", "bearing-only"] {
        assert_selected_report_cannot_borrow_eligibility(invalid, true);
    }
}
#[test]
fn a_valid_selected_asw_track_keeps_its_own_position_and_release_identity() {
    for assigned in [false, true] {
        let (mut w, h, _) = admission_fixture();
        // B sorts before selected A. They deliberately have different own
        // measurements; their common referee association grants no merge.
        duplicate_asw_report(&mut w, ContactId(70), Vec2::new(2., 0.));
        if assigned {
            let actor = w.entity_mut(h).unwrap();
            actor.assigned = Some(ContactId(71));
            actor.fire_on = false;
        }
        let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        for world in [&mut w, &mut restored] {
            if assigned {
                harpoon_core::sim::tick(world, &[]);
            } else {
                harpoon_core::sim::tick(world, &[selected_asw_fire(h)]);
            }
        }
        assert_eq!(w.stats.projectiles_launched, 1);
        assert_eq!(ammo(&w, h), 3);
        let torpedoes = w.rules.torpedoes.as_ref().unwrap();
        let (id, release) = torpedoes.launches.iter().next().unwrap();
        assert_eq!(
            release.contact,
            Some(ContactId(71)),
            "a valid selected report cannot be replaced by an earlier same-body contact; assigned={assigned}"
        );
        let flight = &torpedoes.flights[id];
        assert!((flight.initial_range - 1.).abs() < 1e-10);
        assert!(harpoon_core::geom::angle_diff(flight.launch_course, 0.).abs() < 1e-10);
        assert_eq!(
            serde_json::to_value(w).unwrap(),
            serde_json::to_value(restored).unwrap()
        );
    }
}

fn blue_visible_event_kinds(w: &World) -> Vec<serde_json::Value> {
    w.events
        .iter()
        .filter(|event| event.vis.visible_to(Side::Blue))
        .map(|event| serde_json::to_value(&event.kind).unwrap())
        .collect()
}
#[test]
fn secret_other_air_torpedo_survival_does_not_change_immediate_blue_drop_feedback() {
    let (mut template, h, target) = admission_fixture();
    template.entity_mut(target).unwrap().hp = 10_000.;
    template.entity_mut(target).unwrap().hp_max = 10_000.;
    // An explicit referee fixture clones the already validated aircraft fit
    // and load/fuel state for an unseen Red aircraft. Red's current located
    // contact and Good solution are authored conditions, not hidden Blue data.
    // Its torpedo is released by the real Fire pipeline before the comparison.
    let mut secret = template.entity(h).unwrap().clone();
    secret.id = template.allocate_id();
    secret.side = Side::Red;
    secret.name = "SECRET-HELO".into();
    secret.fire_on = false;
    secret.assigned = None;
    let secret_id = secret.id;
    template.entities.insert(secret_id, secret);
    let flight = template.rules.aviation.as_ref().unwrap().flights[&h].clone();
    template
        .rules
        .aviation
        .as_mut()
        .unwrap()
        .flights
        .insert(secret_id, flight);
    let order = template.rules.torpedoes.as_ref().unwrap().config.orders["HELO:us_mk54"].clone();
    template
        .rules
        .torpedoes
        .as_mut()
        .unwrap()
        .config
        .orders
        .insert("SECRET-HELO:us_mk54".into(), order);
    let mut report = template.sides[0].contacts[&ContactId(71)].clone();
    report.id = ContactId(171);
    template.sides[1].contacts.insert(report.id, report);
    template
        .rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert(
            format!("1:{}", target.0),
            harpoon_core::types::PlatformKind::Sub,
        );
    harpoon_core::sim::tick(
        &mut template,
        &[(
            Side::Red,
            Command::Fire {
                unit: secret_id,
                contact: ContactId(171),
                weapon: "us_mk54".into(),
                rounds: 1,
            },
        )],
    );
    assert_eq!(
        template.stats.projectiles_launched, 1,
        "the secret prior torpedo must be a real source release"
    );
    let other = *template
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    assert_eq!(template.entity(other).unwrap().side, Side::Red);
    assert!(template.entity(other).unwrap().alive);
    template.events.clear();
    let mut feedback = Vec::new();
    for secretly_live in [true, false] {
        let mut w = template.clone();
        if !secretly_live {
            let body = w.entity_mut(other).unwrap();
            body.alive = false;
            body.hp = 0.;
        }
        let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        for world in [&mut w, &mut restored] {
            harpoon_core::sim::tick(world, &[selected_asw_fire(h)]);
        }
        assert_eq!(
            w.stats.projectiles_launched, 2,
            "a hidden other weapon does not refuse Blue's valid release"
        );
        assert_eq!(ammo(&w, h), 3);
        feedback.push(blue_visible_event_kinds(&w));
        assert_eq!(
            serde_json::to_value(w).unwrap(),
            serde_json::to_value(restored).unwrap()
        );
    }
    assert_eq!(
        feedback[0], feedback[1],
        "9.7.1 physical mutual interference may differ in referee state, but the Blue receipt/events cannot reveal survival or count of an unseen Red torpedo"
    );
}

//! §3.1.3 real Navigation commands, nonnegative speed, reverse movement.
//! Hull size/propulsion and deployed decoys are explicit mechanism fixtures.
use harpoon_core::rules::{ew2026 as ew, navigation2026 as nav};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};

fn fixture(sub: bool, initial: f64, ordered: f64, seed: u64, tows: u16) -> (World, EntityId) {
    let class = if sub { "us_virginia" } else { "us_ford" };
    let mut context = json!({"rules_profile":"hv2026-rts-v4", "radar_environment":{"sea_state":0,"precipitation":"None"},
        "navigation_2026":{"source":"scenario_assumption: controlled astern source mechanism, no real hull propulsion claim", "units":{"SHIP":{"source":"scenario_assumption: explicit D size/nonCPP propulsion","size":"D","merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,"undamaged_max_kts":30,"propulsion":"NonSteam","readiness":"Underway"}}}});
    if tows > 0 {
        context["ew_2026"] = json!({"fits":{"TOW":{"source":"scenario_assumption: controlled single towed ACM fixture; exact zero-second deployment declared","kind":"towed_torpedo_decoy","generation":3,"launcher":null,"endurance_s":null,"deployment_s":0,"radar_height_m":null}},"loadouts":{class:{"TOW":tows}}});
    }
    let sc: Scenario = serde_json::from_value(json!({"id":"source-astern", "name":"Astern exact source envelope", "duration_s":5000,"seed":seed,
        "weather":{"visibility_nm":0,"jam":[0,0]},"context":context,
        "units":[{"side":"Blue","class":class,"name":"SHIP","pos":[0,0],"course_deg":0,"speed_kts":initial,"depth_m":if sub {50} else {0},"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(
        sc,
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap(),
    )
    .unwrap();
    let id = *w.entities.keys().next().unwrap();
    let e = w.entity_mut(id).unwrap();
    e.sensors.clear();
    e.weapons.clear();
    e.ordered_speed_kts = ordered;
    for _ in 0..tows {
        harpoon_core::sim::tick(
            &mut w,
            &[(
                Side::Blue,
                Command::ElectronicWarfare {
                    unit: id,
                    action: ew::Action::Deploy {
                        fit: "TOW".into(),
                        at: None,
                        radar_signature: None,
                        course_deg: None,
                        speed_kts: None,
                        depth_m: None,
                    },
                },
            )],
        );
    }
    if tows > 0 {
        assert_eq!(
            w.rules.ew.as_ref().unwrap().objects.len(),
            tows as usize,
            "author inventory must produce actual deployed objects"
        );
        // Deployment is real, but navigation's movement starts from this declared origin.
        let e = w.entity_mut(id).unwrap();
        e.speed_kts = initial;
        e.ordered_speed_kts = ordered;
        e.pos = harpoon_core::Vec2::zero();
    }
    (w, id)
}
fn order(id: EntityId, enabled: bool) -> (Side, Command) {
    (
        Side::Blue,
        Command::Navigation {
            unit: id,
            action: nav::Action::Astern { enabled },
        },
    )
}
fn tick(w: &mut World, n: usize) {
    for _ in 0..n {
        harpoon_core::sim::tick(w, &[]);
    }
}
fn events(w: &World, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule: r, detail, ..
            } if r == rule => Some(serde_json::from_str(detail).unwrap()),
            _ => None,
        })
        .collect()
}
fn die(seed: u64, purpose: &str, window: i64) -> u8 {
    let mut r = Rng::new(seed).derive(Rng::hash_tag(window, 1, purpose));
    1 + (r.next_f64() * 100.) as u8
}
fn seed_for(loss: bool, foul: bool) -> u64 {
    (1..100000)
        .find(|s| {
            (die(*s, "3.1.3.towed_loss", 0) <= 50) == loss
                && (die(*s, "3.1.3.cable_foul", 0) <= 10) == foul
        })
        .unwrap()
}

#[test]
fn astern_accelerates_half_the_ahead_table_and_moves_behind_unchanged_bow() {
    let (mut w, id) = fixture(false, 0., 12., 77, 0);
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    tick(&mut w, 179);
    let e = w.entity(id).unwrap();
    assert!(
        (e.speed_kts - 6.).abs() < 1e-9,
        "12kt/TT ahead becomes6kt/TT astern; actual {}",
        e.speed_kts
    );
    assert_eq!(e.course_deg, 0.);
    assert!(e.pos.y < 0.);
    assert!((e.pos.y + 181. * 3. / 3600.).abs() < 1e-9);
    assert!(e.speed_kts >= 0. && e.ordered_speed_kts >= 0.);
}
#[test]
fn surface_astern_caps_half_forward_max_while_submarine_caps_five_knots() {
    for (sub, want) in [(false, 15.), (true, 5.)] {
        let (mut w, id) = fixture(sub, 0., 30., 77, 0);
        harpoon_core::sim::tick(&mut w, &[order(id, true)]);
        tick(&mut w, 1199);
        let e = w.entity(id).unwrap();
        assert!(
            (e.speed_kts - want).abs() < 1e-9,
            "sub={sub} actual {}",
            e.speed_kts
        );
        assert!(e.pos.y < 0.);
    }
}
#[test]
fn weather_and_astern_are_independent_caps_not_two_multipliers() {
    let (mut w, id) = fixture(false, 0., 30., 77, 0);
    w.context.radar_environment.as_mut().unwrap().sea_state = 5;
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    tick(&mut w, 899);
    assert!((w.entity(id).unwrap().speed_kts - 15.).abs() < 1e-9);
    assert!(w.entity(id).unwrap().pos.y < 0.);
}
#[test]
fn an_ahead_ship_brakes_to_zero_before_it_can_move_astern() {
    let (mut w, id) = fixture(false, 6., 6., 77, 0);
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    assert!((w.entity(id).unwrap().speed_kts - (6. - 15. / 180.)).abs() < 1e-9);
    assert!(w.entity(id).unwrap().pos.y > 0.);
    tick(&mut w, 71);
    assert!(w.entity(id).unwrap().speed_kts.abs() < 1e-9);
    let at_zero = w.entity(id).unwrap().pos;
    tick(&mut w, 1);
    assert!(w.entity(id).unwrap().pos.y < at_zero.y);
    assert!((w.entity(id).unwrap().speed_kts - 6. / 180.).abs() < 1e-9);
}
#[test]
fn returning_ahead_brakes_the_reverse_motion_before_forward_acceleration() {
    let (mut w, id) = fixture(false, 0., 6., 77, 0);
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    tick(&mut w, 179);
    let old = w.entity(id).unwrap().pos;
    harpoon_core::sim::tick(&mut w, &[order(id, false)]);
    assert!(w.entity(id).unwrap().pos.y < old.y);
    assert!((w.entity(id).unwrap().speed_kts - (6. - 15. / 180.)).abs() < 1e-9);
    tick(&mut w, 71);
    let stopped = w.entity(id).unwrap().pos;
    tick(&mut w, 1);
    assert!(w.entity(id).unwrap().pos.y > stopped.y);
}
#[test]
fn astern_course_change_does_not_consume_forward_advance_table() {
    let (mut w, id) = fixture(false, 0., 6., 77, 0);
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    tick(&mut w, 179);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetCourse {
                unit: id,
                course_deg: 90.,
            },
        )],
    );
    tick(&mut w, 80);
    assert_eq!(w.entity(id).unwrap().course_deg, 0.);
    assert!(events(&w, "3.1.4.pivot").is_empty());
    assert!(!events(&w, "3.1.3.turn_pending").is_empty());
}
#[test]
fn astern_native_loss_and_fouling_trials_are_independent_and_only_once_per_turn() {
    for (loss, foul) in [(true, false), (false, true), (true, true), (false, false)] {
        let seed = seed_for(loss, foul);
        let (mut w, id) = fixture(false, 0., 6., seed, 1);
        harpoon_core::sim::tick(&mut w, &[order(id, true)]);
        let risk = events(&w, "3.1.3.towed_risk");
        assert_eq!(
            risk.len(),
            1,
            "single deployed towed body needs source dice"
        );
        let v = &risk[0];
        assert_eq!(v["loss_d100"], die(seed, "3.1.3.towed_loss", 0));
        assert_eq!(v["foul_d100"], die(seed, "3.1.3.cable_foul", 0));
        assert_eq!(v["lost"], loss);
        assert_eq!(v["fouled"], foul);
        assert_eq!(
            w.rules
                .ew
                .as_ref()
                .unwrap()
                .objects
                .values()
                .next()
                .unwrap()
                .status
                == ew::Status::Destroyed,
            loss
        );
        assert_eq!(
            w.entity(id)
                .unwrap()
                .ext
                .get("damage.engine_hits")
                .copied()
                .unwrap_or(0.)
                > 0.,
            foul
        );
        for _ in 0..20 {
            harpoon_core::sim::tick(&mut w, &[order(id, true)]);
        }
        assert_eq!(
            events(&w, "3.1.3.towed_risk").len(),
            1,
            "same-turn reorders cannot reroll the source checks"
        );
    }
}
#[test]
fn a_second_tactical_turn_uses_a_new_trial_only_if_a_tow_still_exists() {
    let seed = seed_for(false, false);
    let (mut w, id) = fixture(false, 0., 6., seed, 1);
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    tick(&mut w, 177);
    assert_eq!(w.tick, 179);
    assert_eq!(events(&w, "3.1.3.towed_risk").len(), 1);
    tick(&mut w, 1);
    let risk = events(&w, "3.1.3.towed_risk");
    assert_eq!(risk.len(), 2);
    assert_eq!(risk[1]["loss_d100"], die(seed, "3.1.3.towed_loss", 1));
    assert_eq!(risk[1]["foul_d100"], die(seed, "3.1.3.cable_foul", 1));
}
#[test]
fn actual_multiple_deployed_tows_are_pending_without_selection_or_dice() {
    let (mut w, id) = fixture(false, 0., 6., 77, 2);
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    tick(&mut w, 30);
    assert_eq!(w.entity(id).unwrap().pos, harpoon_core::Vec2::zero());
    assert!(events(&w, "3.1.3.towed_risk").is_empty());
    assert!(!events(&w, "3.1.3.tow_selection_pending").is_empty());
    assert!(w
        .rules
        .ew
        .as_ref()
        .unwrap()
        .objects
        .values()
        .all(|o| o.status == ew::Status::Deployed));
}
#[test]
fn mid_astern_state_and_original_trials_restore_without_enemy_disclosure() {
    let seed = seed_for(false, false);
    let (mut w, id) = fixture(false, 0., 6., seed, 1);
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    tick(&mut w, 37);
    let own = serde_json::to_value(harpoon_core::observe(&w, Side::Blue, 0, 0)).unwrap();
    assert!(own["units"][0]["navigation"]["source_rules"]
        .get("astern")
        .is_some());
    assert!(harpoon_core::observe(&w, Side::Red, 0, 0).units.is_empty());
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    restored.rules.validate_tables().unwrap();
    tick(&mut w, 210);
    tick(&mut restored, 210);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
#[test]
fn no_tow_and_stationary_astern_order_do_not_invent_cable_risk() {
    let (mut w, id) = fixture(false, 0., 0., 77, 1);
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    tick(&mut w, 190);
    assert!(events(&w, "3.1.3.towed_risk").is_empty());
    assert!(w
        .entity(id)
        .unwrap()
        .ext
        .get("damage.engine_hits")
        .is_none());
    let (mut w, id) = fixture(false, 0., 6., 77, 0);
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    tick(&mut w, 190);
    assert!(events(&w, "3.1.3.towed_risk").is_empty());
    assert!(w.entity(id).unwrap().pos.y < 0.);
}
#[test]
fn source_astern_is_not_silently_enabled_for_legacy_or_missing_fit() {
    let (mut w, id) = fixture(false, 0., 6., 77, 0);
    w.rules.navigation.as_mut().unwrap().units.clear();
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    tick(&mut w, 30);
    assert!(w.entity(id).unwrap().pos.y > 0.);
}

#[test]
fn source_damage_break_reduces_the_forward_max_before_surface_astern_halving() {
    // Verified Ford source speed row32/24/16/8/0, first DP break417.
    // The authored navigation maximum30 cannot raise the damaged24kt cap.
    let (mut w, id) = fixture(false, 0., 30., 77, 0);
    w.entity_mut(id).unwrap().hp -= 417.;
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    tick(&mut w, 899);
    assert!((w.entity(id).unwrap().speed_kts - 12.).abs() < 1e-9);
    assert!(w.entity(id).unwrap().pos.y < 0.);
}

#[test]
fn reverse_swept_coast_guard_blocks_a_thin_strip_before_movement_or_cable_dice() {
    let (mut w, id) = fixture(false, 0., 6., seed_for(false, false), 1);
    let mut terrain: harpoon_core::terrain::Terrain = serde_json::from_value(json!({
        "source":"scenario_assumption: thin coast behind unchanged bow, crossing lies between endpoints",
        "polygons":[{"outer":[{"x":-0.01,"y":-0.000005},{"x":0.01,"y":-0.000005},{"x":0.01,"y":-0.000001},{"x":-0.01,"y":-0.000001}]}]
    })).unwrap();
    terrain.normalize().unwrap();
    w.context.terrain = Some(terrain);
    let hp = w.entity(id).unwrap().hp;
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    assert_eq!(w.entity(id).unwrap().pos, harpoon_core::Vec2::zero());
    assert_eq!(w.entity(id).unwrap().speed_kts, 0.);
    assert_eq!(w.entity(id).unwrap().ordered_speed_kts, 0.);
    assert_eq!(w.entity(id).unwrap().hp, hp);
    assert_eq!(events(&w, "3.1.3.land_stop").len(), 1);
    assert!(events(&w, "3.1.3.towed_risk").is_empty());
}

#[test]
fn an_authored_initially_deployed_sonar_body_is_actually_disabled_on_source_loss() {
    use harpoon_core::rules::detection2026 as d;
    let (mut w, id) = fixture(false, 0., 6., seed_for(true, false), 0);
    let key = format!("{}:TESTTOW", id.0);
    let state = w.rules.detection.as_mut().unwrap();
    state.config.bindings.insert("us_ford:TESTTOW".into(), d::Receiver::Sonar {
        source: "scenario_assumption: explicitly authored initially deployed towed sonar; not a Ford hardware claim".into(),
        generation: 4, passive_range_nm: Some(4.), active_range_nm: None,
        frequency: d::Frequency::Lmf, mount: d::SonarMount::Towed,
        active_torpedo_detection: false, high_speed_tow: false,
    });
    state.arrays.insert(
        key.clone(),
        d::ArrayJob {
            recovering: false,
            ready_at: 0,
            deployed: true,
            lost: false,
            depth_m: 100.,
        },
    );
    harpoon_core::sim::tick(&mut w, &[order(id, true)]);
    let job = &w.rules.detection.as_ref().unwrap().arrays[&key];
    assert!(job.lost && !job.deployed);
    let risk = events(&w, "3.1.3.towed_risk");
    assert_eq!(risk.len(), 1);
    assert_eq!(risk[0]["body"]["type"], "sonar");
}

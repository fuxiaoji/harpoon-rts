use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
fn fixture(
    size: char,
    sub: bool,
    waterjet: bool,
    merchant: bool,
    liner: bool,
    speed: f64,
) -> (World, EntityId) {
    let sc:Scenario=serde_json::from_value(json!({"id":"source-turning","name":"Source advance fixture, no real hull size/propulsion claim","seed":77,"duration_s":20000,
        "weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "navigation_2026":{"source":"scenario_assumption: exact normal-turn table mechanism","units":{"SHIP":{"source":"scenario_assumption: authored geometry fit","size":size,"merchant":merchant,"passenger_liner":liner,"cpp":false,"waterjet":waterjet,"undamaged_max_kts":30,"propulsion":"NonSteam","readiness":"Underway"}}}},
        "units":[{"side":"Blue","class":if sub{"us_virginia"}else{"us_ford"},"name":"SHIP","pos":[0,0],"course_deg":0,"speed_kts":speed,"depth_m":if sub{50}else{0}}]})).unwrap();
    let mut w = harpoon_core::load_scenario(
        sc,
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap(),
    )
    .unwrap();
    let id = *w.entities.keys().next().unwrap();
    w.entity_mut(id).unwrap().sensors.clear();
    w.entity_mut(id).unwrap().fire_on = false;
    (w, id)
}
fn tick(w: &mut World, n: usize) {
    for _ in 0..n {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn course(w: &mut World, id: EntityId, heading: f64) {
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::SetCourse {
                unit: id,
                course_deg: heading,
            },
        )],
    );
}
fn pivots(w: &World) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                domain,
                rule,
                detail,
                ..
            } if domain == "navigation2026" && rule == "3.1.4.pivot" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn standard_advance_is_required_after_order_and_old_straight_travel_is_not_credit() {
    let (mut w, id) = fixture('D', false, false, false, false, 20.);
    tick(&mut w, 200);
    let start = w.entity(id).unwrap().pos;
    course(&mut w, id, 90.);
    tick(&mut w, 7);
    assert_eq!(w.entity(id).unwrap().course_deg, 0.);
    assert!((w.entity(id).unwrap().pos.x - start.x).abs() < 1e-10);
    assert!(w.entity(id).unwrap().pos.y > start.y);
    tick(&mut w, 32);
    assert_eq!(w.entity(id).unwrap().course_deg, 90.);
    let p = pivots(&w);
    assert_eq!(p.len(), 2);
    for event in p {
        assert_eq!(event["advance_yd"], 200.);
        assert_eq!(event["angle_deg"], 45.);
        assert_eq!(event["speed_loss_kts"], 1.);
    }
}
#[test]
fn zero_speed_cannot_rotate_on_the_spot() {
    let (mut w, id) = fixture('D', false, false, false, false, 0.);
    course(&mut w, id, 90.);
    tick(&mut w, 60);
    assert_eq!(w.entity(id).unwrap().course_deg, 0.);
    assert_eq!(w.entity(id).unwrap().pos, harpoon_core::Vec2::zero());
}
#[test]
fn submerged_sub_uses300yd_not_its_surface_size200yd() {
    let (mut surface, sid) = fixture('D', false, false, false, false, 20.);
    let (mut sub, uid) = fixture('D', true, false, false, false, 20.);
    course(&mut surface, sid, 45.);
    course(&mut sub, uid, 45.);
    tick(&mut surface, 18);
    tick(&mut sub, 18);
    assert_eq!(surface.entity(sid).unwrap().course_deg, 45.);
    assert_eq!(sub.entity(uid).unwrap().course_deg, 0.);
    tick(&mut sub, 10);
    assert_eq!(sub.entity(uid).unwrap().course_deg, 45.);
    assert_eq!(pivots(&sub)[0]["advance_yd"], 300.);
}
#[test]
fn waterjet_turns_one_source_size_smaller_and_pivots_up_to60deg() {
    let (mut w, id) = fixture('C', false, true, false, false, 20.);
    course(&mut w, id, 60.);
    tick(&mut w, 19);
    assert_eq!(w.entity(id).unwrap().course_deg, 60.);
    let p = pivots(&w);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0]["advance_yd"], 200.);
    assert_eq!(p[0]["angle_deg"], 60.);
}
#[test]
fn speed_loss_budget_stops_after_four_source45deg_equivalents_in_a_turn() {
    let (mut w, id) = fixture('D', false, false, false, false, 20.);
    course(&mut w, id, 180.);
    while w.entity(id).unwrap().course_deg != 180. && w.tick < 150 {
        tick(&mut w, 1);
    }
    assert_eq!(w.entity(id).unwrap().course_deg, 180.);
    course(&mut w, id, 90.);
    while w.entity(id).unwrap().course_deg != 90. && w.tick < 179 {
        tick(&mut w, 1);
    }
    assert_eq!(w.entity(id).unwrap().course_deg, 90.);
    let p = pivots(&w);
    assert_eq!(p.len(), 6);
    assert_eq!(
        p.iter()
            .map(|x| x["speed_loss_kts"].as_f64().unwrap())
            .sum::<f64>(),
        4.
    );
    assert_eq!(p[4]["speed_loss_kts"], 0.);
    assert_eq!(p[5]["speed_loss_kts"], 0.);
}
#[test]
fn partial_pivot_has_explicit_rts_proportional_loss_and_no_rounding_claim() {
    let (mut w, id) = fixture('D', false, false, false, false, 20.);
    course(&mut w, id, 22.5);
    tick(&mut w, 24);
    assert_eq!(w.entity(id).unwrap().course_deg, 22.5);
    let p = pivots(&w);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0]["speed_loss_kts"], 0.5);
    assert_eq!(
        p[0]["partial_loss_policy"],
        "rts_proportional_to_printed45deg"
    );
}
#[test]
fn repeated_course_order_and_mid_advance_save_do_not_reset_work() {
    let (mut w, id) = fixture('D', false, false, false, false, 20.);
    course(&mut w, id, 90.);
    tick(&mut w, 7);
    let own = serde_json::to_value(harpoon_core::observe(&w, Side::Blue, 0, 0)).unwrap();
    assert!(own["units"][0]["navigation"]["source_rules"]
        .get("turning")
        .is_some());
    assert!(harpoon_core::observe(&w, Side::Red, 0, 0).units.is_empty());
    let mut saved: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    saved.rules.validate_tables().unwrap();
    for _ in 0..20 {
        course(&mut w, id, 90.);
        course(&mut saved, id, 90.);
    }
    assert_eq!(w.entity(id).unwrap().course_deg, 45.);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}
#[test]
fn passenger_liner_keeps_merchant_turn_row_even_though_it_accelerates_as_warship() {
    let (mut w, id) = fixture('C', false, false, true, true, 20.);
    course(&mut w, id, 45.);
    tick(&mut w, 30);
    let p = pivots(&w);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0]["advance_yd"], 300.);
    assert_eq!(p[0]["speed_loss_kts"], 3.);
}
#[test]
fn smallest_waterjet_has_no_invented_smaller_source_row() {
    let (mut w, id) = fixture('G', false, true, false, false, 20.);
    course(&mut w, id, 45.);
    tick(&mut w, 30);
    assert_eq!(w.entity(id).unwrap().course_deg, 0.);
    assert!(w
        .events
        .iter()
        .any(|e| matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="3.1.4.turn_blocked")));
}

#[test]
fn pivot_at179_does_not_move_its_acceleration_penalty_into_the_next_tactical_turn() {
    let (mut w, id) = fixture('D', false, false, false, false, 20.);
    tick(&mut w, 161);
    course(&mut w, id, 45.);
    tick(&mut w, 17);
    assert_eq!(w.tick, 179);
    assert_eq!(w.entity(id).unwrap().course_deg, 45.);
    assert_eq!(pivots(&w).len(), 1);
    let before = w.entity(id).unwrap().speed_kts;
    tick(&mut w, 1);
    let gain = w.entity(id).unwrap().speed_kts - before;
    assert!(
        (gain - 12. / 180.).abs() < 1e-10,
        "the completed prior-turn pivot must not halve this new turn's acceleration; gained {gain}"
    );
}

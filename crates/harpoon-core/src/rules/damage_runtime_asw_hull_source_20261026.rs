//! Existing-API source-red for the common §8.5 critical receiver only.
//! Real load_scenario and actual queue_projector/queue_depth_charge -> primary.
//! A normal scenario seed selects the printed HullPen row; no HP/dice/Contact writes.
//! End-to-end launch/source fit remains the separate native ASW producer suite.
use super::*;
const SLOT: &str = "scenario_assumption:8.5.selected-source-critical-slot";
fn scenario_seed(kind: &str) -> u64 {
    (0..100000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(0, 2, SLOT));
            let d = 1 + (r.next_f64() * 100.) as u8;
            crate::rules::expanded_damage2026::row("sub_major", d).is_some_and(|r| r.0 == kind)
        })
        .unwrap()
}
fn fixture(projector: bool, kind: &str) -> (World, Entity, Resolved) {
    let db = crate::Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let sc:crate::Scenario=serde_json::from_value(serde_json::json!({"id":"source-asw-pressure-common","name":"scenario_assumption: actual common hydrostatic damage source receiver","seed":scenario_seed(kind),"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4"},"units":[
 {"side":"Blue","class":"us_burke_2a","name":"LAUNCHER","pos":[0,0],"speed_kts":0,"fire_on":false},
 {"side":"Red","class":"cn_039a","name":"SUB","pos":[0,0.1],"speed_kts":0,"depth_m":200,"fire_on":false}]})).unwrap();
    let mut w = crate::load_scenario(sc, db).unwrap();
    begin(&mut w);
    if projector {
        queue_projector(
            &mut w,
            EntityId(1),
            EntityId(2),
            "cn_rbu1000",
            7.,
            true,
            "scenario_assumption:physically-adjudicated-projector-hydrostatic-pattern",
        )
        .unwrap();
    } else {
        queue_depth_charge(
            &mut w,
            EntityId(1),
            EntityId(2),
            "cn_rbu1000",
            7.,
            true,
            "scenario_assumption:physically-adjudicated-depth-charge-pattern",
        )
        .unwrap();
    }
    assert_eq!(w.rules.pending.len(), 1);
    let hit = w.rules.pending.pop().unwrap();
    assert!(matches!(hit.projector, Some(ProjectorSeverity::Major)));
    assert!(hit.mine.is_none() && !hit.torpedo_deep);
    assert!(matches!(hit.weapon.kind, WeaponKind::AswRocket));
    let t = w.entity(EntityId(2)).unwrap().clone();
    let h = primary(&mut w, hit, &t);
    assert!(h.dp > 0.);
    (w, t, h)
}
fn no_direct_hull_without_source(projector: bool) {
    let (mut w, t, h) = fixture(projector, "hull_penetration");
    let hp = t.hp;
    critical(&mut w, &t, &h, SLOT, None);
    assert_eq!(w.entity(t.id).unwrap().hp,hp,"English2026 PDF101/8-21: a selected hydrostatic HullPen critical must pass the source Contact HullPen table; missing four-band size/design/year is pending, not immediate pressure hull failure");
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,detail,..}if rule=="14.expanded_pending"&&detail.contains("asw_pressure_hull"))),"the unresolved source Contact-table prerequisite must be an explicit private pending result");
}
#[test]
fn asw_hull_source01_real_projector_hydrostatic_hull_row_requires_source_table_instead_of_direct_effect(
) {
    no_direct_hull_without_source(true)
}
#[test]
fn asw_hull_source02_real_depth_charge_hydrostatic_hull_row_requires_source_table_instead_of_direct_effect(
) {
    no_direct_hull_without_source(false)
}
#[test]
fn asw_hull_source03_non_hull_hydrostatic_critical_is_not_globally_blocked_by_missing_hull_contact_fit(
) {
    let (mut w, t, h) = fixture(false, "flooding");
    critical(&mut w, &t, &h, SLOT, None);
    assert!(w.rules.casualties.get(&t.id).is_some_and(|v|v.iter().any(|c|c.flood)),"ordinary real source flooding still resolves without an unrelated pressure-hull classification");
    assert!(!w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,detail,..}if rule=="14.expanded_pending"&&detail.contains("asw_pressure_hull"))));
}

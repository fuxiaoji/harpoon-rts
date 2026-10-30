//! New common-receiver boundary candidates after actual2red1control source batch.
use super::*;
const CRIT: &str = "8.5.common.selected_hull_slot";
const CONTACT: &str = "scenario_assumption:actual-contact-submunition";
fn fit(size: &str, design: &str, year: u16) -> Value {
    json!({"source":"scenario_assumption: explicit controlled four-band hull, not a real Annex claim","size":size,"design":design,"in_service_year":year})
}
fn fixture(seed: u64, hull: Option<Value>) -> World {
    let mut db = crate::Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let mut d = db.weapons["cw_depth_charge"].clone();
    d.id = "scenario_assumption_asw_cluster_contact".into();
    d.name = "scenario_assumption: controlled direct ASW submunition".into();
    db.weapons.insert(d.id.clone(), d);
    let target = db.platforms.get_mut("cn_039a").unwrap();
    target.weapons.clear();
    target.sensors.clear();
    let mut v = json!({"id":"source-asw-hull-following","name":"scenario_assumption: real common damage receiver with explicit cited hull policy","seed":seed,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","asw_weapons_2026":{"source":"scenario_assumption: controlled source receiver experiment","standoffs":{},"submarine_hulls":{}}},"units":[{"side":"Blue","class":"us_burke_2a","name":"SHOOTER","pos":[0,0],"fire_on":false},{"side":"Red","class":"cn_039a","name":"TARGET","pos":[0,0.1],"depth_m":200,"fire_on":false}]});
    if let Some(h) = hull {
        v["context"]["asw_weapons_2026"]["submarine_hulls"]["TARGET"] = h;
    }
    crate::load_scenario(serde_json::from_value::<crate::Scenario>(v).unwrap(), db).unwrap()
}
fn contact(w: &mut World) -> AswContactOutcome {
    queue_asw_cluster_contact(
        w,
        EntityId(1),
        EntityId(2),
        "scenario_assumption_asw_cluster_contact",
        3.,
        AswContactScale::Full,
        CONTACT,
    )
    .unwrap()
}
fn pressure_events(w: &World) -> usize {
    w.events.iter().filter(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="8.5.contact_pressure_hull")).count()
}
#[test]
fn h01_all_eight_source_cells_and_strict_year_bonus_reach_the_actual_bound_consumer() {
    for (design, expected) in [
        ("single", [90., 60., 30., 15.]),
        ("double", [75., 45., 15., 5.]),
    ] {
        for (size, c) in ["vsmall", "small", "medium", "large"]
            .into_iter()
            .zip(expected)
        {
            let w = fixture(1, Some(fit(size, design, 2000)));
            assert_eq!(chance(&w, EntityId(2), AswContactScale::Full).unwrap(), c);
        }
    }
    assert_eq!(
        chance(
            &fixture(1, Some(fit("vsmall", "single", 1954))),
            EntityId(2),
            AswContactScale::Full
        )
        .unwrap(),
        120.
    );
    assert_eq!(
        chance(
            &fixture(1, Some(fit("vsmall", "single", 1955))),
            EntityId(2),
            AswContactScale::Full
        )
        .unwrap(),
        90.
    );
    assert_eq!(
        chance(
            &fixture(1, Some(fit("small", "single", 2000))),
            EntityId(2),
            AswContactScale::Half
        )
        .unwrap(),
        30.
    );
    assert!(chance(
        &fixture(1, Some(fit("large", "double", 2000))),
        EntityId(2),
        AswContactScale::Half
    )
    .unwrap_err()
    .contains("fractional"));
    assert!(chance(
        &fixture(1, Some(fit("small", "single", 1954))),
        EntityId(2),
        AswContactScale::Half
    )
    .unwrap_err()
    .contains("priority"));
}
#[test]
fn h02_each_missing_explicit_hull_dimension_stays_unknown_instead_of_a_default() {
    for field in ["size", "design", "in_service_year"] {
        let mut f = fit("medium", "double", 2000);
        f[field] = Value::Null;
        let w = fixture(1, Some(f));
        assert!(chance(&w, EntityId(2), AswContactScale::Full).is_err());
    }
}
fn critical_seed(penetrated: bool) -> u64 {
    (0..100000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(0, 2, CRIT));
            let d = 1 + (r.next_f64() * 100.) as u8;
            let pen = 1 + (r.next_f64() * 100.) as u8;
            crate::rules::expanded_damage2026::row("sub_major", d)
                .is_some_and(|v| v.0 == "hull_penetration")
                && pen == if penetrated { 1 } else { 100 }
        })
        .unwrap()
}
#[test]
fn h03_actual_hydrostatic_selected_hull_row_rolls_contact_chance_then_applies_or_ignores_only_that_slot(
) {
    for passed in [false, true] {
        let mut w = fixture(critical_seed(passed), Some(fit("large", "double", 2000)));
        begin(&mut w);
        queue_depth_charge(
            &mut w,
            EntityId(1),
            EntityId(2),
            "cw_depth_charge",
            7.,
            true,
            "scenario_assumption:real-hydrostatic-result",
        )
        .unwrap();
        let t = w.entity(EntityId(2)).unwrap().clone();
        let hit = w.rules.pending.pop().unwrap();
        let h = primary(&mut w, hit, &t);
        let dp = h.dp;
        critical(&mut w, &t, &h, CRIT, None);
        assert_eq!(h.dp, dp);
        assert_eq!(w.entity(t.id).unwrap().hp, if passed { 0. } else { t.hp });
        assert_eq!(
            pressure_events(&w),
            1,
            "one real extra pressure die per selected critical"
        );
    }
}
#[test]
fn h04_actual_contact_success_queues_zero_minor_dp_and_true_hull_effect_then_replays_idempotently()
{
    let mut w = fixture(3, Some(fit("vsmall", "single", 1954)));
    begin(&mut w);
    let result = contact(&mut w);
    assert_eq!(result.penetrated, Some(true));
    assert!(result.damage_queued && result.hull_effect_queued && !result.minor_damage_queued);
    assert_eq!(w.rules.pending.len(), 1);
    assert!(matches!(
        w.rules.pending[0].asw,
        Some(AswImpact::ClusterContact {
            penetrated: true,
            ..
        })
    ));
    assert!(w.rules.pending[0].projector.is_none() && w.rules.pending[0].mine.is_none());
    let before = serde_json::to_value(&w).unwrap();
    assert_eq!(
        serde_json::to_value(contact(&mut w)).unwrap(),
        serde_json::to_value(&result).unwrap()
    );
    assert_eq!(serde_json::to_value(&w).unwrap(), before);
    let mut restored: World = serde_json::from_value(before).unwrap();
    for state in [&mut w, &mut restored] {
        finish(state);
        assert!(!state.entity(EntityId(2)).unwrap().alive);
        let dp: f64 = state
            .events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::Impact { unit, damage, .. } if *unit == EntityId(2) => Some(*damage),
                _ => None,
            })
            .sum();
        assert_eq!(
            dp, 0.,
            "pressure success does not also buy source Minor damage"
        );
        assert_eq!(pressure_events(state), 1);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
#[test]
fn h05_actual_contact_failure_queues_only_minor_damage_and_no_fake_major_or_pressure_effect() {
    let seed = (0..100000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(
                0,
                2,
                &format!("8.5.cluster_contact:{CONTACT}"),
            ));
            1 + (r.next_f64() * 100.) as u8 == 100
        })
        .unwrap();
    let mut w = fixture(seed, Some(fit("large", "double", 2000)));
    let hp = w.entity(EntityId(2)).unwrap().hp;
    begin(&mut w);
    let result = contact(&mut w);
    assert_eq!(result.pressure_roll, Some(100));
    assert_eq!(result.penetrated, Some(false));
    assert!(result.damage_queued && result.minor_damage_queued && !result.hull_effect_queued);
    assert!(w.rules.pending[0].projector.is_none());
    finish(&mut w);
    assert_eq!(w.entity(EntityId(2)).unwrap().hp, hp - 3.);
    assert_eq!(pressure_events(&w), 1);
}
#[test]
fn h06_unknown_contact_hull_source_is_pending_cached_and_never_synthesizes_an_effect_or_minor_hit()
{
    let mut w = fixture(3, None);
    let hp = w.entity(EntityId(2)).unwrap().hp;
    begin(&mut w);
    let result = contact(&mut w);
    assert!(
        result.pending_reason.is_some()
            && result.pressure_roll.is_none()
            && result.penetrated.is_none()
    );
    assert!(!result.damage_queued && !result.minor_damage_queued && !result.hull_effect_queued);
    assert!(w.rules.pending.is_empty());
    let count = w.events.len();
    assert_eq!(
        serde_json::to_value(contact(&mut w)).unwrap(),
        serde_json::to_value(result).unwrap()
    );
    assert_eq!(w.events.len(), count);
    finish(&mut w);
    assert_eq!(w.entity(EntityId(2)).unwrap().hp, hp);
    assert_eq!(pressure_events(&w), 0);
}

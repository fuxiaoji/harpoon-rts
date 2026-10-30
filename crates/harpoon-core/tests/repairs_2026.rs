//! §14.6: real source Engineering critical, functional speed loss, real
//! DamageControl/sim commands, original per-critical D100 and recovery.
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
const CRITICAL: &str = "repair-fixture-engineering";
fn d100(seed: u64, hours: u8) -> u8 {
    let mut r = Rng::new(seed).derive(Rng::hash_tag(
        0,
        1,
        &format!("14.6.repair:{CRITICAL}:{hours}"),
    ));
    1 + (r.next_f64() * 100.) as u8
}
fn clean_engineering_seed(seed: u64) -> bool {
    let mut r = Rng::new(seed).derive(Rng::hash_tag(0, 1, CRITICAL));
    let electrical = 1 + (r.next_f64() * 6.) as u8;
    let fire = 1 + (r.next_f64() * 6.) as u8;
    electrical > 1 && fire <= 2
}
fn seed_for(predicate: impl Fn(u64) -> bool) -> u64 {
    (1..1_000_000)
        .find(|s| clean_engineering_seed(*s) && predicate(*s))
        .unwrap()
}
fn fixture(seed: u64, fraction: f64) -> (World, EntityId) {
    let scenario:Scenario=serde_json::from_value(json!({"id":"post-battle-repair","name":"Source Engineering repair fixture",
        "seed":seed,"duration_s":300000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4"},"units":[{"side":"Blue","class":"us_ford","name":"REPAIR","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(
        scenario,
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap(),
    )
    .unwrap();
    let id = *w.entities.keys().next().unwrap();
    let e = w.entity_mut(id).unwrap();
    e.sensors.clear();
    e.weapons.clear();
    e.hp = e.hp_max * fraction;
    // This is the actual production source-only Engineering critical hook,
    // not a synthetic counter or invented ordinary weapon hit.
    harpoon_core::rules::damage_runtime::astern_cable_engineering(&mut w, id, CRITICAL).unwrap();
    assert_eq!(
        w.entity(id).unwrap().ext.get("damage.engine_levels"),
        Some(&1.)
    );
    assert_eq!(
        w.entity(id).unwrap().ext.get("damage.engine_hits"),
        Some(&1.)
    );
    assert!(
        w.rules.casualties.get(&id).is_none_or(|c| c.is_empty()),
        "native seed excludes unrelated fire to isolate the repair rule"
    );
    assert!(w.entity(id).unwrap().ext.get("damage.power_lost").is_none());
    assert!(
        w.rules.repairs.as_ref().unwrap().units[&id]
            .criticals
            .contains_key(CRITICAL),
        "real Engineering effect must enter the forward-only critical ledger"
    );
    (w, id)
}
fn command(w: &mut World, id: EntityId, action: &str) {
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::DamageControl {
                unit: id,
                action: action.into(),
            },
        )],
    );
}
fn at(w: &mut World, tick: i64) {
    assert!(tick > w.tick);
    w.tick = tick - 1;
    harpoon_core::sim::tick(w, &[]);
}
fn checks(w: &World) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "14.6.repair_check" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn begin(w: &mut World, id: EntityId, automatic: bool) -> i64 {
    command(w, id, "begin_repairs");
    let t = w.tick;
    if automatic {
        command(w, id, "repairs_auto");
    }
    t
}

#[test]
fn silence_never_declares_post_battle_and_early_manual_roll_does_not_repair() {
    let seed = seed_for(|s| d100(s, 6) <= 50);
    let (mut w, id) = fixture(seed, 1.);
    at(&mut w, 300);
    assert_eq!(w.entity(id).unwrap().max_speed(&w.db), 24.);
    at(&mut w, 21600);
    assert!(checks(&w).is_empty());
    assert_eq!(w.entity(id).unwrap().max_speed(&w.db), 24.);
    let started = begin(&mut w, id, false);
    command(&mut w, id, "repair_due");
    assert!(checks(&w).is_empty());
    assert!(w.rules.repairs.as_ref().unwrap().units[&id]
        .session
        .as_ref()
        .is_some_and(|s| s.began_at == started));
}
#[test]
fn six_hour_exact_boundary_restores_real_engineering_speed_without_restoring_hull_or_history() {
    let seed = seed_for(|s| d100(s, 6) == 50);
    let (mut w, id) = fixture(seed, 1.);
    let hp = w.entity(id).unwrap().hp;
    let start = begin(&mut w, id, true);
    at(&mut w, start + 21600 - 1);
    assert!(checks(&w).is_empty());
    assert_eq!(w.entity(id).unwrap().max_speed(&w.db), 24.);
    at(&mut w, start + 21600);
    let c = checks(&w);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0]["checkpoint_hours"], 6);
    assert_eq!(c[0]["d100"], 50);
    assert_eq!(c[0]["success"], true);
    assert_eq!(w.entity(id).unwrap().max_speed(&w.db), 32.);
    assert_eq!(w.entity(id).unwrap().hp, hp);
    assert_eq!(
        w.entity(id).unwrap().ext.get("damage.engine_hits"),
        Some(&1.)
    );
    let old = w.entity(id).unwrap().pos;
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: id,
                speed_kts: 32.,
            },
        )],
    );
    for _ in 0..90 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w.entity(id).unwrap().speed_kts > 24.);
    assert!(w.entity(id).unwrap().pos.y > old.y);
}
#[test]
fn source_native_fifty_one_fails_half_roll_and_a_repeated_due_does_not_reroll() {
    let seed = seed_for(|s| d100(s, 6) == 51);
    let (mut w, id) = fixture(seed, 1.);
    let start = begin(&mut w, id, true);
    at(&mut w, start + 21600);
    let c = checks(&w);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0]["d100"], 51);
    assert_eq!(c[0]["success"], false);
    assert_eq!(w.entity(id).unwrap().max_speed(&w.db), 24.);
    for _ in 0..4 {
        command(&mut w, id, "repair_due");
        command(&mut w, id, "begin_repairs");
    }
    assert_eq!(checks(&w), c);
    assert_eq!(
        w.rules.repairs.as_ref().unwrap().units[&id]
            .session
            .as_ref()
            .unwrap()
            .began_at,
        start
    );
}
#[test]
fn printed_thirty_seven_percent_example_keeps_eighteen_success_nineteen_failure() {
    for (face, success) in [(18, true), (19, false)] {
        let seed = seed_for(|s| d100(s, 6) == face);
        let (mut w, id) = fixture(seed, 0.37);
        let start = begin(&mut w, id, true);
        at(&mut w, start + 21600);
        let c = checks(&w);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0]["d100"], face);
        assert_eq!(c[0]["success"], success);
        assert!((c[0]["chance"].as_f64().unwrap() - 0.185).abs() < 1e-12);
    }
}
#[test]
fn twelve_hour_half_and_twenty_four_hour_full_probability_use_separate_original_dice() {
    let seed =
        seed_for(|s| d100(s, 6) > 25 && d100(s, 12) > 25 && d100(s, 24) > 25 && d100(s, 24) <= 50);
    let (mut w, id) = fixture(seed, 0.5);
    let start = begin(&mut w, id, true);
    for hour in [6, 12, 24] {
        at(&mut w, start + hour * 3600);
    }
    let c = checks(&w);
    assert_eq!(c.len(), 3);
    assert_eq!(c[0]["chance"], 0.25);
    assert_eq!(c[1]["chance"], 0.25);
    assert_eq!(c[2]["chance"], 0.5);
    assert_eq!(c[2]["d100"], d100(seed, 24));
    assert_eq!(c[2]["success"], true);
    assert_eq!(
        w.entity(id).unwrap().ext.get("damage.engine_levels"),
        Some(&0.)
    );
}
#[test]
fn manual_due_snapshots_at_source_time_and_later_button_cannot_use_a_changed_input() {
    let seed = seed_for(|s| d100(s, 6) == 40);
    let (mut w, id) = fixture(seed, 0.5);
    let start = begin(&mut w, id, false);
    at(&mut w, start + 21600);
    assert!(checks(&w).is_empty());
    let snapshot = &w.rules.repairs.as_ref().unwrap().units[&id].criticals[CRITICAL].checks[&6];
    assert_eq!(snapshot.chance, 0.25);
    assert!(snapshot.d100.is_none());
    assert_eq!(snapshot.due_at, start + 21600);
    // Counterfactual input probe only; no in-port hull-healing mechanic claimed.
    let e = w.entity_mut(id).unwrap();
    e.hp = e.hp_max;
    command(&mut w, id, "repair_due");
    let c = checks(&w);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0]["chance"], 0.25);
    assert_eq!(c[0]["d100"], 40);
    assert_eq!(c[0]["success"], false);
    assert_eq!(
        w.entity(id).unwrap().ext.get("damage.engine_levels"),
        Some(&1.)
    );
}
#[test]
fn manual_and_auto_switching_preserves_pending_snapshot_and_resolves_it_once() {
    let seed = seed_for(|s| d100(s, 6) <= 50);
    let (mut w, id) = fixture(seed, 1.);
    let start = begin(&mut w, id, false);
    at(&mut w, start + 21600);
    assert!(checks(&w).is_empty());
    command(&mut w, id, "repairs_auto");
    assert_eq!(checks(&w).len(), 1);
    assert_eq!(checks(&w)[0]["d100"], d100(seed, 6));
    command(&mut w, id, "repairs_manual");
    command(&mut w, id, "repairs_auto");
    command(&mut w, id, "repair_due");
    assert_eq!(checks(&w).len(), 1);
    assert_eq!(
        w.entity(id).unwrap().ext.get("damage.engine_levels"),
        Some(&0.)
    );
}
#[test]
fn failed_forty_eight_hour_check_permanently_caps_sea_repairs_without_begin_reset() {
    let seed =
        seed_for(|s| d100(s, 6) > 10 && d100(s, 12) > 10 && d100(s, 24) > 20 && d100(s, 48) > 20);
    let (mut w, id) = fixture(seed, 0.2);
    let start = begin(&mut w, id, true);
    for hour in [6, 12, 24, 48] {
        at(&mut w, start + hour * 3600);
    }
    assert_eq!(checks(&w).len(), 4);
    assert!(w.rules.repairs.as_ref().unwrap().units[&id].criticals[CRITICAL].sea_unrepairable);
    command(&mut w, id, "begin_repairs");
    command(&mut w, id, "repairs_auto");
    command(&mut w, id, "repair_due");
    at(&mut w, start + 60 * 3600);
    assert_eq!(checks(&w).len(), 4);
    assert_eq!(
        w.entity(id).unwrap().ext.get("damage.engine_levels"),
        Some(&1.)
    );
}
#[test]
fn mid_manual_checkpoint_full_world_save_and_private_observation_remain_exact() {
    let seed = seed_for(|s| d100(s, 6) > 50);
    let (mut w, id) = fixture(seed, 1.);
    let start = begin(&mut w, id, false);
    at(&mut w, start + 21600);
    let own = serde_json::to_value(harpoon_core::observe(&w, Side::Blue, 0, 0)).unwrap();
    let public=own["units"][0]["repairs"]["criticals"].as_object().unwrap();
    assert_eq!(public.len(),1);
    assert!(public.keys().all(|k|k.starts_with("own-system-casualty-")));
    assert!(!own.to_string().contains(CRITICAL),"private source identity must not leak through own repair projection");
    assert!(harpoon_core::observe(&w, Side::Red, 0, 0).units.is_empty());
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    restored.rules.validate_tables().unwrap();
    command(&mut w, id, "repair_due");
    command(&mut restored, id, "repair_due");
    command(&mut w, id, "repairs_auto");
    command(&mut restored, id, "repairs_auto");
    at(&mut w, start + 43200);
    at(&mut restored, start + 43200);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
#[test]
fn unrecorded_aggregate_old_engineering_damage_is_pending_and_cannot_be_invented() {
    let seed = seed_for(|_| true);
    let (mut w, id) = fixture(seed, 1.);
    w.rules.repairs.as_mut().unwrap().units.clear();
    command(&mut w, id, "begin_repairs");
    command(&mut w, id, "repairs_auto");
    at(&mut w, 21610);
    assert!(checks(&w).is_empty());
    assert_eq!(
        w.entity(id).unwrap().ext.get("damage.engine_levels"),
        Some(&1.)
    );
    assert!(w.rules.repairs.as_ref().unwrap().units[&id]
        .pending
        .is_some());
}
#[test]
fn another_real_engineering_critical_restarts_affected_clock_after_new_battle_end() {
    let seed = seed_for(|_| true);
    let (mut w, id) = fixture(seed, 1.);
    let start = begin(&mut w, id, false);
    harpoon_core::rules::damage_runtime::astern_cable_engineering(
        &mut w,
        id,
        "repair-fixture-second-hit",
    )
    .unwrap();
    command(&mut w, id, "repairs_auto");
    at(&mut w, start + 21600);
    assert!(checks(&w).is_empty());
    assert_eq!(
        w.entity(id).unwrap().ext.get("damage.engine_levels"),
        Some(&2.)
    );
    assert!(w.rules.repairs.as_ref().unwrap().units[&id]
        .pending
        .is_some());
    command(&mut w, id, "begin_repairs");
    let restarted=w.tick;
    assert!(restarted>start);
    assert_eq!(
        w.rules.repairs.as_ref().unwrap().units[&id]
            .session
            .as_ref()
            .unwrap()
            .began_at,
        restarted
    );
    assert!(w.rules.repairs.as_ref().unwrap().units[&id].criticals.values().all(|c|c.began_at==Some(restarted)));
}

#[test]
fn a_real_rehit_never_erases_an_existing_failed_forty_eight_hour_sea_cap() {
    let seed=seed_for(|s|d100(s,6)>10&&d100(s,12)>10&&d100(s,24)>20&&d100(s,48)>20);
    let (mut w,id)=fixture(seed,0.2);
    let start=begin(&mut w,id,true);
    for hour in [6,12,24,48]{at(&mut w,start+hour*3600);}
    let prior=w.rules.repairs.as_ref().unwrap().units[&id].criticals[CRITICAL].clone();
    assert!(prior.sea_unrepairable);
    harpoon_core::rules::damage_runtime::astern_cable_engineering(&mut w,id,"second-hit-after-sea-cap").unwrap();
    let current=&w.rules.repairs.as_ref().unwrap().units[&id].criticals[CRITICAL];
    assert!(current.sea_unrepairable);
    assert_eq!(serde_json::to_value(current).unwrap(),serde_json::to_value(prior).unwrap());
}

//! Withdraw only the unbound v4 legacy battery model. This is not a source
//! battery implementation or evidence of zero consumption/unlimited endurance.
use harpoon_core::{observe, scenario_from_json, Database, EntityId, Side, World};

fn world(profile: &str, depth_m: f64) -> World {
    let scenario = serde_json::json!({
        "id":"battery-source-boundary","name":"Unbound source battery boundary",
        "duration_s":10000,"seed":20261024,"context":{"rules_profile":profile},
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[{"side":"Blue","class":"ssk","name":"AUTHOR_GENERIC_DIESEL",
            "pos":[0,0],"speed_kts":20,"depth_m":depth_m,"fire_on":false}]
    });
    harpoon_core::load_scenario(
        scenario_from_json(&scenario.to_string()).unwrap(),
        Database::built_in(),
    )
    .unwrap()
}

#[test]
fn source_battery_v4_does_not_create_default_capacity_charge_or_constant_rates() {
    for depth in [15., 30.] {
        let mut w = world("hv2026-rts-v4", depth);
        for _ in 0..180 {
            harpoon_core::rules::asw::step(&mut w);
        }
        let e = w.entity(EntityId(1)).unwrap();
        assert!(
            e.ext.get("battery").is_none(),
            "no AnnexA capacity, technology or initial charge is bound at depth {depth}"
        );
        assert!(e.ext.get("damage.battery_capacity").is_none());
        assert!(observe(&w, Side::Blue, 0, 0).units[0].battery.is_none());
    }
}

#[test]
fn source_battery_v4_does_not_consume_or_recharge_a_stale_legacy_scalar() {
    for depth in [15., 30.] {
        let mut w = world("hv2026-rts-v4", depth);
        w.entity_mut(EntityId(1))
            .unwrap()
            .ext
            .insert("battery".into(), 50.);
        for _ in 0..180 {
            harpoon_core::rules::asw::step(&mut w);
        }
        assert_eq!(
            w.entity(EntityId(1)).unwrap().ext["battery"],
            50.,
            "unbound v4 must not apply old .05/s drain or .1/s charge at depth {depth}"
        );
    }
}

#[test]
fn source_battery_v4_stale_low_value_does_not_halve_speed_or_emit_legacy_low_event() {
    let mut w = world("hv2026-rts-v4", 30.);
    w.entity_mut(EntityId(1))
        .unwrap()
        .ext
        .insert("battery".into(), 5.);
    harpoon_core::rules::asw::step(&mut w);
    let e = w.entity(EntityId(1)).unwrap();
    assert_eq!(e.speed_kts, 20.);
    assert_eq!(
        e.ordered_speed_kts, 20.,
        "the old20 threshold is not a source speed gate"
    );
    assert!(!e.ext.contains_key("asw.battery_low_announced"));
    assert!(!w.events.iter().any(|event| matches!(&event.kind,
        harpoon_core::EventKind::RulesEvent { domain, rule, .. } if domain=="asw" && rule=="battery_low")));
}

#[test]
fn source_battery_v4_observation_does_not_relabel_old_scalar_as_known_charge() {
    for value in [0., 5., 100.] {
        let mut w = world("hv2026-rts-v4", 30.);
        w.entity_mut(EntityId(1))
            .unwrap()
            .ext
            .insert("battery".into(), value);
        w.entity_mut(EntityId(1))
            .unwrap()
            .ext
            .insert("damage.battery_capacity".into(), 50.);
        let own = observe(&w, Side::Blue, 0, 0);
        assert!(
            own.units[0].battery.is_none(),
            "known normalized damaged capacity does not prove current charge: stale{value}"
        );
        let restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
        assert!(observe(&restored, Side::Blue, 0, 0).units[0]
            .battery
            .is_none());
    }
}

#[test]
fn source_battery_v1_v2_v3_preserve_the_existing_hundred_unit_model() {
    for profile in ["rts-legacy-v1", "hv2026-rts-v2", "hv2026-rts-v3"] {
        let mut w = world(profile, 30.);
        for _ in 0..100 {
            harpoon_core::rules::asw::step(&mut w);
        }
        assert!(
            (w.entity(EntityId(1)).unwrap().ext["battery"] - 95.).abs() < 1e-9,
            "legacy drain {profile}"
        );
        w.entity_mut(EntityId(1)).unwrap().depth_m = 15.;
        for _ in 0..100 {
            harpoon_core::rules::asw::step(&mut w);
        }
        assert_eq!(
            w.entity(EntityId(1)).unwrap().ext["battery"],
            100.,
            "legacy charge {profile}"
        );
        assert_eq!(observe(&w, Side::Blue, 0, 0).units[0].battery, Some(100.));
        let e = w.entity_mut(EntityId(1)).unwrap();
        e.depth_m = 30.;
        e.ext.insert("battery".into(), 5.);
        e.speed_kts = 20.;
        e.ordered_speed_kts = 20.;
        harpoon_core::rules::asw::step(&mut w);
        assert_eq!(
            w.entity(EntityId(1)).unwrap().speed_kts,
            10.,
            "legacy speed gate {profile}"
        );
    }
}

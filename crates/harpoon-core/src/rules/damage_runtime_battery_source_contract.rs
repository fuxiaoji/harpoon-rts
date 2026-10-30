//! Battery capacity casualty does not prove an unknown state of charge.
use super::*;

fn fixture(profile: &str, charge: Option<f64>) -> (World, Entity, Resolved) {
    let sc: crate::Scenario = serde_json::from_value(serde_json::json!({
        "id":"battery-critical-source-boundary","name":"Explicit nonnuclear casualty fixture",
        "seed":20261024,"duration_s":10000,"context":{"rules_profile":profile},
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[{"side":"Blue","class":"ssk","pos":[0,0],"depth_m":50,"fire_on":false}]
    }))
    .unwrap();
    let mut w = crate::load_scenario(sc, crate::Database::built_in()).unwrap();
    let id = EntityId(1);
    let e = w.entity_mut(id).unwrap();
    e.rule_data = Some(PlatformDamage {
        nuclear:false, aip:false, size:"C".into(),
        source:"scenario_assumption: nonnuclear non-AIP casualty mechanism; no AnnexA battery rating asserted".into(),
        ..Default::default()
    });
    e.ext.insert("damage.battery_capacity".into(), 50.);
    if let Some(value) = charge {
        e.ext.insert("battery".into(), value);
    }
    let t = e.clone();
    let h = Resolved {
        hit: Hit {
            target: id,
            weapon: w.db.weapons.values().next().unwrap().clone(),
            underwater: true,
            bearing: Some(90.),
            shooter: None,
            identity: "battery-source-casualty".into(),
            tick: 0,
            secondary: false,
            torpedo_deep: false,
            impact_area: None,
            targeted_sensor: None,
            mine: None,
            navigation: None,
            projector: None,
            asw: None,
        },
        dp: 10.,
        penetrated: true,
        airburst: false,
        auto_flood: false,
        flood_half: false,
    };
    (w, t, h)
}

#[test]
fn source_battery_v4_critical_preserves_known_normalized_capacity_loss_without_filling_unknown_charge(
) {
    let (mut w, t, h) = fixture("hv2026-rts-v4", None);
    effect(
        &mut w,
        &t,
        &h,
        "battery",
        &mut Rng::new(9),
        "source-unknown-charge",
    );
    let e = w.entity(t.id).unwrap();
    assert!(
        (35. ..=47.5).contains(&e.ext["damage.battery_capacity"]),
        "existing50 normalized capacity still receives the legal D6x5percent proportional casualty"
    );
    assert!(
        e.ext.get("battery").is_none(),
        "a casualty is not evidence that the unknown starting charge was full"
    );
    assert!(w.events.iter().any(|event|matches!(&event.kind,
        EventKind::RulesEvent{rule,detail,..} if rule=="damage_roll"&&detail.contains("battery_capacity_loss"))));
}

#[test]
fn source_battery_known_legacy_charge_still_loses_charged_and_depleted_capacity_proportionally() {
    for profile in ["hv2026-rts-v2", "hv2026-rts-v4"] {
        let (mut w, t, h) = fixture(profile, Some(20.));
        effect(
            &mut w,
            &t,
            &h,
            "battery",
            &mut Rng::new(9),
            "source-known-charge",
        );
        let e = w.entity(t.id).unwrap();
        let fraction = e.ext["damage.battery_capacity"] / 50.;
        assert!(
            (e.ext["battery"] - 20. * fraction).abs() < 1e-9,
            "withdrawal must not delete the existing charged/depleted-cell proportional casualty"
        );
    }
}

#[test]
fn source_battery_v2_unknown_charge_preserves_legacy_critical_default() {
    let (mut w, t, h) = fixture("hv2026-rts-v2", None);
    effect(
        &mut w,
        &t,
        &h,
        "battery",
        &mut Rng::new(9),
        "legacy-charge-default",
    );
    let e = w.entity(t.id).unwrap();
    assert_eq!(e.ext["battery"], e.ext["damage.battery_capacity"]);
}

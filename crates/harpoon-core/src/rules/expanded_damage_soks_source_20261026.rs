//! Receiver boundary: an already-adjudicated sensor critical selects the real
//! independent SOKS component. This is not a whole weapon/critical-roll trial.
use super::*;
use crate::{Database, Scenario};

#[test]
fn native_soks_component_receives_common_sensor_casualty_and_uses_normal_repair() {
    let mut db =
        Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
    db.platforms.get_mut("ru_akula1").unwrap().sensors.clear();
    let sc:Scenario=serde_json::from_value(json!({
        "id":"soks-casualty-boundary","name":"scenario_assumption: exact physical retrofit casualty receiver",
        "seed":918,"duration_s":500,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","detection_2026":{
            "environment":{"source":"scenario_assumption: calm test water","sea_state":3,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
            "soks":{"source":"scenario_assumption: exact installed MNK100 laboratory retrofit, not original Akula fit",
                "fits":{"LOOK":{"source":"scenario_assumption: one physical installed component","system":"mnk100_kolos","installed":true,"enabled":true,"mount":"hull"}},
                "geometry":{"source":"scenario_assumption: experimental path mapping","crossing_policy":"exact_centerline_intersection","distinct_fix_separation_nm":0.1,"history_policy":"actual_runtime_only","trial_policy":"one_per_discrete_crossing","course_policy":"historical_local_tangent_after_three"}}}},
        "units":[{"side":"Blue","class":"ru_akula1","name":"LOOK","pos":[0,0],"speed_kts":0,"depth_m":150,"fire_on":false}]
    })).unwrap();
    let mut w = crate::load_scenario(sc, db).unwrap();
    let target = EntityId(1);
    let t = w.entity(target).unwrap().clone();
    let components = super::super::detection2026::soks::physical_sensor_components(&w, target);
    assert_eq!(
        components,
        vec![(
            "native.sensor:soks:mnk100_kolos".into(),
            "soks:mnk100_kolos".into()
        )]
    );
    let (component, binding) = components[0].clone();
    let hit = dr::Hit {
        target,
        weapon: w.db.weapons["us_aim120d"].clone(),
        underwater: false,
        bearing: None,
        shooter: None,
        identity: "scenario_assumption:adjudicated-sensor-critical".into(),
        tick: 0,
        secondary: false,
        torpedo_deep: false,
        impact_area: None,
        targeted_sensor: None,
        mine: None,
        projector: None,
        navigation: None,
        asw: None,
    };
    let resolved = dr::Resolved {
        hit,
        dp: 1.,
        penetrated: true,
        airburst: false,
        auto_flood: false,
        flood_half: false,
    };
    let mut r = w.rng.derive(Rng::hash_tag(
        0,
        target.0,
        "scenario:soks-casualty-boundary",
    ));
    sensor_critical(
        &mut w,
        &t,
        &resolved,
        "sensor",
        &mut r,
        "scenario:soks-native-critical",
    );
    assert!(
        !sensor_operational(&w, target, &binding).unwrap(),
        "the real critical disables the actual installed receiver"
    );
    assert_eq!(
        w.entity(target)
            .unwrap()
            .ext
            .get(&format!("damage.sensor.{binding}")),
        Some(&1.)
    );
    assert!(w.rules.expanded_damage.as_ref().unwrap().units[&target]
        .disabled
        .contains(&component));
    assert_eq!(
        super::super::detection2026::soks::physical_sensor_components(&w, target),
        components,
        "a disabled physical component still occupies its original casualty choice"
    );
    restore_component(&mut w, target, &component).unwrap();
    assert!(sensor_operational(&w, target, &binding).unwrap());
    assert!(!w
        .entity(target)
        .unwrap()
        .ext
        .contains_key(&format!("damage.sensor.{binding}")));
    assert_eq!(
        super::super::detection2026::soks::physical_sensor_components(&w, target),
        components,
        "repair restores the same physical component rather than making another receiver"
    );
}

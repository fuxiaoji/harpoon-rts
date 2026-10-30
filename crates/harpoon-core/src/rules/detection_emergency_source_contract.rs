//! Source28/3-6 Loud; source49/5-10 independent cavitation; combination pending.
use super::*;
use crate::{Command, Database, Scenario, Side};

fn fixture(pumpjet: bool) -> (World, EntityId, EntityId, Receiver) {
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"blow-acoustic","name":"Author mechanism fits; no real hardware claim","seed":24,"duration_s":5000,
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "navigation_2026":{"source":"scenario_assumption: author geometry","units":{
                "BLOW":{"source":"scenario_assumption: author hull","size":"C","merchant":false,
                    "passenger_liner":false,"cpp":false,"waterjet":false,"undamaged_max_kts":34,
                    "propulsion":"Nuclear","readiness":"Underway","submarine":{
                        "source":"scenario_assumption: author test envelope","maximum_depth_m":500,
                        "surface_max_kts":12,"water_depth_m":2000,"very_small":false}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_ford","name":"RECEIVER","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_virginia","name":"BLOW","pos":[0,20],"course_deg":0,"speed_kts":30,"depth_m":300,"fire_on":false}]
    })).unwrap();
    let mut db =
        Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
    db.platforms.get_mut("us_virginia").unwrap().max_depth_m = 500.;
    let mut w = crate::load_scenario(scenario, db).unwrap();
    let o = EntityId(1);
    let t = EntityId(2);
    for e in w.entities.values_mut() {
        e.sensors.clear();
        e.weapons.clear();
    }
    w.entity_mut(o).unwrap().sensors = vec![
        crate::world::SensorInstance {
            sensor: "sonar_p_hull".into(),
            active: true,
        },
        crate::world::SensorInstance {
            sensor: "sonar_a_hull".into(),
            active: true,
        },
    ];
    let receiver = Receiver::Sonar {
        source: "scenario_assumption: explicit controlled sonar".into(),
        // Author Gen5 avoids the unrelated Gen1-4 same-turn mode lock.
        generation: 5,
        passive_range_nm: Some(10.),
        active_range_nm: Some(10.),
        frequency: Frequency::Lmf,
        mount: SonarMount::Bow,
        active_torpedo_detection: false,
        high_speed_tow: false,
    };
    let s = w.rules.detection.as_mut().unwrap();
    for sensor in ["sonar_p_hull", "sonar_a_hull"] {
        s.config
            .bindings
            .insert(format!("us_ford:{sensor}"), receiver.clone());
    }
    s.config.targets.insert(
        "us_virginia".into(),
        TargetProfile {
            source: "scenario_assumption: author noise/pumpjet not real fit".into(),
            size: 2,
            noise: Some(1),
            anechoic: false,
            titanium: false,
            pumpjet,
            stealthy_ir: false,
            radar_signature: None,
        },
    );
    s.config.environment = Some(Environment {
        source: "scenario_assumption: controlled acoustic water".into(),
        sea_state: 0,
        precipitation: 0,
        visibility_percent: 60,
        shipping: 0,
        biologics: 0,
        water_depth_m: 2000.,
        layer: None,
        ice: 0,
        cloud: None,
    });
    (w, o, t, receiver)
}
fn blow(w: &mut World, t: EntityId) {
    let c: Command = serde_json::from_value(
        json!({"cmd":"navigation","unit":t,"action":{"type":"emergency_blow","depth_m":50}}),
    )
    .unwrap();
    crate::sim::tick(w, &[(Side::Red, c)]);
    assert!(super::super::navigation2026::submarine::emergency_blowing(
        w, t
    ));
}
#[test]
fn blow_acoustic_unresolved_nearby_masker_cannot_silently_disappear_from_a_real_other_target_scan()
{
    let (mut w, o, t, _) = fixture(true);
    blow(&mut w, t);
    let mut other = w.entity(t).unwrap().clone();
    other.id = EntityId(3);
    other.name = "AUTHOR_OTHER_TARGET".into();
    other.class = "author_other".into();
    other.pos = Vec2::new(0., 30.);
    w.entities.insert(other.id, other);
    let mut s = w.rules.detection.clone().unwrap();
    s.config.targets.insert(
        "author_other".into(),
        s.config.targets["us_virginia"].clone(),
    );
    assert!(
        evaluate_world(&w, &mut s, o, "sonar_p_hull", EntityId(3)).is_err(),
        "unresolved Blow in the same five-degree masking cone must not be treated as silence"
    );
    w.rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&t)
        .unwrap()
        .depth_operation = None;
    assert!(
        evaluate_world(&w, &mut s, o, "sonar_p_hull", EntityId(3)).is_ok(),
        "control: ordinary author target remains executable"
    );
}
#[test]
fn blow_acoustic_actual_command_is_loud_and_retains_independently_qualified_cavitation_in_both_consumers(
) {
    let (mut w, o, t, receiver) = fixture(false);
    assert!(
        cavitating(1, 30., 300., false).unwrap(),
        "independent source cavitation precondition"
    );
    blow(&mut w, t);
    let mut s = w.rules.detection.clone().unwrap();
    let scan = evaluate_world(&w, &mut s, o, "sonar_p_hull", t).unwrap();
    assert_eq!(
        scan.trace["inputs"]["target_modifier"], 10,
        "source Loud6 + independently established propeller4"
    );
    let strength = passive_strength(
        &w,
        &s,
        w.entity(o).unwrap(),
        w.entity(t).unwrap(),
        "sonar_p_hull",
        &receiver,
    )
    .unwrap();
    assert_eq!(
        strength,
        scan.trace["probability"].as_f64().unwrap(),
        "masking and detection must consume the same physical signature"
    );
    let mut ordinary = w.clone();
    ordinary
        .rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&t)
        .unwrap()
        .depth_operation = None;
    let mut ordinary_s = s.clone();
    let baseline = evaluate_world(&ordinary, &mut ordinary_s, o, "sonar_a_hull", t).unwrap();
    let actual = evaluate_world(&w, &mut s, o, "sonar_a_hull", t).unwrap();
    assert_eq!(
        actual.trace["inputs"]["target_modifier"], baseline.trace["inputs"]["target_modifier"],
        "Blow Loud is passive-only"
    );
}
#[test]
fn blow_acoustic_unresolved_blow_only_extra_modifier_is_pending_in_detection_and_masking_not_a_fabricated_total(
) {
    let (mut w, o, t, receiver) = fixture(true);
    blow(&mut w, t);
    let mut s = w.rules.detection.clone().unwrap();
    assert!(
        evaluate_world(&w, &mut s, o, "sonar_p_hull", t).is_err(),
        "no qualified independent propeller4; Blow-only combination must stay pending"
    );
    assert!(passive_strength(
        &w,
        &s,
        w.entity(o).unwrap(),
        w.entity(t).unwrap(),
        "sonar_p_hull",
        &receiver
    )
    .is_err());
    assert!(
        evaluate_world(&w, &mut s, o, "sonar_a_hull", t).is_ok(),
        "passive uncertainty cannot block unrelated active sonar"
    );
}

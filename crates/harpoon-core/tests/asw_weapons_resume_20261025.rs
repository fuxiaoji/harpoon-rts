//! Unrun source candidates. Native reports/FCS and real carrier/payload bodies
//! are required; no Contact/FCS/HP/die/Flight state is injected.
use harpoon_core::rules::{asw_weapons2026 as a, firecontrol2026::Quality};
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::json;

const ACTIVE_SONAR: &str = "sonar_a_asw_lab_active";
const PASSIVE_SONAR: &str = "asw_lab_passive";
// Explicit laboratory value: C-size strength0 + broadside1 + stationary0 +
// sea-state0/layer0 gives +1, so 16 * 1.25 = the printed 20nm centre row.
// The old 20nm experiment gave 25nm, tied between printed 24/26nm rows;
// the production source ambiguity guard correctly refused that input.
const ACTIVE_RANGE_NM: f64 = 16.0;

fn fixture(seed: u64, enabled: bool) -> World {
    fixture_launcher(seed, enabled, false)
}
fn fixture_launcher(seed: u64, enabled: bool, submarine_launcher: bool) -> World {
    let launcher = if submarine_launcher {
        "us_virginia"
    } else {
        "us_ford"
    };
    // Both original hulls have independently sourced combat-system entries.
    // The VLA mount and the sonar installation remain explicit laboratory fits.
    let sonar_mount = if submarine_launcher {
        "aft_digital"
    } else {
        "bow"
    };
    let context = json!({"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
        "asw_weapons_2026":if enabled{Some(json!({"source":"scenario_assumption: explicit laboratory carrier/payload fit, Annex E4 transport values","standoffs":{"us_vlasroc":{"source":"scenario_assumption: loaded carrier + Mk54 payload declaration; no real platform fit completeness claim","minimum_range_nm":2.5,"maximum_range_nm":12.0,"carrier_speed_kts":660.0,"guided":true,"payload":{"type":"torpedo","weapon":"us_mk54","speed_index":0,"entry_depth_m":50.0}}}}))}else{None},
        "torpedo_2026":{"source":"scenario_assumption: explicit payload environment and target signature only","environment":{"source":"scenario_assumption: deep open water","layer_m":null,"reverberant":false},"targets":{"cn_093":{"source":"scenario_assumption: size/noise/coating experiment","size":2,"noise":2,"anechoic":false,"foil_borne":false}}},
        "detection_2026":{"bindings":{
            format!("{launcher}:{ACTIVE_SONAR}"):{"type":"sonar","source":"scenario_assumption: laboratory active Gen5 LMF16nm, bow on surface/aft_digital on submerged experiment; no real hardware claim","generation":5,"passive_range_nm":null,"active_range_nm":ACTIVE_RANGE_NM,"frequency":"lmf","mount":sonar_mount,"active_torpedo_detection":false,"high_speed_tow":false},
            format!("{launcher}:{PASSIVE_SONAR}"):{"type":"sonar","source":"scenario_assumption: independent laboratory passive Gen5 LMF16nm, bow on surface/aft_digital on submerged experiment, for native held-contact classification; no real hardware claim","generation":5,"passive_range_nm":ACTIVE_RANGE_NM,"active_range_nm":null,"frequency":"lmf","mount":sonar_mount,"active_torpedo_detection":false,"high_speed_tow":false}},
            "environment":{"source":"scenario_assumption: open water","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
            "targets":{"cn_093":{"source":"scenario_assumption: controlled target signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}});
    let sc:Scenario=serde_json::from_value(json!({"id":"asw-native-resume","name":"scenario_assumption: native ASW experiment","seed":seed,"duration_s":5000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":context,"units":[
        {"side":"Blue","class":launcher,"name":"SENDER","pos":[0,0],"course_deg":if submarine_launcher{180}else{0},"speed_kts":if submarine_launcher{3}else{0},"depth_m":if submarine_launcher{50}else{0},"sonar_active":true,"fire_on":false},
        {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,3],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for (sensor_id, kind) in [
        (ACTIVE_SONAR, harpoon_core::db::SensorKind::SonarActive),
        (PASSIVE_SONAR, harpoon_core::db::SensorKind::SonarPassive),
    ] {
        let mut sensor = db.sensors.values().next().unwrap().clone();
        sensor.id = sensor_id.into();
        sensor.name = format!("scenario_assumption: {sensor_id} receiver");
        sensor.kind = kind;
        sensor.air_range_nm = 0.0;
        sensor.surface_range_nm = 0.0;
        sensor.missile_range_nm = 0.0;
        sensor.sub_range_nm = ACTIVE_RANGE_NM;
        db.sensors.insert(sensor.id.clone(), sensor);
    }
    // Keep original hull DP/stocks. Native passive LMF held-contact processing
    // earns Sub classification after a Tactical epoch; active LMF supplies the
    // independent measured fix/FCS. An active echo alone cannot earn that type.
    db.platforms.get_mut(launcher).unwrap().sensors =
        vec![ACTIVE_SONAR.into(), PASSIVE_SONAR.into()];
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    // Explicit experiment only: put the original Burke mount, with its actual
    // stock, on the source-CS Ford/Virginia hull. This does not claim that either
    // real platform carries VLA-ASROC. The submarine begins at its legal3kt
    // release speed, heading away; its declared aft receivers keep real contact.
    let mount = db.platforms["us_burke_2a"]
        .weapons
        .iter()
        .find(|m| m.weapon == "us_vlasroc")
        .unwrap()
        .clone();
    db.platforms.get_mut(launcher).unwrap().weapons.push(mount);
    db.validate().unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let actor = id(&w, "SENDER");
    assert!(
        w.entity(actor)
            .unwrap()
            .sensors
            .iter()
            .any(|s| s.sensor == ACTIVE_SONAR && s.active),
        "actual loaded laboratory sonar must start active"
    );
    assert!(w
        .rules
        .detection
        .as_ref()
        .unwrap()
        .config
        .bindings
        .contains_key(&format!("{launcher}:{ACTIVE_SONAR}")));
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSonar {
                unit: actor,
                active: true,
            },
        )],
    );
    run(&mut w, 725);
    w
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn run(w: &mut World, to: i64) {
    while w.tick < to {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn contact(w: &World) -> ContactId {
    let target = id(w, "TARGET");
    w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == target && !c.stale && !c.bearing_only)
        .unwrap_or_else(|| {
            let traces: Vec<_> = w
                .events
                .iter()
                .filter_map(|e| match &e.kind {
                    EventKind::RulesEvent { rule, detail, .. } if rule == "5.scan" => Some(detail),
                    _ => None,
                })
                .collect();
            panic!("native located contact required; actual sensor scan evidence: {traces:?}")
        })
        .id
}
fn fire(w: &mut World) {
    let actor = id(w, "SENDER");
    let c = contact(w);
    a::validate_fire(w, actor, c, "us_vlasroc").expect("native source FCS checkpoint required");
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: actor,
                contact: c,
                weapon: "us_vlasroc".into(),
                rounds: 1,
            },
        )],
    );
}
fn rounds(w: &World) -> u16 {
    w.entity(id(w, "SENDER"))
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_vlasroc")
        .unwrap()
        .ammo
}

#[test]
fn printed_standoff_modifiers_and_torpedo_only_coating_are_not_legacy_pk() {
    assert_eq!(
        a::placement_chance(Quality::Good, 27.0, false, 0, false, None).unwrap(),
        6
    );
    assert_eq!(
        a::placement_chance(Quality::Fair, 27.0, false, 0, false, None).unwrap(),
        3
    );
    assert_eq!(
        a::placement_chance(Quality::Poor, 27.0, false, 0, false, None).unwrap(),
        0
    );
    assert_eq!(
        a::placement_chance(Quality::Good, 27.0, false, 0, false, Some(200)).unwrap(),
        8
    );
    assert_eq!(
        a::placement_chance(Quality::Good, 27.0, false, 0, true, Some(200)).unwrap(),
        8
    );
    assert_eq!(
        a::placement_chance(Quality::Good, 3.0, true, 2, true, None).unwrap(),
        10
    );
    assert!(a::placement_chance(Quality::NA, 3.0, false, 0, false, None).is_err());
}
#[test]
fn no_range_gap_or_nuclear_yield_modifier_is_guessed() {
    assert_eq!(a::range_modifier(15.0).unwrap(), 0);
    assert!(a::range_modifier(15.05).is_err());
    assert_eq!(a::range_modifier(15.1).unwrap(), -1);
    assert!(a::placement_chance(Quality::Good, 27.0, false, 0, false, Some(50)).is_err());
}
#[test]
fn source_projector_example_rounds_each_threshold_once_for_entire_pattern() {
    assert_eq!(
        a::projector_thresholds(
            [Some(2.0), Some(9.0), Some(13.0)],
            Quality::Good,
            5.0,
            1,
            6,
            false
        )
        .unwrap(),
        [2, 7, 10]
    );
    assert_eq!(
        a::projector_thresholds(
            [None, Some(8.0), Some(13.0)],
            Quality::Fair,
            5.0,
            0,
            6,
            true
        )
        .unwrap(),
        [0, 2, 3]
    );
}
#[test]
fn projector_poor_sea_seven_and_impossible_speed_depth_are_explicit() {
    assert_eq!(
        a::projector_thresholds(
            [Some(2.0), Some(9.0), Some(13.0)],
            Quality::Poor,
            5.0,
            0,
            6,
            false
        )
        .unwrap(),
        [0; 3]
    );
    assert!(a::projector_thresholds(
        [None, Some(9.0), Some(13.0)],
        Quality::Good,
        5.0,
        0,
        7,
        false
    )
    .is_err());
    assert!(a::projector_thresholds(
        [None, Some(9.0), Some(13.0)],
        Quality::Good,
        22.0,
        3,
        6,
        false
    )
    .is_err());
}
#[test]
fn native_standoff_arrives_then_uses_one_placement_and_immediate_payload_attack() {
    let mut w = fixture(20261025, true);
    let before = rounds(&w);
    fire(&mut w);
    assert_eq!(rounds(&w), before - 1);
    let actor = id(&w, "SENDER");
    let carrier = *w
        .rules
        .asw_weapons
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .expect("actual registered carrier required");
    assert_eq!(
        w.entity(carrier)
            .unwrap()
            .projectile
            .as_ref()
            .unwrap()
            .weapon,
        "us_vlasroc"
    );
    let arrival_before = w.tick;
    run(&mut w, arrival_before + 40);
    let f = &w.rules.asw_weapons.as_ref().unwrap().flights[&carrier];
    let p = f
        .placement
        .as_ref()
        .expect("physical carrier must reach source aim");
    assert!(p.placed);
    assert!(p.tick > arrival_before);
    assert_eq!(p.chance, 10);
    let terminals:Vec<_>=w.events.iter().filter(|e|matches!(&e.kind,EventKind::RulesEvent{rule,detail,..} if rule=="10.2.5.source_terminal"&&detail.contains(&format!("\"torpedo\":{}",carrier.0)))).collect();
    assert_eq!(terminals.len(), 1);
    assert_eq!(terminals[0].tick, p.tick);
    assert!(w
        .events
        .iter()
        .all(|e| !matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="10.2.4.placement")));
    assert_eq!(
        w.rules
            .asw_weapons
            .as_ref()
            .unwrap()
            .flights
            .values()
            .filter(|f| f.shooter == actor)
            .count(),
        1
    );
}
#[test]
fn inflight_carrier_and_original_placement_survive_full_world_restore() {
    let mut w = fixture(20261025, true);
    fire(&mut w);
    let middle = w.tick + 2;
    run(&mut w, middle);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    run(&mut w, middle + 50);
    run(&mut restored, middle + 50);
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
}
#[test]
fn full_missing_fit_rejects_native_fire_before_ammunition_or_legacy_damage() {
    let mut w = fixture(20261025, false);
    let actor = id(&w, "SENDER");
    let c = contact(&w);
    let before = serde_json::to_vec(&w).unwrap();
    assert!(a::validate_fire(&w, actor, c, "us_vlasroc").is_err());
    assert_eq!(before, serde_json::to_vec(&w).unwrap());
    let ammunition = rounds(&w);
    let target = id(&w, "TARGET");
    let hp = w.entity(target).unwrap().hp;
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: actor,
                contact: c,
                weapon: "us_vlasroc".into(),
                rounds: 1,
            },
        )],
    );
    let to = w.tick + 60;
    run(&mut w, to);
    assert_eq!(rounds(&w), ammunition);
    assert_eq!(w.entity(target).unwrap().hp, hp);
    assert!(w.rules.asw_weapons.as_ref().unwrap().flights.is_empty());
}

#[test]
fn native_source_asw_submarine_release_records_source_six_noise_transient() {
    let mut w = fixture_launcher(20261025, true, true);
    let actor = id(&w, "SENDER");
    let before = rounds(&w);
    fire(&mut w);
    assert_eq!(rounds(&w), before - 1);
    let launch_tick = w.tick;
    let carrier = *w
        .rules
        .asw_weapons
        .as_ref()
        .unwrap()
        .flights
        .keys()
        .next()
        .unwrap();
    assert_eq!(
        w.entity(carrier)
            .unwrap()
            .projectile
            .as_ref()
            .unwrap()
            .weapon,
        "us_vlasroc"
    );
    let state = w.rules.detection.as_ref().unwrap();
    assert_eq!(state.transients.get(&actor), Some(&(launch_tick + 180, 6)));
    assert!(state.release_weapons.is_empty(),"the lifecycle consumes the launch-identity handoff after recording its trace");
    assert!(!state.unresolved_transients.contains_key(&actor));
    assert!(w.events.iter().any(|e|match &e.kind {
        EventKind::RulesEvent { rule, unit: Some(u), detail, .. } if rule=="5.lifecycle" && *u==actor=>{
            let d:serde_json::Value=serde_json::from_str(detail).unwrap();
            d["rule"]=="5.4.6.5.launch_transient" && d["noise_floor"]==6
                && d["weapon_id"]=="us_vlasroc" && d["projectile"]==carrier.0
        }
        _=>false,
    }));
}

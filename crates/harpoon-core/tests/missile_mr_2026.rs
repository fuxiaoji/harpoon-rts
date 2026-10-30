//! R-059: MR provenance is independent of known incoming signature/AA behavior.
//! Controlled incoming entities exercise defense, never a bypass for ASCM launch.
use harpoon_core::rules::aaw2026;
use harpoon_core::types::PlatformKind;
use harpoon_core::world::{Contact, ProjectileData};
use harpoon_core::{ContactId, Database, EntityId, Scenario, Side, Vec2, World};

const REAL_IDS: [&str; 6] = [
    "cn_yj83",
    "cn_yj82",
    "cn_yj62",
    "cn_yj18",
    "cn_yj12",
    "cn_yj12_surface_skim",
];

fn incoming(weapon: &str) -> (World, EntityId, EntityId, ContactId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(serde_json::json!({
        "id":"r059-mr-source-gate","name":"R059 controlled incoming defense",
        "duration_s":1000,"seed":20261024,
        "context":{"rules_profile":"hv2026-rts-v4",
            "flight_environment":{"wind_from_deg":0,"wind_kts":0},
            "radar_environment":{"sea_state":0,"precipitation":"None"}},
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[
            {"side":"Blue","class":"us_ford","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_052d","pos":[0,50],"speed_kts":0,"fire_on":false}
        ]
    }))
    .unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let b = w
        .entities
        .values()
        .find(|e| e.side == Side::Blue)
        .unwrap()
        .id;
    let r = w
        .entities
        .values()
        .find(|e| e.side == Side::Red)
        .unwrap()
        .id;
    let mut e = w.entity(r).unwrap().clone();
    e.id = w.allocate_id();
    let t = e.id;
    e.class = weapon.into();
    e.name = "controlled incoming source target".into();
    e.kind = PlatformKind::Missile;
    e.rule_data = None;
    e.pos = Vec2::new(0.0, 10.0);
    e.course_deg = 180.0;
    e.ordered_course_deg = 180.0;
    e.speed_kts = 594.0;
    e.ordered_speed_kts = 594.0;
    e.altitude_m = 50.0;
    e.ordered_altitude_m = 50.0;
    e.hp = 1.0;
    e.hp_max = 1.0;
    e.weapons.clear();
    e.sensors.clear();
    e.ext.clear();
    e.deck = None;
    e.mission = None;
    e.station = None;
    e.projectile = Some(ProjectileData {
        targeted_sensor: None,
        weapon: weapon.into(),
        target: Some(b),
        aim: Vec2::zero(),
        launched_by: r,
        launched_by_side: Side::Red,
        live_s: 0.0,
        enabled: true,
        decoyed: false,
        decoy_point: None,
        chaff_checked: false,
        ciws_checked: false,
        passes: 0,
    });
    w.entities.insert(t, e);
    let c = ContactId(5059);
    w.sides[Side::Blue.index()].contacts.insert(
        c,
        Contact {
            id: c,
            about: t,
            category: PlatformKind::Missile,
            class_name: None,
            estimate: Vec2::new(0.0, 10.0),
            est_course_deg: 180.0,
            est_speed_kts: 594.0,
            bearing_only: false,
            quality: 1.0,
            last_update_tick: 0,
            first_tick: 0,
            stale: false,
            sensors: vec!["radar_air_long".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.operations.as_mut().unwrap().tracks.insert(
        format!("{}:radar_air_long:{}", b.0, t.0),
        harpoon_core::rules::operations::RadarTrack {
            seen: 0,
            recognized: 0,
            engage: 0,
            last_eligible: 0,
            roll: 7,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("{}:{}", Side::Blue.index(), t.0),
        PlatformKind::Missile,
    );
    (w, b, t, c)
}

#[test]
fn six_real_incoming_profiles_preserve_unknown_mr() {
    for key in REAL_IDS {
        let p = &aaw2026::tables()["target_profiles"][key];
        assert!(
            p.is_object(),
            "real profile must retain independent AA source data: {key}"
        );
        assert!(
            p["maneuver_rating"].is_null(),
            "R059: no source defines {key} MR=0"
        );
        assert!(p["signature"].is_u64());
        assert!(p["terminal_maneuvers"].is_boolean());
    }
}

#[test]
fn classified_real_incoming_sam_prepare_reports_mr_instead_of_inventing_zero() {
    for key in REAL_IDS {
        let (w, b, t, _) = incoming(key);
        let def = w.db.weapons["us_essm_i"].clone();
        let result = aaw2026::world_prepare(&w, w.rules.aaw.as_ref().unwrap(), b, t, &def, 2);
        assert!(
            result.is_err(),
            "{key} must not acquire a source MR from a false maneuvering flag"
        );
        let reason = result.err().unwrap();
        assert!(
            reason.contains("MR"),
            "R059 must be the reached source gate: {key}: {reason}"
        );
    }
}

#[test]
fn real_incoming_missing_mr_never_spends_sam_or_creates_a_fake_outcome() {
    let (mut w, b, _, c) = incoming("cn_yj83");
    let before_rng = serde_json::to_value(&w.rng).unwrap();
    let before_mounts = serde_json::to_value(&w.entity(b).unwrap().weapons).unwrap();
    assert!(!harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "us_essm_i",
        2
    ));
    assert_eq!(serde_json::to_value(&w.rng).unwrap(), before_rng);
    assert_eq!(
        serde_json::to_value(&w.entity(b).unwrap().weapons).unwrap(),
        before_mounts
    );
    let state = w.rules.aaw.as_ref().unwrap();
    assert!(state.shots.is_empty());
    assert!(state.outcomes.is_empty());
    assert!(state.engagements.is_empty());
}

#[test]
fn same_real_incoming_ciws_keeps_independent_aa_data_without_mr() {
    let (mut w, b, t, c) = incoming("cn_yj83");
    w.entity_mut(t).unwrap().pos = Vec2::new(0.0, 0.8);
    w.sides[Side::Blue.index()]
        .contacts
        .get_mut(&c)
        .unwrap()
        .estimate = Vec2::new(0.0, 0.8);
    let sam = w.db.weapons["us_essm_i"].clone();
    let reason = aaw2026::world_prepare(&w, w.rules.aaw.as_ref().unwrap(), b, t, &sam, 2)
        .err()
        .expect("same object must reach the SAM MR gate");
    assert!(reason.contains("MR"), "{reason}");
    let gun = w.db.weapons["ciws_30mm"].clone();
    let permit =
        aaw2026::world_gun_solution(&w, w.rules.aaw.as_ref().unwrap(), b, t, &gun, 0).unwrap();
    assert!(permit.solution.probability > 0.0);
    assert!(!permit.solution.modifiers.contains_key("maneuver_rating"));
    let before = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == gun.id)
        .unwrap()
        .ammo;
    assert!(harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        &gun.id,
        1
    ));
    assert_eq!(
        w.entity(b)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == gun.id)
            .unwrap()
            .ammo,
        before - 1
    );
    assert!(w.rules.aaw.as_ref().unwrap().shots.is_empty());
}

fn author_profile() -> serde_json::Value {
    serde_json::json!({
        "signature":3,"maneuver_rating":0.0,"maneuvering":false,
        "terminal_maneuvers":false,"high_diving":false,"sprint_vehicle":false,
        "source":"scenario_assumption"
    })
}

#[test]
fn authored_incoming_requires_complete_fields_and_cannot_override_annex_ids() {
    for field in [
        "signature",
        "maneuver_rating",
        "maneuvering",
        "terminal_maneuvers",
        "high_diving",
        "sprint_vehicle",
        "source",
    ] {
        let mut incomplete = author_profile();
        incomplete.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<aaw2026::IncomingProfile>(incomplete).is_err(),
            "missing authored field must not acquire a default: {field}"
        );
    }
    for (key, source) in [
        ("cn_yj83", "scenario_assumption"),
        ("scenario_assumption_", "scenario_assumption"),
        ("scenario_assumption_fiction", "China Navy 2025 D-1"),
    ] {
        let mut profile = author_profile();
        profile["source"] = source.into();
        let config: aaw2026::Configuration = serde_json::from_value(serde_json::json!({
            "incoming_profiles":{(key):profile}
        }))
        .unwrap();
        assert!(aaw2026::validate_configuration(&config).is_err());
        let scenario: Scenario = serde_json::from_value(serde_json::json!({
            "id":"r059-invalid-author","name":"Invalid author provenance",
            "duration_s":10,"seed":20261024,
            "context":{"rules_profile":"hv2026-rts-v4","aaw_2026":config},
            "weather":{"visibility_nm":0,"jam":[0,0]},
            "units":[{"side":"Blue","class":"us_ford","pos":[0,0],"speed_kts":0}]
        }))
        .unwrap();
        let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
        assert!(
            harpoon_core::load_scenario(scenario, db).is_err(),
            "invalid author profile passed loader: {key}"
        );
    }
}

#[test]
fn explicit_fictional_mr_input_persists_without_claiming_real_missile_data() {
    let (mut w, b, t, _) = incoming("cn_yj83");
    let key = "scenario_assumption_mr_packet";
    let config: aaw2026::Configuration = serde_json::from_value(serde_json::json!({
        "incoming_profiles":{(key):author_profile()}
    }))
    .unwrap();
    aaw2026::validate_configuration(&config).unwrap();
    w.context.aaw_2026 = Some(config);
    let mut def = w.db.weapons["cn_yj83"].clone();
    def.id = key.into();
    def.name = "Scenario assumption MR input".into();
    def.rule_data = None;
    w.db.weapons.insert(key.into(), def);
    w.entity_mut(t).unwrap().class = key.into();
    w.entity_mut(t).unwrap().projectile.as_mut().unwrap().weapon = key.into();
    assert_eq!(aaw2026::incoming_radar_signature(&w, key), Some(3));
    let (td, _) = aaw2026::world_target(&w, t).unwrap();
    assert_eq!(td["source"], "scenario_assumption");
    assert_eq!(td["maneuver_rating"], 0.0);
    let sam = w.db.weapons["us_essm_i"].clone();
    let permit = aaw2026::world_prepare(&w, w.rules.aaw.as_ref().unwrap(), b, t, &sam, 2).unwrap();
    let baseline = aaw2026::missile_solution(&permit.attack, 2).unwrap();
    assert_eq!(baseline.probability, 0.91);
    let restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&restored.context.aaw_2026).unwrap(),
        serde_json::to_value(&w.context.aaw_2026).unwrap()
    );
    let again = aaw2026::world_prepare(
        &restored,
        restored.rules.aaw.as_ref().unwrap(),
        b,
        t,
        &sam,
        2,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(again).unwrap(),
        serde_json::to_value(permit).unwrap()
    );
    let profile = w
        .context
        .aaw_2026
        .as_mut()
        .unwrap()
        .incoming_profiles
        .get_mut(key)
        .unwrap();
    profile.maneuver_rating = Some(1.0);
    profile.maneuvering = true;
    profile.terminal_maneuvers = true;
    let permit = aaw2026::world_prepare(&w, w.rules.aaw.as_ref().unwrap(), b, t, &sam, 2).unwrap();
    let combined = aaw2026::missile_solution(&permit.attack, 2).unwrap();
    assert_eq!(combined.modifiers["maneuver_rating"], -1.0);
    assert_eq!(combined.modifiers["terminal_maneuvers"], -1.0);
    assert_eq!(combined.value, baseline.value - 2.0);
    // The author configuration never changes the real source profile.
    assert!(aaw2026::tables()["target_profiles"]["cn_yj83"]["maneuver_rating"].is_null());
}

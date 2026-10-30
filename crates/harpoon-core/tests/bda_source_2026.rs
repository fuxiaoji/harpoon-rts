//! R-049: a known contact is not an observed battle-damage assessment.
//! English Jan2026 PDF75/6-7 Assess returns to sensors; no enemy-HP estimator
//! is invented here. Victim/referee truth and old recordings remain separate.
use harpoon_core::rules::damage_runtime::RuleProfile;
use harpoon_core::{
    ContactId, Database, EntityId, EventKind, Scenario, Side, Vec2, Visibility, World,
};
use serde_json::json;

fn fixture(profile: &str) -> (World, EntityId, ContactId) {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    harpoon_core::rules::aircombat2026::configure_database(&mut db);
    let sc:Scenario=serde_json::from_value(json!({"id":"bda-source-boundary","name":"Known passive contact without visual BDA","duration_s":1000,"seed":2,
        "context":{"rules_profile":profile},"weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"cn_093","name":"OBSERVER","pos":[0,0],"depth_m":100,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_001","name":"VICTIM","pos":[30,0],"speed_kts":0,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let target = w.entities.values().find(|e| e.name == "VICTIM").unwrap().id;
    let observer = w
        .entities
        .values()
        .find(|e| e.name == "OBSERVER")
        .unwrap()
        .id;
    // Explicit controlled passive solution. Even perfect location/classification
    // is not a source-backed observation of the later impact or remaining hull.
    let passive: std::collections::BTreeSet<_> =
        w.db.sensors
            .values()
            .filter(|d| d.kind == harpoon_core::db::SensorKind::SonarPassive)
            .map(|d| d.id.clone())
            .collect();
    w.entity_mut(observer)
        .unwrap()
        .sensors
        .retain(|s| passive.contains(&s.sensor));
    let cid = ContactId(91);
    w.sides[0].contacts.insert(
        cid,
        harpoon_core::world::Contact {
            id: cid,
            about: target,
            category: harpoon_core::types::PlatformKind::Ship,
            class_name: None,
            estimate: Vec2::new(30., 0.),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec!["sonar_passive".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    if let Some(d) = w.rules.detection.as_mut() {
        d.categories.insert(
            format!("0:{}", target.0),
            harpoon_core::types::PlatformKind::Ship,
        );
    }
    (w, target, cid)
}
fn actual_known_hit(w: &mut World, target: EntityId) {
    let mut weapon = w.db.weapons["us_mk82"].clone();
    weapon.rule_data=Some(harpoon_core::rules::damage_runtime::WeaponDamage {ship_dp:Some(35.),penetration_cm:Some(4.),profile:"level".into(),bomb_kind:"LDGP".into(),source:"US Aircraft2025 PDF98/H1-3: Mk82 known physical-hit fixture; hit already adjudicated by referee".into(),..Default::default()});
    harpoon_core::damage::apply_hit_from(w, target, &weapon, false, Side::Blue);
}
fn assert_no_unobserved_enemy_damage(w: &World, target: EntityId, cid: ContactId) {
    let obs = harpoon_core::observe(w, Side::Blue, 0, 1000);
    let c = obs.contacts.iter().find(|c| c.id == cid).unwrap();
    assert_eq!(
        c.bda_hp_fraction, None,
        "a passive/shared track must not project true remaining enemy HP"
    );
    assert_eq!(
        c.bda_damage, 0.,
        "track quality is not observed damage points"
    );
    assert_eq!(
        c.bda_hits, 0,
        "an unobserved physical hit is not an automatic confirmed hit"
    );
    assert!(
        !obs.events
            .iter()
            .any(|e| matches!(e.kind,EventKind::Impact {unit,..} if unit==target)),
        "attacker must not receive the victim's exact damage/hp_left Impact copy"
    );
    assert!(
        !obs.events
            .iter()
            .any(|e| matches!(&e.kind,EventKind::RulesEvent {domain,..} if domain=="bda2026")),
        "raw BDA policy evidence must remain referee-only"
    );
    assert!(w.events.iter().any(|e|e.vis==Visibility::Referee&&matches!(&e.kind,EventKind::RulesEvent {rule,..} if rule=="R-049.unverified_enemy_bda_withheld")),"referee must retain an auditable disclosure boundary");
}
#[test]
fn v4_passive_known_track_does_not_receive_precise_hp_or_damage_after_real_hit() {
    let (mut w, t, c) = fixture("hv2026-rts-v4");
    let before = w.entity(t).unwrap().hp;
    actual_known_hit(&mut w, t);
    assert!(w.entity(t).unwrap().hp < before && w.entity(t).unwrap().hp > 0.);
    let red = harpoon_core::observe(&w, Side::Red, 0, 1000);
    assert!(red
        .units
        .iter()
        .any(|u| u.id == t && u.hp == w.entity(t).unwrap().hp));
    assert!(red
        .events
        .iter()
        .any(|e| matches!(e.kind,EventKind::Impact {unit,damage,..} if unit==t&&damage==35.)));
    assert_no_unobserved_enemy_damage(&w, t, c);
}
#[test]
fn v4_contact_quality_or_a_visual_sensor_label_is_not_a_damage_observation() {
    for (quality, label) in [(0.26, "sonar_passive"), (1., "radar"), (1., "visual")] {
        let (mut w, t, c) = fixture("hv2026-rts-v4");
        let track = w.sides[0].contacts.get_mut(&c).unwrap();
        track.quality = quality;
        track.sensors = vec![label.into()];
        track.class_name = Some("known scenario ship".into());
        actual_known_hit(&mut w, t);
        assert_no_unobserved_enemy_damage(&w, t, c);
    }
}
#[test]
fn v2_and_v3_recorded_bda_projection_keeps_its_previous_semantics() {
    for profile in ["hv2026-rts-v2", "hv2026-rts-v3"] {
        let (mut w, t, c) = fixture(profile);
        assert!(matches!(
            w.rules.profile,
            RuleProfile::Rts2026 | RuleProfile::Rts2026Operations
        ));
        actual_known_hit(&mut w, t);
        let obs = harpoon_core::observe(&w, Side::Blue, 0, 1000);
        let record = obs.contacts.iter().find(|r| r.id == c).unwrap();
        assert_eq!(record.bda_hits, 1);
        assert_eq!(record.bda_damage, 35.);
        assert_eq!(
            record.bda_hp_fraction,
            Some(w.entity(t).unwrap().hp / w.entity(t).unwrap().hp_max)
        );
        assert!(obs
            .events
            .iter()
            .any(|e| e.vis == Visibility::Side(Side::Blue)
                && matches!(e.kind,EventKind::Impact {unit,damage,..} if unit==t&&damage==35.)));
    }
}
#[test]
fn v4_withheld_bda_and_actual_victim_damage_resume_as_identical_worlds() {
    let (mut w, t, c) = fixture("hv2026-rts-v4");
    let mut resumed: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for world in [&mut w, &mut resumed] {
        actual_known_hit(world, t);
        actual_known_hit(world, t);
    }
    assert_no_unobserved_enemy_damage(&w, t, c);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&resumed).unwrap()
    );
}

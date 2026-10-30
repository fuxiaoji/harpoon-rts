//! Actual fire/terminal/fragment chains: referee adjudication stays out of both views.
use harpoon_core::rules::{aaw2026 as aaw, operations as ops};
use harpoon_core::types::PlatformKind;
use harpoon_core::world::{Contact, ProjectileData};
use harpoon_core::{
    ContactId, Database, EntityId, EventKind, Scenario, Side, Vec2, Visibility, World,
};
use serde_json::{json, Value};

fn contact(w: &mut World, side: Side, target: EntityId, id: ContactId, kind: PlatformKind) {
    let pos = w.entity(target).unwrap().pos;
    w.sides[side.index()].contacts.insert(
        id,
        Contact {
            id,
            about: target,
            category: kind,
            class_name: None,
            estimate: pos,
            est_course_deg: 180.,
            est_speed_kts: 594.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: 0,
            stale: false,
            sensors: vec!["radar_air_long".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    if let Some(d) = w.rules.detection.as_mut() {
        d.categories
            .insert(format!("{}:{}", side.index(), target.0), kind);
    }
}

fn fixture(profile: &str, range: f64) -> (World, EntityId, EntityId, ContactId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"aaw-privacy","name":"AAW referee privacy","duration_s":1000,"seed":20261024,
        "context":{"rules_profile":profile,
            "flight_environment":{"wind_from_deg":0,"wind_kts":0},
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "aaw_2026":{"incoming_profiles":{"scenario_assumption_privacy_incoming":{
                "signature":3,"maneuver_rating":0.,"maneuvering":false,
                "terminal_maneuvers":false,"high_diving":false,"sprint_vehicle":false,
                "source":"scenario_assumption"}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[
            {"side":"Blue","class":"us_ford","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_052d","pos":[0,50],"speed_kts":0,"fire_on":false}]
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
    let key = "scenario_assumption_privacy_incoming";
    let mut def = w.db.weapons["cn_yj83"].clone();
    def.id = key.into();
    def.name = "Scenario assumption incoming privacy target".into();
    def.rule_data = None;
    w.db.weapons.insert(key.into(), def);
    let mut e = w.entity(r).unwrap().clone();
    e.id = w.allocate_id();
    let target = e.id;
    e.class = key.into();
    e.name = "Controlled incoming privacy target".into();
    e.kind = PlatformKind::Missile;
    e.rule_data = None;
    e.pos = Vec2::new(0., range);
    e.course_deg = 180.;
    e.ordered_course_deg = 180.;
    e.speed_kts = 594.;
    e.ordered_speed_kts = 594.;
    e.altitude_m = 50.;
    e.ordered_altitude_m = 50.;
    e.hp = 1.;
    e.hp_max = 1.;
    e.weapons.clear();
    e.sensors.clear();
    e.ext.clear();
    e.deck = None;
    e.mission = None;
    e.station = None;
    e.projectile = Some(ProjectileData {
        targeted_sensor: None,
        weapon: key.into(),
        target: Some(b),
        aim: Vec2::zero(),
        launched_by: r,
        launched_by_side: Side::Red,
        live_s: 0.,
        enabled: true,
        decoyed: false,
        decoy_point: None,
        chaff_checked: false,
        ciws_checked: false,
        passes: 0,
    });
    w.entities.insert(target, e);
    let cid = ContactId(8059);
    contact(&mut w, Side::Blue, target, cid, PlatformKind::Missile);
    w.rules.operations.as_mut().unwrap().tracks.insert(
        format!("{}:radar_air_long:{}", b.0, target.0),
        ops::RadarTrack {
            seen: 0,
            recognized: 0,
            engage: 0,
            last_eligible: 0,
            roll: 7,
        },
    );
    (w, b, target, cid)
}

fn detail(w: &World, rule_name: &str) -> Value {
    let event = w
        .events
        .iter()
        .find(|e| {
            matches!(&e.kind,
        EventKind::RulesEvent { rule, .. } if rule == rule_name)
        })
        .expect("actual chain emitted referee evidence");
    match &event.kind {
        EventKind::RulesEvent { detail, .. } => serde_json::from_str(detail).unwrap(),
        _ => unreachable!(),
    }
}

fn referee_only(w: &World, rule_name: &str) {
    let events: Vec<_> = w
        .events
        .iter()
        .filter(|e| {
            matches!(&e.kind,
        EventKind::RulesEvent { rule, .. } if rule == rule_name)
        })
        .collect();
    assert!(!events.is_empty(), "actual chain never reached {rule_name}");
    for event in events {
        assert_eq!(
            event.vis,
            Visibility::Referee,
            "raw {rule_name} is referee evidence"
        );
        assert!(!event.vis.visible_to(Side::Blue));
        assert!(!event.vis.visible_to(Side::Red));
    }
    for side in [Side::Blue, Side::Red] {
        let view = harpoon_core::observe(w, side, 0, 1000);
        assert!(!view.events.iter().any(|e| matches!(&e.kind,
            EventKind::RulesEvent { rule, .. } if rule == rule_name)));
    }
}

fn own_gun_release(w: &World, shooter: EntityId, weapon: &str) {
    let own = harpoon_core::observe(w, Side::Blue, 0, 1000);
    let event = own
        .events
        .iter()
        .find(|e| {
            matches!(&e.kind,
        EventKind::RulesEvent { rule, .. } if rule == "8.1.3.gun_release")
        })
        .expect("manual and automatic fire retain an own-side release notification");
    match &event.kind {
        EventKind::RulesEvent {
            unit,
            contact,
            detail,
            ..
        } => {
            assert_eq!(*unit, Some(shooter));
            assert!(contact.is_none());
            let receipt: Value = serde_json::from_str(detail).unwrap();
            assert_eq!(
                receipt,
                json!({"weapon":weapon,"rounds":1,"physical_mount":0})
            );
        }
        _ => unreachable!(),
    }
    let enemy = harpoon_core::observe(w, Side::Red, 0, 1000);
    assert!(!enemy.events.iter().any(|e| e.seq == event.seq));
}

#[test]
fn actual_manual_ciws_hides_referee_outcome_but_keeps_own_ammo_and_release() {
    let (mut w, b, _, c) = fixture("hv2026-rts-v4", 0.8);
    let before = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "ciws_30mm")
        .unwrap()
        .ammo;
    assert!(harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "ciws_30mm",
        1
    ));
    let raw = detail(&w, "8.1.3.source_gun");
    assert!(raw["outcome"]["d100"].is_u64());
    assert!(raw["outcome"]["solution"]["modifiers"]["signature"].is_number());
    referee_only(&w, "8.1.3.source_gun");
    let own = harpoon_core::observe(&w, Side::Blue, 0, 1000);
    let unit = own.units.iter().find(|u| u.id == b).unwrap();
    assert_eq!(
        unit.weapons
            .iter()
            .find(|m| m.weapon == "ciws_30mm")
            .unwrap()
            .ammo,
        before - 1
    );
    own_gun_release(&w, b, "ciws_30mm");
}

#[test]
fn automatic_ciws_keeps_a_legal_own_release_after_raw_trace_is_private() {
    let (mut w, b, _, c) = fixture("hv2026-rts-v4", 0.8);
    let shooter = w.entity_mut(b).unwrap();
    shooter.weapons.retain(|m| m.weapon == "ciws_30mm");
    let before = shooter.weapons[0].ammo;
    shooter.fire_on = true;
    shooter.assigned = Some(c);
    harpoon_core::weapons::fire_control_scan(&mut w);
    assert_eq!(w.entity(b).unwrap().weapons[0].ammo, before - 1);
    own_gun_release(&w, b, "ciws_30mm");
    referee_only(&w, "8.1.3.source_gun");
}

#[test]
fn actual_sam_packet_terminal_keeps_one_raw_outcome_only_for_referee() {
    let (mut w, b, t, c) = fixture("hv2026-rts-v4", 10.);
    let before = w
        .entity(b)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_essm_i")
        .unwrap()
        .ammo;
    assert!(harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "us_essm_i",
        2
    ));
    let ids: Vec<_> = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .shots
        .keys()
        .copied()
        .collect();
    assert_eq!(ids.len(), 2);
    for id in ids {
        w.entity_mut(id).unwrap().pos = w.entity(t).unwrap().pos;
    }
    harpoon_core::weapons::advance_projectiles(&mut w);
    let raw = detail(&w, "8.1.1.packet_terminal");
    assert_eq!(raw["outcome"]["probability"], 0.91);
    assert!(raw["outcome"]["d100"].is_u64());
    assert_eq!(w.rules.aaw.as_ref().unwrap().outcomes.len(), 1);
    referee_only(&w, "8.1.1.packet_terminal");
    assert_eq!(
        w.entity(b)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "us_essm_i")
            .unwrap()
            .ammo,
        before - 2
    );
    assert!(harpoon_core::observe(&w, Side::Blue, 0, 1000)
        .events
        .iter()
        .any(|e| matches!(&e.kind,
        EventKind::RulesEvent { rule, .. } if rule == "8.1.1.source_launch")));
}

#[test]
fn actual_fragment_roll_hides_true_incoming_speed_and_preserves_raw_evidence() {
    let (mut w, b, t, _) = fixture("hv2026-rts-v4", 0.8);
    w.entity_mut(t).unwrap().speed_kts = 2000.;
    let gun = w.db.weapons["ciws_30mm"].clone();
    ops::intercept(&mut w, t, b, &gun);
    let raw = detail(&w, "8.1.5.fragment_roll");
    assert_eq!(raw["missile_speed_kts"], 2000.);
    assert_eq!(raw["probability"], 0.9);
    assert!(raw["d100"].is_u64());
    referee_only(&w, "8.1.5.fragment_roll");
}

#[test]
fn legacy_v3_fragment_chain_and_trace_dispatch_keep_their_saved_visibility() {
    let (mut w, b, t, _) = fixture("hv2026-rts-v3", 0.8);
    let gun = w.db.weapons["ciws_30mm"].clone();
    ops::intercept(&mut w, t, b, &gun);
    // v3 has no v4 packet/gun executor. These calls only verify its trace
    // compatibility; the fragment event above comes from actual adjudication.
    for rule in [
        "8.1.3.source_gun",
        "8.1.1.packet_terminal",
        "9.5.packet_terminal",
    ] {
        aaw::trace(&mut w, b, rule, json!({"compatibility_control":true}));
    }
    for rule in [
        "8.1.5.fragment_roll",
        "8.1.3.source_gun",
        "8.1.1.packet_terminal",
        "9.5.packet_terminal",
    ] {
        assert!(w.events.iter().any(|e| matches!(&e.kind,
            EventKind::RulesEvent { rule: r, .. } if r == rule)
            && e.vis == Visibility::Side(Side::Blue)));
        assert!(harpoon_core::observe(&w, Side::Blue, 0, 1000)
            .events
            .iter()
            .any(|e| matches!(&e.kind,
            EventKind::RulesEvent { rule: r, .. } if r == rule)));
    }
}

#[test]
fn actual_aam_packet_terminal_keeps_enemy_maneuver_details_for_referee() {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"aam-privacy","name":"Actual AAM referee privacy","duration_s":1000,"seed":20261024,
        "context":{"rules_profile":"hv2026-rts-v4","aviation_2026":{
            "source":"scenario_assumption: controlled AAM privacy geometry",
            "loads":{"cn_j15":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"},
                "us_f18e":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"}},
            "parking":{},"initial_fuel_nm":{"BLUE":700,"RED":700}}},
        "weather":{"visibility_nm":40,"jam":[0,0]},
        "units":[
            {"side":"Blue","class":"us_f18e","name":"BLUE","pos":[0,0],"course_deg":0,"speed_kts":600,"altitude_m":4000,"fire_on":false},
            {"side":"Red","class":"cn_j15","name":"RED","pos":[0,8],"course_deg":180,"speed_kts":600,"altitude_m":4000,"fire_on":false}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    w.rules.detection.as_mut().unwrap().config.environment =
        Some(harpoon_core::rules::detection2026::Environment {
            source: "scenario_assumption: explicit clear AAM privacy environment".into(),
            sea_state: 0,
            precipitation: 0,
            visibility_percent: 100,
            shipping: 0,
            biologics: 0,
            water_depth_m: 1000.,
            layer: None,
            ice: 0,
            cloud: None,
        });
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
    let c = ContactId(9059);
    contact(&mut w, Side::Blue, r, c, PlatformKind::Aircraft);
    assert!(harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        b,
        c,
        "us_aim120d",
        2
    ));
    let ids: Vec<_> = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .aam
        .packets
        .keys()
        .copied()
        .collect();
    assert_eq!(ids.len(), 2);
    for id in ids {
        w.entity_mut(id).unwrap().pos = w.entity(r).unwrap().pos;
    }
    harpoon_core::weapons::advance_projectiles(&mut w);
    let raw = detail(&w, "9.5.packet_terminal");
    assert_eq!(
        raw["outcome"]["solution"]["modifiers"]["maneuver_rating"],
        -4.5
    );
    assert!(raw["outcome"]["d100"].is_u64());
    referee_only(&w, "9.5.packet_terminal");
    assert!(harpoon_core::observe(&w, Side::Blue, 0, 1000)
        .events
        .iter()
        .any(|e| matches!(&e.kind,
        EventKind::RulesEvent { rule, .. } if rule == "9.5.source_launch")));
}

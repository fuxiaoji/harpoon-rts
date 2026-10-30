//! 8.5.4 native patterns; all hardware/DP below are explicit laboratory fits.
//! Native sonar generates the CID. No contact, evasion result or die is injected.
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
const DC: &str = "lab_dc";
const SONAR: &str = "lab_dc_sonar";
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn fixture(seed: u64) -> World {
    fixture_source(seed, 5, 7.)
}
fn fixture_source(seed: u64, generation: usize, damage_major: f64) -> World {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let mut weapon = db.weapons["cw_depth_charge"].clone();
    weapon.id = DC.into();
    weapon.name = "scenario_assumption: laboratory depth charge, no Mk11 fit claim".into();
    // These deliberately conflicting legacy values must never decide the attack.
    weapon.pk = 0.;
    weapon.damage = 999.;
    weapon.reload_s = 9000.;
    weapon.salvo = 1;
    db.weapons.insert(DC.into(), weapon);
    let mut mount = db.platforms["us_ford"].weapons[0].clone();
    mount.weapon = DC.into();
    mount.mounts = 4;
    // Per-mount database input becomes19*4=76 actual aggregate charges:
    // two9-charge rails + two4-charge projectors +50 central reserve.
    mount.ammo = 19;
    db.platforms.get_mut("us_ford").unwrap().weapons = vec![mount];
    let mut sonar = db.sensors.values().next().unwrap().clone();
    sonar.id = SONAR.into();
    sonar.name = "scenario_assumption: Gen5 laboratory bow sonar".into();
    sonar.kind = harpoon_core::db::SensorKind::SonarActive;
    sonar.sub_range_nm = 16.;
    db.sensors.insert(SONAR.into(), sonar);
    db.platforms.get_mut("us_ford").unwrap().sensors = vec![SONAR.into()];
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    let sc:Scenario=serde_json::from_value(json!({"id":"depth-charge-wide","name":"scenario_assumption: sourced procedure, experimental hardware and DP","seed":seed,"duration_s":20000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{
 "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
 "asw_weapons_2026":{"source":"scenario_assumption: fully explicit physical DC experiment","standoffs":{},"depth_charges":{DC:{"source":"scenario_assumption: experimental Major7/Minor3DP, not an Annex claim","damage_major":damage_major,"damage_minor":3.,"max_depth_band":7,"units":{"LAB":{"source":"scenario_assumption: two9-charge rails, two4-charge projectors,50reserve; original Ford sizeA","mount_index":0,"physical_size":0,"reserve":50,"mounts":[{"physical":0,"kind":"rail","capacity":9,"loaded":9},{"physical":1,"kind":"rail","capacity":9,"loaded":9},{"physical":2,"kind":"projector","capacity":4,"loaded":4},{"physical":3,"kind":"projector","capacity":4,"loaded":4}]}}}}},
 "detection_2026":{"bindings":{format!("us_ford:{SONAR}"):{"type":"sonar","source":"scenario_assumption: controlled Gen5 LMF bow receiver","generation":generation,"passive_range_nm":null,"active_range_nm":16.,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false}},"environment":{"source":"scenario_assumption: open-water laboratory","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},"targets":{"cn_093":{"source":"scenario_assumption: controlled acoustic signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
 "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"course_deg":90,"speed_kts":0,"sonar_active":true,"fire_on":false},{"side":"Red","class":"cn_093","name":"TARGET","pos":[0.08,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}]})).expect("source-red: depth charge configuration must be executable, not merely a pure calculator");
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    // This custom receiver is not a sonar_a* spawn shortcut. Use the actual
    // Full-profile command to begin active pings before native acquisition.
    let observer = id(&w, "LAB");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSonar {
                unit: observer,
                active: true,
            },
        )],
    );
    assert!(w
        .entity(observer)
        .unwrap()
        .sensors
        .iter()
        .any(|s| s.sensor == SONAR && s.active));
    let target = id(&w, "TARGET");
    while w.tick < 6000
        && !w.sides[0]
            .contacts
            .values()
            .any(|c| c.about == target && !c.stale && !c.bearing_only)
    {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(
        w.sides[0]
            .contacts
            .values()
            .any(|c| c.about == target && !c.stale && !c.bearing_only),
        "real native sonar must locate the target"
    );
    w
}
fn contact(w: &World) -> ContactId {
    w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == id(w, "TARGET") && !c.stale && !c.bearing_only)
        .unwrap()
        .id
}
fn action(w: &mut World, side: Side, unit: EntityId, action: Value) {
    let c: Command =
        serde_json::from_value(json!({"cmd":"asw","unit":unit,"action":action})).unwrap();
    c.validate_source(w, side).unwrap_or_else(|reason| {
        panic!(
            "actual ASW admission: {reason}; own={}",
            harpoon_core::rules::asw_weapons2026::depth_charge_own_view(w, unit)
        )
    });
    let since = w.event_seq;
    harpoon_core::sim::tick(w, &[(side, c)]);
    let rejected: Vec<_> = w
        .events
        .iter()
        .filter(|e| e.seq > since)
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. }
                if rule == "8.5.4.request_rejected" || rule == "8.5.4.conflicting_requests" =>
            {
                Some((rule, detail))
            }
            _ => None,
        })
        .collect();
    assert!(
        rejected.is_empty(),
        "actual post-movement ASW rejection: {rejected:?}"
    );
}
fn result(w: &World) -> Value {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "8.5.4.pattern_result" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .last()
        .expect("real source pattern resolution trace")
}
fn pattern(w: &mut World, band: usize) {
    let actor = id(w, "LAB");
    let cid = contact(w);
    action(
        w,
        Side::Blue,
        actor,
        json!({"type":"depth_charge_pattern","contact":cid,"weapon":DC,"charges":12,"depth_band":band,"distribution":"straight","allocations":[{"physical":0,"charges":6},{"physical":1,"charges":6}]}),
    );
}

fn advance(w: &mut World, to: i64) {
    while w.tick < to {
        harpoon_core::sim::tick(w, &[]);
    }
}
fn own(w: &World, actor: EntityId) -> Value {
    harpoon_core::rules::asw_weapons2026::depth_charge_own_view(w, actor)
}
fn command(unit: EntityId, a: Value) -> Command {
    serde_json::from_value(json!({"cmd":"asw","unit":unit,"action":a})).unwrap()
}
#[test]
fn dc03_actual_reload_transfers_reserve_once_at_the_original_1800s_clock() {
    let mut w = fixture(20261027);
    let actor = id(&w, "LAB");
    let sub = id(&w, "TARGET");
    action(
        &mut w,
        Side::Red,
        sub,
        json!({"type":"depth_charge_evasion","evading":true}),
    );
    pattern(&mut w, 3);
    let before = w.entity(actor).unwrap().weapons[0].ammo;
    assert_eq!(own(&w, actor)["weapons"][DC]["reserve"], 50);
    action(
        &mut w,
        Side::Blue,
        actor,
        json!({"type":"reload_depth_charges","weapon":DC,"allocations":[{"physical":0,"charges":6},{"physical":1,"charges":6}]}),
    );
    let due = own(&w, actor)["weapons"][DC]["reload"]["due"]
        .as_i64()
        .unwrap();
    assert_eq!(due, w.tick + 1800);
    advance(&mut w, due - 1);
    assert_eq!(own(&w, actor)["weapons"][DC]["reserve"], 50);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    advance(&mut w, due);
    advance(&mut restored, due);
    assert_eq!(own(&w, actor)["weapons"][DC]["reserve"], 38);
    assert_eq!(w.entity(actor).unwrap().weapons[0].ammo, before);
    assert!(own(&w, actor)["weapons"][DC]["mounts"]
        .as_array()
        .unwrap()
        .iter()
        .take(2)
        .all(|m| m["loaded"] == 9));
    assert!(
        serde_json::to_vec(&w).unwrap() == serde_json::to_vec(&restored).unwrap(),
        "complete World after an active reload checkpoint must match"
    );
    advance(&mut w, due + 1);
    assert_eq!(own(&w, actor)["weapons"][DC]["reserve"], 38);
}
#[test]
fn dc04_a_real_physical_rail_cannot_bypass_its_six_per_tactical_turn_limit() {
    let mut w = fixture(20261028);
    let actor = id(&w, "LAB");
    let sub = id(&w, "TARGET");
    action(
        &mut w,
        Side::Red,
        sub,
        json!({"type":"depth_charge_evasion","evading":true}),
    );
    pattern(&mut w, 3);
    let cid = contact(&w);
    let cmd = command(
        actor,
        json!({"type":"depth_charge_pattern","contact":cid,"weapon":DC,"charges":2,"depth_band":3,"distribution":"straight","allocations":[{"physical":0,"charges":1},{"physical":1,"charges":1}]}),
    );
    let before = serde_json::to_vec(&w).unwrap();
    assert!(cmd.validate_source(&w, Side::Blue).is_err());
    assert!(before == serde_json::to_vec(&w).unwrap());
    let next = (w.tick / 180 + 1) * 180;
    advance(&mut w, next);
    let stock = w.entity(actor).unwrap().weapons[0].ammo;
    let cmd = command(
        actor,
        json!({"type":"depth_charge_pattern","contact":contact(&w),"weapon":DC,"charges":2,"depth_band":3,"distribution":"straight","allocations":[{"physical":0,"charges":1},{"physical":1,"charges":1}]}),
    );
    cmd.validate_source(&w, Side::Blue)
        .expect("actual next-Tactical physical rail admission");
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
    assert_eq!(w.entity(actor).unwrap().weapons[0].ammo, stock - 2);
}
#[test]
fn dc05_natural_earlier_sonar_uses_two_true_adjacent_bands_and_one_later_selection() {
    let mut w = fixture_source(20261029, 3, 7.);
    let actor = id(&w, "LAB");
    let sub = id(&w, "TARGET");
    action(
        &mut w,
        Side::Red,
        sub,
        json!({"type":"depth_charge_evasion","evading":true}),
    );
    pattern(&mut w, 3);
    let blue = own(&w, actor);
    let red = own(&w, sub);
    let attack = blue["pending_attacks"][0]["attack"]
        .as_str()
        .unwrap()
        .to_string();
    let alias = red["incoming_patterns"][0]["attack"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(alias.starts_with(&format!("own-dc-{}-", sub.0)));
    assert_ne!(alias, attack);
    assert!(blue["pending_attacks"][0]["depth_report"].is_null());
    assert_eq!(red["incoming_patterns"][0]["own_depth_band_at_pass"], 1);
    let invalid = command(
        sub,
        json!({"type":"depth_charge_depth_report","attack":alias,"bands":[3,4]}),
    );
    let before = serde_json::to_vec(&w).unwrap();
    assert!(invalid.validate_source(&w, Side::Red).is_err());
    assert!(before == serde_json::to_vec(&w).unwrap());
    action(
        &mut w,
        Side::Red,
        sub,
        json!({"type":"depth_charge_depth_report","attack":alias,"bands":[1,2]}),
    );
    assert_eq!(
        own(&w, actor)["pending_attacks"][0]["depth_report"],
        json!([1, 2])
    );
    assert!(
        !w.events.iter().any(
            |e| matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="8.5.4.pattern_result")
        ),
        "unanswered depth selection cannot roll a die"
    );
    action(
        &mut w,
        Side::Blue,
        actor,
        json!({"type":"depth_charge_select_depth","attack":attack,"depth_band":1}),
    );
    assert!(result(&w)["raw_d100"].is_number());
    let again = command(
        actor,
        json!({"type":"depth_charge_select_depth","attack":attack,"depth_band":1}),
    );
    let before = serde_json::to_vec(&w).unwrap();
    assert!(again.validate_source(&w, Side::Blue).is_err());
    assert!(before == serde_json::to_vec(&w).unwrap());
}
#[test]
fn dc06_a_dispersed_pattern_uses_exact_half_rails_half_projectors() {
    let mut w = fixture(20261030);
    let actor = id(&w, "LAB");
    let sub = id(&w, "TARGET");
    action(
        &mut w,
        Side::Red,
        sub,
        json!({"type":"depth_charge_evasion","evading":false}),
    );
    let ammo = w.entity(actor).unwrap().weapons[0].ammo;
    let cid = contact(&w);
    action(
        &mut w,
        Side::Blue,
        actor,
        json!({"type":"depth_charge_pattern","contact":cid,"weapon":DC,"charges":8,"depth_band":1,"distribution":"dispersed","allocations":[{"physical":0,"charges":2},{"physical":1,"charges":2},{"physical":2,"charges":2},{"physical":3,"charges":2}]}),
    );
    assert_eq!(w.entity(actor).unwrap().weapons[0].ammo, ammo - 8);
    assert_eq!(result(&w)["major_threshold"], 8);
    assert_eq!(result(&w)["minor_threshold"], 16);
}

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
    // Database ammo is per mount: the loader multiplies by4. The explicit
    // original inventory is9+9+4+4 loaded +50 reserve =76 aggregate charges.
    let mut mount = db.platforms["us_ford"].weapons[0].clone();
    mount.weapon = DC.into();
    mount.mounts = 4;
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
 "asw_weapons_2026":{"source":"scenario_assumption: fully explicit physical DC experiment","standoffs":{},"depth_charges":{DC:{"source":"scenario_assumption: experimental Major7/Minor3DP, not an Annex claim","damage_major":7.,"damage_minor":3.,"max_depth_band":7,"units":{"LAB":{"source":"scenario_assumption: two9-charge rails, two4-charge projectors,50reserve; original Ford sizeA","mount_index":0,"physical_size":0,"reserve":50,"mounts":[{"physical":0,"kind":"rail","capacity":9,"loaded":9},{"physical":1,"kind":"rail","capacity":9,"loaded":9},{"physical":2,"kind":"projector","capacity":4,"loaded":4},{"physical":3,"kind":"projector","capacity":4,"loaded":4}]}}}}},
 "detection_2026":{"bindings":{format!("us_ford:{SONAR}"):{"type":"sonar","source":"scenario_assumption: controlled Gen5 LMF bow receiver","generation":5,"passive_range_nm":null,"active_range_nm":16.,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false}},"environment":{"source":"scenario_assumption: open-water laboratory","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},"targets":{"cn_093":{"source":"scenario_assumption: controlled acoustic signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
 "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"course_deg":90,"speed_kts":0,"sonar_active":true,"fire_on":false},{"side":"Red","class":"cn_093","name":"TARGET","pos":[0.08,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}]})).expect("source-red: depth charge configuration must be executable, not merely a pure calculator");
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    // The legacy spawn convenience flag recognizes sonar_a* IDs only. This
    // laboratory receiver has its own ID: activate its actual SonarActive
    // instance through the real Full-profile operator command.
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
#[test]
fn dc01_real_native_pattern_spends_twelve_physical_charges_and_ignores_legacy_pk() {
    let mut w = fixture(20261026);
    let actor = id(&w, "LAB");
    let sub = id(&w, "TARGET");
    action(
        &mut w,
        Side::Red,
        sub,
        json!({"type":"depth_charge_evasion","evading":false}),
    );
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: actor,
                speed_kts: 10.,
            },
        )],
    );
    let ammo = w.entity(actor).unwrap().weapons[0].ammo;
    pattern(&mut w, 1);
    assert_eq!(w.entity(actor).unwrap().weapons[0].ammo, ammo - 12);
    let r = result(&w);
    assert_eq!(r["charges"], 12);
    assert_eq!(r["major_threshold"], 6);
    assert_eq!(r["minor_threshold"], 12);
    assert_eq!(
        r["modified_d100"].as_i64().unwrap(),
        r["raw_d100"].as_i64().unwrap() - 10
    );
    assert!(r["actual_pass_yards"].as_f64().unwrap() <= 250.);
    assert!(
        w.entities
            .values()
            .all(|e| e.projectile.as_ref().is_none_or(|p| p.weapon != DC)),
        "no fictional projectile or999legacyDP"
    );
    let before = serde_json::to_vec(&w).unwrap();
    let restored: World = serde_json::from_slice(&before).unwrap();
    assert_eq!(before, serde_json::to_vec(&restored).unwrap());
}
#[test]
fn dc02_wrong_declared_depth_still_spends_the_complete_pattern_without_a_hidden_preflight_or_damage(
) {
    let mut w = fixture(20261027);
    let actor = id(&w, "LAB");
    let sub = id(&w, "TARGET");
    action(
        &mut w,
        Side::Red,
        sub,
        json!({"type":"depth_charge_evasion","evading":true}),
    );
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSpeed {
                unit: actor,
                speed_kts: 10.,
            },
        )],
    );
    let hp = w.entity(sub).unwrap().hp;
    let ammo = w.entity(actor).unwrap().weapons[0].ammo;
    pattern(&mut w, 3);
    assert_eq!(w.entity(actor).unwrap().weapons[0].ammo, ammo - 12);
    assert_eq!(w.entity(sub).unwrap().hp, hp);
    assert_eq!(result(&w)["severity"], "miss");
    assert!(result(&w)["reason"].as_str().unwrap().contains("depth"));
}

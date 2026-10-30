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
    fixture_with_radio(seed, false)
}
fn fixture_with_radio(seed: u64, communicate: bool) -> World {
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
    let mut director = db.platforms["us_ford"].clone();
    director.id = "lab_dc_director".into();
    director.name = "scenario_assumption: independent ASW observer".into();
    director.sensors = vec![SONAR.into()];
    db.platforms.insert(director.id.clone(), director);
    db.platforms.get_mut("us_ford").unwrap().sensors.clear();
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    let mut scene = json!({"id":"depth-charge-wide","name":"scenario_assumption: sourced procedure, experimental hardware and DP","seed":seed,"duration_s":20000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{
 "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
 "asw_weapons_2026":{"source":"scenario_assumption: fully explicit physical DC experiment","standoffs":{},"depth_charges":{DC:{"source":"scenario_assumption: experimental Major7/Minor3DP, not an Annex claim","damage_major":7.,"damage_minor":3.,"max_depth_band":7,"units":{"LAB":{"source":"scenario_assumption: two9-charge rails, two4-charge projectors,50reserve; original Ford sizeA","mount_index":0,"physical_size":0,"reserve":50,"mounts":[{"physical":0,"kind":"rail","capacity":9,"loaded":9},{"physical":1,"kind":"rail","capacity":9,"loaded":9},{"physical":2,"kind":"projector","capacity":4,"loaded":4},{"physical":3,"kind":"projector","capacity":4,"loaded":4}]}}}}},
 "detection_2026":{"bindings":{format!("lab_dc_director:{SONAR}"):{"type":"sonar","source":"scenario_assumption: controlled Gen5 LMF bow receiver","generation":5,"passive_range_nm":null,"active_range_nm":16.,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false}},"environment":{"source":"scenario_assumption: open-water laboratory","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},"targets":{"cn_093":{"source":"scenario_assumption: controlled acoustic signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
 "units":[{"side":"Blue","class":"lab_dc_director","name":"OBSERVER","pos":[0,1],"course_deg":180,"speed_kts":0,"sonar_active":true,"fire_on":false},{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"course_deg":90,"speed_kts":0,"sonar_active":true,"fire_on":false},{"side":"Red","class":"cn_093","name":"TARGET","pos":[0.08,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}]});
    if communicate {
        scene["context"]["communications_2026"] = json!({"source":"scenario_assumption: explicit directed radio, no Annex inference","units":{"OBSERVER":{"source":"scenario_assumption","fits":{"voice":{"source":"scenario_assumption","protocol":"lab-dc-voice","send":true,"receive":true,"operational":true,"peers":["LAB"],"warning_kind":"voice","transport":{"type":"hf","max_range_nm":20}}}},"LAB":{"source":"scenario_assumption","fits":{"voice":{"source":"scenario_assumption","protocol":"lab-dc-voice","send":true,"receive":true,"operational":true,"peers":[],"warning_kind":"voice","transport":{"type":"hf","max_range_nm":20}}}}}});
    }
    let sc: Scenario = serde_json::from_value(scene).expect("source DC configuration");
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    // Its real bow points toward the target instead of placing the target in
    // the >=150-degree baffles. Start actual pings using the operator command;
    // the custom laboratory ID does not use the sonar_a* spawn shortcut.
    let observer = id(&w, "OBSERVER");
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
fn dc07_a_friendly_side_fused_contact_does_not_supply_free_communication_or_own_localization() {
    let w = fixture(20261031);
    let actor = id(&w, "LAB");
    let observer = id(&w, "OBSERVER");
    let target = id(&w, "TARGET");
    assert!(w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .any(|t| t.observer == observer && t.target == target && t.report.is_some()));
    assert!(!w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .any(|t| t.observer == actor && t.target == target));
    let cmd:Command=serde_json::from_value(json!({"cmd":"asw","unit":actor,"action":{"type":"depth_charge_pattern","contact":contact(&w),"weapon":DC,"charges":12,"depth_band":1,"distribution":"straight","allocations":[{"physical":0,"charges":6},{"physical":1,"charges":6}]}})).unwrap();
    let before = serde_json::to_vec(&w).unwrap();
    assert!(
        cmd.validate_source(&w, Side::Blue).is_err(),
        "8.5.4 the firing ship must detect the datum itself or actually receive friendly direction"
    );
    assert!(before == serde_json::to_vec(&w).unwrap());
}

#[test]
fn dc08_actual_received_friendly_location_enables_the_pattern_without_creating_an_own_track() {
    let mut w = fixture_with_radio(20261101, true);
    let actor = id(&w, "LAB");
    let observer = id(&w, "OBSERVER");
    let target = id(&w, "TARGET");
    let cid = contact(&w);
    let send:Command=serde_json::from_value(json!({"cmd":"communications","unit":observer,"action":{"type":"send_warning","contact":cid,"sensor":SONAR,"fit":"voice","receivers":[actor]}})).unwrap();
    send.validate_source(&w, Side::Blue)
        .expect("actual native-report voice delivery admission");
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, send)]);
    let report =
        harpoon_core::rules::communications2026::received_located_report(&w, actor, target)
            .unwrap()
            .expect("actual delivered raw located report");
    assert_eq!(report.observer, observer);
    assert!(report.measured_position.is_some());
    assert!(!w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .any(|t| t.observer == actor && t.target == target));
    let ammo = w.entity(actor).unwrap().weapons[0].ammo;
    let c:Command=serde_json::from_value(json!({"cmd":"asw","unit":actor,"action":{"type":"depth_charge_pattern","contact":contact(&w),"weapon":DC,"charges":12,"depth_band":3,"distribution":"straight","allocations":[{"physical":0,"charges":6},{"physical":1,"charges":6}]}})).unwrap();
    c.validate_source(&w, Side::Blue)
        .expect("actual received-located-report pattern admission");
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, c)]);
    assert_eq!(w.entity(actor).unwrap().weapons[0].ammo, ammo - 12);
    assert!(
        !w.rules
            .aaw
            .as_ref()
            .unwrap()
            .firecontrol
            .tracks
            .values()
            .any(|t| t.observer == actor && t.target == target),
        "a real directing report does not fabricate a native receiver/FCS"
    );
}

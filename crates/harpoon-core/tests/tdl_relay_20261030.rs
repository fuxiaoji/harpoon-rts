//! English2026 §6.3.9 PDF74/6-6 + Annex L Relay footnote (USN 1 Dec 25 PDF94/
//! L-1, China's Navy 1 Dec 25 PDF96/K-3), personally viewed 2026-10-30:
//! "Relay: TDL coverage can be extended by using similarly equipped platforms
//! to relay data to platforms over the horizon." Only us_link_16, cn_dts_03
//! and cn_tjn_906 print Relay; Link 11's beyond-LOS 300 is printed HF (ES-
//! detectable), never relay. Nothing prints a hop limit, per-hop delay or
//! probability, so every case below is a deterministic read-only gate.
//!
//! Three real Ford experimental units at 0/20/40 nm: the printed Link 16
//! surface LOS literal caps each hop at 25 nm, so 40 nm needs exactly one
//! similarly equipped relay. Declared fits/topology/positions are explicit
//! author assumptions; the Red lab contact is never used by the relay (enemy
//! truth is not borrowed). No HP/Contact/dice injection, no seed search (zero
//! dice). REQUIRES root's `pub mod relay;` declaration before the batch
//! compiles; not run by this task.

use harpoon_core::rules::communications2026::{self as c, relay};
use harpoon_core::{Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

const P: &str = "tdl_annex_lab_passive";

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

fn fit(system: &str, kind: &str, transport: Value, peers: Vec<&str>) -> Value {
    json!({"source":"scenario_assumption: declared operational topology only; real hardware is separately source-qualified",
        "annex_system":system,"protocol":system,"send":true,"receive":true,
        "operational":true,"peers":peers,"warning_kind":kind,"automatic_warning":false,
        "targeting_system":false,"transport":transport})
}

fn los() -> Value {
    json!({"type":"line_of_sight","max_range_nm":900.,"antenna_height_m":1000.})
}

fn ford(side: &str, name: &str, y: f64) -> Value {
    json!({"side":side,"class":"us_ford","name":name,"pos":[0,y],"course_deg":90,"speed_kts":0,"fire_on":false})
}

/// Standard lab: three Blue Ford units 0/20/40 nm plus the inert Red lab
/// contact; declared Link 16 LOS chain SENDER -> RELAY -> RECEIVER.
fn units() -> Value {
    json!([
        ford("Blue","SENDER",0.),
        ford("Blue","RELAY",20.),
        ford("Blue","RECEIVER",40.),
        {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}])
}

fn comm_units() -> Value {
    json!({
        "SENDER":{"source":"scenario_assumption: declared position","fits":{"lab":fit("us_link_16","real_time",los(),vec!["RELAY"])}},
        "RELAY":{"source":"scenario_assumption: declared position","fits":{"lab":fit("us_link_16","real_time",los(),vec!["RECEIVER"])}},
        "RECEIVER":{"source":"scenario_assumption: declared position","fits":{"lab":fit("us_link_16","real_time",los(),vec![])}}})
}

fn scenario(units_json: Value, comm: Value) -> Scenario {
    serde_json::from_value(json!({"id":"tdl-relay-lab","name":"scenario_assumption: TDL relay lab",
        "duration_s":10000,"seed":1,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":2,"precipitation":"None"},
            "communications_2026":{"source":"scenario_assumption: declared routes","units":comm},
            "detection_2026":{"bindings":{format!("us_ford:{P}"):{"type":"sonar","source":"scenario_assumption: laboratory passive sensor, not a real Ford fit",
                "generation":5,"passive_range_nm":4.,"active_range_nm":null,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false}},
                "environment":{"source":"scenario_assumption: controlled open water","sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
                "targets":{"cn_093":{"source":"scenario_assumption: controlled acoustic signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
        "units":units_json})).unwrap()
}

fn database() -> Database {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut sensor = db.sensors.values().next().unwrap().clone();
    sensor.id = P.into();
    sensor.name = "scenario_assumption: laboratory passive source".into();
    sensor.kind = harpoon_core::db::SensorKind::SonarPassive;
    sensor.air_range_nm = 0.;
    sensor.surface_range_nm = 0.;
    sensor.missile_range_nm = 0.;
    sensor.sub_range_nm = 4.;
    db.sensors.insert(P.into(), sensor);
    db.platforms.get_mut("us_ford").unwrap().sensors = vec![P.into()];
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    db.validate().unwrap();
    db
}

fn load(units_json: Value, comm: Value) -> World {
    harpoon_core::load_scenario(scenario(units_json, comm), database())
        .expect("the declared relay lab must load")
}

fn path(w: &World) -> Result<Vec<EntityId>, String> {
    relay::find_path(w, id(w, "SENDER"), id(w, "RECEIVER"), "lab")
}

/// 40 nm total needs exactly one similarly equipped relay: the direct gate is
/// refused by the printed 25 nm Link 16 surface LOS cap, find_path returns
/// the three-unit chain, and every hop stays inside the printed cap.
#[test]
fn relay_path_three_ford_units_within_printed_per_hop_cap() {
    let w = load(units(), comm_units());
    let sender = id(&w, "SENDER");
    c::validate_transport(&w, Side::Blue, sender, "lab", &[id(&w, "RECEIVER")])
        .expect("transport uses the qualified relay chain");
    let hops = path(&w).expect("the declared 0/20/40 nm chain qualifies hop by hop");
    assert_eq!(
        hops,
        vec![sender, id(&w, "RELAY"), id(&w, "RECEIVER")],
        "the fewest-hop chain uses the declared relay exactly once"
    );
    let mut total = 0.;
    for pair in hops.windows(2) {
        let range = w.entity(pair[0])
            .unwrap()
            .pos
            .range_to(w.entity(pair[1]).unwrap().pos);
        assert!(
            range <= 25.0,
            "every hop stays inside the printed Link 16 surface LOS literal, got {range}"
        );
        total += range;
    }
    assert!((total - 40.0).abs() < 1e-9, "the whole path is the authored 40 nm, got {total}");
}

/// The printed Relay cell is the gate: a CEC chain (no Relay printed) never
/// relays, a Link16-Link11-Link16 mixed chain stays source-pending, and the
/// printed HF numeric range never becomes a relay leg.
#[test]
fn only_the_printed_relay_terminal_and_similar_equipment_relay() {
    let mut cec = comm_units();
    for name in ["SENDER", "RELAY", "RECEIVER"] {
        cec[name]["fits"]["lab"]["annex_system"] = json!("us_cec");
        cec[name]["fits"]["lab"]["protocol"] = json!("us_cec");
    }
    let err = path(&load(units(), cec)).err().expect("CEC never prints Relay");
    assert!(err.contains("does not print Relay"), "{err}");

    let mut mixed = comm_units();
    mixed["RELAY"]["fits"]["lab"]["annex_system"] = json!("us_link_11");
    mixed["RELAY"]["fits"]["lab"]["protocol"] = json!("us_link_11");
    mixed["RELAY"]["fits"]["lab"]["warning_kind"] = json!("near_real_time");
    let err = path(&load(units(), mixed))
        .err()
        .expect("cross-terminal chains stay source-pending");
    assert!(err.contains("no qualified Annex L relay path"), "{err}");

    let hf = json!({"type":"hf","max_range_nm":900.});
    let mut all_hf = comm_units();
    for name in ["SENDER", "RELAY", "RECEIVER"] {
        all_hf[name]["fits"]["lab"]["transport"] = hf.clone();
    }
    assert!(harpoon_core::load_scenario(scenario(units(), all_hf), database()).is_err(),
        "Link 16 HF is rejected at scenario initialization");
}

/// Same side, alive, operational, declared directed edges only: a dead or
/// silent relay breaks the chain, a declared enemy relay is never borrowed,
/// and with only an enemy relay there is no path at all.
#[test]
fn relay_stays_alive_same_side_and_on_declared_edges_only() {
    let mut silent = comm_units();
    silent["RELAY"]["fits"]["lab"]["operational"] = json!(false);
    let err = path(&load(units(), silent))
        .err()
        .expect("an unfit relay breaks the chain");
    assert!(err.contains("no qualified Annex L relay path"), "{err}");

    let mut broken = comm_units();
    broken["RELAY"]["fits"]["lab"]["peers"] = json!([]);
    let err = path(&load(units(), broken))
        .err()
        .expect("a broken directed edge is a dead edge");
    assert!(err.contains("no qualified Annex L relay path"), "{err}");

    let mut with_enemy = units();
    with_enemy.as_array_mut().unwrap().push(ford("Red", "ENEMYRELAY", 20.5));
    let mut enemy_comm = comm_units();
    enemy_comm["ENEMYRELAY"] = json!({"source":"scenario_assumption: declared position",
        "fits":{"lab":fit("us_link_16","real_time",los(),vec!["RECEIVER"])}});
    enemy_comm["SENDER"]["fits"]["lab"]["peers"] = json!(["ENEMYRELAY", "RELAY"]);
    let w = load(with_enemy, enemy_comm);
    let hops = path(&w).expect("the blue relay remains qualified beside a declared enemy relay");
    assert_eq!(
        hops,
        vec![id(&w, "SENDER"), id(&w, "RELAY"), id(&w, "RECEIVER")],
        "enemy units are never borrowed as relay hops"
    );
    assert!(!hops.contains(&id(&w, "ENEMYRELAY")));

    let mut enemy_only = units();
    enemy_only
        .as_array_mut()
        .unwrap()
        .retain(|u| u["name"] != json!("RELAY"));
    let mut enemy_only_comm = comm_units();
    enemy_only_comm.as_object_mut().unwrap().remove("RELAY");
    enemy_only_comm["ENEMYRELAY"] = json!({"source":"scenario_assumption: declared position",
        "fits":{"lab":fit("us_link_16","real_time",los(),vec!["RECEIVER"])}});
    enemy_only_comm["SENDER"]["fits"]["lab"]["peers"] = json!(["ENEMYRELAY"]);
    assert!(harpoon_core::load_scenario(scenario(enemy_only, enemy_only_comm), database()).is_err(),
        "a declared fitted relay without a matching entity is rejected at initialization");
}

/// Save/restore purity: the same World yields the identical path, and a
/// refused search leaves the serialized state byte-identical.
#[test]
fn restored_world_yields_identical_path_and_rejections_change_nothing() {
    let w = load(units(), comm_units());
    let saved = serde_json::to_vec(&w).unwrap();
    let direct = path(&w).unwrap();
    let restored: World = serde_json::from_slice(&saved).unwrap();
    assert_eq!(path(&restored).unwrap(), direct, "the path is save/restore stable");
    assert_eq!(
        saved,
        serde_json::to_vec(&w).unwrap(),
        "find_path is a pure read-only precheck"
    );

    let mut cec = comm_units();
    for name in ["SENDER", "RELAY", "RECEIVER"] {
        cec[name]["fits"]["lab"]["annex_system"] = json!("us_cec");
        cec[name]["fits"]["lab"]["protocol"] = json!("us_cec");
    }
    let w = load(units(), cec);
    let saved = serde_json::to_vec(&w).unwrap();
    assert!(path(&w).is_err());
    assert_eq!(
        saved,
        serde_json::to_vec(&w).unwrap(),
        "a refused relay search changes nothing for the delivery wiring"
    );
}

#[test]
fn public_transport_follows_distinct_peer_fit_names() {
    let mut comm = comm_units();
    comm["SENDER"]["fits"]["lab"]["peer_fits"] = json!({"RELAY":"middle"});
    let middle = comm["RELAY"]["fits"]["lab"].take();
    comm["RELAY"]["fits"] = json!({"middle":middle});
    comm["RELAY"]["fits"]["middle"]["peer_fits"] = json!({"RECEIVER":"end"});
    let end = comm["RECEIVER"]["fits"]["lab"].take();
    comm["RECEIVER"]["fits"] = json!({"end":end});
    let w = load(units(), comm);
    let sender = id(&w, "SENDER");
    let receiver = id(&w, "RECEIVER");
    assert_eq!(c::validate_transport(&w, Side::Blue, sender, "lab", &[receiver]).unwrap(), "us_link_16");
    assert_eq!(path(&w).unwrap(), vec![sender, id(&w, "RELAY"), receiver]);
}

/// Native sensor report is sent across the real 40 nm chain; save/restore
/// and a pre-adjudicated communication damage input exercise delivery again.
#[test]
fn native_warning_relay_delivers_restores_and_rechecks_damage() {
    let mut w = load(units(), comm_units());
    while w.tick < 181 { harpoon_core::sim::tick(&mut w, &[]); }
    let sender = id(&w, "SENDER");
    let receiver = id(&w, "RECEIVER");
    let relay_unit = id(&w, "RELAY");
    let target = id(&w, "TARGET");
    let contact = w.sides[0].contacts.values().find(|c| !c.stale && c.about == target)
        .expect("seed 1 must produce a native contact").id;
    assert!(c::own_view(&w, sender).unwrap()["reports"].as_array().unwrap().iter()
        .any(|r| r["sensor"] == P && r["contact"] == json!(contact)));
    let action: c::Action = serde_json::from_value(json!({"type":"send_warning", "contact":contact,
        "sensor":P, "fit":"lab", "receivers":[receiver]})).unwrap();
    c::apply(&mut w, Side::Blue, sender, &action).unwrap();
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    let mut damaged: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    c::set_communication_cause(&mut damaged, relay_unit, "scenario_assumption: preadjudicated damage", true).unwrap();
    c::detection_open(&mut w);
    c::detection_open(&mut restored);
    c::detection_open(&mut damaged);
    assert_eq!(serde_json::to_value(&w).unwrap(), serde_json::to_value(&restored).unwrap());
    let received = c::own_view(&w, receiver).unwrap();
    assert!(received["messages"].as_array().unwrap().iter().any(|m|
        m["direction"] == "received" && m["state"] == "delivered" && m["protocol"] == "us_link_16"));
    let sent = c::own_view(&damaged, sender).unwrap();
    assert!(sent["messages"].as_array().unwrap().iter().any(|m| m["state"] == "failed"));
    assert!(!c::own_view(&damaged, receiver).unwrap()["messages"].as_array().unwrap().iter()
        .any(|m| m["direction"] == "received"));
}

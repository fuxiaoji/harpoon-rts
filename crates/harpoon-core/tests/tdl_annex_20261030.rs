//! English2026 §6.3.9 PDF74 + USN2025 Annex L PDF94 and Annex A PDF7; only Ford exists as an exact DB class.
//! Only TDL class fits are real source claims. Geometry, topology and the
//! passive laboratory sensor added to Ford are explicit author assumptions.
//! No contacts/reports/dice/damage are injected. No real CEC JR is invented.
use harpoon_core::rules::communications2026 as c;
use harpoon_core::{Command, ContactId, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

const P: &str = "tdl_annex_lab_passive";
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn endpoint(system: &str, kind: &str, transport: Value, peers: Vec<&str>) -> Value {
    json!({"source":"scenario_assumption: declared operational topology only; real hardware is separately source-qualified",
        "annex_system":system,"protocol":system,"send":true,"receive":true,
        "operational":true,"peers":peers,"warning_kind":kind,"automatic_warning":false,
        "targeting_system":false,"transport":transport})
}
fn scenario(tx: Value, rx: Value, distance: f64, class: &str, seed: u64) -> Scenario {
    serde_json::from_value(json!({"id":"tdl-source-fit-lab","name":"scenario_assumption: TDL-only source fit test",
        "duration_s":10000,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":2,"precipitation":"None"},
            "communications_2026":{"source":"scenario_assumption: declared routes","units":{
                "SENDER":{"source":"scenario_assumption: declared position","fits":{"lab":tx}},
                "RECEIVER":{"source":"scenario_assumption: declared position","fits":{"lab":rx}}}},
            "detection_2026":{"bindings":{format!("us_ford:{P}"):{"type":"sonar","source":"scenario_assumption: laboratory passive sensor, not a real Ford fit",
                "generation":5,"passive_range_nm":4.,"active_range_nm":null,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false}},
                "environment":{"source":"scenario_assumption: controlled open water","sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
                "targets":{"cn_093":{"source":"scenario_assumption: controlled acoustic signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
        "units":[
            {"side":"Blue","class":class,"name":"SENDER","pos":[0,0],"course_deg":90,"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":"us_ford","name":"RECEIVER","pos":[0,-distance],"course_deg":90,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap()
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
fn load(tx: Value, rx: Value, distance: f64) -> Result<World, String> {
    harpoon_core::load_scenario(scenario(tx, rx, distance, "us_ford", 1), database())
}
fn los() -> Value {
    json!({"type":"line_of_sight","max_range_nm":900.,"antenna_height_m":1000.})
}
fn pair(system: &str, kind: &str, transport: Value) -> (Value, Value) {
    (
        endpoint(system, kind, transport.clone(), vec!["RECEIVER"]),
        endpoint(system, kind, transport, vec![]),
    )
}
fn gate(w: &World) -> Result<String, String> {
    c::validate_transport(w, Side::Blue, id(w, "SENDER"), "lab", &[id(w, "RECEIVER")])
}

#[test]
fn printed_surface_cap_overrides_author_range_and_rejects_without_mutation() {
    let (tx, rx) = pair("us_link_16", "real_time", los());
    let at = load(tx.clone(), rx.clone(), 25.).unwrap();
    assert!(gate(&at).is_ok());
    let beyond = load(tx, rx, 25.001).unwrap();
    let before = serde_json::to_value(&beyond).unwrap();
    assert!(gate(&beyond).unwrap_err().contains("Annex L"));
    assert_eq!(before, serde_json::to_value(&beyond).unwrap());
}
#[test]
fn hf_numeric_cap_and_relay_word_do_not_become_unlimited_hf() {
    let hf = json!({"type":"hf","max_range_nm":900.});
    let (tx, rx) = pair("us_link_11", "near_real_time", hf.clone());
    assert!(gate(&load(tx.clone(), rx.clone(), 300.).unwrap()).is_ok());
    assert!(gate(&load(tx, rx, 300.001).unwrap()).is_err());
    let (tx, rx) = pair("us_link_16", "real_time", hf);
    assert!(
        load(tx, rx, 100.).is_err(),
        "Relay is not a numeric HF entry"
    );
}
#[test]
fn satellite_requires_printed_yes_and_author_operational_coverage() {
    let sat = json!({"type":"satellite","available":true});
    let (tx, rx) = pair("us_link_16", "real_time", sat.clone());
    assert!(gate(&load(tx, rx, 600.).unwrap()).is_ok());
    let (tx, rx) = pair("us_link_11", "near_real_time", sat);
    assert!(load(tx, rx, 5.).is_err(), "dash does not grant SATCOM");
    let (tx, rx) = pair(
        "us_link_16",
        "real_time",
        json!({"type":"satellite","available":false}),
    );
    assert!(gate(&load(tx, rx, 5.).unwrap()).is_err());
}
#[test]
fn source_type_targeting_and_platform_qualification_cannot_be_forged() {
    let (tx, rx) = pair("us_link_16", "near_real_time", los());
    assert!(load(tx, rx, 1.).is_err());
    let (mut tx, rx) = pair("us_link_16", "real_time", los());
    tx["targeting_system"] = json!(true);
    assert!(
        load(tx, rx, 1.).is_err(),
        "RT alone is not a dedicated targeting system"
    );
    let (tx, rx) = pair("us_link_16", "real_time", los());
    let result = harpoon_core::load_scenario(scenario(tx, rx, 1., "cn_052d", 1), database());
    assert!(
        result.err().unwrap().contains("Annex L"),
        "a class name cannot acquire a foreign qualified fit"
    );
    let (tx, rx) = pair("us_link_14", "time_late", los());
    assert!(load(tx, rx, 1.).is_err(), "Link14 printed receive-only");
    let (tx, rx) = pair("us_link_16", "real_time", json!({"type":"author_directed"}));
    assert!(load(tx, rx, 1.).is_err());
}
#[test]
fn source_compatibility_is_printed_and_mixed_author_gateway_stays_pending() {
    let tx = endpoint("us_link_16", "real_time", los(), vec!["RECEIVER"]);
    let rx = endpoint("us_link_11", "near_real_time", los(), vec![]);
    assert!(gate(&load(tx.clone(), rx.clone(), 1.).unwrap()).is_ok());
    let mut invented = tx.clone();
    invented["compatible_protocols"] = json!(["us_cec"]);
    assert!(load(invented, rx.clone(), 1.).is_err());
    let mut legacy = rx;
    legacy.as_object_mut().unwrap().remove("annex_system");
    let w = load(tx, legacy, 1.).unwrap();
    assert!(gate(&w).is_err());
}
#[test]
fn native_report_send_delivery_and_restore_use_the_source_fit() {
    let (tx, rx) = pair("us_link_16", "real_time", los());
    let mut selected = None;
    for seed in 1..=32 {
        let mut w = harpoon_core::load_scenario(
            scenario(tx.clone(), rx.clone(), 1., "us_ford", seed),
            database(),
        )
        .unwrap();
        while w.tick < 181 {
            harpoon_core::sim::tick(&mut w, &[]);
        }
        let sender = id(&w, "SENDER");
        let target = id(&w, "TARGET");
        let contact = w.sides[0]
            .contacts
            .values()
            .find(|c| !c.stale && c.about == target)
            .map(|c| c.id);
        if let Some(contact) = contact {
            if c::own_view(&w, sender).unwrap()["reports"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["sensor"] == P && r["contact"] == json!(contact))
            {
                selected = Some((w, sender, contact));
                break;
            }
        }
    }
    let (mut w, sender, contact): (World, EntityId, ContactId) =
        selected.expect("fixture must produce its own native report");
    let receiver = id(&w, "RECEIVER");
    let action:c::Action=serde_json::from_value(json!({"type":"send_warning","contact":contact,"sensor":P,"fit":"lab","receivers":[receiver]})).unwrap();
    c::validate_action(&w, Side::Blue, sender, &action).unwrap();
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    let cmd: Command =
        serde_json::from_value(json!({"cmd":"communications","unit":sender,"action":action}))
            .unwrap();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd.clone())]);
    harpoon_core::sim::tick(&mut restored, &[(Side::Blue, cmd)]);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    let received = c::own_view(&w, receiver).unwrap();
    assert!(
        received["messages"].as_array().is_some_and(|v| v
            .iter()
            .any(|m| m["direction"] == "received"
                && m["state"] == "delivered"
                && m["protocol"] == "us_link_16")),
        "native warning must reach actual receiver through the qualified terminal: {received}"
    );
}

#[test]
fn source_reagan_hawklink_does_not_qualify_the_generic_nimitz_database_class() {
    let (tx, rx) = pair("us_hawklink_arq_59", "real_time", los());
    let result = harpoon_core::load_scenario(scenario(tx, rx, 1., "us_nimitz", 1), database());
    assert!(
        result.err().unwrap().contains("Annex L"),
        "a specific source variant cannot be silently assigned to generic Nimitz"
    );
    let (mut tx, mut rx) = pair("us_cec", "real_time", los());
    tx["targeting_system"] = json!(true);
    rx["targeting_system"] = json!(true);
    assert!(
        gate(&load(tx, rx, 1.).unwrap()).is_ok(),
        "the exact Ford source does qualify USG-2B CEC"
    );
}

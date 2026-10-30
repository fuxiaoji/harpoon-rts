//! ZCODE-COMMS-20261029: printed-row-94 radio qualification consumption over
//! the real public TDL command path.
//!
//! Source, personally re-viewed 2026-10-29 (source pages in
//! artifacts/rules-book-wide-20261026/zcode-parallel-20261028/expanded-atoms/
//! source-pages/):
//! - Expanded manual PDF14 row 94 and PDF15 row 94 (《海上指挥》主要/次要水面
//!   战斗舰船（经调整), identical wording: 1-2 远程无线电通信。舰船不能与
//!   地平线以外的单位通信。3-4 近程无线电通信。舰船不能与同一编队中的其他
//!   舰船进行无线电通信。5-6 飞机无线电通信。舰船不能与飞机通信。
//! - English2026 PDF135/printed 14-9 §14.5 "Communications": long-range
//!   prevents communication with units beyond the horizon; short-range
//!   prevents radio with other ships in the same formation; aircraft radio is
//!   necessary to communicate with any aircraft.
//!
//! The game prints neither an antenna-height table nor a formation roster, so
//! both are explicit typed scenario_assumption inputs
//! (`links::RadioQualification`). Formation is never inferred from side or
//! peer lists; an HF antenna height is never assumed; a link without the
//! required declaration stays source-pending while the damaged group matters.
//!
//! Damage inputs are the already-adjudicated disabled-room boundary (batch-52
//! precedent: direct insert into the expanded disabled set, repaired through
//! the public `restore_component`). No natural weapon hit, no injected die,
//! no modified HP/contact/track; every report comes from a real native scan
//! and every send is the real typed communications command.
use harpoon_core::rules::communications2026 as c;
use harpoon_core::rules::expanded_damage2026::restore_component;
use harpoon_core::{Command, ContactId, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

const P: &str = "cas_radio_lab_passive";
const PROTOCOL: &str = "cas-radio";
const TARGET: &str = "cas_radio_lab_target";
const HF: &str = "hf";
const AUTHOR: &str = "author_directed";

/// horizon_nm(2 m, 2 m) = 1.23·(√6.5617+√6.5617) ≈ 6.30 nm.
const SHIP_HEIGHT_QUAL: f64 = 2.0;
/// horizon_nm(2 m, 300 m + 0 m) ≈ 41.7 nm for the ship–helo pairs.
const HELO_ALT_M: i64 = 300;

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn run(w: &mut World, to: i64) {
    while w.tick < to {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
    assert_eq!(w.tick, to);
}
fn database() -> Database {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut p = db.sensors.values().next().unwrap().clone();
    p.id = P.into();
    p.name = format!("scenario_assumption:{P}");
    p.kind = harpoon_core::db::SensorKind::SonarPassive;
    p.air_range_nm = 0.;
    p.surface_range_nm = 0.;
    p.missile_range_nm = 0.;
    p.sub_range_nm = 4.;
    db.sensors.insert(p.id.clone(), p);
    let mut sender = db.platforms["us_burke_2a"].clone();
    sender.id = "cas_radio_lab_sender".into();
    sender.name = "scenario_assumption: original hull/stock laboratory sender".into();
    sender.sensors = vec![P.into()];
    db.platforms.insert(sender.id.clone(), sender);
    let mut rx = db.platforms["us_burke_2a"].clone();
    rx.id = "cas_radio_lab_receiver".into();
    rx.name = "scenario_assumption: original hull/stock laboratory receiver".into();
    rx.sensors = Vec::new();
    db.platforms.insert(rx.id.clone(), rx);
    let mut target = db.platforms["cn_093"].clone();
    target.id = TARGET.into();
    target.sensors.clear();
    db.platforms.insert(target.id.clone(), target);
    db.validate().unwrap();
    db
}
fn environment() -> Value {
    json!({"source":"scenario_assumption: explicit open-water experiment","sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null})
}
fn passive_sonar() -> Value {
    json!({"type":"sonar","source":"scenario_assumption: Gen5 Bow LMF experimental sonar","generation":5,"passive_range_nm":4.,"active_range_nm":null,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false})
}
fn base_fit(peers: Vec<&str>, transport: Value) -> Value {
    json!({"source":"scenario_assumption: declared laboratory link; no real hardware or Annex claim",
        "protocol":PROTOCOL,"send":true,"receive":true,"operational":true,
        "peers":peers,"warning_kind":"near_real_time","automatic_warning":false,
        "targeting_system":false,"transport":transport})
}
fn height_qual(m: f64) -> Value {
    json!({"source":"scenario_assumption: declared feed height for the printed radio horizon; no real mast data","antenna_height_m":m})
}
fn formation_qual(name: &str) -> Value {
    json!({"source":"scenario_assumption: declared formation roster membership; not side or peer membership","formation":name})
}
fn fit_with(base: Value, qualification: &Option<Value>) -> Value {
    let mut f = base;
    if let Some(q) = qualification {
        f["radio_qualification"] = q.clone();
    }
    f
}
/// One declared room per printed group: no unprinted surviving-room redundancy.
fn three_rooms() -> Value {
    json!({"source":"scenario_assumption: one declared room per printed row-94 group; no unprinted redundancy","units":{"SENDER":{"source":"scenario_assumption: author radio rooms","components":{
        "long_radio":{"source":"scenario_assumption: declared long-range radio room","kind":"communications","cas_group":"long_range"},
        "short_radio":{"source":"scenario_assumption: declared short-range radio room","kind":"communications","cas_group":"short_range"},
        "air_radio":{"source":"scenario_assumption: declared aircraft radio room","kind":"communications","cas_group":"aircraft"}}}}})
}
fn ship_pair_config(transport: Value, tx_q: &Option<Value>, rx_q: &Option<Value>) -> Value {
    json!({"source":"scenario_assumption: explicit row-94 qualification laboratory","units":{
        "SENDER":{"source":"scenario_assumption: original-body laboratory sender","fits":{"lab":fit_with(base_fit(vec!["RECEIVER"],transport.clone()),tx_q)}},
        "RECEIVER":{"source":"scenario_assumption: original-body laboratory receiver","fits":{"lab":fit_with(base_fit(vec![],transport),rx_q)}}}})
}
fn ship_helo_config(transport: Value, ship_q: &Option<Value>, helo_q: &Option<Value>) -> Value {
    json!({"source":"scenario_assumption: explicit row-94 qualification laboratory","units":{
        "SENDER":{"source":"scenario_assumption: original-body laboratory sender","fits":{"lab":fit_with(base_fit(vec!["HELO"],transport.clone()),ship_q)}},
        "HELO":{"source":"scenario_assumption: real au_mh60r body, declared link only","fits":{"lab":fit_with(base_fit(vec!["SENDER"],transport),helo_q)}}}})
}
fn world_units(helo: bool, sep_nm: f64) -> Vec<Value> {
    if helo {
        // Real public au_mh60r helicopter body; only position/altitude/speed
        // are the author's scenario geometry. It is a link endpoint, never a
        // report source.
        vec![
            json!({"side":"Blue","class":"cas_radio_lab_sender","name":"SENDER","pos":[0,0],"course_deg":90,"speed_kts":0,"fire_on":false}),
            json!({"side":"Blue","class":"au_mh60r","name":"HELO","pos":[0,-sep_nm],"course_deg":90,"speed_kts":20,"altitude_m":HELO_ALT_M,"fire_on":false}),
            json!({"side":"Red","class":TARGET,"name":"TARGET","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}),
        ]
    } else {
        vec![
            json!({"side":"Blue","class":"cas_radio_lab_sender","name":"SENDER","pos":[0,0],"course_deg":90,"speed_kts":0,"fire_on":false}),
            json!({"side":"Blue","class":"cas_radio_lab_receiver","name":"RECEIVER","pos":[0,-sep_nm],"course_deg":90,"speed_kts":0,"fire_on":false}),
            json!({"side":"Red","class":TARGET,"name":"TARGET","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}),
        ]
    }
}
fn build_world(
    seed: u64,
    communications: Value,
    expanded: Value,
    helo: bool,
    sep_nm: f64,
) -> Result<World, String> {
    let bindings = [(format!("cas_radio_lab_sender:{P}"), passive_sonar())]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"cas-radio-qualification-20261029",
        "name":"scenario_assumption: printed-row-94 radio qualification laboratory",
        "duration_s":10000,"seed":seed,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":2,"precipitation":"None"},
            "communications_2026":communications,
            "expanded_damage_2026":expanded,
            "detection_2026":{"bindings":bindings,"environment":environment(),
                "targets":{TARGET:{"source":"scenario_assumption: controlled acoustic signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
        "units":world_units(helo, sep_nm)}))
    .unwrap();
    harpoon_core::load_scenario(sc, database())
}
fn world_with(
    seed: u64,
    communications: Value,
    expanded: Value,
    helo: bool,
    sep_nm: f64,
) -> World {
    build_world(seed, communications, expanded, helo, sep_nm)
        .expect("valid qualification laboratory must load")
}
/// Seed-search until the sender owns a real native passive report and a
/// current contact about the same target. No report/contact is ever injected.
fn fixture(communications: Value, helo: bool, sep_nm: f64) -> (World, EntityId, ContactId) {
    for seed in 1..=256u64 {
        let mut w = world_with(seed, communications.clone(), three_rooms(), helo, sep_nm);
        run(&mut w, 181);
        let sender = id(&w, "SENDER");
        let target = id(&w, "TARGET");
        let Some(contact) = w.sides[0]
            .contacts
            .values()
            .find(|c| !c.stale && c.about == target)
            .map(|c| c.id)
        else {
            continue;
        };
        let Some(view) = c::own_view(&w, sender) else {
            continue;
        };
        let contact_value = serde_json::to_value(contact).unwrap();
        let has_report = view["reports"].as_array().is_some_and(|reports| {
            reports.iter().any(|r| r["sensor"] == P && r["contact"] == contact_value)
        });
        if has_report {
            return (w, sender, contact);
        }
    }
    panic!("native source/contact fixture unavailable: fixture failure, never an accepted rule red")
}
fn warning(contact: ContactId, receivers: Vec<EntityId>) -> Value {
    json!({"type":"send_warning","contact":contact,"sensor":P,"fit":"lab","receivers":receivers})
}
/// Real typed communications command through the actual queue: validate,
/// queue, same-tick Detection terminalization.
fn send(w: &mut World, actor: EntityId, action: Value) {
    let typed: c::Action = serde_json::from_value(action.clone()).unwrap();
    c::validate_action(w, Side::Blue, actor, &typed)
        .expect("route must accept this link before queued execution");
    let cmd: Command =
        serde_json::from_value(json!({"cmd":"communications","unit":actor,"action":action}))
            .unwrap();
    harpoon_core::sim::tick(w, &[(Side::Blue, cmd)]);
}
fn messages(w: &World, unit: EntityId) -> Vec<Value> {
    let side = w.entity(unit).unwrap().side;
    let v = serde_json::to_value(harpoon_core::observe(w, side, 0, 10000)).unwrap();
    v["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == json!(unit))
        .expect("own unit in its legitimate observation")["communications"]["messages"]
        .as_array()
        .expect("messages array")
        .clone()
}
fn delivered_count(w: &World, unit: EntityId) -> usize {
    messages(w, unit)
        .iter()
        .filter(|m| m["state"] == json!("delivered"))
        .count()
}
/// Adjudicated-input boundary (batch-52 precedent): directly disables the one
/// declared room of a printed group. Never a natural weapon hit and never a
/// D6 roll; the printed producer (`resolve_extra` "communications") owns the
/// natural path and is not re-implemented here.
fn disable_room(w: &mut World, unit: EntityId, room: &str) {
    w.rules
        .expanded_damage
        .as_mut()
        .unwrap()
        .units
        .entry(unit)
        .or_default()
        .disabled
        .insert(room.into());
}
fn expect_reject(w: &World, actor: EntityId, action: Value, expect: &[&str]) -> String {
    let typed: c::Action = serde_json::from_value(action).unwrap();
    let before = serde_json::to_vec(w).unwrap();
    let err = c::validate_action(w, Side::Blue, actor, &typed).unwrap_err();
    for fragment in expect {
        assert!(
            err.contains(fragment),
            "expected rejection mentioning {fragment:?}, got: {err}"
        );
    }
    assert_eq!(
        before,
        serde_json::to_vec(w).unwrap(),
        "read-only admission cannot mutate a disabled room"
    );
    err
}

#[test]
fn row94_long_group_declared_heights_block_only_beyond_horizon_and_repair_restores() {
    let transport = json!({"type":HF,"max_range_nm":100.0});
    let q = Some(height_qual(SHIP_HEIGHT_QUAL));
    let (mut w, sender, contact) =
        fixture(ship_pair_config(transport, &q, &q), false, 30.0);
    let receiver = id(&w, "RECEIVER");
    // Undamaged control: the qualified HF link delivers across 30 nm.
    send(&mut w, sender, warning(contact, vec![receiver]));
    assert_eq!(
        delivered_count(&w, receiver),
        1,
        "qualified heights resolve the horizon: 30 nm is a normal HF link"
    );
    // Boundary: disable the one declared long-range room.
    disable_room(&mut w, sender, "long_radio");
    expect_reject(
        &w,
        sender,
        warning(contact, vec![receiver]),
        &["long_range", "radio group unavailable"],
    );
    let queued = messages(&w, sender).len();
    let cmd: Command = serde_json::from_value(json!({
        "cmd":"communications","unit":sender,
        "action":warning(contact, vec![receiver])}))
    .unwrap();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
    assert_eq!(
        messages(&w, sender).len(),
        queued,
        "the real command path rejects the beyond-horizon send with no message"
    );
    // Repair through the public entry restores the printed group.
    restore_component(&mut w, sender, "long_radio").unwrap();
    send(&mut w, sender, warning(contact, vec![receiver]));
    assert_eq!(
        delivered_count(&w, receiver),
        2,
        "repair restores the beyond-horizon group"
    );
}

#[test]
fn row94_long_group_within_horizon_is_not_applicable() {
    let transport = json!({"type":HF,"max_range_nm":100.0});
    let q = Some(height_qual(SHIP_HEIGHT_QUAL));
    let (mut w, sender, contact) =
        fixture(ship_pair_config(transport, &q, &q), false, 3.0);
    let receiver = id(&w, "RECEIVER");
    disable_room(&mut w, sender, "long_radio");
    // 3 nm is inside the declared 6.30 nm horizon: the long-range scope
    // ("units beyond the horizon") does not reach this link.
    send(&mut w, sender, warning(contact, vec![receiver]));
    assert_eq!(
        delivered_count(&w, receiver),
        1,
        "a damaged long-range room cannot block within-horizon traffic"
    );
}

#[test]
fn hf_antenna_height_is_never_assumed_default_is_pending() {
    let transport = json!({"type":HF,"max_range_nm":100.0});
    let none: Option<Value> = None;
    let (mut w, sender, contact) =
        fixture(ship_pair_config(transport, &none, &none), false, 30.0);
    let receiver = id(&w, "RECEIVER");
    // Defaults never block a healthy link.
    send(&mut w, sender, warning(contact, vec![receiver]));
    assert_eq!(delivered_count(&w, receiver), 1);
    disable_room(&mut w, sender, "long_radio");
    let err = expect_reject(
        &w,
        sender,
        warning(contact, vec![receiver]),
        &["long_range", "source_pending"],
    );
    assert!(
        err.contains("antenna_height_m"),
        "the pending reason names the missing declared input: {err}"
    );
    assert!(
        !err.contains("unavailable"),
        "unqualified horizon must stay pending, not pretend a printed block: {err}"
    );
}

#[test]
fn row94_short_group_blocks_only_declared_same_formation() {
    let transport = json!({"type":AUTHOR});
    let q = Some(formation_qual("sag-northern"));
    // Same side, mutual peer edges and 2 nm apart: none of that is the
    // printed scope — only the declared shared formation is.
    let (mut w, sender, contact) =
        fixture(ship_pair_config(transport, &q, &q), false, 2.0);
    let receiver = id(&w, "RECEIVER");
    send(&mut w, sender, warning(contact, vec![receiver]));
    assert_eq!(delivered_count(&w, receiver), 1);
    disable_room(&mut w, sender, "short_radio");
    expect_reject(
        &w,
        sender,
        warning(contact, vec![receiver]),
        &["short_range", "radio group unavailable"],
    );
    restore_component(&mut w, sender, "short_radio").unwrap();
    send(&mut w, sender, warning(contact, vec![receiver]));
    assert_eq!(delivered_count(&w, receiver), 2, "repair restores the group");
}

#[test]
fn row94_short_group_different_declared_formations_not_applicable() {
    let transport = json!({"type":AUTHOR});
    let tx_q = Some(formation_qual("sag-northern"));
    let rx_q = Some(formation_qual("sag-southern"));
    let (mut w, sender, contact) =
        fixture(ship_pair_config(transport, &tx_q, &rx_q), false, 2.0);
    let receiver = id(&w, "RECEIVER");
    disable_room(&mut w, sender, "short_radio");
    send(&mut w, sender, warning(contact, vec![receiver]));
    assert_eq!(
        delivered_count(&w, receiver),
        1,
        "declared different formations are outside the same-formation scope"
    );
}

#[test]
fn side_and_peers_never_imply_formation_default_is_pending() {
    let transport = json!({"type":AUTHOR});
    let none: Option<Value> = None;
    let (mut w, sender, contact) =
        fixture(ship_pair_config(transport, &none, &none), false, 2.0);
    let receiver = id(&w, "RECEIVER");
    send(&mut w, sender, warning(contact, vec![receiver]));
    assert_eq!(delivered_count(&w, receiver), 1);
    disable_room(&mut w, sender, "short_radio");
    let err = expect_reject(
        &w,
        sender,
        warning(contact, vec![receiver]),
        &["short_range", "source_pending"],
    );
    assert!(
        err.contains("formation"),
        "the pending reason names the missing declared formation: {err}"
    );
}

#[test]
fn row94_aircraft_group_blocks_ship_helo_link() {
    let transport = json!({"type":AUTHOR});
    let none: Option<Value> = None;
    let (mut w, sender, contact) =
        fixture(ship_helo_config(transport, &none, &none), true, 20.0);
    let helo = id(&w, "HELO");
    send(&mut w, sender, warning(contact, vec![helo]));
    assert_eq!(delivered_count(&w, helo), 1, "healthy ship-to-helo link");
    // The gate loop checks both endpoints; the printed row is a ship table,
    // so the ship's aircraft-band room is the room that can be disabled.
    disable_room(&mut w, sender, "air_radio");
    expect_reject(
        &w,
        sender,
        warning(contact, vec![helo]),
        &["aircraft", "radio group unavailable"],
    );
    restore_component(&mut w, sender, "air_radio").unwrap();
    send(&mut w, sender, warning(contact, vec![helo]));
    assert_eq!(delivered_count(&w, helo), 2, "repair restores the ship-to-air link");
}

#[test]
fn short_scope_never_reaches_helo_but_long_scope_covers_aircraft_beyond_horizon() {
    let transport = json!({"type":AUTHOR});
    let ship_q = Some(height_qual(SHIP_HEIGHT_QUAL));
    let helo_q = Some(height_qual(0.0));
    // Beyond the declared 41.7 nm horizon: long applies, short does not.
    let (mut w, sender, contact) =
        fixture(ship_helo_config(transport.clone(), &ship_q, &helo_q), true, 50.0);
    let helo = id(&w, "HELO");
    disable_room(&mut w, sender, "short_radio");
    send(&mut w, sender, warning(contact, vec![helo]));
    assert_eq!(
        delivered_count(&w, helo),
        1,
        "the same-formation ships scope cannot block a ship-aircraft link"
    );
    restore_component(&mut w, sender, "short_radio").unwrap();
    disable_room(&mut w, sender, "long_radio");
    expect_reject(
        &w,
        sender,
        warning(contact, vec![helo]),
        &["long_range", "radio group unavailable"],
    );
    restore_component(&mut w, sender, "long_radio").unwrap();
    // Within the horizon the damaged long-range room is not applicable.
    let (mut w2, sender2, contact2) =
        fixture(ship_helo_config(transport, &ship_q, &helo_q), true, 20.0);
    let helo2 = id(&w2, "HELO");
    disable_room(&mut w2, sender2, "long_radio");
    send(&mut w2, sender2, warning(contact2, vec![helo2]));
    assert_eq!(
        delivered_count(&w2, helo2),
        1,
        "a low helo inside the declared horizon keeps its long-range link"
    );
}

#[test]
fn world_restore_keeps_qualification_group_state_and_history() {
    let transport = json!({"type":HF,"max_range_nm":100.0});
    let q = Some(height_qual(SHIP_HEIGHT_QUAL));
    let (mut w, sender, contact) =
        fixture(ship_pair_config(transport, &q, &q), false, 30.0);
    let receiver = id(&w, "RECEIVER");
    send(&mut w, sender, warning(contact, vec![receiver]));
    disable_room(&mut w, sender, "long_radio");
    let raw = serde_json::to_value(&w).unwrap();
    let restored: World = serde_json::from_value(raw).unwrap();
    let action = warning(contact, vec![receiver]);
    let live_err = expect_reject(
        &w,
        sender,
        action.clone(),
        &["long_range", "radio group unavailable"],
    );
    let restored_typed: c::Action = serde_json::from_value(action).unwrap();
    let restored_err =
        c::validate_action(&restored, Side::Blue, sender, &restored_typed).unwrap_err();
    assert_eq!(
        live_err, restored_err,
        "restore must keep the declared qualification: blocked, not pending"
    );
    assert_eq!(
        serde_json::to_value(harpoon_core::observe(&restored, Side::Blue, 0, 10000)).unwrap(),
        serde_json::to_value(harpoon_core::observe(&w, Side::Blue, 0, 10000)).unwrap(),
        "restore preserves the delivered history and fit projection"
    );
}

#[test]
fn invalid_qualification_is_a_configuration_error() {
    let transport = json!({"type":HF,"max_range_nm":100.0});
    let broken: Vec<Value> = vec![
        // Source is not an explicit scenario assumption.
        json!({"source":"lab: invented","antenna_height_m":2.0}),
        // Negative feed height.
        json!({"source":"scenario_assumption: declared","antenna_height_m":-1.0}),
        // Empty formation identity.
        json!({"source":"scenario_assumption: declared","formation":"  "}),
        // A qualification without any declared input qualifies nothing.
        json!({"source":"scenario_assumption: declared"}),
    ];
    for q in broken {
        let config = ship_pair_config(transport.clone(), &Some(q.clone()), &Some(q.clone()));
        let error = build_world(7, config, three_rooms(), false, 3.0).unwrap_err();
        assert!(
            error.contains("qualification"),
            "invalid radio qualification {q} must fail configuration naming the cause: {error}"
        );
    }
    // A qualification with only the formation input is valid and loads.
    let ok = Some(formation_qual("sag-northern"));
    let _ = world_with(7, ship_pair_config(transport, &ok, &ok), three_rooms(), false, 3.0);
}

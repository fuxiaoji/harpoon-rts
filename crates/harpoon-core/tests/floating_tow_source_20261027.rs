//! Artifact-only §12.4 FloatingAsm tow source candidates.
//! Real finite Deploy, normal ship orders and native EW motion; no private position,
//! clock, HP, die or contact injection. Cable geometry/owner-loss are labelled lab
//! assumptions, not Annex fits or a source cable-dynamics model.
use harpoon_core::rules::ew2026 as ew;
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, Vec2, World};
use serde_json::{json, Value};

// At tick180: real ship moved 10*180/3600 nm; a raft deployed at tick1 becomes
// effective at61, then drifts north at5kt for119s. This AUTHOR length places its
// pickup point at that existing physical position; attachment must not teleport.
const LENGTH: f64 = 10.0 * 180.0 / 3600.0 - 5.0 * 119.0 / 3600.0;
const TOLERANCE: f64 = 1e-8; // author numerical matching tolerance, not pickup range

fn fixture() -> World {
    let input = json!({
        "id":"floating-tow-source", "name":"author rigid-offset radar raft laboratory; not real Annex fit or cable physics", "seed":20261027,
        "duration_s":12000, "weather":{"visibility_nm":0.0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4", "radar_environment":{"sea_state":0,"precipitation":"None"},
            "ew_2026":{
                "fits":{"lab_float":{"source":"scenario_assumption: explicitly authored radar raft and one expendable; not a Burke Annex fit","kind":"floating_asm","generation":null,"launcher":null,"endurance_s":null,"deployment_s":null,"radar_height_m":3.0}},
                "loadouts":{"us_burke_2a":{"lab_float":1}},
                "environment":{"source":"scenario_assumption: calm open laboratory; wind from south at10kt","sea_state":0,"wind_from_deg":180.0,"wind_speed_kts":10.0},
                "floating_tow_geometries":{"stern_lab":{"source":"scenario_assumption: rigid stern offset and discrete one-second release; no Annex cable length/dynamics claim","fit":"lab_float","model":"rigid_stern_offset","length_nm":LENGTH,"capture_tolerance_nm":TOLERANCE,"owner_loss":"detach_and_drift"}}
            }},
        "units":[
            {"side":"Blue","class":"us_burke_2a","name":"TUG","pos":[0.0,0.0],"course_deg":0.0,"speed_kts":10.0,"fire_on":false},
            {"side":"Blue","class":"us_virginia","name":"CONTROL","pos":[40.0,0.0],"course_deg":0.0,"speed_kts":0.0,"depth_m":50.0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"OTHER","pos":[80.0,0.0],"course_deg":0.0,"speed_kts":0.0,"depth_m":50.0,"fire_on":false}
        ]
    });
    let sc: Scenario = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&sc).unwrap()["context"]["ew_2026"]["floating_tow_geometries"],
        input["context"]["ew_2026"]["floating_tow_geometries"],
        "source-red checkpoint: no silently discarded explicit cable geometry/policy"
    );
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    for class in ["us_burke_2a", "us_virginia", "cn_093"] {
        db.platforms.get_mut(class).unwrap().sensors.clear();
    }
    harpoon_core::load_scenario(sc, db).unwrap()
}
fn named(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn ew_cmd(unit: EntityId, action: Value) -> Command {
    serde_json::from_value(json!({"cmd":"electronic_warfare","unit":unit,"action":action}))
        .expect("source-red checkpoint: prospective floating tow/untow must be normal JSON actions, not future Rust enum references")
}
fn tow_cmd(unit: EntityId, id: EntityId) -> Command {
    ew_cmd(
        unit,
        json!({"type":"floating_tow","object":id,"geometry":"stern_lab"}),
    )
}
fn speed_cmd(unit: EntityId, speed: f64) -> Command {
    serde_json::from_value(json!({"cmd":"set_speed","unit":unit,"speed_kts":speed})).unwrap()
}
fn course_cmd(unit: EntityId, course: f64) -> Command {
    serde_json::from_value(json!({"cmd":"set_course","unit":unit,"course_deg":course})).unwrap()
}
fn obj(w: &World, id: EntityId) -> &ew::Object {
    ew::object(w, id).unwrap()
}
fn tow(w: &World, id: EntityId) -> Value {
    serde_json::to_value(obj(w, id)).unwrap()["floating_tow"].clone()
}
fn stock(w: &World, unit: EntityId) -> Value {
    ew::own_view(w, Side::Blue, unit).unwrap()["stocks"]["lab_float"].clone()
}
fn close(actual: Vec2, expected: Vec2, detail: &str) {
    assert!(
        actual.range_to(expected) < TOLERANCE,
        "{detail}: actual={actual:?}, expected={expected:?}"
    );
}
fn stern(w: &World, unit: EntityId) -> Vec2 {
    let owner = w.entity(unit).unwrap();
    owner.pos.advanced(owner.course_deg + 180.0, LENGTH)
}
fn receipt(w: &World, unit: EntityId, ok: bool) {
    assert!(w.events.iter().any(|e|e.tick==w.tick && matches!(&e.kind, EventKind::RulesEvent {domain,rule,unit:Some(id),detail,..} if domain=="command" && *id==unit && rule==if ok{"executed"}else{"rejected"} && detail.starts_with("electronic_warfare:"))), "real EW command receipt");
}
fn event_count(w: &World, rule: &str) -> usize {
    w.events.iter().filter(|e| matches!(&e.kind, EventKind::RulesEvent {domain,rule:r,..} if domain=="ew2026" && r==rule)).count()
}
fn restore(w: &World) -> World {
    serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap()
}
fn pair(w: &mut World, restored: &mut World, commands: &[(Side, Command)]) {
    harpoon_core::sim::tick(w, commands);
    harpoon_core::sim::tick(restored, commands);
    assert_eq!(
        serde_json::to_value(&*w).unwrap(),
        serde_json::to_value(&*restored).unwrap(),
        "native World resume retains cable/lifetime/position/stock/events"
    );
}
fn deployed_aligned() -> (World, EntityId, EntityId) {
    let mut w = fixture();
    let owner = named(&w, "TUG");
    assert_eq!(stock(&w, owner), 1);
    let cmd = ew_cmd(
        owner,
        json!({"type":"deploy","fit":"lab_float","at":null,"radar_signature":2,"course_deg":null,"speed_kts":null,"depth_m":null}),
    );
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
    receipt(&w, owner, true);
    assert_eq!(stock(&w, owner), 0, "actual finite deployment debit");
    let id = *w.rules.ew.as_ref().unwrap().objects.keys().next().unwrap();
    assert_eq!(obj(&w, id).kind, ew::ObjectKind::FloatingAsm);
    assert_eq!(obj(&w, id).ready_at, 61);
    assert_eq!(obj(&w, id).expires_at, Some(61 + 10800));
    for _ in 1..180 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(w.tick, 180, "native ticks, no injected clocks");
    close(
        obj(&w, id).pos,
        stern(&w, owner),
        "free raft and real ship physically meet author stern offset before attaching",
    );
    assert_eq!(ew::radar_signature(&w, id), Some(2));
    assert!(
        ew::active_acms(&w, owner).is_empty(),
        "radar raft cannot become an acoustic towed countermeasure"
    );
    assert!(
        !w.entities.contains_key(&id),
        "false echo remains separate from combat platform entities"
    );
    (w, owner, id)
}
fn attach(w: &mut World, owner: EntityId, id: EntityId) {
    harpoon_core::sim::tick(w, &[(Side::Blue, tow_cmd(owner, id))]);
    receipt(w, owner, true);
    assert_eq!(tow(w, id)["status"], "attached");
    assert_eq!(tow(w, id)["geometry"], "stern_lab");
    close(
        obj(w, id).pos,
        stern(w, owner),
        "attached raft uses actual post-movement ship pose",
    );
    assert_eq!(
        stock(w, owner),
        0,
        "attaching does not deploy a replacement or refund stock"
    );
    let own = ew::own_view(w, Side::Blue, owner).unwrap();
    assert_eq!(
        own["objects"][0]["floating_tow"],
        tow(w, id),
        "own whitelist exposes own tow state"
    );
    assert!(ew::own_view(w, Side::Red, owner).is_none());
}

#[test]
fn ft01_actual_ship_at_ten_follows_then_overspeed_breaks_once_and_free_drifts_after_restore() {
    let (mut w, owner, id) = deployed_aligned();
    let expiry = obj(&w, id).expires_at;
    attach(&mut w, owner, id);
    let attached_at = tow(&w, id)["attached_at"].clone();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, tow_cmd(owner, id))]);
    receipt(&w, owner, true);
    assert_eq!(
        tow(&w, id)["attached_at"],
        attached_at,
        "same attachment is idempotent"
    );
    for command in [
        speed_cmd(owner, 9.0),
        speed_cmd(owner, 10.0),
        course_cmd(owner, 90.0),
    ] {
        harpoon_core::sim::tick(&mut w, &[(Side::Blue, command)]);
        assert!(w.entity(owner).unwrap().speed_kts <= 10.0);
        assert_eq!(tow(&w, id)["status"], "attached");
        close(
            obj(&w, id).pos,
            stern(&w, owner),
            "actual heading/speed, no additional wind step while attached",
        );
    }
    for _ in 0..30 {
        if (w.entity(owner).unwrap().speed_kts - 10.0).abs() <= 1e-9 {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!((w.entity(owner).unwrap().speed_kts - 10.0).abs() <= 1e-9);
    assert_eq!(tow(&w, id)["status"], "attached", "exactly10kt is legal");
    let mut resumed = restore(&w);
    let before = obj(&w, id).pos;
    pair(
        &mut w,
        &mut resumed,
        &[(Side::Blue, speed_cmd(owner, 12.0))],
    );
    assert_eq!(
        w.entity(owner).unwrap().ordered_speed_kts,
        12.0,
        "source cable break must not reject or clamp the ordinary ship order"
    );
    assert!(
        w.entity(owner).unwrap().speed_kts > 10.0,
        "actual speed crosses source ceiling"
    );
    assert_eq!(tow(&w, id)["status"], "broken");
    assert_eq!(tow(&w, id)["reason"], "overspeed");
    assert_eq!(
        obj(&w, id).status,
        ew::Status::Deployed,
        "broken cable does not sink/destroy the raft"
    );
    close(obj(&w, id).pos, before.advanced(0.0, 5.0 / 3600.0), "explicit author one-second break policy: last raft point then one free-drift interval, no overspeed follow");
    assert_eq!(event_count(&w, "12.4.tow_broken"), 1);
    let ended_at = tow(&w, id)["ended_at"].clone();
    for _ in 0..5 {
        let before = obj(&w, id).pos;
        pair(&mut w, &mut resumed, &[]);
        close(
            obj(&w, id).pos,
            before.advanced(0.0, 5.0 / 3600.0),
            "broken raft resumes source half-wind drift exactly once",
        );
    }
    assert_eq!(tow(&w, id)["ended_at"], ended_at);
    assert_eq!(
        event_count(&w, "12.4.tow_broken"),
        1,
        "no repeated break on later ticks/save continuation"
    );
    assert_eq!(obj(&w, id).expires_at, expiry);
    assert_eq!(ew::radar_signature(&w, id), Some(2));
    assert_eq!(stock(&w, owner), 0);
}

#[test]
fn ft02_explicit_author_release_and_common_owner_loss_detach_without_acoustic_destruction() {
    for owner_lost in [false, true] {
        let (mut w, owner, id) = deployed_aligned();
        attach(&mut w, owner, id);
        let expiry = obj(&w, id).expires_at;
        let before = obj(&w, id).pos;
        let mut resumed = restore(&w);
        if owner_lost {
            // Explicit already-adjudicated common vessel loss. This is not a
            // claimed weapon-attack test, and no private HP/status is injected.
            harpoon_core::damage::sink(&mut w, owner);
            harpoon_core::damage::sink(&mut resumed, owner);
            assert!(!w.entity(owner).unwrap().alive);
            pair(&mut w, &mut resumed, &[]);
        } else {
            let command = ew_cmd(owner, json!({"type":"floating_untow","object":id}));
            pair(&mut w, &mut resumed, &[(Side::Blue, command)]);
            receipt(&w, owner, true);
        }
        assert_eq!(tow(&w, id)["status"], "released");
        assert_eq!(
            tow(&w, id)["reason"],
            if owner_lost { "owner_lost" } else { "manual" }
        );
        assert_eq!(obj(&w, id).status, ew::Status::Deployed);
        assert_eq!(obj(&w, id).expires_at, expiry);
        close(
            obj(&w, id).pos,
            before.advanced(0.0, 5.0 / 3600.0),
            "explicit author detach policy preserves source free-drift raft",
        );
        assert_eq!(ew::radar_signature(&w, id), Some(2));
        assert!(ew::active_acms(&w, owner).is_empty());
        assert!(!w.entities.contains_key(&id));
        for _ in 0..3 {
            let previous = obj(&w, id).pos;
            pair(&mut w, &mut resumed, &[]);
            close(
                obj(&w, id).pos,
                previous.advanced(0.0, 5.0 / 3600.0),
                "released raft drifts after native restore",
            );
        }
        assert_eq!(w.rules.ew.as_ref().unwrap().stocks[&owner]["lab_float"], 0);
        if !owner_lost {
            // Source explicitly permits early sinking of this own radar raft.
            let command = ew_cmd(owner, json!({"type":"sink","object":id}));
            pair(&mut w, &mut resumed, &[(Side::Blue, command)]);
            receipt(&w, owner, true);
            assert_eq!(obj(&w, id).status, ew::Status::Sunk);
            assert_eq!(ew::radar_signature(&w, id), None);
            assert_eq!(stock(&w, owner), 0);
        }
    }
}

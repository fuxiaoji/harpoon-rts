//! Artifact-only §12.6.2 actual Deploy -> owned EW MobileSpeed candidates.
//! No injected count/Contact/lock/die; no future Rust Action/State field reference.
use harpoon_core::rules::ew2026 as ew;
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};

fn fixture(generation: u8) -> World {
    let fit = format!("lab_mobile_gen{generation}");
    let input = json!({"id":"mobile-speed-source","name":"generation-matched authored simulator hardware, no real Annex fit claim","seed":20261027,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "ew_2026":{"fits":{(fit.clone()):{"source":"scenario_assumption: explicit internal mobile simulator generation and finite launcher stock; not a real Virginia Annex fit","kind":"mobile_decoy","generation":generation,"launcher":"internal","endurance_s":null,"deployment_s":null,"radar_height_m":null}},"loadouts":{"us_virginia":{(fit):1}},"environment":{"source":"scenario_assumption: open calm laboratory water","sea_state":0,"wind_from_deg":0.0,"wind_speed_kts":0.0}},
            "detection_2026":{"bindings":{},"targets":{"us_virginia":{"source":"scenario_assumption: explicitly declared submarine signature for actual simulator inheritance","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}},"environment":{"source":"scenario_assumption: deep water, no hidden contact input","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
        "units":[{"side":"Blue","class":"us_virginia","name":"OWNER","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},{"side":"Red","class":"cn_093","name":"OTHER","pos":[20,0],"speed_kts":0,"depth_m":50,"fire_on":false}]});
    let sc: Scenario = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&sc).unwrap()["context"]["ew_2026"],
        input["context"]["ew_2026"]
    );
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    for class in ["us_virginia", "cn_093"] {
        db.platforms.get_mut(class).unwrap().sensors.clear();
    }
    harpoon_core::load_scenario(sc, db).unwrap()
}
fn actor(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn maximum(g: u8) -> f64 {
    [7., 10., 12., 15.][g as usize - 1]
}
fn action(owner: EntityId, kind: &str, object: EntityId, value: f64) -> Command {
    let a = if kind == "mobile_speed" {
        json!({"type":kind,"object":object,"speed_kts":value})
    } else {
        json!({"type":kind,"object":object,"course_deg":value})
    };
    serde_json::from_value(json!({"cmd":"electronic_warfare","unit":owner,"action":a})).expect("source-red checkpoint: prospective mobile_speed must be a real normal Action, not a missing Rust enum compile error")
}
fn stock(w: &World, owner: EntityId, g: u8) -> Value {
    ew::own_view(w, Side::Blue, owner).unwrap()["stocks"][format!("lab_mobile_gen{g}")].clone()
}
fn deploy(w: &mut World, g: u8) -> EntityId {
    let owner = actor(w, "OWNER");
    let cmd:Command=serde_json::from_value(json!({"cmd":"electronic_warfare","unit":owner,"action":{"type":"deploy","fit":format!("lab_mobile_gen{g}"),"at":null,"radar_signature":null,"course_deg":0,"speed_kts":maximum(g),"depth_m":50}})).unwrap();
    harpoon_core::sim::tick(w, &[(Side::Blue, cmd)]);
    receipt(w, owner, true);
    assert_eq!(stock(w, owner, g), 0, "real finite launch stock debit");
    let obj = w
        .rules
        .ew
        .as_ref()
        .unwrap()
        .objects
        .values()
        .next()
        .expect("actual deployed simulator");
    assert_eq!(obj.kind, ew::ObjectKind::MobileDecoy);
    assert_eq!(obj.generation, Some(g));
    assert_eq!(obj.speed_kts, maximum(g));
    assert_eq!(obj.depth_m, 50.);
    assert_eq!(obj.status, ew::Status::Deployed);
    assert!(
        !w.entities.contains_key(&obj.id),
        "keep source EW object separate from combat platform entities"
    );
    obj.id
}
fn receipt(w: &World, owner: EntityId, executed: bool) {
    assert!(w.events.iter().any(|e|e.tick==w.tick&&matches!(&e.kind,EventKind::RulesEvent{domain,rule,unit:Some(id),detail,..}if domain=="command"&&*id==owner&&rule==if executed{"executed"}else{"rejected"}&&detail.starts_with("electronic_warfare:"))),"real command execution/rejection receipt");
}
fn budgets(w: &World, id: EntityId, course: u64, speed: u64) {
    let value = serde_json::to_value(&w.rules.ew.as_ref().unwrap().objects[&id]).unwrap();
    assert_eq!(value["course_changes"].as_u64(), Some(course));
    assert_eq!(
        value["speed_changes"].as_u64(),
        Some(speed),
        "real persisted speed allowance, independent of the existing course counter"
    );
}
fn stable(w: &World, id: EntityId) -> Value {
    let obj = &w.rules.ew.as_ref().unwrap().objects[&id];
    let data = serde_json::to_value(obj).unwrap();
    json!({"generation":obj.generation,"status":obj.status,"course":obj.course_deg,"speed":obj.speed_kts,"expires":obj.expires_at,"course_changes":data["course_changes"],"speed_changes":data["speed_changes"]})
}
fn tick_pair(w: &mut World, r: &mut World, cmd: Command) {
    let commands = [(Side::Blue, cmd)];
    harpoon_core::sim::tick(w, &commands);
    harpoon_core::sim::tick(r, &commands);
    assert_eq!(
        serde_json::to_value(&*w).unwrap(),
        serde_json::to_value(&*r).unwrap()
    );
}
fn actual_step(
    w: &mut World,
    owner: EntityId,
    id: EntityId,
    kind: &str,
    value: f64,
    success: bool,
) {
    let before = w.rules.ew.as_ref().unwrap().objects[&id].pos;
    harpoon_core::sim::tick(w, &[(Side::Blue, action(owner, kind, id, value))]);
    receipt(w, owner, success);
    let obj = &w.rules.ew.as_ref().unwrap().objects[&id];
    let expected = before.advanced(obj.course_deg, obj.speed_kts / 3600.);
    assert!(
        obj.pos.range_to(expected) < 1e-9,
        "ordinary runtime movement uses actual accepted course/speed each second"
    );
}
#[test]
fn ms01_gen3_and4_mix_two_course_and_two_speed_changes_without_same_value_or_turn_reset() {
    for g in [3, 4] {
        let mut w = fixture(g);
        let owner = actor(&w, "OWNER");
        let id = deploy(&mut w, g);
        let expiry = w.rules.ew.as_ref().unwrap().objects[&id].expires_at;
        actual_step(&mut w, owner, id, "mobile_speed", maximum(g), true);
        budgets(&w, id, 0, 0);
        actual_step(&mut w, owner, id, "mobile_course", 90., true);
        actual_step(&mut w, owner, id, "mobile_speed", maximum(g) - 2., true);
        budgets(&w, id, 1, 1);
        let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        tick_pair(
            &mut w,
            &mut restored,
            action(owner, "mobile_speed", id, maximum(g) - 2.),
        );
        budgets(&w, id, 1, 1);
        tick_pair(
            &mut w,
            &mut restored,
            action(owner, "mobile_course", id, 180.),
        );
        budgets(&w, id, 2, 1);
        tick_pair(
            &mut w,
            &mut restored,
            action(owner, "mobile_speed", id, maximum(g) - 1.),
        );
        budgets(&w, id, 2, 2);
        let before = stable(&w, id);
        tick_pair(
            &mut w,
            &mut restored,
            action(owner, "mobile_course", id, 270.),
        );
        receipt(&w, owner, false);
        assert_eq!(stable(&w, id), before);
        tick_pair(
            &mut w,
            &mut restored,
            action(owner, "mobile_speed", id, maximum(g) - 3.),
        );
        receipt(&w, owner, false);
        assert_eq!(stable(&w, id), before);
        while w.tick < 181 {
            harpoon_core::sim::tick(&mut w, &[]);
            harpoon_core::sim::tick(&mut restored, &[]);
        }
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            serde_json::to_value(&restored).unwrap()
        );
        tick_pair(
            &mut w,
            &mut restored,
            action(owner, "mobile_speed", id, maximum(g)),
        );
        receipt(&w, owner, false);
        budgets(&w, id, 2, 2);
        assert_eq!(
            stable(&w, id),
            before,
            "newTac cannot reset the individual object's two allowances"
        );
        assert_eq!(
            w.rules.ew.as_ref().unwrap().objects[&id].expires_at,
            expiry,
            "no new lifetime or guessed low-speed energy on change"
        );
        assert_eq!(stock(&w, owner, g), 0);
    }
}
#[test]
fn ms02_lower_generations_invalid_speed_foreign_owner_and_manned_sub_are_distinct() {
    for g in [1, 2] {
        let mut w = fixture(g);
        let owner = actor(&w, "OWNER");
        let id = deploy(&mut w, g);
        actual_step(&mut w, owner, id, "mobile_speed", maximum(g), true);
        budgets(&w, id, 0, 0);
        let before = stable(&w, id);
        actual_step(&mut w, owner, id, "mobile_speed", maximum(g) - 1., false);
        actual_step(&mut w, owner, id, "mobile_course", 90., false);
        assert_eq!(stable(&w, id), before);
    }
    let g = 3;
    let mut w = fixture(g);
    let owner = actor(&w, "OWNER");
    let other = actor(&w, "OTHER");
    let id = deploy(&mut w, g);
    actual_step(&mut w, owner, id, "mobile_speed", maximum(g), true);
    let before = stable(&w, id);
    actual_step(&mut w, owner, id, "mobile_speed", maximum(g) + 1., false);
    assert_eq!(stable(&w, id), before);
    harpoon_core::sim::tick(
        &mut w,
        &[(Side::Red, action(other, "mobile_speed", id, 10.))],
    );
    receipt(&w, other, false);
    assert_eq!(stable(&w, id), before);
    harpoon_core::sim::tick(
        &mut w,
        &[(Side::Red, action(owner, "mobile_speed", id, 10.))],
    );
    assert_eq!(
        stable(&w, id),
        before,
        "wrong side cannot use the real owner's identity"
    );
    for speed in [2., 4., 6., 8.] {
        let cmd: Command =
            serde_json::from_value(json!({"cmd":"set_speed","unit":owner,"speed_kts":speed}))
                .unwrap();
        harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
        assert_eq!(
            w.entity(owner).unwrap().ordered_speed_kts,
            speed,
            "manned platform has normal speed control, not a simulator's two-change hardware cap"
        );
        assert_eq!(stable(&w, id), before);
    }
    actual_step(&mut w, owner, id, "mobile_speed", 10., true);
    budgets(&w, id, 0, 1);
    assert_eq!(stock(&w, owner, g), 0);
}

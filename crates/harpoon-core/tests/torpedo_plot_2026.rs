//! Source PDF74 §6.3.7 and PDF79 §7.5.2: own prelaunch declarations.
//! Plot does not supply FCS, move an in-flight weapon, or finish R-054 search.
use harpoon_core::rules::torpedo2026 as torp;
use harpoon_core::types::PlatformKind;
use harpoon_core::{ContactId, Database, EntityId, Scenario, Side, Vec2, World};
use serde_json::{json, Value};

fn fixture() -> (World, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"torpedo-own-plot","name":"Own prelaunch declaration","duration_s":1000,"seed":20261024,
        "context":{"rules_profile":"hv2026-rts-v4","torpedo_2026":{
            "source":"scenario_assumption: controlled source mechanism fits",
            "environment":{"source":"scenario_assumption: no layer/reverberation","layer_m":null,"reverberant":false},
            "targets":{"cn_093":{"source":"scenario_assumption: controlled target metadata","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
            "platforms":{"us_virginia":{"source":"scenario_assumption: two loaded mechanism tubes","wire_capacity":2,"reload_parallel":2,
                "tubes":[{"bearing_deg":0,"loaded":"us_mk48"},{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}},
            "orders":{"SHOOTER:us_mk48":{"source":"scenario_assumption: legacy author order only","speed_index":0,"depth_m":50,
                "enable_run_nm":0,"bearing_deg":null,"cut_wire":true,"spread_deg":[],
                "solution":{"quality":"Good","source":"scenario_assumption: unrelated author FCS must not be inherited"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,0.5],"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let actor = w
        .entities
        .values()
        .find(|e| e.name == "SHOOTER")
        .unwrap()
        .id;
    let target = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    w.entity_mut(target).unwrap().weapons.clear();
    w.sides[0].contacts.insert(
        ContactId(99),
        harpoon_core::world::Contact {
            id: ContactId(99),
            about: target,
            category: PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 0.5),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: 0,
            stale: false,
            sensors: vec![],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert(format!("0:{}", target.0), PlatformKind::Sub);
    (w, actor, target)
}
fn value() -> Value {
    json!({"type":"plot","contact":99,"weapon":"us_mk48","speed_index":0,
        "depth_m":50.,"enable_run_nm":0.,"bearing_deg":0.,"spread_deg":[],"cut_wire":true})
}
fn action(v: Value) -> torp::Action {
    serde_json::from_value(v).expect("legal Plot schema")
}
fn ammo(w: &World, actor: EntityId) -> u16 {
    w.entity(actor)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_mk48")
        .unwrap()
        .ammo
}
fn mount(w: &World, actor: EntityId) -> usize {
    w.entity(actor)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_mk48")
        .unwrap()
}
fn plans(w: &World, actor: EntityId) -> Value {
    torp::own_view(w, Side::Blue, actor).unwrap()["plans"].clone()
}

#[test]
fn legal_plot_is_read_only_to_validate_and_only_stores_an_own_prelaunch_plan() {
    let (mut w, actor, _) = fixture();
    let a = action(value());
    let before = serde_json::to_value(&w).unwrap();
    let p = torp::validate_plot(&w, Side::Blue, actor, &a).unwrap();
    assert_eq!(p.actor, actor);
    assert_eq!(p.contact, ContactId(99));
    assert_eq!(p.weapon, "us_mk48");
    assert_eq!(serde_json::to_value(&w).unwrap(), before);
    let before_ammo = ammo(&w, actor);
    let before_rng = serde_json::to_value(&w.rng).unwrap();
    let before_tubes = serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().tubes).unwrap();
    torp::apply(&mut w, actor, &a).unwrap();
    assert_eq!(ammo(&w, actor), before_ammo);
    assert_eq!(serde_json::to_value(&w.rng).unwrap(), before_rng);
    assert_eq!(
        serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().tubes).unwrap(),
        before_tubes
    );
    assert!(w.rules.torpedoes.as_ref().unwrap().flights.is_empty());
    assert_eq!(plans(&w, actor).as_array().unwrap().len(), 1);
    assert_eq!(plans(&w, actor)[0]["bearing_deg"], 0.);
}

#[test]
fn plot_schema_rejects_author_fcs_truth_and_dice_injection() {
    for field in [
        "quality",
        "solution",
        "source",
        "d10",
        "target",
        "enemy_depth_m",
    ] {
        let mut bad = value();
        bad[field] = json!("injected");
        assert!(
            serde_json::from_value::<torp::Action>(bad).is_err(),
            "Plot accepted extra {field}"
        );
    }
    for field in [
        "contact",
        "weapon",
        "speed_index",
        "depth_m",
        "enable_run_nm",
        "bearing_deg",
        "spread_deg",
        "cut_wire",
    ] {
        let mut missing = value();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<torp::Action>(missing).is_err(),
            "Plot silently defaulted missing {field}"
        );
    }
    let command: harpoon_core::Command = serde_json::from_value(json!({
        "cmd":"torpedo","unit":1,"action":value()
    }))
    .unwrap();
    assert!(matches!(command, harpoon_core::Command::Torpedo { .. }));
}

#[test]
fn invalid_plot_has_no_mutation_and_validation_does_not_read_secret_target_state() {
    let (mut w, actor, target) = fixture();
    for (field, v) in [
        ("speed_index", json!(999)),
        ("depth_m", json!(-1)),
        ("enable_run_nm", json!(1e9)),
        ("bearing_deg", json!(360)),
        ("spread_deg", json!([0, 14])),
        ("contact", json!(100)),
        ("weapon", json!("us_aim120d")),
    ] {
        let mut invalid = value();
        invalid[field] = v;
        let a = action(invalid);
        let before = serde_json::to_value(&w).unwrap();
        assert!(
            torp::validate_plot(&w, Side::Blue, actor, &a).is_err(),
            "invalid {field}"
        );
        assert_eq!(serde_json::to_value(&w).unwrap(), before);
    }
    assert!(torp::validate_plot(&w, Side::Red, actor, &action(value())).is_err());
    let expected =
        serde_json::to_value(torp::validate_plot(&w, Side::Blue, actor, &action(value())).unwrap())
            .unwrap();
    let enemy = w.entity_mut(target).unwrap();
    enemy.alive = false;
    enemy.pos = Vec2::new(999., 999.);
    enemy.depth_m = 9999.;
    assert_eq!(
        serde_json::to_value(torp::validate_plot(&w, Side::Blue, actor, &action(value())).unwrap())
            .unwrap(),
        expected
    );
}

#[test]
fn same_name_units_keep_separate_entity_id_weapon_plans() {
    let (mut w, actor, _) = fixture();
    let mut second = w.entity(actor).unwrap().clone();
    second.id = w.allocate_id();
    let other = second.id;
    // scenario_assumption: a second identical mechanism actor deliberately has
    // the same display name. No real fleet composition is asserted.
    w.entities.insert(other, second);
    let fit = w.rules.torpedoes.as_ref().unwrap().tubes[&actor].clone();
    w.rules.torpedoes.as_mut().unwrap().tubes.insert(other, fit);
    torp::apply(&mut w, actor, &action(value())).unwrap();
    let mut changed = value();
    changed["bearing_deg"] = json!(30.);
    torp::apply(&mut w, other, &action(changed)).unwrap();
    assert_eq!(plans(&w, actor)[0]["bearing_deg"], 0.);
    assert_eq!(plans(&w, other)[0]["bearing_deg"], 30.);
    assert_eq!(plans(&w, actor)[0]["actor"], json!(actor));
    assert_eq!(plans(&w, other)[0]["actor"], json!(other));
}

#[test]
fn new_plot_never_borrows_author_good_fcs_or_a_different_report_of_the_same_body() {
    let (mut w, actor, _) = fixture();
    torp::apply(&mut w, actor, &action(value())).unwrap();
    let before = ammo(&w, actor);
    let mi = mount(&w, actor);
    let mut other = w.sides[0].contacts[&ContactId(99)].clone();
    other.id = ContactId(100);
    w.sides[0].contacts.insert(other.id, other);
    let error = torp::fire_at_contact(&mut w, actor, ContactId(100), mi, 1).unwrap_err();
    assert!(
        error.contains("plan") || error.contains("Plot"),
        "wrong report must hit the plan binding: {error}"
    );
    let error = torp::fire_at_contact(&mut w, actor, ContactId(99), mi, 1).unwrap_err();
    assert!(
        error.contains("6.3") || error.contains("FCS") || error.contains("solution"),
        "must require source own FCS: {error}"
    );
    assert_eq!(ammo(&w, actor), before);
    assert!(w.rules.torpedoes.as_ref().unwrap().flights.is_empty());
    assert!(
        w.rules.torpedoes.as_ref().unwrap().config.orders["SHOOTER:us_mk48"]
            .solution
            .is_some()
    );
}

#[test]
fn legacy_order_release_is_compatible_and_replot_does_not_edit_its_ledger_or_wire() {
    let (mut w, actor, _) = fixture();
    let mi = mount(&w, actor);
    assert_eq!(
        torp::fire_at_contact(&mut w, actor, ContactId(99), mi, 1).unwrap(),
        1
    );
    let state = w.rules.torpedoes.as_ref().unwrap();
    assert_eq!(state.flights.len(), 1);
    let releases = serde_json::to_value(&state.launches).unwrap();
    let flights = serde_json::to_value(&state.flights).unwrap();
    let before = ammo(&w, actor);
    let mut changed = value();
    changed["bearing_deg"] = json!(30.);
    changed["depth_m"] = json!(100.);
    changed["enable_run_nm"] = json!(0.1);
    changed["cut_wire"] = json!(false);
    torp::apply(&mut w, actor, &action(changed)).unwrap();
    assert_eq!(
        serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().launches).unwrap(),
        releases
    );
    assert_eq!(
        serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().flights).unwrap(),
        flights
    );
    assert_eq!(ammo(&w, actor), before);
}

#[test]
fn saved_prelaunch_projection_is_own_only_and_contains_no_author_solution_or_enemy_truth() {
    let (mut w, actor, _) = fixture();
    torp::apply(&mut w, actor, &action(value())).unwrap();
    let own = plans(&w, actor);
    assert!(own.is_array());
    let serialized = serde_json::to_string(&own).unwrap();
    for forbidden in ["quality", "solution", "source", "target", "d10", "enemy"] {
        assert!(
            !serialized.contains(forbidden),
            "prelaunch projection leaked {forbidden}: {serialized}"
        );
    }
    assert!(torp::own_view(&w, Side::Red, actor).is_none());
    let restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(plans(&restored, actor), own);
}

#[test]
fn own_carried_weapon_profiles_are_source_bound_and_visible_without_a_tube_inventory() {
    let (mut w, actor, _) = fixture();
    w.entity_mut(actor)
        .unwrap()
        .weapons
        .retain(|m| m.weapon == "us_mk48");
    let bound = w.rules.torpedoes.as_ref().unwrap().config.weapons["us_mk48"].clone();
    let own = torp::own_view(&w, Side::Blue, actor).unwrap();
    assert!(
        own["weapon_profiles"].is_object(),
        "own settings must not require guessing speed_index"
    );
    assert_eq!(own["weapon_profiles"].as_object().unwrap().len(), 1);
    assert_eq!(
        own["weapon_profiles"]["us_mk48"],
        json!({
            "source":bound.source,"guidance":bound.guidance,"generation":bound.generation,
            "settings":bound.settings,"max_depth_m":bound.max_depth_m,
            "surface":bound.surface,"submarine":bound.submarine
        })
    );
    assert!(
        own["weapon_profiles"].get("us_mk54").is_none(),
        "uncarried source config must not be projected"
    );
    assert!(own["weapon_profiles"].get("targets").is_none());
    assert!(own["weapon_profiles"].get("orders").is_none());
    for kind in [PlatformKind::Aircraft, PlatformKind::Ship] {
        let mut carrier = w.clone();
        let e = carrier.entity_mut(actor).unwrap();
        // scenario_assumption: only the own-profile projection mechanism is
        // exercised; this fictional carrier asserts no real aircraft/hull fit.
        e.class = "scenario_assumption_profile_carrier".into();
        e.kind = kind;
        e.weapons = vec![harpoon_core::world::WeaponMount {
            weapon: "us_mk54".into(),
            mounts: 1,
            ammo: 2,
            cooldown_s: 0.,
        }];
        carrier
            .rules
            .torpedoes
            .as_mut()
            .unwrap()
            .tubes
            .remove(&actor);
        let view = torp::own_view(&carrier, Side::Blue, actor)
            .expect("own carried source profiles survive missing tube inventory");
        assert_eq!(view["verified"], false);
        for field in ["tubes", "reserves", "reloads"] {
            assert!(view[field].is_null());
        }
        assert!(view["weapon_profiles"]["us_mk54"].is_object());
        assert!(view["plans"].is_array());
        assert!(torp::own_view(&carrier, Side::Red, actor).is_none());
    }
}

#[test]
fn unknown_carried_profile_is_not_guessed_from_generic_database_speed_or_range() {
    let (mut w, actor, _) = fixture();
    w.entity_mut(actor)
        .unwrap()
        .weapons
        .retain(|m| m.weapon == "us_mk48");
    assert!(w.db.weapons.contains_key("us_mk48"));
    w.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .config
        .weapons
        .remove("us_mk48");
    let before = serde_json::to_value(&w).unwrap();
    assert!(torp::validate_plot(&w, Side::Blue, actor, &action(value())).is_err());
    assert_eq!(serde_json::to_value(&w).unwrap(), before);
    let view = torp::own_view(&w, Side::Blue, actor).unwrap();
    assert!(view["weapon_profiles"].is_object());
    assert!(view["weapon_profiles"].as_object().unwrap().is_empty());
}

#[test]
fn actual_legacy_release_projects_settings_without_author_fcs_and_preserves_internal_history() {
    let (mut w, actor, _) = fixture();
    w.rules
        .torpedoes
        .as_mut()
        .unwrap()
        .config
        .orders
        .get_mut("SHOOTER:us_mk48")
        .unwrap()
        .source = "scenario_assumption: private author FCS narrative".into();
    let mi = mount(&w, actor);
    assert_eq!(
        torp::fire_at_contact(&mut w, actor, ContactId(99), mi, 1).unwrap(),
        1,
        "the privacy assertion must follow an actual legacy source release"
    );
    let state = w.rules.torpedoes.as_ref().unwrap();
    let id = *state.launches.keys().next().unwrap();
    assert!(state.launches[&id]
        .order
        .as_ref()
        .unwrap()
        .solution
        .is_some());
    assert!(state.flights[&id].order.solution.is_some());
    let internal = serde_json::to_value(state).unwrap();
    let expected_settings = json!({
        "speed_index":0,"depth_m":50.,"enable_run_nm":0.,"bearing_deg":null,
        "spread_deg":[],"cut_wire":true
    });
    let assert_public = |declared: &Value| {
        for (field, expected) in expected_settings.as_object().unwrap() {
            assert_eq!(&declared[field], expected, "release setting {field}");
        }
        assert!(
            declared.get("solution").is_none(),
            "author FCS must be absent from public release declarations: {declared}"
        );
        assert!(
            declared.get("source").is_none(),
            "arbitrary author order narration is not a public weapon source: {declared}"
        );
        let serialized = serde_json::to_string(declared).unwrap();
        assert!(!serialized.contains("quality"));
        assert!(!serialized.contains("unrelated author FCS"));
        assert!(!serialized.contains("private author FCS narrative"));
    };
    let own = torp::own_view(&w, Side::Blue, actor).unwrap();
    assert_public(&own["releases"][0]["declared_order"]);
    let report = torp::unreported_releases(&w, Side::Blue, ContactId(99)).unwrap();
    assert_public(&report["released"][0]["declared_order"]);
    assert_eq!(
        serde_json::to_value(w.rules.torpedoes.as_ref().unwrap()).unwrap(),
        internal
    );

    // A saved pre-ledger flight has the same projection boundary. Removing
    // the declaration here is a controlled old-save fixture, not a new fire.
    let mut historical = w.clone();
    historical
        .rules
        .torpedoes
        .as_mut()
        .unwrap()
        .launches
        .clear();
    let old_flight =
        serde_json::to_value(&historical.rules.torpedoes.as_ref().unwrap().flights).unwrap();
    let own = torp::own_view(&historical, Side::Blue, actor).unwrap();
    assert_public(&own["releases"][0]["declared_order"]);
    assert_eq!(
        serde_json::to_value(&historical.rules.torpedoes.as_ref().unwrap().flights).unwrap(),
        old_flight
    );
}

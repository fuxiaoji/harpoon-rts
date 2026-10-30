use super::*;

#[test]
fn session_and_independent_selfplay_load_the_same_sorted_database_volumes_as_http() {
    let temp = std::env::temp_dir().join(format!(
        "harpoon-mcp-volume-contract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&temp).unwrap();
    let temp = temp.canonicalize().unwrap();
    let scenarios = temp.join("scenarios");
    let volumes = temp.join("volumes");
    std::fs::create_dir(&scenarios).unwrap();
    std::fs::create_dir(&volumes).unwrap();
    let mut db = Database::built_in();
    let mut class = db.platforms["us_ford"].clone();
    class.id = "scenario_assumption_volume_probe".into();
    class.name = "Explicit volume fixture".into();
    class.weapons.clear();
    class.sensors.clear();
    class.deck = None;
    db.platforms.clear();
    db.weapons.clear();
    db.sensors.clear();
    db.platforms.insert(class.id.clone(), class);
    let volume = volumes.join("source.json");
    std::fs::write(&volume, serde_json::to_vec(&db).unwrap()).unwrap();
    let scenario = scenarios.join("volume.json");
    std::fs::write(&scenario,serde_json::to_vec(&json!({"id":"volume","name":"Volume contract","seed":37,"duration_s":300,"context":{"rules_profile":"rts-legacy-v1"},"weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"scenario_assumption_volume_probe","name":"VOLUME","pos":[0,0],"fire_on":false}]})).unwrap()).unwrap();
    let mut session = Session::with_paths(scenarios.clone(), &volumes);
    let started = session.call("new_game", &json!({"scenario":"volume"}));
    let selfplay = session.call("run_selfplay", &json!({"scenario":"volume","max_ticks":1}));
    // Remove only the individually created files/directories in this exact
    // owned temporary root. No recursive or computed cross-shell removal.
    for p in [&volume, &scenario] {
        assert!(p.starts_with(&temp));
        std::fs::remove_file(p).unwrap();
    }
    for p in [&volumes, &scenarios, &temp] {
        std::fs::remove_dir(p).unwrap();
    }
    assert!(
        started.is_ok(),
        "MCP must load catalog classes present in HTTP volumes: {started:?}"
    );
    assert!(
        selfplay.is_ok(),
        "independent selfplay must use the same database: {selfplay:?}"
    );
    assert_eq!(
        session.world.as_ref().unwrap().tick,
        0,
        "selfplay cannot alter the live session"
    );
}

fn session(source_navigation: bool) -> Session {
    let context = if source_navigation {
        json!({"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},"navigation_2026":{"source":"scenario_assumption: protocol fixture","units":{"BLUE":{"source":"scenario_assumption: source navigation contract","size":"D","merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,"undamaged_max_kts":30,"propulsion":"NonSteam","readiness":"Underway"}}}})
    } else {
        json!({"rules_profile":"rts-legacy-v1"})
    };
    let sc:Scenario=serde_json::from_value(json!({"id":"protocol","name":"Command protocol fixture","seed":77,"duration_s":3000,"context":context,"weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"us_ford","name":"BLUE","pos":[0,0],"speed_kts":0,"course_deg":0,"fire_on":false},{"side":"Red","class":"us_ford","name":"RED","pos":[1000,0],"speed_kts":0,"course_deg":0,"fire_on":false}]})).unwrap();
    let mut world = load_scenario(sc, Database::built_in()).unwrap();
    for e in world.entities.values_mut() {
        e.sensors.clear();
        e.fire_on = false;
    }
    Session {
        world: Some(world),
        ..Session::new()
    }
}
fn blue(s: &Session) -> harpoon_core::EntityId {
    s.world
        .as_ref()
        .unwrap()
        .entities
        .values()
        .find(|e| e.name == "BLUE")
        .unwrap()
        .id
}
fn branch(schema: &Value, cmd: &str) -> Value {
    schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["properties"]["cmd"]["const"] == cmd)
        .unwrap_or_else(|| panic!("command {cmd} is not discoverable"))
        .clone()
}

#[test]
fn source_commands_and_player_takeover_are_discoverable_to_mcp_clients() {
    let schema = command_schema();
    for cmd in [
        "set_unit_automation",
        "navigation",
        "aviation",
        "air_combat",
        "detection",
        "electronic_warfare",
        "mines",
        "ground",
        "search_and_rescue",
        "torpedo",
        "set_array",
        "set_radar",
        "set_throttle",
        "return_to_base",
        "retask_aircraft",
    ] {
        assert!(
            schema["properties"]["cmd"]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x == cmd),
            "missing command {cmd}"
        );
        branch(&schema, cmd);
    }
}
#[test]
fn documented_waypoints_and_air_mission_points_match_real_command_serialization() {
    let command = Command::Goto {
        unit: harpoon_core::EntityId(1),
        waypoints: vec![harpoon_core::geom::Vec2::new(4., 5.)],
    };
    assert!(serde_json::to_value(command).unwrap()["waypoints"][0].is_object());
    for kind in [
        branch(&command_schema(), "goto")["properties"]["waypoints"]["items"]["type"].clone(),
        mission_schema()["oneOf"][0]["properties"]["station"]["type"].clone(),
    ] {
        assert!(
            kind == "object"
                || kind
                    .as_array()
                    .is_some_and(|k| k.iter().any(|v| v == "object")),
            "canonical serialized point must be accepted: {kind}"
        );
    }
}
#[test]
fn a_surface_depth_order_and_contact_zero_are_not_excluded_by_the_schema() {
    assert_eq!(
        branch(&command_schema(), "set_depth")["properties"]["depth_m"]["minimum"],
        0
    );
    assert_eq!(
        branch(&command_schema(), "fire")["properties"]["contact"]["minimum"],
        0
    );
}
#[test]
fn selfplay_staff_preserves_an_explicit_player_reservation() {
    let mut s = session(false);
    let id = blue(&s);
    s.call(
        "send_command",
        &json!({"side":"Blue","command":{"cmd":"set_unit_automation","unit":id,"enabled":false}}),
    )
    .unwrap();
    s.call("step", &json!({"ticks":1})).unwrap();
    let own = observe(s.world.as_ref().unwrap(), Side::Blue, 0, 100);
    assert!(own.units[0].manual_control);
    assert!(
        heuristic(&own).is_empty(),
        "staff must not turn weapons-free or assign attacks on a reserved unit"
    );
    s.call(
        "send_command",
        &json!({"side":"Blue","command":{"cmd":"set_unit_automation","unit":id,"enabled":true}}),
    )
    .unwrap();
    s.call("step", &json!({"ticks":1})).unwrap();
    assert!(!heuristic(&observe(s.world.as_ref().unwrap(), Side::Blue, 0, 100)).is_empty());
}
#[test]
fn foreign_and_unknown_units_receive_the_same_rejection_before_entering_the_inbox() {
    let mut s = session(false);
    let foreign = s
        .world
        .as_ref()
        .unwrap()
        .entities
        .values()
        .find(|e| e.side == Side::Red)
        .unwrap()
        .id;
    let foreign = s.call(
        "send_command",
        &json!({"side":"Blue","command":{"cmd":"set_speed","unit":foreign,"speed_kts":5}}),
    );
    let unknown = s.call(
        "send_command",
        &json!({"side":"Blue","command":{"cmd":"set_speed","unit":999999,"speed_kts":5}}),
    );
    assert!(foreign.is_err());
    assert_eq!(foreign.unwrap_err(), unknown.unwrap_err());
    assert!(s.inbox.is_empty());
}
#[test]
fn a_real_source_navigation_command_reaches_the_step_and_own_observation() {
    let mut s = session(true);
    let id = blue(&s);
    s.call("send_command",&json!({"side":"Blue","command":{"cmd":"navigation","unit":id,"action":{"type":"coasting","enabled":true}}})).unwrap();
    s.call("step", &json!({"ticks":1})).unwrap();
    let own = s.call("get_state", &json!({"side":"Blue"})).unwrap();
    assert_eq!(
        own["units"][0]["navigation"]["source_rules"]["coasting"],
        true
    );
    assert_eq!(
        s.call("get_state", &json!({"side":"Red"})).unwrap()["units"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

// Exercise the published descriptions against the actual serde command types,
// including every nested action; the schema cannot silently invent a command.
fn contract_sample(schema: &Value) -> Value {
    if let Some(v) = schema.get("const") {
        return v.clone();
    }
    if let Some(v) = schema.get("enum").and_then(Value::as_array) {
        return v[0].clone();
    }
    for union in ["oneOf", "anyOf"] {
        if let Some(branches) = schema[union].as_array() {
            return contract_sample(&branches[0]);
        }
    }
    let kind = schema["type"]
        .as_str()
        .or_else(|| schema["type"].as_array()?.first()?.as_str())
        .unwrap_or("object");
    match kind {
        "object" => schema["properties"]
            .as_object()
            .map(|fields| {
                Value::Object(
                    fields
                        .iter()
                        .map(|(k, v)| (k.clone(), contract_sample(v)))
                        .collect(),
                )
            })
            .unwrap_or(json!({})),
        "array" => Value::Array((0..schema["minItems"].as_u64().unwrap_or(0)).map(|_|contract_sample(&schema["items"])).collect()),
        "string" => json!("fixture"),
        "boolean" => json!(false),
        "null" => Value::Null,
        "integer" | "number" => schema.get("minimum").cloned().unwrap_or(json!(0)),
        _ => panic!("unsupported contract type {kind}"),
    }
}
#[test]
fn each_published_command_and_nested_action_deserializes_as_a_real_engine_order() {
    let mut s = session(false);
    let id = blue(&s);
    let schema = command_schema();
    let mut count = 0;
    let mut parse_only = 0;
    assert_eq!(schema["oneOf"].as_array().unwrap().len(), 42);
    for variant in schema["oneOf"].as_array().unwrap() {
        let base = contract_sample(variant);
        let actions = variant["properties"]["action"]["oneOf"].as_array();
        let samples = if let Some(actions) = actions {
            actions
                .iter()
                .map(|a| {
                    let mut v = base.clone();
                    v["action"] = contract_sample(a);
                    v
                })
                .collect::<Vec<_>>()
        } else {
            vec![base]
        };
        for mut sample in samples {
            sample["unit"] = json!(id);
            let native: Command = serde_json::from_value(sample.clone())
                .unwrap_or_else(|e| panic!("contract advertised invalid command {sample}: {e}"));
            assert_eq!(native.unit_id(), id);
            count += 1;
            if matches!(
                native,
                Command::Asw{..} | Command::Antiship { .. } | Command::Communications { .. } | Command::Unmanned { .. } | Command::Nuclear { .. } | Command::SearchAndRescue {..}
                    | Command::Ground {
                        action: harpoon_core::rules::ground2026::Action::AlongsideResupply { .. }
                            | harpoon_core::rules::ground2026::Action::CancelAlongsideResupply { .. },
                        ..
                    }
                    | Command::Ground {
                        action: harpoon_core::rules::ground2026::Action::GciVector { .. },
                        ..
                    }
                    | Command::Ground {
                        action: harpoon_core::rules::ground2026::Action::PumpDock { .. },
                        ..
                    }
                    | Command::Navigation { action:harpoon_core::rules::navigation2026::Action::Battery{..},..}
                    | Command::Navigation { action:harpoon_core::rules::navigation2026::Action::Ram{..}|harpoon_core::rules::navigation2026::Action::CancelRam{},..}
                    | Command::Navigation {
                        action: harpoon_core::rules::navigation2026::Action::EmergencyBlow { .. },
                        ..
                    }
                    | Command::Torpedo {
                        action: harpoon_core::rules::torpedo2026::Action::Plot { .. },
                        ..
                    }
                    | Command::Navigation {
                        action: harpoon_core::rules::navigation2026::Action::HardRudder { .. }
                            | harpoon_core::rules::navigation2026::Action::RudderRecovery { .. }
                            | harpoon_core::rules::navigation2026::Action::FreeRudder {},
                        ..
                    }
            ) {
                // This probes shape parsing. The legacy fixture has neither
                // legal Plot inputs nor a source submarine fit for Blow.
                assert!(s
                    .call("send_command", &json!({"side":"Blue","command":sample}))
                    .is_err());
                parse_only += 1;
            } else {
                s.call("send_command", &json!({"side":"Blue","command":sample}))
                    .unwrap();
            }
        }
    }
    assert!(
        count > 60,
        "nested commands must be exercised, not merely the outer discriminator"
    );
    assert_eq!(
        parse_only, 43,
        "source-fitted Plot, navigation, communications, unmanned, nuclear and berth actions require separate real-context execution tests"
    );
    assert_eq!(s.inbox.len(), count - parse_only);
}

fn torpedo_plot_session() -> Session {
    let mut s = session(true);
    let actor = blue(&s);
    let w = s.world.as_mut().unwrap();
    // The minimal built-in protocol database does not carry Mk48. This
    // explicit adapter fixture uses the same sourced weapon definitions as
    // the real HTTP/MCP database loader, without changing old shape probes.
    w.db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let target = w
        .entities
        .values()
        .find(|e| e.side == Side::Red)
        .unwrap()
        .id;
    let e = w.entity_mut(actor).unwrap();
    let mut mount = e.weapons[0].clone();
    mount.weapon = "us_mk48".into();
    mount.ammo = 4;
    mount.mounts = 1;
    mount.cooldown_s = 0.;
    e.weapons.push(mount);
    let contact = serde_json::from_value(json!({"id":99,"about":target,"category":"Sub","class_name":null,
        "estimate":{"x":0,"y":0.5},"est_course_deg":0,"est_speed_kts":0,"bearing_only":false,"quality":1,
        "last_update_tick":0,"first_tick":0,"stale":false,"sensors":["scenario_assumption: own report"],
        "offset":{"x":0,"y":0},"announced":true})).unwrap();
    w.sides[0]
        .contacts
        .insert(harpoon_core::ContactId(99), contact);
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", target.0),
        harpoon_core::types::PlatformKind::Sub,
    );
    s
}
fn torpedo_plot_command(unit: harpoon_core::EntityId) -> Value {
    json!({"cmd":"torpedo","unit":unit,"action":{"type":"plot","contact":99,"weapon":"us_mk48",
        "speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":0,"spread_deg":[],"cut_wire":false}})
}
#[test]
fn source_torpedo_mcp_plot_is_prechecked_and_cannot_inject_fcs() {
    let mut s = torpedo_plot_session();
    let actor = blue(&s);
    for (field, bad) in [
        ("speed_index", json!(999)),
        ("depth_m", json!(1e9)),
        ("enable_run_nm", json!(1e9)),
        ("contact", json!(99999)),
    ] {
        let mut invalid = torpedo_plot_command(actor);
        invalid["action"][field] = bad;
        assert!(
            s.call("send_command", &json!({"side":"Blue","command":invalid}))
                .is_err(),
            "invalid {field} must not queue"
        );
        assert!(s.inbox.is_empty());
    }
    for field in ["source", "quality", "solution", "d100", "target"] {
        let mut forged = torpedo_plot_command(actor);
        forged["action"][field] = json!(1);
        assert!(s
            .call("send_command", &json!({"side":"Blue","command":forged}))
            .unwrap_err()
            .contains("unknown field"));
    }
    assert!(s.inbox.is_empty());
}
#[test]
fn source_torpedo_mcp_plan_executes_with_persistent_manual_control_and_unknown_fcs_holds_fire() {
    let mut s = torpedo_plot_session();
    let actor = blue(&s);
    s.call("send_command",&json!({"side":"Blue","command":{"cmd":"set_unit_automation","unit":actor,"enabled":false}})).unwrap();
    s.call("step", &json!({"ticks":1})).unwrap();
    s.call(
        "send_command",
        &json!({"side":"Blue","command":torpedo_plot_command(actor)}),
    )
    .unwrap();
    s.call("step", &json!({"ticks":1})).unwrap();
    let own = s.call("get_state", &json!({"side":"Blue"})).unwrap();
    assert_eq!(own["units"][0]["manual_control"], true);
    assert_eq!(own["units"][0]["torpedoes"]["plans"][0]["contact"], 99);
    let profile = &own["units"][0]["torpedoes"]["weapon_profiles"]["us_mk48"];
    assert!(!profile.is_null());
    assert!(profile["settings"]
        .as_array()
        .is_some_and(|a| !a.is_empty()));
    let enemy = s.call("get_state", &json!({"side":"Red"})).unwrap();
    assert!(enemy["units"][0]["torpedoes"]["plans"]
        .as_array()
        .is_none_or(Vec::is_empty));
    s.call("send_command",&json!({"side":"Blue","command":{"cmd":"fire","unit":actor,"contact":99,"weapon":"us_mk48","rounds":1}})).unwrap();
    s.call("step", &json!({"ticks":1})).unwrap();
    let w = s.world.as_ref().unwrap();
    assert_eq!(
        w.stats.projectiles_launched, 0,
        "a Plot cannot manufacture sensor FCS or a verified physical tube fit"
    );
    assert_eq!(
        w.entity(actor)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "us_mk48")
            .unwrap()
            .ammo,
        4
    );
    assert!(observe(w, Side::Blue, 0, 0).units[0].manual_control);
}

fn antiship_session() -> Session {
    let mut s = session(true);
    let actor = blue(&s);
    let w = s.world.as_mut().unwrap();
    assert!(w.db.weapons.contains_key("us_rgm84d_ic"));
    // Author adapter fixture only: the numerical hull is not evidence that a
    // real Ford carries IC. This explicit mount tests the player's source plan.
    let launcher = w.entities.get_mut(&actor).unwrap();
    launcher.weapons.truncate(1);
    launcher.weapons[0].weapon = "us_rgm84d_ic".into();
    launcher.weapons[0].ammo = 4;
    let target = w
        .entities
        .values()
        .find(|e| e.side == Side::Red)
        .unwrap()
        .id;
    let contact = harpoon_core::world::Contact {
        id: harpoon_core::ContactId(71),
        about: target,
        category: harpoon_core::types::PlatformKind::Ship,
        class_name: None,
        estimate: harpoon_core::Vec2::new(0., 60.),
        est_course_deg: 0.,
        est_speed_kts: 0.,
        bearing_only: false,
        quality: 1.,
        last_update_tick: 0,
        first_tick: 0,
        stale: false,
        sensors: vec!["scenario_assumption: MCP current located ship report".into()],
        offset: harpoon_core::Vec2::zero(),
        announced: true,
    };
    w.sides[0].contacts.insert(contact.id, contact.clone());
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", target.0),
        harpoon_core::types::PlatformKind::Ship,
    );
    let mut foreign = contact;
    foreign.id = harpoon_core::ContactId(72);
    foreign.about = actor;
    w.sides[1].contacts.insert(foreign.id, foreign);
    s
}

fn antiship_plot(unit: harpoon_core::EntityId) -> Value {
    json!({"cmd":"antiship","unit":unit,"action":{"type":"plot","contact":71,"weapon":"us_rgm84d_ic","course_deg":0,"speed_kts":561,"enable_run_nm":40,"waypoints":[],"popup":false}})
}
#[test]
fn source_antiship_plot_is_discoverable_and_native_input_rejects_forged_fcs_and_dice() {
    let shape = branch(&command_schema(), "antiship");
    assert_eq!(
        shape["properties"]["action"]["oneOf"][0]["additionalProperties"],
        false
    );
    let mut s = antiship_session();
    let id = blue(&s);
    let command = antiship_plot(id);
    assert!(serde_json::from_value::<Command>(command.clone()).is_ok());
    s.call(
        "send_command",
        &json!({"side":"Blue","command":command.clone()}),
    )
    .unwrap();
    s.inbox.clear();
    for field in [
        "quality",
        "fcs",
        "source_ecm_generation",
        "d100",
        "probability",
        "unknown",
    ] {
        let mut forged = command.clone();
        forged["action"][field] = json!(1);
        let rejected = s
            .call("send_command", &json!({"side":"Blue","command":forged}))
            .unwrap_err();
        assert!(
            rejected.contains("unknown field"),
            "MCP must reject hidden adjudication input {field} during native decoding: {rejected}"
        );
    }
    let mut forged_outer = command.clone();
    forged_outer["fcs"] = json!("Good");
    assert!(
        s.call(
            "send_command",
            &json!({"side":"Blue","command":forged_outer})
        )
        .is_err(),
        "outer command fields cannot smuggle referee-only FCS either"
    );
    assert!(s.inbox.is_empty());
}
#[test]
fn source_antiship_mcp_plot_preserves_manual_human_orders_and_rejects_foreign_actor() {
    let mut s = antiship_session();
    let id = blue(&s);
    s.call(
        "send_command",
        &json!({"side":"Blue","command":{"cmd":"set_unit_automation","unit":id,"enabled":false}}),
    )
    .unwrap();
    s.call("step", &json!({"ticks":1})).unwrap();
    assert!(observe(s.world.as_ref().unwrap(), Side::Blue, 0, 0).units[0].manual_control);
    for (field, value) in [
        ("contact", json!(99999)),
        ("contact", json!(72)),
        ("speed_kts", json!(560)),
        ("enable_run_nm", json!(0)),
        ("popup", json!(true)),
        ("waypoints", json!([{"x":0,"y":1}])),
    ] {
        let mut invalid = antiship_plot(id);
        invalid["action"][field] = value;
        assert!(
            s.call("send_command", &json!({"side":"Blue","command":invalid}))
                .is_err(),
            "invalid source Plot {field} must be rejected before the inbox"
        );
        assert!(s.inbox.is_empty());
    }
    s.call(
        "send_command",
        &json!({"side":"Blue","command":antiship_plot(id)}),
    )
    .unwrap();
    let foreign = s
        .world
        .as_ref()
        .unwrap()
        .entities
        .values()
        .find(|e| e.side == Side::Red)
        .unwrap()
        .id;
    let denied = s
        .call(
            "send_command",
            &json!({"side":"Blue","command":antiship_plot(foreign)}),
        )
        .unwrap_err();
    let unknown = s
        .call(
            "send_command",
            &json!({"side":"Blue","command":antiship_plot(harpoon_core::EntityId(99999))}),
        )
        .unwrap_err();
    assert_eq!(denied, unknown);
    assert_eq!(s.inbox.len(), 1);
    assert_eq!(s.inbox[0].1.unit_id(), id);
    assert_eq!(
        serde_json::to_value(&s.inbox[0].1).unwrap()["action"]["contact"],
        71
    );
    s.call("step", &json!({"ticks":1})).unwrap();
    let blue_view = s.call("get_state", &json!({"side":"Blue"})).unwrap();
    let unit = blue_view["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == json!(id))
        .unwrap();
    assert_eq!(unit["antiship"]["plots"][0]["contact"], 71);
    assert_eq!(unit["manual_control"], true);
    let red_view = s.call("get_state", &json!({"side":"Red"})).unwrap();
    assert!(
        red_view["units"]
            .as_array()
            .unwrap()
            .iter()
            .all(|u| u["antiship"]["plots"].as_array().is_none_or(Vec::is_empty)),
        "the other side cannot receive the player's plot"
    );
}

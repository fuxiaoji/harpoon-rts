//! Own prelaunch contract adapters. These are not firing-solution fixtures.
use super::*;
use harpoon_core::rules::torpedo2026::{self, Action};

fn public_weapon_profile(weapon: &str) -> Value {
    let source = torpedo2026::tables()["weapons"][weapon].clone();
    json!({"guidance":source["guidance"],"generation":source["generation"],
        "settings":source["settings"],"max_depth_m":source["max_depth_m"],
        "surface":source["surface"],"submarine":source["submarine"],"source":source["source"]})
}

fn state() -> (harpoon_core::World, Observation) {
    let sc=scenario_from_json(r#"{"id":"torpedo-controller-contract","name":"Author adapter fixture","duration_s":1000,"seed":1,"context":{"rules_profile":"rts-legacy-v1"},"weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"ffg","name":"OWN","pos":[0,0],"fire_on":false},{"side":"Red","class":"ffg","name":"REFEREE_TORPEDO_SECRET","pos":[999,999],"fire_on":false}]}"#).unwrap();
    let world = load_scenario(sc, Database::built_in()).unwrap();
    let mut obs = harpoon_core::observe(&world, Side::Blue, 0, 0);
    obs.rules_profile = harpoon_core::rules::damage_runtime::RuleProfile::Rts2026Full;
    let u = &mut obs.units[0];
    u.kind = harpoon_core::types::PlatformKind::Sub;
    u.weapons[0].weapon = "us_mk48".into();
    u.weapons[0].ammo = 4;
    u.torpedoes = Some(
        json!({"verified":true,"tubes":[{"index":0,"loaded":"us_mk48","servicing":false}],"reserves":{},"reloads":[],"releases":[],"plans":[],"weapon_profiles":{"us_mk48":public_weapon_profile("us_mk48")}}),
    );
    obs.contacts.push(serde_json::from_value(json!({"id":71,"category":"Ship","class_name":null,"estimate":{"x":0,"y":20},"est_course_deg":0,"est_speed_kts":0,"quality":1,"bearing_only":false,"last_seen_s":0,"stale":false,"sensors":[],"own_in_flight":0})).unwrap());
    (world, obs)
}
fn plot(unit: EntityId, contact: u32) -> Command {
    Command::Torpedo {
        unit,
        action: Action::Plot {
            contact: harpoon_core::ContactId(contact),
            weapon: "us_mk48".into(),
            speed_index: 1,
            depth_m: 30.,
            enable_run_nm: 8.,
            bearing_deg: Some(0.),
            spread_deg: vec![0.],
            cut_wire: false,
        },
    }
}
fn context(language: &str) -> TurnContext {
    let mut settings = Settings::from_env();
    settings.language = language.into();
    TurnContext {
        supervision: Value::Null,
        side: Side::Blue,
        trigger: "test".into(),
        settings,
        autonomy: "advise".into(),
        memory: Memory::default(),
        recent: vec![],
        generation: 0,
        scenario_id: "torpedo-controller-contract".into(),
        seed: 1,
        tick: 0,
        director: false,
        wake_rules: vec![],
        trigger_detail: String::new(),
    }
}
#[test]
fn source_torpedo_plot_validation_requires_own_contact_v4_and_finite_legal_own_settings() {
    let (_, obs) = state();
    let unit = obs.units[0].id;
    assert!(validate_command(&obs, Side::Blue, &plot(unit, 71)).is_ok());
    assert!(validate_command(&obs, Side::Blue, &plot(unit, 999)).is_err());
    for invalid in ["depth", "enable", "bearing", "spread", "setting", "mount"] {
        let mut command = plot(unit, 71);
        let Command::Torpedo {
            action:
                Action::Plot {
                    weapon,
                    speed_index,
                    depth_m,
                    enable_run_nm,
                    bearing_deg,
                    spread_deg,
                    ..
                },
            ..
        } = &mut command
        else {
            unreachable!()
        };
        match invalid {
            "depth" => *depth_m = f64::NAN,
            "enable" => *enable_run_nm = -1.,
            "bearing" => *bearing_deg = Some(f64::INFINITY),
            "spread" => spread_deg.push(f64::NAN),
            "setting" => *speed_index = 999,
            "mount" => *weapon = "unmounted_author_fixture".into(),
            _ => unreachable!(),
        }
        assert!(
            validate_command(&obs, Side::Blue, &command).is_err(),
            "invalid own prelaunch order: {invalid}"
        );
    }
    let mut old = obs;
    old.rules_profile = harpoon_core::rules::damage_runtime::RuleProfile::Legacy;
    assert!(validate_command(&old, Side::Blue, &plot(unit, 71)).is_err());
}
#[test]
fn source_torpedo_plot_keeps_human_reservation_and_rejects_client_author_solution_fields() {
    let (_, mut obs) = state();
    let unit = obs.units[0].id;
    obs.units[0].manual_control = true;
    let command = plot(unit, 71);
    assert!(validate_commands(&obs, Side::Blue, &[command.clone()])
        .0
        .is_empty());
    assert_eq!(
        validate_player_commands(&obs, Side::Blue, &[command.clone()])
            .0
            .len(),
        1
    );
    for field in ["source", "quality", "solution", "d100", "target"] {
        let mut raw = serde_json::to_value(&command).unwrap();
        raw["action"][field] = json!("REFEREE_FIELD_NOT_A_PLAYER_ORDER");
        assert!(
            serde_json::from_value::<Command>(raw).is_err(),
            "client cannot smuggle {field}"
        );
    }
}
#[test]
fn source_torpedo_llm_keeps_only_own_plans_profiles_and_unreported_status() {
    let (world, mut obs) = state();
    let unit = obs.units[0].id;
    let Command::Torpedo {
        action:
            Action::Plot {
                contact,
                weapon,
                speed_index,
                depth_m,
                enable_run_nm,
                bearing_deg,
                spread_deg,
                cut_wire,
            },
        ..
    } = plot(unit, 71)
    else {
        unreachable!()
    };
    obs.units[0].torpedoes.as_mut().unwrap()["plans"] = json!([{"actor":unit,"side":"Blue","contact":contact,"weapon":weapon,"plotted_at":0,"speed_index":speed_index,"depth_m":depth_m,"enable_run_nm":enable_run_nm,"bearing_deg":bearing_deg,"spread_deg":spread_deg,"cut_wire":cut_wire}]);
    obs.contacts[0].own_in_flight = None;
    obs.contacts[0].own_torpedo_releases = Some(
        json!({"status":"unknown","physical_in_flight":null,"released":[{"weapon":"us_mk48","reported_status":null,"physical_alive":null}]}),
    );
    for lang in ["zh-CN", "en-US"] {
        let prompt = build_user_prompt(&context(lang), &obs, &world.db, None);
        for public in [
            "\"plans\":[",
            "\"weapon_profiles\":{",
            "\"speed_index\":1",
            "\"own_in_flight\":null",
            "\"physical_in_flight\":null",
        ] {
            assert!(
                prompt.contains(public),
                "own contract was dropped: {public}"
            );
        }
        for private in [
            "REFEREE_TORPEDO_SECRET",
            "AUTHOR_GOOD_MUST_NOT_BE_INHERITED",
            "\"last_d100\":",
            "\"last_probability\":",
            "\"solution\":",
        ] {
            assert!(!prompt.contains(private), "referee data leaked: {private}");
        }
        let system = build_system_prompt(&context(lang));
        assert!(
            system.contains("torpedoes.plans"),
            "LLM needs the prelaunch plan boundary, not only generic 2-4 torpedo doctrine"
        );
        assert!(system.contains("weapon_profiles"));
    }
}

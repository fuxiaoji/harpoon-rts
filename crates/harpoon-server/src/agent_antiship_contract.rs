//! Controller transport/observation tests; author fixture data are not annex claims.
use super::*;
use harpoon_core::rules::surface2026::antiship::Action;

fn state() -> (harpoon_core::World, Observation) {
    let sc = scenario_from_json(r#"{"id":"ascm-adapter","name":"Author controller fixture","duration_s":1000,"seed":1,"weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"ffg","name":"OWN","pos":[0,0],"fire_on":false},{"side":"Red","class":"ffg","name":"REFEREE_ASCM_SECRET","pos":[99,88],"fire_on":false}]}"#).unwrap();
    let world = load_scenario(sc, Database::built_in()).unwrap();
    let mut obs = harpoon_core::observe(&world, Side::Blue, 0, 0);
    obs.rules_profile = harpoon_core::rules::damage_runtime::RuleProfile::Rts2026Full;
    obs.units[0].weapons[0].weapon = "us_rgm84d_ic".into();
    obs.units[0].weapons[0].ammo = 4;
    obs.contacts.push(serde_json::from_value(json!({"id":71,"category":"Ship","class_name":null,"estimate":{"x":0,"y":40},"est_course_deg":0,"est_speed_kts":0,"quality":1,"bearing_only":false,"last_seen_s":0,"stale":false,"sensors":[],"own_in_flight":0})).unwrap());
    (world, obs)
}
fn plot(unit: EntityId, contact: u32) -> Command {
    Command::Antiship {
        unit,
        action: Action::Plot {
            contact: harpoon_core::ContactId(contact),
            weapon: "us_rgm84d_ic".into(),
            course_deg: 0.,
            speed_kts: 561.,
            enable_run_nm: 40.,
            waypoints: vec![],
            popup: false,
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
        scenario_id: "ascm-adapter".into(),
        seed: 1,
        tick: 0,
        director: false,
        wake_rules: vec![],
        trigger_detail: String::new(),
    }
}

#[test]
fn source_antiship_plot_requires_the_selected_own_contact_and_v4_before_queue() {
    let (_, mut obs) = state();
    let unit = obs.units[0].id;
    assert!(validate_command(&obs, Side::Blue, &plot(unit, 71)).is_ok());
    assert!(
        validate_command(&obs, Side::Blue, &plot(unit, 999)).is_err(),
        "a referee entity or foreign contact cannot replace a selected own contact"
    );
    obs.rules_profile = harpoon_core::rules::damage_runtime::RuleProfile::Rts2026Operations;
    assert!(validate_command(&obs, Side::Blue, &plot(unit, 71)).is_err());
}

#[test]
fn source_antiship_plot_checks_finite_orders_and_own_mount_without_guessing_source_data() {
    let (_, obs) = state();
    let unit = obs.units[0].id;
    for field in ["course", "speed", "enable", "waypoint", "mount"] {
        let mut command = plot(unit, 71);
        let Command::Antiship {
            action:
                Action::Plot {
                    course_deg,
                    speed_kts,
                    enable_run_nm,
                    waypoints,
                    weapon,
                    ..
                },
            ..
        } = &mut command
        else {
            unreachable!()
        };
        match field {
            "course" => *course_deg = f64::NAN,
            "speed" => *speed_kts = -1.,
            "enable" => *enable_run_nm = -1.,
            "waypoint" => waypoints.push(harpoon_core::Vec2::new(f64::INFINITY, 0.)),
            "mount" => *weapon = "unmounted_author_fixture".into(),
            _ => unreachable!(),
        }
        assert!(
            validate_command(&obs, Side::Blue, &command).is_err(),
            "invalid own order {field} must be rejected before transfer of control"
        );
    }
}

#[test]
fn source_antiship_human_plot_survives_manual_reservation_but_ai_cannot_override_it() {
    let (_, mut obs) = state();
    let unit = obs.units[0].id;
    obs.units[0].manual_control = true;
    let command = plot(unit, 71);
    assert!(validate_commands(&obs, Side::Blue, &[command.clone()])
        .0
        .is_empty());
    assert_eq!(
        validate_player_commands(&obs, Side::Blue, &[command])
            .0
            .len(),
        1
    );
    let human_salvo = Command::Fire {
        unit,
        contact: harpoon_core::ContactId(71),
        weapon: "us_rgm84d_ic".into(),
        rounds: 4,
    };
    assert_eq!(validate_player_commands(&obs,Side::Blue,&[human_salvo.clone()]).0.len(),1,"conservative FSM assistance must retain the player's explicit four-round request; actual source limits remain an engine gate");
    assert!(validate_commands(&obs, Side::Blue, &[human_salvo])
        .0
        .is_empty());
}

#[test]
fn source_antiship_llm_preserves_the_existing_own_plot_in_both_languages() {
    let (world, mut obs) = state();
    let unit = obs.units[0].id;
    obs.units[0].antiship = Some(
        json!({"profile":"hv2026-rts-v4","execution":"source_executor","plots":[{"actor":unit,"side":"Blue","plotted_at":0,"contact":71,"weapon":"us_rgm84d_ic","course_deg":0,"speed_kts":561,"enable_run_nm":40,"waypoints":[],"popup":false}]}),
    );
    for language in ["zh-CN", "en-US"] {
        let prompt = build_user_prompt(&context(language), &obs, &world.db, None);
        assert!(
            prompt.contains("\"enable_run_nm\":40"),
            "own flight orders must reach the LLM"
        );
        assert!(prompt.contains("us_rgm84d_ic"));
        assert!(!prompt.contains("REFEREE_ASCM_SECRET"));
        for private in [
            "lock_trials",
            "fcs_source_snapshot",
            "source_ecm_generation",
            "d100",
            "placement",
        ] {
            // Match actual serialized referee fields, not public prose such
            // as assessment.plan_checks' "ready replacement" policy.
            let private_key = format!("\"{private}\":");
            assert!(
                !prompt.contains(&private_key),
                "referee flight state leaked: {private}"
            );
        }
        let system = build_system_prompt(&context(language));
        assert!(system.contains("antiship"));
    }
}

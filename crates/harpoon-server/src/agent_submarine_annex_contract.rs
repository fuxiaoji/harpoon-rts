//! The LLM consumes a game-band source cap, not an invented engineering depth.
use super::*;

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
        scenario_id: "annex-agent".into(),
        seed: 20261024,
        tick: 0,
        director: false,
        wake_rules: vec![],
        trigger_detail: String::new(),
    }
}
fn fixture() -> World {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc=scenario_from_json(r#"{"id":"annex-agent","name":"Own source capability","seed":20261024,"duration_s":5000,"context":{"rules_profile":"hv2026-rts-v4"},"weather":{"visibility_nm":40,"jam":[0,0]},"units":[{"side":"Blue","class":"cn_kilo","name":"OWN","pos":[0,0],"depth_m":50,"speed_kts":0,"fire_on":false}]}"#).unwrap();
    load_scenario(sc, db).unwrap()
}

#[test]
fn annex_llm_receives_real_own_game_band_cap_and_unknown_charge_in_both_languages() {
    let world = fixture();
    let own = harpoon_core::observe(&world, Side::Blue, 0, 0);
    for language in ["zh-CN", "en-US"] {
        let prompt = build_user_prompt(&context(language), &own, &world.db, None);
        assert!(
            prompt.contains("\"maximum_game_depth_m\":400") && prompt.contains("Int IV"),
            "actual own Annex capability missing for {language}"
        );
        assert!(prompt.contains("\"battery\":null"));
        let system = build_system_prompt(&context(language));
        assert!(
            system.contains("maximum_game_depth_m") && system.contains("engineering"),
            "both languages must distinguish game-band conversion from measured engineering limits"
        );
    }
}

#[test]
fn annex_llm_and_player_observation_validators_enforce_the_same_known_game_cap() {
    let world = fixture();
    let own = harpoon_core::observe(&world, Side::Blue, 0, 0);
    for depth in [400., 401.] {
        let command = Command::SetDepth {
            unit: harpoon_core::EntityId(1),
            depth_m: depth,
        };
        for (accepted, rejected) in [
            validate_commands(&own, Side::Blue, &[command.clone()]),
            validate_player_commands(&own, Side::Blue, &[command]),
        ] {
            if depth == 400. {
                assert_eq!(accepted.len(), 1);
                assert!(rejected.is_empty());
            } else {
                assert!(
                    accepted.is_empty(),
                    "known Int IV limit must be used before LLM/player queue"
                );
                assert!(!rejected.is_empty());
            }
        }
    }
}

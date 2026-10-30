//! An unknown v4 battery is neither a full battery nor free endurance.
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
        scenario_id: "battery-source-contract".into(),
        seed: 20261024,
        tick: 0,
        director: false,
        wake_rules: vec![],
        trigger_detail: String::new(),
    }
}

#[test]
fn source_battery_llm_consumes_real_observation_without_fake_full_charge() {
    let sc=scenario_from_json(r#"{"id":"battery-source-contract","name":"Unbound battery projection","duration_s":10000,"seed":20261024,"context":{"rules_profile":"hv2026-rts-v4"},"weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"ssk","name":"OWN_UNBOUND_BATTERY","pos":[0,0],"depth_m":30,"fire_on":false}]}"#).unwrap();
    let mut w = load_scenario(sc, Database::built_in()).unwrap();
    // A saved old normalized scalar does not bind AnnexA rating, technology
    // or current charge units for the v4 source runtime.
    w.entity_mut(EntityId(1))
        .unwrap()
        .ext
        .insert("battery".into(), 100.);
    let obs = harpoon_core::observe(&w, Side::Blue, 0, 0);
    for language in ["zh-CN", "en-US"] {
        let prompt = build_user_prompt(&context(language), &obs, &w.db, None);
        assert!(
            prompt.contains("\"battery\":null"),
            "unknown source charge must stay null for {language}: {prompt}"
        );
        assert!(!prompt.contains("\"battery\":100"));
    }
}

#[test]
fn source_battery_llm_explains_unadjudicated_endurance_without_implying_unlimited_power() {
    for language in ["zh-CN", "en-US"] {
        let system = build_system_prompt(&context(language)).to_lowercase();
        assert!(system.contains("3.2.4") && system.contains("battery")
            && system.contains("unadjudicated") && system.contains("unlimited"),
            "both prompt languages need the source-battery unknown boundary, including that withdrawing defaults is not unlimited endurance");
    }
}

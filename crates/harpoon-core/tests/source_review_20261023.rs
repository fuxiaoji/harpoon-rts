//! Independent source review regressions. English 2026 PDF73 (§6.3.3),
//! PDF69–70 (§6.2.2–6.2.3), US Navy2025 PDF34/75 (Mk45 Mod4 N).
//! Added before repairing the source executors; run by the parent integration agent.
use harpoon_core::rules::{aaw2026, firecontrol2026 as fcs};
use harpoon_core::{Database, EntityId, Scenario, Side, Vec2, World};

fn passive_rf_crossfix(generation: usize, turns: i32, range: f64) -> fcs::Input {
    fcs::Input {
        kind: fcs::Kind::PassiveRf,
        track_turns: turns,
        range_nm: range,
        contact_speed: 18.,
        combat_generation: 4,
        combat_aircraft: false,
        oth_targeting: true,
        aircraft_rf: false,
        seeker_generation: 2,
        weapon_speed_kts: 1000.,
        long_range_seeker: false,
        scout: false,
        tdl_cue: 0,
        decoy: false,
        crossfix_generation: Some(generation),
        direct_visual_ranging: false,
        passive_array: None,
        zig: false,
        cz: None,
    }
}

#[test]
fn crossfix_uses_active_rf_table_before_es_generation_quality_adjustment() {
    // PDF73: every ES cross-fix generation uses Active RF, not Passive RF.
    // At CT0 and 10 nmi the active table is Good, the passive table is Fair.
    assert_eq!(
        fcs::solve(&passive_rf_crossfix(4, 0, 10.)).unwrap().quality,
        fcs::Quality::Good
    );
    assert_eq!(
        fcs::solve(&passive_rf_crossfix(1, 0, 10.)).unwrap().quality,
        fcs::Quality::Fair
    );
    // At CT1 / 40 nmi, the printed active Good rises no further than Good.
    assert_eq!(
        fcs::solve(&passive_rf_crossfix(6, 1, 40.)).unwrap().quality,
        fcs::Quality::Good
    );
}

#[test]
fn acoustic_passive_ranging_cannot_be_added_to_active_or_visual_solution() {
    let mut input = passive_rf_crossfix(4, 0, 10.);
    input.crossfix_generation = None;
    input.passive_array = Some("wavefront".into());
    for kind in [fcs::Kind::ActiveAcoustic, fcs::Kind::Visual] {
        input.kind = kind;
        assert!(
            fcs::solve(&input).is_err(),
            "§6.3.4 passive ranging bonus requires a passive-acoustic solution"
        );
    }
}

fn burke_gun_fixture() -> (World, EntityId, EntityId, String) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let scenario: Scenario = serde_json::from_value(serde_json::json!({
        "id":"director-source-review","name":"Director source review",
        "duration_s":1000,"seed":20261023,
        "context":{"rules_profile":"hv2026-rts-v4",
            "flight_environment":{"wind_from_deg":0,"wind_kts":0},
            "radar_environment":{"sea_state":0,"precipitation":"None"}},
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[
            {"side":"Blue","class":"us_burke_2a","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_052d","pos":[0,50],"speed_kts":0,"fire_on":false}
        ]
    }))
    .unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let blue = w
        .entities
        .values()
        .find(|e| e.side == Side::Blue)
        .unwrap()
        .id;
    let red = w
        .entities
        .values()
        .find(|e| e.side == Side::Red)
        .unwrap()
        .id;
    // Controlled incoming defense target. This tests the directed gun solution,
    // not a YJ-83 source release; naked v4 ASCM launches require a source permit.
    let mut m = w.entity(red).unwrap().clone();
    m.id = w.allocate_id();
    let missile = m.id;
    m.class = "cn_yj83".into();
    m.name = "controlled incoming director test".into();
    m.kind = harpoon_core::types::PlatformKind::Missile;
    m.rule_data = None;
    m.pos = Vec2::new(0., 1.);
    m.course_deg = 180.;
    m.ordered_course_deg = 180.;
    m.speed_kts = 594.;
    m.ordered_speed_kts = 594.;
    m.altitude_m = 50.;
    m.ordered_altitude_m = 50.;
    m.hp = 1.;
    m.hp_max = 1.;
    m.weapons.clear();
    m.sensors.clear();
    m.ext.clear();
    m.deck = None;
    m.mission = None;
    m.station = None;
    m.projectile = Some(harpoon_core::world::ProjectileData {
        targeted_sensor: None, weapon: "cn_yj83".into(), target: Some(blue),
        aim: Vec2::zero(), launched_by: red, launched_by_side: Side::Red,
        live_s: 0., enabled: true, decoyed: false, decoy_point: None,
        chaff_checked: false, ciws_checked: false, passes: 0,
    });
    w.entities.insert(missile, m);
    let gun = w
        .entity(blue)
        .unwrap()
        .weapons
        .iter()
        .find(|m| {
            aaw2026::tables()["gun_fits"]
                .get(format!("us_burke_2a:{}", m.weapon))
                .is_some()
        })
        .expect("source Mk45 mount")
        .weapon
        .clone();
    (w, blue, missile, gun)
}

#[test]
fn no_local_mk45_cannot_keep_directed_aa_solution_after_director_is_removed() {
    let (mut w, blue, missile, gun) = burke_gun_fixture();
    let gun_def = w.db.weapons[&gun].clone();
    let state = aaw2026::State::default();
    assert!(aaw2026::world_gun_solution(&w, &state, blue, missile, &gun_def, 0).is_ok());
    // The verified source fit is Mk45 Mod4 N (No Local Control). Removing
    // all directors makes a directed AA solution illegal, rather than unchanged.
    let shooter = w.entity_mut(blue).unwrap();
    shooter.sensors.clear();
    shooter.systems.radar = 0.;
    assert!(
        aaw2026::world_gun_solution(&w, &state, blue, missile, &gun_def, 0).is_err(),
        "§6.2.3 and Annex C N require withholding fire when its director is unavailable"
    );
}

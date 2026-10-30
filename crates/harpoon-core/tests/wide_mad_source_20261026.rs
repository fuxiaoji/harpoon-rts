//! Existing-API source candidates; original source snapshots retained.
use harpoon_core::{Database, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
fn seed(die: u8) -> u64 {
    (0..100_000)
        .find(|s| {
            let mut r = Rng::new(*s).derive(Rng::hash_tag(1, 0, "5.7.2:field:1:2:0"));
            1 + (r.next_f64() * 100.) as u8 == die
        })
        .unwrap()
}
fn input(enabled: bool, western: bool, die: u8) -> Value {
    json!({"id":"wide-mad-source","name":"scenario_assumption: prelaid900nm2 MAD field","seed":seed(die),"duration_s":5000,
 "weather":{"visibility_nm":40,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4",
 "detection_2026":{"bindings":{"ru_ka27:ru_mad":{"type":"mad","source":"scenario_assumption: physically fitted Russian MAD, controlled nonwestern operating declaration","range_nm":0.5,"western":western}},
 "targets":{"cn_093":{"source":"scenario_assumption: explicit target signature, not inferred Annex applicability","size":2,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":null}},
 "environment":{"source":"scenario_assumption: clear daylight","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
 "wide_mad":{"source":"English2026 PDF60/61 section5.7.2; explicit author prelaid field is not a native buoy-laying claim","fields":{"field":{"source":"scenario_assumption: prelaid8x3 at5nm as English example","observer":"SEARCH","sensor":"ru_mad","center":{"x":0,"y":0},"course_deg":90,"columns":8,"rows":3,"spacing_nm":5,"prelaid":true,"enabled":enabled,"partial_turn_policy":"pending","depth_tie_policy":"pending"}}}}},
 "units":[{"side":"Blue","class":"ru_ka27","name":"SEARCH","pos":[-15,0],"course_deg":90,"speed_kts":60,"altitude_m":200,"fire_on":false},
 {"side":"Red","class":"cn_093","name":"SUB","pos":[0,5],"course_deg":0,"speed_kts":0,"depth_m":300,"fire_on":false}]})
}
fn load(v: Value) -> Result<World, String> {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    db.platforms.get_mut("ru_ka27").unwrap().sensors = vec!["ru_mad".into()];
    db.platforms.get_mut("ru_ka27").unwrap().weapons.clear();
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    db.platforms.get_mut("cn_093").unwrap().weapons.clear();
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(v).unwrap(), db)
        .map_err(|e| e.to_string())
}
#[test]
fn mad01_actual_it_search_yields_real_native_report_and_restore_without_short_range_mad() {
    let mut w = load(input(true, false, 1)).unwrap();
    for _ in 0..900 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(w
        .find_contact(Side::Blue, harpoon_core::EntityId(2))
        .is_none());
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 900..1800 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    let contact = w
        .find_contact(Side::Blue, harpoon_core::EntityId(2))
        .expect(
            "5.7.2 sourceD1001 at900nm2/IntIII must create actual native contact at IT resolution",
        );
    assert!(!w.contact(Side::Blue, contact).unwrap().bearing_only);
    assert!(w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .any(|t| t.observer == harpoon_core::EntityId(1)
            && t.target == harpoon_core::EntityId(2)
            && t.report.as_ref().is_some_and(|r| r.at == 1800
                && r.sensor == "ru_mad"
                && r.measured_position.is_some())));
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    assert_eq!(
        w.events
            .iter()
            .filter(
                |e| matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="5.7.2.wide_mad_result")
            )
            .count(),
        1
    );
    let actor = harpoon_core::EntityId(1);
    let target = harpoon_core::EntityId(2);
    assert!(harpoon_core::rules::detection2026::wide_mad::only_mad_waiting(&w, actor, target));
    assert!(
        harpoon_core::rules::firecontrol2026::attack_solution(&w, actor, target, 1, 40.).is_err(),
        "the actual same-turn MAD report cannot authorize an immediate attack"
    );
    while w.tick < 1980 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    let solution = harpoon_core::rules::firecontrol2026::attack_solution(&w, actor, target, 1, 40.)
        .expect("the actual MAD report earns Good in the next Tactical Turn");
    assert_eq!(
        solution.quality,
        harpoon_core::rules::firecontrol2026::Quality::Good
    );
    assert!(
        solution.native.is_none(),
        "MAD supplies no numeric ContactTrack or range-column"
    );
    assert_eq!(solution.reported_aim, Some(harpoon_core::Vec2::new(0., 5.)));
    let held = w
        .contact(Side::Blue, contact)
        .expect("earned source datum remains usable");
    assert_eq!(
        held.last_update_tick, 1800,
        "holding a datum cannot forge a later measurement"
    );
    assert_eq!(held.estimate, harpoon_core::Vec2::new(0., 5.));
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}
#[test]
fn mad02_disabled_or_original_failed_it_check_does_not_mint_a_side_contact() {
    for (enabled, die) in [(false, 1), (true, 100)] {
        let mut w = load(input(enabled, false, die)).unwrap();
        for _ in 0..1800 {
            harpoon_core::sim::tick(&mut w, &[]);
        }
        assert!(w
            .find_contact(Side::Blue, harpoon_core::EntityId(2))
            .is_none());
    }
}
#[test]
fn mad03_western_search_receiver_cannot_initialize_a_wide_area_search_field() {
    assert!(
        load(input(true, true, 1)).is_err(),
        "an installed MAD alone is not a nonwestern wide-area source fit"
    );
}

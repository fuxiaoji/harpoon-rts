//! Unregistered source candidates; no Cargo run or witnessed red/green.
//! Native sink_by is an explicitly adjudicated loss boundary, not a claim of a
//! complete weapon engagement. Only actual sim sensors may discover its group.
use harpoon_core::rules::expanded_damage2026 as ex;
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn fixture(distance: f64, visual: bool, extra_hidden_loss: bool) -> World {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    // Original hull DP and all weapon stock retained. Restrict only the
    // experiment's installed receivers; loader-added Ford radar remains radar.
    db.platforms.get_mut("us_ford").unwrap().sensors = if visual {
        vec!["visual".into()]
    } else {
        vec![]
    };
    db.platforms.get_mut("us_burke_2a").unwrap().sensors.clear();
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    let fit = json!({"source":"scenario_assumption: laboratory ten-person complement, no actual Ford/Burke crew claim","crew":10});
    let sc: Scenario = serde_json::from_value(json!({
        "id":"next-native-sar-visual","name":"scenario_assumption: adjudicated loss and native surface lookout","seed":20261025,"duration_s":100000,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":270,"wind_kts":3},"radar_environment":{"sea_state":0,"precipitation":"None"},
            "expanded_damage_2026":{"source":"scenario_assumption: SAR option and explicit complements only","optional":{"search_and_rescue":true},"units":{
                "LOOK":{"source":"scenario_assumption: explicit laboratory rescue station and ten-person capacity","crew":10,"rescue":{"source":"scenario_assumption: ship rescue station geometry only","kind":"ship","capacity":10,"station_radius_m":500}},
                "HIDDEN":fit,"LOST":fit}},
            "detection_2026":{"environment":{"source":"scenario_assumption: clear daytime open water, no terrain or clouds","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
        "units":[
            {"side":"Blue","class":"us_ford","name":"LOOK","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"HIDDEN","pos":[1000,0],"course_deg":0,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"LOST","pos":[distance,0],"course_deg":0,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DUMMY","pos":[2000,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}]
    }))
    .unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let lost = id(&w, "LOST");
    // This is a native loss-consumer input, not manual HP/casualty/survivor state.
    // No pre-loss scan exists; an own kill/loss report is not visual witnessing.
    harpoon_core::damage::sink_by(&mut w, lost, Side::Blue);
    if extra_hidden_loss {
        let hidden = id(&w, "HIDDEN");
        harpoon_core::damage::sink_by(&mut w, hidden, Side::Blue);
    }
    let g = w
        .rules
        .expanded_damage
        .as_ref()
        .unwrap()
        .survivors
        .values()
        .find(|g| g.lost_unit == lost)
        .expect("actual sink_by must create the physical source group");
    assert!(
        (1..10).contains(&g.count),
        "natural D6 and alongside modifier stay in unambiguous <10 scope"
    );
    assert!(ex::survivor_view(&w, id(&w, "LOOK")).is_empty());
    w
}
fn public_reports(w: &World) -> Vec<Value> {
    let own = id(w, "LOOK");
    let observation = serde_json::to_value(harpoon_core::observe(w, Side::Blue,0,256)).unwrap();
    observation["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == own.0)
        .unwrap()["survivors"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}
fn check_whitelist(reports: &[Value]) {
    assert_eq!(reports.len(), 1);
    let r = &reports[0];
    assert_eq!(r["label"], "survivors-1");
    assert_eq!(r["estimated_group_size"], "less_than_ten");
    assert_eq!(r["report_is_live_track"], false);
    for private in [
        "lost_unit",
        "source_crew",
        "count",
        "casualty_d6",
        "drift",
        "sightings",
        "observed_by",
        "group",
    ] {
        assert!(r.get(private).is_none(), "private field {private}");
    }
    assert!(!serde_json::to_string(r).unwrap().contains("survivors:"));
}
#[test]
fn s01_native_surface_visual_finds_a_real_group_without_a_combat_contact() {
    let mut w = fixture(0.01, true, false);
    assert!(public_reports(&w).is_empty());
    harpoon_core::sim::tick(&mut w, &[]);
    let reports = ex::survivor_view(&w, id(&w, "LOOK"));
    check_whitelist(&reports);
    assert_eq!(reports[0]["observed_at"], w.tick);
    assert_eq!(
        public_reports(&w),
        reports,
        "root UnitObs hook must consume only this whitelist"
    );
    assert!(
        w.sides[0].contacts.is_empty(),
        "survivors are noncombat physical points, not synthesized Contact entries"
    );
    let group = reports[0]["label"].as_str().unwrap();
    let cmd: Command = serde_json::from_value(json!({"cmd":"search_and_rescue","unit":id(&w,"LOOK"),"action":{"type":"begin","group":group,"automatic":false}}))
        .expect("actual shared public command must parse");
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
    assert!(w
        .rules
        .expanded_damage
        .as_ref()
        .unwrap()
        .rescues
        .contains_key(&id(&w, "LOOK")));
}
#[test]
fn s02_radar_and_known_loss_are_not_a_successful_survivor_visual_scan() {
    let mut w = fixture(0.01, false, false);
    harpoon_core::sim::tick(&mut w, &[]);
    assert!(ex::survivor_view(&w, id(&w, "LOOK")).is_empty());
    assert!(public_reports(&w).is_empty());
}
#[test]
fn s03_no_wake_half_range_rejects_the_otherwise_visible_g_group_at_nine_nm() {
    // A-to-G at100% is27kyd; no-wake13.5kyd maximum <9nm.
    // Without the required half, even the worst27kyd row sees18.5kyd >9nm.
    let mut w = fixture(9., true, false);
    for _ in 0..181 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(ex::survivor_view(&w, id(&w, "LOOK")).is_empty());
    assert!(public_reports(&w).is_empty());
}
#[test]
fn s04_an_extra_unseen_native_loss_does_not_renumber_the_one_real_sighting() {
    let mut base = fixture(0.01, true, false);
    let mut hidden = fixture(0.01, true, true);
    harpoon_core::sim::tick(&mut base, &[]);
    harpoon_core::sim::tick(&mut hidden, &[]);
    let reports = ex::survivor_view(&base, id(&base, "LOOK"));
    check_whitelist(&reports);
    assert_eq!(ex::survivor_view(&hidden, id(&hidden, "LOOK")), reports);
    assert_eq!(public_reports(&hidden), public_reports(&base));
}
#[test]
fn s05_full_world_restore_before_discovery_keeps_native_evidence_and_snapshot_exact() {
    let mut w = fixture(0.01, true, false);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..181 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    check_whitelist(&ex::survivor_view(&w, id(&w, "LOOK")));
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
    assert_eq!(public_reports(&w), public_reports(&restored));
}
#[test]
fn s06_each_zero_argument_rescue_action_rejects_private_fields_before_execution() {
    for name in ["manual", "automatic", "recover_due", "cancel"] {
        let valid = json!({"type":name});
        assert!(serde_json::from_value::<ex::RescueAction>(valid.clone()).is_ok());
        for field in ["position", "d100", "crew", "ready_at", "picked_up"] {
            let mut bad = valid.clone();
            bad[field] = json!(123);
            assert!(
                serde_json::from_value::<ex::RescueAction>(bad.clone()).is_err(),
                "{name} swallowed private field {field}"
            );
            assert!(serde_json::from_value::<Command>(
                json!({"cmd":"search_and_rescue","unit":1,"action":bad})
            )
            .is_err());
        }
    }
}

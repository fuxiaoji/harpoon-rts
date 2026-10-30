//! Source-only5.8.11 producer candidates. Existing Command envelope + JSON,
//! no future Rust enum, radio/TDL simulation, Contact/FCS/shot/dice injection.
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
const TEXT30: &str = "FORM LINE AHEAD KEEP STATION 1";
fn input(night: bool, visibility: u8, precipitation: usize, port_range_nm: f64) -> Value {
    let mut v = json!({"id":"source-optical-non-tactical-signal","name":"author installed visual signals, actual table visibility, no RF equipment","seed":61027,"duration_s":600,
      "day_night":if night{"Night"}else{"Day"},"weather":{"visibility_nm":40,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","detection_2026":{
        "bindings":{},"targets":{},
        "environment":{"source":"scenario_assumption: controlled open-water unobstructed signal rays; explicit light/precipitation conditions, no cloud","sea_state":0,"precipitation":precipitation,"visibility_percent":visibility,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
        "visual_signaling":{"source":"English2026 PDF65/5-26 Visual Signals Range and68/5-29 section5.8.11; actual author-installed optical terminals only","units":{},"character_policy":"ascii_only_first_scope","phase_policy":"same_tick_normal_detection"}}},
      "units":[{"side":"Blue","class":"us_burke_2a","name":"SEND","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false},
       {"side":"Blue","class":"us_burke_2a","name":"PORT","pos":[-port_range_nm,0],"course_deg":0,"speed_kts":0,"fire_on":false},
       {"side":"Blue","class":"us_burke_2a","name":"STARBOARD","pos":[1,0],"course_deg":0,"speed_kts":0,"fire_on":false},
       {"side":"Red","class":"us_burke_2a","name":"UNSEEN","pos":[100,100],"course_deg":0,"speed_kts":0,"fire_on":false}]});
    let receiver = json!({"source":"scenario_assumption: actual installed optical signal station and personnel; no inferred real Burke terminal fit","installed":true,"receive":["flag_hoist","flashing_light"],"send":{},"height_m":10,"height_source":"scenario_assumption: both actual signal/observer apertures10m","los_source":"scenario_assumption: controlled unobstructed open-water scene, not automatic terrain clearance"});
    for name in ["PORT", "STARBOARD"] {
        v["context"]["detection_2026"]["visual_signaling"]["units"][name] = receiver.clone();
    }
    v["context"]["detection_2026"]["visual_signaling"]["units"]["SEND"] = receiver;
    v["context"]["detection_2026"]["visual_signaling"]["units"]["SEND"]["send"] =
        json!({"port":["flag_hoist","flashing_light"],"starboard":["flag_hoist","flashing_light"]});
    v
}
fn world(night: bool, visibility: u8, port_range_nm: f64) -> World {
    world_with_weather(night, visibility, 0, port_range_nm)
}
fn world_with_weather(
    night: bool,
    visibility: u8,
    precipitation: usize,
    port_range_nm: f64,
) -> World {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let p = db.platforms.get_mut("us_burke_2a").unwrap();
    p.sensors.clear();
    p.weapons.clear();
    p.deck = None;
    let mut w = harpoon_core::load_scenario(
        serde_json::from_value::<Scenario>(input(night, visibility, precipitation, port_range_nm))
            .unwrap(),
        db,
    )
    .unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    assert!(
        harpoon_core::rules::communications2026::validate_transport(
            &w,
            Side::Blue,
            EntityId(1),
            "voice",
            &[EntityId(2)]
        )
        .is_err(),
        "the genuine existing RF transport must remain unavailable; no visual-as-voice fallback"
    );
    isolated(&w);
    w
}
fn isolated(w: &World) {
    assert!(w.sides.iter().all(|s|s.contacts.is_empty()),"optical administrative/formation text cannot create combat Contact about a sender or an unseen target");
    assert!(
        w.rules.aaw.as_ref().unwrap().firecontrol.tracks.is_empty(),
        "non-tactical signal is not a native sensing or FCS report"
    );
    for id in [EntityId(1), EntityId(2), EntityId(3)] {
        let v = harpoon_core::rules::communications2026::own_view(w, id);
        assert!(
            v.as_ref()
                .and_then(|v| v["messages"].as_array())
                .is_none_or(|a| a.is_empty()),
            "a dedicated visual message must not become a voice/TDL warning or solution"
        );
    }
}
fn signal(board: &str, medium: &str, text: &str, receiver: u32) -> (Side, Command) {
    let raw = json!({"cmd":"communications","unit":1,"action":{"type":"send_visual_signal","board":board,"medium":medium,"purpose":"formation","text":text,"receivers":[receiver]}});
    (Side::Blue,serde_json::from_value(raw).expect("5.8.11 missing actual native visual-signal admission; table presence/voice cannot satisfy this source producer"))
}
fn delivered(w: &World, unit: EntityId, text: &str) -> usize {
    w.events
        .iter()
        .filter(|e| match &e.kind {
            EventKind::RulesEvent {
                rule,
                unit: Some(receiver),
                detail,
                ..
            } if rule == "5.8.11.visual_signal_received"
                && *receiver == unit
                && e.vis == harpoon_core::Visibility::Side(Side::Blue) =>
            {
                let v: Value = serde_json::from_str(detail).unwrap();
                v["text"] == text
            }
            _ => false,
        })
        .count()
}
fn pair_tick(w: &mut World, saved: &mut World, cmd: Option<(Side, Command)>) {
    let commands: Vec<_> = cmd.into_iter().collect();
    harpoon_core::sim::tick(w, &commands);
    harpoon_core::sim::tick(saved, &commands);
    isolated(w);
    isolated(saved);
}
#[test]
fn visual_signal01_actual_per_board_30_character_window_persists_without_radio_or_free_fcs() {
    assert_eq!(TEXT30.chars().count(), 30);
    assert!(TEXT30.is_ascii());
    let mut w = world(false, 100, 1.);
    let c = signal("port", "flag_hoist", TEXT30, 2);
    harpoon_core::sim::tick(&mut w, &[c]);
    assert_eq!(delivered(&w,EntityId(2),TEXT30),1,"a real source-qualified flag signal must reach the actual fitted friendly receiver without an RF route");
    isolated(&w);
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let too_long = format!("{TEXT30}X");
    pair_tick(
        &mut w,
        &mut saved,
        Some(signal("starboard", "flag_hoist", &too_long, 3)),
    );
    assert_eq!(
        delivered(&w, EntityId(3), &too_long),
        0,
        "31 ASCII characters are not a valid source signal"
    );
    pair_tick(
        &mut w,
        &mut saved,
        Some(signal("starboard", "flag_hoist", TEXT30, 3)),
    );
    assert_eq!(
        delivered(&w, EntityId(3), TEXT30),
        1,
        "invalid31 chars must not consume the separate starboard window"
    );
    pair_tick(
        &mut w,
        &mut saved,
        Some(signal("port", "flag_hoist", "SECOND FORMATION MESSAGE", 2)),
    );
    assert_eq!(
        delivered(&w, EntityId(2), "SECOND FORMATION MESSAGE"),
        0,
        "only one signal per port side/TacticalTurn, not one per receiving unit or device"
    );
    while w.tick < 178 {
        pair_tick(&mut w, &mut saved, None);
    }
    pair_tick(
        &mut w,
        &mut saved,
        Some(signal("port", "flag_hoist", "BEFORE NEXT TURN", 2)),
    );
    assert_eq!(w.tick, 179);
    assert_eq!(delivered(&w, EntityId(2), "BEFORE NEXT TURN"), 0);
    pair_tick(
        &mut w,
        &mut saved,
        Some(signal(
            "port",
            "flag_hoist",
            "NEXT TURN FORMATION MESSAGE",
            2,
        )),
    );
    assert_eq!(w.tick, 180);
    assert_eq!(
        delivered(&w, EntityId(2), "NEXT TURN FORMATION MESSAGE"),
        1,
        "declared fixed180s TacticalTurn opens a genuinely new sender-side window"
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
}
#[test]
fn visual_signal02_reception_uses_actual_source_range_and_flags_require_daylight() {
    let table = &harpoon_core::rules::detection2026::tables()["visual"]["signal_ranges_kyd"];
    assert_eq!(table[0][0], 8);
    assert_eq!(table[0][1], 16);
    assert_eq!(table[5][2], 20);
    let yard_to_nm = 1. / 2025.3718285;
    for (night, visibility, precipitation, medium, range, expected) in [
        (false, 100, 0, "flag_hoist", 7999. * yard_to_nm, 1),
        (false, 100, 0, "flag_hoist", 8001. * yard_to_nm, 0),
        (true, 50, 0, "flag_hoist", 1., 0),
        (true, 50, 0, "flashing_light", 8., 1),
        // English PDF65/5-26 original10percent Night cell20.0/10.0*:
        //7nm is >10kyd but <20kyd. Misty precipitation must actually stop
        //reception that was valid in clear new-moon conditions; not a string-table check.
        (true, 10, 0, "flashing_light", 7., 1),
        (true, 10, 1, "flashing_light", 7., 0),
    ] {
        let mut w = world_with_weather(night, visibility, precipitation, range);
        let c = signal("port", medium, "MAINTAIN FORMATION", 2);
        harpoon_core::sim::tick(&mut w, &[c]);
        assert_eq!(delivered(&w,EntityId(2),"MAINTAIN FORMATION"),expected,
            "real range/night mode must govern delivery: night={night},visibility={visibility},precipitation={precipitation},medium={medium},distance={range}; no guessed RF or free optical range");
        isolated(&w);
    }
}

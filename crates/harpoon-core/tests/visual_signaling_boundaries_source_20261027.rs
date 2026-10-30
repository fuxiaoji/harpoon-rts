//! Additional5.8.11 source boundary candidates; original source2 remains unchanged.
//! Copied real no-RF source fixture; only normal Command/sim/serde entry points.
//! No direct optical state/manual/damage/Contact/clock/order injection.
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

fn restored(w: &World) -> World {
    serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap()
}
fn manual(w: &World, unit: u32) -> bool {
    w.entity(EntityId(unit)).unwrap().ext.get("control.manual") == Some(&1.)
}
fn trusted(board: &str, text: &str, receiver: u32) -> (Side, Command) {
    let (side, mut cmd) = signal(board, "flag_hoist", text, receiver);
    assert_eq!(cmd.manual_intent(), Some(false));
    assert!(
        cmd.mark_player_intent(),
        "actual trusted transport helper, not a forged retain_manual JSON field"
    );
    assert_eq!(cmd.manual_intent(), Some(true));
    (side, cmd)
}
fn optical(w: &World, unit: u32) -> Value {
    harpoon_core::rules::detection2026::visual_signaling::own_view(w, Side::Blue, EntityId(unit))
}
fn used(w: &World, unit: u32, board: &str) -> bool {
    optical(w, unit)["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["board"] == board)
        .unwrap()["used"]
        .as_bool()
        .unwrap()
}
fn event_count(w: &World, wanted: &str) -> usize {
    w.events
        .iter()
        .filter(|e| {
            matches!(&e.kind,
        EventKind::RulesEvent {rule, ..} if rule == wanted)
        })
        .count()
}
fn assert_side_only_receipts(w: &World) {
    for e in &w.events {
        if matches!(&e.kind, EventKind::RulesEvent {rule, ..}
            if rule == "5.8.11.visual_signal_transmitted" || rule == "5.8.11.visual_signal_received")
        {
            assert_eq!(
                e.vis,
                harpoon_core::Visibility::Side(Side::Blue),
                "actual optical Tx/Rx is an own-side receipt; no public/referee-only fake success"
            );
        }
    }
}
fn tick_pair(w: &mut World, replay: &mut World, commands: &[(Side, Command)]) {
    harpoon_core::sim::tick(w, commands);
    harpoon_core::sim::tick(replay, commands);
    isolated(w);
    isolated(replay);
    assert_eq!(serde_json::to_value(&*w).unwrap(), serde_json::to_value(&*replay).unwrap(),
        "same-order FullWorld serde restore must preserve the complete optical window/mailbox/manual/event state");
}
#[test]
fn visual_signal_boundary01_same_board_conflicts_all_refuse_and_exact_duplicates_merge_in_both_orders(
) {
    for reverse in [false, true] {
        let mut w = world(false, 100, 1.);
        for id in [1, 2, 3] {
            assert!(!manual(&w, id));
        }
        let mut commands = vec![
            trusted("port", "FORM LINE AHEAD", 2),
            trusted("port", "FORM LINE ABREAST", 2),
        ];
        if reverse {
            commands.reverse();
        }
        let untouched = serde_json::to_value(&w).unwrap();
        for (side, cmd) in &commands {
            cmd.validate_source(&w, *side).unwrap();
        }
        assert_eq!(serde_json::to_value(&w).unwrap(), untouched,
            "pure successful individual preflight cannot select a batch winner, spend a slot or acquire manual control");
        let mut replay = restored(&w);
        tick_pair(&mut w, &mut replay, &commands);
        assert_eq!(
            event_count(&w, "5.8.11.visual_signal_transmitted"),
            0,
            "different same-tick sender/board signals must all refuse, regardless of client order"
        );
        assert_eq!(event_count(&w, "5.8.11.visual_signal_received"), 0);
        assert!(
            !used(&w, 1, "port"),
            "batch conflict must not consume the source side/turn window"
        );
        for id in [1, 2, 3] {
            assert!(
                !manual(&w, id),
                "conflict cannot acquire any manual authority"
            );
        }
        assert!(optical(&w, 1)["messages"].as_array().unwrap().is_empty());
        // Restore after refusal, then exercise the still-unused port window.
        replay = restored(&w);
        let duplicate = trusted("port", TEXT30, 2);
        tick_pair(&mut w, &mut replay, &[duplicate.clone(), duplicate]);
        assert_eq!(
            event_count(&w, "5.8.11.visual_signal_transmitted"),
            1,
            "canonical identical requests produce one actual transmission, not one per client"
        );
        assert_eq!(delivered(&w, EntityId(2), TEXT30), 1);
        assert_eq!(event_count(&w, "5.8.11.visual_signal_received"), 1);
        assert!(used(&w, 1, "port"));
        assert!(
            manual(&w, 1),
            "only actual accepted trusted Tx commits sender manual control"
        );
        for id in [2, 3] {
            assert!(
                !manual(&w, id),
                "optical reception cannot take over a receiver"
            );
        }
        assert_side_only_receipts(&w);
        let second = trusted("port", "SECOND SAME TURN SIGNAL", 2);
        tick_pair(&mut w, &mut replay, &[second]);
        assert_eq!(
            event_count(&w, "5.8.11.visual_signal_transmitted"),
            1,
            "merged duplicates still consume exactly one immutable side/turn slot"
        );
        assert_eq!(delivered(&w, EntityId(2), "SECOND SAME TURN SIGNAL"), 0);
    }
}
#[test]
fn visual_signal_boundary02_trusted_intent_commits_only_real_tx_even_when_actual_receiver_is_out_of_range(
) {
    //5nm is physically >8kyd daylight flag range. Sender/reader fits and no-RF
    //preconditions remain genuine; no private position or visibility mutation.
    let mut w = world(false, 100, 5.);
    let too_long = format!("{TEXT30}X");
    let invalid = trusted("port", &too_long, 2);
    let valid = trusted("port", TEXT30, 2);
    let before = serde_json::to_value(&w).unwrap();
    assert!(invalid.1.validate_source(&w, invalid.0).is_err());
    valid.1.validate_source(&w, valid.0).unwrap();
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        before,
        "both rejected and successful pure preflight preserve the complete world"
    );
    let mut replay = restored(&w);
    tick_pair(&mut w, &mut replay, &[invalid]);
    assert_eq!(event_count(&w, "5.8.11.visual_signal_transmitted"), 0);
    assert!(
        !used(&w, 1, "port"),
        "31 printable ASCII chars cannot reserve a window"
    );
    for id in [1, 2, 3] {
        assert!(
            !manual(&w, id),
            "invalid trusted input cannot acquire control"
        );
    }
    replay = restored(&w);
    tick_pair(&mut w, &mut replay, &[valid]);
    assert_eq!(
        event_count(&w, "5.8.11.visual_signal_transmitted"),
        1,
        "a valid actual transmission is committed even when its addressed reader is too far away"
    );
    assert_eq!(delivered(&w, EntityId(2), TEXT30), 0);
    assert_eq!(event_count(&w, "5.8.11.visual_signal_received"), 0);
    assert!(used(&w, 1, "port"), "unreadable actual Tx is not refunded");
    assert!(
        manual(&w, 1),
        "actual trusted Tx must commit sender control without requiring a radio acknowledgement"
    );
    for id in [2, 3] {
        assert!(
            !manual(&w, id),
            "out-of-range recipients receive no authority mutation"
        );
    }
    let sender = optical(&w, 1);
    assert_eq!(sender["messages"].as_array().unwrap().len(), 1);
    assert_eq!(sender["messages"][0]["direction"], "sent");
    assert!(sender["messages"][0]["received_at"].is_null());
    assert!(
        optical(&w, 2)["messages"].as_array().unwrap().is_empty(),
        "no actual reception means no fabricated optical mailbox receipt"
    );
    assert_side_only_receipts(&w);
    tick_pair(
        &mut w,
        &mut replay,
        &[trusted("port", "NO REFUND SAME TURN", 2)],
    );
    assert_eq!(event_count(&w, "5.8.11.visual_signal_transmitted"), 1);
    assert_eq!(delivered(&w, EntityId(2), "NO REFUND SAME TURN"), 0);
}

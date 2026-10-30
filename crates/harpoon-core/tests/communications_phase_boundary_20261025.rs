//! Additional public-domain A boundaries; candidate only, not run by author.
//! Native report/CID provenance comes from real sim::tick scans on BOTH sides.
//! Radio edges, acoustic fits and geometry are explicit scenario assumptions.
//! No Contact, SensorReport, classification, FCS, die or communications state is
//! injected. Marked post-capture referee perturbations isolate noninterference
//! and expiry; they are not represented as natural combat or exercise play.
use harpoon_core::rules::communications2026 as comms;
use harpoon_core::types::PlatformKind;
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, Vec2, World};
use serde_json::{json, Value};

const SENSOR: &str = "lab_phase_native_passive";
const SENDER_CLASS: &str = "lab_phase_native_sender";
const RECEIVER_CLASS: &str = "lab_phase_voice_receiver";
const FIT: &str = "lab_voice";

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn config() -> Value {
    let unit = |peers: Vec<&str>, send: bool| json!({
        "source":"scenario_assumption: exact named laboratory radio fit",
        "fits":{"lab_voice":{"source":"scenario_assumption: explicit directed voice edge, no Annex LOS/range claim",
            "protocol":"author_phase_voice","send":send,"receive":true,"operational":true,"peers":peers}}});
    json!({"source":"scenario_assumption: two-sided warning privacy boundary laboratory",
        "units":{"BLUE_SENDER":unit(vec!["BLUE_RX1","BLUE_RX2"],true),
            "BLUE_RX1":unit(vec![],false),"BLUE_RX2":unit(vec![],false),
            "RED_SENDER":unit(vec!["RED_RX"],true),"RED_RX":unit(vec![],false)}})
}
fn database() -> Database {
    let mut db = Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"))).unwrap();
    let mut sensor = db.sensors.values().next().unwrap().clone();
    sensor.id = SENSOR.into();
    sensor.name = "scenario_assumption: Gen5 LMF laboratory passive source".into();
    sensor.kind = harpoon_core::db::SensorKind::SonarPassive;
    sensor.air_range_nm = 0.; sensor.surface_range_nm = 0.;
    sensor.sub_range_nm = 4.; sensor.missile_range_nm = 0.;
    db.sensors.insert(SENSOR.into(), sensor);
    for (key, sensors) in [(SENDER_CLASS, vec![SENSOR.into()]),(RECEIVER_CLASS,vec![])] {
        let mut p = db.platforms["us_virginia"].clone();
        p.id = key.into();
        p.name = format!("scenario_assumption: original-DP no-weapons laboratory {key}");
        p.sensors = sensors;
        p.weapons.clear();
        db.platforms.insert(key.into(),p);
    }
    db.validate().unwrap(); db
}
fn create(db: Database, seed: u64, communications: Option<Value>) -> World {
    let receiver = json!({"type":"sonar","source":"scenario_assumption: verified Gen5 Bow LMF4nm50percent mechanism fit",
        "generation":5,"passive_range_nm":4,"active_range_nm":null,"frequency":"lmf","mount":"bow",
        "active_torpedo_detection":false,"high_speed_tow":false});
    let signature = json!({"source":"scenario_assumption: controlled Quiet target, not a complete real hull acoustic fit",
        "size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    let bindings = [(format!("{SENDER_CLASS}:{SENSOR}"),receiver)].into_iter().collect::<std::collections::BTreeMap<_,_>>();
    let targets = [(SENDER_CLASS.to_string(),signature.clone()),(RECEIVER_CLASS.to_string(),signature)]
        .into_iter().collect::<std::collections::BTreeMap<_,_>>();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"communications-A-public-phase-boundaries","name":"scenario_assumption: two-sided native reports and API boundary isolation",
        "duration_s":10000,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","communications_2026":communications,
            "radar_environment":{"sea_state":2,"precipitation":"None"},
            "detection_2026":{"environment":{"source":"scenario_assumption: calm open-water source laboratory",
                "sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
                "water_depth_m":3000,"layer":null,"ice":0,"cloud":null},"bindings":bindings,"targets":targets}},
        "units":[
            {"side":"Blue","class":SENDER_CLASS,"name":"BLUE_SENDER","pos":[0,0],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Blue","class":RECEIVER_CLASS,"name":"BLUE_RX1","pos":[-20,-20],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Blue","class":RECEIVER_CLASS,"name":"BLUE_RX2","pos":[-21,-20],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":SENDER_CLASS,"name":"RED_SENDER","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":RECEIVER_CLASS,"name":"RED_RX","pos":[20,20],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}
        ]})).unwrap();
    harpoon_core::load_scenario(sc,db).unwrap()
}
fn fixture_with(communications: Option<Value>) -> (World, ContactId, ContactId) {
    let db = database();
    for seed in 1..=128 {
        let mut w = create(db.clone(),seed,communications.clone());
        let commands: Vec<_> = w.entities.values().map(|e| (e.side,Command::SetUnitAutomation { unit:e.id,enabled:false })).collect();
        harpoon_core::sim::tick(&mut w,&commands);
        let blue = id(&w,"BLUE_SENDER"); let red = id(&w,"RED_SENDER");
        let Some(bcid) = w.sides[0].contacts.values().find(|c| !c.stale && c.about==red).map(|c|c.id) else {continue};
        let Some(rcid) = w.sides[1].contacts.values().find(|c| !c.stale && c.about==blue).map(|c|c.id) else {continue};
        let tracks = &w.rules.aaw.as_ref().unwrap().firecontrol.tracks;
        let native = |observer,target| tracks.values().filter(|t|t.observer==observer&&t.target==target)
            .filter_map(|t|t.report.as_ref()).any(|r|r.sensor==SENSOR&&r.at==1&&r.measured_position.is_none());
        if !native(blue,red)||!native(red,blue) {continue}
        assert!(w.contact(Side::Blue,bcid).unwrap().bearing_only);
        assert!(w.contact(Side::Red,rcid).unwrap().bearing_only);
        assert!(w.entities.values().all(|e|e.ext.get("control.manual")==Some(&1.)));
        return (w,bcid,rcid)
    }
    panic!("native bilateral sonar/report fixture unavailable; do not count this fixture failure as a rule-red")
}
fn fixture() -> (World, ContactId, ContactId) { fixture_with(Some(config())) }
fn action(contact: ContactId, receivers: Vec<EntityId>) -> comms::Action {
    comms::Action::SendWarning { contact,sensor:SENSOR.into(),fit:FIT.into(),receivers }
}
fn bytes(w: &World) -> Vec<u8> { serde_json::to_vec(w).unwrap() }
fn view(w: &World, unit: EntityId) -> Value { comms::own_view(w,unit).unwrap() }
fn messages(w: &World, unit: EntityId) -> Vec<Value> {
    comms::own_view(w,unit).map(|v|v["messages"].as_array().unwrap().clone()).unwrap_or_default()
}
fn only_message(w: &World, unit: EntityId) -> Value {
    let list=messages(w,unit);assert_eq!(list.len(),1);list[0].clone()
}
fn comm_events(w: &World) -> Vec<Value> {
    w.events.iter().filter_map(|e|match &e.kind {
        EventKind::RulesEvent{domain,rule,unit,detail,..} if domain=="communications2026" =>
            Some(json!({"seq":e.seq,"tick":e.tick,"vis":e.vis,"rule":rule,"unit":unit,"detail":serde_json::from_str::<Value>(detail).unwrap()})),
        _=>None
    }).collect()
}
fn physical(w: &World) -> Value {
    json!({"rng":w.rng,"entities":w.entities,"contacts_blue":w.sides[0].contacts,
        "contacts_red":w.sides[1].contacts,"firecontrol":w.rules.aaw.as_ref().map(|s|&s.firecontrol),
        "detection":w.rules.detection})
}

#[test]
fn p01_public_validate_success_and_rejection_are_complete_world_readonly() {
    let (w,cid,_)=fixture();let sender=id(&w,"BLUE_SENDER");let rx=id(&w,"BLUE_RX1");
    let before=bytes(&w);
    for _ in 0..3 {assert!(comms::validate_action(&w,Side::Blue,sender,&action(cid,vec![rx])).is_ok());assert_eq!(bytes(&w),before);}
    for candidate in [action(cid,vec![]),action(cid,vec![rx,rx]),action(cid,vec![sender]),action(ContactId(u32::MAX),vec![rx])] {
        assert!(comms::validate_action(&w,Side::Blue,sender,&candidate).is_err());assert_eq!(bytes(&w),before);
    }
    assert!(comms::validate_action(&w,Side::Red,sender,&action(cid,vec![rx])).is_err());assert_eq!(bytes(&w),before);
}

#[test]
fn p02_public_apply_only_queues_open_delivers_once_without_physical_bonus() {
    let (mut w,cid,_)=fixture();let sender=id(&w,"BLUE_SENDER");let rx=id(&w,"BLUE_RX1");
    let protected=physical(&w);comms::apply(&mut w,Side::Blue,sender,&action(cid,vec![rx])).unwrap();
    assert_eq!(only_message(&w,sender)["state"],"queued");assert!(messages(&w,rx).is_empty());
    assert_eq!(physical(&w),protected,"queue only: no RNG/contact/FCS/unit/detector/manual change");
    comms::detection_open(&mut w);assert_eq!(only_message(&w,rx)["state"],"delivered");assert_eq!(physical(&w),protected);
    let terminal=bytes(&w);comms::detection_open(&mut w);assert_eq!(bytes(&w),terminal,"terminal opening is fully idempotent, including events");
    assert_eq!(comm_events(&w).iter().filter(|e|e["rule"]=="6.3.10.warning_received").count(),1);
}

#[test]
fn p03_receiver_removed_after_queue_fails_entire_batch_once() {
    let (mut w,cid,_)=fixture();let sender=id(&w,"BLUE_SENDER");let a=id(&w,"BLUE_RX1");let b=id(&w,"BLUE_RX2");
    comms::apply(&mut w,Side::Blue,sender,&action(cid,vec![a,b])).unwrap();
    // Explicit referee lifetime boundary after real report/queue, not a natural kill.
    w.entities.remove(&b);let physical_before=physical(&w);
    comms::detection_open(&mut w);let m=only_message(&w,sender);assert_eq!(m["state"],"failed");assert_eq!(m["terminal_at"],w.tick);
    assert!(messages(&w,a).is_empty());assert_eq!(physical(&w),physical_before);
    assert!(comm_events(&w).iter().all(|e|e["rule"]!="6.3.10.warning_received"));
    let terminal=bytes(&w);comms::detection_open(&mut w);assert_eq!(bytes(&w),terminal);
}

#[test]
fn p04_missed_execution_tick_fails_once_without_delayed_retransmission() {
    let (mut w,cid,_)=fixture();let sender=id(&w,"BLUE_SENDER");let rx=id(&w,"BLUE_RX1");
    comms::apply(&mut w,Side::Blue,sender,&action(cid,vec![rx])).unwrap();let queued_at=w.tick;
    // Deliberately omit the public Detection-open in this domain caller, then
    // advance one REAL native tick. This is missed-phase isolation, not normal plotting.
    harpoon_core::sim::tick(&mut w,&[]);assert_eq!(w.tick,queued_at+1);
    comms::detection_open(&mut w);let m=only_message(&w,sender);
    assert_eq!(m["state"],"failed");assert_eq!(m["failure"],"execution_window_closed");assert_eq!(m["queued_at"],queued_at);
    assert!(messages(&w,rx).is_empty());let terminal=bytes(&w);comms::detection_open(&mut w);assert_eq!(bytes(&w),terminal);
}

#[test]
fn p05_changed_report_source_configuration_rejects_purely() {
    let (mut w,cid,_)=fixture();let sender=id(&w,"BLUE_SENDER");let rx=id(&w,"BLUE_RX1");
    // Explicit author source-contract invalidation, no report/die/target setter.
    w.rules.detection.as_mut().unwrap().config.environment.as_mut().unwrap().source.push_str("; changed source revision");
    let before=bytes(&w);let result=comms::validate_action(&w,Side::Blue,sender,&action(cid,vec![rx]));
    assert!(result.is_err());assert_eq!(bytes(&w),before);
    assert!(!result.unwrap_err().contains("RED_SENDER"),"source error is generic, no truth identity");
    assert!(comms::apply(&mut w,Side::Blue,sender,&action(cid,vec![rx])).is_err());assert_eq!(bytes(&w),before);
}

#[test]
fn p06_enemy_truth_perturbations_do_not_change_cached_warning() {
    let (original,cid,_)=fixture();let sender=id(&original,"BLUE_SENDER");let target=id(&original,"RED_SENDER");let rx=id(&original,"BLUE_RX1");
    let act=action(cid,vec![rx]);let original_choices=view(&original,sender)["reports"].clone();
    let mut control=original.clone();comms::apply(&mut control,Side::Blue,sender,&act).unwrap();comms::detection_open(&mut control);
    let expected=only_message(&control,sender);
    for perturbation in 0..5 {
        let mut w=original.clone();
        // Explicit referee noninterference fixtures, NOT natural movement/damage.
        match perturbation {
            0=>w.entity_mut(target).unwrap().pos=Vec2::new(999.,-999.),
            1=>w.entity_mut(target).unwrap().kind=PlatformKind::Aircraft,
            2=>w.entity_mut(target).unwrap().hp=0.,
            3=>w.entity_mut(target).unwrap().alive=false,
            _=>{w.entities.remove(&target);}
        }
        assert_eq!(view(&w,sender)["reports"],original_choices);
        let before=bytes(&w);assert!(comms::validate_action(&w,Side::Blue,sender,&act).is_ok());assert_eq!(bytes(&w),before);
        comms::apply(&mut w,Side::Blue,sender,&act).unwrap();comms::detection_open(&mut w);
        assert_eq!(only_message(&w,sender),expected,"enemy truth perturbation {perturbation}");
        assert_eq!(only_message(&w,rx)["report"],expected["report"]);
    }
}

#[test]
fn p07_red_sending_cannot_advance_blue_local_message_sequence() {
    let (w,bcid,rcid)=fixture();let blue=id(&w,"BLUE_SENDER");let red=id(&w,"RED_SENDER");let brx=id(&w,"BLUE_RX1");let rrx=id(&w,"RED_RX");
    let mut control=w.clone();comms::apply(&mut control,Side::Blue,blue,&action(bcid,vec![brx])).unwrap();comms::detection_open(&mut control);
    let mut interleaved=w;comms::apply(&mut interleaved,Side::Red,red,&action(rcid,vec![rrx])).unwrap();
    comms::apply(&mut interleaved,Side::Blue,blue,&action(bcid,vec![brx])).unwrap();comms::detection_open(&mut interleaved);
    assert_eq!(only_message(&interleaved,red)["id"],1);assert_eq!(only_message(&interleaved,blue)["id"],1);
    assert_eq!(messages(&interleaved,blue),messages(&control,blue));
    let red_received=only_message(&interleaved,rrx);assert_eq!(red_received["sender"],json!(red));
    for e in interleaved.events_for(Side::Blue,0) {
        if let EventKind::RulesEvent{domain,unit,..}=&e.kind {if domain=="communications2026" {assert!(*unit==Some(blue)||*unit==Some(brx));}}
    }
}

#[test]
fn p08_historical_source_label_stale_or_rebound_never_becomes_live_track() {
    let (mut w,cid,_)=fixture();let sender=id(&w,"BLUE_SENDER");let rx=id(&w,"BLUE_RX1");
    comms::apply(&mut w,Side::Blue,sender,&action(cid,vec![rx])).unwrap();comms::detection_open(&mut w);
    let report=only_message(&w,sender)["report"].clone();assert_eq!(only_message(&w,sender)["contact_current"],true);
    for rebind in [false,true] {
        let mut altered=w.clone();
        // Explicit own-picture lifetime/group-representative boundary, no claim
        // that this mutation was produced by an actual native grouping event.
        let c=altered.sides[0].contacts.get_mut(&cid).unwrap();if rebind {c.about=rx;} else {c.stale=true;}
        let before=bytes(&altered);let m=only_message(&altered,sender);assert_eq!(bytes(&altered),before);
        assert_eq!(m["contact_current"],false);assert_eq!(m["contact_is_source_label"],true);assert_eq!(m["report_is_live_track"],false);assert_eq!(m["report"],report);
        assert_eq!(only_message(&altered,rx)["report"],report);
    }
}

#[test]
fn p09_public_api_over_and_exercise_lock_reject_without_mutation() {
    let (w,cid,_)=fixture();let sender=id(&w,"BLUE_SENDER");let rx=id(&w,"BLUE_RX1");
    for exercise_lock in [false,true] {
        let mut blocked=w.clone();
        if exercise_lock {
            // Explicit post-capture author command-lock boundary, not a claimed
            // complete objective/alert exercise run or a classification setter.
            blocked.context.island_exercise=Some(serde_json::from_value(json!({"locked_side":"Blue","briefing_side":"Red",
                "briefing_targets":[],"protected_targets":[],"counter_targets":[],"required_carrier_points":0.,"required_infrastructure_points":0.,"state":null})).unwrap());
            assert!(harpoon_core::exercise::locked(&blocked,Side::Blue));
        } else {blocked.over=true;}
        let before=bytes(&blocked);let act=action(cid,vec![rx]);assert!(comms::validate_action(&blocked,Side::Blue,sender,&act).is_err());assert_eq!(bytes(&blocked),before);
        assert!(comms::apply(&mut blocked,Side::Blue,sender,&act).is_err());assert_eq!(bytes(&blocked),before);
    }
}

#[test]
fn p10_no_fit_default_configuration_still_caches_real_native_reports() {
    let (mut w,cid,_)=fixture_with(None);let sender=id(&w,"BLUE_SENDER");let rx=id(&w,"BLUE_RX1");
    let choice=view(&w,sender);assert!(choice["fits"].as_array().unwrap().is_empty());
    assert!(choice["reports"].as_array().unwrap().iter().any(|r|r["contact"]==json!(cid)&&r["sensor"]==SENSOR));
    let before=bytes(&w);assert!(comms::validate_action(&w,Side::Blue,sender,&action(cid,vec![rx])).is_err());assert_eq!(bytes(&w),before);
    assert!(comms::apply(&mut w,Side::Blue,sender,&action(cid,vec![rx])).is_err());assert_eq!(bytes(&w),before);
}

#[test]
fn p11_queued_and_terminal_world_restore_is_exact_and_no_second_delivery() {
    let (mut w,cid,_)=fixture();let sender=id(&w,"BLUE_SENDER");let rx=id(&w,"BLUE_RX1");
    comms::apply(&mut w,Side::Blue,sender,&action(cid,vec![rx])).unwrap();
    let queued=bytes(&w);let mut restored:World=serde_json::from_slice(&queued).unwrap();restored.rules.validate_tables().unwrap();assert_eq!(bytes(&restored),queued);
    comms::detection_open(&mut w);comms::detection_open(&mut restored);assert_eq!(bytes(&restored),bytes(&w));
    let terminal=bytes(&w);let mut again:World=serde_json::from_slice(&terminal).unwrap();again.rules.validate_tables().unwrap();comms::detection_open(&mut again);assert_eq!(bytes(&again),terminal);
    assert_eq!(comm_events(&again).iter().filter(|e|e["rule"]=="6.3.10.warning_received").count(),1);
}

#[test]
fn p12_actual_tick179_and180_use_execution_tick_tactical_windows() {
    let (mut w,_,_)=fixture();let sender=id(&w,"BLUE_SENDER");let target=id(&w,"RED_SENDER");let rx=id(&w,"BLUE_RX1");
    for at in [179,180] {
        while w.tick<at {assert!(!w.over);harpoon_core::sim::tick(&mut w,&[]);}
        let cid=w.sides[0].contacts.values().find(|c|!c.stale&&c.about==target).unwrap().id;
        // Direct public phase-domain call at a native-reached real tick; the
        // separate Command suite proves its actual scheduler placement.
        comms::apply(&mut w,Side::Blue,sender,&action(cid,vec![rx])).unwrap();comms::detection_open(&mut w);
        let m=messages(&w,sender).into_iter().find(|m|m["queued_at"]==at).unwrap();
        assert_eq!(m["terminal_at"],at);assert_eq!(m["state"],"delivered");assert_eq!(m["tactical_turn"],at/180);
        let stable=bytes(&w);comms::detection_open(&mut w);assert_eq!(bytes(&w),stable);
    }
    assert_eq!(messages(&w,sender).len(),2);
    assert_eq!(messages(&w,sender).iter().map(|m|m["id"].as_u64().unwrap()).collect::<Vec<_>>(),vec![1,2]);
    // Fire control still holds only actual native scanner records; the warning
    // path never gives the distant receiver a native observation.
    let tracks=&w.rules.aaw.as_ref().unwrap().firecontrol.tracks;
    assert!(!tracks.values().any(|t|t.observer==rx));
}

#[test]
fn p13_already_manual_sender_keeps_its_previously_adjudicated_delayed_navigation() {
    let (mut w,cid,_)=fixture();let sender=id(&w,"BLUE_SENDER");let rx=id(&w,"BLUE_RX1");
    assert_eq!(w.entity(sender).unwrap().ext.get("control.manual"),Some(&1.));
    // Explicit admitted delayed-order boundary, not a naturally generated
    // Bridge critical: this player already owns both ship and pending order.
    w.rules.delayed_orders.push(harpoon_core::rules::damage_runtime::DelayedOrder {
        due:w.tick+360,side:Side::Blue,
        command:Command::SetCourse{unit:sender,course_deg:135.},
    });
    let before=serde_json::to_value(&w.rules.delayed_orders).unwrap();
    let cmd:Command=serde_json::from_value(json!({"cmd":"communications","unit":sender,
        "retain_manual":true,"action":{"type":"send_warning","contact":cid,
            "sensor":SENSOR,"fit":FIT,"receivers":[rx]}})).unwrap();
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);
    assert_eq!(only_message(&w,sender)["state"],"delivered");
    assert_eq!(serde_json::to_value(&w.rules.delayed_orders).unwrap(),before,
        "an already-manual sender's warning is not a fresh ownership transfer and must preserve its pending player navigation");
}

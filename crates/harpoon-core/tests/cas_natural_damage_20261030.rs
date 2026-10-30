//! ZCODE-EXPANDED-20261030: the printed row-94 CaS communications room
//! damage produced by a NATURAL ordinary weapon hit, then consumed by the
//! real TDL reject / repair / recovery public API.
//!
//! Personally re-viewed source (reusing the 2026-10-29 batch pages in
//! artifacts/rules-book-wide-20261026/zcode-parallel-20261028/expanded-atoms/
//! source-pages/, re-opened 2026-10-30):
//! - Expanded manual PDF14 (expanded-p14.png), printed row 94 «通信»: one of
//!   the ship's radio rooms is hit; roll D6: 1-2 long-range radio (no
//!   communication beyond the horizon), 3-4 short-range radio (no radio with
//!   other ships of the same formation), 5-6 aircraft radio (no
//!   communication with aircraft). PDF15 row 94 is the same row for the
//!   minor-combatant adjusted column.
//! - English2026 PDF135 / printed 14-9 §14.5 "Communications"
//!   (english-p135.png): identical three groups and scopes.
//!
//! Natural production chain (all steps are the real engine path; this test
//! never writes the expanded disabled set, never calls component_critical,
//! never injects HP/contact/dice/projectile positions):
//! 1. Blue shooter issues the real surface-gunnery Fire (root's sourced
//!    8.3 executor, weapon cn_pj45a "PJ-45A 130mm/70 Gun"). Caliber 130 mm
//!    passes the printed light-weapon boundary in damage_runtime::critical
//!    (communications is not in the ≤65 mm allowed set, damage_runtime.rs
//!    light-weapon gate), so the row can be reached at all.
//! 2. The shared damage adjudication rolls the 14.1.2 damage-ratio
//!    criticals; each rolls the real D100 "critical_type" die.
//! 3. With the optional CaS pre-1955 columns enabled and the hit hull
//!    declared pre-1955 (see the rule_data boundary below), the expanded
//!    cas_minor column resolves D100=94 to the printed "communications"
//!    effect (data/rules/expanded-damage-2026.json cas_major/cas_minor,
//!    column_pages 14/15, row through=94).
//! 4. expanded_damage2026::resolve_extra "communications" rolls the real D6
//!    group die (1-2 long_range, 3-4 short_range, 5-6 aircraft) and
//!    component_critical disables one physically declared room of that
//!    group; the disabled set, the repair ledger entry and the private
//!    "14.5.component_disabled" trace are all written by the engine.
//! 5. The real typed communications admission
//!    (communications2026::validate_action over the fitted CaS group
//!    queries) rejects the send; the public restore_component repairs the
//!    printed group and the same link then delivers.
//!
//! Boundary of the declared laboratory inputs: the static damage platform
//! tables carry no pre-1955 hull, so the hit hull is a new class whose
//! rule_data (year/role/size) and hp are explicit scenario_assumption
//! declarations for this rules-consumer experiment. It is NOT a claim about
//! any real ship's service year, and it is not an annex certification.
//! Radio rooms, link fits, qualifications (antenna heights / formations)
//! follow the same declared-input convention as
//! cas_radio_qualification_20261029.rs, whose ten adjudicated-set cases stay
//! untouched above this file's natural chain.
use harpoon_core::rules::communications2026 as c;
use harpoon_core::rules::expanded_damage2026::cas_communications::communications_group_available;
use harpoon_core::rules::expanded_damage2026::restore_component;
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};

const SENDER_CLASS: &str = "cas_natural_lab_hull";
const RECEIVER_CLASS: &str = "cas_natural_lab_receiver";
const TARGET_CLASS: &str = "cas_natural_lab_target";
const SONAR: &str = "cas_natural_sonar";
const GUN: &str = "cn_pj45a";
const PROTOCOL: &str = "cas-natural-20261030";
const FIRE_TICK: i64 = 181;
const SEED_LIMIT: u64 = 1_000_000;
const WORLD_LIMIT: usize = 32;
/// horizon_nm(2 m, 2 m) = 1.23·(√6.5617+√6.5617) ≈ 6.30 nm; 30 nm is beyond.
const SHIP_HEIGHT_QUAL_M: f64 = 2.0;
/// Declared hull hit points: with the sourced 25 DP gun hit the printed
/// damage-ratio table yields at least one ordinary critical without making
/// the hull's survival implausible. scenario_assumption, not a real hull.
const LAB_HP: f64 = 90.0;

struct Cfg {
    label: &'static str,
    group: &'static str,
    room: &'static str,
    die_span: [u8; 2],
    transport: Value,
    tx_qual: Option<Value>,
    rx_qual: Option<Value>,
    helo: bool,
    sep_nm: f64,
    peer: &'static str,
}

fn long_cfg() -> Cfg {
    Cfg {
        label: "long_range",
        group: "long_range",
        room: "long_radio",
        die_span: [1, 2],
        transport: json!({"type":"hf","max_range_nm":100.0}),
        tx_qual: Some(height_qual()),
        rx_qual: Some(height_qual()),
        helo: false,
        sep_nm: 30.0,
        peer: "RECEIVER",
    }
}
fn short_cfg() -> Cfg {
    Cfg {
        label: "short_range",
        group: "short_range",
        room: "short_radio",
        die_span: [3, 4],
        transport: json!({"type":"author_directed"}),
        tx_qual: Some(formation_qual("sag-natural")),
        rx_qual: Some(formation_qual("sag-natural")),
        helo: false,
        sep_nm: 2.0,
        peer: "RECEIVER",
    }
}
fn aircraft_cfg() -> Cfg {
    Cfg {
        label: "aircraft",
        group: "aircraft",
        room: "air_radio",
        die_span: [5, 6],
        transport: json!({"type":"author_directed"}),
        tx_qual: None,
        rx_qual: None,
        helo: true,
        sep_nm: 20.0,
        peer: "HELO",
    }
}

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn run(w: &mut World, to: i64) -> bool {
    while w.tick < to {
        if w.over {
            return false;
        }
        harpoon_core::sim::tick(w, &[]);
    }
    true
}
fn database() -> Database {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut sonar = db.sensors.values().next().unwrap().clone();
    sonar.id = SONAR.into();
    sonar.name = "scenario_assumption: declared laboratory passive sensor".into();
    sonar.kind = harpoon_core::db::SensorKind::SonarPassive;
    sonar.air_range_nm = 0.;
    sonar.surface_range_nm = 0.;
    sonar.missile_range_nm = 0.;
    sonar.sub_range_nm = 4.;
    db.sensors.insert(SONAR.into(), sonar);
    let shooter = db.platforms.get_mut("cn_052d").unwrap();
    shooter.sensors = vec!["visual".into()];
    shooter.rule_data=Some(harpoon_core::rules::damage_runtime::PlatformDamage{
        source:"scenario_assumption: C unarmored laboratory firing hull, not a Type052D source certification".into(),
        size:"C".into(),role:"surface".into(),belt_cm:0.,deck_cm:0.,..Default::default()});
    let mut sender = db.platforms["us_burke_2a"].clone();
    sender.id = SENDER_CLASS.into();
    sender.name = "scenario_assumption: pre-1955 rules-consumer laboratory hull; no claim about any real ship's service year or annex certification".into();
    sender.hp = LAB_HP;
    sender.sensors = vec![SONAR.into()];
    sender.weapons = Vec::new();
    sender.rule_data = Some(harpoon_core::rules::damage_runtime::PlatformDamage {
        source: "scenario_assumption: declared laboratory rule inputs (year/role/size) exercising the printed pre-1955 CaS columns; not a real hull row".into(),
        size: "C".into(),
        role: "surface".into(),
        year: 1944,
        belt_cm: 0.,
        deck_cm: 0.,
        ..Default::default()
    });
    db.platforms.insert(sender.id.clone(), sender);
    let mut rx = db.platforms["us_burke_2a"].clone();
    rx.id = RECEIVER_CLASS.into();
    rx.name = "scenario_assumption: original-body laboratory receiver".into();
    rx.sensors = Vec::new();
    rx.weapons = Vec::new();
    db.platforms.insert(rx.id.clone(), rx);
    let mut target = db.platforms["cn_093"].clone();
    target.id = TARGET_CLASS.into();
    target.sensors.clear();
    db.platforms.insert(target.id.clone(), target);
    db.validate().unwrap();
    db
}
fn passive_sonar() -> Value {
    json!({"type":"sonar","source":"scenario_assumption: Gen5 Bow LMF experimental sonar","generation":5,"passive_range_nm":4.,"active_range_nm":null,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false})
}
fn lookout() -> Value {
    json!({"type":"visual","source":"scenario_assumption: native lookout"})
}
fn height_qual() -> Value {
    json!({"source":"scenario_assumption: declared feed height for the printed radio horizon; no real mast data","antenna_height_m":SHIP_HEIGHT_QUAL_M})
}
fn formation_qual(name: &str) -> Value {
    json!({"source":"scenario_assumption: declared formation roster membership; not side or peer membership","formation":name})
}
fn base_fit(peers: Vec<&str>, transport: Value) -> Value {
    json!({"source":"scenario_assumption: declared laboratory link; no real hardware or Annex claim",
        "protocol":PROTOCOL,"send":true,"receive":true,"operational":true,
        "peers":peers,"warning_kind":"near_real_time","automatic_warning":false,
        "targeting_system":false,"transport":transport})
}
fn fit_with(base: Value, qualification: &Option<Value>) -> Value {
    let mut f = base;
    if let Some(q) = qualification {
        f["radio_qualification"] = q.clone();
    }
    f
}
fn communications_context(cfg: &Cfg) -> Value {
    json!({"source":"scenario_assumption: declared row-94 laboratory links; no real hardware or Annex claim","units":{
        "SENDER":{"source":"scenario_assumption: laboratory sender link","fits":{"lab":fit_with(base_fit(vec![cfg.peer], cfg.transport.clone()), &cfg.tx_qual)}},
        (cfg.peer):{"source":"scenario_assumption: laboratory receiver link","fits":{"lab":fit_with(base_fit(vec![], cfg.transport.clone()), &cfg.rx_qual)}}}})
}
fn expanded_context() -> Value {
    json!({"source":"scenario_assumption: one declared radio room per printed row-94 group; no unprinted redundancy",
        "optional":{"cas_pre1955":true},
        "units":{"SENDER":{"source":"scenario_assumption: author radio rooms","components":{
            "long_radio":{"source":"scenario_assumption: declared long-range radio room","kind":"communications","cas_group":"long_range"},
            "short_radio":{"source":"scenario_assumption: declared short-range radio room","kind":"communications","cas_group":"short_range"},
            "air_radio":{"source":"scenario_assumption: declared aircraft radio room","kind":"communications","cas_group":"aircraft"}}}}})
}
fn units(cfg: &Cfg) -> Vec<Value> {
    let mut u = vec![
        json!({"side":"Blue","class":"cn_052d","name":"SHOOTER","pos":[0,0],"speed_kts":0,"fire_on":false}),
        json!({"side":"Red","class":SENDER_CLASS,"name":"SENDER","pos":[0,3],"course_deg":90,"speed_kts":0,"fire_on":false}),
        json!({"side":"Blue","class":TARGET_CLASS,"name":"TARGET","pos":[1.5,3],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}),
    ];
    let endpoint = if cfg.helo {
        json!({"side":"Red","class":"au_mh60r","name":"HELO","pos":[0,3.0-cfg.sep_nm],"course_deg":90,"speed_kts":20,"altitude_m":300,"fire_on":false})
    } else {
        json!({"side":"Red","class":RECEIVER_CLASS,"name":"RECEIVER","pos":[0,3.0-cfg.sep_nm],"course_deg":90,"speed_kts":0,"fire_on":false})
    };
    u.push(endpoint);
    u
}
fn build_world(db: Database, cfg: &Cfg, seed: u64) -> World {
    let bindings = [
        (format!("{SENDER_CLASS}:{SONAR}"), passive_sonar()),
        ("cn_052d:visual".to_string(), lookout()),
    ]
    .into_iter()
    .collect::<std::collections::BTreeMap<_, _>>();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"cas-natural-damage-20261030",
        "name":"scenario_assumption: natural row-94 CaS room damage laboratory",
        "duration_s":10000,"seed":seed,
        "weather":{"visibility_nm":20,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "surface_gunnery_2026":{"source":"scenario_assumption: qualified test service, no claim of actual director fitting","units":{"SHOOTER":{"source":"scenario_assumption: explicit two-barrel test mount and clear conditions","weapon":GUN,"standard":"gs5","barrels":2,"arcs":[[0,360]],"under_fire":true,"local_control":false,"radar_gfc":false,"reaction_only":false,"terrain_clear":true,"other_conditions_clear":true,"ammunition_per_attack":2}}},
            "communications_2026":communications_context(cfg),
            "expanded_damage_2026":expanded_context(),
            "detection_2026":{"bindings":bindings,"environment":environment(),
                "targets":{
                    "cn_052d":signature_profile(2,json!(null)),
                    (SENDER_CLASS):signature_profile(2,json!(null)),
                    (TARGET_CLASS):signature_profile(2,json!(2))}}},
        "units":units(cfg)}))
    .unwrap();
    harpoon_core::load_scenario(sc, db).expect("valid natural-damage laboratory must load")
}
fn environment() -> Value {
    json!({"source":"scenario_assumption: explicit open-water experiment","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null})
}
fn signature_profile(size: i64, noise: Value) -> Value {
    json!({"source":"scenario_assumption: controlled laboratory signature","size":size,"noise":noise,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2})
}
fn surface_rows(w: &World) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "8.3.source_resolution" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn has_report(w: &World, sender: EntityId, contact: ContactId) -> bool {
    c::own_view(w, sender).is_some_and(|view| {
        view["reports"].as_array().is_some_and(|reports| {
            reports.iter().any(|r| {
                r["sensor"] == json!(SONAR)
                    && r["contact"] == serde_json::to_value(contact).unwrap()
            })
        })
    })
}
/// Predicate over the real engine products after the fire tick: the printed
/// group was disabled by a natural critical (D100 94 + in-span group D6 +
/// physical-room index die), not by this test. Returns false for seeds where
/// the natural chain did not land.
fn row94_id(w: &World, sender: EntityId) -> Option<String> {
    w.events.iter().find_map(|e| match &e.kind {
        EventKind::RulesEvent {
            unit: Some(id),
            rule,
            detail,
            ..
        } if *id == sender && rule == "damage_roll" => {
            let v: Value = serde_json::from_str(detail).ok()?;
            (v["purpose"] == "critical_type" && v["roll"] == 94)
                .then(|| v["hit"].as_str().map(str::to_owned))
                .flatten()
        }
        _ => None,
    })
}
// Pure replay of the first ordinary critical's deterministic dice, not a
// changed die or injected result. Only matching seeds instantiate a World.
fn seed_candidate(seed: u64, cfg: &Cfg) -> bool {
    let tick = FIRE_TICK + 1;
    let identity = format!("8.3:{tick}:1:{GUN}:2:ratio:0");
    let mut r =
        harpoon_core::Rng::new(seed).derive(harpoon_core::Rng::hash_tag(tick, 2, &identity));
    let kind = 1 + (r.next_f64() * 100.) as u8;
    let group = 1 + (r.next_f64() * 6.) as u8;
    if kind != 94 || !(cfg.die_span[0]..=cfg.die_span[1]).contains(&group) {return false;}
    harpoon_core::rules::detection2026::trial(&mut Default::default(),seed,FIRE_TICK,"5.8.visual:1:visual",180).0<=5
}
fn naturally_disabled(w: &World, sender: EntityId, cfg: &Cfg) -> bool {
    let Some(dice) = w.rules.expanded_dice.get(&sender) else {
        return false;
    };
    let Some(critical_id) = row94_id(w, sender) else {
        return false;
    };
    let group_rolls = dice
        .iter()
        .filter(|d| {
            d["purpose"] == json!("cas_communications")
                && d["identity"]==json!(critical_id)
                && matches!(d["roll"].as_u64(), Some(r) if (cfg.die_span[0] as u64..=cfg.die_span[1] as u64).contains(&r))
        })
        .count();
    if group_rolls == 0 {
        return false;
    }
    if !dice.iter().any(|d| {
        d["purpose"] == json!("physical_component")
            && d["roll"] == json!(1)
            && d["identity"] == json!(critical_id)
    }) {
        return false;
    }
    let Some(state) = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| s.units.get(&sender))
    else {
        return false;
    };
    if !state.disabled.contains(cfg.room) {
        return false;
    }
    if communications_group_available(w, sender, cfg.group) != Ok(false) {
        return false;
    }
    for g in ["long_range", "short_range", "aircraft"] {
        if g != cfg.group && communications_group_available(w, sender, g) != Ok(true) {
            return false;
        }
    }
    w.events
        .iter()
        .any(|e| matches!(&e.kind, EventKind::CriticalHit { unit, .. } if *unit == sender))
}
/// Seed-search until a real native gun hit naturally disables the printed
/// group. No damage input is ever injected; a missed seed is skipped and an
/// exhausted search is a fixture failure, never an accepted rule red.
fn natural_case(cfg: &Cfg) -> (World, EntityId, EntityId, ContactId, u64) {
    let db = database();
    let mut attempts = 0;
    let mut skipped=std::collections::BTreeMap::<String,usize>::new();
    for seed in (1..=SEED_LIMIT).filter(|seed| seed_candidate(*seed, cfg)) {
        attempts += 1;
        if attempts > WORLD_LIMIT {
            break;
        }
        let mut w = build_world(db.clone(), cfg, seed);
        assert_eq!(id(&w, "SHOOTER"), EntityId(1));
        assert_eq!(id(&w, "SENDER"), EntityId(2));
        if !run(&mut w,FIRE_TICK) {*skipped.entry(format!("ended tick{} over{}",w.tick,w.over)).or_default()+=1;continue;}
        let sender = id(&w, "SENDER");
        let target = id(&w, "TARGET");
        let Some(fire_contact) = w.sides[0]
            .contacts
            .values()
            .find(|k| !k.stale && k.about == sender)
            .map(|k| k.id)
        else {*skipped.entry("no shooter visual contact".into()).or_default()+=1;continue;};
        let Some(sub_contact) = w.sides[1]
            .contacts
            .values()
            .find(|k| !k.stale && k.about == target)
            .map(|k| k.id)
        else {*skipped.entry("no sender passive contact".into()).or_default()+=1;continue;};
        if !has_report(&w, sender, sub_contact) {*skipped.entry("no native sender report".into()).or_default()+=1;continue;}
        let fire = Command::Fire {
            unit: id(&w, "SHOOTER"),
            contact: fire_contact,
            weapon: GUN.into(),
            rounds: 1,
        };
        if let Err(reason)=fire.validate_source(&w,Side::Blue) {*skipped.entry(reason).or_default()+=1;continue;}
        harpoon_core::sim::tick(&mut w, &[(Side::Blue, fire)]);
        if !surface_rows(&w).iter().any(|r|r["hit"]==json!(true)) {*skipped.entry(format!("no hit {:?}",surface_rows(&w))).or_default()+=1;continue;}
        if w.entity(sender)
            .is_none_or(|e| !e.alive || e.hp <= 0. || e.hp >= LAB_HP)
        {*skipped.entry(format!("no surviving damage hp={:?}",w.entity(sender).map(|e|e.hp))).or_default()+=1;continue;}
        if !naturally_disabled(&w, sender, cfg) {
            let rolls:Vec<_>=w.events.iter().filter_map(|e|match &e.kind {EventKind::RulesEvent{rule,unit:Some(id),detail,..} if *id==sender && rule=="damage_roll"=>Some(detail.clone()),_=>None}).take(4).collect();
            *skipped.entry(format!("no selected room: rolls={rolls:?}; expanded={:?}",w.rules.expanded_dice.get(&sender))).or_default()+=1;continue;
        }
        eprintln!("cas_natural_damage_20261030: {} seed={seed}", cfg.label);
        let peer = id(&w, cfg.peer);
        return (w, sender, peer, sub_contact, seed);
    }
    panic!(
        "natural row-94 {} production fixture unavailable after {SEED_LIMIT} seeds: fixture failure, never an accepted rule red",
        format!("{} world_attempts={attempts} skipped={skipped:?}",cfg.label)
    );
}
fn warning(contact: ContactId, receivers: Vec<EntityId>) -> Value {
    json!({"type":"send_warning","contact":contact,"sensor":SONAR,"fit":"lab","receivers":receivers})
}
fn expect_reject(w: &World, actor: EntityId, action: Value, expect: &[&str]) -> String {
    let typed: c::Action = serde_json::from_value(action).unwrap();
    let before = serde_json::to_vec(w).unwrap();
    let err = c::validate_action(w, Side::Red, actor, &typed).unwrap_err();
    for fragment in expect {
        assert!(
            err.contains(fragment),
            "expected rejection mentioning {fragment:?}, got: {err}"
        );
    }
    assert_eq!(
        before,
        serde_json::to_vec(w).unwrap(),
        "read-only admission cannot mutate a naturally damaged room"
    );
    err
}
fn send(w: &mut World, actor: EntityId, action: Value) {
    let typed: c::Action = serde_json::from_value(action.clone()).unwrap();
    c::validate_action(w, Side::Red, actor, &typed)
        .expect("repaired link must pass the real admission");
    let cmd: Command =
        serde_json::from_value(json!({"cmd":"communications","unit":actor,"action":action}))
            .unwrap();
    harpoon_core::sim::tick(w, &[(Side::Red, cmd)]);
}
fn messages(w: &World, unit: EntityId) -> Vec<Value> {
    let side = w.entity(unit).unwrap().side;
    let v = serde_json::to_value(harpoon_core::observe(w, side, 0, 10000)).unwrap();
    v["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["id"] == json!(unit))
        .expect("own unit in its legitimate observation")["communications"]["messages"]
        .as_array()
        .expect("messages array")
        .clone()
}
fn delivered_count(w: &World, unit: EntityId) -> usize {
    messages(w, unit)
        .iter()
        .filter(|m| m["state"] == json!("delivered"))
        .count()
}
fn assert_natural_evidence(w: &World, sender: EntityId, cfg: &Cfg) {
    let dice = &w.rules.expanded_dice[&sender];
    assert!(
        row94_id(w, sender).is_some(),
        "the printed row must be reached by a real D100 critical_type roll"
    );
    assert!(
        dice.iter().any(|d| {
            d["purpose"] == json!("cas_communications")
                && matches!(d["roll"].as_u64(), Some(r) if (cfg.die_span[0] as u64..=cfg.die_span[1] as u64).contains(&r))
        }),
        "the group must come from the real printed D6 roll"
    );
    assert!(
        dice.iter()
            .any(|d| d["purpose"] == json!("physical_component") && d["roll"] == json!(1)),
        "the physical room selection must be the engine's own index die over the one declared room"
    );
    assert!(
        w.entity(sender).unwrap().hp < LAB_HP,
        "real ordinary weapon damage must precede the room critical"
    );
    assert!(
        surface_rows(w).iter().any(|r| r["hit"] == json!(true)),
        "the 8.3 surface gunnery resolution must record a genuine hit"
    );
}

#[test]
fn natural_hit_disables_long_range_room_rejects_beyond_horizon_tdl_and_restore_recovers() {
    let cfg = long_cfg();
    let (mut w, sender, receiver, contact, _seed) = natural_case(&cfg);
    assert_natural_evidence(&w, sender, &cfg);
    // Real admission rejects with the printed beyond-horizon group block.
    expect_reject(
        &w,
        sender,
        warning(contact, vec![receiver]),
        &["long_range", "radio group unavailable"],
    );
    // The real queued path delivers nothing while the room is down.
    let queued = messages(&w, sender).len();
    let cmd: Command = serde_json::from_value(json!({
        "cmd":"communications","unit":sender,
        "action":warning(contact, vec![receiver])}))
    .unwrap();
    harpoon_core::sim::tick(&mut w, &[(Side::Red, cmd)]);
    assert_eq!(
        messages(&w, sender).len(),
        queued,
        "the real command path must not leave a message behind the rejection"
    );
    // Repair through the public entry restores the printed group.
    restore_component(&mut w, sender, cfg.room).unwrap();
    assert_eq!(
        communications_group_available(&w, sender, cfg.group),
        Ok(true)
    );
    // Recovery: the same real link delivers again.
    send(&mut w, sender, warning(contact, vec![receiver]));
    assert_eq!(
        delivered_count(&w, receiver),
        1,
        "after the natural damage and public repair the beyond-horizon link delivers"
    );
}

#[test]
fn natural_hit_disables_short_range_room_rejects_same_formation_tdl_and_restore_recovers() {
    let cfg = short_cfg();
    let (mut w, sender, receiver, contact, _seed) = natural_case(&cfg);
    assert_natural_evidence(&w, sender, &cfg);
    expect_reject(
        &w,
        sender,
        warning(contact, vec![receiver]),
        &["short_range", "radio group unavailable"],
    );
    restore_component(&mut w, sender, cfg.room).unwrap();
    send(&mut w, sender, warning(contact, vec![receiver]));
    assert_eq!(
        delivered_count(&w, receiver),
        1,
        "repaired same-formation short-range group delivers"
    );
}

#[test]
fn natural_hit_disables_aircraft_room_rejects_ship_helo_tdl_and_restore_recovers() {
    let cfg = aircraft_cfg();
    let (mut w, sender, helo, contact, _seed) = natural_case(&cfg);
    assert_natural_evidence(&w, sender, &cfg);
    expect_reject(
        &w,
        sender,
        warning(contact, vec![helo]),
        &["aircraft", "radio group unavailable"],
    );
    restore_component(&mut w, sender, cfg.room).unwrap();
    send(&mut w, sender, warning(contact, vec![helo]));
    assert_eq!(
        delivered_count(&w, helo),
        1,
        "repaired aircraft-band group delivers to the real helicopter endpoint"
    );
}

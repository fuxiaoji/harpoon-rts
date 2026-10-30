//! Full9.4.7 native boundary candidates after the original3 source-red cases.
//! Native first report and real Declare; no Contact/FCS/ext/draw injection.
use harpoon_core::rules::{aircombat2026 as air, communications2026 as comm};
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
fn scenario(enabled: bool, friend: bool) -> Value {
    let fit = |peers: Vec<&str>| {
        json!({"source":"scenario_assumption: explicit HF voice experiment; no Annex radio inference",
        "protocol":"wide-gci-voice","send":true,"receive":true,"operational":true,"peers":peers,
        "automatic_warning":false,"transport":{"type":"hf","max_range_nm":20}})
    };
    let unit = |attempt| {
        json!({"source":"scenario_assumption: explicit single-seat/RWR/ES and passive-gun starting plan, not a real hardware audit",
        "single_seat_fighter":true,"rwr":false,"es":false,"attack_method":"passive_gun",
        "attempt_surprise":attempt,"out_of_sun":false})
    };
    let target = json!({"source":"scenario_assumption: controlled visual signature","size":5,"noise":null,
        "anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2});
    let mut v = json!({"id":"wide-air-surprise","name":"scenario_assumption: real native starting conditions",
      "seed":26,"duration_s":10000,"weather":{"visibility_nm":40,"jam":[0,0]},"context":{
       "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
       "aviation_2026":{"source":"scenario_assumption: source-mechanism load/fuel experiment",
          "loads":{"cn_j15":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"},
           "us_f35c":{"payload_kg":100,"tanks":[],"source":"scenario_assumption"}},"parking":{},
          "initial_fuel_nm":{"ATTACK":700,"DEFEND":700}},
       "aircombat_2026":{"source":"English2026 PDF103-104 §9.4.7; scenario_assumption: both sides agree optional conditions",
          "enabled":enabled,"starting_speed":true,"astern":true,"surprise":true,"units":{"ATTACK":unit(true),"DEFEND":unit(false)},
          "groups":{"A":{"source":"scenario_assumption: actual attacking formation roster","units":["ATTACK"]},
           "D":{"source":"scenario_assumption: actual defending formation roster","units":["DEFEND"]}}},
       "detection_2026":{"bindings":{"cn_j15:visual":{"type":"visual","source":"scenario_assumption: actually fitted forward observer"},
          "us_ford:visual":{"type":"visual","source":"scenario_assumption: actually fitted GCI observer"}},
        "targets":{"cn_j15":target,"us_f35c":target,"us_ford":{"source":"scenario_assumption: physical observer vessel", "size":0,
          "noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":0}},
        "environment":{"source":"scenario_assumption: clear daylight no obscuration","sea_state":0,"precipitation":0,"visibility_percent":100,
          "shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
      "units":[{"side":"Blue","class":"cn_j15","name":"ATTACK","pos":[-0.1,0],"course_deg":90,"speed_kts":400,"altitude_m":4000,"fire_on":false},
        {"side":"Red","class":"us_f35c","name":"DEFEND","pos":[0,0],"course_deg":90,"speed_kts":400,"altitude_m":4000,"fire_on":false}]});
    if friend {
        v["units"].as_array_mut().unwrap().push(json!({"side":"Red","class":"us_ford","name":"GCI","pos":[0,-0.05],"speed_kts":0,"fire_on":false}));
        v["context"]["communications_2026"] = json!({"source":"scenario_assumption: real compatible operational link",
            "units":{"GCI":{"source":"scenario_assumption: declared observer radio","fits":{"voice":fit(vec!["DEFEND"])}},
              "DEFEND":{"source":"scenario_assumption: declared receiving radio","fits":{"voice":fit(vec![])}}}});
    }
    v
}
fn load(v: Value) -> World {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let method = v["context"]["aircombat_2026"]["units"]["ATTACK"]["attack_method"]
        .as_str()
        .unwrap_or("");
    for class in ["cn_j15", "us_f35c", "us_ford", "us_mh60r"] {
        let old = db.platforms[class].clone();
        let radar = old
            .sensors
            .iter()
            .find(|key| {
                db.sensors
                    .get(*key)
                    .is_some_and(|d| d.kind == harpoon_core::db::SensorKind::Radar)
            })
            .cloned();
        let p = db.platforms.get_mut(class).unwrap();
        let missile_plan = class == "cn_j15" && matches!(method, "ir_missile" | "radar_weapon");
        if !missile_plan {
            p.weapons.clear();
        }
        p.sensors = if class == "us_f35c"
            && !v["context"]["detection_2026"]["bindings"]
                .as_object()
                .unwrap()
                .contains_key("us_f35c:visual")
            || class == "us_mh60r"
        {
            vec![]
        } else {
            vec!["visual".into()]
        };
        if class == "cn_j15" && matches!(method,"radar_gun_ranging"|"ir_missile") {
            p.sensors
                .push(radar.expect("source fixture requires a real installed aircraft radar"));
        }
    }
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(v).unwrap(), db).unwrap()
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn declare(w: &mut World) {
    let ir_plan=w.context.aircombat_2026.as_ref().and_then(|c|c.units.get("ATTACK"))
        .is_some_and(|f|f.attack_method==Some(air::starting::AttackMethod::IrMissile));
    let initial=if ir_plan {vec![(Side::Blue,Command::SetRadar{unit:id(w,"ATTACK"),active:false})]}else{vec![]};
    harpoon_core::sim::tick(w, &initial);
    let actor = id(w, "ATTACK");
    let defender = id(w, "DEFEND");
    let contact = w
        .find_contact(Side::Blue, defender)
        .expect("actual native observer must discover the defender");
    assert!(w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .firecontrol
        .tracks
        .values()
        .any(|t| t.observer == actor && t.target == defender && t.report.is_some()));
    let c = Command::AirCombat {
        unit: actor,
        action: air::Action::Declare { contact },
    };
    c.validate_source(w, Side::Blue).unwrap();
    harpoon_core::sim::tick(w, &[(Side::Blue, c)]);
    assert!(air::in_dogfight(w, actor));
}
fn episode(w: &World) -> Value {
    w.events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "9.4.7.starting" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .expect(
            "actual agreed optional declaration must produce a persisted source starting episode",
        )
}
use harpoon_core::Rng;
fn seeded(limit: u8, sun: bool) -> u64 {
    (0..100000)
        .find(|seed| {
            let r = Rng::new(*seed);
            let mut surprise = r.derive(Rng::hash_tag(2, 1, "9.4.7.surprise.1"));
            let mut position = r.derive(Rng::hash_tag(62, 1, "9.4.1.position.1.1"));
            let mut sunroll = r.derive(Rng::hash_tag(2, 1, "9.4.7.sun_position.1"));
            let survives = [62, 122].iter().all(|tick| {
                let mut d = r.derive(Rng::hash_tag(
                    *tick,
                    1,
                    &format!("9.4.2.gun.1.{}", if *tick == 62 { 1 } else { 2 }),
                ));
                d.next_f64() > 0.7
            });
            (surprise.next_f64() * 100.0).floor() as u8 + 1 <= limit
                && position.next_f64() > 0.8
                && (!sun || (sunroll.next_f64() * 100.0).floor() as u8 + 1 <= 30)
                && survives
        })
        .unwrap()
}
fn run(w: &mut World, to: i64) {
    while w.tick < to {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn position(w: &World, actor: EntityId, round: u64) -> Value {
    w.events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule,
                unit: Some(unit),
                detail,
                ..
            } if rule == "9.4.7.position_inputs" && *unit == actor => {
                let v: Value = serde_json::from_str(detail).unwrap();
                (v["round"] == round).then_some(v)
            }
            _ => None,
        })
        .unwrap()
}
fn optional(v: &mut Value) -> &mut Value {
    &mut v["context"]["aircombat_2026"]
}
fn formation(attackers: usize, defenders: usize, multi: bool) -> Value {
    let mut v = scenario(true, false);
    v["seed"] = json!(seeded(5, false));
    if multi {
        v["units"][1]["class"] = json!("us_mh60r");
        v["units"][1]["speed_kts"] = json!(100);
        v["units"][0]["altitude_m"] = json!(1000);
        v["units"][1]["altitude_m"] = json!(1000);
        v["context"]["aviation_2026"]["loads"]["us_mh60r"] =
            v["context"]["aviation_2026"]["loads"]["us_f35c"].clone();
        v["context"]["detection_2026"]["targets"]["us_mh60r"] =
            v["context"]["detection_2026"]["targets"]["us_f35c"].clone();
        v["context"]["aviation_2026"]["initial_fuel_nm"]["DEFEND"] = json!(200);
        optional(&mut v)["units"]["DEFEND"]["single_seat_fighter"] = json!(false);
    }
    for (count, index, prefix, group) in [(attackers, 0, "A", "A"), (defenders, 1, "D", "D")] {
        for n in 2..=count {
            let name = format!("{prefix}{n}");
            let mut u = v["units"][index].clone();
            u["name"] = json!(name);
            u["pos"][1] = json!(n as f64 * 0.001);
            v["units"].as_array_mut().unwrap().push(u);
            let template = if index == 0 { "ATTACK" } else { "DEFEND" };
            let mut fit = optional(&mut v)["units"][template].clone();
            fit["attempt_surprise"] = json!(false);
            optional(&mut v)["units"][&name] = fit;
            optional(&mut v)["groups"][group]["units"]
                .as_array_mut()
                .unwrap()
                .push(json!(name));
            v["context"]["aviation_2026"]["initial_fuel_nm"][&name] = if index == 1 && multi { json!(200) } else { json!(700) };
        }
    }
    v
}
#[test]
fn f01_first_free_round_has_actual_position_effect_and_expires_before_second_round() {
    let mut v = scenario(true, false);
    v["seed"] = json!(seeded(20, false));
    let mut w = load(v);
    declare(&mut w);
    let (a, d) = (id(&w, "ATTACK"), id(&w, "DEFEND"));
    assert_eq!(episode(&w)["surprised"], true);
    run(&mut w, 62);
    let p = position(&w, a, 1);
    assert_eq!(p["target_mr"], 2.5);
    assert_eq!(p["astern_maneuver_difference"], 1.0);
    let initial = episode(&w)["entries"][a.0.to_string()]["speed_kts"]
        .as_f64()
        .unwrap();
    assert_eq!(p["attacker_speed_kts"], initial);
    assert!(!air::maneuvering(&w, d));
    assert_eq!(air::target_maneuver(&w, d).unwrap(), 0.0);
    assert!(air::release_allowed(&w, d).is_err());
    assert!(!w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,unit:Some(x),detail,..} if rule=="9.4.1.position"&&*x==d&&serde_json::from_str::<Value>(detail).unwrap()["round"]==1)));
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    run(&mut w, 122);
    run(&mut restored, 122);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert!(air::maneuvering(&w, d));
    assert_eq!(air::target_maneuver(&w, d).unwrap(), 4.5);
    let second = position(&w, a, 2);
    assert_eq!(second["astern_maneuver_difference"], 0.0);
    assert_eq!(
        second["attacker_speed_kts"],
        harpoon_core::rules::aviation2026::entity_source_speed(
            &w,
            a,
            4000.,
            harpoon_core::rules::aviation2026::Throttle::Fmp
        )
        .unwrap()
    );
}
#[test]
fn f02_real_explicit_two_by_three_multicrew_formation_consumes_the_shifted_source_cell() {
    let mut w = load(formation(2, 3, true));
    declare(&mut w);
    let e = episode(&w);
    assert_eq!(
        w.rules
            .aircombat
            .as_ref()
            .unwrap()
            .fights
            .values()
            .next()
            .unwrap()
            .members
            .len(),
        5
    );
    assert_eq!(e["attacking_count"], 2);
    assert_eq!(e["defending_count"], 3);
    assert_eq!(e["chance_percent"].as_f64().unwrap(), 10.0);
    assert!(e["d100"].is_number());
    assert!(e["pending"].is_null());
    // Actual multicrew helicopter fit givesMR2 -> source floor0.5 in position.
    run(&mut w, 62);
    assert_eq!(position(&w, id(&w, "ATTACK"), 1)["target_mr"], 0.5);
}
#[test]
fn f03_exact_half_crew_mix_is_pending_instead_of_guessing_a_majority() {
    let mut v = formation(1, 2, false);
    optional(&mut v)["units"]["D2"]["single_seat_fighter"] = json!(false);
    let d2 = v["units"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|u| u["name"] == "D2")
        .unwrap();
    d2["class"] = json!("us_mh60r");
    d2["speed_kts"] = json!(100);
    v["context"]["aviation_2026"]["initial_fuel_nm"]["D2"] = json!(200);
    v["context"]["aviation_2026"]["loads"]["us_mh60r"] =
        v["context"]["aviation_2026"]["loads"]["us_f35c"].clone();
    v["context"]["detection_2026"]["targets"]["us_mh60r"] =
        v["context"]["detection_2026"]["targets"]["us_f35c"].clone();
    let mut w = load(v);
    declare(&mut w);
    let e = episode(&w);
    assert!(e["pending"].as_str().unwrap().contains("50:50"));
    assert!(e["d100"].is_null());
}
#[test]
fn f04_multicrew_nine_plus_cannot_silently_cap_an_unprinted_right_shift() {
    let mut w = load(formation(1, 9, true));
    declare(&mut w);
    let e = episode(&w);
    assert_eq!(e["defending_count"], 9);
    assert!(e["pending"].as_str().unwrap().contains("last printed"));
    assert!(e["d100"].is_null());
}
#[test]
fn f05_rwr_protects_the_actual_radar_gun_method_but_not_an_actual_ir_inventory_plan() {
    let mut radar = scenario(true, false);
    optional(&mut radar)["units"]["DEFEND"]["rwr"] = json!(true);
    optional(&mut radar)["units"]["ATTACK"]["attack_method"] = json!("radar_gun_ranging");
    let mut rw = load(radar);
    declare(&mut rw);
    assert_eq!(episode(&rw)["immunity"], "radar_warning_receiver");
    assert!(episode(&rw)["d100"].is_null());
    let mut ir = scenario(true, false);
    ir["seed"] = json!(seeded(20, false));
    optional(&mut ir)["units"]["DEFEND"]["rwr"] = json!(true);
    optional(&mut ir)["units"]["ATTACK"]["attack_method"] = json!("ir_missile");
    let mut iw = load(ir);
    declare(&mut iw);
    assert!(episode(&iw)["immunity"].is_null());
    assert!(episode(&iw)["pending"].is_null());
    assert_eq!(episode(&iw)["surprised"], true);
    run(&mut iw, 62);
    let (a, d) = (id(&iw, "ATTACK"), id(&iw, "DEFEND"));
    let contact = iw.find_contact(Side::Blue, d).unwrap();
    let original = iw
        .entity(a)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "cn_pl8b")
        .unwrap()
        .ammo;
    let wrong = Command::Fire {
        unit: a,
        contact,
        weapon: "cn_pl12".into(),
        rounds: 1,
    };
    let before = serde_json::to_value(&iw).unwrap();
    assert!(wrong.validate_source(&iw, Side::Blue).is_err());
    assert_eq!(serde_json::to_value(&iw).unwrap(), before);
    let fire = Command::Fire {
        unit: a,
        contact,
        weapon: "cn_pl8b".into(),
        rounds: 1,
    };
    fire.validate_source(&iw, Side::Blue).unwrap();
    harpoon_core::sim::tick(&mut iw, &[(Side::Blue, fire)]);
    assert_eq!(
        iw.entity(a)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "cn_pl8b")
            .unwrap()
            .ammo,
        original - 1
    );
    let packet = iw
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .aam
        .packets
        .values()
        .find(|p| p.shooter == a && p.weapon == "cn_pl8b")
        .unwrap();
    assert_eq!(packet.attack.mr, 0.0);
    assert!(!packet.attack.maneuvering);
}
#[test]
fn f06_a_friendly_side_contact_without_operational_transport_is_not_tracking_immunity() {
    let mut v = scenario(true, true);
    v["seed"] = json!(seeded(20, false));
    v["context"]["communications_2026"]["units"]["GCI"]["fits"]["voice"]["operational"] =
        json!(false);
    let mut w = load(v);
    declare(&mut w);
    assert!(episode(&w)["immunity"].is_null());
    assert_eq!(episode(&w)["chance_percent"].as_f64().unwrap(), 30.0);
    assert_eq!(episode(&w)["surprised"], true);
}
#[test]
fn f07_an_actual_defender_observer_report_blocks_undetected_surprise() {
    let mut v = scenario(true, false);
    v["context"]["detection_2026"]["bindings"]["us_f35c:visual"] =
        json!({"type":"visual","source":"scenario_assumption: actual defender lookout"});
    let mut w = load(v);
    declare(&mut w);
    assert_eq!(episode(&w)["immunity"], "detected_by_defender");
    assert!(episode(&w)["d100"].is_null());
}
fn sun_scene(policy: Value) -> Value {
    let mut v = scenario(true, false);
    v["seed"] = json!(seeded(20, true));
    v["units"][0]["pos"] = json!([0.1, 0]);
    v["units"][0]["course_deg"] = json!(270);
    v["units"][1]["course_deg"] = json!(270);
    optional(&mut v)["units"]["ATTACK"]["out_of_sun"] = json!(true);
    optional(&mut v)["sun"] = json!({"source":"scenario_assumption: declared morning and explicit dice adjudication","period":"morning","position_roll_policy":policy});
    v
}
#[test]
fn f08_real_east_morning_sun_uses_separate_explicit_preparation_roll_then_adds_thirty() {
    let mut w = load(sun_scene(json!("d100_percent")));
    declare(&mut w);
    let e = episode(&w);
    assert_eq!(e["sun"]["eligible"], true);
    assert_eq!(e["sun"]["positioned"], true);
    assert_eq!(e["sun"]["roll_sides"], 100);
    assert_eq!(e["chance_percent"].as_f64().unwrap(), 60.0);
    assert!(e["d100"].is_number());
    let mut missing = load(sun_scene(Value::Null));
    declare(&mut missing);
    assert!(episode(&missing)["pending"]
        .as_str()
        .unwrap()
        .contains("D10"));
    assert!(episode(&missing)["d100"].is_null());
}
#[test]
fn f09_west_in_morning_gets_no_sun_positioning_roll_or_bonus() {
    let mut v = sun_scene(json!("d100_percent"));
    v["units"][0]["pos"] = json!([-0.1, 0]);
    v["units"][0]["course_deg"] = json!(90);
    v["units"][1]["course_deg"] = json!(90);
    let mut w = load(v);
    declare(&mut w);
    let e = episode(&w);
    assert_eq!(e["sun"]["eligible"], false);
    assert!(e["sun"]["roll"].is_null());
    assert_eq!(e["chance_percent"].as_f64().unwrap(), 30.0);
}
#[test]
fn f10_real_public_loiter_state_uses_eighty_percent_of_altitude_cruise_only_for_first_round() {
    let mut v = scenario(true, false);
    optional(&mut v)["surprise"] = json!(false);
    let mut w = load(v);
    let a = id(&w, "ATTACK");
    let c = Command::Aviation {
        unit: a,
        action: harpoon_core::rules::aviation2026::Action::Throttle {
            setting: harpoon_core::rules::aviation2026::Throttle::Loiter,
        },
    };
    c.validate_source(&w, Side::Blue).unwrap();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, c)]);
    declare(&mut w);
    let e = episode(&w);
    assert_eq!(e["entries"][a.0.to_string()]["loiter"], true);
    let source = harpoon_core::rules::aviation2026::entity_source_speed(
        &w,
        a,
        4000.,
        harpoon_core::rules::aviation2026::Throttle::Cruise,
    )
    .unwrap()
        * 0.8;
    assert_eq!(e["entries"][a.0.to_string()]["starting_speed_kts"], source);
}
#[test]
fn f11_unknown_hardware_method_stays_pending_and_never_fills_a_source_probability() {
    let mut v = scenario(true, false);
    optional(&mut v)["units"]["ATTACK"]["attack_method"] = Value::Null;
    let mut w = load(v);
    declare(&mut w);
    let e = episode(&w);
    assert!(e["pending"].is_string());
    assert!(e["d100"].is_null());
    let own = air::own_view(&w, id(&w, "ATTACK")).unwrap();
    let text = own["starting_conditions"].to_string();
    for key in [
        "d100",
        "chance_percent",
        "defending_count",
        "RWR",
        "entries",
    ] {
        assert!(!text.contains(key));
    }
}

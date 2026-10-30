//! Full9.4.7 native boundary candidates after the original3 source-red cases.
//! Source-red candidate: actual native surprise declaration, real AAM launch and physical arrival after the first window. No state/die/clock injection.
use harpoon_core::rules::{aam2026 as aam, aircombat2026 as air};
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
        if class == "cn_j15" && (missile_plan || method == "radar_gun_ranging") {
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
    harpoon_core::sim::tick(w, &[]);
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

#[test]
fn aam_d01_when_fired_defense_declaration_survives_window_expiry_and_world_restore() {
    let mut v = scenario(true, false);
    v["seed"] = json!(seeded(20, false));
    v["context"]["aircombat_2026"]["units"]["ATTACK"]["attack_method"] = json!("ir_missile");
    let mut w = load(v);
    declare(&mut w);
    assert_eq!(episode(&w)["surprised"], true);
    let (a, d) = (id(&w, "ATTACK"), id(&w, "DEFEND"));
    let ends = air::own_view(&w, d).unwrap()["starting_conditions"]["first_window_ends_at"]
        .as_i64()
        .unwrap();
    run(&mut w, ends - 1);
    assert!(
        !air::maneuvering(&w, d),
        "natural launch declaration must still be within the first window"
    );
    let mount = w
        .entity(a)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "cn_pl8b" && m.ammo > 0)
        .unwrap();
    let stock = w.entity(a).unwrap().weapons[mount].ammo;
    // Commit the actual source producer after this tick's phases. Its genuine
    // physical projectile next advances at ends, beyond the declaration window.
    assert_eq!(aam::fire_at(&mut w, a, d, mount, 1).unwrap(), 1);
    assert_eq!(w.entity(a).unwrap().weapons[mount].ammo, stock - 1);
    let packet = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .aam
        .packets
        .values()
        .find(|p| p.shooter == a && p.weapon == "cn_pl8b")
        .unwrap()
        .clone();
    assert_eq!(packet.attack.mr, 0.);
    assert!(!packet.attack.maneuvering);
    assert!(packet
        .members
        .iter()
        .all(|id| w.entity(*id).is_some_and(|e| e.alive)));
    let expected_cm = harpoon_core::rules::aaw2026::countermeasure_modifier(
        &packet.attack.countermeasures,
        false,
    )
    .unwrap();
    let snapshot = serde_json::to_value(&w).unwrap();
    let mut restored: World = serde_json::from_value(snapshot.clone()).unwrap();
    assert_eq!(serde_json::to_value(&restored).unwrap(), snapshot);
    for state in [&mut w, &mut restored] {
        for _ in 0..6 {
            if state
                .rules
                .aaw
                .as_ref()
                .unwrap()
                .aam
                .outcomes
                .contains_key(&packet.identity)
            {
                break;
            }
            assert!(!state.over, "fixture ended before the real missile arrival");
            harpoon_core::sim::tick(state, &[]);
        }
        let outcome = state
            .rules
            .aaw
            .as_ref()
            .unwrap()
            .aam
            .outcomes
            .get(&packet.identity)
            .expect("actual projectile arrival must adjudicate the original packet");
        assert!(state.tick >= ends);
        assert_eq!(outcome.solution.modifiers["maneuver_rating"],0.,"PDF105 section9.5.1 declares MR0 when fired; window expiry cannot replace it with later maneuvering");
        assert_eq!(outcome.solution.modifiers["countermeasures"],expected_cm,"the same when-fired nonmaneuvering declaration still halves the eligible countermeasure modifier");
        assert!(
            state.events.iter().any(|e| e.tick >= ends
                && matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="9.5.packet_terminal")),
            "must witness physical terminal trace, not launch-only MR0"
        );
    }
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        serde_json::to_value(&w).unwrap()
    );
}

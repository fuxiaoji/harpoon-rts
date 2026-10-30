//! Source-only candidates: English Jan2026 PDF101/8-21 section8.5.4.4;
//! PDF109/9-8 section9.7.2 refers back to that section. Every aircraft store,
//! damage value, mass-neutral load and release geometry below is explicitly
//! laboratory policy, not a claim about MH60R or any real Annex DC fit.
use harpoon_core::rules::{asw_weapons2026 as asw, communications2026 as comms};
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
const DC: &str = "lab_air_dc";
const SONAR: &str = "lab_air_dc_sonar";

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn cmd(actor: EntityId, domain: &str, action: Value) -> Command {
    serde_json::from_value(json!({"cmd":domain,"unit":actor,"action":action}))
        .expect("future typed action through the existing command envelope")
}
fn issue(w: &mut World, side: Side, actor: EntityId, domain: &str, action: Value) {
    let command = cmd(actor, domain, action);
    command
        .validate_source(w, side)
        .expect("real source/own knowledge admission");
    let since = w.event_seq;
    harpoon_core::sim::tick(w, &[(side, command)]);
    let rejected: Vec<_> = w
        .events
        .iter()
        .filter(|e| e.seq > since)
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. }
                if rule == "8.5.4.4.request_rejected" || rule == "8.5.4.request_rejected" =>
            {
                Some((rule, detail))
            }
            _ => None,
        })
        .collect();
    assert!(
        rejected.is_empty(),
        "real post-movement ASW rejection: {rejected:?}"
    );
}
fn contact(w: &World) -> ContactId {
    w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == id(w, "TARGET") && !c.stale && !c.bearing_only)
        .unwrap()
        .id
}
fn own(w: &World, actor: EntityId) -> Value {
    asw::depth_charge_own_view(w, actor)
}
fn ammo(w: &World, actor: EntityId) -> u16 {
    w.entity(actor)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == DC)
        .unwrap()
        .ammo
}
fn result(w: &World) -> Value {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "8.5.4.4.pattern_result" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .last()
        .expect("actual typed air-depth-charge common-batch trace")
}
fn fixture(payload: Value, dipping: bool) -> World {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut weapon = db.weapons["cw_depth_charge"].clone();
    weapon.id = DC.into();
    weapon.name =
        "scenario_assumption: laboratory airborne store; no real aircraft/DC fitting claim".into();
    weapon.pk = 0.;
    weapon.damage = 999.;
    weapon.reload_s = 9000.;
    db.weapons.insert(DC.into(), weapon);
    let mut mount = db.platforms["us_mh60r"].weapons[0].clone();
    mount.weapon = DC.into();
    mount.mounts = 1;
    mount.ammo = 24;
    db.platforms.get_mut("us_mh60r").unwrap().weapons = vec![mount];
    db.platforms.get_mut("us_mh60r").unwrap().sensors =
        if dipping { vec![SONAR.into()] } else { vec![] };
    let mut sonar = db.sensors.values().next().unwrap().clone();
    sonar.id = SONAR.into();
    sonar.kind = harpoon_core::db::SensorKind::SonarActive;
    sonar.name = "scenario_assumption: controlled laboratory receiver".into();
    sonar.sub_range_nm = 16.;
    db.sensors.insert(SONAR.into(), sonar);
    db.platforms.get_mut("us_ford").unwrap().sensors = vec![SONAR.into()];
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    let mut scene = json!({"id":"air-dc-source-candidate","name":"scenario_assumption: laboratory algorithm only, real Annex applicability unresolved","seed":20261104,"duration_s":20000,
      "weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
        "aviation_2026":{"source":"scenario_assumption: explicit mass-neutral mechanism fixture, not operational aircraft stores","loads":{"us_mh60r":{"payload_kg":0,"tanks":[],"source":"scenario_assumption: mass-neutral experimental DC mechanism case"}},"parking":{},"initial_fuel_nm":{"AIR":200}},
        "asw_weapons_2026":{"source":"scenario_assumption: explicit laboratory air-DC hardware","standoffs":{},"air_depth_charges":{DC:{
          "source":"scenario_assumption: independent laboratory store, no Annex interpretation","payload":payload,"damage_major":7.,"damage_minor":3.,"max_depth_band":7,
          "units":{"AIR":{"source":"scenario_assumption: original helicopter body carrying laboratory stores","mount_index":0,"round_mass_kg":0.,
            "release_policy":{"source":"scenario_assumption: experiment geometry; no 9.7.1 torpedo envelope inherited","maximum_datum_range_nm":2.}}}}}},
        "communications_2026":{"source":"scenario_assumption: directed physical voice route","units":{
          "OBSERVER":{"source":"scenario_assumption","fits":{"voice":{"source":"scenario_assumption","protocol":"lab-air-dc","send":true,"receive":true,"operational":true,"peers":["AIR"],"warning_kind":"voice","transport":{"type":"hf","max_range_nm":20}}}},
          "AIR":{"source":"scenario_assumption","fits":{"voice":{"source":"scenario_assumption","protocol":"lab-air-dc","send":true,"receive":true,"operational":true,"peers":[],"warning_kind":"voice","transport":{"type":"hf","max_range_nm":20}}}}}},
        "ew_2026":{"fits":{"lab_acm":{"source":"scenario_assumption: explicit original-body acoustic CM experiment","kind":"torpedo_acm","generation":1,"launcher":"internal","endurance_s":null,"deployment_s":null,"radar_height_m":null}},"loadouts":{"cn_093":{"lab_acm":1}},"environment":null},
        "detection_2026":{"bindings":{format!("us_ford:{SONAR}"):{"type":"sonar","source":"scenario_assumption: independent Gen5 bow receiver","generation":5,"passive_range_nm":null,"active_range_nm":16.,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false}},
          "environment":{"source":"scenario_assumption: controlled open-water target","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
          "targets":{"cn_093":{"source":"scenario_assumption: explicit laboratory acoustic attributes","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
      "units":[{"side":"Blue","class":"us_ford","name":"OBSERVER","pos":[0,1],"course_deg":180,"speed_kts":0,"sonar_active":true,"fire_on":false},
        {"side":"Blue","class":"us_mh60r","name":"AIR","pos":[0,0],"speed_kts":0,"altitude_m":if dipping{10}else{1000},"fire_on":false},
        {"side":"Red","class":"cn_093","name":"TARGET","pos":[0.08,0],"speed_kts":0,"depth_m":50,"fire_on":false}]});
    if dipping {
        scene["context"]["detection_2026"]["bindings"][format!("us_mh60r:{SONAR}")] = json!({"type":"sonar","source":"scenario_assumption: actually deployed Gen4 active dipping receiver","generation":4,"passive_range_nm":null,"active_range_nm":16.,"frequency":"lmf","mount":"dipping","active_torpedo_detection":false,"high_speed_tow":false});
        scene["context"]["detection_2026"]["dipping_platforms"] = json!({"us_mh60r":"helicopter"});
    }
    if matches!(
        scene["context"]["asw_weapons_2026"]["air_depth_charges"][DC]["payload"]["type"].as_str(),
        Some("cluster_rbk100")
    ) {
        scene["context"]["asw_weapons_2026"]["submarine_hulls"] = json!({"TARGET":{"source":"scenario_assumption: laboratory four-class hull target for source contact algorithm; not a real Type093 design claim","size":"medium","design":"double","in_service_year":2000}});
    }
    let sc:Scenario=serde_json::from_value(scene).expect("source red must reach ordinary strict air_depth_charges configuration, not a new Rust enum compile error");
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let air = id(&w, "AIR");
    // Actual operator activation is required for custom sensor IDs. The
    // legacy sonar_a* spawn shortcut does not activate this experiment's ID.
    let observer = id(&w, "OBSERVER");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSonar {
                unit: observer,
                active: true,
            },
        )],
    );
    if dipping {
        harpoon_core::sim::tick(
            &mut w,
            &[(
                Side::Blue,
                Command::SetSonar {
                    unit: air,
                    active: true,
                },
            )],
        );
    }
    issue(
        &mut w,
        Side::Blue,
        air,
        "aviation",
        json!({"type":"throttle","setting":"Hover"}),
    );
    if dipping {
        issue(
            &mut w,
            Side::Blue,
            air,
            "detection",
            json!({"type":"dip","sensor":SONAR,"deploy":true,"depth_m":15.}),
        );
    }
    let target = id(&w, "TARGET");
    while w.tick < 1800 {
        let located = w.sides[0]
            .contacts
            .values()
            .any(|c| c.about == target && !c.stale && !c.bearing_only);
        let own_dip = !dipping
            || w.rules
                .aaw
                .as_ref()
                .unwrap()
                .firecontrol
                .tracks
                .values()
                .any(|t| {
                    t.observer == air
                        && t.target == target
                        && t.report
                            .as_ref()
                            .is_some_and(|r| r.measured_position.is_some())
                });
        if located && own_dip {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(
        w.sides[0]
            .contacts
            .values()
            .any(|c| c.about == target && !c.stale && !c.bearing_only),
        "the original sensor executor must produce the located contact"
    );
    if dipping {
        assert!(
            w.rules
                .aaw
                .as_ref()
                .unwrap()
                .firecontrol
                .tracks
                .values()
                .any(|t| t.observer == air && t.target == target && t.report.is_some()),
            "the exception needs an actual native deployed dipping report"
        );
    } else {
        let observer = id(&w, "OBSERVER");
        let cid = contact(&w);
        issue(
            &mut w,
            Side::Blue,
            observer,
            "communications",
            json!({"type":"send_warning","contact":cid,"sensor":SONAR,"fit":"voice","receivers":[air]}),
        );
        assert!(
            comms::received_located_report(&w, air, target)
                .unwrap()
                .is_some(),
            "genuine current received location, never free side fusion"
        );
    }
    let target = id(&w, "TARGET");
    issue(
        &mut w,
        Side::Red,
        target,
        "asw",
        json!({"type":"depth_charge_evasion","evading":true}),
    );
    w
}
fn drop(w: &mut World, charges: u16, depth: Option<usize>) {
    let actor = id(w, "AIR");
    let cid = contact(w);
    issue(
        w,
        Side::Blue,
        actor,
        "asw",
        json!({"type":"air_depth_charge_pattern","contact":cid,"weapon":DC,"charges":charges,"depth_band":depth}),
    );
}

#[test]
fn adc01_conventional_eight_store_limit_three_depth_choices_and_real_gen4_dipping_exception() {
    let mut w = fixture(json!({"type":"conventional"}), false);
    let air = id(&w, "AIR");
    let sub = id(&w, "TARGET");
    let nine = cmd(
        air,
        "asw",
        json!({"type":"air_depth_charge_pattern","contact":contact(&w),"weapon":DC,"charges":9,"depth_band":1}),
    );
    let raw = serde_json::to_value(&w).unwrap();
    assert!(nine.validate_source(&w, Side::Blue).is_err());
    assert_eq!(serde_json::to_value(&w).unwrap(), raw);
    let stock = ammo(&w, air);
    drop(&mut w, 8, Some(1));
    assert_eq!(ammo(&w, air), stock - 8);
    let blue = own(&w, air);
    let red = own(&w, sub);
    let attack = blue["air_pending_attacks"][0]["attack"]
        .as_str()
        .unwrap()
        .to_string();
    let alias = red["air_incoming_patterns"][0]["attack"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(attack, alias);
    assert_eq!(blue["air_pending_attacks"][0]["depth_options_required"], 3);
    assert!(
        blue["air_pending_attacks"][0]["depth_report"].is_null(),
        "friendly Gen5 bow sonar cannot become the aircraft's Gen4 dipping exception"
    );
    issue(
        &mut w,
        Side::Red,
        sub,
        "asw",
        json!({"type":"air_depth_charge_depth_report","attack":alias,"bands":[1,2,3]}),
    );
    let select = json!({"type":"air_depth_charge_select_depth","attack":attack,"depth_band":1});
    let mut restored: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    for state in [&mut w, &mut restored] {
        issue(state, Side::Blue, air, "asw", select.clone());
        let r = result(state);
        assert_eq!(r["base_percent"].as_f64(), Some(8.));
        assert_eq!(r["major_threshold"], 8);
        assert_eq!(r["minor_threshold"], 16);
        assert_eq!(r["distribution"], "dispersed");
        assert!(r["raw_d100"]
            .as_u64()
            .is_some_and(|v| (1..=100).contains(&v)));
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    let mut dip = fixture(json!({"type":"conventional"}), true);
    drop(&mut dip, 6, Some(1));
    assert!(
        result(&dip)["raw_d100"].is_number(),
        "an actual Gen4 active dipping report resolves exact depth without three-band negotiation"
    );
}

#[test]
fn adc02_homing_depth_search_uses_generation_multiplier_and_actual_acoustic_countermeasure_once() {
    let mut w = fixture(json!({"type":"homing","generation":2}), false);
    let air = id(&w, "AIR");
    let sub = id(&w, "TARGET");
    issue(
        &mut w,
        Side::Red,
        sub,
        "electronic_warfare",
        json!({"type":"deploy","fit":"lab_acm","at":null,"radar_signature":null,"course_deg":null,"speed_kts":null,"depth_m":null}),
    );
    assert!(
        w.rules
            .ew
            .as_ref()
            .unwrap()
            .objects
            .values()
            .any(|o| o.owner == sub && o.effective(w.tick)),
        "actual deployed acoustic CM, never a test flag"
    );
    let stock = ammo(&w, air);
    drop(&mut w, 6, None);
    assert_eq!(ammo(&w, air), stock - 6);
    let r = result(&w);
    assert_eq!(r["base_percent"].as_f64(), Some(60.));
    assert_eq!(r["homing_multiplier"].as_f64(), Some(10.));
    assert_eq!(r["countermeasure_multiplier"].as_f64(), Some(0.5));
    assert_eq!(r["major_threshold"], 30);
    assert_eq!(r["minor_threshold"], 60);
    assert!(
        r["raw_d100"].is_number(),
        "homing searches real depth through its maximum; no depth-band guess required"
    );
    assert!(own(&w, air)["air_pending_attacks"]
        .as_array()
        .is_some_and(|v| v.is_empty()));
}

#[test]
fn adc03_rbk100_cluster_uses_two_percent_per_bomb_eight_bomb_limit_and_one_submunition_opportunity()
{
    let mut w = fixture(json!({"type":"cluster_rbk100"}), false);
    let air = id(&w, "AIR");
    let sub = id(&w, "TARGET");
    let stock = ammo(&w, air);
    drop(&mut w, 8, Some(1));
    assert_eq!(ammo(&w, air), stock - 8);
    let attack = own(&w, air)["air_pending_attacks"][0]["attack"]
        .as_str()
        .unwrap()
        .to_string();
    let alias = own(&w, sub)["air_incoming_patterns"][0]["attack"]
        .as_str()
        .unwrap()
        .to_string();
    issue(
        &mut w,
        Side::Red,
        sub,
        "asw",
        json!({"type":"air_depth_charge_depth_report","attack":alias,"bands":[1,2,3]}),
    );
    issue(
        &mut w,
        Side::Blue,
        air,
        "asw",
        json!({"type":"air_depth_charge_select_depth","attack":attack,"depth_band":1}),
    );
    let r = result(&w);
    assert_eq!(r["base_percent"].as_f64(), Some(16.));
    assert_eq!(r["cluster_max_submunition_hits"], 1);
    assert!(r["submunition_hits"].as_u64().is_some_and(|n| n <= 1));
    assert!(r["raw_d100"].is_number());
    assert!(
        w.entities
            .values()
            .all(|e| e.projectile.as_ref().is_none_or(|p| p.weapon != DC)),
        "no fictitious torpedo or legacy999DP branch"
    );
}

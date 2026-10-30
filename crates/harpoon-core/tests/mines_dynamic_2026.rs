//! Source §13.3/13.4/13.7/13.8; explicit scenario equipment, never real-class fit claims.
use harpoon_core::{rules::mines2026 as m, Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};
const YD: f64 = 2025.3718285;
fn row() -> Value {
    json!({"id":"row","owner":"Blue","mine":"cn_chen2_500","area":{"start":{"x":0,"y":0},"end":{"x":75./YD,"y":0},"width_yd":400},"rows":1,"spacing_yd":75,"row_spacing_yd":200,"case_band":1,"water_band":1,"armed_at":0,"source":"scenario_assumption: surveyed Shallow bottom, complete source row"})
}
fn config() -> Value {
    json!({"source":"scenario_assumption: equipment and quantities are authored, not Ford annex fit",
        "laying_fits":{"WORKER":{"source":"scenario_assumption: controlled ordinary equipped ship","kind":"ordinary","variants":["cn_chen2_500"]}},
        "carried_mines":{"WORKER":{"source":"scenario_assumption: friendly forces exact stock","mines":{"cn_chen2_500":10}}},
        "lay_plans":{"row":{"source":"scenario_assumption: one whole-row RTS batch","unit":"WORKER","field":row()}},
        "permissive":[{"start":{"x":-2,"y":0},"end":{"x":2,"y":0},"width_yd":10000}]})
}
fn load(c: Value, speed: f64, sea: u8) -> Result<World, String> {
    let sc: Scenario = serde_json::from_value(json!({"id":"source-dynamic-mines","name":"source controlled equipment","duration_s":30000,"seed":20261023,
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":sea,"precipitation":"None"},"mines_2026":c},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"us_ford","name":"WORKER","pos":[0,0],"speed_kts":speed,"course_deg":90}]})).unwrap();
    harpoon_core::load_scenario(sc, Database::built_in())
}
fn action(v: Value) -> m::Action {
    serde_json::from_value(v).expect("new source Mines action must deserialize")
}
fn command(w: &mut World, a: Value) {
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::Mines {
                unit: EntityId(1),
                action: action(a),
            },
        )],
    );
}
fn advance(w: &mut World, until: i64) {
    for _ in 0..=(until - w.tick).max(0) {
        if w.tick >= until {
            return;
        }
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
    assert_eq!(w.tick, until);
}
fn state(w: &World) -> Value {
    serde_json::to_value(w.rules.mines.as_ref().unwrap()).unwrap()
}
fn bodies(w: &World) -> usize {
    state(w)["bodies"].as_object().map_or(0, |v| v.len())
}
fn stock(w: &World) -> u64 {
    state(w)["stocks"]["1"]["cn_chen2_500"].as_u64().unwrap()
}
#[test]
fn real_complete_row_uses_work_clock_actual_track_stock_and_release_arming() {
    let mut w = load(config(), 10., 4).unwrap();
    command(&mut w, json!({"type":"lay","plan_id":"row"}));
    advance(&mut w, 180);
    assert_eq!(bodies(&w), 0, "source quota is per180s, never per second");
    advance(&mut w, 181);
    assert_eq!(bodies(&w), 2);
    assert_eq!(stock(&w), 8);
    let s = state(&w);
    for body in s["bodies"].as_object().unwrap().values() {
        assert_eq!(body["released_at"], 181);
        assert_eq!(body["armed_at"], 541);
    }
    assert_eq!(s["fields"]["row"]["armed_at"], 541);
}
#[test]
fn stationary_ship_and_cancel_cannot_fabricate_remote_row_or_spend_stock() {
    let mut w = load(config(), 0., 4).unwrap();
    command(&mut w, json!({"type":"lay","plan_id":"row"}));
    advance(&mut w, 400);
    assert_eq!(bodies(&w), 0);
    assert_eq!(stock(&w), 10);
    command(&mut w, json!({"type":"cancel_lay"}));
    assert!(state(&w)["lay_jobs"]
        .as_object()
        .is_none_or(|v| v.is_empty()));
}
#[test]
fn real_source_ship_speed_sea_and_missing_own_equipment_block_atomically() {
    for (speed, sea) in [(20.001, 4), (20., 5)] {
        let mut w = load(config(), speed, sea).unwrap();
        assert!(m::apply(
            &mut w,
            EntityId(1),
            &action(json!({"type":"lay","plan_id":"row"}))
        )
        .is_err());
        assert_eq!(stock(&w), 10);
        assert_eq!(bodies(&w), 0);
    }
    let mut c = config();
    c["laying_fits"] = json!({});
    let mut w = load(c, 10., 4).unwrap();
    assert!(m::apply(
        &mut w,
        EntityId(1),
        &action(json!({"type":"lay","plan_id":"row"}))
    )
    .is_err());
    assert_eq!(stock(&w), 10);
}
#[test]
fn exact_variant_method_and_arming_unknowns_are_not_guessed() {
    for change in ["method", "bottom", "arming", "quota", "stock"] {
        let mut c = config();
        match change {
            "method" => {
                c["laying_fits"]["WORKER"]["kind"] = json!("air");
            }
            "bottom" => {
                c["lay_plans"]["row"]["field"]["water_band"] = json!(2);
            }
            "arming" => {
                c["lay_plans"]["row"]["field"]["case_band"] = json!(0);
                c["lay_plans"]["row"]["field"]["water_band"] = json!(0);
            }
            "quota" => {
                c["lay_plans"]["row"]["field"]["area"]["end"]["x"] = json!(600. / YD);
            }
            _ => {
                c["carried_mines"]["WORKER"]["mines"]["cn_chen2_500"] = json!(1);
            }
        }
        let mut w = load(c, 10., 4).unwrap();
        assert!(
            m::apply(
                &mut w,
                EntityId(1),
                &action(json!({"type":"lay","plan_id":"row"}))
            )
            .is_err(),
            "{change}"
        );
        assert_eq!(bodies(&w), 0);
    }
}
#[test]
fn source_roles_and_actual_combat_halving_limit_whole_row_quota() {
    for (role, quota) in [
        ("minelayer", 20),
        ("rails", 16),
        ("ordinary", 8),
        ("merchant", 12),
    ] {
        let mut c = config();
        c["laying_fits"]["WORKER"]["kind"] = json!(role);
        c["carried_mines"]["WORKER"]["mines"]["cn_chen2_500"] = json!(100);
        c["lay_plans"]["row"]["field"]["area"]["end"]["x"] = json!((quota - 1) as f64 * 75. / YD);
        let mut w = load(c, 20., 4).unwrap();
        assert!(
            m::apply(
                &mut w,
                EntityId(1),
                &action(json!({"type":"lay","plan_id":"row"}))
            )
            .is_ok(),
            "{role} full quota"
        );
        // Real event hook, no hidden global enemy-state predicate.
        w.emit(
            harpoon_core::EventKind::WeaponsRelease {
                side: Side::Blue,
                shooter: EntityId(1),
                shooter_name: "WORKER".into(),
                weapon: "controlled combat event".into(),
                count: 1,
            },
            harpoon_core::Visibility::Referee,
        );
        advance(&mut w, 181);
        assert_eq!(
            bodies(&w),
            0,
            "combat half-rate cannot finish a full-rate cohort"
        );
    }
}
#[test]
fn actual_lay_save_resume_and_activation_timer_preserve_full_world() {
    let mut c = config();
    c["lay_plans"]["row"]["activation_delay_s"] = json!(7200);
    let mut w = load(c, 10., 4).unwrap();
    command(&mut w, json!({"type":"lay","plan_id":"row"}));
    advance(&mut w, 90);
    let mut r: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    advance(&mut w, 200);
    advance(&mut r, 200);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&r).unwrap()
    );
    assert_eq!(bodies(&w), 2);
    assert_eq!(state(&w)["fields"]["row"]["activation_at"], 7381);
    assert!(
        m::apply(
            &mut w,
            EntityId(1),
            &action(json!({"type":"lay","plan_id":"row"}))
        )
        .is_err(),
        "same plan cannot create a duplicate cohort"
    );
}
fn hunting() -> Value {
    let mut c = config();
    c["fields"] = json!([row()]);
    c["lay_plans"] = json!({});
    c["mine_bodies"] = json!([
        {"id":"mine-a","field":"row","mine":"cn_chen2_500","owner":"Blue","position":{"x":0,"y":0},"case_band":1,"released_at":0,"armed_at":0,"source":"scenario_assumption: complete known source manifest"},
        {"id":"mine-b","field":"row","mine":"cn_chen2_500","owner":"Blue","position":{"x":75./YD,"y":0},"case_band":1,"released_at":0,"armed_at":0,"source":"scenario_assumption: complete known source manifest"}]);
    c["field_manifests"] = json!({"row":["mine-a","mine-b"]});
    c["known_mine_contacts"] = json!([
        {"id":"classified-a","side":"Blue","mine_id":"mine-a","position":{"x":0,"y":0},"case_band":1,"classified":true,"source":"scenario_assumption: previously classified by referee; not engine discovery"},
        {"id":"classified-b","side":"Blue","mine_id":"mine-b","position":{"x":75./YD,"y":0},"case_band":1,"classified":true,"source":"scenario_assumption: previously classified by referee; not engine discovery"}]);
    c["hunter_fits"] = json!({"WORKER":{"source":"scenario_assumption: controlled ROV fit, not a real Ford capability","method":"rov","range_yd":1000,"max_depth_band":1,"max_work_speed_kts":1,"charges":2}});
    c
}
#[test]
fn hidden_truth_id_and_unclassified_echo_are_not_hunting_permissions() {
    let mut c = hunting();
    c["known_mine_contacts"][0]["classified"] = json!(false);
    let mut w = load(c, 0., 4).unwrap();
    for contact in ["mine-a", "nonexistent", "classified-a"] {
        let before = serde_json::to_value(&w.rules.mines).unwrap();
        assert!(m::apply(
            &mut w,
            EntityId(1),
            &action(json!({"type":"hunt","contact_id":contact}))
        )
        .is_err());
        assert_eq!(before, serde_json::to_value(&w.rules.mines).unwrap());
    }
}
#[test]
fn real_hunt_cost_original_half_hour_die_and_resume_are_saved() {
    let mut w = load(hunting(), 0., 4).unwrap();
    command(&mut w, json!({"type":"hunt","contact_id":"classified-a"}));
    let j = state(&w)["hunts"]["mine-a"].clone();
    let roll = j["d6"].as_i64().unwrap();
    let duration = j["required_s"].as_i64().unwrap();
    assert!((1..=6).contains(&roll));
    assert_eq!(duration, roll * 1800);
    assert_eq!(state(&w)["charges"]["1"], 1);
    advance(&mut w, 900);
    let mut r: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    advance(&mut w, duration);
    advance(&mut r, duration);
    assert!(!state(&w)["bodies"]["mine-a"]["neutralized"]
        .as_bool()
        .unwrap());
    advance(&mut w, duration + 1);
    advance(&mut r, duration + 1);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&r).unwrap()
    );
    assert_eq!(state(&w)["bodies"]["mine-a"]["neutralized"], true);
    assert_eq!(state(&w)["bodies"]["mine-b"]["neutralized"], false);
    assert!(
        state(&w)["fields"]["row"].is_object(),
        "partial clear cannot clear a whole field"
    );
}
#[test]
fn cancelled_hunt_resumes_same_die_claim_and_charge() {
    let mut w = load(hunting(), 0., 4).unwrap();
    command(&mut w, json!({"type":"hunt","contact_id":"classified-a"}));
    advance(&mut w, 100);
    let before = state(&w)["hunts"]["mine-a"].clone();
    command(&mut w, json!({"type":"cancel_hunt"}));
    advance(&mut w, 200);
    command(&mut w, json!({"type":"hunt","contact_id":"classified-a"}));
    let now = state(&w)["hunts"]["mine-a"].clone();
    assert_eq!(before["d6"], now["d6"]);
    assert_eq!(before["worked_s"], now["worked_s"]);
    assert_eq!(state(&w)["charges"]["1"], 1);
}
#[test]
fn real_own_combat_pauses_hunting_without_hidden_enemy_predicate() {
    let mut w = load(hunting(), 0., 4).unwrap();
    command(&mut w, json!({"type":"hunt","contact_id":"classified-a"}));
    advance(&mut w, 100);
    let worked = state(&w)["hunts"]["mine-a"]["worked_s"].clone();
    w.emit(
        harpoon_core::EventKind::WeaponsRelease {
            side: Side::Blue,
            shooter: EntityId(1),
            shooter_name: "WORKER".into(),
            weapon: "controlled combat event".into(),
            count: 1,
        },
        harpoon_core::Visibility::Referee,
    );
    advance(&mut w, 279);
    assert_eq!(state(&w)["hunts"]["mine-a"]["worked_s"], worked);
    advance(&mut w, 280);
    assert_eq!(
        state(&w)["hunts"]["mine-a"]["worked_s"].as_i64().unwrap(),
        worked.as_i64().unwrap() + 1
    );
}
#[test]
fn hunting_range_depth_work_speed_and_exact_consumables_block_before_roll() {
    for gate in ["range", "depth", "speed", "charges", "source"] {
        let mut c = hunting();
        match gate {
            "range" => {
                c["known_mine_contacts"][0]["position"]["x"] = json!(10.);
            }
            "depth" => {
                c["hunter_fits"]["WORKER"]["max_depth_band"] = json!(0);
            }
            "speed" => {
                c["hunter_fits"]["WORKER"]["max_work_speed_kts"] = json!(0.);
            }
            "charges" => {
                c["hunter_fits"]["WORKER"]["charges"] = json!(0);
            }
            _ => {
                c["hunter_fits"]["WORKER"]["source"] = json!("");
            }
        }
        let mut w = load(c, if gate == "speed" { 0.1 } else { 0. }, 4).unwrap();
        let before = serde_json::to_value(&w.rules.mines).unwrap();
        assert!(
            m::apply(
                &mut w,
                EntityId(1),
                &action(json!({"type":"hunt","contact_id":"classified-a"}))
            )
            .is_err(),
            "{gate}"
        );
        assert_eq!(before, serde_json::to_value(&w.rules.mines).unwrap());
    }
}
#[test]
fn only_complete_manifest_neutralization_retires_field_and_partial_path_is_pending() {
    let mut w = load(hunting(), 0., 4).unwrap();
    command(&mut w, json!({"type":"hunt","contact_id":"classified-a"}));
    let t = state(&w)["hunts"]["mine-a"]["required_s"].as_i64().unwrap() + 1;
    advance(&mut w, t);
    // Referee crossing fixture exercises real field path code after partial neutralization.
    let e = w.entity_mut(EntityId(1)).unwrap();
    e.pos = harpoon_core::Vec2::new(0.01, 0.2);
    w.tick += 1;
    m::step(&mut w);
    assert!(state(&w)["partial_paths"]
        .as_array()
        .is_some_and(|a| !a.is_empty()));
    assert!(w.events.iter().all(|e|!matches!(&e.kind,harpoon_core::EventKind::RulesEvent{domain,rule,..}if domain=="mines2026"&&rule=="13.5.attack")));
    w.entity_mut(EntityId(1)).unwrap().pos = harpoon_core::Vec2::new(0., 0.);
    command(&mut w, json!({"type":"hunt","contact_id":"classified-b"}));
    let duration = state(&w)["hunts"]["mine-b"]["required_s"].as_i64().unwrap();
    let end = w.tick + duration;
    advance(&mut w, end);
    assert!(state(&w)["fields"]["row"].is_null());
    assert_eq!(state(&w)["charges"]["1"], 0);
}
#[test]
fn real_submarine_row_uses_declared_same_tubes_and_exact_360_second_reload() {
    let mut c = config();
    c["laying_fits"]["WORKER"]["kind"] = json!("sub");
    c["laying_fits"]["WORKER"]["tube_ids"] = json!(["bow-a", "bow-b"]);
    let sc:Scenario=serde_json::from_value(json!({"id":"source-sub-mines","name":"physical mine tubes","duration_s":3000,"seed":20261023,
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":5,"precipitation":"None"},"mines_2026":c,
            "torpedo_2026":{"source":"scenario_assumption: controlled mine/torpedo shared physical tubes","platforms":{"us_virginia":{
                "source":"scenario_assumption: declared same physical tubes; not Virginia operational load","wire_capacity":2,"reload_parallel":2,"tube_ids":{"bow-a":0,"bow-b":1},
                "tubes":[{"bearing_deg":0,"loaded":"cn_chen2_500"},{"bearing_deg":0,"loaded":"cn_chen2_500"}],"reserves":{"cn_chen2_500":8}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"us_virginia","name":"WORKER","pos":[0,0],"depth_m":50,"speed_kts":5,"course_deg":90,"fire_on":false}]})).unwrap();
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    command(&mut w, json!({"type":"lay","plan_id":"row"}));
    advance(&mut w, 60);
    assert_eq!(
        bodies(&w),
        2,
        "source submerged sea5/two real tubes must lay two mines after actual route"
    );
    assert_eq!(stock(&w), 8);
    let release = state(&w)["bodies"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()["released_at"]
        .as_i64()
        .unwrap();
    let torps = w.rules.torpedoes.as_ref().unwrap();
    assert!(torps.tubes[&EntityId(1)]
        .tubes
        .iter()
        .all(|t| t.loaded.is_none()));
    assert_eq!(torps.reloads.len(), 2);
    assert!(torps.reloads.iter().all(|r| r.due == release + 360));
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    advance(&mut w, release + 359);
    advance(&mut restored, release + 359);
    assert!(w.rules.torpedoes.as_ref().unwrap().tubes[&EntityId(1)]
        .tubes
        .iter()
        .all(|t| t.loaded.is_none()));
    advance(&mut w, release + 360);
    advance(&mut restored, release + 360);
    assert!(w.rules.torpedoes.as_ref().unwrap().tubes[&EntityId(1)]
        .tubes
        .iter()
        .all(|t| t.loaded.as_deref() == Some("cn_chen2_500")));
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
#[test]
fn own_observation_discovers_authorized_plan_and_geometry_without_enemy_plan() {
    let mut w = load(config(), 10., 4).unwrap();
    let mut foreign = w.entity(EntityId(1)).unwrap().clone();
    foreign.id = EntityId(2);
    foreign.side = Side::Red;
    foreign.name = "FOREIGN".into();
    foreign.pos = harpoon_core::Vec2::new(100., 100.);
    w.entities.insert(foreign.id, foreign);
    let c = w.context.mines_2026.as_mut().unwrap();
    let mut value = serde_json::to_value(&*c).unwrap();
    value["laying_fits"]["FOREIGN"] = value["laying_fits"]["WORKER"].clone();
    value["carried_mines"]["FOREIGN"] = value["carried_mines"]["WORKER"].clone();
    let mut secret = value["lay_plans"]["row"].clone();
    secret["unit"] = json!("FOREIGN");
    secret["field"]["id"] = json!("enemy-row");
    secret["field"]["owner"] = json!("Red");
    value["lay_plans"]["enemy-secret-plan"] = secret;
    *c = serde_json::from_value(value).unwrap();
    m::initialize(&mut w).unwrap();
    let blue = m::own_view(&w, Side::Blue).unwrap();
    let plans = blue["lay_plans"]
        .as_array()
        .expect("own player must discover actionable source plan IDs");
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0]["id"], "row");
    let expected_geometry: m::Rect = serde_json::from_value(row()["area"].clone()).unwrap();
    assert_eq!(
        plans[0]["geometry"],
        serde_json::to_value(expected_geometry).unwrap()
    );
    assert!(plans[0]["source"]
        .as_str()
        .unwrap()
        .starts_with("scenario_assumption:"));
    let observation = harpoon_core::observe::observe(&w, Side::Blue, 0, 1000);
    assert_eq!(serde_json::to_value(observation).unwrap()["mines"], blue);
    assert!(!blue.to_string().contains("enemy-secret-plan"));
    let red = m::own_view(&w, Side::Red).unwrap();
    assert_eq!(red["lay_plans"][0]["id"], "enemy-secret-plan");
    assert!(!red.to_string().contains("\"id\":\"row\""));
    let r: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(blue, m::own_view(&r, Side::Blue).unwrap());
    assert_eq!(red, m::own_view(&r, Side::Red).unwrap());
}
#[test]
fn duplicate_bound_unit_names_never_duplicate_source_mine_inventory() {
    let mut w = load(config(), 0., 4).unwrap();
    let mut duplicate = w.entity(EntityId(1)).unwrap().clone();
    duplicate.id = EntityId(2);
    duplicate.side = Side::Red;
    w.entities.insert(duplicate.id, duplicate);
    let before = state(&w);
    assert!(
        m::initialize(&mut w).is_err(),
        "one authored stock row cannot bind two physical units"
    );
    assert_eq!(
        state(&w),
        before,
        "failed source initialization must not replace a valid inventory"
    );
}
#[test]
fn complete_manifest_cannot_attack_before_body_arming_or_activation_clock() {
    for clock in ["armed_at", "activation_at"] {
        let mut c = hunting();
        c["mine_bodies"][0][clock] = json!(360);
        assert!(load(c,0.,4).is_err(),"mismatched {clock} could let the abstract Field attack before this source body is ready");
    }
}
#[test]
fn own_classified_hunt_view_survives_save_and_ignores_hidden_mine_changes() {
    let w = load(hunting(), 0., 4).unwrap();
    let own = m::own_view(&w, Side::Blue).unwrap();
    assert_eq!(own["contacts"].as_array().unwrap().len(), 2);
    assert_eq!(own["contacts"][0]["id"], "classified-a");
    assert!(!own.to_string().contains("mine-a"));
    assert!(!own.to_string().contains("mine-b"));
    let mut other: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let b = other
        .rules
        .mines
        .as_mut()
        .unwrap()
        .bodies
        .get_mut("mine-b")
        .unwrap();
    b.mine = "cn_chen0_500".into();
    b.owner = Side::Red;
    b.position = harpoon_core::Vec2::new(200., 200.);
    assert_eq!(
        own,
        m::own_view(&other, Side::Blue).unwrap(),
        "hidden physical state is not own report geometry"
    );
    assert!(m::own_view(&other, Side::Red).is_none());
}
#[test]
fn contested_cross_side_classified_body_is_explicitly_unsupported_at_initialization() {
    let mut c = hunting();
    let mut foreign = c["known_mine_contacts"][0].clone();
    foreign["id"] = json!("red-classified-a");
    foreign["side"] = json!("Red");
    c["known_mine_contacts"]
        .as_array_mut()
        .unwrap()
        .push(foreign);
    assert!(load(c,0.,4).is_err(),"trusted scenario must reject unsupported contested neutralization; enemy work claim must not become an action-admission oracle");
}
#[test]
fn reported_hunt_admission_cannot_neutralize_a_physically_unreachable_mine() {
    for mismatch in ["range", "depth"] {
        let mut c = hunting();
        if mismatch == "range" {
            c["fields"][0]["area"]["start"]["x"] = json!(100.);
            c["fields"][0]["area"]["end"]["x"] = json!(100. + 75. / YD);
            for b in c["mine_bodies"].as_array_mut().unwrap() {
                b["position"]["x"] = json!(b["position"]["x"].as_f64().unwrap() + 100.);
            }
        } else {
            c["fields"][0]["mine"] = json!("ru_mdm1_china_g");
            c["fields"][0]["case_band"] = json!(2);
            c["fields"][0]["water_band"] = json!(2);
            for b in c["mine_bodies"].as_array_mut().unwrap() {
                b["mine"] = json!("ru_mdm1_china_g");
                b["case_band"] = json!(2);
            }
        }
        // An inaccurate own source report stays locally admissible. It grants
        // no physical ability to operate beyond the actual neutralizer envelope.
        let mut w = load(c, 0., 4).unwrap();
        let mut reachable = load(hunting(), 0., 4).unwrap();
        command(&mut w, json!({"type":"hunt","contact_id":"classified-a"}));
        command(
            &mut reachable,
            json!({"type":"hunt","contact_id":"classified-a"}),
        );
        assert_eq!(
            m::own_view(&w, Side::Blue),
            m::own_view(&reachable, Side::Blue),
            "{mismatch}: immediate own receipt must not disclose physical truth"
        );
        assert!(
            state(&w)["hunts"]["mine-a"].is_object(),
            "hidden truth must not turn into a command-admission rejection"
        );
        assert_eq!(state(&w)["charges"]["1"], 1);
        let original = state(&w)["hunts"]["mine-a"]["d6"].clone();
        advance(&mut w, 60);
        advance(&mut reachable, 60);
        assert_eq!(
            m::own_view(&w, Side::Blue),
            m::own_view(&reachable, Side::Blue),
            "{mismatch}: whole own task view uses attempted time, not private effective work"
        );
        let own = m::own_view(&w, Side::Blue).unwrap();
        assert!(own["units"][0]["hunts"][0].get("worked_s").is_none());
        assert_eq!(own["units"][0]["hunts"][0]["attempted_s"], 59);
        assert_eq!(
            state(&w)["hunts"]["mine-a"]["worked_s"],
            0,
            "{mismatch}: actual equipment must reach the real body before work can advance"
        );
        assert_eq!(state(&w)["hunts"]["mine-a"]["d6"], original);
        assert_eq!(state(&w)["bodies"]["mine-a"]["neutralized"], false);
        let r: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        assert_eq!(state(&w), state(&r));
    }
}

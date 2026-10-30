//! Source algorithm: English2026 §5.4.6.1/PDF49/50/52, §13.8.3/PDF126.
//! Positive receiver parameters and platform are explicit author assumptions.
//! The real Ton/Type193M source profile is blocked: K-2 has no such row.
//! Search produces an uncertain echo only, never mine classification.
use harpoon_core::{
    rules::{detection2026 as d, mines2026 as m},
    Database, EntityId, Scenario, Side, World,
};
use serde_json::{json, Value};
const PLATFORM: &str = "scenario_assumption:mine_search_platform";
const SENSOR: &str = "sonar_a_scenario_assumption:active_mh_receiver";
const ACTOR: &str = "ASSUMED MH1";
const REAL_TON: &str = "uk_ton_m1151_1971";
const REAL_SENSOR: &str = "sonar_a_uk_type193m";
const YD: f64 = 2025.3718285;
fn config() -> Value {
    json!({"source":"scenario_assumption: exact surveyed two-mine source row; no pre-known contacts",
        "fields":[{"id":"unobserved-field","owner":"Red","mine":"cn_chen2_500","area":{"start":{"x":0,"y":0},"end":{"x":75./YD,"y":0},"width_yd":400},"rows":1,"spacing_yd":75,"row_spacing_yd":200,"case_band":1,"water_band":1,"armed_at":0,"source":"scenario_assumption: surveyed40m bottom and complete homogeneous row"}],
        "mine_bodies":[
            {"id":"private-mine-alpha","field":"unobserved-field","mine":"cn_chen2_500","owner":"Red","position":{"x":0,"y":0},"case_band":1,"released_at":0,"armed_at":0,"source":"scenario_assumption: referee physical manifest"},
            {"id":"private-mine-beta","field":"unobserved-field","mine":"cn_chen2_500","owner":"Red","position":{"x":75./YD,"y":0},"case_band":1,"released_at":0,"armed_at":0,"source":"scenario_assumption: referee physical manifest"}],
        "field_manifests":{"unobserved-field":["private-mine-alpha","private-mine-beta"]},
        "case_depths":{"private-mine-alpha":{"source":"scenario_assumption: surveyed40m actual bottom","depth_m":40},"private-mine-beta":{"source":"scenario_assumption: surveyed40m actual bottom","depth_m":40}},
        "search_fits":{ACTOR:[{"source":"scenario_assumption: fictional dedicated bottom-minehunting keel receiver; no published model", "platform_fit":PLATFORM,"sensor":SENSOR,
          "scenario_assumption":{"source":"scenario_assumption: author supplied Gen3/HF/keel/.4nm solely to exercise core scan algorithm", "generation":3,"frequency":"hf","mount":"keel","active50_nm":0.4}}]}})
}
fn fixture(c: Value, sea: usize, layer: Option<usize>) -> World {
    fixture_with_identity(c, sea, layer, PLATFORM, SENSOR, ACTOR)
}
fn fixture_with_identity(
    c: Value,
    sea: usize,
    layer: Option<usize>,
    platform: &str,
    sensor_id: &str,
    actor: &str,
) -> World {
    let mut db = Database::built_in();
    let mut p = db.platforms["us_ford"].clone();
    p.id = platform.into();
    p.name = "scenario_assumption: fictitious mine-search engineering platform".into();
    p.hp = 32.;
    p.max_speed_kts = 15.;
    // Every platform number is an explicit engineering-fixture assumption,
    // never a historical Ton card or a substitute published equipment row.
    p.cruise_speed_kts = 0.;
    p.turn_rate_deg_s = 0.;
    p.accel_kts_per_s = 0.;
    p.vp_value = 0.;
    p.weapons.clear();
    p.deck = None;
    p.chaff = 0;
    p.decoys = 0;
    p.sensors = vec![sensor_id.into()];
    p.rule_data=Some(harpoon_core::rules::damage_runtime::PlatformDamage{size:"D".into(),role:"surface".into(),year:2000,dp_breaks:vec![0.,8.,16.,24.,29.,32.],speed_steps:vec![15.,11.,8.,4.,0.],source:"scenario_assumption: all fictional platform fixture scalars; no historical card claimed".into(),..Default::default()});
    db.platforms.insert(platform.into(), p);
    let sensor: harpoon_core::db::SensorDef = serde_json::from_value(
        json!({"id":sensor_id,"name":"scenario_assumption: fictional active receiver","kind":"SonarActive","sub_range_nm":0.4,"alt_m":0}),
    )
    .unwrap();
    db.sensors.insert(sensor_id.into(), sensor);
    let sc:Scenario=serde_json::from_value(json!({"id":"authored-mine-search","name":"scenario_assumption: core mine sonar scan fixture","duration_s":3000,"seed":20261023,
        "context":{"rules_profile":"hv2026-rts-v4","mines_2026":c,"detection_2026":{"environment":{"source":"scenario_assumption: surveyed40m no ice/shipping/rain","sea_state":sea,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":40,"layer":layer,"ice":0,"cloud":null}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":platform,"name":actor,"pos":[0,-0.02],"speed_kts":0,"course_deg":0,"sonar_active":true,"fire_on":false}]})).unwrap();
    harpoon_core::load_scenario(sc, db).unwrap()
}
fn tick_to(w: &mut World, t: i64) {
    for _ in 0..=(t - w.tick).max(0) {
        if w.tick >= t {
            return;
        }
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
    assert_eq!(w.tick, t);
}
fn echoes(w: &World, side: Side) -> Vec<Value> {
    m::own_view(w, side).unwrap_or_default()["echoes"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}
fn scan_traces(w: &World) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            harpoon_core::EventKind::RulesEvent {
                domain,
                rule,
                detail,
                ..
            } if domain == "mines2026" && rule == "5.mine.scan" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn wait_echo(w: &mut World) {
    tick_to(w, 541);
    assert!(
        !echoes(w, Side::Blue).is_empty(),
        "explicit fictitious receiver must execute the core algorithm and issue an author-labelled echo"
    );
}
#[test]
fn source_nearest_centre_row_includes_positive_below_minimum_without_new_cells() {
    assert_eq!(d::sonar_probability(0.24, 0.09, false).unwrap(), 90.);
    assert_eq!(d::sonar_probability(0.24, 0.49, false).unwrap(), 55.);
    assert!(d::sonar_probability(0., 0.09, false).is_err());
    assert!(d::sonar_probability(f64::NAN, 0.09, false).is_err());
    assert!(d::sonar_probability(f64::INFINITY, 0.09, false).is_err());
}
#[test]
fn explicitly_assumed_receiver_creates_opaque_labelled_echo_without_identity_or_hunt_qualification()
{
    let mut w = fixture(config(), 0, None);
    assert!(
        w.entity(EntityId(1)).unwrap().sensors[0].active,
        "positive fixture must use the real spawn active-sonar switch"
    );
    assert!(echoes(&w, Side::Blue).is_empty());
    wait_echo(&mut w);
    let own = echoes(&w, Side::Blue);
    let text = serde_json::to_string(&own).unwrap();
    assert!(!text.contains("private-mine"));
    assert!(!text.contains("cn_chen2"));
    assert!(!text.contains("Red"));
    for e in own {
        assert_eq!(e["classified"], false);
        assert!(e["reports"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["evidence_basis"] == "scenario_assumption"
                && r["source"]
                    .as_str()
                    .unwrap()
                    .starts_with("scenario_assumption:")));
        assert!(e["reports"].as_array().is_some_and(|a| a
            .iter()
            .any(|r| r["observer"] == 1 && r["sensor"] == SENSOR)));
        let id = e["id"].as_str().unwrap();
        let a: m::Action = serde_json::from_value(json!({"type":"hunt","contact_id":id})).unwrap();
        assert!(
            m::apply(&mut w, EntityId(1), &a).is_err(),
            "uncertain echo is not a classified mine receipt"
        );
    }
    assert_eq!(
        w.entities.len(),
        1,
        "mines never become combat/score entities"
    );
    assert!(
        w.sides[0].contacts.is_empty(),
        "uncertain objects do not enter ordinary FCS/weapon target contacts"
    );
}
#[test]
fn source_mine_strength_and_actual_environment_are_used_without_a_launcher_profile() {
    let mut w = fixture(config(), 0, None);
    wait_echo(&mut w);
    let traces = scan_traces(&w);
    let first = traces
        .iter()
        .find(|t| t.get("base50_nm").is_some())
        .expect("actual scan must trace source modifiers");
    assert_eq!(first["base50_nm"], 0.4);
    assert_eq!(first["target_strength"], -1);
    assert_eq!(first["relative_speed"], 0);
    assert_eq!(first["environment"], -1);
    assert_eq!(first["total"], -2);
    assert!((first["modified50_nm"].as_f64().unwrap() - 0.24).abs() < 1e-12);
    assert_eq!(first["selected50_nm"], 0.5);
}
#[test]
fn inactive_damaged_powerless_or_unverified_hf_is_not_an_actual_mine_scan() {
    for gate in ["inactive", "damaged", "power", "generic_hf"] {
        let mut c = config();
        if gate == "generic_hf" {
            c["search_fits"][ACTOR][0]["platform_fit"] = json!("ordinary_hf_not_minehunting");
        }
        let mut w = fixture(c, 0, None);
        match gate {
            "inactive" => w.entity_mut(EntityId(1)).unwrap().sensors[0].active = false,
            "damaged" => {
                w.entity_mut(EntityId(1))
                    .unwrap()
                    .ext
                    .insert(format!("damage.sensor.{SENSOR}"), 1.);
            }
            "power" => {
                // This casualty marker is an absolute recovery tick, not a
                // boolean. One source Intermediate Turn is1800seconds; the
                // prior value1 expired at the first simulated second.
                w.entity_mut(EntityId(1))
                    .unwrap()
                    .ext
                    .insert("damage.power_lost".into(), 1800.);
            }
            _ => {}
        }
        tick_to(&mut w, 541);
        assert!(echoes(&w, Side::Blue).is_empty(), "{gate}");
    }
}
#[test]
fn successful_search_is_side_and_receiver_scoped_and_save_restore_keeps_one_trial() {
    let mut w = fixture(config(), 0, None);
    tick_to(&mut w, 90);
    let before = serde_json::to_value(&w.rules.detection.as_ref().unwrap().trials).unwrap();
    assert!(
        before
            .as_object()
            .unwrap()
            .keys()
            .any(|k| k.starts_with("5.mine:")),
        "real mine scan must cache original receiver dice"
    );
    let mut r: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    tick_to(&mut w, 179);
    tick_to(&mut r, 179);
    assert_eq!(
        before,
        serde_json::to_value(&w.rules.detection.as_ref().unwrap().trials).unwrap()
    );
    tick_to(&mut w, 541);
    tick_to(&mut r, 541);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&r).unwrap()
    );
    assert!(!echoes(&w, Side::Blue).is_empty());
    assert!(echoes(&w, Side::Red).is_empty());
}
#[test]
fn same_side_mine_ownership_does_not_make_it_invisible_to_real_sonar() {
    let mut c = config();
    c["fields"][0]["owner"] = json!("Blue");
    for b in c["mine_bodies"].as_array_mut().unwrap() {
        b["owner"] = json!("Blue");
    }
    let mut w = fixture(c, 0, None);
    wait_echo(&mut w);
    assert!(echoes(&w, Side::Blue)
        .iter()
        .all(|e| e.get("owner").is_none()));
}
#[test]
fn unknown_exact_case_depth_cannot_be_replaced_by_a_band_midpoint() {
    let mut c = config();
    c["case_depths"] = json!({});
    let mut w = fixture(c, 0, None);
    tick_to(&mut w, 541);
    assert!(echoes(&w, Side::Blue).is_empty());
    assert!(
        scan_traces(&w)
            .iter()
            .any(|t| t["blocked"].as_str().is_some_and(|s| s.contains("depth"))),
        "source-depth absence must leave referee evidence"
    );
}
#[test]
fn sea_state_and_real_layer_change_only_source_modifiers_not_case_signature() {
    let mut calm = fixture(config(), 0, None);
    let mut rough = fixture(config(), 5, None);
    let mut layer = fixture(config(), 0, Some(0));
    tick_to(&mut calm, 1);
    tick_to(&mut rough, 1);
    tick_to(&mut layer, 1);
    let c = scan_traces(&calm);
    let r = scan_traces(&rough);
    let l = scan_traces(&layer);
    let first = |ts: Vec<Value>| {
        ts.into_iter()
            .find(|t| t.get("target_strength").is_some())
            .expect("actual source scan trace")
    };
    let (c, r, l) = (first(c), first(r), first(l));
    assert_eq!(c["target_strength"], -1);
    assert_eq!(r["target_strength"], -1);
    assert_eq!(l["target_strength"], -1);
    assert_eq!(r["environment"], -4);
    assert_eq!(l["layer"], -5);
}
#[test]
fn hidden_remote_mine_and_truth_metadata_do_not_change_own_echo_payload() {
    let mut a = fixture(config(), 0, None);
    let mut b = fixture(config(), 0, None);
    let s = b.rules.mines.as_mut().unwrap();
    let mut hidden = s.bodies["private-mine-beta"].clone();
    hidden.id = "unobserved-extra-body".into();
    hidden.position = harpoon_core::Vec2::new(900., 900.);
    s.bodies.insert(hidden.id.clone(), hidden);
    // Body ownership is hidden and supplies neither echo signature nor IFF.
    s.bodies.get_mut("private-mine-alpha").unwrap().owner = Side::Blue;
    wait_echo(&mut a);
    wait_echo(&mut b);
    assert_eq!(m::own_view(&a, Side::Blue), m::own_view(&b, Side::Blue));
}
#[test]
fn generic_hf_receiver_cannot_inherit_a_dedicated_receiver_fit_or_its_success() {
    let mut w = fixture(config(), 0, None);
    let other = "sonar_a_generic_hf";
    let mut def = w.db.sensors[SENSOR].clone();
    def.id = other.into();
    def.name = "scenario_assumption: generic HF; no source MH permission".into();
    w.db.sensors.insert(other.into(), def);
    w.entity_mut(EntityId(1))
        .unwrap()
        .sensors
        .push(harpoon_core::world::SensorInstance {
            sensor: other.into(),
            active: true,
        });
    let mut off = w.clone();
    off.entity_mut(EntityId(1)).unwrap().sensors[0].active = false;
    wait_echo(&mut w);
    tick_to(&mut off, 541);
    assert!(echoes(&off, Side::Blue).is_empty());
    assert!(echoes(&w, Side::Blue)
        .iter()
        .flat_map(|e| e["reports"].as_array().unwrap())
        .all(|r| r["sensor"] == SENSOR));
    assert!(w
        .rules
        .detection
        .as_ref()
        .unwrap()
        .trials
        .keys()
        .filter(|k| k.starts_with("5.mine:"))
        .all(|k| !k.contains(other)));
    assert!(w
        .events
        .iter()
        .filter(
            |e| matches!(&e.kind,harpoon_core::EventKind::RulesEvent {domain,rule,..}
        if domain=="mines2026" && rule=="5.mine.scan")
        )
        .all(|e| e.vis == harpoon_core::Visibility::Referee));
}
#[test]
fn native_electrical_recovery_clock_resumes_actual_search_after_mid_outage_restore() {
    let mut w = fixture(config(), 0, None);
    // scenario_assumption: an already sustained electrical casualty has600s
    // remaining. This is the native absolute recovery clock, not a boolean or
    // an invented separate sonar power supply.
    w.entity_mut(EntityId(1))
        .unwrap()
        .ext
        .insert("damage.power_lost".into(), 600.);
    tick_to(&mut w, 599);
    assert_eq!(
        w.entity(EntityId(1)).unwrap().ext.get("damage.power_lost"),
        Some(&600.)
    );
    assert!(echoes(&w, Side::Blue).is_empty());
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for world in [&mut w, &mut restored] {
        tick_to(world, 600);
        assert!(!world
            .entity(EntityId(1))
            .unwrap()
            .ext
            .contains_key("damage.power_lost"));
        // The existing rules schedule scans before damage::step removes the
        // expired casualty. The next native scan isT601; do not reorder the
        // engine or clear a casualty from this test to manufacture detection.
        assert!(echoes(world, Side::Blue).is_empty());
        tick_to(world, 601);
        let own = echoes(world, Side::Blue);
        assert!(!own.is_empty());
        assert!(own
            .iter()
            .all(|e| e["first_seen"] == 601 && e["classified"] == false));
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn unsupported_ton_type193m_source_claim_cannot_create_an_automatic_receiver_or_echo() {
    // Independent 2026-10-24 visual recheck: Commonwealth2025 PDF90/K-2
    // contains no Type193M row. PDF65/A-61 proves only the 1971 M1151 fit.
    // Its generation/range/frequency cannot be granted by a fabricated row.
    let mut c = config();
    let fits = c["search_fits"].as_object_mut().unwrap();
    fits.clear();
    fits.insert("M1151".into(), json!([{"source":"Commonwealth2025 PDF65/A-61 confirms1971 fit only; old PDF90/K-2 parameter claim is retracted", "platform_fit":REAL_TON,"sensor":REAL_SENSOR}]));
    let mut w = fixture_with_identity(c, 0, None, REAL_TON, REAL_SENSOR, "M1151");
    tick_to(&mut w, 541);
    assert!(
        echoes(&w, Side::Blue).is_empty(),
        "unsupported Type193M source claim must remain blocked"
    );
    assert!(!w
        .rules
        .detection
        .as_ref()
        .unwrap()
        .config
        .bindings
        .contains_key(&format!("{REAL_TON}:{REAL_SENSOR}")));
    assert!(scan_traces(&w).iter().any(|t| t["blocked"]
        .as_str()
        .is_some_and(|s| s.contains("verified"))));
}

#[test]
fn nearest_source_centre_uses_finite_positive_endpoints_without_overflow_or_false_ties() {
    assert_eq!(d::sonar_selected_50(f64::MAX).unwrap(), 100.);
    assert_eq!(d::sonar_selected_50(f64::MIN_POSITIVE).unwrap(), 0.5);
    // PDF51/5-12 continues the same table; do not truncate it at PDF50's15.
    assert_eq!(d::sonar_selected_50(99.9).unwrap(), 100.);
    assert_eq!(d::sonar_probability(f64::MAX, 179.9, false).unwrap(), 10.);
    assert_eq!(d::sonar_probability(f64::MAX, 180.1, false).unwrap(), 0.);
    // The source supplies no exact halfway priority; retain the existing block.
    assert!(d::sonar_selected_50(0.75).is_err());
}

#[test]
fn explicit_assumptions_cannot_masquerade_as_verified_historical_equipment() {
    for gate in [
        "published_platform",
        "published_sensor",
        "missing_label",
        "missing_parameters",
        "invalid_range",
        "invalid_generation",
        "unsupported_mount",
    ] {
        let mut c = config();
        let f = &mut c["search_fits"][ACTOR][0];
        let mut platform = PLATFORM;
        let mut sensor = SENSOR;
        match gate {
            "published_platform" => {
                platform = REAL_TON;
                f["platform_fit"] = json!(platform);
            }
            "published_sensor" => {
                sensor = REAL_SENSOR;
                f["sensor"] = json!(sensor);
            }
            "missing_label" => {
                f["scenario_assumption"]["source"] = json!("claimed published parameters")
            }
            "missing_parameters" => f
                .as_object_mut()
                .unwrap()
                .remove("scenario_assumption")
                .map(|_| ())
                .unwrap(),
            "invalid_range" => f["scenario_assumption"]["active50_nm"] = json!(0),
            "invalid_generation" => f["scenario_assumption"]["generation"] = json!(0),
            "unsupported_mount" => f["scenario_assumption"]["mount"] = json!("bow"),
            _ => unreachable!(),
        }
        let mut w = fixture_with_identity(c, 0, None, platform, sensor, ACTOR);
        tick_to(&mut w, 541);
        assert!(echoes(&w, Side::Blue).is_empty(), "{gate}");
        assert!(
            !w.rules
                .detection
                .as_ref()
                .unwrap()
                .config
                .bindings
                .contains_key(&format!("{platform}:{sensor}")),
            "{gate}"
        );
    }
}

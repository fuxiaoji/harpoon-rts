//! Native consumer regressions for14.2.2/14.2.3. Added before production.
//! Synthetic sensor/signature/geometry and adjudicated laboratory primary hits
//! are explicit scenario assumptions. No real hardware completeness or native
//! FCS is claimed. No HP, damage flag, Flight, phase, fuel or die is injected.
use harpoon_core::rules::{damage_runtime as damage, detection2026 as detection, torpedo2026};
use harpoon_core::{
    Command, ContactId, Database, EntityId, EventKind, Scenario, Side, Vec2, World,
};
use serde_json::{json, Value};

fn database() -> Database {
    Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap()
}
fn profile(size: usize, noise: usize, coating: bool, radar: usize) -> Value {
    json!({"source":"scenario_assumption: explicit consumer laboratory signature, not real platform fit",
        "size":size,"noise":noise,"anechoic":coating,"titanium":false,"pumpjet":false,
        "stealthy_ir":false,"radar_signature":radar})
}
fn environment() -> Value {
    json!({"source":"scenario_assumption: open water zero modifiers at sea state2 LMF",
        "sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
        "water_depth_m":3000,"layer":null,"ice":0,"cloud":null})
}
fn details(w: &World, wanted: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == wanted => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn restore(w: &World) -> World {
    let copy: World = serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap();
    copy.rules.validate_tables().unwrap();
    copy
}
fn entity(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn adjudicated_primary(w: &mut World, target: EntityId, underwater: bool) {
    let original = w.entity(target).unwrap().hp_max;
    let mut weapon = w.db.weapons["us_mk48"].clone();
    weapon.id = "scenario_signature_laboratory_primary".into();
    weapon.name = "scenario_assumption: adjudicated11percent primary laboratory hit".into();
    weapon.kind = if underwater {
        harpoon_core::db::WeaponKind::Torpedo
    } else {
        harpoon_core::db::WeaponKind::Ascm
    };
    weapon.damage = original * 0.11;
    weapon.rule_data = Some(damage::WeaponDamage {
        ship_dp: Some(original * 0.11),
        sub_dp: Some(original * 0.11),
        penetration_cm: Some(10000.),
        // Explicit lab influence fuze avoids contact armor/TPS and secondary
        // auto-flood. This is an adjudicated hit, not a source release claim.
        fuze: "influence".into(),
        source:
            "scenario_assumption: synthetic exact primary damage; no real warhead performance claim"
                .into(),
        ..Default::default()
    });
    let prior = w.entity(target).unwrap().hp;
    harpoon_core::damage::apply_hit_at(w, target, &weapon, underwater, Some(90.));
    let resolved = details(w, "hit_resolved")
        .into_iter()
        .find(|e| e["weapon"] == weapon.id)
        .expect("actual primary resolution event");
    assert!((resolved["dp"].as_f64().unwrap() - original * 0.11).abs() < 1e-8);
    assert!(
        w.entity(target).is_some_and(|e| e.alive && e.hp < prior),
        "actual committed primary damage, no HP setter"
    );
}

fn sonar_fixture() -> (World, EntityId, EntityId) {
    let sc:Scenario=serde_json::from_value(json!({
        "id":"damage-signature-native-sonar-consumer","name":"scenario_assumption: signature consumer laboratory",
        "seed":23,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "detection_2026":{"environment":environment(),"targets":{
                "us_virginia":profile(2,2,true,2),"cn_093":profile(2,2,true,2)}}},
        "units":[{"side":"Blue","class":"us_virginia","name":"OBSERVER","pos":[0,0],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false},
                 {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,1.5],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, database()).unwrap();
    for e in w.entities.values_mut() {
        e.weapons.clear();
        e.sensors.clear();
    }
    let o = entity(&w, "OBSERVER");
    let t = entity(&w, "TARGET");
    (w, o, t)
}
fn receiver(
    w: &mut World,
    id: EntityId,
    kind: harpoon_core::db::SensorKind,
    binding: detection::Receiver,
    suffix: &str,
) -> String {
    let key = format!("signature_lab_{suffix}_{}", id.0);
    let mut def = w.db.sensors.values().next().unwrap().clone();
    def.id = key.clone();
    def.kind = kind;
    def.air_range_nm = 0.;
    def.surface_range_nm = 0.;
    def.sub_range_nm = 0.;
    def.missile_range_nm = 0.;
    w.db.sensors.insert(key.clone(), def);
    w.entity_mut(id)
        .unwrap()
        .sensors
        .push(harpoon_core::world::SensorInstance {
            sensor: key.clone(),
            active: true,
        });
    let class = w.entity(id).unwrap().class.clone();
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .bindings
        .insert(format!("{class}:{key}"), binding);
    key
}
fn sonar(w: &mut World, id: EntityId, active: bool) -> String {
    receiver(
        w,
        id,
        if active {
            harpoon_core::db::SensorKind::SonarActive
        } else {
            harpoon_core::db::SensorKind::SonarPassive
        },
        detection::Receiver::Sonar {
            source: "scenario_assumption: explicit Gen5 LMF4nm50percent laboratory receiver".into(),
            generation: 5,
            passive_range_nm: Some(4.),
            active_range_nm: Some(4.),
            frequency: detection::Frequency::Lmf,
            mount: detection::SonarMount::Bow,
            active_torpedo_detection: true,
            high_speed_tow: false,
        },
        if active { "active" } else { "passive" },
    )
}
fn scan(w: &mut World, o: EntityId, sensor: &str, t: EntityId) -> detection::ScanDecision {
    let mut state = w.rules.detection.take().unwrap();
    let result = detection::evaluate_world(w, &mut state, o, sensor, t)
        .expect("native source consumer must be available; no helper projection substitute");
    w.rules.detection = Some(state);
    result
}

#[test]
fn native_radar_after_committed_surface_damage_selects_larger_annex_column_and_detects() {
    let sc:Scenario=serde_json::from_value(json!({"id":"signature-damage-radar","name":"scenario_assumption: radar column laboratory",
        "seed":23,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
            "detection_2026":{"environment":environment(),"targets":{"us_burke_2a":profile(1,2,false,2)}}},
        "units":[{"side":"Blue","class":"us_f18e","name":"AIR_OBSERVER","pos":[0,0],"altitude_m":6000,"course_deg":0,"speed_kts":0,"fire_on":false},
                 {"side":"Red","class":"us_burke_2a","name":"SMALL_SIGNATURE_MEDIUM_SHIP","pos":[0,104.5],"speed_kts":0,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, database()).unwrap();
    let o = entity(&w, "AIR_OBSERVER");
    let t = entity(&w, "SMALL_SIGNATURE_MEDIUM_SHIP");
    for e in w.entities.values_mut() {
        e.weapons.clear();
        if e.id == t {
            e.sensors.clear();
        }
    }
    let sensor = w
        .entity(o)
        .unwrap()
        .sensors
        .iter()
        .find(|s| s.sensor == "radar_air_med")
        .expect("actual APG79 bound source radar")
        .sensor
        .clone();
    w.entity_mut(o)
        .unwrap()
        .sensors
        .retain(|s| s.sensor == sensor);
    let before = scan(&mut w, o, &sensor, t);
    assert!(!before.detected);
    assert_eq!(
        before.trace["range_nm"], 76.0,
        "actual source Small Annex cell, horizon above both cells"
    );
    harpoon_core::sensors::update_sensors(&mut w);
    assert!(w.sides[0].contacts.is_empty());
    adjudicated_primary(&mut w, t, false);
    let after = scan(&mut w, o, &sensor, t);
    assert_eq!(
        after.trace["range_nm"], 133.0,
        "actual source Medium Annex cell, not a post-horizon arbitrary multiplier"
    );
    assert!(after.detected);
    harpoon_core::sensors::update_sensors(&mut w);
    assert!(w.sides[0]
        .contacts
        .values()
        .any(|c| c.about == t && !c.stale));
    assert_eq!(
        w.rules.detection.as_ref().unwrap().config.targets["us_burke_2a"].radar_signature,
        Some(2),
        "authored baseline remains immutable"
    );
}

#[test]
fn native_active_sonar_reads_lost_coating_after_actual_underwater_primary() {
    let (mut w, o, t) = sonar_fixture();
    let s = sonar(&mut w, o, true);
    let before = scan(&mut w, o, &s, t);
    assert_eq!(
        before.trace["inputs"]["target_modifier"], 0,
        "printed broad-aspect+1 and original coating-1 are separate source inputs"
    );
    assert_eq!(before.trace["inputs"]["modified50_nm"], 3.2);
    adjudicated_primary(&mut w, t, true);
    let after = scan(&mut w, o, &s, t);
    assert_eq!(
        after.trace["inputs"]["target_modifier"], 1,
        "real active return must stop subtracting carried coating while retaining broad-aspect+1"
    );
    assert_eq!(after.trace["inputs"]["modified50_nm"], 4.0);
    assert_eq!(
        after.trace["inputs"]["total"].as_i64().unwrap(),
        before.trace["inputs"]["total"].as_i64().unwrap() + 1
    );
    assert_eq!(
        after.trace["d100"], before.trace["d100"],
        "same scan window keeps its real original die"
    );
    assert!(w.rules.detection.as_ref().unwrap().config.targets["cn_093"].anechoic);
}

#[test]
fn native_passive_sonar_uses_persistent_noisy_and_complete_world_restore() {
    let (mut w, o, t) = sonar_fixture();
    let s = sonar(&mut w, o, false);
    let before = scan(&mut w, o, &s, t);
    assert_eq!(before.trace["inputs"]["target_modifier"], 0);
    assert_eq!(before.trace["inputs"]["modified50_nm"], 4.0);
    adjudicated_primary(&mut w, t, true);
    let after = scan(&mut w, o, &s, t);
    assert_eq!(
        after.trace["inputs"]["target_modifier"], 3,
        "real passive scan must use Noisy rather than fitted Quiet"
    );
    assert_eq!(after.trace["inputs"]["modified50_nm"], 8.0);
    assert_eq!(after.trace["d100"], before.trace["d100"]);
    let mut b = restore(&w);
    for _ in 0..12 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut b, &[]);
    }
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert_eq!(
        scan(&mut w, o, &s, t).trace["inputs"]["target_modifier"],
        3,
        "lower speed or time do not repair damage signatures"
    );
}

#[test]
fn native_air_reverse_active_strength_uses_the_damaged_listener_coating() {
    let (mut w, o, t) = sonar_fixture();
    let _emitter = sonar(&mut w, t, true);
    let air = receiver(
        &mut w,
        o,
        harpoon_core::db::SensorKind::SonarPassive,
        detection::Receiver::AcousticIntercept {
            source: "scenario_assumption: Gen5 direct AIR laboratory receiver".into(),
            generation: 5,
        },
        "air",
    );
    let before = scan(&mut w, o, &air, t);
    assert!(before.detected);
    assert_eq!(before.trace["modified50_nm"], 3.2);
    adjudicated_primary(&mut w, o, true);
    let after = scan(&mut w, o, &air, t);
    assert_eq!(
        after.trace["modified50_nm"], 4.0,
        "reverse emitter sonar must stop seeing intact coating on damaged AIR listener"
    );
    assert!(after.detected);
    assert!(w.rules.detection.as_ref().unwrap().config.targets["us_virginia"].anechoic);
}

#[test]
fn native_active_coating_can_use_verified_torpedo_noise_when_detection_noise_is_unneeded() {
    let (mut w, o, t) = sonar_fixture();
    // These are authored source inputs before damage, not a damage-state
    // setter. Active sonar has size/coating; a complete independent torpedo
    // source supplies the original Quiet baseline for14.2.3's exemption.
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .targets
        .get_mut("cn_093")
        .unwrap()
        .noise = None;
    w.rules.torpedoes.as_mut().unwrap().config.targets.insert(
        "cn_093".into(),
        torpedo2026::Target {
            source:
                "scenario_assumption: complete original acoustic source supplied by torpedo fit"
                    .into(),
            size: 2,
            noise: 2,
            anechoic: true,
            foil_borne: false,
        },
    );
    let sensor = sonar(&mut w, o, true);
    let before = scan(&mut w, o, &sensor, t);
    assert_eq!(before.trace["inputs"]["modified50_nm"], 3.2);
    adjudicated_primary(&mut w, t, true);
    let after = scan(&mut w, o, &sensor, t);
    assert_eq!(after.trace["inputs"]["modified50_nm"], 4.0,
        "active coating consumer must not require an unused duplicate detection noise field when the committed original source is complete");
    assert_eq!(after.trace["d100"], before.trace["d100"]);
}

fn die(seed: u64, id: u32, purpose: &str, sides: u8) -> u8 {
    let mut r = harpoon_core::Rng::new(seed).derive(harpoon_core::Rng::hash_tag(0, id, purpose));
    1 + (r.next_f64() * sides as f64) as u8
}
fn seed_for(test: impl Fn(u64) -> bool) -> u64 {
    (1..=2_000_000).find(|s| test(*s)).unwrap()
}
fn torpedo_fixture(
    seed: u64,
    passive: bool,
    range: f64,
    opportunity: bool,
) -> (World, EntityId, EntityId, EntityId) {
    let mut weapon = torpedo2026::tables()["weapons"]["us_mk48"].clone();
    if passive {
        weapon["guidance"] = json!("Passive");
        weapon["source"]=json!("scenario_assumption: passive-only mechanism variant using unchanged Mk48 source performance; not a real hardware-fit claim");
    }
    let other_pos = if opportunity {
        json!([0.8, 5.5])
    } else {
        json!([500, 500])
    };
    let mut torpedo = json!({"source":"scenario_assumption: source consumer geometry/author Good, not native FCS",
        "environment":{"source":"scenario_assumption: open no layer/reverb","layer_m":null,"reverberant":false},
        "targets":{"cn_093":{"source":"scenario_assumption: original target baseline","size":2,"noise":2,"anechoic":true,"foil_borne":false},
                   "cn_094":{"source":"scenario_assumption: independent ordinary opportunity baseline","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
        "platforms":{"us_virginia":{"source":"scenario_assumption: legal two loaded tubes","wire_capacity":2,"reload_parallel":2,
            "tubes":[{"bearing_deg":0,"loaded":"us_mk48"},{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}},
        "orders":{"SHOOTER:us_mk48":{"source":"scenario_assumption: explicit legal initial order","speed_index":0,"depth_m":50,
            "enable_run_nm":0,"bearing_deg":null,"cut_wire":true,"spread_deg":[],"solution":{"quality":"Good","source":"scenario_assumption: controlled post-release consumer fixture"}}}});
    if passive {
        torpedo["weapons"] = json!({"us_mk48":weapon});
    }
    let sc:Scenario=serde_json::from_value(json!({"id":"damaged-torpedo-consumer","name":"scenario_assumption: damage consumer native torpedo laboratory",
        "duration_s":4000,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},"torpedo_2026":torpedo,
            "detection_2026":{"environment":environment(),"targets":{"cn_093":profile(2,2,true,2),"cn_094":profile(2,2,true,2)}}},
        "units":[{"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},
                 {"side":"Red","class":"cn_093","name":"ORIGINAL","pos":[0,range],"speed_kts":0,"depth_m":50,"fire_on":false},
                 {"side":"Red","class":"cn_094","name":"OTHER","pos":other_pos,"speed_kts":0,"depth_m":50,"fire_on":false},
                 {"side":"Blue","class":"us_burke_2a","name":"FAR_RETAINER","pos":[500,-500],"speed_kts":0,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, database()).unwrap();
    let s = entity(&w, "SHOOTER");
    let t = entity(&w, "ORIGINAL");
    let other = entity(&w, "OTHER");
    for e in w.entities.values_mut() {
        e.sensors.clear();
        if e.side == Side::Red {
            e.weapons.clear();
        }
    }
    // Explicit authored located ORIGINAL report only. OTHER is never given to
    // Blue. This isolates released physics; native sonar/FCS is not claimed.
    w.sides[0].contacts.insert(
        ContactId(99),
        harpoon_core::world::Contact {
            id: ContactId(99),
            about: t,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., range),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: 0,
            stale: false,
            sensors: vec![],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert(format!("0:{}", t.0), harpoon_core::types::PlatformKind::Sub);
    (w, s, t, other)
}
fn launch(w: &mut World, s: EntityId) -> EntityId {
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: s,
                contact: ContactId(99),
                weapon: "us_mk48".into(),
                rounds: 1,
            },
        )],
    );
    w.entities
        .values()
        .find(|e| e.kind == harpoon_core::types::PlatformKind::Torpedo)
        .expect("real legal native Fire released a torpedo")
        .id
}
fn until(w: &mut World, rule: &str, max: i64) -> Value {
    for _ in 0..max {
        if let Some(e) = details(w, rule).last() {
            return e.clone();
        }
        harpoon_core::sim::tick(w, &[]);
    }
    panic!("native source event {rule} absent; no Flight/dice fallback")
}

#[test]
fn native_new_active_placement_reads_damage_even_with_explicit_torpedo_target_override() {
    let seed = seed_for(|s| {
        die(s, 0, "10.placement:5:0", 10) >= 8 && die(s, 0, "10.placement:5:0", 10) <= 9
    });
    let (mut w, s, t, _) = torpedo_fixture(seed, false, 3., false);
    adjudicated_primary(&mut w, t, true);
    let p = launch(&mut w, s);
    assert_eq!(p.0, 5);
    let placed = until(&mut w, "10.2.4.placement", 500);
    assert_eq!(
        placed["placement"]["chance"], 9,
        "new active placement loses its coating-2 from explicit torpedo-domain profile"
    );
    assert!(placed["placement"]["placed"].as_bool().unwrap());
    assert!(
        w.rules.torpedoes.as_ref().unwrap().config.targets["cn_093"].anechoic,
        "immutable fitted coating not rewritten"
    );
}

#[test]
fn native_new_passive_acquisition_uses_damaged_noisy_in_explicit_torpedo_domain() {
    let seed = seed_for(|s| die(s, 0, "10.placement:5:0", 10) <= 9);
    let (mut w, s, t, _) = torpedo_fixture(seed, true, 3., false);
    adjudicated_primary(&mut w, t, true);
    let p = launch(&mut w, s);
    assert_eq!(p.0, 5);
    let acquired = until(&mut w, "7.5.7.acquired", 500);
    let expected = 2000. * 0.9144 / 1852. * 2.;
    assert!(
        (acquired["source_acquisition_nm"].as_f64().unwrap() - expected).abs() < 1e-8,
        "native passive capture consumes Noisy2x, not original Quiet1x"
    );
    assert!(
        w.entity(p).unwrap().pos.range_to(w.entity(t).unwrap().pos) > 2000. * 0.9144 / 1852.,
        "actual lock occurs beyond undamaged Quiet range"
    );
    let placement = details(&w, "10.2.4.placement").remove(0);
    assert_eq!(
        placement["placement"]["chance"], 9,
        "pure passive never had active-coating penalty"
    );
}

fn opportunity_seed() -> u64 {
    seed_for(|s| {
        die(s, 0, "10.placement:5:0", 10) <= 7
            && die(s, 5, "10.2.5.attack:0", 100) > 80
            && die(s, 0, "10.placement:5:1", 10) > 7
            && die(s, 0, "10.placement:5:1:op:3", 10) <= 7
            && die(s, 5, "10.2.5.attack:1", 100) <= 80
    })
}

#[test]
fn native_opportunity_scan_uses_its_own_damaged_coating_and_noise_before_capture() {
    let (mut w, s, _, other) = torpedo_fixture(opportunity_seed(), false, 5., true);
    adjudicated_primary(&mut w, other, true);
    let _p = launch(&mut w, s);
    let capture = until(&mut w, "10.2.7.opportunity_acquired", 1600);
    assert_eq!(capture["candidate"], other.0);
    let flight =
        serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().flights[&EntityId(5)]).unwrap();
    let inputs = &flight["opportunity"]["capture"]["inputs"];
    assert_eq!(
        inputs["profile"]["noise"], 1,
        "actual candidate scan consumes own persistent Noisy overlay"
    );
    assert_eq!(
        inputs["profile"]["anechoic"], false,
        "candidate scan direct config lookup cannot bypass coating damage"
    );
    assert_eq!(inputs["placement_modifier"], 0);
    assert!(
        w.sides[0].contacts.values().all(|c| c.about != other),
        "private physical capture is not a free Blue contact"
    );
}

#[test]
fn captured_entity_then_real_damage_keeps_original_dice_pursuit_manual_and_full_world_restore() {
    let (mut w, s, original, other) = torpedo_fixture(opportunity_seed(), false, 5., true);
    let p = launch(&mut w, s);
    let _capture = until(&mut w, "10.2.7.opportunity_acquired", 1600);
    let before = serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().flights[&p]).unwrap();
    let original_placement = before["opportunity"]["capture"]["original_placement"].clone();
    let candidate_placement = before["opportunity"]["capture"]["candidate_placement"].clone();
    let original_hp = w.entity(original).unwrap().hp;
    adjudicated_primary(&mut w, other, true);
    let manual: Command =
        serde_json::from_value(json!({"cmd":"set_unit_automation","unit":s,"enabled":false}))
            .unwrap();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, manual)]);
    let mut restored = restore(&w);
    let origin = w.entity(p).unwrap().pos;
    for _ in 0..120 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
    assert!(
        !w.rules.pending_adjudications.contains_key(&p),
        "legitimate damage overlay cannot look like a changed original source and stall pursuit"
    );
    let terminal = details(&w, "10.2.5.source_terminal")
        .into_iter()
        .find(|e| e["opportunity"] == other.0)
        .expect("actual captured Entity must continue to its one real native terminal");
    assert_eq!(terminal["target"], other.0);
    assert_eq!(w.entity(original).unwrap().hp, original_hp);
    let final_f = serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().flights[&p]).unwrap();
    assert_eq!(
        final_f["opportunity"]["capture"]["original_placement"],
        original_placement
    );
    assert_eq!(
        final_f["opportunity"]["capture"]["candidate_placement"],
        candidate_placement
    );
    assert_eq!(details(&w, "10.2.7.opportunity_acquired").len(), 1);
    assert!(w.entity(p).map_or(true, |e| !e.alive || e.pos != origin));
    assert!(
        harpoon_core::observe(&w, Side::Blue, 0, 0)
            .units
            .iter()
            .find(|u| u.id == s)
            .unwrap()
            .manual_control
    );
    assert!(w.sides[0].contacts.values().all(|c| c.about != other));
    let own = torpedo2026::own_view(&w, Side::Blue, s).unwrap();
    assert_eq!(own["releases"][0]["status"], "unknown");
    assert!(own["releases"][0]["physical_alive"].is_null());
    for e in &w.events {
        if let EventKind::RulesEvent { rule, .. } = &e.kind {
            if rule.starts_with("14.2.") || rule.starts_with("10.2.7.opportunity") {
                assert!(matches!(e.vis, harpoon_core::Visibility::Referee));
            }
        }
    }
}

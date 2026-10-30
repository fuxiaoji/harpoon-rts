//! Independent source boundaries for English2026 PDF132/14-6 §§14.2.2/3.
//! Synthetic signatures, equipment and adjudicated laboratory hits are explicit
//! scenario assumptions. Original loaded hull DP remains unchanged. Projections
//! only observe actual committed damage; no HP/signature state/Flight/die setter.
use harpoon_core::db::{Guidance, SensorKind, WeaponDef, WeaponKind};
use harpoon_core::rules::{
    damage_runtime as damage, detection2026 as detection, ew2026 as ew,
    signature_damage2026 as signature,
};
use harpoon_core::{
    ContactId, Database, EntityId, EventKind, Scenario, Side, Vec2, Visibility, World,
};
use serde_json::{json, Value};
use std::sync::OnceLock;

fn database() -> Database {
    static DB: OnceLock<Database> = OnceLock::new();
    DB.get_or_init(|| {
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
    })
    .clone()
}

fn profile(size: usize, noise: Option<usize>, coating: bool, radar: usize) -> Value {
    json!({"source":"scenario_assumption: boundary laboratory fitted signature, not actual complete platform fit",
        "size":size,"noise":noise,"anechoic":coating,"titanium":false,"pumpjet":false,
        "stealthy_ir":false,"radar_signature":radar})
}

fn fixture(
    rules: &str,
    submarine: bool,
    noise: Option<usize>,
    coating: bool,
    speed: f64,
    torpedo_target: Option<Value>,
) -> (World, EntityId, EntityId) {
    let mut db = database();
    let class = if submarine { "cn_093" } else { "us_burke_2a" };
    for key in ["us_virginia", class] {
        let p = db.platforms.get_mut(key).unwrap();
        p.weapons.clear();
        p.sensors.clear();
        p.rule_data = Some(damage::PlatformDamage {
            size: if key == "us_burke_2a" { "B" } else { "C" }.into(),
            role: "surface".into(),
            source: "scenario_assumption: controlled physical Size and zero armor; source hull DP unchanged".into(),
            ..Default::default()
        });
    }
    let mut targets = json!({"us_virginia":profile(2,Some(2),true,2)});
    targets[class] = profile(if submarine { 2 } else { 1 }, noise, coating, 2);
    let mut torpedo_targets = json!({});
    if let Some(p) = torpedo_target {
        torpedo_targets[class] = p;
    }
    let sc: Scenario = serde_json::from_value(json!({
        "id":"signature-boundaries-20261025","name":"scenario_assumption: independent source and privacy laboratory",
        "seed":23,"duration_s":5000,"weather":{"visibility_nm":100,"jam":[0,0]},
        "context":{"rules_profile":rules,"radar_environment":{"sea_state":0,"precipitation":"None"},
            "detection_2026":{"targets":targets,"environment":{
                "source":"scenario_assumption: open-water zero-modifier LMF laboratory",
                "sea_state":2,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
                "water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
            "torpedo_2026":{"source":"scenario_assumption: independent fitted-source boundary",
                "targets":torpedo_targets}},
        "units":[{"side":"Blue","class":"us_virginia","name":"OBSERVER","pos":[0,0],
                "course_deg":0,"speed_kts":0,"depth_m":0,"fire_on":false},
            {"side":"Red","class":class,"name":"TARGET","pos":[0,1.5],
                "course_deg":90,"speed_kts":speed,"depth_m":if submarine {50}else{0},"fire_on":false}]
    })).unwrap();
    let w = harpoon_core::load_scenario(sc, db).unwrap();
    let find = |name| w.entities.values().find(|e| e.name == name).unwrap().id;
    let observer = find("OBSERVER");
    let target = find("TARGET");
    assert_eq!(
        w.entity(target).unwrap().hp_max,
        if submarine { 145. } else { 341. }
    );
    (w, observer, target)
}

fn weapon(dp: f64, kind: WeaponKind) -> WeaponDef {
    let mut p = database().weapons["us_mk48"].clone();
    p.id = "scenario_assumption_signature_boundary_warhead".into();
    p.name = "scenario_assumption: adjudicated boundary laboratory primary".into();
    p.kind = kind;
    p.guidance = if kind == WeaponKind::Torpedo {
        Guidance::TorpedoActive
    } else {
        Guidance::AswBallistic
    };
    p.damage = dp;
    p.rule_data = Some(damage::WeaponDamage {
        ship_dp: Some(dp),
        sub_dp: Some(dp),
        penetration_cm: Some(0.),
        fuze: "influence".into(),
        source: "scenario_assumption: independent laboratory warhead, not altered real Mk48 data"
            .into(),
        ..Default::default()
    });
    p
}

fn hit(w: &mut World, target: EntityId, kind: WeaponKind) {
    let original = w.entity(target).unwrap().hp_max;
    let prior = w.entity(target).unwrap().hp;
    let p = weapon(original * 0.11, kind);
    harpoon_core::damage::apply_hit_at(w, target, &p, true, Some(90.));
    assert!(
        w.entity(target).unwrap().hp < prior,
        "real primary DP must commit"
    );
    if w.rules.profile != damage::RuleProfile::Legacy {
        assert!(
            events(w, target, "damage", "hit_resolved")
                .iter()
                .any(|e| (e["dp"].as_f64().unwrap() - original * 0.11).abs() < 1e-8),
            "actual resolved source DP, not a preloaded damaged flag"
        );
    }
}

fn events(w: &World, id: EntityId, domain: &str, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                domain: d,
                rule: r,
                unit,
                detail,
                ..
            } if d == domain && r == rule && *unit == Some(id) => {
                if d == "signature_damage2026" {
                    assert!(matches!(e.vis, Visibility::Referee));
                }
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn b01_missing_noise_does_not_erase_explicit_coating_source_conflict() {
    // Detection provides an explicit uncoated fit while the independent torpedo
    // source says coated. Its absent noise cell cannot delete that conflict.
    let torpedo = json!({"source":"scenario_assumption: independent conflicting torpedo fit",
        "size":2,"noise":2,"anechoic":true,"foil_borne":false});
    let (mut w, _, target) = fixture("hv2026-rts-v4", true, None, false, 0., Some(torpedo));
    hit(&mut w, target, WeaponKind::Torpedo);
    let pending = events(&w, target, "signature_damage2026", "14.2.signature_pending");
    assert_eq!(
        pending.len(),
        1,
        "source conflict must survive an absent detection noise cell"
    );
    assert!(pending[0]["branches"]["acoustic"]
        .as_str()
        .is_some_and(|s| s.contains("conflict")));
    assert!(
        signature::project_acoustic(&w, target, Some(2), true).is_err(),
        "cannot choose one contradictory source and silently strip its coating"
    );
    assert!(events(&w, target, "signature_damage2026", "14.2.3.acoustic").is_empty());
}

fn receiver(w: &mut World, owner: EntityId, passive: bool, visual: bool) -> String {
    let name = format!("scenario_signature_boundary_sensor_{}", owner.0);
    let mut def = w.db.sensors.values().next().unwrap().clone();
    def.id = name.clone();
    def.kind = if visual {
        SensorKind::Visual
    } else if passive {
        SensorKind::SonarPassive
    } else {
        SensorKind::SonarActive
    };
    def.air_range_nm = 0.;
    def.surface_range_nm = 0.;
    def.sub_range_nm = 0.;
    def.missile_range_nm = 0.;
    w.db.sensors.insert(name.clone(), def);
    w.entity_mut(owner)
        .unwrap()
        .sensors
        .push(harpoon_core::world::SensorInstance {
            sensor: name.clone(),
            active: true,
        });
    let binding = if visual {
        detection::Receiver::Visual {
            source: "scenario_assumption: ordinary source visual lookout".into(),
        }
    } else {
        detection::Receiver::Sonar {
            source: "scenario_assumption: controlled source Gen5 LMF4nm laboratory receiver".into(),
            generation: 5,
            passive_range_nm: Some(4.),
            active_range_nm: Some(4.),
            frequency: detection::Frequency::Lmf,
            mount: detection::SonarMount::Bow,
            active_torpedo_detection: true,
            high_speed_tow: false,
        }
    };
    let class = w.entity(owner).unwrap().class.clone();
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .config
        .bindings
        .insert(format!("{class}:{name}"), binding);
    name
}

fn scan(
    w: &mut World,
    observer: EntityId,
    sensor: &str,
    target: EntityId,
) -> Result<detection::ScanDecision, String> {
    let mut state = w.rules.detection.take().unwrap();
    let result = detection::evaluate_world(w, &mut state, observer, sensor, target);
    w.rules.detection = Some(state);
    result
}

#[test]
fn b02_missing_acoustic_source_is_pending_without_blocking_real_visual_or_radar_branch() {
    let (mut w, observer, target) = fixture("hv2026-rts-v4", false, None, true, 0., None);
    let visual = receiver(&mut w, observer, false, true);
    let before = scan(&mut w, observer, &visual, target).expect("source visual scan before damage");
    assert_eq!(before.trace["rule"], "5.8");
    assert!(
        before.detected,
        "close surface hull is actually visible in this fixed natural trial"
    );
    hit(&mut w, target, WeaponKind::Torpedo);
    let pending = events(&w, target, "signature_damage2026", "14.2.signature_pending");
    assert_eq!(pending.len(), 1);
    assert!(pending[0]["branches"]["acoustic"].is_string());
    assert!(pending[0]["branches"]["radar"].is_null());
    assert!(signature::project_acoustic(&w, target, None, true).is_err());
    assert_eq!(signature::project_radar(&w, target, 2).unwrap(), 1);
    let after = scan(&mut w, observer, &visual, target)
        .expect("acoustic pending must not block native visual");
    assert_eq!(after.trace["rule"], "5.8");
    assert_eq!(
        after.trace["d100"], before.trace["d100"],
        "no fresh trial to hide the source boundary"
    );
    assert!(after.detected);
}

#[test]
fn b03_unknown_underwater_cause_remains_pending_instead_of_guessing_all_asw_classes() {
    let (mut w, _, target) = fixture("hv2026-rts-v4", false, Some(2), true, 0., None);
    // An explicit laboratory Rocket + underwater bool is not a verified
    // torpedo, source mine, depth-charge fit, nuclear or ASW payload provenance.
    hit(&mut w, target, WeaponKind::Rocket);
    let pending = events(&w, target, "signature_damage2026", "14.2.signature_pending");
    assert_eq!(pending.len(), 1);
    assert!(pending[0]["branches"]["acoustic"]
        .as_str()
        .is_some_and(|s| s.contains("provenance")));
    assert!(events(&w, target, "signature_damage2026", "14.2.3.acoustic").is_empty());
    assert!(signature::project_acoustic(&w, target, Some(2), true).is_err());
    assert_eq!(
        signature::project_radar(&w, target, 2).unwrap(),
        1,
        "unknown underwater producer does not erase separately proven surface DP/RCS"
    );
}

#[test]
fn b04_legacy_v2_v3_actual_primary_damage_never_enables_new_signature_state() {
    for rules in ["rts-legacy-v1", "hv2026-rts-v2", "hv2026-rts-v3"] {
        let (mut w, _, target) = fixture(rules, false, Some(2), true, 0., None);
        hit(&mut w, target, WeaponKind::Torpedo);
        assert!(
            serde_json::to_value(&w).unwrap()["rules"]
                .get("signature_damage")
                .is_none(),
            "empty field must remain absent for historical profile {rules}"
        );
        assert!(signature::own_view(&w, target).is_none());
        assert_eq!(signature::project_radar(&w, target, 2).unwrap(), 2);
        assert_eq!(
            signature::project_acoustic(&w, target, Some(2), true).unwrap(),
            (Some(2), true)
        );
        assert!(events(&w, target, "signature_damage2026", "14.2.2.signature").is_empty());
        assert!(events(&w, target, "signature_damage2026", "14.2.3.acoustic").is_empty());
    }
}

#[test]
fn b05_native_passive_damage_uses_noisy_without_replacing_original_propeller_cavitation_gate() {
    let (mut w, observer, target) = fixture("hv2026-rts-v4", true, Some(4), true, 20., None);
    let passive = receiver(&mut w, observer, true, false);
    assert!(
        !detection::cavitating(4, 20., 50., false).unwrap(),
        "original ExtremelyQuiet propeller is below its source gate"
    );
    assert!(
        detection::cavitating(1, 20., 50., false).unwrap(),
        "substituting damage Noisy would manufacture cavitation"
    );
    let before = scan(&mut w, observer, &passive, target).unwrap();
    assert_eq!(before.trace["inputs"]["target_modifier"], -6);
    hit(&mut w, target, WeaponKind::Torpedo);
    assert_eq!(
        w.entity(target).unwrap().speed_kts,
        20.,
        "natural hit must preserve this boundary geometry"
    );
    assert_eq!(signature::project_noise(&w, target, 4).unwrap(), 1);
    let after = scan(&mut w, observer, &passive, target).unwrap();
    assert_eq!(
        after.trace["inputs"]["target_modifier"], 3,
        "Noisy +3 only, never +7 from replacing the original cavitation input"
    );
    assert_eq!(after.trace["d100"], before.trace["d100"]);
    assert_eq!(
        w.rules.detection.as_ref().unwrap().config.targets["cn_093"].noise,
        Some(4)
    );
}

#[test]
fn b06_whole_noisy_loud_exception_preserves_actual_carried_coating_in_native_active_scan() {
    for noise in [0, 1] {
        let (mut w, observer, target) = fixture("hv2026-rts-v4", true, Some(noise), true, 0., None);
        let active = receiver(&mut w, observer, false, false);
        let before = scan(&mut w, observer, &active, target).unwrap();
        hit(&mut w, target, WeaponKind::Torpedo);
        assert_eq!(
            signature::project_acoustic(&w, target, Some(noise), true).unwrap(),
            (Some(noise), true)
        );
        assert!(events(&w, target, "signature_damage2026", "14.2.3.acoustic").is_empty());
        let after = scan(&mut w, observer, &active, target).unwrap();
        assert_eq!(
            after.trace["inputs"]["target_modifier"],
            before.trace["inputs"]["target_modifier"]
        );
        assert_eq!(
            after.trace["inputs"]["modified50_nm"],
            before.trace["inputs"]["modified50_nm"]
        );
        assert_eq!(after.trace["d100"], before.trace["d100"]);
    }
}

#[test]
fn b07_previously_deployed_mobile_decoy_retains_its_independent_saved_signature_after_mother_hit() {
    let (mut w, _, target) = fixture("hv2026-rts-v4", true, Some(2), true, 0., None);
    let state = w.rules.ew.as_mut().unwrap();
    state.config.fits.insert(
        "boundary_mobile".into(),
        ew::Fit {
            source:
                "scenario_assumption: one generic Gen3 mobile simulator, not actual Type093 fit"
                    .into(),
            kind: ew::ObjectKind::MobileDecoy,
            generation: Some(3),
            launcher: Some(ew::Launcher::Internal),
            endurance_s: None,
            deployment_s: None,
            radar_height_m: None,
        },
    );
    state
        .config
        .loadouts
        .entry("cn_093".into())
        .or_default()
        .insert("boundary_mobile".into(), 1);
    ew::apply(
        &mut w,
        target,
        &ew::Action::Deploy {
            fit: "boundary_mobile".into(),
            at: None,
            radar_signature: None,
            course_deg: Some(90.),
            speed_kts: Some(12.),
            depth_m: Some(50.),
        },
    )
    .expect("real source legal simulator deployment");
    let object = w
        .rules
        .ew
        .as_ref()
        .unwrap()
        .objects
        .values()
        .find(|o| o.owner == target)
        .unwrap();
    let id = object.id;
    let origin = object.pos;
    let saved = serde_json::to_value(&object.imitated_profile).unwrap();
    assert_eq!(saved["noise"], 2);
    hit(&mut w, target, WeaponKind::Torpedo);
    assert_eq!(
        signature::project_acoustic(&w, target, Some(2), true).unwrap(),
        (Some(1), false)
    );
    for _ in 0..12 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let object = &w.rules.ew.as_ref().unwrap().objects[&id];
    assert!(object.effective(w.tick));
    assert!(
        object.pos.range_to(origin) > 0.001,
        "real EW lifecycle moves the separately launched object"
    );
    assert_eq!(
        serde_json::to_value(&object.imitated_profile).unwrap(),
        saved,
        "saved simulator metadata is independent; do not infer a real carried coating benefit"
    );
    assert!(signature::own_view(&w, id).is_none());
    assert!(
        !serde_json::to_value(&w).unwrap()["rules"]["signature_damage"]["units"]
            .as_object()
            .unwrap()
            .contains_key(&id.0.to_string())
    );
}

#[test]
fn b08_own_signature_white_list_never_becomes_enemy_contact_bda_or_referee_event() {
    let (mut w, _, target) = fixture("hv2026-rts-v4", true, Some(2), true, 0., None);
    // Author a current ordinary located report. It does not contain damage,
    // noise/coating, class ID or a source threshold report.
    w.sides[0].contacts.insert(
        ContactId(91),
        harpoon_core::world::Contact {
            id: ContactId(91),
            about: target,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 1.5),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: 0,
            stale: false,
            sensors: vec![
                "scenario_assumption: ordinary located report without damage truth".into(),
            ],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    hit(&mut w, target, WeaponKind::Torpedo);
    let red = harpoon_core::observe(&w, Side::Red, 0, 1000);
    let own = red
        .units
        .iter()
        .find(|u| u.id == target)
        .unwrap()
        .signature_damage
        .as_ref()
        .unwrap();
    assert_eq!(own["acoustic"]["effective_noise"], 1);
    assert_eq!(own["acoustic"]["anechoic_benefit"], false);
    let expected_keys = ["acoustic", "pending", "policy", "radar", "source"];
    assert_eq!(
        own.as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        expected_keys
    );
    for forbidden in [
        "hits",
        "hit",
        "weapon",
        "shooter",
        "resolved_dp",
        "threshold_dp",
        "original_dp",
    ] {
        assert!(
            own.get(forbidden).is_none(),
            "own summary is a whitelist, not full evidence"
        );
    }
    let blue = harpoon_core::observe(&w, Side::Blue, 0, 1000);
    assert!(blue.units.iter().all(|u| u.id != target));
    let contact = serde_json::to_value(
        blue.contacts
            .iter()
            .find(|c| c.id == ContactId(91))
            .unwrap(),
    )
    .unwrap();
    assert!(contact.get("signature_damage").is_none());
    assert!(contact.get("noise").is_none());
    assert!(contact.get("anechoic").is_none());
    assert_eq!(contact["bda_hits"], 0);
    assert_eq!(contact["bda_damage"], 0.);
    assert!(contact["bda_hp_fraction"].is_null());
    assert!(contact["class_name"].is_null());
    for observation in [&red, &blue] {
        assert!(observation.events.iter().all(|e|!matches!(&e.kind,EventKind::RulesEvent{domain,..} if domain=="signature_damage2026")));
    }
}

#[test]
fn b09_mismatched_consumer_baselines_fail_readonly_without_reroll_or_state_mutation() {
    let (mut w, _, target) = fixture("hv2026-rts-v4", false, Some(2), true, 0., None);
    hit(&mut w, target, WeaponKind::Torpedo);
    let before = serde_json::to_vec(&w).unwrap();
    assert_eq!(signature::project_radar(&w, target, 2).unwrap(), 1);
    assert_eq!(
        signature::project_acoustic(&w, target, Some(2), true).unwrap(),
        (Some(1), false)
    );
    assert!(signature::project_radar(&w, target, 3).is_err());
    assert!(signature::project_noise(&w, target, 3).is_err());
    assert!(signature::project_acoustic(&w, target, Some(2), false).is_err());
    assert_eq!(serde_json::to_vec(&w).unwrap(),before,
        "all projections are read-only; source conflict cannot consume a new die or mutate damage evidence");
}

//! §8.5.3.3 additional natural two/three-contact source boundaries.
//! Seeds were selected read-only from the existing original RNG identity;
//! actual native FCS, inventory, original die and common damage still execute.
//! All numerical hardware is explicitly authored experimental fit. It tests
//! source algorithms without pretending to certify a real Annex installation.
//! Original native report/FCS, true target and dice are never injected.
use harpoon_core::rules::detection2026;
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};

const ACTIVE: &str = "sonar_a_projector_lab";
const PASSIVE: &str = "projector_lab_passive";
const WEAPON: &str = "lab_projector_contact";

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn run(w: &mut World, until: i64) {
    while w.tick < until {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn fixture(seed: u64, hedgehog: bool, attacks: u8, chance: f64) -> World {
    let sea = 0;
    let wrong_arc = false;
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let mut def = db.weapons["cn_rbu1000"].clone();
    def.id = WEAPON.into();
    def.name = "scenario_assumption: contact pattern algorithm laboratory".into();
    def.pk = 0.;
    def.damage = 999.;
    db.weapons.insert(WEAPON.into(), def);
    let mut mount = db.platforms["cn_956em"]
        .weapons
        .iter()
        .find(|m| m.weapon == "cn_rbu1000")
        .unwrap()
        .clone();
    assert_eq!((mount.mounts, mount.ammo), (2, 4));
    mount.weapon = WEAPON.into();
    let mount_index = db.platforms["us_ford"].weapons.len();
    db.platforms.get_mut("us_ford").unwrap().weapons.push(mount);
    for (name, kind) in [
        (ACTIVE, harpoon_core::db::SensorKind::SonarActive),
        (PASSIVE, harpoon_core::db::SensorKind::SonarPassive),
    ] {
        let mut s = db.sensors.values().next().unwrap().clone();
        s.id = name.into();
        s.name = format!("scenario_assumption:{name}");
        s.kind = kind;
        s.air_range_nm = 0.;
        s.surface_range_nm = 0.;
        s.missile_range_nm = 0.;
        s.sub_range_nm = 16.;
        db.sensors.insert(name.into(), s);
    }
    db.platforms.get_mut("us_ford").unwrap().sensors = vec![ACTIVE.into(), PASSIVE.into()];
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    db.validate().unwrap();
    let receiver = |active| json!({"type":"sonar","source":"scenario_assumption: independent laboratory Gen5 LMF16nm Bow receiver, no real Ford fit claim","generation":5,"passive_range_nm":if active{None}else{Some(16.)},"active_range_nm":if active{Some(16.)}else{None},"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false});
    let value = json!({"id":"next-rgb10-pattern","name":"scenario_assumption: original source Ford CS6 hull and experimental RBU fitting","seed":seed,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{
        "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":sea,"precipitation":"None"},
        "asw_weapons_2026":{"source":"scenario_assumption: source algorithms only; experiment hardware is not a real Hedgehog/RBU/MBU Annex fit","standoffs":{},"projectors":{(WEAPON):{"source":"scenario_assumption: explicit laboratory model and original stock allocation","model":"scenario_assumption","experiment":{"source":"scenario_assumption: contact100 or0 is selected solely to exercise source branches; contactDP20 is the core rule illustrative value, not a real equipment claim","fuzing":"contact","range_nm":0.5,"max_depth_band_index":5,"bombs":24,"contact_percent":chance,"major_percent":null,"minor_percent":null,"damage_contact":20.,"damage_major":null,"damage_minor":null,"loaded_capacity_patterns":3,"attacks_per_tactical_turn":attacks,"hedgehog":hedgehog,"paired_mounts":false,"physical_arcs":["P","S"]},"units":{"LAB":{"source":"scenario_assumption: original Ford body and source CS6, original two956EM mount count/aggregate8 tokens;3loaded+1reserve each, no reload claim","mount_index":mount_index,"mounts":[{"physical":0,"arc":"P","loaded_patterns":3,"reserve_patterns":1},{"physical":1,"arc":"S","loaded_patterns":3,"reserve_patterns":1}],"reload":null}}}},"submarine_hulls":{"TARGET":{"source":"scenario_assumption: explicit laboratory four-band Large/Double/year2000; not a Type093 fit claim","size":"large","design":"double","in_service_year":2000}}},
        "detection_2026":{"bindings":{format!("us_ford:{ACTIVE}"):receiver(true),format!("us_ford:{PASSIVE}"):receiver(false)},"environment":{"source":"scenario_assumption: explicit open-water laboratory","sea_state":sea,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},"targets":{"cn_093":{"source":"scenario_assumption: controlled acoustic signature, original loaded hull DP retained","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
        "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"course_deg":0,"speed_kts":0,"sonar_active":true,"fire_on":false},{"side":"Red","class":"cn_093","name":"TARGET","pos":if wrong_arc{json!([0,0.3])}else{json!([0.3,0])},"course_deg":if wrong_arc{90}else{0},"speed_kts":0,"depth_m":50,"fire_on":false}]});
    let scenario: Scenario = serde_json::from_value(value)
        .expect("next-window source-red checkpoint: strict projector config must become a real engine configuration");
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let actor = id(&w, "LAB");
    let target = id(&w, "TARGET");
    // Acoustic trials cap at90%; a fixed author time cannot guarantee the
    // last epoch succeeded. Wait for actual consecutive native reports rather
    // than injecting success or changing the original deterministic dice.
    while w.tick < 6000 {
        let classified = detection2026::known_category(&w, Side::Blue, target)
            == Some(harpoon_core::types::PlatformKind::Sub);
        let located = w.sides[0]
            .contacts
            .values()
            .any(|c| c.about == target && !c.stale && !c.bearing_only);
        if classified
            && located
            && harpoon_core::rules::firecontrol2026::own_projector_solution(
                &w, actor, target, ACTIVE,
            )
            .is_ok_and(|s| s.quality == harpoon_core::rules::firecontrol2026::Quality::Good)
        {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(
        detection2026::known_category(&w, Side::Blue, target),
        Some(harpoon_core::types::PlatformKind::Sub),
        "actual passive held-contact classification must precede the attack"
    );
    assert!(
        w.rules
            .aaw
            .as_ref()
            .unwrap()
            .firecontrol
            .tracks
            .values()
            .any(|t| {
                t.observer == actor
                    && t.target == target
                    && t.sensor == ACTIVE
                    && t.report
                        .as_ref()
                        .is_some_and(|r| r.measured_position.is_some())
                    && t.motion.as_ref().is_some_and(|m| m.known)
            }),
        "actual measured same-receiver motion is required; no author FCS quality"
    );
    // Stay clear of the next source Tactical-Turn boundary using ordinary
    // simulation and a genuinely held solution, not a private clock rewrite.
    while w.tick.rem_euclid(180) > 165 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    w
}
fn contact(w: &World) -> ContactId {
    let target = id(w, "TARGET");
    w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == target && !c.stale && !c.bearing_only)
        .expect("actual native located CID required")
        .id
}
fn ammo(w: &World) -> u16 {
    w.entity(id(w, "LAB"))
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == WEAPON)
        .unwrap()
        .ammo
}
fn declaration(w: &World, phase: &str) -> Command {
    serde_json::from_value(json!({"cmd":"asw","unit":id(w,"LAB"),"action":{
        "type":"projector_pattern","contact":contact(w),"weapon":WEAPON,
        "physical":1,"phase":phase
    }})).expect("8.5.3 source pattern action must preserve explicit physical mount and Planned/Reaction declaration")
}
fn release(w: &mut World, phase: &str) {
    let c = declaration(w, phase);
    c.validate_source(w, Side::Blue)
        .expect("source-qualified original native projectile pattern");
    harpoon_core::sim::tick(w, &[(Side::Blue, c)]);
}
fn results(w: &World) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "8.5.3.pattern_result" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn direct_contact_ids(w: &World) -> std::collections::BTreeSet<String> {
    results(w)
        .iter()
        .flat_map(|r| {
            r["contact_outcomes"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|c| c["identity"].as_str().map(str::to_owned))
        .collect()
}
fn common_hits(w: &World) -> Vec<Value> {
    let direct = direct_contact_ids(w);
    let target = id(w, "TARGET");
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule, unit, detail, ..
            } if rule == "hit_resolved" && *unit == Some(target) => {
                let v: Value = serde_json::from_str(detail).unwrap();
                (v["weapon"] == WEAPON && v["hit"].as_str().is_some_and(|h| direct.contains(h)))
                    .then_some(v)
            }
            _ => None,
        })
        .collect()
}
fn pressure_trials(w: &World) -> Vec<Value> {
    let direct = direct_contact_ids(w);
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "8.5.contact_pressure_hull" => {
                let v: Value = serde_json::from_str(detail).unwrap();
                v["hit"]
                    .as_str()
                    .is_some_and(|h| direct.contains(h))
                    .then_some(v)
            }
            _ => None,
        })
        .collect()
}
fn natural_contacts(seed: u64, raw: u64, count: usize) {
    // This ordinary seed is the only branch selection. The original World,
    // native reports, FCS, random stream and weapon results are never edited.
    let mut w = fixture(seed, true, 1, 100.);
    let actor = id(&w, "LAB");
    let target = id(&w, "TARGET");
    let original_stock = ammo(&w);
    let loaded_before = harpoon_core::rules::asw_weapons2026::projector_own_view(&w, actor)[WEAPON]
        ["mounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["physical"] == 1)
        .unwrap()["loaded_patterns"]
        .as_u64()
        .unwrap();
    release(&mut w, "planned");
    let rs = results(&w);
    assert_eq!(rs.len(), 1);
    assert_eq!(rs[0]["raw_d100"].as_u64(),Some(raw),"ordinary preselected seed must reproduce its original pattern die; actual release tick={}, pattern={}",w.tick,rs[0]);
    assert_eq!(
        rs[0]["quality"], "Good",
        "actual original same-receiver native FCS, never an author quality insertion"
    );
    assert_eq!(rs[0]["thresholds"]["contact"], 100);
    assert_eq!(rs[0]["contact_hits"].as_u64(), Some(count as u64));
    assert_eq!(rs[0]["dp"], 20. * count as f64);
    assert_eq!(
        ammo(&w),
        original_stock - 1,
        "several hitting charges still spend exactly one complete physical pattern"
    );
    let loaded_after = harpoon_core::rules::asw_weapons2026::projector_own_view(&w, actor)[WEAPON]
        ["mounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["physical"] == 1)
        .unwrap()["loaded_patterns"]
        .as_u64()
        .unwrap();
    assert_eq!(loaded_after, loaded_before - 1);
    let contacts = rs[0]["contact_outcomes"]
        .as_array()
        .expect("true subhit outcomes retained by original pattern");
    assert_eq!(
        contacts.len(),
        count,
        "all actual contacts must reach the common consumer; pattern={}",
        rs[0]
    );
    assert!(contacts.iter().all(|c| c["damage_queued"] == true
        && c["minor_damage_queued"] == false
        && c["pending_reason"].is_null()));
    let hits = common_hits(&w);
    assert_eq!(
        hits.len(),
        count,
        "each original subhit queues real ContactDP, pattern={}",
        rs[0]
    );
    assert!(hits.iter().all(|h| h["dp"] == 20.));
    let trials = pressure_trials(&w);
    assert_eq!(trials.len(), count);
    let identities = direct_contact_ids(&w);
    assert_eq!(
        identities.len(),
        count,
        "independent physical subhits have different common identities"
    );
    for contact in contacts {
        let identity = contact["identity"].as_str().unwrap();
        let trial: Vec<_> = trials.iter().filter(|p| p["hit"] == identity).collect();
        assert_eq!(
            trial.len(),
            1,
            "each contact has exactly its own pressure trial"
        );
        assert_eq!(trial[0]["chance_percent"], 5.);
        assert!(trial[0]["roll"]
            .as_u64()
            .is_some_and(|v| (1..=100).contains(&v)));
        assert_eq!(contact["pressure_roll"], trial[0]["roll"]);
        assert_eq!(contact["penetrated"], trial[0]["penetrated"]);
        assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,unit,detail,..} if rule=="hit_resolved"&&*unit==Some(target)&&serde_json::from_str::<Value>(detail).is_ok_and(|h|h["hit"]==identity&&h["dp"]==20.))));
    }
    // One bounded no-time-advance check verifies the stored producer result;
    // it is not a replay and cannot acquire a new report or supply a new die.
    harpoon_core::rules::asw_weapons2026::resolve_projector_patterns(&mut w);
    assert_eq!(ammo(&w), original_stock - 1);
    assert_eq!(common_hits(&w).len(), count);
    assert_eq!(pressure_trials(&w).len(), count);
}
#[test]
fn h04_natural_two_contact_pattern_has_two_real_twenty_dp_hits_and_independent_pressure_trials() {
    // integration17: actual native FCS first qualifies at tick181; release182 D10040.
    natural_contacts(20261030, 40, 2);
}
#[test]
fn h05_natural_three_contact_pattern_has_three_real_twenty_dp_hits_and_independent_pressure_trials()
{
    // integration17: actual FCS waits through the real sensing history; release541/epoch3 D10015.
    natural_contacts(20261028, 15, 3);
}

//! Source-only §8.5.3 paired-projector pattern and one-mount half-chance boundaries.
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
fn fixture(full_arcs: bool) -> World {
    let seed = 20261030;
    let hedgehog = false;
    let attacks = 1;
    let chance = if full_arcs { 100. } else { 20. };
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
        "asw_weapons_2026":{"source":"scenario_assumption: source algorithms only; experiment hardware is not a real Hedgehog/RBU/MBU Annex fit","standoffs":{},"projectors":{(WEAPON):{"source":"scenario_assumption: explicit laboratory model and original stock allocation","model":"scenario_assumption","experiment":{"source":"scenario_assumption: independently authored identical paired hardware,24 bombs per complete pair/12 per mount; Contact100 isolates the complete pair; half-pair uses authorC/H20/60/100 with illustrative20/6/3DP, not a real equipment claim","fuzing":if full_arcs{"contact"}else{"contact_hydrostatic"},"range_nm":0.5,"max_depth_band_index":5,"bombs":24,"contact_percent":chance,"major_percent":if full_arcs{None}else{Some(60.)},"minor_percent":if full_arcs{None}else{Some(100.)},"damage_contact":20.,"damage_major":if full_arcs{None}else{Some(6.)},"damage_minor":if full_arcs{None}else{Some(3.)},"loaded_capacity_patterns":3,"attacks_per_tactical_turn":attacks,"hedgehog":hedgehog,"paired_mounts":true,"physical_arcs":if full_arcs{json!(["F","F"])}else{json!(["P","S"])}},"units":{"LAB":{"source":"scenario_assumption: original Ford body and source CS6, original two956EM mount count/aggregate8 physical mount-salvo tokens;3loaded+1reserve per actual mount, pair firing spends one token from each, no real equipment/reload claim","mount_index":mount_index,"mounts":[{"physical":0,"arc":if full_arcs{"F"}else{"P"},"loaded_patterns":3,"reserve_patterns":1},{"physical":1,"arc":if full_arcs{"F"}else{"S"},"loaded_patterns":3,"reserve_patterns":1}],"reload":null}}}},"submarine_hulls":{"TARGET":{"source":"scenario_assumption: explicit laboratory four-band Large/Double/year2000; not a Type093 fit claim","size":"large","design":"double","in_service_year":2000}}},
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
fn pair_declaration(w: &World, physicals: &[usize]) -> Command {
    serde_json::from_value(json!({"cmd":"asw","unit":id(w,"LAB"),"action":{
        "type":"projector_pair_pattern","contact":contact(w),"weapon":WEAPON,
        "physicals":physicals,"phase":"planned"
    }}))
    .expect("source paired pattern must preserve its selected actual physical mount set")
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
fn loaded(w: &World, physical: usize) -> u64 {
    harpoon_core::rules::asw_weapons2026::projector_own_view(w, id(w, "LAB"))[WEAPON]["mounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["physical"].as_u64() == Some(physical as u64))
        .unwrap()["loaded_patterns"]
        .as_u64()
        .unwrap()
}
#[test]
fn q01_both_eligible_paired_mounts_fire_one_complete_pattern_with_two_real_salvo_debits_and_one_die(
) {
    let mut w = fixture(true);
    let stock = ammo(&w);
    let before = [loaded(&w, 0), loaded(&w, 1)];
    let omit = pair_declaration(&w, &[1]);
    assert!(omit.validate_source(&w,Side::Blue).is_err(),"if both source-paired mounts can fire, one cannot voluntarily omit a healthy loaded in-arc partner");
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, omit)]);
    assert_eq!(ammo(&w), stock);
    assert!(results(&w).is_empty());
    let c = pair_declaration(&w, &[1, 0]);
    c.validate_source(&w, Side::Blue)
        .expect("both original loaded mounts can fire the actual complete pair");
    // Preserve the actual submitted own CID. A genuine Contact/pressure
    // casualty can stale the target book before this retransmission check.
    let extra = c.clone();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, c)]);
    assert_eq!(
        ammo(&w),
        stock - 2,
        "one physical salvo from each required mount, not free paired charges"
    );
    assert_eq!(
        [loaded(&w, 0), loaded(&w, 1)],
        [before[0] - 1, before[1] - 1]
    );
    let rs = results(&w);
    assert_eq!(
        rs.len(),
        1,
        "the two mounts form one pattern and one original D100"
    );
    assert_eq!(
        rs[0]["thresholds"]["contact"], 100,
        "complete pair uses the listed source probability"
    );
    assert!(rs[0]["raw_d100"]
        .as_u64()
        .is_some_and(|d| (1..=100).contains(&d)));
    let outcomes = rs[0]["contact_outcomes"].as_array().unwrap();
    assert_eq!(
        outcomes.len(),
        1,
        "no second independently adjudicated single-mount attack"
    );
    let identity = outcomes[0]["identity"].as_str().unwrap();
    let target = id(&w, "TARGET");
    let dp: Vec<_> = w
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule, unit, detail, ..
            } if rule == "hit_resolved" && *unit == Some(target) => {
                let h: Value = serde_json::from_str(detail).unwrap();
                (h["hit"] == identity).then_some(h)
            }
            _ => None,
        })
        .collect();
    assert_eq!(dp.len(), 1);
    assert_eq!(
        dp[0]["dp"], 20.,
        "a complete paired pattern inflicts listed ContactDP once"
    );
    assert_eq!(
        harpoon_core::rules::asw_weapons2026::projector_own_view(&w, id(&w, "LAB"))[WEAPON]
            ["pattern_used_this_turn"],
        true,
        "the actual logical pair budget is consumed independently of target survival"
    );
    eprintln!("paired q01 after actual common contact: tick={}, target_alive={:?}, raw_d100={}, pressure={}",w.tick,w.entity(target).map(|e|e.alive),rs[0]["raw_d100"],outcomes[0]["penetrated"]);
    assert!(
        extra.validate_source(&w, Side::Blue).is_err(),
        "the source pair has already used its one system pattern this Tactical Turn"
    );
    assert_eq!(ammo(&w), stock - 2);
}
#[test]
fn q02_only_one_paired_mount_in_source_arc_fires_half_chance_with_atomic_rejection_of_unavailable_partner(
) {
    let mut w = fixture(false);
    let stock = ammo(&w);
    let before = [loaded(&w, 0), loaded(&w, 1)];
    // Original target is to starboard. P mount0 cannot fire, S mount1 can.
    let bad = pair_declaration(&w, &[0, 1]);
    assert!(bad.validate_source(&w, Side::Blue).is_err());
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, bad)]);
    assert_eq!(
        ammo(&w),
        stock,
        "a rejected pair never debits its first available mount"
    );
    assert_eq!([loaded(&w, 0), loaded(&w, 1)], before);
    assert!(results(&w).is_empty());
    let c = pair_declaration(&w, &[1]);
    c.validate_source(&w, Side::Blue).expect(
        "only one actual source-paired mount can fire; half chance is the printed fallback",
    );
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, c)]);
    assert_eq!(ammo(&w), stock - 1);
    assert_eq!([loaded(&w, 0), loaded(&w, 1)], [before[0], before[1] - 1]);
    let rs = results(&w);
    assert_eq!(rs.len(), 1);
    assert_eq!(
        rs[0]["thresholds"]["contact"], 10,
        "source half-pair factor applies to the actual pattern Ph"
    );
    assert_eq!(rs[0]["thresholds"]["major"], 30);
    assert_eq!(rs[0]["thresholds"]["minor"], 50);
    let d = rs[0]["raw_d100"]
        .as_u64()
        .expect("one actual original pattern die");
    assert_eq!(rs[0]["contact_hits"].as_u64(), Some(u64::from(d <= 10)));
    assert_eq!(
        rs[0]["severity"],
        if d <= 10 {
            "contact"
        } else if d <= 30 {
            "major"
        } else if d <= 50 {
            "minor"
        } else {
            "miss"
        }
    );
    if d <= 50 {
        assert_eq!(
            rs[0]["dp"],
            if d <= 10 {
                20.
            } else if d <= 30 {
                6.
            } else {
                3.
            },
            "half chance does not halve the listed per-hit damage"
        );
    }
    let extra = pair_declaration(&w, &[1]);
    assert!(extra.validate_source(&w, Side::Blue).is_err());
    assert_eq!(ammo(&w), stock - 1);
}

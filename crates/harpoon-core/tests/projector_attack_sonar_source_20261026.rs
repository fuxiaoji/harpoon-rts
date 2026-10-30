//! Source-only §8.5.3.1 dedicated early attack-sonar boundaries.
//! All numerical hardware is explicitly authored experimental fit. It tests
//! source algorithms without pretending to certify a real Annex installation.
//! Original native report/FCS, true target and dice are never injected.
use harpoon_core::rules::detection2026;
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};

const ACTIVE: &str = "sonar_a_projector_lab";
const PASSIVE: &str = "projector_lab_passive";
const ATTACK: &str = "sonar_a_projector_attack_lab";
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
fn fixture(with_search: bool) -> World {
    let seed = 20261028;
    let hedgehog = false;
    let attacks = 1;
    let chance = 100.;
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
        (ATTACK, harpoon_core::db::SensorKind::SonarActive),
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
    db.platforms.get_mut("us_ford").unwrap().sensors = if with_search {
        vec![ACTIVE.into(), PASSIVE.into(), ATTACK.into()]
    } else {
        vec![ATTACK.into()]
    };
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    db.validate().unwrap();
    let receiver = |active, generation| json!({"type":"sonar","source":format!("scenario_assumption: independently installed laboratory Gen{generation} LMF16nm Bow receiver active={active}; no real Ford fit claim"),"generation":generation,"passive_range_nm":if active{None}else{Some(16.)},"active_range_nm":if active{Some(16.)}else{None},"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false});
    let mut bindings = serde_json::Map::new();
    bindings.insert(format!("us_ford:{ATTACK}"), receiver(true, 2));
    if with_search {
        bindings.insert(format!("us_ford:{ACTIVE}"), receiver(true, 3));
        bindings.insert(format!("us_ford:{PASSIVE}"), receiver(false, 3));
    }
    let value = json!({"id":"next-rgb10-pattern","name":"scenario_assumption: original source Ford CS6 hull and experimental RBU fitting","seed":seed,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{
        "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":sea,"precipitation":"None"},
        "asw_weapons_2026":{"source":"scenario_assumption: source algorithms only; experiment hardware is not a real Hedgehog/RBU/MBU Annex fit","standoffs":{},"projectors":{(WEAPON):{"source":"scenario_assumption: explicit laboratory model and original stock allocation","model":"scenario_assumption","experiment":{"source":"scenario_assumption: contact100 or0 is selected solely to exercise source branches; contactDP20 is the core rule illustrative value, not a real equipment claim","fuzing":"contact","range_nm":0.5,"max_depth_band_index":5,"bombs":24,"contact_percent":chance,"major_percent":null,"minor_percent":null,"damage_contact":20.,"damage_major":null,"damage_minor":null,"loaded_capacity_patterns":3,"attacks_per_tactical_turn":attacks,"hedgehog":hedgehog,"paired_mounts":false,"physical_arcs":["P","S"]},"units":{"LAB":{"source":"scenario_assumption: original Ford body and source CS6, original two956EM mount count/aggregate8 tokens;3loaded+1reserve each, no reload claim","mount_index":mount_index,"attack_sensor":ATTACK,"mounts":[{"physical":0,"arc":"P","loaded_patterns":3,"reserve_patterns":1},{"physical":1,"arc":"S","loaded_patterns":3,"reserve_patterns":1}],"reload":null}}}},"submarine_hulls":{"TARGET":{"source":"scenario_assumption: explicit laboratory four-band Large/Double/year2000; not a Type093 fit claim","size":"large","design":"double","in_service_year":2000}}},
        "detection_2026":{"bindings":bindings,"environment":{"source":"scenario_assumption: explicit open-water laboratory","sea_state":sea,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},"targets":{"cn_093":{"source":"scenario_assumption: controlled acoustic signature, original loaded hull DP retained","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
        "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"course_deg":0,"speed_kts":0,"sonar_active":true,"fire_on":false},{"side":"Red","class":"cn_093","name":"TARGET","pos":if wrong_arc{json!([0,0.3])}else{json!([0.3,0])},"course_deg":if wrong_arc{90}else{0},"speed_kts":0,"depth_m":50,"fire_on":false}]});
    let scenario: Scenario = serde_json::from_value(value)
        .expect("next-window source-red checkpoint: strict projector config must become a real engine configuration");
    harpoon_core::load_scenario(scenario, db)
        .expect("original installed dedicated attack receiver must bind the projector hardware")
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
fn native_attack(w: &World) -> bool {
    let actor = id(w, "LAB");
    let target = id(w, "TARGET");
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
                && t.sensor == ATTACK
                && t.report
                    .as_ref()
                    .is_some_and(|r| r.measured_position.is_some())
        })
}
#[test]
fn a01_an_installed_active_gen2_attack_sonar_cannot_search_or_create_its_own_initial_contact() {
    let mut w = fixture(false);
    let actor = id(&w, "LAB");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSonar {
                unit: actor,
                active: true,
            },
        )],
    );
    assert!(w
        .entity(actor)
        .unwrap()
        .sensors
        .iter()
        .any(|s| s.sensor == ATTACK && s.active));
    run(&mut w, 181);
    assert!(w.sides[0].contacts.is_empty(),"8.5.3.1: dedicated attack sonar performs no search, even though the original installed emitter is active");
    assert!(
        !native_attack(&w),
        "no actual located seed and no selected target means no dedicated track/FCS"
    );
    assert!(w.entity(actor).unwrap().assigned.is_none());
}
#[test]
fn a02_gen3_search_location_then_actual_assigned_gen2_attack_tracking_forms_source_fcs_before_real_release(
) {
    let mut w = fixture(true);
    let actor = id(&w, "LAB");
    let target = id(&w, "TARGET");
    while w.tick < 360
        && !w.sides[0]
            .contacts
            .values()
            .any(|c| c.about == target && !c.stale && !c.bearing_only)
    {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let cid = contact(&w);
    assert!(
        !native_attack(&w),
        "dedicated receiver must not independently search before selection"
    );
    assert!(
        harpoon_core::rules::firecontrol2026::own_projector_solution(&w, actor, target, ACTIVE)
            .is_err(),
        "early Gen3 search sonar still cannot supply projector firing data"
    );
    let c = Command::AssignTarget {
        unit: actor,
        contact: cid,
    };
    c.validate_source(&w, Side::Blue)
        .expect("ordinary own located contact selection");
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, c)]);
    assert!(
        harpoon_core::rules::firecontrol2026::own_projector_solution(&w, actor, target, ATTACK)
            .is_err(),
        "selection or an initial native sample cannot manufacture completed motion/Good FCS"
    );
    let early = declaration(&w, "planned");
    assert!(
        early.validate_source(&w, Side::Blue).is_err(),
        "unknown dedicated firing solution cannot spend a pattern"
    );
    let start = w.tick;
    while w.tick < start + 900
        && !harpoon_core::rules::firecontrol2026::own_projector_solution(&w, actor, target, ATTACK)
            .is_ok_and(|s| s.quality == harpoon_core::rules::firecontrol2026::Quality::Good)
    {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let solution=harpoon_core::rules::firecontrol2026::own_projector_solution(&w,actor,target,ATTACK).expect("actual dedicated same-receiver position/time geometry must form the source acoustic solution");
    assert_eq!(
        solution.quality,
        harpoon_core::rules::firecontrol2026::Quality::Good
    );
    assert!(native_attack(&w));
    assert!(
        harpoon_core::rules::firecontrol2026::own_projector_solution(&w, actor, target, ACTIVE)
            .is_err(),
        "the Gen4 search exception remains a distinct gate"
    );
    let stock = ammo(&w);
    release(&mut w, "planned");
    assert_eq!(
        ammo(&w),
        stock - 1,
        "real installed dedicated sonar admits one actually spent pattern"
    );
    let rs = results(&w);
    assert_eq!(rs.len(), 1);
    assert_eq!(rs[0]["source_fcs"]["sensor"], ATTACK);
    assert_eq!(rs[0]["source_fcs"]["report"]["observer"], actor.0);
    assert_eq!(rs[0]["source_fcs"]["motion"]["known"], true);
    assert_eq!(rs[0]["quality"], "Good");
}

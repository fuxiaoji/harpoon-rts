//! Source-only §2.5 HITL no Reaction Fire through explicit §8.5.3 phases.
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
fn fixture(unmanned: bool) -> World {
    let (seed, hedgehog, attacks, chance) = (20261025u64, false, 2u8, 0.);
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
    let mut control = db.platforms["us_ford"].clone();
    control.id = "lab_unmanned_control_ship".into();
    control.name =
        "scenario_assumption: independent surface control body, no real fit claim".into();
    control.weapons.clear();
    control.sensors.clear();
    control.deck = None;
    db.platforms.insert(control.id.clone(), control);
    db.validate().unwrap();
    let receiver = |active| json!({"type":"sonar","source":"scenario_assumption: independent laboratory Gen5 LMF16nm Bow receiver, no real Ford fit claim","generation":5,"passive_range_nm":if active{None}else{Some(16.)},"active_range_nm":if active{Some(16.)}else{None},"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false});
    let mut value = json!({"id":"next-rgb10-pattern","name":"scenario_assumption: original source Ford CS6 hull and experimental RBU fitting","seed":seed,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{
        "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":sea,"precipitation":"None"},
        "asw_weapons_2026":{"source":"scenario_assumption: source algorithms only; experiment hardware is not a real Hedgehog/RBU/MBU Annex fit","standoffs":{},"projectors":{(WEAPON):{"source":"scenario_assumption: explicit laboratory model and original stock allocation","model":"scenario_assumption","experiment":{"source":"scenario_assumption: contact100 or0 is selected solely to exercise source branches; contactDP20 is the core rule illustrative value, not a real equipment claim","fuzing":"contact","range_nm":0.5,"max_depth_band_index":5,"bombs":24,"contact_percent":chance,"major_percent":null,"minor_percent":null,"damage_contact":20.,"damage_major":null,"damage_minor":null,"loaded_capacity_patterns":3,"attacks_per_tactical_turn":attacks,"hedgehog":hedgehog,"paired_mounts":false,"physical_arcs":["P","S"]},"units":{"LAB":{"source":"scenario_assumption: original Ford body and source CS6, original two956EM mount count/aggregate8 tokens;3loaded+1reserve each, no reload claim","mount_index":mount_index,"mounts":[{"physical":0,"arc":"P","loaded_patterns":3,"reserve_patterns":1},{"physical":1,"arc":"S","loaded_patterns":3,"reserve_patterns":1}],"reload":null}}}},"submarine_hulls":{"TARGET":{"source":"scenario_assumption: explicit laboratory four-band Large/Double/year2000; not a Type093 fit claim","size":"large","design":"double","in_service_year":2000}}},
        "detection_2026":{"bindings":{format!("us_ford:{ACTIVE}"):receiver(true),format!("us_ford:{PASSIVE}"):receiver(false)},"environment":{"source":"scenario_assumption: explicit open-water laboratory","sea_state":sea,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},"targets":{"cn_093":{"source":"scenario_assumption: controlled acoustic signature, original loaded hull DP retained","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
        "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"course_deg":0,"speed_kts":0,"sonar_active":true,"fire_on":false},{"side":"Red","class":"cn_093","name":"TARGET","pos":if wrong_arc{json!([0,0.3])}else{json!([0.3,0])},"course_deg":if wrong_arc{90}else{0},"speed_kts":0,"depth_m":50,"fire_on":false}]});
    // Keep the same independent controller body in both scenarios, including
    // the manned control case. Every link/hardware/endurance value is authored.
    value["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":"lab_unmanned_control_ship","name":"CONTROL","pos":[0,-1],"speed_kts":0,"fire_on":false}));
    if unmanned {
        value["context"]["unmanned_2026"] = json!({
            "source":"scenario_assumption: explicit original-body USV control experiment; not a real remotely operated Ford/Annex fit",
            "controllers":{"CONTROL":{"source":"scenario_assumption: installed one-element laboratory control terminal","elements":["E1"],"operational":true,"los_range_nm":10.,"antenna_height_m":10.,"satellite":false,"satellite_available":false,"acoustic_modem":false,"overt_recall_transmitter":false}},
            "vehicles":{"LAB":{"source":"scenario_assumption: source actor body/hardware held unchanged, experimental surface HITL control","domain":"surface","autonomous_capable":true,"initial_mode":"hitl","controller":"CONTROL","element":"E1","initial_link":"line_of_sight","los_range_nm":10.,"antenna_height_m":10.,"satellite":false,"satellite_available":false,"acoustic_modem":false,"periscope_depth_m":null,"endurance_s":10000.,"loss_behavior":{"type":"return_to_home","home":"CONTROL"}}}
        });
    }
    let scenario: Scenario = serde_json::from_value(value)
        .expect("next-window source-red checkpoint: strict projector config must become a real engine configuration");
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let actor = id(&w, "LAB");
    let target = id(&w, "TARGET");
    if unmanned {
        assert!(harpoon_core::rules::unmanned2026::is_configured(&w, actor));
        harpoon_core::rules::unmanned2026::can_control(&w, Side::Blue, actor).expect(
            "actual original controller/HITL/LOS hardware must qualify before native acquisition",
        );
        assert!(harpoon_core::rules::unmanned2026::can_attack(&w, actor));
        assert!(!harpoon_core::rules::unmanned2026::can_reaction_fire(
            &w, actor
        ));
    }
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
#[test]
fn ur01_manned_reaction_control_and_real_hitl_planned_pattern_do_not_grant_unmanned_reaction_admission(
) {
    // Run the real manned phase control first. The restriction must not erase
    // §8.5.3's legitimate second attack in Reaction for a manned source ship.
    let mut manned = fixture(false);
    let stock = ammo(&manned);
    let epoch = manned.tick.div_euclid(180);
    release(&mut manned, "planned");
    release(&mut manned, "reaction");
    assert_eq!(ammo(&manned), stock - 2);
    assert_eq!(manned.tick.div_euclid(180), epoch);
    assert_eq!(
        results(&manned)
            .iter()
            .filter(|r| r["fire_phase"] == "reaction")
            .count(),
        1
    );
    let mut w = fixture(true);
    let actor = id(&w, "LAB");
    let stock = ammo(&w);
    release(&mut w, "planned");
    assert_eq!(
        ammo(&w),
        stock - 1,
        "HITL really supports source Planned combat"
    );
    assert_eq!(results(&w).len(), 1);
    assert_eq!(results(&w)[0]["fire_phase"], "planned");
    harpoon_core::rules::unmanned2026::can_control(&w, Side::Blue, actor).unwrap();
    assert!(
        harpoon_core::rules::unmanned2026::can_attack(&w, actor),
        "generic controlled-combat gate remains genuinely open"
    );
    assert!(!harpoon_core::rules::unmanned2026::can_reaction_fire(
        &w, actor
    ));
    let reaction = declaration(&w, "reaction");
    let original = serde_json::to_vec(&w).unwrap();
    assert!(
        reaction.validate_source(&w, Side::Blue).is_err(),
        "2.5 actual HITL may execute Planned attacks but cannot use explicit8.5.3 Reaction phase"
    );
    assert!(
        serde_json::to_vec(&w).unwrap() == original,
        "readonly phase refusal cannot alter control/FCS/stock/dice"
    );
}
#[test]
fn ur02_actual_hitl_reaction_submit_cannot_spend_a_second_physical_pattern_or_commit_a_reaction() {
    let mut w = fixture(true);
    let actor = id(&w, "LAB");
    let epoch = w.tick.div_euclid(180);
    release(&mut w, "planned");
    assert_eq!(results(&w).len(), 1);
    assert_eq!(results(&w)[0]["fire_phase"], "planned");
    harpoon_core::rules::unmanned2026::can_control(&w, Side::Blue, actor).unwrap();
    assert!(harpoon_core::rules::unmanned2026::can_attack(&w, actor));
    let stock = ammo(&w);
    let seq = w.event_seq;
    let reaction = declaration(&w, "reaction");
    // Use the ordinary runtime submission without relying on caller preflight.
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, reaction)]);
    assert_eq!(
        w.tick.div_euclid(180),
        epoch,
        "the real scenario must stay in its original Tactical Turn"
    );
    assert_eq!(
        ammo(&w),
        stock,
        "2.5 runtime Reaction refusal must precede physical pattern debit"
    );
    assert_eq!(
        results(&w).len(),
        1,
        "only the actual legal Planned pattern may resolve"
    );
    assert!(!w.events.iter().any(|e| e.seq > seq
        && matches!(&e.kind,EventKind::WeaponsRelease{shooter,..} if *shooter==actor)));
    assert!(!w.events.iter().any(|e|e.seq>seq&&matches!(&e.kind,EventKind::RulesEvent{rule,unit,..} if rule=="8.5.3.pattern_committed"&&*unit==Some(actor))));
}

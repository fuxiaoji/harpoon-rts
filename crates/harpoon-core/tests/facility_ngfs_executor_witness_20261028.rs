//! ZCODE-FACILITY-NEXT-02 candidate C2 — 11.4 naval gunfire vs land targets:
//! witness that the source spotting producer is live while the gunnery
//! executor consumer is the registered missing piece
//! ("11 source gunnery/air-ordnance executor not yet bound; legacy
//! range-fraction Pk forbidden", weapons.rs::source_land_launch_ready).
//!
//! Source: English2026 PDF116/11-1 11.4 (all spotting clauses), PDF117/11-2
//! 11.6/11.7 reverse-hit scope, PDF91-93 8.3 tables (root renders, personally
//! cross-checked; transcription in eight-three-land-gunfire-20261028.json).
//! No natural weapon hit occurs in this file: the fire command is expected to
//! be rejected until the root authorizes the source 11.4 executor. No HP,
//! disabled, inventory or critical state is injected anywhere.
//!
//! Not compiled/run/accepted. Scenario seed 26 is selected before
//! construction; if the native lookout/classification rolls need calibration,
//! only the seed changes in the source-red batch (documented convention).

use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use harpoon_core::rules::detection2026 as detection;
use harpoon_core::rules::ground2026 as ground;

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

fn configuration() -> Scenario {
    let scenario = serde_json::json!({
     "id":"facility-ngfs-witness-source-lab",
     "name":"scenario_assumption: 11.4 NGFS executor witness with source spotting producer",
     "seed":(0..100000).find(|seed| detection::trial(&mut detection::State::default(),*seed,1,"5.8.visual:1:visual",180).0 <= 5).unwrap(),"duration_s":20000,
     "weather":{"visibility_nm":10,"jam":[0,0]},
     "context":{"rules_profile":"hv2026-rts-v4",
      "detection_2026":{"bindings":{"cn_052d:visual":{"type":"visual","source":"scenario_assumption: fitted native controlled-range lookout"}},"targets":{
       "cn_052d":{"source":"scenario_assumption: native controlled observer profile","size":1,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2},
       "airfield":{"source":"scenario_assumption: explicit E visual structure profile","size":4,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}},
       "environment":{"source":"scenario_assumption: clear daylight controlled range","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
      "ground_2026":{"source":"scenario_assumption: explicit experimental target facility and observer team",
       "facilities":{"TARGET":{"source":"English2026 PDF118 Magazine Bunker, experimental placement","template":"magazine_bunker"}},
       "observers":{"OBS":{"source":"scenario_assumption: experimental gunnery observer team","radius_nm":5.0,"beacon":true}}}},
     "units":[
      {"side":"Blue","class":"cn_052d","name":"SHIP","pos":[0,0],"speed_kts":0,"fire_on":false},
      {"side":"Blue","class":"airfield","name":"OBS","pos":[2.5,0],"fire_on":false},
      {"side":"Red","class":"airfield","name":"TARGET","pos":[2,0],"fire_on":false}]});
    serde_json::from_value(scenario).unwrap()
}

fn load() -> World {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    db.platforms.get_mut("cn_052d").unwrap().sensors=vec!["visual".into()];
    db.platforms.get_mut("airfield").unwrap().sensors.clear();
    harpoon_core::load_scenario(configuration(), db)
        .expect("explicit source facility configuration must load without ignored fields")
}

fn step(w: &mut World, commands: &[(Side, Command)]) {
    harpoon_core::sim::tick(w, commands);
}

fn advance(w: &mut World, ticks: i64) {
    for _ in 0..ticks {
        step(w, &[]);
    }
}

/// The only contact channel in this file: a real native lookout on the Blue
/// ship against the stationary Red facility. No contact/FCS injection.
fn visual_contact(w: &World) -> harpoon_core::ContactId {
    let target = id(w, "TARGET");
    w.sides[Side::Blue.index()]
        .contacts
        .values()
        .find(|c| c.about == target && !c.stale && c.sensors.iter().any(|s| s == "visual"))
        .expect("seed 26 must give the lookout a fresh visual contact on the facility; recalibrate seed only")
        .id
}

#[test]
fn g1_native_lookout_earns_visual_contact_and_base_category() {
    let mut w = load();
    advance(&mut w, 60);
    let contact = visual_contact(&w);
    let target = id(&w, "TARGET");
    assert!(
        detection::known_category(&w, Side::Blue, target).is_some(),
        "11.4 direct destruction fire needs an established native category; fixture premise"
    );
    // Validation is read-only whatever it decides: the source precheck must
    // not mutate the world. (Whether Fire itself passes source validation
    // before the executor exists is batch information, not a fixture gate.)
    let fire = Command::Fire {
        unit: id(&w, "SHIP"),
        contact,
        weapon: "cn_pj45a".into(),
        rounds: 1,
    };
    let before = serde_json::to_vec(&w).unwrap();
    let _ = fire.validate_source(&w, Side::Blue);
    assert_eq!(before, serde_json::to_vec(&w).unwrap());
}

#[test]
fn g2_public_spot_command_records_the_source_ship_spotting_task() {
    let mut w = load();
    advance(&mut w, 60);
    let ship = id(&w, "SHIP");
    let contact = visual_contact(&w);
    let cmd = Command::Ground {
        unit: ship,
        action: ground::Action::Spot {
            contact,
            method: ground::Spotting::Ship,
        },
    };
    // The public JSON schema must accept exactly this action shape.
    let value = serde_json::to_value(&cmd).unwrap();
    let parsed: Command = serde_json::from_value(value).unwrap();
    step(&mut w, &[(Side::Blue, parsed)]);
    let state = w
        .rules
        .ground
        .as_ref()
        .unwrap()
        .spotting
        .get(&ship)
        .expect("accepted Spot must persist the spotting task");
    assert_eq!(state.target, id(&w, "TARGET"));
    assert!(matches!(state.method, ground::Spotting::Ship));
    assert_eq!(state.started, w.tick);
    assert!(state.beacon_ready.is_none());
}

#[test]
fn g3_source_expected_ngfs_resolution_is_the_registered_missing_consumer() {
    let mut w = load();
    advance(&mut w, 60);
    let ship = id(&w, "SHIP");
    let target = id(&w, "TARGET");
    let contact = visual_contact(&w);
    let spot = Command::Ground {
        unit: ship,
        action: ground::Action::Spot {
            contact,
            method: ground::Spotting::Ship,
        },
    };
    step(&mut w, &[(Side::Blue, spot)]);
    let fire = Command::Fire {
        unit: ship,
        contact,
        weapon: "cn_pj45a".into(),
        rounds: 1,
    };
    step(&mut w, &[(Side::Blue, fire)]);
    advance(&mut w, 30);
    // No Annex C/service qualification was declared in this negative fixture.
    // It must retain the source rejection instead of inventing gunnery.
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,detail,..}if rule=="11.source_launch_blocked"&&detail.contains("executor not yet bound"))));
    assert!(w.rules.ground.as_ref().unwrap().facilities.contains_key(&target));
}

#[test]
fn g4_beacon_producer_runs_and_the_witness_world_roundtrips() {
    let mut w = load();
    advance(&mut w, 60);
    let ship = id(&w, "SHIP");
    let obs = id(&w, "OBS");
    let contact = visual_contact(&w);
    let cmd = Command::Ground {
        unit: ship,
        action: ground::Action::Spot {
            contact,
            method: ground::Spotting::Beacon { observer: obs },
        },
    };
    step(&mut w, &[(Side::Blue, cmd)]);
    let state = w.rules.ground.as_ref().unwrap().spotting.get(&ship).unwrap();
    let ready = state.beacon_ready.expect("beacon emplacement clock must start");
    let offset = ready - state.started;
    assert!((180..=1080).contains(&offset), "original D6 Tactical Turns x 180 s");
    // Complete-World restore: the spotting task and its clock serialize.
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    advance(&mut w, 5);
    advance(&mut restored, 5);
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
}

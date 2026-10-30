//! Source Cmd/1 game adjudication with an explicitly fictional carrier fit.
//! Native command/release/flight/LOS/terminal, no injected contact or die.
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use serde_json::{json,Value};
const W:&str="us_agm12b_bullpup_a";
fn seed()->u64 {
    (0..100000).find(|s| {
        let identity="9.6.8:1:2:1:0";
        let mut r=Rng::new(*s).derive(Rng::hash_tag(2,1,identity));
        let die=1+(r.next_f64()*100.) as u8;
        let placement=ac::budget::claim_identity(ac::budget::AirAttackKind::Pgm,EntityId(1),2,
            &json!({"weapon":W,"rounds":1,"briefing":"range"}))+":rf-placement";
        let mut p=Rng::new(*s).derive(Rng::hash_tag(0,0,&placement));
        (11..=40).contains(&die) && i64::from(1+(p.next_f64()*10.) as u8)<=fc::tables()["placement_quality"]["Good"].as_i64().unwrap()
    }).unwrap()
}
fn world(smoke:bool)->World { world_section(smoke,false) }
fn world_section(smoke:bool,four:bool)->World {
    let mut input=json!({"id":"command-source","name":"scenario_assumption: Cmd1 experimental carriage","seed":seed(),"duration_s":600,
      "weather":{"visibility_nm":40,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},
       "controlled_pgm_los_2026":{"source":"scenario_assumption: clear local optical volume, authored bounds","minimum_aircraft_altitude_m":500,"maximum_ray_range_nm":40,"no_intervening_terrain":true},
       "aviation_2026":{"source":"scenario_assumption: experimental Bullpup carriage, no historical F18E claim",
        "loads":{"us_f18e":{"payload_kg":520,"tanks":[],"external_weapon_rounds":{(W):2},"source":"scenario_assumption: two actual source 260kg stores"}},
        "parking":{},"initial_fuel_nm":{"STRIKE":100},
        "surface_aimpoints":{"range":{"source":"scenario_assumption: fixed experimental aimpoint","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
       "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: clear daylight","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
       "command_sarh_2026":{"source":"English2026 PDF108 9.6.8 Cmd/SARH plus US Aircraft Annex H2-1",
        "weapons":{(W):{"source":"US Aircraft PDF99 H2-1 AGM12B","guidance":"Cmd/1","generation":1,"min_range_nm":1,"range_nm":6,"speed_kts":1188,"flight_path":"Direct","hang_weight_kg":260,"damage_points":33,"additional_criticals":"D6/2","penetration_cm":8,"signature":"VSm"}},
        "fits":{"STRIKE":{"source":"scenario_assumption: two Bullpup stores on experimental carrier","weapon":W,"rounds":2,"external":true}},
        "briefings":{"range":{"source":"scenario_assumption: explicit allocated fixed point","target":"TARGET","aimpoint":"range","known_kind":"Base","known_motion":"stationary","allowed_sides":["Blue"],"fcs":{"source":"scenario_assumption: declared Good experiment","quality":"Good"}}}},
       "ground_2026":{"source":"English2026 source hangar","facilities":{"TARGET":{"template":"hangar_medium","source":"English2026 PDF118"}}}},
      "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-4],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
        {"side":"Red","class":"airfield","name":"TARGET","pos":[0,0],"fire_on":false}]});
    input["context"]["pgm_2026"]=json!({"source":"scenario_assumption: three-aircraft section experiment","sections":{"alpha":{"source":"scenario_assumption: fixed section composition and relative geometry","members":["STRIKE","WING1","WING2"],"briefing":"range"}}});
    input["context"]["command_sarh_2026"]["fits"]["WING1"]=input["context"]["command_sarh_2026"]["fits"]["STRIKE"].clone();
    input["context"]["command_sarh_2026"]["fits"]["WING2"]=input["context"]["command_sarh_2026"]["fits"]["STRIKE"].clone();
    input["context"]["aviation_2026"]["initial_fuel_nm"]["WING1"]=json!(100);
    input["context"]["aviation_2026"]["initial_fuel_nm"]["WING2"]=json!(100);
    input["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":"us_f18e","name":"WING1","pos":[0.01,-4],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false}));
    input["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":"us_f18e","name":"WING2","pos":[-0.01,-4],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false}));
    if four {
        input["context"]["pgm_2026"]["sections"]["alpha"]["members"].as_array_mut().unwrap().push(json!("WING3"));
        input["context"]["command_sarh_2026"]["fits"]["WING3"]=input["context"]["command_sarh_2026"]["fits"]["STRIKE"].clone();
        input["context"]["aviation_2026"]["initial_fuel_nm"]["WING3"]=json!(100);
        input["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":"us_f18e","name":"WING3","pos":[0.02,-4],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false}));
    }
    if smoke {input["context"]["command_sarh_2026"]["smoke_screens"]=json!({"screen":{"source":"scenario_assumption: temporary controller-ray cylinder, dimensions not printed","target":"TARGET","radius_nm":0.5,"height_m":100,"obscures_controller_los_from_tick":3,"until_tick":5}});}
    let mut db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for class in ["us_f18e","airfield"] {db.platforms.get_mut(class).unwrap().sensors.clear();}
    db.platforms.get_mut("us_f18e").unwrap().weapons=vec![harpoon_core::db::WeaponFit{weapon:W.into(),mounts:1,ammo:2}];
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(),db).unwrap()
}

#[test]
fn three_aircraft_share_one_section_placement_roll_and_independent_physical_packets() {
    let mut w=world(false); harpoon_core::sim::tick(&mut w,&[]);
    let commands:Vec<_>=[EntityId(1),EntityId(3),EntityId(4)].into_iter().map(|unit|(Side::Blue,Command::AirCombat{unit,action:ac::Action::GuidedSurfaceAttack{weapon:W.into(),rounds:1,briefing:"range".into()}})).collect();
    harpoon_core::sim::tick(&mut w,&commands);
    let flights:Vec<_>=w.rules.pgm.as_ref().unwrap().flights.values().filter(|f|f.weapon==W).collect();
    assert_eq!(flights.len(),3,"three real aircraft must release separate packets");
    let placements:Vec<_>=flights.iter().map(|f|f.fcs.as_ref().unwrap().placement.as_ref().unwrap()).collect();
    assert_eq!(placements[0].identity,placements[1].identity);assert_eq!(placements[1].identity,placements[2].identity);
    assert_eq!(placements[0].d10,placements[1].d10);assert_eq!(placements[1].d10,placements[2].d10);
    assert!(flights.iter().all(|f|w.entity(f.actor).unwrap().weapons.iter().find(|m|m.weapon==W).unwrap().ammo==1));
}
#[test]
fn incomplete_declared_section_does_not_fire_or_spend_ammunition() {
    let mut w=world(false);harpoon_core::sim::tick(&mut w,&[]);
    let commands:Vec<_>=[EntityId(1),EntityId(3)].into_iter().map(|unit|(Side::Blue,Command::AirCombat{unit,action:ac::Action::GuidedSurfaceAttack{weapon:W.into(),rounds:1,briefing:"range".into()}})).collect();
    harpoon_core::sim::tick(&mut w,&commands);
    assert!(w.rules.pgm.as_ref().unwrap().flights.is_empty());
    assert!(!w.rules.aircombat.as_ref().unwrap().surface_last_turn.contains_key(&EntityId(1)));
    assert!(!w.rules.aircombat.as_ref().unwrap().surface_last_turn.contains_key(&EntityId(3)));
    for unit in [EntityId(1),EntityId(3),EntityId(4)] {assert_eq!(w.entity(unit).unwrap().weapons.iter().find(|m|m.weapon==W).unwrap().ammo,2);}
}

#[test]
fn four_aircraft_share_one_section_placement_roll() {
    let mut w=world_section(false,true);harpoon_core::sim::tick(&mut w,&[]);
    let commands:Vec<_>=[EntityId(1),EntityId(3),EntityId(4),EntityId(5)].into_iter().map(|unit|(Side::Blue,Command::AirCombat{unit,action:ac::Action::GuidedSurfaceAttack{weapon:W.into(),rounds:1,briefing:"range".into()}})).collect();
    harpoon_core::sim::tick(&mut w,&commands);
    let flights:Vec<_>=w.rules.pgm.as_ref().unwrap().flights.values().filter(|f|f.weapon==W).collect(); assert_eq!(flights.len(),4);
    let p:Vec<_>=flights.iter().map(|f|f.fcs.as_ref().unwrap().placement.as_ref().unwrap()).collect();
    assert!(p.iter().all(|r|r.identity==p[0].identity && r.d10==p[0].d10));
}

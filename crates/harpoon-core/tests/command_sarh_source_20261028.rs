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
fn world(smoke:bool)->World {
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
    if smoke {input["context"]["command_sarh_2026"]["smoke_screens"]=json!({"screen":{"source":"scenario_assumption: temporary controller-ray cylinder, dimensions not printed","target":"TARGET","radius_nm":0.5,"height_m":100,"obscures_controller_los_from_tick":3,"until_tick":5}});}
    let mut db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for class in ["us_f18e","airfield"] {db.platforms.get_mut(class).unwrap().sensors.clear();}
    db.platforms.get_mut("us_f18e").unwrap().weapons=vec![harpoon_core::db::WeaponFit{weapon:W.into(),mounts:1,ammo:2}];
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(),db).unwrap()
}
fn release(w:&mut World) {
    harpoon_core::sim::tick(w,&[]);
    harpoon_core::sim::tick(w,&[(Side::Blue,Command::AirCombat{unit:EntityId(1),action:ac::Action::GuidedSurfaceAttack{weapon:W.into(),rounds:1,briefing:"range".into()}})]);
    assert!(w.entities.values().any(|e|e.projectile.as_ref().is_some_and(|p|p.weapon==W)),"actual source release required");
}
fn terminal(w:&World)->Value {
    let rows:Vec<Value>=w.events.iter().filter_map(|e|match &e.kind {
        EventKind::RulesEvent{rule,detail,..} if rule=="9.6.8.source_terminal"=>{
            let v:Value=serde_json::from_str(detail).unwrap();(v["weapon"]==W).then_some(v)
        },_=>None}).collect();
    assert_eq!(rows.len(),1,"one original terminal settlement");rows[0].clone()
}
#[test]
fn cmd1_clear_controller_los_uses_source_cell_and_separate_extra_criticals() {
    let mut w=world(false);release(&mut w);
    let def=&w.db.weapons[W];
    assert_eq!(def.damage,33.);assert_eq!(def.rule_data.as_ref().unwrap().extra,"D6/2");
    assert_eq!(def.kind,harpoon_core::db::WeaponKind::Ascm);
    let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..28 {harpoon_core::sim::tick(&mut w,&[]);harpoon_core::sim::tick(&mut restored,&[]);}
    let t=terminal(&w);assert_eq!(t["probability"],0.4);assert_eq!(t["hit"],true);
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&restored).unwrap());
}
#[test]
fn cmd1_temporary_actual_controller_obscuration_preserves_original_die_and_quarter() {
    let mut clear=world(false);let mut smoke=world(true);release(&mut clear);release(&mut smoke);
    for _ in 0..28 {harpoon_core::sim::tick(&mut clear,&[]);harpoon_core::sim::tick(&mut smoke,&[]);}
    let a=terminal(&clear);let b=terminal(&smoke);
    assert_eq!(a["d100"],b["d100"]);assert_eq!(a["probability"],0.4);assert_eq!(b["probability"],0.1);
    assert_eq!(a["hit"],true);assert_eq!(b["hit"],false);
}

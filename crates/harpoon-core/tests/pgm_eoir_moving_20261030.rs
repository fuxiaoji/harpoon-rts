//! Source IIRH/2 game adjudication with an explicitly fictional carrier fit.
//! Native command/release/flight/LOS/terminal, no injected contact or die.
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use serde_json::{json,Value};
const W:&str="lab_agm65d_maverick";
fn seed()->u64 {
    (0..100000).find(|s| {
        let identity="9.6.8:1:2:1:0";
        let mut r=Rng::new(*s).derive(Rng::hash_tag(2,1,identity));
        let die=1+(r.next_f64()*100.) as u8;
        let placement=ac::budget::claim_identity(ac::budget::AirAttackKind::Pgm,EntityId(1),2,
            &json!({"weapon":W,"rounds":1,"briefing":"range"}))+":rf-placement";
        let mut p=Rng::new(*s).derive(Rng::hash_tag(0,0,&placement));
        (1..=80).contains(&die) && i64::from(1+(p.next_f64()*10.) as u8)<=fc::tables()["placement_quality"]["Good"].as_i64().unwrap()
    }).unwrap()
}
fn world(smoke:bool)->World {
    let mut input=json!({"id":"command-source","name":"scenario_assumption: IIRH2 experimental carriage","seed":seed(),"duration_s":600,
      "weather":{"visibility_nm":40,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},
       "controlled_pgm_los_2026":{"source":"scenario_assumption: clear local optical volume, authored bounds","minimum_aircraft_altitude_m":500,"maximum_ray_range_nm":40,"no_intervening_terrain":true},
       "aviation_2026":{"source":"scenario_assumption: experimental Maverick carriage, no historical F18E claim",
        "loads":{"us_f18e":{"payload_kg":220,"tanks":[],"external_weapon_rounds":{(W):1},"source":"scenario_assumption: one actual source 220kg store"}},
        "parking":{},"initial_fuel_nm":{"STRIKE":100},
        "surface_aimpoints":{"range":{"source":"scenario_assumption: fixed experimental aimpoint","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":false,"allowed_sides":["Blue"]}}},
       "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: clear daylight","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
       "eoir_2026":{"source":"English2026 PDF108/109 9.6.8 EO/IR plus US Aircraft Annex H2-1",
        "weapons":{(W):{"source":"US Aircraft PDF99 H2-1 AGM-65D","guidance":"IIRH/2","generation":2,"min_range_nm":0.5,"range_nm":11,"speed_kts":791,"flight_path":"Direct","hang_weight_kg":220,"damage_points":24,"penetration_cm":139,"signature":"VSm"}},
        "fits":{"STRIKE":{"source":"scenario_assumption: one Maverick store on experimental carrier","weapon":W,"rounds":1,"external":true}},
        "briefings":{"range":{"source":"scenario_assumption: explicit allocated fixed point","target":"TARGET","aimpoint":"range","known_kind":"Ship","known_motion":"moving","moving_lead":{"source":"scenario_assumption: own prebriefed course/speed estimate and physical target radius","observed_at":0,"course_deg":90,"speed_kts":10,"footprint_nm":0.004},"allowed_sides":["Blue"],"fcs":{"source":"scenario_assumption: declared Good experiment","quality":"Good"}}}},
       "ground_2026":{"source":"English2026 source hangar","facilities":{"TARGET":{"template":"hangar_medium","source":"English2026 PDF118"}}}},
      "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-3],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
        {"side":"Red","class":"cn_052d","name":"TARGET","pos":[0,0],"course_deg":90,"speed_kts":10,"fire_on":false}]});
    input["context"].as_object_mut().unwrap().remove("ground_2026");
    input["context"]["pgm_2026"]=json!({"source":"scenario_assumption: no PGM ECM","countermeasures":{"TARGET":{"source":"scenario_assumption: no PGM ECM","generation":0}}});
    if smoke {input["context"]["eoir_2026"]["smoke_screens"]=json!({"screen":{"source":"scenario_assumption: temporary sensor-ray cylinder, dimensions not printed","target":"TARGET","radius_nm":0.5,"height_m":100,"obscures_bomb_sensor_from_tick":3,"until_tick":5}});}
    let mut db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for class in ["us_f18e","cn_052d"] {db.platforms.get_mut(class).unwrap().sensors.clear();}
    db.platforms.get_mut("us_f18e").unwrap().weapons=vec![harpoon_core::db::WeaponFit{weapon:W.into(),mounts:1,ammo:1}];
    db.platforms.get_mut("cn_052d").unwrap().rule_data=Some(harpoon_core::rules::damage_runtime::PlatformDamage{source:"scenario_assumption: C unarmored target hull".into(),size:"C".into(),role:"surface".into(),belt_cm:0.,deck_cm:0.,..Default::default()});
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
fn moving_ship_uses_frozen_own_lead_and_escaped_target_is_geometric_miss() {
    for escape in [false,true] {
        let mut w=world(false); release(&mut w);
        if escape { harpoon_core::sim::tick(&mut w,&[(Side::Red,Command::SetCourse{unit:EntityId(2),course_deg:270.}),(Side::Red,Command::SetSpeed{unit:EntityId(2),speed_kts:30.})]); }
        for _ in 0..30 {harpoon_core::sim::tick(&mut w,&[]);}
        let t=terminal(&w);
        assert_eq!(t["hit"],!escape);
        if escape {assert_eq!(t["geometric_miss"],true);} else {assert_eq!(t["probability"],0.8);assert!(t["d100"].as_u64().is_some());}
    }
}

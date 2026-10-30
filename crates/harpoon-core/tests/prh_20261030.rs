//! Source PRH/1 game adjudication with an explicitly fictional carrier fit.
//! Native command/release/flight/LOS/terminal, no injected contact or die.
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use serde_json::{json,Value};
const W:&str="lab_agm45a_shrike";
fn seed()->u64 {
    (0..100000).find(|s| {
        let identity="9.6.8:1:2:1:0";
        let mut r=Rng::new(*s).derive(Rng::hash_tag(2,1,identity));
        let die=1+(r.next_f64()*100.) as u8;
        let placement=ac::budget::claim_identity(ac::budget::AirAttackKind::Pgm,EntityId(1),2,
            &json!({"weapon":W,"rounds":1,"briefing":"range"}))+":rf-placement";
        let mut p=Rng::new(*s).derive(Rng::hash_tag(0,0,&placement));
        (1..=40).contains(&die) && i64::from(1+(p.next_f64()*10.) as u8)<=fc::tables()["placement_quality"]["Good"].as_i64().unwrap()
    }).unwrap()
}
fn world(smoke:bool)->World { world_with_binding(smoke,true) }
fn world_with_binding(smoke:bool,binding:bool)->World {
    let mut input=json!({"id":"command-source","name":"scenario_assumption: Cmd1 experimental carriage","seed":seed(),"duration_s":600,
      "weather":{"visibility_nm":40,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},
       "controlled_pgm_los_2026":{"source":"scenario_assumption: clear local optical volume, authored bounds","minimum_aircraft_altitude_m":500,"maximum_ray_range_nm":40,"no_intervening_terrain":true},
       "aviation_2026":{"source":"scenario_assumption: experimental Shrike carriage, no historical F18E claim",
        "loads":{"us_f18e":{"payload_kg":182,"tanks":[],"external_weapon_rounds":{(W):1},"source":"scenario_assumption: one source 182kg store"}},
        "parking":{},"initial_fuel_nm":{"STRIKE":100},
        "surface_aimpoints":{"range":{"source":"scenario_assumption: fixed experimental aimpoint","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
       "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: clear daylight","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
       "command_sarh_2026":{"source":"English2026 PDF108 9.6.8 Cmd/SARH plus US Aircraft Annex H2-1",
        "weapons":{(W):{"source":"US Aircraft PDF99 H2-1 AGM-45A","guidance":"PRH/1","generation":1,"min_range_nm":2,"range_nm":7,"speed_kts":1320,"flight_path":"Direct","hang_weight_kg":182,"damage_points":0,"additional_criticals":"","penetration_cm":1,"signature":"VSm"}},
        "fits":{"STRIKE":{"source":"scenario_assumption: one Shrike store on experimental carrier","weapon":W,"rounds":1,"external":true}},
        "prh_radars":{"range":"cn_klc7"},"briefings":{"range":{"source":"scenario_assumption: explicit allocated fixed point","target":"TARGET","aimpoint":"range","known_kind":"Base","known_motion":"stationary","allowed_sides":["Blue"],"fcs":{"source":"scenario_assumption: declared Good experiment","quality":"Good"}}}},
       "ground_2026":{"source":"English2026 source hangar","facilities":{"TARGET":{"template":"search_mfc_radar","source":"English2026 PDF118"}}}},
      "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-4],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
        {"side":"Red","class":"airfield","name":"TARGET","pos":[0,0],"fire_on":false}]});
    if !binding { input["context"]["command_sarh_2026"].as_object_mut().unwrap().remove("prh_radars"); }
    if smoke {input["context"]["command_sarh_2026"]["smoke_screens"]=json!({"screen":{"source":"scenario_assumption: temporary controller-ray cylinder, dimensions not printed","target":"TARGET","radius_nm":0.5,"height_m":100,"obscures_controller_los_from_tick":3,"until_tick":5}});}
    let mut db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for class in ["us_f18e","airfield"] {db.platforms.get_mut(class).unwrap().sensors.clear();}
    db.platforms.get_mut("us_f18e").unwrap().weapons=vec![harpoon_core::db::WeaponFit{weapon:W.into(),mounts:1,ammo:1}];
    db.platforms.get_mut("airfield").unwrap().sensors=vec!["cn_klc7".into()];
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
fn shrike_actual_release_uses_original_d6_and_disables_exact_radar() {
    let mut w=world(false); release(&mut w);
    for _ in 0..28 {harpoon_core::sim::tick(&mut w,&[]);}
    let t=terminal(&w); assert_eq!(t["probability"],0.4); assert_eq!(t["hit"],true);
    let d=t["damage_d6"].as_u64().unwrap(); assert!((1..=6).contains(&d));
    assert_eq!(t["damage_points_per_hit"].as_f64().unwrap(),(d+2) as f64);
    let f=&w.rules.ground.as_ref().unwrap().facilities[&EntityId(2)];
    assert!(f.disabled); assert!(f.critical_hits>=1); assert_eq!(w.entity(EntityId(2)).unwrap().systems.radar,0.);
}
#[test]
fn shrike_missing_radar_binding_rejects_before_release() {
    let mut w=world_with_binding(false,false);
    harpoon_core::sim::tick(&mut w,&[]);
    let c=Command::AirCombat{unit:EntityId(1),action:ac::Action::GuidedSurfaceAttack{weapon:W.into(),rounds:1,briefing:"range".into()}};
    let before=serde_json::to_vec(&w).unwrap();
    assert!(c.validate_source(&w,Side::Blue).is_err()); assert_eq!(before,serde_json::to_vec(&w).unwrap());
}

#[test]
fn admission_does_not_reveal_private_radar_state_or_hp() {
    let mut a=world(false); harpoon_core::sim::tick(&mut a,&[]);
    let mut b=a.clone();
    { let e=b.entity_mut(EntityId(2)).unwrap(); e.ext.insert("emission.radar".into(),0.); e.hp=1.; e.systems.radar=0.; }
    let c=Command::AirCombat{unit:EntityId(1),action:ac::Action::GuidedSurfaceAttack{weapon:W.into(),rounds:1,briefing:"range".into()}};
    assert_eq!(c.validate_source(&a,Side::Blue).is_ok(),c.validate_source(&b,Side::Blue).is_ok());
    assert!(c.validate_source(&a,Side::Blue).is_ok());
}
#[test]
fn referee_radar_shutdown_after_release_preserves_pending_without_critical() {
    let mut w=world(false); release(&mut w);
    // Explicit adjudicated emission-state boundary; no claim of natural detection.
    w.entity_mut(EntityId(2)).unwrap().ext.insert("emission.radar".into(),0.);
    for _ in 0..28 {harpoon_core::sim::tick(&mut w,&[]);}
    assert!(!w.rules.pending_adjudications.is_empty());
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&EntityId(2)].critical_hits,0);
    assert!(!w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="9.6.8.prh_radar_critical")));
}

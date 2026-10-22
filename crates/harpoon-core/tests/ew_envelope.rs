use harpoon_core::{*,world::Contact,types::PlatformKind,geom::Vec2};
fn world()->World {
 let sc=scenario_from_json(r#"{"id":"ew","name":"ew","seed":3,"duration_s":600,"weather":{"visibility_nm":1,"jam":[0,0]},"context":{"radar_environment":{"sea_state":8,"precipitation":"None"}},"units":[{"side":"Blue","class":"cn_j15","pos":[0,0],"altitude_m":3000},{"side":"Blue","class":"us_f18e","pos":[20,0],"altitude_m":3000},{"side":"Red","class":"us_ea18g","pos":[10,0],"altitude_m":3000}]}"#).unwrap();
 load_scenario(sc,Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()).unwrap()
}
#[test]fn offensive_jamming_has_real_geometry_and_clutter_sources_do_not_multiply(){
 let mut w=world();
 w.sides[1].contacts.insert(ContactId(1),Contact {id:ContactId(1),about:EntityId(1),category:PlatformKind::Aircraft,class_name:None,estimate:Vec2::zero(),est_course_deg:0.0,est_speed_kts:0.0,quality:0.5,bearing_only:true,last_update_tick:0,first_tick:0,stale:false,sensors:vec!["esm_std".into()],offset:Vec2::zero(),announced:false});
 rules::electronic_warfare::configure(&mut w,EntityId(3),"spot",&[ContactId(1)]).unwrap();
 let radar=w.entities[&EntityId(1)].clone();let def=w.db.sensors["cn_klj4"].clone();
 let mut target=w.entities[&EntityId(2)].clone();target.kind=PlatformKind::Ship;
 assert_eq!(rules::electronic_warfare::combined_modifier(&w,&radar,&def,&target),Some(0.25),"sea clutter .35 and jammer .25 combine by strongest source, not .0875");
 target.kind=PlatformKind::Aircraft;target.altitude_m=3000.0;target.pos=Vec2::zero().advanced(125.0,20.0);
 assert_eq!(rules::electronic_warfare::combined_modifier(&w,&radar,&def,&target),Some(0.5),"35 degrees lies in gen4 sidelobe; net clutter is halved and floored");
 target.pos=Vec2::new(-20.0,0.0);assert_eq!(rules::electronic_warfare::combined_modifier(&w,&radar,&def,&target),Some(1.0));
}
#[test]fn low_altitude_boundary_uses_real_band_and_verified_clutter(){
 let w=world();let radar=&w.entities[&EntityId(1)];let def=&w.db.sensors["cn_klj4"];
 let mut target=w.entities[&EntityId(2)].clone();
 for altitude in [49.9,50.0,60.0] {target.altitude_m=altitude;assert_eq!(rules::electronic_warfare::combined_modifier(&w,radar,def,&target),Some(0.35));}
 target.altitude_m=61.0;assert_eq!(rules::electronic_warfare::combined_modifier(&w,radar,def,&target),Some(1.0));
}
#[test]fn spot_requires_observed_emitter_and_unknown_jammer_cannot_gain_capability(){
 let mut w=world();assert!(rules::electronic_warfare::configure(&mut w,EntityId(3),"spot",&[ContactId(999)]).is_err());
 assert!(rules::electronic_warfare::configure(&mut w,EntityId(1),"standoff",&[]).is_err());
 assert!(rules::electronic_warfare::configure(&mut w,EntityId(3),"standoff",&[]).is_err());
 assert!(!rules::electronic_warfare::emitting(&w.entities[&EntityId(3)]));
}

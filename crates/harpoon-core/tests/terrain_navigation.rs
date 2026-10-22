use harpoon_core::{terrain::{Terrain,LandPolygon},Vec2};
fn island()->Terrain {
    Terrain {source:"test rectangle".into(),polygons:vec![LandPolygon {outer:vec![Vec2::new(4.0,-2.0),Vec2::new(6.0,-2.0),Vec2::new(6.0,2.0),Vec2::new(4.0,2.0)],holes:vec![],bounds:None} ],..Default::default()}
}
#[test]fn spatial_index_matches_full_scan_for_sweeps_holes_negative_cells_and_saved_geometry() {
    let rect=|x:f64,y:f64,size:f64|vec![Vec2::new(x,y),Vec2::new(x+size,y),Vec2::new(x+size,y+size),Vec2::new(x,y+size)];
    let mut slow=Terrain::default();
    for x in -25..25 {for y in -15..15 {slow.polygons.push(LandPolygon {outer:rect(x as f64*9.0,y as f64*11.0,4.0),holes:vec![rect(x as f64*9.0+1.0,y as f64*11.0+1.0,2.0)],bounds:None});}}
    let mut indexed=slow.clone();indexed.normalize().unwrap();
    for i in 0..800 {
        let a=Vec2::new(((i*71)%500) as f64-250.0,((i*137)%360) as f64-180.0);
        let b=Vec2::new(a.x+(i%40) as f64-20.0,a.y+((i*17)%50) as f64-25.0);
        assert_eq!(indexed.contains(a),slow.contains(a));assert_eq!(indexed.blocked(a,b),slow.blocked(a,b));
    }
    for x in [-64.0,-32.0,0.0,32.0,64.0] {assert_eq!(indexed.blocked(Vec2::new(x,-200.0),Vec2::new(x,200.0)),slow.blocked(Vec2::new(x,-200.0),Vec2::new(x,200.0)));}
    assert_eq!(indexed.blocked(Vec2::new(-1e20,0.0),Vec2::new(1e20,0.0)),slow.blocked(Vec2::new(-1e20,0.0),Vec2::new(1e20,0.0)));
    let saved=serde_json::to_vec(&indexed).unwrap();assert!(!String::from_utf8_lossy(&saved).contains("\"index\""));
    let restored:Terrain=serde_json::from_slice(&saved).unwrap();assert_eq!(restored.blocked(Vec2::new(-100.0,0.0),Vec2::new(100.0,0.0)),indexed.blocked(Vec2::new(-100.0,0.0),Vec2::new(100.0,0.0)));
    // normalize invalidates a cloned cache after an intentional geometry edit.
    let mut edited=indexed.clone();edited.polygons.clear();edited.normalize().unwrap();assert!(!edited.blocked(Vec2::new(-100.0,0.0),Vec2::new(100.0,0.0)));
}
#[test] fn swept_path_catches_an_island_between_water_endpoints() {
    let t=island();assert!(!t.contains(Vec2::zero()));assert!(!t.contains(Vec2::new(10.0,0.0)));
    assert!(t.blocked(Vec2::zero(),Vec2::new(10.0,0.0)));
    assert!(t.blocked(Vec2::new(4.0,-3.0),Vec2::new(4.0,3.0)),"coast boundary is not navigable");
}
#[test] fn route_goes_around_land_and_rejects_land_destination() {
    let t=island();assert!(t.route(Vec2::zero(),Vec2::new(5.0,0.0)).is_none());
    let path=t.route(Vec2::zero(),Vec2::new(10.0,0.0)).unwrap();assert!(path.len()>1);
    let mut p=Vec2::zero();for q in path {assert!(!t.blocked(p,q));p=q;}
    assert_eq!(p,Vec2::new(10.0,0.0));
}
#[test] fn water_holes_are_preserved() {
    let mut t=island();t.polygons[0].holes.push(vec![Vec2::new(4.5,-1.0),Vec2::new(5.5,-1.0),Vec2::new(5.5,1.0),Vec2::new(4.5,1.0)]);
    assert!(!t.contains(Vec2::new(5.0,0.0)));assert!(t.contains(Vec2::new(4.25,0.0)));
}

fn world()->harpoon_core::World {
 let sc=harpoon_core::scenario_from_json(r#"{"id":"coast","name":"coast","duration_s":600,"seed":1,"weather":{"visibility_nm":1,"jam":[0,0]},"units":[{"side":"Blue","class":"ffg","pos":[3.99,0],"course_deg":90,"speed_kts":30}]}"#).unwrap();
 let mut w=harpoon_core::load_scenario(sc,harpoon_core::Database::built_in()).unwrap();w.context.terrain=Some(island());w
}
#[test] fn moving_ship_stops_before_land_and_aircraft_is_not_stopped() {
 let mut w=world();for _ in 0..30 {harpoon_core::sim::tick(&mut w,&[]);}
 let ship=&w.entities[&harpoon_core::EntityId(1)];assert!(ship.pos.x<4.0);assert_eq!(ship.speed_kts,0.0);
 assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent {domain,rule,..} if domain=="navigation"&&rule=="land_stop")));
 let mut w=world();let e=w.entities.get_mut(&harpoon_core::EntityId(1)).unwrap();e.kind=harpoon_core::types::PlatformKind::Aircraft;
 for _ in 0..30 {harpoon_core::sim::tick(&mut w,&[]);}assert!(w.entities[&harpoon_core::EntityId(1)].pos.x>4.0);
}
#[test] fn scenario_land_spawn_is_rejected_and_goto_is_routed_at_execution() {
 let mut w=world();let start=w.entities[&harpoon_core::EntityId(1)].pos;
 harpoon_core::sim::tick(&mut w,&[(harpoon_core::Side::Blue,harpoon_core::Command::Goto {unit:harpoon_core::EntityId(1),waypoints:vec![Vec2::new(10.0,0.0)]})]);
 let mut p=start;for q in &w.entities[&harpoon_core::EntityId(1)].waypoints {assert!(!w.context.terrain.as_ref().unwrap().blocked(p,*q));p=*q;}
 let mut s=harpoon_core::scenario_from_json(r#"{"id":"bad","name":"bad","duration_s":10,"seed":1,"weather":{"visibility_nm":1,"jam":[0,0]},"units":[{"side":"Blue","class":"ffg","pos":[5,0]}]}"#).unwrap();s.context.terrain=Some(island());
 assert!(harpoon_core::load_scenario(s,harpoon_core::Database::built_in()).unwrap_err().contains("spawn is on charted land"));
}

#[test] fn rerouted_ship_can_stop_turn_and_resume_without_entering_land() {
 let mut w=world();let goal=Vec2::new(10.0,0.0);
 harpoon_core::sim::tick(&mut w,&[(harpoon_core::Side::Blue,harpoon_core::Command::Goto {unit:harpoon_core::EntityId(1),waypoints:vec![goal]})]);
 for _ in 0..4000 {
  harpoon_core::sim::tick(&mut w,&[]);
  assert!(!w.context.terrain.as_ref().unwrap().contains(w.entities[&harpoon_core::EntityId(1)].pos));
  if w.entities[&harpoon_core::EntityId(1)].pos.range_to(goal)<0.2 {return;}
 }
 panic!("safe detour must resume rather than remain permanently stopped: {:?}",w.entities[&harpoon_core::EntityId(1)].pos);
}

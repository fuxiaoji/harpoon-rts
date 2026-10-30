//! §14.2.4 actual navigation commands and adjudicated hit input; no fabricated HP.
use harpoon_core::{Command,Database,Scenario,Side,World,EntityId,EventKind};
use harpoon_core::db::{WeaponDef,WeaponKind,Guidance};
use harpoon_core::rules::{beached2026 as b,damage_runtime as d,navigation2026 as n};
use serde_json::{json,Value};
fn id(w:&World,name:&str)->EntityId { w.entities.values().find(|e|e.name==name).unwrap().id }
fn fixture(seed:u64,initial:bool)->World {
 let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
 let p=db.platforms.get_mut("us_burke_2a").unwrap();p.weapons.clear();p.sensors.clear();p.rule_data=Some(d::PlatformDamage{size:"B".into(),role:"surface".into(),belt_cm:0.,deck_cm:0.,source:"scenario_assumption: controlled zero armor laboratory".into(),..Default::default()});
 let sc:Scenario=serde_json::from_value(json!({"id":"beached-test","name":"scenario_assumption: shore and adjudicated hit","seed":seed,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","beached_2026":{"source":"scenario_assumption: explicit shore","units":{"SHIP":{"source":"scenario_assumption: station and refloat completion","shore_position":{"x":0,"y":0},"maximum_distance_nm":0.1,"initially_beached":initial,"refloat_authorized":true}}}},"units":[{"side":"Blue","class":"us_burke_2a","name":"SHIP","pos":[0,0],"speed_kts":0,"fire_on":false},{"side":"Red","class":"cn_093","name":"DISTANT","pos":[200,200],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
 harpoon_core::load_scenario(sc,db).unwrap()
}
fn command(w:&World,action:n::Action)->Command {Command::Navigation{unit:id(w,"SHIP"),action}}
fn apply(w:&mut World,action:n::Action) {let c=command(w,action);c.validate_source(w,Side::Blue).unwrap();harpoon_core::sim::tick(w,&[(Side::Blue,c)]);}
fn hit(w:&mut World,damage:f64,underwater:bool,key:&str) {
 let mut weapon:WeaponDef=w.db.weapons["us_mk48"].clone();weapon.id="scenario_assumption:calibrated_hit".into();weapon.kind=if underwater {WeaponKind::Torpedo}else{WeaponKind::Gun};weapon.guidance=if underwater{Guidance::TorpedoActive}else{Guidance::BallisticGun};weapon.damage=damage;
 weapon.rule_data=Some(d::WeaponDamage{ship_dp:Some(damage),sub_dp:Some(damage),penetration_cm:Some(0.),fuze:if underwater{"contact"}else{"influence"}.into(),source:"scenario_assumption: adjudicated calibrated hit, not real weapon performance".into(),..Default::default()});
 let target=id(w,"SHIP");d::queue(w,target,&weapon,underwater,Some(90.),Some(Side::Red),Some(key.into()));d::finish(w);
}
fn roundtrip(w:&World) {let x=serde_json::to_vec(w).unwrap();let y:World=serde_json::from_slice(&x).unwrap();assert_eq!(x,serde_json::to_vec(&y).unwrap());}
#[test]
fn public_beach_holds_motion_and_refloat_removes_reserve_without_repair() {
 let mut w=fixture(20261030,false);let ship=id(&w,"SHIP");let listed=w.entity(ship).unwrap().hp_max;apply(&mut w,n::Action::Beach{});assert!(b::active(&w,ship));assert_eq!(b::capacity(&w,ship),Some(2.*listed));assert_eq!(w.entity(ship).unwrap().hp_max,listed);
 let c=Command::SetSpeed{unit:ship,speed_kts:10.};harpoon_core::sim::tick(&mut w,&[(Side::Blue,c)]);for _ in 0..10 {harpoon_core::sim::tick(&mut w,&[]);} assert_eq!(w.entity(ship).unwrap().speed_kts,0.);roundtrip(&w);
 apply(&mut w,n::Action::RefloatBeached{});assert!(!b::active(&w,ship));assert_eq!(w.entity(ship).unwrap().hp,listed);assert_eq!(w.entity(ship).unwrap().hp_max,listed);
}
#[test]
fn adjudicated_damage_preserves_exact_strict_refloat_and_two_dp_boundaries() {
 for seed in 1..=32 {let mut w=fixture(seed,true);let ship=id(&w,"SHIP");let original=w.entity(ship).unwrap().hp_max;hit(&mut w,original,false,"exact-original");
  if !w.entity(ship).is_some_and(|e|e.alive) || (b::damage_taken(&w,ship).unwrap()-original).abs()>1e-6 {continue;}
  assert!(command(&w,n::Action::RefloatBeached{}).validate_source(&w,Side::Blue).is_ok());roundtrip(&w);
  hit(&mut w,1.,false,"above-original");if !w.entity(ship).is_some_and(|e|e.alive) {continue;} assert!(b::damage_taken(&w,ship).unwrap()>original);assert!(command(&w,n::Action::RefloatBeached{}).validate_source(&w,Side::Blue).is_err());
  hit(&mut w,original-1.,false,"two-original");
  // finish commits DP; the normal damage step starts §14.3's sinking clock.
  // Destruction capacity is exhausted before the physical hull is removed.
  assert!(w.entity(ship).unwrap().hp<=0.);
  d::step(&mut w);
  assert!(w.entity(ship).is_none_or(|e|!e.alive||e.ext.get("damage.doomed")==Some(&1.)));
  return;
 } panic!("fixture precondition: no seed yields exact committed hit values without catastrophic critical");
}
#[test]
fn real_flood_severity_trace_uses_original_die_minus_two() {
 for seed in 1..=32 {let mut w=fixture(seed,true);let ship=id(&w,"SHIP");hit(&mut w,5.,true,"source-flood");
  let trace=w.events.iter().find_map(|e|match &e.kind {EventKind::RulesEvent{rule,detail,..} if rule=="14.2.4.flood_modifier"=>Some(serde_json::from_str::<Value>(detail).unwrap()),_=>None});
  if let Some(v)=trace {let raw=v["raw_d6"].as_u64().unwrap();let other=v["other_modifiers"].as_u64().unwrap();assert_eq!(v["beached_modifier"],json!(-2));assert_eq!(v["modified_severity"].as_f64().unwrap(),raw.saturating_sub(other+2) as f64);return;}
 } panic!("fixture precondition: no actual flooding critical in 32 seeds");
}

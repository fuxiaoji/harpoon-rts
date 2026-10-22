//! Exact post-game replay and measurements; never supplies live AI inputs.
#[path="support/flight_audit.rs"]mod flight_audit;
use harpoon_core::{World,Side,Command,Event,EventKind,types::PlatformKind,db::WeaponKind};
use serde::Deserialize;
use serde_json::{json,Value};
use std::collections::BTreeSet;
#[derive(Deserialize)]struct Input{tick:i64,commands:Vec<(Side,Command)>,injected:Vec<Event>}
fn hash(w:&World)->String{format!("{:016x}",serde_json::to_vec(w).unwrap().iter().fold(0xcbf29ce484222325u64,|h,b|(h^*b as u64).wrapping_mul(0x100000001b3)))}
fn main(){
 let args:Vec<_>=std::env::args().collect();let save:Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
 let expected:World=serde_json::from_value(save["payload"]["world"].clone()).unwrap();assert!(expected.over);
 let mut w:World=serde_json::from_value(save["payload"]["recording"]["initial"].clone()).unwrap();
 let inputs:Vec<Input>=serde_json::from_value(save["payload"]["recording"]["inputs"].clone()).unwrap();
 let mut events=w.events.clone();let mut cursor=0;let mut checks=0_u64;let mut audit=flight_audit::Audit::default();let memory=[Value::Null,Value::Null];
 let mut releases=Vec::new();let mut terminal=Vec::new();let mut seen=BTreeSet::new();audit.sample(&w,&[],&memory);
 loop{
  if let Some(terrain)=&w.context.terrain{for u in w.entities.values().filter(|u|u.alive&&matches!(u.kind,PlatformKind::Ship|PlatformKind::Sub)&&u.class!=harpoon_core::rules::asw::SONOBUOY_CLASS){assert!(!terrain.contains(u.pos),"land entry {} at {}",u.name,w.tick);checks+=1;}}
  if w.tick>=expected.tick{break;}
  let prior=w.event_seq;
  if let Some(i)=inputs.get(cursor).filter(|i|i.tick==w.tick){for e in &i.injected{assert_eq!(e.seq,w.event_seq+1);w.emit(e.kind.clone(),e.vis);}harpoon_core::sim::tick(&mut w,&i.commands);cursor+=1;}else{harpoon_core::sim::tick(&mut w,&[]);}
  let fresh:Vec<_>=w.events.iter().filter(|e|e.seq>prior).cloned().collect();audit.sample(&w,&fresh,&memory);
  for e in &fresh{if let EventKind::WeaponsRelease{side,shooter,weapon,count,..}=&e.kind{if let Some(u)=w.entity(*shooter){releases.push(json!({"tick":e.tick,"side":side,"shooter":shooter,"kind":u.kind,"altitude_m":u.altitude_m,"weapon":weapon,"count":count}));}}}
  events.extend(fresh);
  for u in w.entities.values().filter(|u|u.alive&&u.kind==PlatformKind::Missile){if let Some(p)=&u.projectile{if let Some(t)=p.target.and_then(|id|w.entity(id)).filter(|t|t.alive){if w.db.weapons.get(&p.weapon).is_some_and(|d|matches!(d.kind,WeaponKind::Ascm|WeaponKind::Slcm))&&u.pos.range_to(t.pos)<=10.0&&seen.insert(u.id){terminal.push(json!({"tick":w.tick,"projectile":u.id,"side":u.side,"target":t.id,"shooter":p.launched_by,"shooter_kind":w.entity(p.launched_by).map(|s|s.kind),"weapon":p.weapon}));}}}}
 }
 assert_eq!(hash(&w),hash(&expected));assert_eq!(serde_json::to_value(&events).unwrap(),save["payload"]["recording"]["events"]);
 let result=json!({"verified":true,"tick":w.tick,"world_fnv64":hash(&w),"command_batches":inputs.len(),"events_verified":events.len(),"exact_sea_position_checks":checks,"land_entries":0,"releases":releases,"terminal_entries":terminal,"flight_audit":audit,"limits":"Exact replay; no historical private AI memory reconstructed. Aircraft fate/release measured, but strike target-phase and EW traces are unavailable here. Terminal entries require live projectile and live target, not a hit."});
 std::fs::write(&args[2],serde_json::to_vec_pretty(&result).unwrap()).unwrap();println!("{}",json!({"verified":true,"tick":w.tick,"world_fnv64":hash(&w),"events":events.len(),"terminal_entries":terminal.len()}));
}

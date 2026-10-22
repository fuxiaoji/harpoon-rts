//! Post-game verifier. Adds assertions, never changes the match simulation.
use harpoon_core::{World,Side,Command,Event};
use serde::Deserialize;
#[derive(Deserialize)]struct Input{tick:i64,commands:Vec<(Side,Command)>,injected:Vec<Event>}
fn hash(w:&World)->String {format!("{:016x}",serde_json::to_vec(w).unwrap().iter().fold(0xcbf29ce484222325u64,|h,b|(h^*b as u64).wrapping_mul(0x100000001b3)))}
fn main(){
 let a:Vec<_>=std::env::args().collect();let save:serde_json::Value=serde_json::from_slice(&std::fs::read(&a[1]).unwrap()).unwrap();
 let expected:World=serde_json::from_value(save["payload"]["world"].clone()).unwrap();assert!(expected.over);
 let mut w:World=serde_json::from_value(save["payload"]["recording"]["initial"].clone()).unwrap();
 let mut events:Vec<harpoon_core::events::Event>=w.events.iter().cloned().collect();
 let inputs:Vec<Input>=serde_json::from_value(save["payload"]["recording"]["inputs"].clone()).unwrap();let mut cursor=0;let mut checks=0_u64;
 loop{
  let terrain=w.context.terrain.as_ref().expect("charted coastal game required");
  for u in w.entities.values().filter(|u|u.alive && matches!(u.kind,harpoon_core::types::PlatformKind::Ship|harpoon_core::types::PlatformKind::Sub) && u.class!=harpoon_core::rules::asw::SONOBUOY_CLASS){assert!(!terrain.contains(u.pos),"{} entered land at {}",u.name,w.tick);checks+=1;}
  if w.tick>=expected.tick{break;}
  let prior=w.event_seq;
  if let Some(i)=inputs.get(cursor).filter(|i|i.tick==w.tick){for e in &i.injected {assert_eq!(e.seq,w.event_seq+1);w.emit(e.kind.clone(),e.vis);}harpoon_core::sim::tick(&mut w,&i.commands);cursor+=1;}else{harpoon_core::sim::tick(&mut w,&[]);}
  events.extend(w.events.iter().filter(|e|e.seq>prior).cloned());
  if w.tick%3600==0 {eprintln!("replayed tick {} / {}",w.tick,expected.tick);}
 }
 assert_eq!(hash(&w),hash(&expected));assert_eq!(serde_json::to_value(&events).unwrap(),save["payload"]["recording"]["events"]);
 let result=serde_json::json!({"verified":true,"tick":w.tick,"events_verified":events.len(),"command_batches":inputs.len(),"exact_sea_position_checks":checks,"land_entries":0,"world_fnv64":hash(&w),"note":"Exact replay coordinates, before presentation rounding; live ships and submarines, excludes sonobuoys"});
 if let Some(path)=a.get(2){std::fs::write(path,serde_json::to_vec_pretty(&result).unwrap()).unwrap();}println!("{result}");
}

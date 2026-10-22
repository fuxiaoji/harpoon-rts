//! Reconstruct every simulated second from a completed authoritative save.
//! No live camera sampling or fabricated trajectories. Referee-only output.
//! Usage: cinematic_replay <save.json> <new-output-directory>
use harpoon_core::{World,Side,Command,Event,observe};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use std::{collections::BTreeMap,io::Write};
#[derive(Deserialize)]struct Input {tick:i64,commands:Vec<(Side,Command)>,injected:Vec<Event>}
#[derive(Deserialize)]struct Recording {initial:World,inputs:Vec<Input>}
fn digest<T:Serialize>(x:&T)->String{let h=serde_json::to_vec(x).unwrap().iter().fold(0xcbf29ce484222325u64,|h,b|(h^*b as u64).wrapping_mul(0x100000001b3));format!("{h:016x}")}
fn rounded(v:f64)->f64{(v*1000.0).round()/1000.0}
fn main(){
 let args:Vec<_>=std::env::args().collect();let raw=std::fs::read(&args[1]).unwrap();let save:Value=serde_json::from_slice(&raw).unwrap();
 let expected:World=serde_json::from_value(save["payload"]["world"].clone()).unwrap();assert!(expected.over,"only completed games can export referee motion");
 let r:Recording=serde_json::from_value(save["payload"]["recording"].clone()).unwrap();let mut w=r.initial;
 let dest=std::path::Path::new(&args[2]);std::fs::create_dir(dest).expect("new evidence directory required");
 let mut frames=std::io::BufWriter::new(std::fs::File::create(dest.join("motion.jsonl")).unwrap());
 let mut catalog:BTreeMap<u32,Value>=BTreeMap::new();let mut all_events:Vec<harpoon_core::events::Event>=w.events.iter().cloned().collect();let mut cursor=0;let mut count=0;
 loop {
  for e in w.entities.values().filter(|e|e.alive){catalog.entry(e.id.0).or_insert_with(||json!({"id":e.id,"side":e.side,"kind":e.kind,"name":e.name,"class":e.class,"hp_max":e.hp_max}));}
  let units:Vec<_>=w.entities.values().filter(|e|e.alive && e.class!=harpoon_core::rules::asw::SONOBUOY_CLASS)
   .map(|e|json!([e.id.0,rounded(e.pos.x),rounded(e.pos.y),rounded(e.course_deg),rounded(e.altitude_m),rounded(e.depth_m),rounded(e.hp)])).collect();
  let contacts=|side|->Vec<Value>{observe(&w,side,w.event_seq,0).contacts.iter().map(|c|json!([c.id.0,rounded(c.estimate.x),rounded(c.estimate.y),rounded(c.est_course_deg),c.category,c.class_name,rounded(c.quality),c.stale,c.bearing_only,rounded(c.last_seen_s)])).collect()};
  serde_json::to_writer(&mut frames,&json!({"t":w.tick,"u":units,"b":contacts(Side::Blue),"r":contacts(Side::Red)})).unwrap();frames.write_all(b"\n").unwrap();count+=1;
  if w.tick>=expected.tick||w.over {break;}
  let seq=w.event_seq;let cap=w.event_cap;w.event_cap=usize::MAX;
  if let Some(input)=r.inputs.get(cursor).filter(|i|i.tick==w.tick){
   for e in &input.injected {assert_eq!(e.seq,w.event_seq+1);w.emit(e.kind.clone(),e.vis);}
   harpoon_core::sim::tick(&mut w,&input.commands);cursor+=1;
  } else {harpoon_core::sim::tick(&mut w,&[]);}
  all_events.extend(w.events.iter().filter(|e|e.seq>seq).cloned());w.event_cap=cap;
  if w.events.len()>cap {w.events.drain(0..w.events.len()-cap);}
  if w.tick%1800==0 {eprintln!("motion export {} / {}",w.tick,expected.tick);}
 }
 frames.flush().unwrap();assert_eq!(digest(&w),digest(&expected),"full World replay must match before motion can be used");
 let expected_events:Vec<Event>=serde_json::from_value(save["payload"]["recording"]["events"].clone()).unwrap();
 assert_eq!(digest(&all_events),digest(&expected_events),"every original event must match");
 let meta=json!({"format":"harpoon-cinematic-v1","scenario_id":w.scenario_id,"seed":w.seed,"duration_s":w.tick,"result":w.result,
   "theater":w.context.theater,"terrain":w.context.terrain,"catalog":catalog,"frames":count,"world_fnv64":digest(&w),"source_save":args[1],
   "precision":"One exact replay step per simulated second. Plot positions rounded to 0.001nm; animation between successive one-second states is presentation interpolation. Side contacts are their own observations; truth catalog is director-only."});
 std::fs::write(dest.join("metadata.json"),serde_json::to_vec(&meta).unwrap()).unwrap();
 std::fs::write(dest.join("events.json"),serde_json::to_vec(&all_events).unwrap()).unwrap();
 println!("{} frames, {} events, World {}",count,all_events.len(),digest(&w));
}

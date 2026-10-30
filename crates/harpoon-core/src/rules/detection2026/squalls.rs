//! 5.8.7.2 authored initial squalls; wind is explicit scenario input.
use super::*;
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration { pub source:String, pub centers:Vec<Vec2>, pub wind_kts:Vec2, #[serde(default)] pub crossing_policy:Option<String> }
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Cell { pub center:Vec2, pub d10:u8 }
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct State { pub source:String, pub wind_kts:Vec2, pub started_at:i64, pub cells:Vec<Cell>, #[serde(default)] pub crossing_policy:Option<String> }
pub fn tables()-> &'static Value { static T:OnceLock<Value>=OnceLock::new(); T.get_or_init(||serde_json::from_str(include_str!("../../../../../data/rules/squalls-2026.json")).expect("squall source")) }
pub(super) fn initialize(w:&mut World)->Result<(),String> {
 let Some(c)=w.rules.detection.as_ref().and_then(|s|s.config.squalls.clone()) else {return Ok(())};
 if c.crossing_policy.as_deref().is_some_and(|p|p != "whole_path_worst") || !c.source.starts_with("scenario_assumption") || !c.wind_kts.x.is_finite() || !c.wind_kts.y.is_finite() || c.centers.iter().any(|p|!p.x.is_finite()||!p.y.is_finite()) {return Err("5.8.7.2 squalls require explicit finite authored positions and wind".into());}
 let mut cells=Vec::new();
 for (i,center) in c.centers.iter().enumerate() { let mut r=Rng::new(w.seed).derive(Rng::hash_tag(w.tick,i as u32,"5.8.7.2.squall")); let d10=1+(r.next_f64()*10.) as u8; cells.push(Cell{center:*center,d10});
 w.emit(crate::EventKind::RulesEvent{domain:"detection2026".into(),rule:"5.8.7.2.squall".into(),side:None,unit:None,contact:None,detail:json!({"index":i,"d10":d10}).to_string()},crate::Visibility::Referee); }
 w.rules.detection.as_mut().unwrap().squalls=Some(State{source:c.source,wind_kts:c.wind_kts,started_at:w.tick,cells,crossing_policy:c.crossing_policy}); Ok(())
}
/// Scope: endpoints both in one circle, or no circle intersects the line.
/// A partial traversal has no sourced rule for combining visibility distances.
pub(super) fn visibility(s:&State,now:i64,a:Vec2,b:Vec2,ambient:u8)->Result<u8,String>{
 let mut result=ambient; let delta=b.sub(a); let len2=delta.x*delta.x+delta.y*delta.y;
 for cell in &s.cells { let center=cell.center.add(s.wind_kts.scaled((now-s.started_at) as f64/3600.));
 let inside_a=a.range_to(center)<=1.5; let inside_b=b.range_to(center)<=1.5;
 let u=if len2==0. {0.} else {((center.x-a.x)*delta.x+(center.y-a.y)*delta.y)/len2}.clamp(0.,1.);
 let crosses=a.add(delta.scaled(u)).range_to(center)<=1.5;
 if inside_a && inside_b {result=result.min(if cell.d10<=7 {10} else {5});}
 else if crosses { if s.crossing_policy.as_deref()==Some("whole_path_worst") { result=result.min(if cell.d10<=7 {10} else {5}); } else { return Err("5.8.7.2 partially intersected squall visibility combination source-pending".into()); }}
 }
 Ok(result)
}



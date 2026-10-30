//! §11.5 maintenance-building casualty applied to actual §14.9 repair work.
//! Provider assignment, distance and reduced rate are explicit scenario inputs;
//! the book does not print a universal reduction multiplier.
use crate::{EntityId,World};
use serde::{Serialize,Deserialize};
use std::collections::BTreeMap;
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {pub source:String,pub provider:String,pub maximum_distance_nm:f64}
#[derive(Clone,Debug,Default,Serialize,Deserialize)]
pub struct State {pub bindings:BTreeMap<EntityId,Binding>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Binding {pub provider:EntityId,pub maximum_distance_nm:f64,pub work:Option<Work>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Work {pub began_at:i64,pub last_tick:i64,pub earned_seconds:f64,pub pending:Option<String>}
pub fn initialize(w:&mut World)->Result<(),String>{
 let assignments=w.context.ground_2026.as_ref().map(|c|c.aircraft_maintenance.clone()).unwrap_or_default();
 if assignments.is_empty(){return Ok(())}
 let mut state=State::default();
 for (name,a) in assignments {
  if a.source.trim().is_empty()||!a.maximum_distance_nm.is_finite()||a.maximum_distance_nm<=0. {return Err("11.5 aircraft maintenance explicit source/distance required".into())}
  let id=super::find_named(w,&name)?;let e=w.entity(id).unwrap();
  if e.kind!=crate::types::PlatformKind::Aircraft{return Err("11.5 maintenance assignment requires an individual aircraft".into())}
  let provider=super::maintenance_provider(w,w.rules.ground.as_ref().unwrap(),e.side,&a.provider)?;
  state.bindings.insert(id,Binding{provider,maximum_distance_nm:a.maximum_distance_nm,work:None});
 }
 w.rules.ground.as_mut().unwrap().aircraft_maintenance=Some(state);Ok(())
}
fn rate(w:&World,id:EntityId,b:&Binding)->Result<f64,String>{
 let a=w.entity(id).filter(|e|e.alive).ok_or("11.5 maintained aircraft unavailable")?;
 let p=w.entity(b.provider).filter(|e|e.alive&&e.side==a.side).ok_or("11.5 own maintenance provider unavailable")?;
 if !super::is_parked_aircraft(w,id)||a.pos.range_to(p.pos)>b.maximum_distance_nm {
  return Err("11.5 aircraft must be explicitly parked within its assigned maintenance station".into())
 }
 super::maintenance_rate(w,b.provider,a.side)
}
/// Consume each elapsed interval once, before damage can change its rate.
/// A pending interval earns zero and can never be caught up after restoration.
pub fn step(w:&mut World){
 let Some(mut state)=w.rules.ground.as_mut().and_then(|g|g.aircraft_maintenance.take())else{return};
 for (id,b) in &mut state.bindings {
  let session=crate::rules::repairs2026::optional_session(w,*id);
  let eligible=rate(w,*id,b);
  let now=w.tick;
  if let Ok(began)=session.as_ref() {
   if b.work.as_ref().is_none_or(|x|x.began_at!=*began) {
    b.work=Some(Work{began_at:*began,last_tick:now,earned_seconds:0.,pending:(now!=*began).then(||"11.5 repair start was not observed; elapsed history is not reconstructed".into())});
   }
  }
  let Some(work)=b.work.as_mut()else{continue};
  let previous=work.pending.clone();
  let historical_start=work.pending.as_deref()==Some("11.5 repair start was not observed; elapsed history is not reconstructed");
  let current=session.and_then(|_|eligible);
  if !historical_start {
   match current {
    Ok(value)=>{work.earned_seconds+=(now-work.last_tick).max(0)as f64*value;work.pending=None;},
    Err(reason)=>work.pending=Some(reason),
   }
  }
  work.last_tick=now;
  if work.pending!=previous {super::trace(w,*id,"11.5.aircraft_maintenance_rate",serde_json::json!({"provider":b.provider,"earned_seconds":work.earned_seconds,"pending":work.pending,"policy":"explicit scenario reduced rate; paused intervals never banked"}));}
 }
 w.rules.ground.as_mut().unwrap().aircraft_maintenance=Some(state);
}
pub fn effective_elapsed(w:&mut World,id:EntityId,began:i64)->Result<Option<i64>,String>{
 let bound=w.rules.ground.as_ref().and_then(|g|g.aircraft_maintenance.as_ref()).is_some_and(|s|s.bindings.contains_key(&id));
 if !bound{return Ok(None)}
 step(w);
 let work=w.rules.ground.as_ref().and_then(|g|g.aircraft_maintenance.as_ref()).and_then(|s|s.bindings.get(&id)).and_then(|b|b.work.as_ref()).ok_or("11.5 declared aircraft repair work has not started")?;
 if work.began_at!=began{return Err("11.5 aircraft repair session mismatch".into())}
 if let Some(reason)=&work.pending{return Err(reason.clone())}
 Ok(Some(work.earned_seconds.floor()as i64))
}
pub fn own_view(w:&World,id:EntityId)->Option<serde_json::Value>{
 let b=w.rules.ground.as_ref()?.aircraft_maintenance.as_ref()?.bindings.get(&id)?;
 Some(serde_json::json!({"provider":b.provider,"work":b.work}))
}

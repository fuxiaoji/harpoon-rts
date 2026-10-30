//! 14.2.5 explicit crew-factor transfer, independent of 14.8 people.
use crate::{EntityId, Side, Vec2, World, EventKind, Visibility, Rng};
use serde::{Serialize,Deserialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::OnceLock;
fn tables()->&'static serde_json::Value { static T:OnceLock<serde_json::Value>=OnceLock::new(); T.get_or_init(||serde_json::from_str(include_str!("../../../../../data/rules/crew-transfer-2026.json")).expect("14.2.5 source table")) }
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit { pub source:String, pub healthy:u32, pub injured:u32, pub capacity:u32, pub alongside_yards:f64, #[serde(default)] pub crew_role:bool }
#[derive(Clone,Debug,Default,Serialize,Deserialize)]
pub struct State { pub inventories:BTreeMap<EntityId,Inventory>, pub jobs:BTreeMap<EntityId,Job>, pub overboard:Vec<Overboard>, pub sequence:u64 }
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Inventory { pub healthy:u32, pub injured:u32, pub capacity:u32, pub alongside_yards:f64, #[serde(default)] pub crew_role:bool }
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Job { pub receiver:EntityId, pub healthy:u32, pub injured:u32, pub due:i64, pub moving:bool }
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Overboard { pub owner:Side, pub position:Vec2, pub healthy:u32, pub injured:u32, pub at:i64, pub collision_roll:u8, pub collision_pending:bool }
fn trace(w:&mut World,id:EntityId,rule:&str,detail:serde_json::Value) { let side=w.entity(id).unwrap().side; w.emit(EventKind::RulesEvent {domain:"personnel_transfer".into(),rule:rule.into(),side:Some(side),unit:Some(id),contact:None,detail:detail.to_string()},Visibility::Side(side)); }
fn die(w:&World,id:EntityId,seq:u64,purpose:&str,sides:u8)->u8 { let mut r=w.rng.derive(Rng::hash_tag(w.tick,id.0,&format!("14.2.5:{seq}:{purpose}"))); 1+(r.next_f64()*f64::from(sides)).floor() as u8 }
pub fn initialize(w:&mut World)->Result<(),String> {
 if w.rules.profile != super::dr::RuleProfile::Rts2026Full || w.rules.expanded_damage.is_none() { return Ok(()); }
 let mut rows=Vec::new();
 for e in w.entities.values() { if let Some(f)=super::fit(w,e.id).and_then(|x|x.transfer_factors.as_ref()) {
  if f.source.trim().is_empty() || e.kind!=crate::types::PlatformKind::Ship || f.capacity>12 || (f.crew_role && (f.healthy>6 || !super::dr::platform_of(e).is_some_and(|p| matches!(p.size.as_str(), "F"|"G")))) || f.capacity==0 || f.healthy.saturating_add(f.injured)>f.capacity || !f.alongside_yards.is_finite() || f.alongside_yards<=0. || f.alongside_yards>50. { return Err("14.2.5 explicit ship factor inventory/capacity/alongside geometry invalid".into()); }
  rows.push((e.id,Inventory{healthy:f.healthy,injured:f.injured,capacity:f.capacity,alongside_yards:f.alongside_yards,crew_role:f.crew_role}));
 }}
 let s=&mut w.rules.expanded_damage.as_mut().unwrap().transfers; for (id,row) in rows {s.inventories.insert(id,row);}
 let ids:Vec<_>=w.rules.expanded_damage.as_ref().unwrap().transfers.inventories.keys().copied().collect(); for id in ids { sync_manning(w,id); } Ok(())
}
pub fn validate(w:&World, from:EntityId,to:EntityId,healthy:u32,injured:u32)->Result<(),String> {
 let a=w.entity(from).filter(|e|e.alive).ok_or("14.2.5 sender unavailable")?; let b=w.entity(to).filter(|e|e.alive).ok_or("14.2.5 receiver unavailable")?;
 if from==to || a.side!=b.side || healthy.saturating_add(injured)==0 { return Err("14.2.5 own distinct ships and factors required".into()); }
 let s=&w.rules.expanded_damage.as_ref().ok_or("14.2.5 state unavailable")?.transfers;
 let x=s.inventories.get(&from).ok_or("14.2.5 sender inventory absent")?; let y=s.inventories.get(&to).ok_or("14.2.5 receiver inventory absent")?;
 if s.jobs.iter().any(|(sender,j)| *sender==from || *sender==to || j.receiver==from || j.receiver==to) || healthy>x.healthy || injured>x.injured || y.healthy.saturating_add(y.injured).saturating_add(healthy).saturating_add(injured)>y.capacity { return Err("14.2.5 unavailable factors, capacity or active transfer".into()); }
 if a.pos.range_to(b.pos)>50.0/2025.3718285 {return Err("14.2.5 vessels must be within 50 yards".into());}
 if a.speed_kts>0. && b.speed_kts>0. && a.speed_kts>5. && a.speed_kts<6. {return Err("14.2.5 source_pending: speed strictly between printed 5 and 6 kt bands".into());}
 if a.speed_kts>10.0 || b.speed_kts>10.0 {return Err("14.2.5 speed above 10 kt prohibited".into());}
 if a.speed_kts>0. && b.speed_kts>0. && ((a.speed_kts-b.speed_kts).abs()>1e-6 || crate::geom::angle_diff(a.course_deg,b.course_deg).abs()>1e-6 || a.speed_kts<1.) {return Err("14.2.5 moving vessels require same course and speed 1..10kt".into());}
 Ok(())
}
pub fn begin(w:&mut World,from:EntityId,to:EntityId,healthy:u32,injured:u32)->Result<(),String> {
 validate(w,from,to,healthy,injured)?; let a=w.entity(from).unwrap(); let b=w.entity(to).unwrap(); let moving=a.speed_kts>0. && b.speed_kts>0.;
 let alongside=a.pos.range_to(b.pos)<=w.rules.expanded_damage.as_ref().unwrap().transfers.inventories[&from].alongside_yards.min(w.rules.expanded_damage.as_ref().unwrap().transfers.inventories[&to].alongside_yards)/2025.3718285;
 let already_stopped=!moving && a.speed_kts.abs()<1e-6 && b.speed_kts.abs()<1e-6 && alongside;
 let seq=w.rules.expanded_damage.as_ref().unwrap().transfers.sequence; let roll=if moving || already_stopped {0} else {die(w,from,seq,"approach",6)};
 let due=w.tick+if moving || already_stopped {30} else {i64::from(roll)*30};
 let s=&mut w.rules.expanded_damage.as_mut().unwrap().transfers; s.sequence+=1; s.jobs.insert(from,Job{receiver:to,healthy,injured,due,moving});
 trace(w,from,"14.2.5.begin",json!({"receiver":to,"moving":moving,"approach_d6":if moving || already_stopped {None}else{Some(roll)},"due":due})); Ok(())
}
pub fn step(w:&mut World) {
 let jobs=w.rules.expanded_damage.as_ref().unwrap().transfers.jobs.clone();
 for (from,mut job) in jobs { if w.tick<job.due {continue;} let Some(a)=w.entity(from).filter(|e|e.alive).cloned() else {w.rules.expanded_damage.as_mut().unwrap().transfers.jobs.remove(&from);continue}; let Some(b)=w.entity(job.receiver).filter(|e|e.alive).cloned() else {w.rules.expanded_damage.as_mut().unwrap().transfers.jobs.remove(&from);continue};
 let s=&w.rules.expanded_damage.as_ref().unwrap().transfers; let Some(x)=s.inventories.get(&from) else {continue}; let Some(y)=s.inventories.get(&job.receiver) else {continue};
 let enough=x.healthy>=job.healthy && x.injured>=job.injured && y.healthy+y.injured+job.healthy+job.injured<=y.capacity;
 let within=enough && a.pos.range_to(b.pos)<=x.alongside_yards.min(y.alongside_yards)/2025.3718285;
 let valid=within && if job.moving {(a.speed_kts-b.speed_kts).abs()<1e-6 && crate::geom::angle_diff(a.course_deg,b.course_deg).abs()<1e-6 && a.speed_kts>=1. && (a.speed_kts<=5. || (a.speed_kts>=6. && a.speed_kts<=10.))} else {a.speed_kts.abs()<1e-6 && b.speed_kts.abs()<1e-6};
 if !valid { trace(w,from,"14.2.5.pending",json!({"reason":"awaiting actual alongside, stopped or valid moving geometry; no teleport"})); job.due=w.tick+30; w.rules.expanded_damage.as_mut().unwrap().transfers.jobs.insert(from,job);continue; }
 let seq=s.sequence; let chance=if job.moving {if a.speed_kts<=5. {tables()["moving_chances"]["1_to_5"].as_u64().unwrap() as u8}else{tables()["moving_chances"]["6_to_10"].as_u64().unwrap() as u8}} else {100}; let roll=if job.moving {die(w,from,seq,"transfer",100)} else {1}; let ok=roll<=chance;
 let nh=job.healthy; let ni=job.injured.min(1);
 let collision=if !ok {die(w,from,seq,"collision",100)} else {0};
 let state=&mut w.rules.expanded_damage.as_mut().unwrap().transfers; state.sequence+=1;
 let x=state.inventories.get_mut(&from).unwrap(); x.healthy-=nh; x.injured-=ni;
 if ok {let y=state.inventories.get_mut(&job.receiver).unwrap(); y.healthy+=nh;y.injured+=ni;} else {state.overboard.push(Overboard{owner:a.side,position:a.pos,healthy:nh,injured:ni,at:w.tick,collision_roll:collision,collision_pending:collision<=50});}
 let receiver=job.receiver;
 job.healthy-=nh; job.injured-=ni; if job.healthy+job.injured==0 || !ok {state.jobs.remove(&from);} else {job.due=w.tick+30;state.jobs.insert(from,job);}
 sync_manning(w,from); sync_manning(w,receiver);
 trace(w,from,"14.2.5.transfer",json!({"success":ok,"d100":roll,"chance":chance,"healthy":nh,"injured":ni,"collision_d100":if ok {None}else{Some(collision)},"collision_effect":if !ok && collision<=50 {"pending_source_collision_geometry"} else {"none"}}));
 }
}

/// Only an explicitly declared six-factor normal crew affects 14.5 manning.
fn sync_manning(w:&mut World,id:EntityId) {
 let Some(i)=w.rules.expanded_damage.as_ref().and_then(|s|s.transfers.inventories.get(&id)).filter(|i|i.crew_role).cloned() else{return};
 let loss=6u32.saturating_sub(i.healthy.min(6));
 if let Some(e)=w.entity_mut(id) {e.ext.insert("damage.personnel".into(),loss as f64);}
 if let Some(s)=w.rules.expanded_damage.as_mut() {let u=s.units.entry(id).or_default(); let removed:Vec<_>=u.personnel_choices.iter().filter(|(n,_)|u32::from(**n)>loss).map(|(n,c)|(*n,c.clone())).collect(); for (n,c) in removed {u.personnel_choices.remove(&n); if !u.personnel_choices.values().any(|x|x==&c) {u.abandoned.remove(&c);}}}
 for level in 2..=loss.min(3) {
  if let Some(e)=w.entity_mut(id) {e.ext.insert("damage.personnel".into(),level as f64);}
  super::personnel(w,id,"14.2.5 crew transfer");
 }
 if let Some(e)=w.entity_mut(id) {e.ext.insert("damage.personnel".into(),loss as f64);}
}
/// Existing personnel critical converts one healthy normal crew factor to injured.
pub(super) fn casualty(w:&mut World,id:EntityId) {
 if let Some(i)=w.rules.expanded_damage.as_mut().and_then(|s|s.transfers.inventories.get_mut(&id)).filter(|i|i.crew_role && i.healthy>0) {i.healthy-=1;i.injured+=1;}
}

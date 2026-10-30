//! §14.2.4: source DP reserve for exposed beached/grounded vessels.
//! hp_max remains the listed source DP; hp includes the extra destruction
//! reserve while beached. Original-DP percentages therefore stay unchanged.
//! Observation uses capacity() for its runtime health scale. Refloating removes
//! the reserve, never repairs damage. Catastrophic destruction remains distinct.
use crate::{EntityId,World,EventKind,Visibility};
use crate::types::PlatformKind;
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use std::{collections::BTreeMap,sync::OnceLock};
#[derive(Clone,Debug,Default,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct Configuration {pub source:String,pub units:BTreeMap<String,Declaration>}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
 pub source:String,pub shore_position:crate::Vec2,pub maximum_distance_nm:f64,
 #[serde(default)]pub initially_beached:bool,
 /// Explicit completion authorization, not an invented universal refloat time.
 #[serde(default)]pub refloat_authorized:bool,
}
pub fn tables()->&'static Value {static T:OnceLock<Value>=OnceLock::new();T.get_or_init(||serde_json::from_str(include_str!("../../../../data/rules/beached-2026.json")).unwrap())}
fn n(k:&str)->f64 {tables()[k].as_f64().expect("beached source value")}
fn authored(s:&str)->bool{s=="scenario_assumption"||s.starts_with("scenario_assumption:")}
pub fn active(w:&World,id:EntityId)->bool {
 w.rules.profile==super::damage_runtime::RuleProfile::Rts2026Full &&
 w.entity(id).is_some_and(|e|e.ext.get("damage.beached")==Some(&1.))
}
pub fn capacity(w:&World,id:EntityId)->Option<f64>{
 let e=w.entity(id)?;Some(if active(w,id){e.ext["damage.beached.capacity"]}else{e.hp_max})
}
pub fn damage_taken(w:&World,id:EntityId)->Option<f64>{
 let e=w.entity(id)?;Some((capacity(w,id)?-e.hp).max(0.))
}
/// The reserve only changes destruction capacity, not listed-DP operational
/// thresholds such as §14.2.1 or the original speed/damage breakdown.
pub fn listed_remaining(e:&crate::world::Entity)->f64 {
 if e.ext.get("damage.beached")==Some(&1.) {
  e.hp-(e.ext.get("damage.beached.capacity").copied().unwrap_or(e.hp_max)-e.hp_max)
 }else{e.hp}
}
fn trace(w:&mut World,id:EntityId,rule:&str,detail:Value){
 let side=w.entity(id).map(|e|e.side);
 w.emit(EventKind::RulesEvent{domain:"beached2026".into(),rule:rule.into(),side,unit:Some(id),contact:None,detail:detail.to_string()},
 side.map(Visibility::Side).unwrap_or(Visibility::Referee));
}
fn body(w:&World,id:EntityId)->Result<(),String>{
 if w.rules.profile!=super::damage_runtime::RuleProfile::Rts2026Full{return Err("14.2.4 requires the source Full profile".into())}
 let e=w.entity(id).filter(|e|e.alive).ok_or("14.2.4 live vessel required")?;
 if !matches!(e.kind,PlatformKind::Ship|PlatformKind::Sub)||e.depth_m!=0.||!e.hp_max.is_finite()||e.hp_max<=0.{
  return Err("14.2.4 exposed surface vessel with source DP required; bottom-grounded submerged subs are not inferred".into())
 }
 Ok(())
}
pub fn initialize(w:&mut World)->Result<(),String>{
 if w.rules.profile!=super::damage_runtime::RuleProfile::Rts2026Full{return Ok(())}
 let Some(c)=w.context.beached_2026.clone()else{return Ok(())};
 if !authored(&c.source){return Err("14.2.4 beach configuration requires author source".into())}
 for (name,d) in &c.units{
  if !authored(&d.source)||!d.maximum_distance_nm.is_finite()||d.maximum_distance_nm<=0. ||
      !d.shore_position.x.is_finite()||!d.shore_position.y.is_finite(){return Err("14.2.4 declared shore geometry/source required".into())}
  let ids:Vec<_>=w.entities.values().filter(|e|&e.name==name).map(|e|e.id).collect();
  if ids.len()!=1{return Err("14.2.4 initial declared vessel must resolve uniquely".into())}
  body(w,ids[0])?;
  if d.initially_beached {validate_beach(w,ids[0])?;mark(w,ids[0],None)?;}
 }
 Ok(())
}
pub fn validate_beach(w:&World,id:EntityId)->Result<(),String>{
 body(w,id)?;
 if active(w,id){return Err("14.2.4 vessel already beached".into())}
 let e=w.entity(id).unwrap();
 let c=w.context.beached_2026.as_ref().ok_or("14.2.4 explicit shore declaration required")?;
 let d=c.units.get(&e.name).ok_or("14.2.4 own declared shore assignment absent")?;
 if !authored(&c.source)||!authored(&d.source)||e.speed_kts.abs()>1e-9 ||
    e.pos.range_to(d.shore_position)>d.maximum_distance_nm {
  return Err("14.2.4 stop at the explicitly authored shore before beaching".into())
 }
 Ok(())
}
/// Called after qualified §3.4 surface grounding as well as explicit beaching.
pub fn mark(w:&mut World,id:EntityId,refloat_due:Option<i64>)->Result<(),String>{
 body(w,id)?;
 if active(w,id){return Ok(())}
 let multiplier=n("destroy_original_dp_multiple");
 let now=w.tick;
 let e=w.entity_mut(id).unwrap();
 let original=e.hp_max;
 let suffered=(original-e.hp).max(0.);
 let cap=original*multiplier;
 e.hp=cap-suffered;
 e.ext.insert("damage.beached".into(),1.);
 e.ext.insert("damage.beached.original_dp".into(),original);
 e.ext.insert("damage.beached.capacity".into(),cap);
 e.ext.insert("damage.beached.since".into(),now as f64);
 if let Some(at)=refloat_due{e.ext.insert("damage.beached.refloat_due".into(),at as f64);}
 e.speed_kts=0.;e.ordered_speed_kts=0.;e.station=None;e.waypoints.clear();
 trace(w,id,"14.2.4.beached",json!({"listed_dp":original,"destruction_capacity":cap,"damage_already_taken":suffered,"refloat_due":refloat_due,"policy":tables()["execution_policy"]}));
 Ok(())
}
pub fn flood_modifier(w:&World,id:EntityId)->u8 {
 if active(w,id){(-n("flood_severity_d6_modifier")) as u8}else{0}
}
pub fn validate_refloat(w:&World,id:EntityId,timer:bool)->Result<(),String>{
 body(w,id)?;
 if !active(w,id){return Err("14.2.4 vessel is not beached".into())}
 let e=w.entity(id).unwrap();
 if damage_taken(w,id).unwrap()>e.ext["damage.beached.original_dp"] {
  return Err("14.2.4 damage strictly exceeds listed DP: cannot refloat, not automatically destroyed".into())
 }
 if let Some(at)=e.ext.get("damage.beached.refloat_due"){
  if w.tick<*at as i64{return Err("3.4 original grounding freeing clock has not elapsed".into())}
 }else if !timer && !w.context.beached_2026.as_ref().and_then(|c|c.units.get(&e.name)).is_some_and(|d|d.refloat_authorized){
  return Err("14.2.4 no scenario-authorized completion of refloating work".into())
 }
 Ok(())
}
pub fn refloat(w:&mut World,id:EntityId,timer:bool)->Result<(),String>{
 validate_refloat(w,id,timer)?;
 let d=damage_taken(w,id).unwrap();
 let e=w.entity_mut(id).unwrap();
 e.hp=e.hp_max-d;
 e.ext.remove("damage.beached");
 e.ext.remove("damage.beached.capacity");
 e.ext.remove("damage.beached.refloat_due");
 trace(w,id,"14.2.4.refloated",json!({"damage_taken":d,"no_damage_repaired":true}));
 if w.entity(id).is_some_and(|e|e.hp<=0.){crate::damage::sink(w,id);}
 Ok(())
}
pub fn hold(w:&mut World,id:EntityId)->bool{
 if !active(w,id){return false}
 let e=w.entity_mut(id).unwrap();e.speed_kts=0.;e.ordered_speed_kts=0.;e.station=None;e.waypoints.clear();true
}
pub fn own_view(w:&World,id:EntityId)->Option<Value>{
 if !active(w,id){return None}
 let e=w.entity(id)?;
 Some(json!({"beached":true,"listed_dp":e.hp_max,"destruction_capacity":capacity(w,id),"damage_taken":damage_taken(w,id),"cannot_refloat":damage_taken(w,id).unwrap()>e.hp_max,"refloat_due":e.ext.get("damage.beached.refloat_due")}))
}


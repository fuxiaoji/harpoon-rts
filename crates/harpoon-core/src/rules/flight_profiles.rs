//! Source-backed flight envelopes and range-cost multipliers. Existing seconds
//! of fuel are cruise-equivalent budget, not a new claim of tank capacity.
use crate::{World,EntityId,world::Entity};
use std::{sync::OnceLock,collections::BTreeMap};
use serde::Deserialize;
#[derive(Deserialize)]struct Profile {low:[f64;3],medium:[f64;3],high:[f64;3]}
#[derive(Deserialize)]struct Data {low_range_cost:f64,fmp_range_cost:f64,reheat_range_cost:f64,loiter_range_cost:f64,vlow_max_m:f64,vlow_absolute_cap_kts:f64,jets:BTreeMap<String,Profile>}
fn data()->&'static Data {static D:OnceLock<Data>=OnceLock::new();D.get_or_init(||serde_json::from_str(include_str!("../../../../data/rules/flight-profiles.json")).unwrap())}
fn speeds(e:&Entity,p:&Profile)->[f64;3] {if e.altitude_m<=2000.0 {p.low}else if e.altitude_m<=7500.0 {p.medium}else{p.high}}
pub fn supported(class:&str)->bool{data().jets.contains_key(class)}
pub fn cruise_at(class:&str,altitude:f64)->Option<f64>{let p=data().jets.get(class)?;Some(if altitude<=2000.0 {p.low[0]}else if altitude<=7500.0 {p.medium[0]}else{p.high[0]})}
pub fn speed_cap(e:&Entity)->Option<f64>{
    let p=data().jets.get(&e.class)?;
    Some(if e.altitude_m<=data().vlow_max_m {p.low[1].min(data().vlow_absolute_cap_kts)}else{speeds(e,p)[2]})
}
pub fn configure(w:&mut World,id:EntityId,setting:&str)->Result<(),String> {
    let e=w.entity(id).ok_or("no aircraft")?;let p=data().jets.get(&e.class).ok_or("no verified throttle profile for this aircraft")?;
    let mode=match setting {"cruise"=>0,"fmp"=>1,"reheat"=>2,"loiter"=>3,_=>return Err("throttle must be cruise/fmp/reheat/loiter".into())};
    if e.altitude_m<=60.0 && mode==2 {return Err("VLow prohibits afterburner; full military or 650kt, whichever is lower (§4.5.4)".into());}
    let speed=if mode==3 {0.0}else{speeds(e,p)[mode]};
    let e=w.entity_mut(id).unwrap();e.ext.insert("flight.throttle".into(),mode as f64);e.ordered_speed_kts=speed;Ok(())
}
pub fn fuel_rate(e:&Entity,cruise:f64)->f64 {
    let Some(p)=data().jets.get(&e.class) else {return 1.0};
    let d=data();let mode=e.ext.get("flight.throttle").copied().unwrap_or(0.0) as i32;
    if mode==3 && e.speed_kts<1.0 {return d.loiter_range_cost;}
    let throttle=if mode==2 {d.reheat_range_cost}else if mode==1 || e.speed_kts>speeds(e,p)[0]+5.0 {d.fmp_range_cost}else{1.0};
    (e.speed_kts/cruise.max(1.0)).max(0.2)*throttle*if e.altitude_m<=2000.0 {d.low_range_cost}else{1.0}
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn source_speed_rows_and_costs(){assert_eq!(data().jets["cn_j15"].low[1],600.0);assert_eq!(data().jets["us_f18e"].medium[2],815.0);assert_eq!(data().low_range_cost*data().fmp_range_cost,6.0);assert!(!supported("aew"));}
}

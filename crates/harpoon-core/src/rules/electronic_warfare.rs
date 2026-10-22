//! Source-backed noise jamming (§12.5). Tables live outside controllers/UI.
//! Unknown radar generations do not acquire an invented susceptibility.
use crate::{World,EntityId,ContactId,db::{SensorDef,SensorKind},world::Entity,geom::{angle_diff,horizon_nm}};
use serde::{Deserialize,Serialize};
use std::{collections::BTreeMap,sync::OnceLock};

#[derive(Deserialize)]struct Tables {resistance:Vec<i32>,sea_clutter_low_air:Vec<i32>,precipitation_clutter:BTreeMap<String,i32>,main_arc_deg:Vec<f64>,sidelobe_deg:Vec<f64>,range_modifiers:Vec<f64>,radar_generations:BTreeMap<String,usize>,radar_sensor_generations:BTreeMap<String,usize>,jammers:BTreeMap<String,Jammer>}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct RadarEnvironment {pub sea_state:u8,pub precipitation:String}
impl RadarEnvironment {pub fn valid(&self)->bool{self.sea_state<=9 && data().precipitation_clutter.contains_key(&self.precipitation)}}
#[derive(Deserialize)]pub struct Jammer {pub clutter:i32,pub spot_targets:usize,pub standoff:bool,pub source:String}
fn data()->&'static Tables {static D:OnceLock<Tables>=OnceLock::new();D.get_or_init(||serde_json::from_str(include_str!("../../../../data/rules/electronic-warfare.json")).expect("verified EW tables"))}
pub fn capability(class:&str)->Option<&'static Jammer>{data().jammers.get(class)}
pub fn emitting(e:&Entity)->bool {e.ext.get("ew.mode").copied().unwrap_or(0.0)>0.0}
fn generation(radar:&Entity,def:&SensorDef)->Option<usize>{data().radar_sensor_generations.get(&def.id).or_else(||data().radar_generations.get(&radar.class)).copied()}

pub fn configure(w:&mut World,id:EntityId,mode:&str,contacts:&[ContactId])->Result<(),String> {
    if mode!="off" && w.context.radar_environment.as_ref().is_none_or(|e|!e.valid()) {return Err("offensive EW requires explicit radar_environment clutter conditions (§5.2.9.4/12.5)".into());}
    if mode!="off" && w.weather.jam.iter().any(|j|*j>0.0) {return Err("do not combine legacy weather.jam with the source-backed noise-jamming model".into());}
    let e=w.entity(id).ok_or("no aircraft")?;
    let cap=capability(&e.class).ok_or("no verified offensive jammer fit")?;
    let value=match mode {"off"=>0.0,"standoff" if cap.standoff=>1.0,"escort"=>2.0,"spot" if cap.spot_targets>0=>3.0,_=>return Err("unsupported jamming mode for this fitted pod".into())};
    if mode=="spot" {
        if contacts.is_empty()||contacts.len()>cap.spot_targets {return Err("spot target count exceeds fitted pods".into());}
        for cid in contacts {
            let c=w.contact(e.side,*cid).ok_or("spot target absent")?;
            if c.stale || !c.sensors.iter().any(|s|w.db.sensors.get(s).is_some_and(|d|d.kind==SensorKind::Esm)) {return Err("spot requires a current ES/RWR detection of the radiating radar".into());}
        }
    }
    let targets:Vec<_>=contacts.iter().filter_map(|c|w.contact(e.side,*c).map(|c|c.about)).collect();
    let e=w.entity_mut(id).unwrap();e.ext.retain(|k,_|!k.starts_with("ew.spot."));e.ext.insert("ew.mode".into(),value);
    for target in targets {e.ext.insert(format!("ew.spot.{}",target.0),1.0);}
    Ok(())
}

fn angular_multiplier(generation:usize,clutter:i32,separation:f64)->f64 {
    let d=data();let mut net=(clutter-d.resistance[generation]).max(0);
    if separation>d.main_arc_deg[generation]/2.0 {
        if separation>d.main_arc_deg[generation]/2.0+d.sidelobe_deg[generation] {return 1.0;}
        net/=2;
    }
    d.range_modifiers[net.min(20) as usize]
}

/// Observable interference at the affected radar; never report the hidden
/// jammer id or position to its opponent as a consequence of this trace.
pub fn trace(w:&mut World) {
    if w.tick%60!=0 {return;}
    let mut results=Vec::new();
    for o in w.entities.values().filter(|e|e.alive && crate::sensors::radar_emitting(e)) {
        for sensor in &o.sensors {
            let Some(def)=w.db.sensors.get(&sensor.sensor) else {continue};
            let mut m:f64=1.0;
            for target in w.entities.values().filter(|e|e.alive && e.side!=o.side && emitting(e)) {m=m.min(range_modifier(w,o,def,target));}
            if m<1.0 {results.push((o.side,o.id,sensor.sensor.clone(),m));}
        }
    }
    for (side,unit,sensor,m) in results {
        w.emit(crate::EventKind::RulesEvent {domain:"electronic_warfare".into(),rule:"noise_interference".into(),side:Some(side),unit:Some(unit),contact:None,
            detail:format!("H5-2026-12.5; radar={sensor}; main-bearing range multiplier={m}; other bearings may differ; jammer identities/positions withheld")},crate::Visibility::Side(side));
    }
}

pub fn range_modifier(w:&World,radar:&Entity,def:&SensorDef,target:&Entity)->f64 {
    if def.kind!=SensorKind::Radar || !def.sensor_is_primary_air_search() {return 1.0;}
    let Some(generation)=generation(radar,def) else {return 1.0;};
    range_modifier_for(w,radar,def,target,generation)
}
pub(crate) fn range_modifier_for(w:&World,radar:&Entity,def:&SensorDef,target:&Entity,generation:usize)->f64 {
    if def.kind!=SensorKind::Radar||!(1..=6).contains(&generation){return 1.0;}
    let mut modifier:f64=1.0;
    for j in w.entities.values().filter(|j|j.alive && j.side!=radar.side && emitting(j)) {
        let Some(cap)=capability(&j.class) else {continue};
        if radar.pos.range_to(j.pos)>horizon_nm(crate::sensors::observer_height(radar,def),j.altitude_m) {continue;}
        let mode=j.ext.get("ew.mode").copied().unwrap_or(0.0) as i32;
        if mode==3 && !j.ext.contains_key(&format!("ew.spot.{}",radar.id.0)) {continue;}
        if mode==2 && angle_diff(j.course_deg,j.pos.bearing_to(radar.pos)).abs()>15.0 {continue;}
        let clutter=if mode==2 && !cap.standoff {cap.clutter/2}else{cap.clutter};
        let separation=angle_diff(radar.pos.bearing_to(j.pos),radar.pos.bearing_to(target.pos)).abs();
        modifier=modifier.min(angular_multiplier(generation,clutter,separation));
    }
    modifier
}

/// §5.2.9.4 uses the largest source of clutter, not a product. The old fixed
/// `<50m ×.60` estimate is replaced only when radar and environment are verified.
pub fn combined_modifier(w:&World,radar:&Entity,def:&SensorDef,target:&Entity)->Option<f64> {
    if def.kind!=SensorKind::Radar || !def.sensor_is_primary_air_search(){return None;}
    let generation=generation(radar,def)?;
    let env=w.context.radar_environment.as_ref()?;
    if !env.valid(){return None;}
    let low=target.kind==crate::types::PlatformKind::Ship || matches!(target.kind,crate::types::PlatformKind::Aircraft|crate::types::PlatformKind::Missile)&&target.altitude_m<=60.0;
    let sea=if low {data().sea_clutter_low_air[env.sea_state as usize]}else{0};
    let clutter=sea.max(data().precipitation_clutter[&env.precipitation]);
    let net=(clutter-data().resistance[generation]).clamp(0,20) as usize;
    Some(data().range_modifiers[net].min(range_modifier(w,radar,def,target)))
}

impl SensorDef {fn sensor_is_primary_air_search(&self)->bool {self.radar_class.eq_ignore_ascii_case("LAS")||self.id.starts_with("radar_air")}}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn official_fencer_example_and_sidelobe_rounding(){
        assert_eq!(angular_multiplier(3,13,0.0),0.4);
        assert_eq!(angular_multiplier(3,13,45.0),0.7);
        assert_eq!(angular_multiplier(4,13,0.0),0.85);
        assert_eq!(angular_multiplier(4,13,25.0),1.0);
        assert_eq!(angular_multiplier(3,13,61.0),1.0);
    }
    #[test]fn source_fit_capacity_and_modern_resistance(){assert_eq!(capability("us_ea18g").unwrap().spot_targets,6);assert_eq!(angular_multiplier(6,14,0.0),1.0);assert!(capability("fighter").is_none());}
}

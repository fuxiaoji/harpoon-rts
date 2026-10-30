//! §14.1.8 and §5.4.6.5. Discrete game-table effects and explicit fuze lifecycle.
//! No yield interpolation, real-world blast physics, EMP or fallout surrogate.
use crate::{EntityId, Side, World, Vec2, EventKind, Visibility, Rng};
use crate::types::PlatformKind;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use std::{collections::{BTreeMap,BTreeSet},sync::OnceLock};
#[path="nuclear2026/standoff.rs"]
mod standoff;
pub use standoff::{accept_standoff_nuclear,validate_standoff_nuclear,standoff_outcome,AswOutcome};
#[path="nuclear2026/asw_remaining.rs"]
pub mod asw_remaining;

pub fn tables()->&'static Value {static T:OnceLock<Value>=OnceLock::new();T.get_or_init(||serde_json::from_str(include_str!("../../../../data/rules/nuclear-2026.json").trim_start_matches('\u{feff}')).expect("source nuclear table"))}
fn rule(name:&str)->f64 {tables()["rules"][name].as_f64().expect("source nuclear rule")}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Medium {Surface,Subsurface,Air}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Fuze {TerminalSeeker,TimeToTarget,FreeFall}
#[derive(Clone,Debug,Default,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct Configuration {pub source:String,pub units:BTreeMap<String,BTreeMap<String,Warhead>>}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Warhead {
    pub source:String,
    /// Exact game-annex candidate, or an explicitly labelled source-table laboratory.
    pub source_variant:String,
    pub allowed_yields_kt:Vec<u32>,
    pub selected_yield_kt:u32,
    pub medium:Medium,
    pub fuze:Fuze,
    pub authorized:bool,
}
#[derive(Clone,Debug,Default,Serialize,Deserialize)]
#[serde(default)]
pub struct State {
    pub fits:BTreeMap<EntityId,BTreeMap<String,Warhead>>,
    flights:BTreeMap<EntityId,Flight>,
    bursts:BTreeMap<String,Burst>,
    resolved:BTreeSet<String>,
    blueout:BTreeMap<String,Blueout>,
    pending:BTreeMap<String,String>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
struct Flight {weapon:String,warhead:Warhead,shooter:EntityId,side:Side,armed:bool,acquired:bool,terminal:bool,last_pos:Vec2,last_course:f64,speed_kts:f64,altitude_m:f64,
    #[serde(default,skip_serializing_if="Option::is_none")] surface_sam_hit:Option<EntityId>,
    #[serde(default,skip_serializing_if="Option::is_none")] hit_target:Option<EntityId>}
#[derive(Clone,Debug,Serialize,Deserialize)]
struct Burst {id:String,at:i64,pos:Vec2,medium:Medium,altitude_m:f64,yield_kt:u32,side:Side,shooter:EntityId,weapon:String,cause:String,
    #[serde(default,skip_serializing_if="Option::is_none")] primary_asw:Option<standoff::Primary>,
    #[serde(default,skip_serializing_if="Option::is_none")] surface_sam_hit:Option<EntityId>}
#[derive(Clone,Debug,Serialize,Deserialize)]
struct Blueout {until:i64,burst:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(tag="type",rename_all="snake_case",deny_unknown_fields)]
pub enum Action {SetYield{weapon:String,yield_kt:u32},Authorize{weapon:String,enabled:bool}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Removal {Hit,GuidedMiss,LostAcquiredLock,CarrierDestroyed,Runout,Unresolved}
#[derive(Clone,Debug,PartialEq)]
pub enum Effect {None,Destroyed,SurfaceDamage,AircraftAbort,Pending(&'static str)}
/// Created only after genuine nuclear primary DP has been committed. Shared
/// signature consumers receive a typed provenance, not a fabricated torpedo.
pub struct PrimaryDamageEvidence {identity:String,weapon:String,tick:i64,medium:Medium,dp:f64}
impl PrimaryDamageEvidence {
    pub fn identity(&self)->&str {&self.identity}
    pub fn weapon(&self)->&str {&self.weapon}
    pub fn tick(&self)->i64 {self.tick}
    pub fn medium(&self)->Medium {self.medium}
    pub fn dp(&self)->f64 {self.dp}
}
pub fn radii(yield_kt:u32)->Result<[f64;3],String>{
    let row=tables()["rows"].as_array().unwrap().iter().find(|r|r[0].as_u64()==Some(u64::from(yield_kt))).ok_or("14.1.8 exact yield row absent; interpolation forbidden")?;
    Ok([row[1].as_f64().unwrap(),row[2].as_f64().unwrap(),row[3].as_f64().unwrap()])
}
fn active(w:&World)->bool {w.rules.profile==super::damage_runtime::RuleProfile::Rts2026Full&&w.rules.nuclear.is_some()}
fn source_fit(f:&Warhead)->Result<(),String>{
    if f.source.trim().is_empty()||f.allowed_yields_kt.is_empty()||!f.allowed_yields_kt.contains(&f.selected_yield_kt){return Err("14.1.8 incomplete exact warhead source/selection".into())}
    for y in &f.allowed_yields_kt {radii(*y)?;}
    if !f.source_variant.starts_with("scenario_assumption:") {
        let row=tables()["annex_yield_candidates"].get(&f.source_variant).ok_or("14.1.8 unknown source variant; no implicit nuclear counterpart")?;
        if !f.allowed_yields_kt.iter().all(|y|row["yield_kt"].as_array().unwrap().iter().any(|n|n.as_u64()==Some(u64::from(*y)))) {return Err("14.1.8 yield not offered by exact Annex row".into())}
    }
    Ok(())
}
pub fn initialize(w:&mut World)->Result<(),String>{
    if w.rules.profile!=super::damage_runtime::RuleProfile::Rts2026Full{return Ok(())}
    let Some(config)=w.context.nuclear_2026.clone() else{return Ok(())};
    if config.source.trim().is_empty(){return Err("14.1.8 nuclear scenario source required".into())}
    let mut state=State::default();
    for (name,fits) in config.units {
        let rows:Vec<_>=w.entities.values().filter(|e|e.name==name).collect();
        if rows.len()!=1{return Err("14.1.8 exact unique nuclear carrier required".into())}
        let e=rows[0];
        for (weapon,fit) in &fits {source_fit(fit)?;if !w.db.weapons.contains_key(weapon)||!e.weapons.iter().any(|m|m.weapon==*weapon){return Err("14.1.8 warhead lacks an actual installed weapon and complete delivery definition".into())}}
        state.fits.insert(e.id,fits);
    }
    w.rules.nuclear=Some(state);Ok(())
}
pub fn configured(w:&World,actor:EntityId,weapon:&str)->bool {active(w)&&w.rules.nuclear.as_ref().is_some_and(|s|s.fits.get(&actor).is_some_and(|f|f.contains_key(weapon)))}
pub fn release_gate(w:&World,actor:EntityId,weapon:&str)->Result<(),String>{
    if !configured(w,actor,weapon){return Ok(())}
    let f=&w.rules.nuclear.as_ref().unwrap().fits[&actor][weapon];source_fit(f)?;
    if !f.authorized{return Err("14.1.8 this nuclear weapon is not authorized for release".into())}Ok(())
}
pub fn validate_action(w:&World,side:Side,actor:EntityId,action:&Action)->Result<(),String>{
    if !active(w)||w.over||crate::exercise::locked(w,side)||!w.entity(actor).is_some_and(|e|e.alive&&e.side==side){return Err("14.1.8 own active nuclear carrier required".into())}
    let (weapon,yield_kt)=match action {Action::SetYield{weapon,yield_kt}=>(weapon,Some(*yield_kt)),Action::Authorize{weapon,..}=>(weapon,None)};
    let f=w.rules.nuclear.as_ref().unwrap().fits.get(&actor).and_then(|f|f.get(weapon)).ok_or("14.1.8 exact installed nuclear variant required")?;source_fit(f)?;
    if yield_kt.is_some_and(|y|!f.allowed_yields_kt.contains(&y)){return Err("14.1.8 requested yield is not an explicit selectable source value".into())}
    Ok(())
}
pub fn apply(w:&mut World,side:Side,actor:EntityId,action:&Action)->Result<(),String>{
    validate_action(w,side,actor,action)?;
    let (weapon,yield_kt,authorization)=match action {Action::SetYield{weapon,yield_kt}=>(weapon,Some(*yield_kt),None),Action::Authorize{weapon,enabled}=>(weapon,None,Some(*enabled))};
    let f=w.rules.nuclear.as_mut().unwrap().fits.get_mut(&actor).unwrap().get_mut(weapon).unwrap();
    if let Some(y)=yield_kt {f.selected_yield_kt=y}if let Some(a)=authorization {f.authorized=a}
    // Existing flights retain their own immutable warhead snapshots.
    Ok(())
}
fn trace(w:&mut World,unit:EntityId,rule:&str,detail:Value){w.emit(EventKind::RulesEvent{domain:"nuclear2026".into(),rule:rule.into(),side:None,unit:Some(unit),contact:None,detail:detail.to_string()},Visibility::Referee)}
fn die(w:&World,id:&str,sides:u8)->u8 {let mut r=w.rng.derive(Rng::hash_tag(0,0,id));1+(r.next_f64()*f64::from(sides)).floor() as u8}
/// Called before/after each actual delivery update, never by a player payload.
pub fn observe_flight(w:&mut World,id:EntityId){
    if !active(w){return}
    let Some(e)=w.entity(id).cloned() else{return};let Some(p)=e.projectile.as_ref() else{return};
    let existing=w.rules.nuclear.as_ref().unwrap().flights.get(&id).cloned();
    let mut f=if let Some(f)=existing {f}else{
        let Some(fit)=w.rules.nuclear.as_ref().unwrap().fits.get(&p.launched_by).and_then(|f|f.get(&p.weapon)).cloned() else{return};
        Flight{weapon:p.weapon.clone(),warhead:fit,shooter:p.launched_by,side:p.launched_by_side,armed:false,acquired:false,terminal:false,last_pos:e.pos,last_course:e.course_deg,speed_kts:e.speed_kts,altitude_m:e.altitude_m,surface_sam_hit:None,hit_target:None}
    };
    if f.terminal{return}
    let was_armed=f.armed;
    f.armed|=match f.warhead.fuze {Fuze::FreeFall=>true,Fuze::TerminalSeeker=>p.enabled,Fuze::TimeToTarget=>e.speed_kts>0.&&e.pos.range_to(p.aim)/e.speed_kts*3600.<=rule("no_terminal_seeker_arming_tta_seconds")};
    // A nominated target is a release intention, not a seeker acquisition.
    f.last_pos=e.pos;f.last_course=e.course_deg;f.speed_kts=e.speed_kts;f.altitude_m=e.altitude_m;
    if let Some(target)=f.hit_target.and_then(|target|w.entity(target)) {f.last_pos=target.pos;f.altitude_m=target.altitude_m;}
    w.rules.nuclear.as_mut().unwrap().flights.insert(id,f.clone());
    if !was_armed&&f.armed {trace(w,f.shooter,"14.1.8.fuzed",json!({"projectile":id,"fuze":f.warhead.fuze,"yield_kt":f.warhead.selected_yield_kt}))}
}
/// Trusted delivery hook for a physically activated seeker/actual lock.
/// No player payload or target assignment can call this transition.
pub fn seeker_activated(w:&mut World,id:EntityId,acquired:bool){
    observe_flight(w,id);
    let Some(f)=w.rules.nuclear.as_mut().and_then(|s|s.flights.get_mut(&id))else{return};
    if f.terminal{return}
    let was_armed=f.armed;
    if f.warhead.fuze==Fuze::TerminalSeeker{f.armed=true;}
    f.acquired|=acquired;
    let transition=(!was_armed&&f.armed).then(||(f.shooter,f.warhead.fuze.clone(),f.warhead.selected_yield_kt));
    if let Some((shooter,fuze,yield_kt))=transition {
        trace(w,shooter,"14.1.8.fuzed",json!({"projectile":id,"fuze":fuze,"yield_kt":yield_kt}));
    }
}
/// Successful/failed guidance is distinct from the warhead's D10 function.
pub fn terminal(w:&mut World,id:EntityId,hit:bool)->bool{
    terminal_with_target(w,id,hit,None)
}
/// Only a genuine adjudicated hit may provide this target. A nomination or
/// missed attack cannot qualify the special low-altitude nuclear-SAM branch.
pub fn terminal_with_target(w:&mut World,id:EntityId,hit:bool,actual_target:Option<EntityId>)->bool{
    if !is_nuclear_projectile(w,id){return false}
    seeker_activated(w,id,hit);
    if hit {
        if let Some(t)=actual_target.and_then(|target|w.entity(target)).cloned() {
            let sam=w.rules.nuclear.as_ref().and_then(|s|s.flights.get(&id))
                .and_then(|f|w.db.weapons.get(&f.weapon)).is_some_and(|d|d.kind==crate::db::WeaponKind::Sam);
            if let Some(f)=w.rules.nuclear.as_mut().and_then(|s|s.flights.get_mut(&id)) {
                if !f.terminal {
                    f.hit_target=Some(t.id);
                    f.last_pos=t.pos;
                    f.altitude_m=t.altitude_m;
                    if sam&&f.warhead.medium==Medium::Air&&t.kind==PlatformKind::Ship {
                        f.surface_sam_hit=Some(t.id);
                    }
                }
            }
        }
    }
    removed(w,id,if hit{Removal::Hit}else{Removal::GuidedMiss})
}
pub fn is_nuclear_projectile(w:&World,id:EntityId)->bool {
    active(w)&&(w.rules.nuclear.as_ref().unwrap().flights.contains_key(&id)||w.entity(id).and_then(|e|e.projectile.as_ref()).is_some_and(|p|configured(w,p.launched_by,&p.weapon)))
}
/// A typed physical outcome, not an inferred bounded event-log string.
pub fn removed(w:&mut World,id:EntityId,reason:Removal)->bool{
    if !is_nuclear_projectile(w,id){return false}
    observe_flight(w,id);let Some(mut f)=w.rules.nuclear.as_ref().unwrap().flights.get(&id).cloned() else{return true};
    if f.terminal{return true}f.terminal=true;
    let key=format!("14.1.8:{}",id.0);
    let mut burst_at=None;
    match reason {
        Removal::Hit if f.armed=>burst_at=Some(w.tick),
        Removal::LostAcquiredLock if f.armed&&f.acquired=>burst_at=Some(w.tick),
        Removal::GuidedMiss if f.armed=>burst_at=Some(w.tick+rule("miss_continue_seconds") as i64),
        Removal::CarrierDestroyed if f.armed=>{
            let d=die(w,&format!("{key}:carrier_destroyed"),100);let detonates=f64::from(d)<=100.*rule("armed_carrier_destroyed_probability");
            trace(w,f.shooter,"14.1.8.carrier_destroyed",json!({"projectile":id,"d100":d,"detonates":detonates}));if detonates{burst_at=Some(w.tick)}
        },
        Removal::Unresolved|Removal::Runout if f.armed=>{w.rules.nuclear.as_mut().unwrap().pending.insert(key.clone(),"Armed delivery ended without a source-qualified terminal cause; no conventional damage or invented detonation".into());},
        _=>{},
    }
    w.rules.nuclear.as_mut().unwrap().flights.insert(id,f.clone());
    if let Some(at)=burst_at{
        let pos=if reason==Removal::GuidedMiss{f.last_pos.advanced(f.last_course,f.speed_kts*rule("miss_continue_seconds")/3600.)}else{f.last_pos};
        w.rules.nuclear.as_mut().unwrap().bursts.insert(key.clone(),Burst{id:key,at,pos,medium:f.warhead.medium,altitude_m:f.altitude_m,yield_kt:f.warhead.selected_yield_kt,side:f.side,shooter:f.shooter,weapon:f.weapon,cause:format!("{reason:?}"),primary_asw:None,surface_sam_hit:f.surface_sam_hit});
    }
    true
}
/// Boundary inclusivity is the explicit versioned RTS policy, not a new row.
pub fn effect(yield_kt:u32,medium:Medium,kind:PlatformKind,surfaced:bool,low_air:bool,range:f64)->Result<Effect,String>{
    if !range.is_finite()||range<0.{return Err("14.1.8 invalid game-table range".into())}
    let [ship,air,damage]=radii(yield_kt)?;
    Ok(match kind {
        PlatformKind::Aircraft|PlatformKind::Missile=>{
            if medium==Medium::Subsurface {if low_air&&range<=ship{Effect::Destroyed}else{Effect::None}}
            else if range<=air{Effect::Destroyed}else if range<=damage{Effect::AircraftAbort}else{Effect::None}
        },
        PlatformKind::Ship=>{
            if medium==Medium::Air {if range<=damage{Effect::Pending("14.1.8 airburst against surface vessel requires separately qualified nuclear-SAM surface-hit branch")}else{Effect::None}}
            else {let factor=if medium==Medium::Subsurface{rule("opposite_medium_range_factor")}else{1.};if range<=ship*factor{Effect::Destroyed}else if range<=damage*factor{Effect::SurfaceDamage}else{Effect::None}}
        },
        PlatformKind::Sub=>{let factor=if medium==Medium::Surface&&!surfaced{rule("opposite_medium_range_factor")}else{1.};if medium==Medium::Air{Effect::Pending("14.1.8 airburst versus submarine unspecified")}else if range<=ship*factor{Effect::Destroyed}else if range<=damage*factor{Effect::Pending("14.1.8 submarine damage-radius casualty not specified by the surface-ship D6 clause")}else{Effect::None}},
        PlatformKind::Base=>if range<=damage{Effect::Pending("14.1.8 has no generic facility nuclear DP/EMP/fallout procedure")}else{Effect::None},
        _=>Effect::None,
    })
}
pub fn blueout_active(w:&World,observer:EntityId,sensor:&str)->bool {
    active(w)&&w.rules.nuclear.as_ref().unwrap().blueout.get(&format!("{}:{sensor}",observer.0)).is_some_and(|b|w.tick<b.until)
}
fn physical_effect(w:&World,e:&crate::world::Entity,b:&Burst,range:f64)->Result<Effect,String>{
    if b.medium==Medium::Air&&b.surface_sam_hit.is_some()&&e.kind==PlatformKind::Ship {
        return effect(b.yield_kt,Medium::Surface,e.kind,true,false,range);
    }
    if e.kind==PlatformKind::Aircraft && e.altitude_m<=0. {
        let [ship,_,damage]=radii(b.yield_kt)?;
        if range>damage{return Ok(Effect::None)}
        if !super::ground2026::is_parked_aircraft(w,e.id){
            return Ok(Effect::Pending("14.1.8 ground/deck versus airborne state has no explicit physical parking binding"));
        }
        if b.medium!=Medium::Surface && b.surface_sam_hit.is_none() {
            return Ok(Effect::Pending("14.1.8 parked aircraft effects outside a surface burst require qualified source mapping"));
        }
        return Ok(if range<=ship{Effect::Destroyed}else{Effect::AircraftAbort});
    }
    effect(b.yield_kt,b.medium,e.kind,e.depth_m<=0.,super::operations::altitude_band(e.altitude_m)<=2,range)
}
pub fn step(w:&mut World){
    if !active(w){return}
    let due:Vec<_>=w.rules.nuclear.as_ref().unwrap().bursts.values().filter(|b|b.at<=w.tick&&!w.rules.nuclear.as_ref().unwrap().resolved.contains(&b.id)).cloned().collect();
    for burst in due {
        w.rules.nuclear.as_mut().unwrap().resolved.insert(burst.id.clone());
        let d10=die(w,&format!("{}:function",burst.id),10);let failed=f64::from(d10)==rule("detonation_failure_d10");
        trace(w,burst.shooter,"14.1.8.detonation",json!({"burst":burst,"d10":d10,"failed":failed,"source":"English2026 PDF131/132","policy":tables()["execution_policy"]["id"]}));
        if failed{continue}
        let victims:Vec<_>=w.entities.values().filter(|e|e.alive&&!super::ew2026::is_object(w,e.id)).cloned().collect();
        if burst.medium!=Medium::Air {
            let d6=die(w,&format!("{}:blueout",burst.id),6);let until=w.tick+((rule("blueout_base_hours")+f64::from(d6)*rule("blueout_additional_d6_hours"))*3600.) as i64;
            for e in &victims {if e.pos.range_to(burst.pos)>rule("blueout_range_nm"){continue}
                for sensor in &e.sensors {if !w.db.sensors.get(&sensor.sensor).is_some_and(|d|matches!(d.kind,crate::db::SensorKind::SonarActive|crate::db::SensorKind::SonarPassive)){continue}
                    let key=format!("{}:{}",e.id.0,sensor.sensor);let old=w.rules.nuclear.as_ref().unwrap().blueout.get(&key).map_or(0,|b|b.until);
                    if until>old{w.rules.nuclear.as_mut().unwrap().blueout.insert(key,Blueout{until,burst:burst.id.clone()});}
                }
            }
            trace(w,burst.shooter,"5.4.6.5.blueout",json!({"burst":burst.id,"d6":d6,"until":until,"sea_state":rule("blueout_sea_state"),"cz_allowed":false}));
        }
        let primary=standoff::resolve_primary(w,&burst);
        for e in victims {
            if primary==Some(e.id){continue;}
            let range=e.pos.range_to(burst.pos);let result=physical_effect(w,&e,&burst,range);
            let effect=match result{Ok(x)=>x,Err(reason)=>{w.rules.nuclear.as_mut().unwrap().pending.insert(format!("{}:{}",burst.id,e.id.0),reason);continue}};
            match effect {
                Effect::Destroyed=>{if e.kind.is_projectile(){removed(w,e.id,Removal::CarrierDestroyed);}super::expanded_damage2026::record_catastrophic_loss(w,e.id,&burst.id);crate::damage::sink_by(w,e.id,burst.side);},
                Effect::SurfaceDamage=>{
                    let d6=die(w,&format!("{}:{}:damage",burst.id,e.id.0),6);let dp=e.hp_max*f64::from(d6)*rule("ship_damage_original_dp_fraction_per_d6");
                    let hp=(e.hp-dp).max(0.);if let Some(t)=w.entity_mut(e.id){t.hp=hp;}
                    let evidence=PrimaryDamageEvidence {identity:format!("{}:{}:primary",burst.id,e.id.0),weapon:burst.weapon.clone(),tick:w.tick,medium:burst.medium,dp};
                    super::signature_damage2026::commit_nuclear(w,&e,&evidence);
                    w.emit(EventKind::Impact{side:e.side,unit:e.id,unit_name:e.name.clone(),weapon:burst.weapon.clone(),damage:dp,hp_left:hp},Visibility::Side(e.side));
                    trace(w,e.id,"14.1.8.ship_damage",json!({"burst":burst.id,"d6":d6,"original_dp":e.hp_max,"damage":dp}));
                    w.rules.nuclear.as_mut().unwrap().pending.insert(format!("{}:{}:systems_damage",burst.id,e.id.0),"PDF131 note1 specifies damaged weapons/sensors in the2psi ring but gives no per-component allocation; primary ship DP is committed, equipment detail remains pending".into());
                    if hp<=0.{super::expanded_damage2026::record_catastrophic_loss(w,e.id,&burst.id);crate::damage::sink_by(w,e.id,burst.side);}
                },
                Effect::AircraftAbort=>{
                    if let Some(t)=w.entity_mut(e.id){t.fire_on=false;t.assigned=None;t.ext.insert("nuclear.mission_abort".into(),1.);}
                    if e.kind.is_projectile(){
                        // Abort removes combat capability, without inventing a
                        // platform kill or an armed-carrier destruction roll.
                        removed(w,e.id,Removal::Unresolved);
                        if let Some(t)=w.entity_mut(e.id){t.alive=false;}
                    }
                    trace(w,e.id,"14.1.8.mission_abort",json!({"burst":burst.id,"kind":e.kind}));
                    if super::ground2026::is_parked_aircraft(w,e.id){
                        w.rules.nuclear.as_mut().unwrap().pending.insert(format!("{}:{}:parked_damage",burst.id,e.id.0),"PDF131 note1: parked aircraft damaged and mission aborted; no source-qualified per-airframe DP or component allocation supplied".into());
                    }
                },
                Effect::Pending(reason)=>{w.rules.nuclear.as_mut().unwrap().pending.insert(format!("{}:{}",burst.id,e.id.0),reason.into());trace(w,e.id,"14.1.8.pending",json!({"burst":burst.id,"reason":reason}));},
                Effect::None=>{},
            }
        }
    }
}
pub fn own_view(w:&World,id:EntityId)->Option<Value>{
    let state=w.rules.nuclear.as_ref()?;let fits=state.fits.get(&id)?;
    Some(json!({"warheads":fits.iter().map(|(weapon,f)|json!({"weapon":weapon,"selected_yield_kt":f.selected_yield_kt,"allowed_yields_kt":f.allowed_yields_kt,"authorized":f.authorized,"medium":f.medium,"fuze":f.fuze})).collect::<Vec<_>>() }))
}

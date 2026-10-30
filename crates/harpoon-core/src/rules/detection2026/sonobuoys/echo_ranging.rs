//! IEER/AEER finite charge and actual receiver localization, English PDF54.
use super::*;
use crate::rules::detection2026 as det;

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit { pub source:String, pub charges:u16, pub charge_mass_kg:f64, pub audible_radius_nm:f64 }
#[derive(Clone,Debug,Default,Serialize,Deserialize)]
#[serde(default)]
pub struct State { pub charges:BTreeMap<EntityId,u16>, pub last_turn:BTreeMap<EntityId,i64>, pub fixes:BTreeMap<String,EchoFix> }
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct EchoFix { pub receiver:EntityId, pub target:EntityId, pub buoy:EntityId, pub report:SensorReport, pub at:i64, pub good:bool, pub probability:f64, pub d100:u8 }

pub(super) fn initialize(w:&World, s:&mut super::State)->Result<(),String> {
    for (name,f) in &s.config.as_ref().unwrap().echo_fits {
        if !f.source.starts_with("scenario_assumption:") || f.charges==0 || !finite_positive(f.charge_mass_kg) || !finite_positive(f.audible_radius_nm) { return Err("5.4.6.3 IEER requires authored finite charges/mass and audible radius".into()); }
        let e=exact_body(w,name)?;
        let flight=w.rules.aviation.as_ref().and_then(|a|a.flights.get(&e.id)).ok_or("5.4.6.3 IEER flight state absent")?;
        let buoy_mass: f64 = s.carriers.get(&e.id).map(|c|c.carried.iter().map(|(model,n)|s.config.as_ref().unwrap().models[model].mass_kg * f64::from(*n)).sum()).unwrap_or(0.);
        if !s.monitors.contains_key(&e.id) || flight.spec.max_payload_kg * flight.payload_fraction + 1e-9 < buoy_mass + f.charge_mass_kg * f64::from(f.charges) {return Err("5.4.6.3 IEER requires actual monitor and finite authored payload".into());}
        s.echo.as_mut().unwrap().charges.insert(e.id,f.charges);
    }
    Ok(())
}
fn fit<'a>(w:&'a World,s:&'a super::State,id:EntityId)->Option<&'a Fit> {
    s.config.as_ref()?.echo_fits.get(&w.entity(id)?.name)
}
fn compatible(w:&World,s:&super::State,id:EntityId,m:&Monitor,f:&Float)->bool {
    if !radio(w,s,id,m,f) {return false;}
    let key=format!("{}:{}",f.class,f.sensor);
    matches!(w.rules.detection.as_ref().and_then(|d|d.config.bindings.get(&key)),Some(Receiver::Sonar{generation:6|7,active_range_nm:Some(x),frequency:Frequency::Lmf,..}) if finite_positive(*x))
}
pub(super) fn validate(w:&World,s:&super::State,id:EntityId,initial:EntityId)->Result<(),String> {
    let m=s.monitors.get(&id).ok_or("5.4.6.3 IEER monitor absent")?;
    let aircraft=w.entity(id).filter(|e|e.alive && e.kind==PlatformKind::Aircraft && e.side==m.side && e.altitude_m>0. && e.speed_kts>0.).ok_or("5.4.6.3 IEER airborne moving monitor required")?;
    let f=s.floats.get(&initial).filter(|f|m.selected.contains(&f.id) && radio(w,s,id,m,f)).ok_or("5.4.6.3 IEER selected live received buoy required")?;
    if aircraft.pos.range_to(w.entity(f.id).unwrap().pos)>tables()["echo_ranging"]["initial_detector_maximum_distance_nm"].as_f64().unwrap() {return Err("5.4.6.3 IEER aircraft must pass within 2nm of detecting buoy".into());}
    if m.selected.iter().filter_map(|bid|s.floats.get(bid)).filter(|f|compatible(w,s,id,m,f)).count()<2 {return Err("5.4.6.3 IEER requires at least two selected live compatible receivers".into());}
    let count=s.alerts.values().filter(|a|a.receiver==id && a.report.observer==initial && operator_alert(w,s,id,m,a.report.target).is_some()).count();
    if count!=1 {return Err("5.4.6.3 IEER requires one own received initial detection for this buoy".into());}
    if fit(w,s,id).is_none() || s.echo.as_ref().unwrap().charges.get(&id).copied().unwrap_or(0)==0 || s.echo.as_ref().unwrap().last_turn.get(&id)==Some(&epoch(w.tick)) {return Err("5.4.6.3 IEER finite charge or Tactical Turn slot unavailable".into());}
    let model=s.config.as_ref().unwrap().models.get(&f.model).unwrap();
    let binding=w.rules.detection.as_ref().unwrap().config.bindings.get(&format!("{}:{}",model.platform_class,model.sensor));
    if !matches!(binding,Some(Receiver::Sonar{generation:6|7,active_range_nm:Some(x),frequency:Frequency::Lmf,..}) if finite_positive(*x)) {return Err("5.4.6.3 IEER receiver requires qualified generation6/7 LMF active range".into());}
    Ok(())
}
pub(super) fn pulse(w:&mut World,id:EntityId,initial:EntityId)->Result<(),String> {
    let s=runtime(w).ok_or("5.4 buoy state absent")?; validate(w,s,id,initial)?;
    let m=&s.monitors[&id]; let alert=s.alerts.values().find(|a|a.receiver==id && a.report.observer==initial && operator_alert(w,s,id,m,a.report.target).is_some()).unwrap();
    let target_id=alert.report.target;
    let fit=fit(w,s,id).unwrap().clone(); let side=m.side;
    let flight=w.rules.aviation.as_mut().and_then(|a|a.flights.get_mut(&id)).ok_or("5.4.6.3 IEER flight absent")?;
    flight.payload_fraction=(flight.payload_fraction-fit.charge_mass_kg/flight.spec.max_payload_kg).max(0.);
    let turn=epoch(w.tick);
    let s=&mut w.rules.detection.as_mut().unwrap().sonobuoys;
    *s.echo.as_mut().unwrap().charges.get_mut(&id).unwrap()-=1;
    s.echo.as_mut().unwrap().last_turn.insert(id,turn);
    let remaining=s.echo.as_ref().unwrap().charges[&id];
    event(w,id,Some(side),"5.4.6.3.charge_used",json!({"charges_remaining":remaining}));
    let origin=w.entity(id).unwrap().pos;
    let warned:Vec<_>=w.entities.values().filter(|e| e.alive && e.kind==PlatformKind::Sub && e.pos.range_to(origin)<=fit.audible_radius_nm).map(|e|(e.id,e.side)).collect();
    for (sub,sub_side) in warned {event(w,sub,Some(sub_side),"5.4.6.3.echo_warning",json!({"audible":true,"source":"scenario_assumption: declared audible radius"}));}
    if let Err(reason)=adjudicate(w,id,initial,target_id,&fit) {
        event(w,id,None,"5.4.6.3.ieer_pending",json!({"reason":reason,"charge_committed":true}));
    }
    Ok(())
}
fn adjudicate(w:&mut World,id:EntityId,initial:EntityId,target_id:EntityId,_fit:&Fit)->Result<(),String> {
    let s=runtime(w).ok_or("5.4 buoy state absent")?;
    let m=s.monitors.get(&id).ok_or("5.4 monitor absent")?;
    let target=w.entity(target_id).filter(|e|e.alive && e.kind==PlatformKind::Sub).ok_or("5.4.6.3 IEER source target unavailable")?;
    let target=target.clone();
    let selected:Vec<_>=m.selected.iter().filter_map(|bid|s.floats.get(bid)).filter(|f|compatible(w,s,id,m,f)).cloned().collect();
    if selected.len()<2 {return Err("5.4.6.3 IEER compatible receivers unavailable after commit".into());}
    let nearest=selected.into_iter().min_by(|a,b|w.entity(a.id).unwrap().pos.range_to(target.pos).total_cmp(&w.entity(b.id).unwrap().pos.range_to(target.pos))).ok_or("5.4.6.3 IEER no compatible receiver")?;
    let buoy=w.entity(nearest.id).unwrap(); let range=buoy.pos.range_to(target.pos);
    let parent=w.rules.detection.as_ref().unwrap(); let env=parent.config.environment.as_ref().ok_or("5.4.6.3 IEER environment absent")?;
    let profile=det::sensed_profile(w,&parent.config,&target,true)?;
    if profile.size>6 {return Err("5.4.6.3 IEER target size unsupported".into());}
    let key=format!("{}:{}",nearest.class,nearest.sensor);
    if !matches!(parent.config.propagation.get(&key),None | Some(det::Propagation::Direct)) || crate::rules::nuclear2026::blueout_active(w,nearest.id,&nearest.sensor) {
        return Err("5.4.6.3 IEER non-direct propagation or nuclear blueout source pending".into());
    }
    let base=match &parent.config.bindings[&key] {Receiver::Sonar{active_range_nm:Some(x),..}=>*x,_=>return Err("5.4.6.3 active range missing".into())};
    let relative=det::sonar_speed_modifier(true,target.speed_kts,0.,false)?.ok_or("5.4.6.3 speed cell unavailable")?;
    let aspect=det::active_aspect_modifier(crate::geom::angle_diff(target.course_deg,target.pos.bearing_to(buoy.pos)));
    let target_mod=det::i(&det::tables()["sonar"]["active_strength"][profile.size])-i32::from(profile.anechoic)+aspect;
    let environment=det::sonar_environment(true,Frequency::Lmf,env)?;
    let layer=det::layer_modifier(Frequency::Lmf,false,nearest.depth_m,target.depth_m,env.layer)?;
    let modified=base*det::sonar_range_modifier(target_mod+relative+environment+layer)?;
    let cell=det::sonar_range_cell(modified,range)?;
    let bonus=tables()["echo_ranging"]["additional_probability_percent"].as_f64().unwrap();
    let cap=tables()["echo_ranging"]["maximum_probability_percent"].as_f64().unwrap();
    let probability=cell.map_or(0.,|p|(p+bonus).min(cap));
    let buoy_pos=buoy.pos; let sensor=nearest.sensor.clone();
    let mut rng=w.rng.derive(crate::Rng::hash_tag(w.tick,id.0,&format!("5.4.6.3.ieer:{}:{}",initial.0,target_id.0)));
    let d100=1+(rng.next_f64()*100.) as u8; let detected=cell.is_some() && f64::from(d100)<=probability; let good=detected && f64::from(d100)<probability/2.;
    let report=SensorReport{observer:nearest.id,target:target_id,sensor,at:w.tick,observer_position:buoy_pos,bearing_deg:buoy_pos.bearing_to(target.pos),measured_position:Some(target.pos),bearing_error_deg:None,sensor_kind:SensorKind::SonarActive,receiver:None,bearing_alternatives:vec![]};
    let s=&mut w.rules.detection.as_mut().unwrap().sonobuoys;
    if detected {s.echo.as_mut().unwrap().fixes.insert(receiver_key(id,target_id),EchoFix{receiver:id,target:target_id,buoy:nearest.id,report,at:w.tick,good,probability,d100});}
    event(w,id,None,"5.4.6.3.ieer_pulse",json!({"receiver":id,"buoy":nearest.id,"target":target_id,"d100":d100,"probability":probability,"detected":detected,"good":good,"modified50_nm":modified,"range_nm":range,"range_qualified":cell.is_some(),"source":"English2026 PDF54"}));
    Ok(())
}
pub(super) fn integrate(w:&mut World,s:&mut super::State) {
    let Some(echo)=s.echo.as_ref() else {return};
    let fixes:Vec<_>=echo.fixes.values().filter(|f|epoch(f.at)==epoch(w.tick)).cloned().collect();
    for f in fixes {
        let Some(m)=s.monitors.get(&f.receiver) else {continue};
        if !m.selected.contains(&f.buoy) || !s.floats.get(&f.buoy).is_some_and(|b|radio(w,s,f.receiver,m,b)) {continue}
        let side=m.side; let class=m.class.clone(); let quality=if f.good {Quality::Good}else{Quality::Fair};
        let cid=publish_contact(w,side,&f.report,f.report.measured_position);
        s.fixes.insert(receiver_key(f.receiver,f.target),Fix{monitor:f.receiver,monitor_class:class,side,target:f.target,report:f.report.clone(),buoys:vec![f.buoy],formed_at:w.tick,tactical_turn:epoch(w.tick),contact:cid,quality,aim:f.report.measured_position});
        if f.at==w.tick {event(w,f.receiver,Some(side),"5.4.6.3.ieer_received",json!({"buoy":f.buoy,"quality":quality,"d100":f.d100,"probability":f.probability}));}
    }
    s.echo.as_mut().unwrap().fixes.retain(|_,f|epoch(f.at)==epoch(w.tick));
}

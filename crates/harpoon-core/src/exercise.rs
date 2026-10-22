//! Fictional island exercise objectives and command lock. These are authored
//! game conditions, not Harpoon detection, damage or real-world readiness rules.
use crate::{Side,World,EntityId,ContactId,Vec2,EventKind,Visibility};
use crate::types::PlatformKind;
use serde::{Serialize,Deserialize};
use std::collections::BTreeMap;

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Objective {
    pub name:String,
    /// "carrier", "infrastructure" or "counter"; scenario accounting only.
    pub group:String,
    pub points:f64,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct IslandExercise {
    pub locked_side:Side,
    pub briefing_side:Side,
    pub briefing_targets:Vec<String>,
    pub protected_targets:Vec<Objective>,
    pub counter_targets:Vec<Objective>,
    pub required_carrier_points:f64,
    pub required_infrastructure_points:f64,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub naval_groups:Vec<NavalGroup>,
    #[serde(default,skip_serializing_if="zero_timer")]
    pub defender_air_preparation_s:i64,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub opening_flights:Vec<OpeningFlight>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub opening_salvos:Vec<OpeningSalvo>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub joint_groups:Vec<JointGroup>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub state:Option<ExerciseState>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct ExerciseState {
    pub targets:BTreeMap<String,(EntityId,f64)>,
    pub alerted_at:Option<i64>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub naval_groups:Vec<NavalGroupView>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub opening_flights:Vec<OpeningFlightView>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub joint_groups:Vec<JointGroupView>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct JointGroup {pub side:Side,pub name:String,pub role:String,pub members:Vec<String>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct JointGroupView {pub side:Side,pub name:String,pub role:String,pub members:Vec<EntityId>}
/// Authored pre-game flight snapshot, not a normal turn's deck launch command.
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct OpeningFlight {pub home:String,pub class:String,pub count:u8,pub target:String,pub progress:f64,pub altitude_m:f64,pub escort:bool}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct OpeningSalvo {pub launcher:String,pub weapon:String,pub count:u8,pub target:String,pub progress:f64}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct OpeningFlightView {pub unit:EntityId,pub home:EntityId,pub target:ContactId}
fn zero_timer(n:&i64)->bool {*n==0}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct NavalGroup {pub side:Side,pub name:String,pub leader:String,pub members:Vec<String>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct NavalGroupView {pub side:Side,pub name:String,pub leader:EntityId,pub members:Vec<EntityId>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct ObjectiveView {
    pub name:String,pub group:String,pub points:f64,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub contact_id:Option<ContactId>,
    /// Own truth or own observed BDA only; None means unknown, not undamaged.
    pub hp_fraction:Option<f64>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct ExerciseView {
    pub locked:bool,
    pub alerted_at:Option<i64>,
    pub objectives:Vec<ObjectiveView>,
    pub required_carrier_points:f64,
    pub required_infrastructure_points:f64,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub naval_groups:Vec<NavalGroupView>,
    #[serde(default,skip_serializing_if="zero_timer")]
    pub defender_air_preparation_s:i64,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub opening_flights:Vec<OpeningFlightView>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub joint_groups:Vec<JointGroupView>,
}

pub fn validate(sc:&crate::scenario::Scenario, db:&crate::Database)->Result<(),String> {
    let Some(c)=&sc.context.island_exercise else{return Ok(())};
    if sc.context.area_denial.is_some() || c.locked_side==c.briefing_side || c.state.is_some() {
        return Err("exercise needs opposite sides, no preset runtime state or competing area objective".into());
    }
    if c.defender_air_preparation_s<0 || c.defender_air_preparation_s>=sc.duration_s {return Err("invalid authored air preparation interval".into());}
    let mut grouped=std::collections::BTreeSet::new();let mut group_names=std::collections::BTreeSet::new();
    let mut joint_members=std::collections::BTreeSet::new();let mut joint_names=std::collections::BTreeSet::new();
    for g in &c.joint_groups {
        if g.name.is_empty()||!joint_names.insert(&g.name)||g.members.is_empty()||!matches!(g.role.as_str(),"shore_missile"|"shore_aviation"|"naval"|"patrol"|"infrastructure") {return Err("invalid exercise service group".into());}
        for n in &g.members {
            let matches:Vec<_>=sc.units.iter().filter(|u|u.name.as_ref()==Some(n)&&u.side==g.side).collect();
            if matches.len()!=1||!joint_members.insert(n) {return Err("ambiguous or duplicated exercise service-group membership".into());}
        }
    }
    for g in &c.naval_groups {
        if g.name.is_empty() || !group_names.insert(&g.name) || !g.members.contains(&g.leader) {return Err("invalid naval group leader/name".into());}
        for n in &g.members {
            let matches:Vec<_>=sc.units.iter().filter(|u|u.name.as_ref()==Some(n)).collect();
            if !grouped.insert(n) || matches.len()!=1 || matches[0].side!=g.side
                || !db.platforms.get(&matches[0].class).is_some_and(|p|p.kind==PlatformKind::Ship) {return Err("invalid or duplicated naval group member".into());}
        }
    }
    let mut names=std::collections::BTreeSet::new();
    for (items,side,counter) in [(&c.protected_targets,c.locked_side,false),(&c.counter_targets,c.briefing_side,true)] {
        for o in items {
            if !names.insert(&o.name) || !o.points.is_finite() || o.points<=0.0
                || !matches!(o.group.as_str(),"carrier"|"infrastructure"|"counter") {return Err("invalid exercise objective".into());}
            let matches:Vec<_>=sc.units.iter().filter(|u|u.name.as_ref()==Some(&o.name)).collect();
            if matches.len()!=1 || matches[0].side!=side {return Err(format!("invalid objective unit {}",o.name));}
            let class=db.platforms.get(&matches[0].class).ok_or("unknown objective class")?;
            if !matches!(class.kind,PlatformKind::Ship|PlatformKind::Base) {return Err("objective must be a surface ship or installation".into());}
            if (o.group=="carrier" && (class.kind!=PlatformKind::Ship || class.deck.is_none()))
                || (o.group=="infrastructure" && class.kind!=PlatformKind::Base)
                || (counter && o.group!="counter") || (!counter && o.group=="counter") {return Err("invalid objective group".into());}
        }
    }
    let carrier:f64=c.protected_targets.iter().filter(|o|o.group=="carrier").map(|o|o.points).sum();
    let infrastructure:f64=c.protected_targets.iter().filter(|o|o.group=="infrastructure").map(|o|o.points).sum();
    if !c.required_carrier_points.is_finite() || !c.required_infrastructure_points.is_finite()
        || c.required_carrier_points<=0.0 || c.required_carrier_points>carrier
        || c.required_infrastructure_points<=0.0 || c.required_infrastructure_points>infrastructure {
        return Err("invalid exercise success thresholds".into());
    }
    let mut briefed=std::collections::BTreeSet::new();
    for n in &c.briefing_targets {
        let matches:Vec<_>=sc.units.iter().filter(|u|u.name.as_ref()==Some(n)).collect();
        if !briefed.insert(n) || matches.len()!=1 || matches[0].side!=c.locked_side
            || !db.platforms.get(&matches[0].class).is_some_and(|p|matches!(p.kind,PlatformKind::Ship|PlatformKind::Base)) {
            return Err(format!("invalid briefing target {n}"));
        }
    }
    // All configured targets must already be part of this side's initial brief.
    // Inventory, elapsed fuel and envelope checks occur during local loading;
    // an error discards that new World before a running session can adopt it.
    for (source,target,progress,count) in c.opening_flights.iter().map(|f|(&f.home,&f.target,f.progress,f.count))
        .chain(c.opening_salvos.iter().map(|s|(&s.launcher,&s.target,s.progress,s.count))) {
        if count==0 || !progress.is_finite() || !(0.0..0.95).contains(&progress)
            || !briefed.contains(target) || !sc.units.iter().any(|u|u.name.as_ref()==Some(source)&&u.side==c.briefing_side) {
            return Err("invalid opening snapshot source, target, count or progress".into());
        }
    }
    Ok(())
}

pub fn initialize(world:&mut World)->Result<(),String> {
    let Some(config)=world.context.island_exercise.clone() else{return Ok(())};
    let targets=config.protected_targets.iter().chain(&config.counter_targets).map(|o| {
        let u=world.entities.values().find(|u|u.name==o.name).expect("validated objective");
        (o.name.clone(),(u.id,u.hp_max))
    }).collect();
    for name in &config.briefing_targets {
        let u=world.entities.values().find(|u|u.name==*name).expect("validated briefing");
        let (id,kind,pos,class)=(u.id,u.kind,u.pos,u.class_def(&world.db).map(|d|d.name.clone()));
        let side=&mut world.sides[config.briefing_side.index()];side.next_contact+=1;
        let cid=ContactId(side.next_contact);
        side.contacts.insert(cid,crate::world::Contact {id:cid,about:id,category:kind,class_name:class,
            estimate:pos,est_course_deg:0.0,est_speed_kts:0.0,bearing_only:false,quality:0.85,
            last_update_tick:world.tick,first_tick:world.tick,stale:false,sensors:vec!["exercise_briefing".into()],
            offset:Vec2::zero(),announced:true});
    }
    let naval_groups=config.naval_groups.iter().map(|g|NavalGroupView {side:g.side,name:g.name.clone(),
        leader:world.entities.values().find(|u|u.name==g.leader).unwrap().id,
        members:g.members.iter().map(|n|world.entities.values().find(|u|u.name==*n).unwrap().id).collect()}).collect();
    let joint_groups=config.joint_groups.iter().map(|g|JointGroupView {side:g.side,name:g.name.clone(),role:g.role.clone(),members:g.members.iter().map(|n|world.entities.values().find(|u|u.name==*n).unwrap().id).collect()}).collect();
    world.context.island_exercise.as_mut().unwrap().state=Some(ExerciseState {targets,alerted_at:None,naval_groups,opening_flights:Vec::new(),joint_groups});
    initialize_opening(world,&config)?;
    if config.defender_air_preparation_s>0 {
        let cap_classes:std::collections::BTreeSet<_>=world.db.platforms.iter().filter(|(_,p)|p.weapons.iter().any(|m|
            world.db.weapons.get(&m.weapon).is_some_and(|w|w.kind==crate::db::WeaponKind::Aam))).map(|(id,_)|id.clone()).collect();
        let until=world.tick+config.defender_air_preparation_s;
        for u in world.entities.values_mut().filter(|u|u.side==config.locked_side) {
            if let Some(deck)=&mut u.deck {
                let mut immediate=2u8;
                for (class,n) in &mut deck.ready {
                    let keep=if cap_classes.contains(class) {(*n).min(immediate)}else{0};
                    immediate-=keep;let preparing=*n-keep;*n=keep;
                    if preparing>0 {deck.cycling.push((class.clone(),preparing,until));}
                }
            }
        }
    }
    Ok(())
}

fn initialize_opening(world:&mut World,c:&IslandExercise)->Result<(),String> {
    let locate=|w:&World,name:&str|w.entities.values().find(|u|u.name==name).map(|u|u.id).ok_or_else(||format!("unknown opening unit {name}"));
    for flight in &c.opening_flights {
        let home=locate(world,&flight.home)?;let target=locate(world,&flight.target)?;
        let start=world.entity(home).unwrap().pos;let aim=world.entity(target).unwrap().pos;
        let contact=world.sides[c.briefing_side.index()].contacts.values().find(|t|t.about==target).ok_or("opening target not briefed")?.id;
        let class=world.db.platforms.get(&flight.class).ok_or("unknown opening aircraft")?;
        let speed=if class.cruise_speed_kts>0.0 {class.cruise_speed_kts}else{class.max_speed_kts*0.7};
        if speed<=0.0 {return Err("invalid opening aircraft speed".into());}
        let elapsed=start.range_to(aim)*flight.progress/speed*3600.0;
        if elapsed>=class.endurance_s*0.5 {return Err("opening flight exceeds authored fuel reserve".into());}
        let point=start.advanced(start.bearing_to(aim),start.range_to(aim)*flight.progress);
        let mission=if flight.escort {crate::commands::AirMission::Cap {station:point,radius_nm:12.0,follow:false}}
            else{crate::commands::AirMission::Strike {contact}};
        let profile=crate::commands::LaunchProfile {waypoints:Vec::new(),altitude_m:flight.altitude_m,weapons_free:flight.escort,manual_strike:!flight.escort,
            engagement:Some(crate::commands::LaunchEngagement {air_only:flight.escort,max_in_flight:2})};
        let first=world.next_entity;
        crate::air::launch_planned_aircraft(world,home,&flight.class,flight.count,&mission,Some(&profile))?;
        let ids:Vec<_>=world.entities.keys().filter(|id|id.0>first).copied().collect();
        for (i,id) in ids.iter().enumerate() {
            let u=world.entity_mut(*id).unwrap();
            u.pos=point.advanced(start.bearing_to(aim)+90.0,(i as f64-(flight.count as f64-1.0)/2.0)*0.25);
            u.endurance_s=u.endurance_s.map(|s|s-elapsed);u.course_deg=start.bearing_to(aim);u.ordered_course_deg=u.course_deg;
            u.speed_kts=speed;u.ordered_speed_kts=speed;
            if !flight.escort {u.waypoints.push_back(aim);u.assigned=Some(contact);}
        }
        if !flight.escort {world.context.island_exercise.as_mut().unwrap().state.as_mut().unwrap().opening_flights
            .extend(ids.into_iter().map(|unit|OpeningFlightView {unit,home,target:contact}));}
    }
    for salvo in &c.opening_salvos {
        let launcher=locate(world,&salvo.launcher)?;let target=locate(world,&salvo.target)?;
        let shooter=world.entity(launcher).unwrap();let victim=world.entity(target).unwrap();
        let def=world.db.weapons.get(&salvo.weapon).ok_or("unknown opening weapon")?.clone();
        let distance=shooter.pos.range_to(victim.pos);let aim=victim.pos;let start=shooter.pos;
        if !matches!(def.kind,crate::db::WeaponKind::Ascm|crate::db::WeaponKind::Slcm) || !def.targets.accepts(victim.kind)
            || distance<def.min_range_nm || distance>def.range_nm || def.speed_kts<=0.0 {
            return Err(format!("opening weapon envelope/type invalid: {}",salvo.launcher));
        }
        let index=shooter.weapons.iter().position(|m|m.weapon==salvo.weapon&&m.ammo>=salvo.count as u16).ok_or("opening salvo exceeds magazine")?;
        let elapsed=distance*salvo.progress/def.speed_kts*3600.0;
        for _ in 0..salvo.count {
            let id=crate::weapons::launch(world,launcher,&def,Some(target),Some(aim)).ok_or("opening launch failed")?;
            let u=world.entity_mut(id).unwrap();u.pos=start.advanced(start.bearing_to(aim),distance*salvo.progress);
            u.projectile.as_mut().unwrap().live_s=elapsed;
        }
        world.entity_mut(launcher).unwrap().weapons[index].ammo-=salvo.count as u16;
    }
    if !c.opening_flights.is_empty() || !c.opening_salvos.is_empty() {
        world.emit(EventKind::RulesEvent {domain:"exercise".into(),rule:"opening_snapshot".into(),side:Some(c.briefing_side),unit:None,contact:None,
            detail:"Authored pre-game sorties and releases: deck/magazine inventories deducted; elapsed flight fuel and missile time charged".into()},Visibility::Side(c.briefing_side));
    }
    Ok(())
}

pub fn locked(world:&World, side:Side)->bool {
    world.context.island_exercise.as_ref().is_some_and(|c|c.locked_side==side
        && c.state.as_ref().is_none_or(|s|s.alerted_at.is_none()))
}

/// A routine aircraft/sensor keeps running. The first observed hostile missile
/// or an actual hostile hit opens active command authority on the next tick.
pub fn update_alert(world:&mut World) {
    let Some(c)=&world.context.island_exercise else{return};
    let side=c.locked_side;if !locked(world,side){return;}
    let detected=world.sides[side.index()].contacts.values().any(|t|t.category==PlatformKind::Missile && !t.stale && t.quality>0.0);
    let hit=world.events.iter().rev().take_while(|e|e.tick==world.tick).any(|e|match &e.kind {
        EventKind::Impact {unit,damage,..}=>*damage>0.0 && world.entity(*unit).is_some_and(|u|u.side==side),
        EventKind::AirShotDown {side:s,by_side,..}=>*s==side && *by_side!=side,
        _=>false,
    });
    if detected || hit {
        world.context.island_exercise.as_mut().unwrap().state.as_mut().unwrap().alerted_at=Some(world.tick);
        world.emit(EventKind::RulesEvent {domain:"exercise".into(),rule:"alert".into(),side:Some(side),unit:None,contact:None,
            detail:if detected {"Routine sensors observed a hostile missile; active commands unlocked"}else{"Hostile impact confirmed; active commands unlocked"}.into()},Visibility::Side(side));
    }
}

fn fraction(world:&World,id:EntityId,max:f64)->f64 {
    world.entity(id).filter(|u|u.alive).map(|u|(u.hp/max).clamp(0.0,1.0)).unwrap_or(0.0)
}
pub fn scores(world:&World)->Option<(f64,f64,f64,f64)> {
    let c=world.context.island_exercise.as_ref()?;let s=c.state.as_ref()?;
    let value=|o:&Objective| {let(id,max)=s.targets[&o.name];o.points*(1.0-fraction(world,id,max))};
    let carrier:f64=c.protected_targets.iter().filter(|o|o.group=="carrier").map(value).sum();
    let infrastructure:f64=c.protected_targets.iter().filter(|o|o.group=="infrastructure").map(value).sum();
    let damage=carrier+infrastructure;
    let preserved=c.protected_targets.iter().map(|o|o.points).sum::<f64>()-damage;
    let retaliation=c.counter_targets.iter().map(value).sum::<f64>();
    let (attacker,defender)=(damage,preserved+retaliation);
    Some(if c.briefing_side==Side::Blue {(attacker,defender,carrier,infrastructure)}else{(defender,attacker,carrier,infrastructure)})
}
pub fn adjudicate(world:&World)->Option<(Option<Side>,String)> {
    let c=world.context.island_exercise.as_ref()?;
    if world.tick<world.duration_s {return None;}
    let (blue,red,carrier,infrastructure)=scores(world)?;
    let (attacker,defender)=if c.briefing_side==Side::Blue {(blue,red)}else{(red,blue)};
    let success=carrier>=c.required_carrier_points && infrastructure>=c.required_infrastructure_points && attacker>defender;
    Some((Some(if success {c.briefing_side}else{c.locked_side}),
        if success {"exercise_objectives_achieved"}else{"exercise_forces_preserved"}.into()))
}
pub fn observe(world:&World,side:Side)->Option<ExerciseView> {
    let c=world.context.island_exercise.as_ref()?;let s=c.state.as_ref()?;
    let objectives=c.protected_targets.iter().chain(&c.counter_targets).map(|o| {
        let (id,max)=s.targets[&o.name];
        let own=world.entity(id).is_some_and(|u|u.side==side)
            || if o.group=="counter" {side==c.briefing_side}else{side==c.locked_side};
        let hp=if own||world.over {Some(fraction(world,id,max))}
            else{world.sides[side.index()].bda.get(&id).and_then(|b|b.hp_fraction)};
        let contact_id=world.sides[side.index()].contacts.values()
            .find(|t|t.about==id && t.sensors.iter().any(|s|s=="exercise_briefing")).map(|t|t.id);
        ObjectiveView {name:o.name.clone(),group:o.group.clone(),points:o.points,hp_fraction:hp,contact_id}
    }).collect();
    Some(ExerciseView {locked:locked(world,side),alerted_at:if side==c.locked_side||world.over {s.alerted_at}else{None},
        objectives,required_carrier_points:c.required_carrier_points,required_infrastructure_points:c.required_infrastructure_points,
        naval_groups:s.naval_groups.iter().filter(|g|g.side==side).cloned().collect(),defender_air_preparation_s:c.defender_air_preparation_s,
        opening_flights:if side==c.briefing_side {s.opening_flights.iter().filter(|f|world.entity(f.unit).is_some_and(|u|u.alive)).cloned().collect()}else{Vec::new()},
        joint_groups:s.joint_groups.iter().filter(|g|g.side==side).cloned().collect()})
}

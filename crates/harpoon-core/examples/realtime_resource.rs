//! Finite public resource/precedence diagnostic. Mission rules are experiment
//! assumptions; navigation uses the frozen production core without alterations.
use harpoon_core::{load_scenario, sim, Command, Database, EntityId, Scenario, Side, Vec2, World};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeSet, fs};

#[derive(Clone, Serialize, Deserialize)]
struct Vessel { id: u32, pos: Vec2, home: Vec2, capacity: u32, skills: Vec<String> }
#[derive(Clone, Serialize, Deserialize)]
struct Task {
    id: usize, goal: Vec2, deadline: i64, reward: u32, service_s: i64,
    load: u32, kits: u32, skill: String, predecessor: Option<usize>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Case { vessels: Vec<Vessel>, tasks: Vec<Task>, horizon: i64, shared_kits: u32 }
#[derive(Clone, Serialize, Deserialize)]
struct Route { unit: u32, task_ids: Vec<usize> }
#[derive(Clone, Serialize, Deserialize)]
struct Plan { routes: Vec<Route>, brief: String }
#[derive(Clone, Serialize, Deserialize)]
struct Visit { task: usize, arrived: i64, completed: i64, on_time: bool, pos: Vec2 }
#[derive(Clone, Serialize, Deserialize)]
struct Evaluation {
    unit: u32, task_ids: Vec<usize>, mask: u32, score: u32, feasible: bool,
    reason: String, visits: Vec<Visit>, return_tick: i64, world_hash: String,
}
fn fingerprint(w: &World) -> String {
    let mut h=0xcbf29ce484222325u64;
    for b in serde_json::to_vec(w).unwrap() {h=(h^b as u64).wrapping_mul(0x100000001b3);}
    format!("{h:016x}")
}
fn world(c:&Case,v:&Vessel) -> World {
    let s:Scenario=serde_json::from_value(json!({"id":"resource_diag_v1","name":"Fictional service exercise",
        "duration_s":c.horizon,"seed":0,"weather":{"visibility_nm":12,"jam":[0,0]},"victory":{"target_vp":0},
        "units":[{"side":"Blue","class":"ddg_aegis","name":"Service vessel","pos":v.pos,
            "course_deg":90,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"ddg_aegis","name":"Distant control","pos":{"x":5000,"y":5000},"speed_kts":0,"fire_on":false}]})).unwrap();
    load_scenario(s,Database::built_in()).unwrap()
}
fn step(w:&mut World,cmds:&[Command]) {
    sim::tick(w,&cmds.iter().cloned().map(|c|(Side::Blue,c)).collect::<Vec<_>>());
}
fn travel(w:&mut World,goal:Vec2,horizon:i64) -> bool {
    if w.entity(EntityId(1)).unwrap().pos.range_to(goal)<=0.12 {return true;}
    if w.tick>=horizon {return false;}
    step(w,&[Command::Goto{unit:EntityId(1),waypoints:vec![goal]},Command::SetSpeed{unit:EntityId(1),speed_kts:18.0}]);
    while w.tick<horizon && w.entity(EntityId(1)).unwrap().pos.range_to(goal)>0.12 {step(w,&[]);}
    w.entity(EntityId(1)).unwrap().pos.range_to(goal)<=0.12
}
fn stop(w:&mut World,seconds:i64,horizon:i64) {
    // Clear waypoints as well as speed. Service and end-of-route holding are
    // shared task-executor semantics, not a new Harpoon adjudication rule.
    for i in 0..seconds {
        if w.tick>=horizon {break;}
        if i==0 {step(w,&[Command::SetSpeed{unit:EntityId(1),speed_kts:0.0},Command::Goto{unit:EntityId(1),waypoints:vec![]}]);}
        else {step(w,&[]);}
    }
}
fn evaluate(c:&Case,v:&Vessel,ids:&[usize],delay:i64) -> Evaluation {
    let mut w=world(c,v);
    for _ in 0..delay.min(c.horizon) {step(&mut w,&[]);}
    let mut result=Evaluation{unit:v.id,task_ids:ids.to_vec(),mask:0,score:0,feasible:true,
        reason:"ok".into(),visits:vec![],return_tick:0,world_hash:String::new()};
    let mut load=0;
    for &id in ids {
        let t=&c.tasks[id];
        let reason=if !v.skills.contains(&t.skill) {Some("capability")}
            else if t.predecessor.map(|p|result.mask&(1<<p)==0).unwrap_or(false) {Some("same_vessel_predecessor")}
            else if load+t.load>v.capacity {Some("vessel_capacity")} else {None};
        if let Some(r)=reason {result.feasible=false;result.reason=r.into();break;}
        load+=t.load;
        if !travel(&mut w,t.goal,c.horizon) {result.feasible=false;result.reason="travel_horizon".into();break;}
        let arrived=w.tick;
        if w.tick+t.service_s>c.horizon {result.feasible=false;result.reason="service_horizon".into();break;}
        stop(&mut w,t.service_s,c.horizon);
        result.mask|=1<<id;
        let on_time=w.tick<=t.deadline;
        if on_time {result.score+=t.reward;}
        result.visits.push(Visit{task:id,arrived,completed:w.tick,on_time,pos:w.entity(EntityId(1)).unwrap().pos});
    }
    if result.feasible && !travel(&mut w,v.home,c.horizon) {result.feasible=false;result.reason="return_horizon".into();}
    result.return_tick=w.tick;
    let remaining=(c.horizon-w.tick).max(0);
    stop(&mut w,remaining,c.horizon);
    if !result.feasible {result.score=0;}
    result.world_hash=fingerprint(&w);
    result
}
fn validate_case(c:&Case) {
    assert!(c.tasks.len()<=12 && c.vessels.len()==3 && c.horizon>0);
    for (i,t) in c.tasks.iter().enumerate() {
        assert!(t.id==i && t.service_s>0 && t.load>0 && t.deadline>0);
        assert!(t.predecessor.map(|p|p<i).unwrap_or(true));
    }
}
fn enumerate(c:&Case,v:&Vessel,ids:&mut Vec<usize>,out:&mut Vec<Evaluation>) {
    let r=evaluate(c,v,ids,0);
    // Feasibility of returning now is NOT a safe pruning condition: a later
    // task could lie along the return leg. Only structural constraints prune.
    out.push(r);
    if ids.len()==4 {return;}
    let load:u32=ids.iter().map(|i|c.tasks[*i].load).sum();
    let kits:u32=ids.iter().map(|i|c.tasks[*i].kits).sum();
    for t in &c.tasks {
        if ids.contains(&t.id) || !v.skills.contains(&t.skill) || load+t.load>v.capacity
            || kits+t.kits>c.shared_kits || t.predecessor.map(|p|!ids.contains(&p)).unwrap_or(false) {continue;}
        ids.push(t.id);enumerate(c,v,ids,out);ids.pop();
    }
}
fn main() -> Result<(),String> {
    let args:Vec<String>=std::env::args().collect();
    let c:Case=serde_json::from_str(&fs::read_to_string(args.get(2).ok_or("case path")?).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    validate_case(&c);
    if args[1]=="--catalog" {
        let mut catalog=vec![];
        for v in &c.vessels {enumerate(&c,v,&mut vec![],&mut catalog);}
        println!("{}",json!({"catalog":catalog,"semantics":"finite real-navigation routes, max4 visits"}));
    } else if args[1]=="--evaluate" {
        let p:Plan=serde_json::from_str(&fs::read_to_string(args.get(3).ok_or("plan path")?).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        let delay=args.get(4).map(|s|s.parse::<i64>().unwrap()).unwrap_or(0);
        assert!((0..=c.horizon).contains(&delay));
        assert!(p.routes.len()==3);
        let mut units=BTreeSet::new();let mut tasks=BTreeSet::new();let mut kits=0;
        let mut results=vec![];
        for r in &p.routes {
            assert!(units.insert(r.unit) && r.task_ids.len()<=4);
            for id in &r.task_ids {assert!(*id<c.tasks.len() && tasks.insert(*id));kits+=c.tasks[*id].kits;}
            let v=c.vessels.iter().find(|v|v.id==r.unit).ok_or("unknown unit")?;
            results.push(evaluate(&c,v,&r.task_ids,delay));
        }
        let earned=if kits>c.shared_kits {0} else {results.iter().map(|r|r.score).sum::<u32>()};
        println!("{}",json!({"earned":earned,"kits":kits,"shared_budget_ok":kits<=c.shared_kits,"routes":results,"delay_s":delay}));
    } else {return Err("unknown mode".into());}
    Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    fn sample()->Case {Case{vessels:(1..=3).map(|id|Vessel{id,pos:Vec2::new(0.,0.),home:Vec2::new(0.,0.),capacity:2,skills:vec!["survey".into()]}).collect(),
        tasks:vec![Task{id:0,goal:Vec2::new(0.,0.),deadline:20,reward:3,service_s:10,load:1,kits:1,skill:"survey".into(),predecessor:None},
        Task{id:1,goal:Vec2::new(0.,0.),deadline:40,reward:5,service_s:10,load:1,kits:1,skill:"survey".into(),predecessor:Some(0)}],horizon:100,shared_kits:2}}
    #[test] fn service_deadline_inclusive(){let mut c=sample();c.tasks[0].deadline=10;assert_eq!(evaluate(&c,&c.vessels[0],&[0],0).score,3);assert_eq!(evaluate(&c,&c.vessels[0],&[0],1).score,0);}
    #[test] fn predecessor_is_same_vessel_and_ordered(){let c=sample();assert_eq!(evaluate(&c,&c.vessels[0],&[1],0).reason,"same_vessel_predecessor");assert_eq!(evaluate(&c,&c.vessels[0],&[0,1],0).score,8);}
    #[test] fn capacity_and_capability_are_actual_failures(){let mut c=sample();c.vessels[0].capacity=1;assert_eq!(evaluate(&c,&c.vessels[0],&[0,1],0).score,0);c.tasks[0].skill="other".into();assert_eq!(evaluate(&c,&c.vessels[0],&[0],0).reason,"capability");}
    #[test] fn return_is_required(){let mut c=sample();c.vessels[0].home=Vec2::new(10.,0.);assert_eq!(evaluate(&c,&c.vessels[0],&[0],0).reason,"return_horizon");}
    #[test] fn replay_is_deterministic(){let c=sample();let a=evaluate(&c,&c.vessels[0],&[0,1],0);let b=evaluate(&c,&c.vessels[0],&[0,1],0);assert_eq!(serde_json::to_value(a).unwrap(),serde_json::to_value(b).unwrap());}
}

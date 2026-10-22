//! Exact navigation segments over the frozen production World.
//! The experiment task layer lives outside this service; no new physics.
use harpoon_core::{load_scenario, sim, Command, Database, EntityId, Scenario, Side, Vec2, World};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

#[derive(Clone)]
struct Segment { start: World, commands: Vec<Command> }
struct Service { segments: Vec<Segment> }
fn tick(w: &mut World, commands: &[Command]) {
    sim::tick(w, &commands.iter().cloned().map(|c|(Side::Blue,c)).collect::<Vec<_>>());
}
fn hash(w: &World) -> String {
    let mut h=0xcbf29ce484222325u64;
    for b in serde_json::to_vec(w).unwrap() {h=(h^b as u64).wrapping_mul(0x100000001b3);}
    format!("{h:016x}")
}
fn initial(pos: Vec2, course: f64) -> World {
    let sc: Scenario = serde_json::from_value(json!({
        "id":"research_navigation_service_v1", "name":"Fictional navigation laboratory",
        "duration_s":1000000,"seed":0,"weather":{"visibility_nm":12,"jam":[0,0]},
        "victory":{"target_vp":0},"units":[
            {"side":"Blue","class":"ddg_aegis","name":"Work vessel","pos":pos,
             "course_deg":course,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"ddg_aegis","name":"Distant control",
             "pos":{"x":5000,"y":5000},"speed_kts":0,"fire_on":false}]
    })).unwrap();
    load_scenario(sc,Database::built_in()).unwrap()
}
impl Service {
    fn frame(&self, w: &World, segment: usize, offset: i64) -> Value {
        let e=w.entity(EntityId(1)).unwrap();
        json!({"ref":segment,"offset":offset,"tick":w.tick,"pos":e.pos,
            "course_deg":e.course_deg,"speed_kts":e.speed_kts,
            "ordered_course_deg":e.ordered_course_deg,"ordered_speed_kts":e.ordered_speed_kts,
            "waypoints":e.waypoints})
    }
    fn restore(&self, state: &Value) -> Result<World,String> {
        let id=state["ref"].as_u64().ok_or("ref")? as usize;
        let offset=state["offset"].as_i64().ok_or("offset")?;
        if !(0..=10000).contains(&offset) {return Err("invalid offset".into());}
        let seg=self.segments.get(id).ok_or("unknown segment")?;
        let mut w=seg.start.clone();
        for i in 0..offset {tick(&mut w,if i==0 {&seg.commands} else {&[]});}
        if self.frame(&w,id,offset)!=*state {return Err("state token changed or from another session".into());}
        Ok(w)
    }
    fn add(&mut self,w:World,commands:Vec<Command>)->usize {
        let id=self.segments.len();self.segments.push(Segment{start:w,commands});id
    }
    fn request(&mut self,q:&Value)->Result<Value,String> {
        match q["op"].as_str().ok_or("op")? {
            "initial" => {
                let pos:Vec2=serde_json::from_value(q["pos"].clone()).map_err(|e|e.to_string())?;
                let w=initial(pos,q["course"].as_f64().unwrap_or(90.));
                let id=self.add(w.clone(),vec![]);Ok(self.frame(&w,id,0))
            },
            "load" => {
                let w:World=serde_json::from_value(q["world"].clone()).map_err(|e|e.to_string())?;
                let id=self.add(w.clone(),vec![]);Ok(self.frame(&w,id,0))
            },
            "dump" => Ok(serde_json::to_value(self.restore(&q["state"])?).unwrap()),
            "fingerprint" => Ok(json!(hash(&self.restore(&q["state"])?))),
            "segment" => {
                let mut w=self.restore(&q["state"])?;
                let n=q["seconds"].as_i64().ok_or("seconds")?;
                if !(0..=10000).contains(&n) {return Err("seconds outside 0..10000".into());}
                let goal:Option<Vec2>=if q["goal"].is_null(){None}else{Some(serde_json::from_value(q["goal"].clone()).map_err(|e|e.to_string())?)};
                let stop=q["stop"].as_bool().unwrap_or(false);
                if stop && goal.is_some(){return Err("stop and goal are exclusive".into());}
                let until=q["until_arrival"].as_bool().unwrap_or(true);
                if n==0 || (until && goal.is_some_and(|g|w.entity(EntityId(1)).unwrap().pos.range_to(g)<=0.12)) {
                    return Ok(json!([q["state"].clone()]));
                }
                let commands=if let Some(g)=goal {
                    vec![Command::Goto{unit:EntityId(1),waypoints:vec![g]},Command::SetSpeed{unit:EntityId(1),speed_kts:18.}]
                } else if stop {vec![Command::Goto{unit:EntityId(1),waypoints:vec![]},Command::SetSpeed{unit:EntityId(1),speed_kts:0.}]}
                else {vec![]};
                let id=self.add(w.clone(),commands.clone());
                let mut frames=vec![self.frame(&w,id,0)];
                for i in 0..n {
                    tick(&mut w,if i==0 {&commands}else{&[]});
                    frames.push(self.frame(&w,id,i+1));
                    if until && goal.is_some_and(|g|w.entity(EntityId(1)).unwrap().pos.range_to(g)<=0.12){break;}
                }
                Ok(json!(frames))
            },
            _=>Err("unknown op".into())
        }
    }
}
fn main() {
    let mut service=Service{segments:vec![]};
    let mut out=io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let response=line.map_err(|e|e.to_string())
            .and_then(|s|serde_json::from_str::<Value>(&s).map_err(|e|e.to_string()))
            .and_then(|q|service.request(&q));
        let value=match response {Ok(v)=>json!({"ok":v}),Err(e)=>json!({"error":e})};
        writeln!(out,"{}",value).unwrap();out.flush().unwrap();
    }
}
#[cfg(test)] mod tests {
    use super::*;
    fn svc()->Service {Service{segments:vec![]}}
    #[test] fn split_continuation_preserves_complete_world() {
        let mut s=svc();let a=s.request(&json!({"op":"initial","pos":{"x":0.,"y":0.}})).unwrap();
        let all=s.request(&json!({"op":"segment","state":a,"goal":{"x":1.5,"y":0.6},"seconds":300,"until_arrival":false})).unwrap();
        let frames=all.as_array().unwrap();
        let tail=s.request(&json!({"op":"segment","state":frames[100],"seconds":200,"until_arrival":false})).unwrap();
        assert_eq!(hash(&s.restore(&frames[300]).unwrap()),hash(&s.restore(tail.as_array().unwrap().last().unwrap()).unwrap()));
    }
    #[test] fn serialization_restores_full_dynamic_state() {
        let mut s=svc();let a=s.request(&json!({"op":"initial","pos":{"x":0.,"y":0.}})).unwrap();
        let path=s.request(&json!({"op":"segment","state":a,"goal":{"x":1.,"y":-1.},"seconds":40})).unwrap();
        let mid=path.as_array().unwrap().last().unwrap();
        let w=s.request(&json!({"op":"dump","state":mid})).unwrap();
        let mut other=svc();let restored=other.request(&json!({"op":"load","world":w})).unwrap();
        let x=s.request(&json!({"op":"segment","state":mid,"seconds":50})).unwrap();
        let y=other.request(&json!({"op":"segment","state":restored,"seconds":50})).unwrap();
        assert_eq!(hash(&s.restore(x.as_array().unwrap().last().unwrap()).unwrap()),hash(&other.restore(y.as_array().unwrap().last().unwrap()).unwrap()));
    }
    #[test] fn mutating_token_is_rejected() {
        let mut s=svc();let mut a=s.request(&json!({"op":"initial","pos":{"x":0.,"y":0.}})).unwrap();
        a["pos"]["x"]=json!(5.);assert!(s.restore(&a).is_err());
    }
    #[test] fn stop_is_real_deceleration_and_clears_route() {
        let mut s=svc();let a=s.request(&json!({"op":"initial","pos":{"x":0.,"y":0.}})).unwrap();
        let p=s.request(&json!({"op":"segment","state":a,"goal":{"x":3.,"y":0.},"seconds":50})).unwrap();
        let at=p.as_array().unwrap().last().unwrap();
        let stop=s.request(&json!({"op":"segment","state":at,"stop":true,"seconds":1})).unwrap();
        let after=stop.as_array().unwrap().last().unwrap();
        assert!(after["speed_kts"].as_f64().unwrap()>0.);
        assert_eq!(after["waypoints"],json!([]));
        assert_ne!(at["pos"],after["pos"]);
    }
}

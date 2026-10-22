//! Public chart geometry in the same nautical-mile plane as movement. Coast
//! intersection is swept, not an endpoint test. This is a navigation guard;
//! it does not invent bathymetry, grounding damage or terrain elevation (§3.4).
use crate::geom::Vec2;
use serde::{Serialize,Deserialize};
use std::collections::{BTreeMap,BinaryHeap};

#[derive(Clone,Debug,Serialize,Deserialize,Default)]
pub struct Terrain {
    pub source:String, pub polygons:Vec<LandPolygon>,
    /// Rebuilt from frozen geometry; excluded from saves, hashes and RNG.
    #[serde(skip)]
    pub index:std::sync::Arc<std::sync::OnceLock<SpatialIndex>>,
}
#[derive(Clone,Debug,Default)]
pub struct SpatialIndex {cells:BTreeMap<(i32,i32),Vec<usize>>,large:Vec<usize>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct LandPolygon {pub outer:Vec<Vec2>,#[serde(default)]pub holes:Vec<Vec<Vec2>>,#[serde(default)]pub bounds:Option<[f64;4]>}

fn cross(a:Vec2,b:Vec2,c:Vec2)->f64 {(b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x)}
fn on(a:Vec2,b:Vec2,p:Vec2)->bool {cross(a,b,p).abs()<1e-9 && p.x>=a.x.min(b.x)-1e-9 && p.x<=a.x.max(b.x)+1e-9 && p.y>=a.y.min(b.y)-1e-9 && p.y<=a.y.max(b.y)+1e-9}
fn intersects(a:Vec2,b:Vec2,c:Vec2,d:Vec2)->bool {
    if a.x.max(b.x)<c.x.min(d.x) || a.x.min(b.x)>c.x.max(d.x) || a.y.max(b.y)<c.y.min(d.y) || a.y.min(b.y)>c.y.max(d.y) {return false;}
    let (x,y,z,w)=(cross(a,b,c),cross(a,b,d),cross(c,d,a),cross(c,d,b));
    (x*y<0.0 && z*w<0.0) || on(a,b,c)||on(a,b,d)||on(c,d,a)||on(c,d,b)
}
fn edges(r:&[Vec2])->impl Iterator<Item=(Vec2,Vec2)>+'_ {r.iter().copied().zip(r.iter().copied().cycle().skip(1)).take(r.len())}
fn in_ring(r:&[Vec2],p:Vec2)->bool {
    let mut inside=false;
    for (a,b) in edges(r) {
        if on(a,b,p) {return true;}
        if (a.y>p.y)!=(b.y>p.y) && p.x<(b.x-a.x)*(p.y-a.y)/(b.y-a.y)+a.x {inside=!inside;}
    }
    inside
}
impl Terrain {
    pub fn normalize(&mut self)->Result<(),String> {
        self.index=Default::default();
        for l in &mut self.polygons {
            if l.outer.len()<3 || l.outer.iter().chain(l.holes.iter().flatten()).any(|p|!p.x.is_finite()||!p.y.is_finite()) {return Err("invalid land polygon".into());}
            l.bounds=Some(l.outer.iter().fold([f64::INFINITY,f64::INFINITY,f64::NEG_INFINITY,f64::NEG_INFINITY],|v,p|[v[0].min(p.x),v[1].min(p.y),v[2].max(p.x),v[3].max(p.y)]));
        } Ok(())
    }
    fn candidates(&self,a:Vec2,b:Vec2)->Vec<usize> {
        const CELL:f64=32.0;
        let key=|x:f64|(x/CELL).floor() as i32;
        let (x0,x1,y0,y1)=(key(a.x.min(b.x)),key(a.x.max(b.x)),key(a.y.min(b.y)),key(a.y.max(b.y)));
        if (x1 as i64-x0 as i64+1) as u128*(y1 as i64-y0 as i64+1) as u128>8192 {return (0..self.polygons.len()).collect();}
        let index=self.index.get_or_init(|| {
            let mut index=SpatialIndex::default();
            for (id,l) in self.polygons.iter().enumerate() {
                let Some(bounds)=l.bounds else {index.large.push(id);continue};
                let (x0,x1,y0,y1)=(key(bounds[0]),key(bounds[2]),key(bounds[1]),key(bounds[3]));
                if (x1 as i64-x0 as i64+1) as u128*(y1 as i64-y0 as i64+1) as u128>8192 {index.large.push(id);continue;}
                for x in x0..=x1 {for y in y0..=y1 {index.cells.entry((x,y)).or_default().push(id);}}
            } index
        });
        let mut ids=index.large.clone();
        for x in x0..=x1 {for y in y0..=y1 {if let Some(cell)=index.cells.get(&(x,y)){ids.extend_from_slice(cell);}}}
        ids.sort_unstable();ids.dedup();ids
    }
    pub fn contains(&self,p:Vec2)->bool {self.candidates(p,p).into_iter().any(|id| {let l=&self.polygons[id];
        if l.bounds.is_some_and(|b|p.x<b[0]||p.y<b[1]||p.x>b[2]||p.y>b[3]) {return false;}
        in_ring(&l.outer,p)&&!l.holes.iter().any(|h|in_ring(h,p))
    })}
    pub fn blocked(&self,a:Vec2,b:Vec2)->bool {
        self.candidates(a,b).into_iter().any(|id| {let l=&self.polygons[id];
            let bbox=l.bounds.unwrap_or_else(||l.outer.iter().fold([f64::INFINITY,f64::INFINITY,f64::NEG_INFINITY,f64::NEG_INFINITY],|v,p|[v[0].min(p.x),v[1].min(p.y),v[2].max(p.x),v[3].max(p.y)]));
            if a.x.max(b.x)<bbox[0] || a.y.max(b.y)<bbox[1] || a.x.min(b.x)>bbox[2] || a.y.min(b.y)>bbox[3] {return false;}
            (in_ring(&l.outer,a)&&!l.holes.iter().any(|h|in_ring(h,a))) || (in_ring(&l.outer,b)&&!l.holes.iter().any(|h|in_ring(h,b)))
                || edges(&l.outer).any(|(c,d)|intersects(a,b,c,d)) || l.holes.iter().any(|h|edges(h).any(|(c,d)|intersects(a,b,c,d)))
        })
    }
    /// Bounded deterministic A*, followed by swept line-of-sight simplification.
    /// 2nm is a routing policy grid, never the coastline collision resolution.
    pub fn route(&self,start:Vec2,goal:Vec2)->Option<Vec<Vec2>> {
        if self.contains(start)||self.contains(goal) {return None;}
        if !self.blocked(start,goal) {return Some(vec![goal]);}
        let step=2.0;let margin=30.0;
        let lo=Vec2::new(start.x.min(goal.x)-margin,start.y.min(goal.y)-margin);
        let hi=Vec2::new(start.x.max(goal.x)+margin,start.y.max(goal.y)+margin);
        let grid=|p:Vec2| (((p.x-lo.x)/step).round() as i32,((p.y-lo.y)/step).round() as i32);
        let origin=grid(start);let point=|n:(i32,i32)| if n==origin {start}else{Vec2::new(lo.x+n.0 as f64*step,lo.y+n.1 as f64*step)};
        let mut heap=BinaryHeap::new();let mut scores=BTreeMap::new();let mut previous=BTreeMap::new();
        scores.insert(origin,0i64);heap.push((0i64,origin));let mut visited=0;
        while let Some((_,n))=heap.pop() {
            visited+=1;if visited>50000 {return None;}
            let p=point(n);let cost=*scores.get(&n)?;
            if p.range_to(goal)<step*1.5 && !self.blocked(p,goal) {
                let mut path=vec![goal,p];let mut cur=n;
                while cur!=origin {cur=*previous.get(&cur)?;path.push(point(cur));}
                path.reverse();let mut smooth=Vec::new();let mut at=0;
                while at+1<path.len() {let mut next=at+1;for j in at+2..path.len(){if !self.blocked(path[at],path[j]) {next=j;}}smooth.push(path[next]);at=next;}
                return Some(smooth);
            }
            for dx in -1..=1 {for dy in -1..=1 {
                if dx==0 && dy==0 {continue;}
                let v=(n.0+dx,n.1+dy);let q=point(v);
                if q.x<lo.x || q.y<lo.y || q.x>hi.x || q.y>hi.y || self.blocked(p,q) {continue;}
                let candidate=cost+if dx!=0 && dy!=0 {1414}else{1000};
                if scores.get(&v).is_none_or(|old|candidate<*old) {
                    scores.insert(v,candidate);previous.insert(v,n);let h=(q.range_to(goal)/step*1000.0) as i64;
                    heap.push((-(candidate+h),v));
                }
            }}
        }
        None
    }
}

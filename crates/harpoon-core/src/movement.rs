//! Real-time steering: continuous course/speed/depth changes, waypoint
//! following, formation stations. Replaces the tabletop's impulse movement —
//! the referee integrates motion every tick at time-compressible rates.

use crate::geom::{angle_diff, normalize_deg, KTS_TO_NM_PER_S};
use crate::types::{PlatformKind, EntityId};
use crate::world::{World, TICK_SECONDS};

pub fn update_movement(world: &mut World) {
    let dt = TICK_SECONDS;
    let ids: Vec<EntityId> = world.entities.keys().copied().collect();
    let mut blocked=Vec::new();

    for id in ids {
        let Some(e) = world.entities.get(&id) else { continue };
        if !e.alive {
            continue;
        }
        let Some(class) = e.class_def(&world.db) else { continue };
        let turn_rate = class.turn_rate_deg_s;
        let accel = class.accel_kts_per_s;
        let _cruise = if class.cruise_speed_kts > 0.0 {
            class.cruise_speed_kts
        } else {
            class.max_speed_kts * 0.7
        };

        // Formation station: derive course/speed orders from leader geometry.
        apply_station(world, id, dt);

        let Some(e) = world.entities.get_mut(&id) else { continue };

        // Waypoint march.
        if let Some(wp) = e.waypoints.front().copied() {
            let dist = e.pos.range_to(wp);
            let safe_to_advance=e.waypoints.get(1).is_none_or(|next|world.context.terrain.as_ref().is_none_or(|t|!matches!(e.kind,PlatformKind::Ship|PlatformKind::Sub)||!t.blocked(e.pos,*next)));
            if dist < 0.1 && safe_to_advance {
                e.waypoints.pop_front();
                if let Some(next) = e.waypoints.front().copied() {
                    e.ordered_course_deg = e.pos.bearing_to(next);
                }
            } else {
                e.ordered_course_deg = e.pos.bearing_to(wp);
                // Slow for the final turn-in.
                if dist < 0.5 {
                    let class = e.class_def(&world.db).cloned();
                    if let Some(c) = class {
                        if c.kind != PlatformKind::Aircraft {
                            e.ordered_speed_kts = e.ordered_speed_kts.min(c.max_speed_kts * 0.5);
                        }
                    }
                }
            }
        }

        // Turn at bounded rate.
        let diff = angle_diff(e.course_deg, e.ordered_course_deg);
        let v2=crate::rules::damage_runtime::marker(e);
        let bridge=crate::rules::damage_runtime::bridge_holds(e,world.tick);
        let disabled=v2&&e.ext.get("damage.rudder_disabled").copied().unwrap_or(0.0)>0.0;
        let max_turn = turn_rate * dt * if disabled{1.0/3.0}else{1.0};
        if bridge{e.course_deg=e.ext.get("damage.bridge_course").copied().unwrap_or(e.course_deg);}
        else if v2&&e.ext.get("damage.rudder_jammed").copied().unwrap_or(0.0)>0.0{
            e.course_deg=normalize_deg(e.course_deg+e.ext.get("damage.rudder_rate").copied().unwrap_or(0.0)*dt);
        }else{e.course_deg = normalize_deg(e.course_deg + diff.clamp(-max_turn, max_turn));}

        // Accelerate/decelerate (decelerate faster than accelerate).
        if !e.waypoints.is_empty() && angle_diff(e.course_deg,e.ordered_course_deg).abs()<5.0 {
            if let Some(speed)=e.ext.remove("terrain.resume_speed") {e.ordered_speed_kts=speed;}
        }
        let cap = e.max_speed(&world.db);
        let target = e.ordered_speed_kts.clamp(0.0, cap);
        let delta = target - e.speed_kts;
        let limit = if delta < 0.0 { accel * 3.0 * dt } else { accel * dt };
        e.speed_kts = (e.speed_kts + delta.clamp(-limit, limit)).max(0.0);
        if bridge{e.speed_kts=e.ext.get("damage.bridge_speed").copied().unwrap_or(e.speed_kts).min(cap);}

        // Vertical motion.
        if e.kind == PlatformKind::Aircraft {
            let climb = (e.ordered_altitude_m - e.altitude_m).clamp(-20.0 * dt, 20.0 * dt);
            e.altitude_m = (e.altitude_m + climb).max(15.0);
        }
        if e.kind == PlatformKind::Sub {
            if let Some(forced)=crate::rules::damage_runtime::forced_depth(e){e.ordered_depth_m=e.ordered_depth_m.min(forced);}
            let drop = (e.ordered_depth_m - e.depth_m).clamp(-3.0 * dt, 3.0 * dt);
            let surface=v2&&e.ordered_depth_m==0.0&&e.depth_m>0.0&&e.depth_m<=25.0;
            let diving=v2&&e.depth_m==0.0&&e.ordered_depth_m>0.0;
            let broach=v2&&e.ext.get("damage.broach_until").copied().unwrap_or(0.0)>world.tick as f64;
            let mut depth=e.depth_m+drop;
            if surface{
                e.ext.remove("nav.dive_progress");
                let progress=e.ext.entry("nav.surface_progress".into()).or_default();*progress+=dt;
                depth=if *progress>=crate::rules::damage_runtime::surface_seconds() as f64{0.0}else{depth.max(1.0)};
            }else if diving&&!broach{
                e.ext.remove("nav.surface_progress");
                let progress=e.ext.entry("nav.dive_progress".into()).or_default();*progress+=dt;
                if *progress<crate::rules::damage_runtime::surface_seconds() as f64{depth=0.0;}
            }else if v2{e.ext.remove("nav.surface_progress");e.ext.remove("nav.dive_progress");}
            if broach{depth=0.0;}
            e.depth_m = depth.clamp(if v2{0.0}else{10.0}, {
                let md = world.db.platforms.get(&e.class).map(|c| c.max_depth_m).unwrap_or(200.0);
                md.max(10.0)
            });
        }

        // Integrate position.
        let v = e.speed_kts * KTS_TO_NM_PER_S * dt;
        let next=e.pos.advanced(e.course_deg,v);
        if v>1e-12 && matches!(e.kind,PlatformKind::Ship|PlatformKind::Sub) && world.context.terrain.as_ref().is_some_and(|t|t.blocked(e.pos,next)) {
            if !e.waypoints.is_empty() {e.ext.entry("terrain.resume_speed".into()).or_insert(e.ordered_speed_kts);}
            e.speed_kts=0.0;e.ordered_speed_kts=0.0;e.station=None;
            let last=e.ext.get("terrain.last_block").copied().unwrap_or(-100000.0);
            if world.tick as f64-last>=30.0 {blocked.push((e.side,e.id));e.ext.insert("terrain.last_block".into(),world.tick as f64);}
        } else {e.pos=next;}
    }
    for (side,id) in blocked {
        world.emit(crate::events::EventKind::RulesEvent {domain:"navigation".into(),rule:"land_stop".into(),side:Some(side),unit:Some(id),contact:None,
            detail:"Charted coast intersects swept movement; stop before land and plot a safe sea route. Navigation guard, not a grounding damage roll.".into()},crate::events::Visibility::Side(side));
    }
}

/// Station-keeping relative to a leader. Bearing is relative to the leader's
/// bow (0 = ahead).
fn apply_station(world: &mut World, id: EntityId, _dt: f64) {
    let (leader_id, bearing_rel, distance) = {
        let Some(e) = world.entities.get(&id) else { return };
        match &e.station {
            Some(s) => (s.leader, s.bearing_rel_deg, s.distance_nm),
            None => return,
        }
    };
    let (leader_pos, leader_course, leader_speed, leader_alive) = {
        let Some(l) = world.entities.get(&leader_id) else { return };
        if !l.alive {
            (l.pos, l.course_deg, l.speed_kts, false)
        } else {
            (l.pos, l.course_deg, l.speed_kts, true)
        }
    };
    if !leader_alive {
        if let Some(e) = world.entities.get_mut(&id) {
            e.station = None;
        }
        return;
    }
    let station_point = leader_pos.advanced(normalize_deg(leader_course + bearing_rel), distance);
    let Some(e) = world.entities.get_mut(&id) else { return };
    let err = e.pos.range_to(station_point);
    if err > 0.25 {
        e.ordered_course_deg = e.pos.bearing_to(station_point);
        // Catch up / slow down proportional to station error.
        let catchup = (err * 4.0).clamp(-15.0, 20.0);
        e.ordered_speed_kts = (leader_speed + catchup).max(4.0);
    } else {
        e.ordered_course_deg = leader_course;
        e.ordered_speed_kts = leader_speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{load_scenario, Scenario, UnitSpawn};

    fn two_ships() -> World {
        let sc = Scenario {
            id: "t".into(),
            name: "T".into(),
            description: String::new(),
            context: Default::default(),
            duration_s: 3600,
            weather: crate::world::Weather { visibility_nm: 12.0, jam: [0.0, 0.0], overcast: 0.0, rain: 0.0 },
            day_night: crate::types::DayNight::Day,
            seed: 1,
            victory: Default::default(),
            units: vec![
                UnitSpawn {
                    side: crate::types::Side::Blue,
                    class: "ffg".into(),
                    name: Some("L".into()),
                    pos: crate::geom::Vec2::new(0.0, 0.0),
                    course_deg: 0.0,
                    speed_kts: 20.0,
                    depth_m: None,
                    altitude_m: None,
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: false,
                },
                UnitSpawn {
                    side: crate::types::Side::Blue,
                    class: "ffg".into(),
                    name: Some("S".into()),
                    pos: crate::geom::Vec2::new(3.0, 0.0),
                    course_deg: 90.0,
                    speed_kts: 20.0,
                    depth_m: None,
                    altitude_m: None,
                    waypoints: None,
                    follow: Some(crate::scenario::FollowSpec {
                        leader_name: "L".into(),
                        bearing_rel_deg: 0.0,
                        distance_nm: 2.0,
                    }),
                    sonar_active: false,
                    fire_on: false,
                },
            ],
        };
        load_scenario(sc, crate::db::Database::built_in()).unwrap()
    }

    #[test]
    fn ship_moves_along_course() {
        let mut w = two_ships();
        let start = w.entities[&crate::types::EntityId(1)].pos;
        for _ in 0..60 {
            crate::sim::tick(&mut w, &[]);
        }
        let end = w.entities[&crate::types::EntityId(1)].pos;
        let moved = start.range_to(end);
        // 20 kts for 60 s = 1/3 nm
        assert!((moved - 1.0 / 3.0).abs() < 0.05, "moved {moved}");
        // heading north
        assert!(w.entities[&crate::types::EntityId(1)].pos.y > start.y);
    }

    #[test]
    fn station_keeper_converges() {
        let mut w = two_ships();
        let sid = crate::types::EntityId(2);
        for _ in 0..2400 {
            crate::sim::tick(&mut w, &[]);
        }
        let l = w.entities[&crate::types::EntityId(1)].clone();
        let s = w.entities[&sid].clone();
        let want = l.pos.advanced(normalize_deg(l.course_deg), 2.0);
        let err = s.pos.range_to(want);
        assert!(err < 0.4, "station error {err}");
    }
}

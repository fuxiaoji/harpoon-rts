//! Real-time steering: continuous course/speed/depth changes, waypoint
//! following, formation stations. Replaces the tabletop's impulse movement —
//! the referee integrates motion every tick at time-compressible rates.

use crate::geom::{angle_diff, normalize_deg, KTS_TO_NM_PER_S};
use crate::types::{PlatformKind, EntityId};
use crate::world::{Entity, World, TICK_SECONDS};

pub fn update_movement(world: &mut World) {
    let dt = TICK_SECONDS;
    let ids: Vec<EntityId> = world.entities.keys().copied().collect();

    for id in ids {
        let Some(e) = world.entities.get(&id) else { continue };
        if !e.alive {
            continue;
        }
        let Some(class) = e.class_def(&world.db) else { continue };
        let turn_rate = class.turn_rate_deg_s;
        let accel = class.accel_kts_per_s;
        let cruise = if class.cruise_speed_kts > 0.0 {
            class.cruise_speed_kts
        } else {
            class.max_speed_kts * 0.7
        };
        drop(class);

        // Formation station: derive course/speed orders from leader geometry.
        apply_station(world, id, dt);

        let Some(e) = world.entities.get_mut(&id) else { continue };

        // Waypoint march.
        if let Some(wp) = e.waypoints.front().copied() {
            let dist = e.pos.range_to(wp);
            if dist < 0.1 {
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
        let max_turn = turn_rate * dt;
        e.course_deg = normalize_deg(e.course_deg + diff.clamp(-max_turn, max_turn));

        // Accelerate/decelerate (decelerate faster than accelerate).
        let cap = e.max_speed(&world.db);
        let target = e.ordered_speed_kts.clamp(0.0, cap);
        let delta = target - e.speed_kts;
        let limit = if delta < 0.0 { accel * 3.0 * dt } else { accel * dt };
        e.speed_kts = (e.speed_kts + delta.clamp(-limit, limit)).max(0.0);

        // Vertical motion.
        if e.kind == PlatformKind::Aircraft {
            let climb = (e.ordered_altitude_m - e.altitude_m).clamp(-20.0 * dt, 20.0 * dt);
            e.altitude_m = (e.altitude_m + climb).max(15.0);
        }
        if e.kind == PlatformKind::Sub {
            let drop = (e.ordered_depth_m - e.depth_m).clamp(-3.0 * dt, 3.0 * dt);
            e.depth_m = (e.depth_m + drop).clamp(10.0, {
                let md = world.db.platforms.get(&e.class).map(|c| c.max_depth_m).unwrap_or(200.0);
                md.max(10.0)
            });
        }

        // Integrate position.
        let v = e.speed_kts * KTS_TO_NM_PER_S * dt;
        e.pos = e.pos.advanced(e.course_deg, v);
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
            duration_s: 3600,
            weather: crate::world::Weather { visibility_nm: 12.0, jam: [0.0, 0.0] },
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

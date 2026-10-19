//! The simulation loop: one tick = one simulated second. Deterministic given
//! (seed, scenario, command log) — replays and RL rollouts rely on this.

use crate::air;
use crate::commands::Command;
use crate::damage;
use crate::events::{EventKind, Visibility};
use crate::movement;
use crate::sensors;
use crate::types::{PlatformKind, Side};
use crate::weapons;
use crate::world::World;

pub fn tick(world: &mut World, commands: &[(Side, Command)]) {
    if world.over {
        return;
    }
    world.tick += 1;

    process_commands(world, commands);
    movement::update_movement(world);
    air::update_air_ops(world);
    crate::rules::step(world);
    weapons::update_weapons(world);
    damage::update_fires(world);
    sensors::update_sensors(world);
    damage::cleanup_dead(world);
    check_victory(world);
}

/// Advance `seconds` of simulated time with a fixed command set (applied once
/// per tick — commands persist like standing orders in an RTS queue).
pub fn run_seconds(world: &mut World, seconds: i64, commands: &[(Side, Command)]) {
    for _ in 0..seconds {
        if world.over {
            return;
        }
        tick(world, commands);
    }
}

fn own_side_of(world: &World, unit: crate::types::EntityId) -> Option<Side> {
    world.entity(unit).map(|e| e.side)
}

fn process_commands(world: &mut World, commands: &[(Side, Command)]) {
    for (side, cmd) in commands {
        if world.log_commands {
            world.command_log.push(crate::world::LoggedCommand {
                tick: world.tick,
                side: *side,
                command: cmd.clone(),
            });
        }
        apply_command(world, *side, cmd);
    }
}

fn apply_command(world: &mut World, side: Side, cmd: &Command) {
    // Ownership validation for unit-bound commands.
    macro_rules! own {
        ($unit:expr) => {
            if own_side_of(world, $unit) != Some(side) {
                return;
            }
        };
    }

    match cmd {
        Command::SetCourse { unit, course_deg } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.ordered_course_deg = crate::geom::normalize_deg(*course_deg);
        }
        Command::SetSpeed { unit, speed_kts } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.ordered_speed_kts = speed_kts.max(0.0);
        }
        Command::SetDepth { unit, depth_m } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.ordered_depth_m = depth_m.max(10.0);
        }
        Command::SetAltitude { unit, altitude_m } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.ordered_altitude_m = altitude_m.max(15.0);
        }
        Command::Goto { unit, waypoints } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.waypoints = waypoints.iter().copied().collect();
            if let Some(first) = waypoints.first() {
                e.ordered_course_deg = e.pos.bearing_to(*first);
            }
        }
        Command::Stop { unit } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.ordered_speed_kts = 0.0;
            e.speed_kts = 0.0;
            e.waypoints.clear();
            e.station = None;
        }
        Command::Follow { unit, leader, bearing_rel_deg, distance_nm } => {
            own!(*unit);
            if own_side_of(world, *leader) != Some(side) {
                return;
            }
            let e = world.entity_mut(*unit).unwrap();
            e.station = Some(crate::world::StationState {
                leader: *leader,
                bearing_rel_deg: *bearing_rel_deg,
                distance_nm: *distance_nm,
            });
        }
        Command::ClearStation { unit } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.station = None;
        }
        Command::SetSonar { unit, active } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            for s in e.sensors.iter_mut() {
                if s.sensor.starts_with("sonar_a") {
                    s.active = *active;
                }
            }
        }
        Command::SetWeaponsFree { unit, free } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.fire_on = *free;
        }
        Command::SetAutoChaff { unit, on } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.auto_chaff = *on;
        }
        Command::AssignTarget { unit, contact } => {
            own!(*unit);
            // Contact must belong to the issuing side.
            if !world.sides[side.index()].contacts.contains_key(contact) {
                return;
            }
            let e = world.entity_mut(*unit).unwrap();
            e.assigned = Some(*contact);
        }
        Command::ClearTarget { unit } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.assigned = None;
        }
        Command::Fire { unit, contact, weapon, rounds } => {
            own!(*unit);
            weapons::manual_fire(world, side, *unit, *contact, weapon, *rounds);
        }
        Command::DeployChaff { unit } => {
            own!(*unit);
            let (has_chaff, uside) = {
                let e = world.entity_mut(*unit).unwrap();
                let has = e.chaff > 0 && e.chaff_active_s <= 0.0;
                if has {
                    e.chaff -= 1;
                    e.chaff_active_s = 30.0;
                }
                (has, e.side)
            };
            if has_chaff {
                sensors::chaff_breaks_lock(world, *unit);
                world.emit(
                    EventKind::ChaffDeployed { side: uside, unit: *unit },
                    Visibility::Side(uside),
                );
            }
        }
        Command::LaunchAircraft { unit, class, count, mission } => {
            own!(*unit);
            if let Err(reason) = air::launch_aircraft(world, *unit, class, *count, mission) {
                world.emit(
                    EventKind::DeckBusy { side, carrier: *unit },
                    Visibility::Side(side),
                );
                let _ = reason;
            }
        }
        Command::RecoverAircraft { unit } => {
            own!(*unit);
            // Recall all own aircraft loosely near this carrier.
            let cpos = world.entity(*unit).map(|c| c.pos).unwrap_or_default();
            let recall_ids: Vec<crate::types::EntityId> = world
                .entities
                .values()
                .filter(|e| {
                    e.alive
                        && e.kind == PlatformKind::Aircraft
                        && e.side == side
                        && e.pos.range_to(cpos) < 60.0
                })
                .map(|e| e.id)
                .collect();
            for aid in recall_ids {
                let e = world.entity_mut(aid).unwrap();
                e.mission = Some(crate::world::MissionState::Rtb);
            }
        }
    }
}

fn check_victory(world: &mut World) {
    if world.over {
        return;
    }
    // Victory needs two participating sides.
    if !(world.sides_present[0] && world.sides_present[1]) {
        return;
    }
    let (vp_blue, vp_red) = world.victory_points();
    let target = world.scenario_target_vp;

    // Annihilation: a side with no units left loses.
    let blue_alive = world
        .entities
        .values()
        .any(|e| e.alive && e.side == Side::Blue && !e.kind.is_projectile());
    let red_alive = world
        .entities
        .values()
        .any(|e| e.alive && e.side == Side::Red && !e.kind.is_projectile());

    let mut outcome: Option<(Option<Side>, String)> = None;
    if world.context.area_denial.is_some() {
        // Scenario objectives supersede the generic VP/time-limit outcome.
        outcome = crate::mission::adjudicate(world);
    } else if !blue_alive && !red_alive {
        outcome = Some((None, "mutual annihilation".into()));
    } else if !blue_alive {
        outcome = Some((Some(Side::Red), "Blue forces destroyed".into()));
    } else if !red_alive {
        outcome = Some((Some(Side::Blue), "Red forces destroyed".into()));
    } else if target > 0.0 && (vp_blue - vp_red).abs() >= target {
        let winner = if vp_blue > vp_red { Side::Blue } else { Side::Red };
        outcome = Some((Some(winner), "VP margin reached".into()));
    } else if world.tick >= world.duration_s {
        let winner = if vp_blue > vp_red {
            Some(Side::Blue)
        } else if vp_red > vp_blue {
            Some(Side::Red)
        } else {
            None
        };
        outcome = Some((winner, "time limit".into()));
    }

    if let Some((winner, reason)) = outcome {
        world.over = true;
        world.result = Some(crate::world::VictoryResult {
            winner,
            reason,
            vp_blue,
            vp_red,
        });
        world.emit(
            EventKind::ScenarioEnded { winner, reason: world.result.as_ref().unwrap().reason.clone(), vp_blue, vp_red },
            Visibility::All,
        );
    }
}

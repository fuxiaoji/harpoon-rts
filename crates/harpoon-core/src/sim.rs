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

    crate::rules::damage_runtime::begin(world);
    process_commands(world, commands);
    movement::update_movement(world);
    air::update_air_ops(world);
    crate::rules::step(world);
    weapons::update_weapons(world);
    crate::rules::damage_runtime::finish(world);
    damage::update_fires(world);
    sensors::update_sensors(world);
    crate::exercise::update_alert(world);
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
    for order in crate::rules::damage_runtime::due_orders(world) {
        if !crate::rules::damage_runtime::bridge_holds_order(world,&order) {apply_command_now(world,order.side,&order.command);}
    }
    for (side, cmd) in commands {
        if world.log_commands {
            world.command_log.push(crate::world::LoggedCommand {
                tick: world.tick,
                side: *side,
                command: cmd.clone(),
            });
        }
        if !crate::rules::damage_runtime::active(world){apply_command(world, *side, cmd);}
    }
    if crate::rules::damage_runtime::active(world){
        // Preserve each side's ordered command stream while eliminating
        // cross-side network interleaving as an allocation/RNG advantage.
        // Emission/navigation orders take effect before planned Fire orders.
        for firing in [false,true]{for side in [Side::Blue,Side::Red]{
            for (owner,cmd) in commands.iter().filter(|(owner,cmd)|*owner==side&&matches!(cmd,Command::Fire{..})==firing){apply_command(world,*owner,cmd);}
        }}
    }
}

fn apply_command(world: &mut World, side: Side, cmd: &Command) {
    if crate::rules::damage_runtime::delay_navigation_order(world,side,cmd){return;}
    apply_command_now(world,side,cmd);
}
fn apply_command_now(world: &mut World, side: Side, cmd: &Command) {
    if crate::exercise::locked(world,side) {
        world.emit(EventKind::RulesEvent {domain:"exercise".into(),rule:"command_locked".into(),side:Some(side),
            unit:None,contact:None,detail:"Active orders are locked until the exercise alert; routine flight and sensors continue".into()},Visibility::Side(side));
        return;
    }
    // Ownership validation for unit-bound commands.
    macro_rules! own {
        ($unit:expr) => {
            if own_side_of(world, $unit) != Some(side) {
                return;
            }
        };
    }

    match cmd {
        Command::SetUnitAutomation{unit,enabled}=>{
            own!(*unit);
            // Transferring command authority cancels navigation waiting under
            // the previous owner. Newly submitted orders still incur the
            // actual bridge-damage delay; takeover never repairs the bridge.
            world.rules.delayed_orders.retain(|o|match &o.command {
                Command::SetCourse{unit:id,..}|Command::SetSpeed{unit:id,..}|Command::Goto{unit:id,..}|Command::Stop{unit:id}|Command::Follow{unit:id,..}|Command::ClearStation{unit:id}=>id!=unit,
                _=>true,
            });
            let e=world.entity_mut(*unit).unwrap();
            if *enabled {e.ext.remove("control.manual");}else{e.ext.insert("control.manual".into(),1.0);}
            command_receipt(world,side,*unit,"set_unit_automation",true,if *enabled{"Higher-level AI may issue orders again"}else{"Manual command retained; current task and basic self-defence continue"});
        }
        Command::SetCourse { unit, course_deg } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.ordered_course_deg = crate::geom::normalize_deg(*course_deg);
        }
        Command::SetSpeed { unit, speed_kts } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.ext.remove("terrain.resume_speed");
            e.ordered_speed_kts = speed_kts.max(0.0);
        }
        Command::SetDepth { unit, depth_m } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.ordered_depth_m = depth_m.max(if crate::rules::damage_runtime::marker(e){0.0}else{10.0});
        }
        Command::SetAltitude { unit, altitude_m } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.ordered_altitude_m = altitude_m.max(15.0);
        }
        Command::SetThrottle {unit,setting} => {
            own!(*unit);
            let result=crate::rules::flight_profiles::configure(world,*unit,setting);
            command_receipt(world,side,*unit,"set_throttle",result.is_ok(),&result.err().unwrap_or_else(||setting.clone()));
        }
        Command::DamageControl {unit,action}=>{
            own!(*unit);
            let result=crate::rules::damage_runtime::configure_control(world,*unit,action);
            command_receipt(world,side,*unit,"damage_control",result.is_ok(),&result.err().unwrap_or_else(||action.clone()));
        }
        Command::SetTorpedoRun{unit,weapon,deep}=>{
            own!(*unit);
            let result=crate::rules::damage_runtime::configure_torpedo(world,*unit,weapon,*deep);
            command_receipt(world,side,*unit,"set_torpedo_run",result.is_ok(),&result.err().unwrap_or_else(||if *deep{"deep contact run"}else{"shallow contact run"}.into()));
        }
        Command::Goto { unit, waypoints } => {
            own!(*unit);
            let mut plotted=waypoints.clone();
            if let (Some(t),Some(e))=(&world.context.terrain,world.entity(*unit)) {
                if matches!(e.kind,PlatformKind::Ship|PlatformKind::Sub) {
                    plotted.clear();let mut from=e.pos;
                    for goal in waypoints {
                        let Some(route)=t.route(from,*goal) else {
                            command_receipt(world,side,*unit,"goto",false,"charted land blocks destination or no safe sea route within planner bounds");return;
                        };
                        plotted.extend(route);from=*goal;
                    }
                }
            }
            let e = world.entity_mut(*unit).unwrap();
            e.waypoints = plotted.iter().copied().collect();
            if let Some(first) = plotted.first() {
                e.ordered_course_deg = e.pos.bearing_to(*first);
            }
        }
        Command::Stop { unit } => {
            own!(*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.ext.remove("terrain.resume_speed");
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
        Command::SetRadar { unit, active } => {
            own!(*unit);
            let changed=world.entity(*unit).is_some_and(|e|crate::sensors::radar_emitting(e)!=*active);
            let e = world.entity_mut(*unit).unwrap();
            e.ext.insert("emission.radar".into(), if *active {1.0} else {0.0});
            if changed {command_receipt(world, side, *unit, "set_radar", true, if *active {"search radar emitting"} else {"search radar silent; passive sensors retained"});}
        }
        Command::SetJamming {unit,mode,contacts} => {
            own!(*unit);
            let result=crate::rules::electronic_warfare::configure(world,*unit,mode,contacts);
            command_receipt(world,side,*unit,"set_jamming",result.is_ok(),&result.err().unwrap_or_else(||format!("{mode}; effect requires verified victim-radar generation and valid horizon/sector; jammer effects do not stack")));
        }
        Command::SetEngagementPolicy { unit, air_only, max_in_flight } => {
            own!(*unit);
            let e=world.entity_mut(*unit).unwrap();
            e.ext.insert("doctrine.air_only".into(),if *air_only {1.0}else{0.0});
            e.ext.insert("doctrine.max_in_flight".into(),*max_in_flight as f64);
        }
        Command::RetaskAircraft { unit, mission, manual } => {
            own!(*unit);
            if world.entity(*unit).is_some_and(|e| e.alive && e.kind == PlatformKind::Aircraft) {
                let e = world.entity_mut(*unit).unwrap();
                e.mission = Some(crate::world::MissionState::from_air_mission(mission));
                e.ext.insert("air.manual_control".into(), if *manual {1.0} else {0.0});
                e.ext.insert("manual_strike_release".into(), if *manual {1.0} else {0.0});
                e.waypoints.clear();
                e.station = None;
                e.assigned = None;
                command_receipt(world, side, *unit, "retask_aircraft", true, mission.tag());
            } else {
                command_receipt(world, side, *unit, "retask_aircraft", false, "requires a live aircraft");
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
            let executed = weapons::manual_fire(world, side, *unit, *contact, weapon, *rounds);
            command_receipt(world, side, *unit, "fire", executed,
                &format!("weapon={weapon}; contact={}; requested={rounds}; {}", contact.0,
                    if executed {"weapon action executed; see weapons_release/gun_action for actual rounds"}
                    else {"not executed: check fresh firing track, weapon compatibility, range, ammunition, cooldown and channels"}));
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
        Command::LaunchAircraft { unit, class, count, mission, profile } => {
            own!(*unit);
            if let Err(reason) = air::launch_planned_aircraft(world, *unit, class, *count, mission, profile.as_ref()) {
                world.emit(
                    EventKind::DeckBusy { side, carrier: *unit, reason },
                    Visibility::Side(side),
                );
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
        Command::ReturnToBase {unit} => {
            own!(*unit);
            let e=world.entity_mut(*unit).unwrap();
            if e.alive && e.kind==PlatformKind::Aircraft {
                e.mission=Some(crate::world::MissionState::Rtb);
                e.waypoints.clear();
                e.station=None;
                e.assigned=None;
                e.fire_on=false;
            }
        }
    }
}

fn command_receipt(world: &mut World, side: Side, unit: crate::types::EntityId, cmd: &str, executed: bool, detail: &str) {
    world.emit(EventKind::RulesEvent { domain:"command".into(), rule:if executed {"executed"} else {"rejected"}.into(),
        side:Some(side), unit:Some(unit), contact:None, detail:format!("{cmd}: {detail}") }, Visibility::Side(side));
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
    let (blue_kills, red_kills) = world.kill_counts();
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
    if world.context.island_exercise.is_some() {
        outcome=crate::exercise::adjudicate(world);
    }else if world.context.area_denial.is_some() {
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
        let (winner, reason) = if world.scenario_kill_count_at_end {
            // Weighted scores decide when the scenario configures per-class
            // kill values; raw platform counts otherwise.
            let winner = if world.scenario_kill_weights.is_some() {
                let (bp, rp) = (world.kill_score(Side::Blue), world.kill_score(Side::Red));
                if bp > rp { Some(Side::Blue) } else if rp > bp { Some(Side::Red) } else { None }
            } else if blue_kills > red_kills {
                Some(Side::Blue)
            } else if red_kills > blue_kills {
                Some(Side::Red)
            } else {
                None
            };
            (winner, "time limit: kill count".to_string())
        } else {
            let winner = if vp_blue > vp_red {
                Some(Side::Blue)
            } else if vp_red > vp_blue {
                Some(Side::Red)
            } else {
                None
            };
            (winner, "time limit".to_string())
        };
        outcome = Some((winner, reason));
    }

    if let Some((winner, reason)) = outcome {
        world.over = true;
        let exercise=crate::exercise::scores(world);
        let scored = world.scenario_kill_weights.is_some() || exercise.is_some();
        let (blue_points, red_points) = if let Some((b,r,_,_))=exercise {(b,r)}else if scored {
            (world.kill_score(Side::Blue), world.kill_score(Side::Red))
        } else {
            (blue_kills as f64, red_kills as f64)
        };
        let (vp_blue,vp_red)=if exercise.is_some() {(blue_points,red_points)}else{(vp_blue,vp_red)};
        world.result = Some(crate::world::VictoryResult {
            winner,
            reason,
            vp_blue,
            vp_red,
            blue_kills,
            red_kills,
            blue_points,
            red_points,
            scored,
            blue_kills_by_kind: world.sides[Side::Blue.index()].confirmed_kills_by_kind.clone(),
            red_kills_by_kind: world.sides[Side::Red.index()].confirmed_kills_by_kind.clone(),
        });
        world.emit(
            EventKind::ScenarioEnded { winner, reason: world.result.as_ref().unwrap().reason.clone(), vp_blue, vp_red },
            Visibility::All,
        );
    }
}

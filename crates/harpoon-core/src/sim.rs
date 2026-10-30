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
    crate::rules::unmanned2026::phase_open(world);

    crate::rules::damage_runtime::begin(world);
    crate::rules::asw_weapons2026::begin_depth_charge_phase(world);
    crate::rules::torpedo2026::begin(world);
    // Hard-rudder Resolution consumes the just-closed180s window before this
    // tick's commands and movement: a jam from windowW is freed no later than
    // the resolution ofW, including its own turn of jam.
    crate::rules::navigation2026::hard::resolution(world);
    process_commands(world, commands);
    crate::rules::navigation2026::step(world);
    crate::rules::aviation2026::step(world);
    crate::rules::navigation2026::hazards::before_movement(world);
    movement::update_movement(world);
    crate::rules::navigation2026::hazards::after_movement(world);
    crate::rules::aviation2026::waterborne::after_movement(world);
    crate::rules::aviation2026::waterborne::swamp_step(world);
    air::update_air_ops(world);
    crate::rules::step(world);
    weapons::update_weapons(world);
    // Source9.6 waits for actual defensive SAM and the following normal gun
    // scan. The resolver never reruns global fire control or releases from a
    // plane that was destroyed by those defenses.
    crate::rules::aircombat2026::surface::prepare_surface_releases(world);
    crate::rules::pgm2026::prepare_releases(world);
    crate::rules::rocket2026::prepare_releases(world);
    crate::rules::cluster2026::prepare_releases(world);
    crate::rules::strafe2026::prepare_releases(world);
    crate::rules::aircombat2026::budget::close_ready(world);
    crate::rules::aircombat2026::surface::commit_surface_releases(world);
    crate::rules::pgm2026::commit_releases(world);
    crate::rules::rocket2026::commit_releases(world);
    crate::rules::cluster2026::commit_releases(world);
    crate::rules::strafe2026::commit_releases(world);
    // All this tick's projectile and instant ordnance land hits share one
    // damage-ratio/functional-loss batch, independent of provider order.
    crate::rules::surface2026::gunnery::arbitrate_domains(world);
    crate::rules::surface2026::gunnery::interrupt_starshells(world);
    crate::rules::surface2026::gunnery::resolve_starshells(world);
    crate::rules::ground2026::ngfs::resolve_referee(world);
    crate::rules::surface2026::gunnery::resolve(world);
    crate::rules::ground2026::ngfs::late_batch(world);
    crate::rules::surface2026::flush_land_hits(world);
    crate::rules::asw_weapons2026::resolve_projector_patterns(world);
    crate::rules::asw_weapons2026::resolve_depth_charge_patterns(world);
    crate::rules::damage_runtime::finish(world);
    crate::rules::navigation2026::hazards::after_damage(world);
    crate::rules::nuclear2026::step(world);
    damage::update_fires(world);
    // SALH samples its already-claimed live designator after this tick's
    // conventional/nuclear casualties. It resolves one deferred terminal
    // batch and never performs another fire-control or whole sensor scan.
    crate::rules::pgm2026::resolve_deferred_guidance(world);
    // Alongside inventory settles after this tick's movement and all damage;
    // a berth disabled in the common batch cannot complete its former due job.
    crate::rules::ground2026::berth::step(world);
    crate::rules::ground2026::gci::step(world);
    crate::rules::ground2026::drydock::step(world);
    sensors::update_sensors(world);
    crate::rules::detection2026::soks::step(world);
    crate::rules::detection2026::laser::step(world);
    crate::rules::surface2026::antiship::after_detection(world);
    crate::rules::unmanned2026::finish_telemetry(world);
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
pub(crate) fn commit_manual(world:&mut World,unit:crate::EntityId){
    if world.entity(unit).is_some_and(|e|e.ext.get("control.manual")!=Some(&1.0)) {
        world.rules.delayed_orders.retain(|o|!matches!(&o.command,Command::SetCourse{unit:id,..}|Command::SetSpeed{unit:id,..}|Command::Goto{unit:id,..}|Command::Stop{unit:id}|Command::Follow{unit:id,..}|Command::ClearStation{unit:id} if *id==unit));
    }
    if let Some(e)=world.entity_mut(unit){e.ext.insert("control.manual".into(),1.);}
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

pub(crate) fn deliver_gci_order(world: &mut World, side: Side, cmd: &Command) {
    apply_command(world, side, cmd);
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
    // Hardware command/control is independent from higher-level FSM ownership.
    // Basic modem data/control uses its own typed actions; it cannot carry an
    // arbitrary aircraft task, weapon release or complete sensor configuration.
    let actor=cmd.unit_id();
    if !matches!(cmd,Command::Unmanned{..}|Command::SetUnitAutomation{..}) {
        if crate::rules::unmanned2026::can_control(world,side,actor).is_err(){return;}
        if crate::rules::unmanned2026::basic_control_only(world,actor)&&!matches!(cmd,Command::SetCourse{..}|Command::SetSpeed{..}|Command::SetDepth{..}|Command::Stop{..}){return;}
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
            crate::rules::navigation2026::hard::replaced_by_standard_order(world,*unit);
            let e = world.entity_mut(*unit).unwrap();
            e.ordered_course_deg = crate::geom::normalize_deg(*course_deg);
        }
        Command::SetSpeed { unit, speed_kts } => {
            own!(*unit);
            if crate::rules::aviation2026::waterborne::controls_water_motion(world,*unit) && *speed_kts!=0. {
                command_receipt(world,side,*unit,"set_speed",false,"4.6.5 resting aircraft requires declared water takeoff");return;
            }
            let e = world.entity_mut(*unit).unwrap();
            e.ext.remove("terrain.resume_speed");
            e.ordered_speed_kts = speed_kts.max(0.0);
        }
        Command::SetDepth { unit, depth_m } => {
            own!(*unit);
            if let Err(reason)=crate::rules::battery2026::validate_depth_change(world,*unit,*depth_m) {
                command_receipt(world,side,*unit,"set_depth",false,&reason);return;
            }
            if let Err(reason)=crate::rules::navigation2026::submarine::validate_depth_declaration(world,*unit,*depth_m) {
                command_receipt(world,side,*unit,"set_depth",false,&reason);
                return;
            }
            if crate::rules::navigation2026::submarine::configured_depth(world,*unit) {
                let result=crate::rules::navigation2026::submarine::order_depth(world,*unit,*depth_m);
                command_receipt(world,side,*unit,"set_depth",result.is_ok(),&result.err().unwrap_or_else(||"来源潜深命令已执行".into()));
            } else {
                let e = world.entity_mut(*unit).unwrap();
                e.ordered_depth_m = depth_m.max(if crate::rules::damage_runtime::marker(e){0.0}else{10.0});
            }
        }
        Command::SetAltitude { unit, altitude_m } => {
            own!(*unit);
            if crate::rules::aviation2026::waterborne::controls_water_motion(world,*unit){
                if *altitude_m!=0.{command_receipt(world,side,*unit,"set_altitude",false,"4.6.5 resting aircraft requires declared water takeoff");}
                return;
            }
            let e = world.entity_mut(*unit).unwrap();
            e.ordered_altitude_m = altitude_m.max(15.0);
        }
        Command::SetThrottle {unit,setting} => {
            own!(*unit);
            let result=if crate::rules::aviation2026::active(world){
                let throttle=match setting.as_str(){"cruise"=>Some(crate::rules::aviation2026::Throttle::Cruise),"fmp"=>Some(crate::rules::aviation2026::Throttle::Fmp),"reheat"=>Some(crate::rules::aviation2026::Throttle::Reheat),"loiter"=>Some(crate::rules::aviation2026::Throttle::Loiter),"hover"=>Some(crate::rules::aviation2026::Throttle::Hover),_=>None};
                throttle.ok_or_else(||"未知油门模式".to_string()).and_then(|setting|crate::rules::aviation2026::apply(world,*unit,&crate::rules::aviation2026::Action::Throttle{setting}))
            }else{crate::rules::flight_profiles::configure(world,*unit,setting)};
            command_receipt(world,side,*unit,"set_throttle",result.is_ok(),&result.err().unwrap_or_else(||setting.clone()));
        }
        Command::Aviation {unit,action}=>{
            own!(*unit);
            let result=crate::rules::aviation2026::apply(world,*unit,action);
            command_receipt(world,side,*unit,"aviation",result.is_ok(),&result.err().unwrap_or_else(||"航空勤务命令已执行".into()));
        }
        Command::AirCombat {unit,action}=>{
            own!(*unit);
            let result=crate::rules::aircombat2026::apply(world,*unit,action);
            command_receipt(world,side,*unit,"air_combat",result.is_ok(),&result.err().unwrap_or_else(||"航空交战命令已执行".into()));
        }
        Command::SetArray {unit,sensor,deploy,depth_m}=>{
            own!(*unit);
            let result=if world.rules.profile==crate::rules::damage_runtime::RuleProfile::Rts2026Full{crate::rules::detection2026::order_array(world,*unit,sensor,*deploy,*depth_m)}else{Err("拖曳阵勤务命令需要v4规则".into())};
            command_receipt(world,side,*unit,"set_array",result.is_ok(),&result.err().unwrap_or_else(||"拖曳阵命令已执行".into()));
        }
        Command::Mines {unit,action}=>{
            own!(*unit);
            let result=crate::rules::mines2026::apply(world,*unit,action);
            command_receipt(world,side,*unit,"mines",result.is_ok(),&result.err().unwrap_or_else(||"水雷勤务命令已执行".into()));
        }
        Command::Navigation {unit,action}=>{
            own!(*unit);
            let result=crate::rules::navigation2026::apply(world,*unit,action);
            command_receipt(world,side,*unit,"navigation",result.is_ok(),&result.err().unwrap_or_else(||"来源航行命令已执行".into()));
        }
        Command::Antiship {unit,action}=>{
            own!(*unit);
            let result=crate::rules::surface2026::antiship::apply(world,*unit,action);
            command_receipt(world,side,*unit,"antiship",result.is_ok(),&result.err().unwrap_or_else(||"反舰弹航行计划已登记".into()));
        }
        Command::Communications {unit,action,retain_manual}=>{
            own!(*unit);
            let optical=crate::rules::communications2026::visual_request(action);
            let optical_queued=optical.is_some();
            let result=if let Some(request)=optical {
                crate::rules::detection2026::visual_signaling::queue(world,side,*unit,request,*retain_manual)
            }else{crate::rules::communications2026::apply(world,side,*unit,action)};
            if result.is_ok() && *retain_manual && !optical_queued {
                if world.entity(*unit).unwrap().ext.get("control.manual")!=Some(&1.0) {
                    // Only an actual ownership transition cancels navigation
                    // from the previous controller. Existing player orders stay.
                    world.rules.delayed_orders.retain(|o|match &o.command {
                        Command::SetCourse{unit:id,..}|Command::SetSpeed{unit:id,..}|Command::Goto{unit:id,..}|Command::Stop{unit:id}|Command::Follow{unit:id,..}|Command::ClearStation{unit:id}=>id!=unit,
                        _=>true,
                    });
                }
                world.entity_mut(*unit).unwrap().ext.insert("control.manual".into(),1.0);
            }
            command_receipt(world,side,*unit,"communications",result.is_ok(),&result.err().unwrap_or_else(||if optical_queued{"光信号已排入本回合发送阶段".into()}else{"本方原始报告警告已排入本回合探测阶段".into()}));
        }
        Command::Unmanned {unit,action,retain_manual}=>{
            own!(*unit);let result=crate::rules::unmanned2026::apply(world,side,*unit,action);
            if result.is_ok()&&*retain_manual {commit_manual(world,*unit);}
            command_receipt(world,side,*unit,"unmanned",result.is_ok(),&result.err().unwrap_or_else(||"无人系统控制命令已执行".into()));
        }
        Command::Nuclear {unit,action,retain_manual}=>{
            own!(*unit);let result=crate::rules::nuclear2026::apply(world,side,*unit,action);
            if result.is_ok()&&*retain_manual {commit_manual(world,*unit);}
            command_receipt(world,side,*unit,"nuclear",result.is_ok(),&result.err().unwrap_or_else(||"本方核战斗部设置已更新".into()));
        }
        Command::Ground {unit,action}=>{
            own!(*unit);
            let result=crate::rules::ground2026::apply(world,*unit,action);
            command_receipt(world,side,*unit,"ground",result.is_ok(),&result.err().unwrap_or_else(||"场站勤务命令已执行".into()));
        }
        Command::Asw{unit,action,retain_manual}=>{
            own!(*unit);
            let result=crate::rules::asw_weapons2026::control(world,*unit,action.clone());
            if result.is_ok()&&*retain_manual{commit_manual(world,*unit);}
            command_receipt(world,side,*unit,"asw",result.is_ok(),&result.err().unwrap_or_else(||"反潜阵形命令已执行".into()));
        }
        Command::Detection{unit,action}=>{
            own!(*unit);
            let result=crate::rules::detection2026::apply(world,*unit,action);
            command_receipt(world,side,*unit,"detection",result.is_ok(),&result.err().unwrap_or_else(||"探测勤务命令已执行".into()));
        }
        Command::SearchAndRescue {unit,action,retain_manual}=>{
            own!(*unit);
            let result=crate::rules::expanded_damage2026::rescue_control(world,*unit,action.clone());
            if result.is_ok()&&*retain_manual {commit_manual(world,*unit);}
            command_receipt(world,side,*unit,"search_and_rescue",result.is_ok(),&result.err().unwrap_or_else(||"搜救命令已执行".into()));
        }
        Command::ElectronicWarfare{unit,action}=>{
            own!(*unit);
            let result=crate::rules::ew2026::apply(world,*unit,action);
            command_receipt(world,side,*unit,"electronic_warfare",result.is_ok(),&result.err().unwrap_or_else(||"电子战部署命令已执行".into()));
        }
        Command::Torpedo{unit,action}=>{
            own!(*unit);
            let result=crate::rules::torpedo2026::apply(world,*unit,action);
            command_receipt(world,side,*unit,"torpedo",result.is_ok(),&result.err().unwrap_or_else(||"鱼雷勤务命令已执行".into()));
        }
        Command::AamHoj{unit,weapon,enabled}=>{
            own!(*unit);
            let result=crate::rules::aam2026::set_hoj(world,side,*unit,weapon,*enabled);
            command_receipt(world,side,*unit,"aam_hoj",result.is_ok(),&result.err().unwrap_or_else(||"空空导弹被动寻干扰模式已设定".into()));
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
            crate::rules::navigation2026::hard::replaced_by_standard_order(world,*unit);
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
            crate::rules::navigation2026::hard::replaced_by_standard_order(world,*unit);
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
            crate::rules::navigation2026::hard::replaced_by_standard_order(world,*unit);
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
            let full=world.rules.profile==crate::rules::damage_runtime::RuleProfile::Rts2026Full;
            let active_sensors:std::collections::BTreeSet<_>=world.db.sensors.iter().filter(|(_,s)|s.kind==crate::db::SensorKind::SonarActive).map(|(id,_)|id.clone()).collect();
            let e = world.entity_mut(*unit).unwrap();
            for s in e.sensors.iter_mut() {
                if if full{active_sensors.contains(&s.sensor)}else{s.sensor.starts_with("sonar_a")} {
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
            crate::rules::operations::cancel_launches(world,*unit);
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
        .any(|e| e.alive && e.side == Side::Blue && !e.kind.is_projectile()
            && !(world.rules.profile == crate::rules::damage_runtime::RuleProfile::Rts2026Full
                && e.class == crate::rules::asw::SONOBUOY_CLASS)
            && !crate::rules::detection2026::sonobuoys::is_sensor_float(world,e.id));
    let red_alive = world
        .entities
        .values()
        .any(|e| e.alive && e.side == Side::Red && !e.kind.is_projectile()
            && !(world.rules.profile == crate::rules::damage_runtime::RuleProfile::Rts2026Full
                && e.class == crate::rules::asw::SONOBUOY_CLASS)
            && !crate::rules::detection2026::sonobuoys::is_sensor_float(world,e.id));

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

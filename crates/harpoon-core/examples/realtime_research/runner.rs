use harpoon_core::{
    load_scenario, observe, sim, Command, Database, EntityId, Observation, Rng, Scenario, Side,
    Vec2, World,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub earliest_s: i64,
    pub latest_s: i64,
    pub lifetime_s: i64,
    pub offset_nm: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Template {
    pub schema_version: u32,
    pub family: String,
    pub episode_s: i64,
    pub goal_radius_nm: f64,
    pub initial_goal: Vec2,
    pub change: Option<Change>,
    pub scenario: Scenario,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Clock {
    Injected,
    Wall { tick_ms: u64 },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Policy {
    Hold,
    Event,
    Fixed { every_s: i64 },
}

impl Policy {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "hold" => Ok(Self::Hold),
            "event" => Ok(Self::Event),
            "f60" => Ok(Self::Fixed { every_s: 60 }),
            "f300" => Ok(Self::Fixed { every_s: 300 }),
            "f600" => Ok(Self::Fixed { every_s: 600 }),
            _ => Err("--policy must be hold, event, f60, f300 or f600".into()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub policy: Policy,
    pub latency_s: i64,
    pub seed: u64,
    pub side: Side,
    pub mirror: bool,
    pub clock: Clock,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub revision: u32,
    pub goal: Vec2,
    pub visible_since_tick: i64,
    pub deadline_tick: i64,
    pub radius_nm: f64,
}

/// This value is the complete planner input. No template, future update or World.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub observation: Observation,
    pub public_task: Task,
    pub standing_revision: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: usize,
    pub input: Input,
    #[serde(default)]
    pub triggered_tick: i64,
    #[serde(default)]
    pub triggered_wall_us: u64,
    pub observed_tick: i64,
    pub requested_tick: i64,
    pub requested_wall_us: u64,
    pub trigger: String,
    pub response_tick: Option<i64>,
    pub response_wall_us: Option<u64>,
    pub validated_tick: Option<i64>,
    pub validated_wall_us: Option<u64>,
    pub applied_tick: Option<i64>,
    pub applied_wall_us: Option<u64>,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Applied {
    pub tick: i64,
    pub request_id: usize,
    pub commands: Vec<Command>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub tick: i64,
    pub pos: Vec2,
    pub task_revision: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Update {
    pub tick: i64,
    pub goal: Vec2,
    pub deadline_tick: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub schema_version: String,
    pub planner_kind: String,
    pub family: String,
    pub config: Config,
    pub initial_scenario: Scenario,
    pub database_hash: String,
    pub update: Option<Update>,
    pub final_task: Task,
    pub success: bool,
    pub reached_tick: Option<i64>,
    pub first_reaction_tick: Option<i64>,
    pub requests: Vec<Request>,
    pub commands: Vec<Applied>,
    pub positions: Vec<Position>,
    pub world_hash: String,
    pub replay_verified: bool,
    pub episode_wall_us: u64,
    pub monitor_checks: usize,
    pub model_tokens: Option<u64>,
    pub model_cost: Option<f64>,
}

fn hash<T: Serialize>(v: &T) -> String {
    let mut h = 0xcbf29ce484222325u64;
    for b in serde_json::to_vec(v).expect("serializable state") {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn elapsed_us(start: Instant) -> u64 {
    start.elapsed().as_micros() as u64
}

fn generate(t: &Template, c: &Config) -> Result<(Scenario, Option<Update>), String> {
    if t.schema_version != 1
        || t.episode_s < 1
        || !t.goal_radius_nm.is_finite()
        || t.goal_radius_nm <= 0.0
    {
        return Err("invalid research template".into());
    }
    if !(0..=3600).contains(&c.latency_s) {
        return Err("latency must be 0..3600 simulated seconds".into());
    }
    if matches!(c.clock, Clock::Wall { tick_ms: 0 }) {
        return Err("wall tick_ms must be positive".into());
    }
    if matches!(c.policy, Policy::Fixed { every_s } if every_s <= 0) {
        return Err("fixed period must be positive".into());
    }
    let mut sc = t.scenario.clone();
    sc.seed = c.seed;
    sc.duration_s = t.episode_s;
    if sc.units.len() != 2 {
        return Err("v1 expects one task vessel and one distant control".into());
    }
    sc.units[0].side = c.side;
    sc.units[1].side = c.side.other();
    if c.mirror {
        for u in &mut sc.units {
            u.pos.y = -u.pos.y;
            u.course_deg = (180.0 - u.course_deg).rem_euclid(360.0);
            if let Some(points) = &mut u.waypoints {
                for p in points {
                    p.y = -p.y;
                }
            }
        }
    }
    let update = if let Some(change) = &t.change {
        if change.earliest_s < 1
            || change.latest_s < change.earliest_s
            || change.lifetime_s < 1
            || change.latest_s + change.lifetime_s > t.episode_s
            || !change.offset_nm.is_finite()
            || change.offset_nm <= 0.0
        {
            return Err("invalid task-change range/deadline".into());
        }
        let mut rng = Rng::new(c.seed).derive(0x7461736b);
        let tick = change.earliest_s
            + (rng.next_u64() % (change.latest_s - change.earliest_s + 1) as u64) as i64;
        let sign = if rng.next_u64() % 2 == 0 { 1.0 } else { -1.0 };
        // Exogenous update: generated from initial orders, never from policy outcomes.
        let u = &t.scenario.units[0];
        let mut goal = u
            .pos
            .advanced(u.course_deg, u.speed_kts * tick as f64 / 3600.0);
        goal.y += sign * change.offset_nm;
        if c.mirror {
            goal.y = -goal.y;
        }
        Some(Update {
            tick,
            goal,
            deadline_tick: tick + change.lifetime_s,
        })
    } else {
        None
    };
    Ok((sc, update))
}

/// Cheap scripted reference: it reads only the supplied own observation/task.
fn plan(input: &Input) -> Vec<Command> {
    if input.standing_revision == input.public_task.revision {
        return vec![];
    }
    let Some(unit) = input.observation.units.first() else {
        return vec![];
    };
    vec![
        Command::Goto {
            unit: unit.id,
            waypoints: vec![input.public_task.goal],
        },
        Command::SetSpeed {
            unit: unit.id,
            speed_kts: 18.0,
        },
    ]
}

struct Reply {
    commands: Vec<Command>,
    completed_wall_us: u64,
}
enum Pending {
    Injected {
        id: usize,
        ready_tick: i64,
        commands: Vec<Command>,
    },
    Wall {
        id: usize,
        receiver: mpsc::Receiver<Reply>,
        thread: thread::JoinHandle<()>,
    },
}
impl Pending {
    fn id(&self) -> usize {
        match self {
            Self::Injected { id, .. } | Self::Wall { id, .. } => *id,
        }
    }
    fn poll(&self, now: i64, start: Instant) -> Option<Reply> {
        match self {
            Self::Injected {
                ready_tick,
                commands,
                ..
            } if now >= *ready_tick => Some(Reply {
                commands: commands.clone(),
                completed_wall_us: elapsed_us(start),
            }),
            Self::Wall { receiver, .. } => receiver.try_recv().ok(),
            _ => None,
        }
    }
}

pub fn run(t: &Template, config: Config) -> Result<Run, String> {
    let (scenario, update) = generate(t, &config)?;
    let db = Database::built_in();
    let database_hash = hash(&db);
    let mut world = load_scenario(scenario.clone(), db)?;
    let mut initial_goal = t.initial_goal;
    if config.mirror {
        initial_goal.y = -initial_goal.y;
    }
    let mut task = Task {
        revision: 0,
        goal: initial_goal,
        visible_since_tick: 0,
        deadline_tick: t.episode_s,
        radius_nm: t.goal_radius_nm,
    };
    let start = Instant::now();
    let mut pending: Option<Pending> = None;
    let mut queued: Option<(usize, Vec<Command>)> = None;
    let mut requests: Vec<Request> = vec![];
    let mut commands: Vec<Applied> = vec![];
    let mut positions = vec![];
    let mut standing_revision = 0;
    let mut reached_tick = None;
    let mut first_reaction_tick = None;
    let mut monitor_revision = 0;
    let mut latched_at: Option<(i64, u64)> = None;
    let mut monitor_checks = 0;
    for now in 0..=t.episode_s {
        if let Clock::Wall { tick_ms } = config.clock {
            let due = Duration::from_millis(now as u64 * tick_ms);
            if let Some(wait) = due.checked_sub(start.elapsed()) {
                thread::sleep(wait);
            }
        }
        if let Some(u) = &update {
            if u.tick == now {
                task = Task {
                    revision: 1,
                    goal: u.goal,
                    visible_since_tick: now,
                    deadline_tick: u.deadline_tick,
                    radius_nm: t.goal_radius_nm,
                };
                reached_tick = None;
            }
        }
        let mut this_tick = vec![];
        if let Some((id, batch)) = queued.take() {
            let req = &mut requests[id];
            if req.input.public_task.revision != task.revision {
                req.status = "stale_before_application".into();
            } else if now > task.deadline_tick {
                req.status = "expired_before_application".into();
            } else {
                if !batch.is_empty() && first_reaction_tick.is_none() && task.revision > 0 {
                    first_reaction_tick = Some(now);
                }
                standing_revision = task.revision;
                req.status = "applied".into();
                req.applied_tick = Some(now);
                req.applied_wall_us = Some(elapsed_us(start));
                this_tick = batch.iter().cloned().map(|c| (config.side, c)).collect();
                commands.push(Applied {
                    tick: now,
                    request_id: id,
                    commands: batch,
                });
            }
        }
        if now > 0 {
            sim::tick(&mut world, &this_tick);
        }
        if world.tick != now {
            return Err(format!(
                "engine stopped unexpectedly at {} (wanted {now})",
                world.tick
            ));
        }
        let obs = observe(&world, config.side, 0, 32);
        let vessel = obs
            .units
            .iter()
            .find(|u| u.id == EntityId(1))
            .ok_or("task vessel missing from own observation")?;
        if now <= task.deadline_tick
            && vessel.pos.range_to(task.goal) <= task.radius_nm
            && reached_tick.is_none()
        {
            reached_tick = Some(now);
        }
        positions.push(Position {
            tick: now,
            pos: vessel.pos,
            task_revision: task.revision,
        });
        if let Some(reply) = pending.as_ref().and_then(|p| p.poll(now, start)) {
            let p = pending.take().unwrap();
            let id = p.id();
            if let Pending::Wall { thread, .. } = p {
                thread.join().map_err(|_| "planner thread panicked")?;
            }
            let req = &mut requests[id];
            req.response_tick = Some(now);
            req.response_wall_us = Some(reply.completed_wall_us);
            req.validated_tick = Some(now);
            req.validated_wall_us = Some(elapsed_us(start));
            if req.input.public_task.revision != task.revision {
                req.status = "stale_response".into();
            } else if now + 1 > task.deadline_tick {
                req.status = "expired_response".into();
            } else if now == t.episode_s {
                req.status = "after_horizon".into();
            } else {
                req.status = "queued".into();
                queued = Some((id, reply.commands));
            }
        }
        let trigger = match config.policy {
            Policy::Hold => false,
            Policy::Event => {
                monitor_checks += 1;
                let changed = task.revision != monitor_revision;
                monitor_revision = task.revision;
                changed
            }
            Policy::Fixed { every_s } => now > 0 && now % every_s == 0,
        };
        if trigger && latched_at.is_none() {
            latched_at = Some((now, elapsed_us(start)));
        }
        if latched_at.is_some() && pending.is_none() && now < t.episode_s {
            let (triggered_tick, triggered_wall_us) = latched_at.take().unwrap();
            let input = Input {
                observation: obs,
                public_task: task.clone(),
                standing_revision,
            };
            let id = requests.len();
            requests.push(Request {
                id,
                triggered_tick,
                triggered_wall_us,
                observed_tick: now,
                requested_tick: now,
                requested_wall_us: elapsed_us(start),
                input: input.clone(),
                trigger: match config.policy {
                    Policy::Event => "task_revision",
                    _ => "fixed_grid",
                }
                .into(),
                response_tick: None,
                response_wall_us: None,
                validated_tick: None,
                validated_wall_us: None,
                applied_tick: None,
                applied_wall_us: None,
                status: "in_flight".into(),
            });
            let batch = plan(&input);
            pending = Some(match config.clock {
                Clock::Injected => Pending::Injected {
                    id,
                    ready_tick: now + config.latency_s.max(1),
                    commands: batch,
                },
                Clock::Wall { tick_ms } => {
                    let (sender, receiver) = mpsc::channel();
                    let latency_ms = config.latency_s as u64 * tick_ms;
                    let worker = thread::spawn(move || {
                        thread::sleep(Duration::from_millis(latency_ms));
                        let _ = sender.send(Reply {
                            commands: batch,
                            completed_wall_us: elapsed_us(start),
                        });
                    });
                    Pending::Wall {
                        id,
                        receiver,
                        thread: worker,
                    }
                }
            });
        }
    }
    let episode_wall_us = elapsed_us(start);
    if let Some(p) = pending {
        requests[p.id()].status = "in_flight_at_horizon".into();
        if let Pending::Wall { thread, .. } = p {
            thread.join().map_err(|_| "planner thread panicked")?;
        }
    }
    let world_hash = hash(&world);
    let mut result = Run {
        schema_version: "harpoon.realtime-diagnostic.v1".into(),
        planner_kind: "scripted_own_observation_v1".into(),
        family: t.family.clone(),
        config,
        initial_scenario: scenario,
        database_hash,
        update,
        final_task: task,
        success: reached_tick.is_some(),
        reached_tick,
        first_reaction_tick,
        requests,
        commands,
        positions,
        world_hash,
        replay_verified: false,
        episode_wall_us,
        monitor_checks,
        model_tokens: None,
        model_cost: None,
    };
    replay(&result)?;
    result.replay_verified = true;
    Ok(result)
}

pub fn replay(r: &Run) -> Result<(), String> {
    let db = Database::built_in();
    if hash(&db) != r.database_hash {
        return Err("replay database fingerprint differs".into());
    }
    let mut w: World = load_scenario(r.initial_scenario.clone(), db)?;
    let duration = r.initial_scenario.duration_s;
    let mut at = 0;
    for tick in 1..=duration {
        let mut batch = vec![];
        while at < r.commands.len() && r.commands[at].tick == tick {
            batch.extend(
                r.commands[at]
                    .commands
                    .iter()
                    .cloned()
                    .map(|c| (r.config.side, c)),
            );
            at += 1;
        }
        sim::tick(&mut w, &batch);
        let p = r
            .positions
            .get(tick as usize)
            .ok_or("missing trajectory tick")?;
        if w.tick != p.tick || w.entity(EntityId(1)).unwrap().pos != p.pos {
            return Err(format!("trajectory replay differs at {tick}"));
        }
    }
    if at != r.commands.len() || hash(&w) != r.world_hash {
        return Err("full World replay differs".into());
    }
    Ok(())
}

impl Run {
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({"family": self.family, "seed": self.config.seed, "side": self.config.side,
            "mirror": self.config.mirror, "policy": self.config.policy, "clock": self.config.clock,
            "latency_s": self.config.latency_s, "success": self.success, "reached_tick": self.reached_tick,
            "event_tick": self.update.as_ref().map(|u| u.tick), "deadline_tick": self.final_task.deadline_tick,
            "first_reaction_tick": self.first_reaction_tick, "requests": self.requests.len(),
            "stale_responses": self.requests.iter().filter(|r| r.status.starts_with("stale")).count(),
            "expired_responses": self.requests.iter().filter(|r| r.status.starts_with("expired")).count(),
            "monitor_checks": self.monitor_checks, "world_hash": self.world_hash,
            "replay_verified": self.replay_verified, "episode_wall_us": self.episode_wall_us,
            "model_tokens": self.model_tokens, "model_cost": self.model_cost})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn template(case: &str) -> Template {
        let raw = match case {
            "d0" => include_str!("../../../../research/realtime/scenarios/d0.json"),
            "d1" => include_str!("../../../../research/realtime/scenarios/d1.json"),
            _ => include_str!("../../../../research/realtime/scenarios/d2.json"),
        };
        serde_json::from_str(raw).unwrap()
    }
    fn config(policy: Policy, latency_s: i64) -> Config {
        Config {
            policy,
            latency_s,
            seed: 7,
            side: Side::Blue,
            mirror: false,
            clock: Clock::Injected,
        }
    }
    #[test]
    fn standing_order_completes_without_a_call() {
        let r = run(&template("d0"), config(Policy::Event, 5)).unwrap();
        assert!(r.success && r.replay_verified);
        assert!(r.requests.is_empty());
        assert_eq!(r.positions.len(), 1201);
    }
    #[test]
    fn paired_tasks_differ_only_in_deadline() {
        let c = config(Policy::Event, 5);
        let (a, u) = generate(&template("d1"), &c).unwrap();
        let (b, v) = generate(&template("d2"), &c).unwrap();
        assert_eq!(
            serde_json::to_value(a).unwrap(),
            serde_json::to_value(b).unwrap()
        );
        let (u, v) = (u.unwrap(), v.unwrap());
        assert_eq!((u.tick, u.goal), (v.tick, v.goal));
        assert_eq!(u.deadline_tick - v.deadline_tick, 720);
    }
    #[test]
    fn timely_response_succeeds_but_late_response_misses_tight_window() {
        let fast = run(&template("d2"), config(Policy::Event, 5)).unwrap();
        let slow = run(&template("d2"), config(Policy::Event, 180)).unwrap();
        let wide = run(&template("d1"), config(Policy::Event, 180)).unwrap();
        assert!(fast.success, "{}", fast.summary());
        assert!(!slow.success);
        assert!(wide.success, "{}", wide.summary());
    }
    #[test]
    fn delayed_old_revision_is_rejected_and_trigger_is_latched() {
        let r = run(&template("d1"), config(Policy::Fixed { every_s: 60 }, 200)).unwrap();
        assert!(r
            .requests
            .iter()
            .any(|q| q.requested_tick > q.triggered_tick));
        assert!(r.requests.iter().any(|q| q.status == "stale_response"));
        assert!(r
            .requests
            .iter()
            .any(|q| q.input.public_task.revision == 1 && q.status == "applied"));
        assert!(r
            .commands
            .iter()
            .all(|a| r.requests[a.request_id].input.public_task.revision == 1));
    }
    #[test]
    fn pre_update_input_does_not_disclose_future_or_family() {
        let a = run(&template("d1"), config(Policy::Fixed { every_s: 60 }, 5)).unwrap();
        let b = run(&template("d2"), config(Policy::Fixed { every_s: 60 }, 5)).unwrap();
        let ai = serde_json::to_value(&a.requests[0].input).unwrap();
        let bi = serde_json::to_value(&b.requests[0].input).unwrap();
        assert_eq!(ai, bi);
        assert!(a.requests[0]
            .input
            .observation
            .units
            .iter()
            .all(|u| u.side == Side::Blue));
        assert!(a.requests[0].input.observation.contacts.is_empty());
        assert_eq!(a.requests[0].input.public_task.revision, 0);
    }
    #[test]
    fn side_and_geometric_mirror_preserve_task_outcome() {
        let a = run(&template("d2"), config(Policy::Event, 5)).unwrap();
        let mut c = config(Policy::Event, 5);
        c.side = Side::Red;
        c.mirror = true;
        let b = run(&template("d2"), c).unwrap();
        assert_eq!(a.success, b.success);
        assert!((a.reached_tick.unwrap() - b.reached_tick.unwrap()).abs() <= 1);
        assert!(b.requests.iter().all(|r| r
            .input
            .observation
            .units
            .iter()
            .all(|u| u.side == Side::Red)));
    }
    #[test]
    fn trace_survives_roundtrip_and_command_tampering_is_detected() {
        let r = run(&template("d1"), config(Policy::Event, 5)).unwrap();
        let mut restored: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        replay(&restored).unwrap();
        restored.commands[0].commands.clear();
        assert!(replay(&restored).is_err());
    }
}

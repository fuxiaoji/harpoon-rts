//! Line-delimited private judge bridge for a continuously paced external caller.
//! Only the public Input crosses the bridge during play; future events stay here.
#[allow(dead_code)]
mod base {
    include!("realtime_research/runner.rs");
    pub fn setup(t: &Template, seed: u64) -> Result<(Scenario, Option<Update>), String> {
        generate(
            t,
            &Config {
                policy: Policy::Hold,
                latency_s: 0,
                seed,
                side: Side::Blue,
                mirror: false,
                clock: Clock::Injected,
            },
        )
    }
    pub fn fingerprint<T: Serialize>(value: &T) -> String {
        hash(value)
    }
}

use harpoon_core::{
    load_scenario, observe, sim, Command, Database, EntityId, Scenario, Side, Vec2, World,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{self, BufRead, Write},
    path::PathBuf,
};

#[derive(Clone, Serialize, Deserialize)]
struct Point {
    tick: i64,
    pos: Vec2,
    task_revision: u32,
    #[serde(default)]
    public_notice_seq: u32,
}
#[derive(Clone, Serialize, Deserialize)]
struct Batch {
    tick: i64,
    request_id: usize,
    commands: Vec<Command>,
}
#[derive(Serialize, Deserialize)]
struct Trace {
    schema_version: String,
    initial_scenario: Scenario,
    update: Option<base::Update>,
    #[serde(default)]
    updates: Vec<base::Update>,
    #[serde(default)]
    notice_ticks: Vec<i64>,
    final_task: base::Task,
    positions: Vec<Point>,
    commands: Vec<Batch>,
    success: bool,
    reached_tick: Option<i64>,
    world_hash: String,
    database_hash: String,
    replay_verified: bool,
}
struct Judge {
    template: base::Template,
    scenario: Scenario,
    update: Option<base::Update>,
    updates: Vec<base::Update>,
    notice_ticks: Vec<i64>,
    notice_seq: u32,
    world: World,
    task: base::Task,
    standing_revision: u32,
    positions: Vec<Point>,
    commands: Vec<Batch>,
    reached: Option<i64>,
}
impl Judge {
    fn new(template: base::Template, seed: u64) -> Result<Self, String> {
        let (scenario, update) = base::setup(&template, seed)?;
        let world = load_scenario(scenario.clone(), Database::built_in())?;
        let task = base::Task {
            revision: 0,
            goal: template.initial_goal,
            visible_since_tick: 0,
            deadline_tick: template.episode_s,
            radius_nm: template.goal_radius_nm,
        };
        let pos = world.entity(EntityId(1)).ok_or("missing task vessel")?.pos;
        Ok(Self {
            template,
            scenario,
            updates: update.iter().cloned().collect(),
            update,
            notice_ticks: vec![],
            notice_seq: 0,
            world,
            task,
            standing_revision: 0,
            positions: vec![Point {
                tick: 0,
                pos,
                task_revision: 0,
                public_notice_seq: 0,
            }],
            commands: vec![],
            reached: None,
        })
    }
    fn input(&self) -> base::Input {
        base::Input {
            observation: observe(&self.world, Side::Blue, 0, 32),
            public_task: self.task.clone(),
            standing_revision: self.standing_revision,
        }
    }
    fn snapshot(&self) -> Value {
        let mut input = serde_json::to_value(self.input()).unwrap();
        input["public_notice_seq"] = json!(self.notice_seq);
        input["public_notice_meaning"] = json!("Notification delivery count, including routine repeats. Task revision changes only on replacement.");
        input
    }
    fn add_dense_changes(&mut self) {
        self.configure_dense_changes(30, 900, 30);
    }
    fn configure_dense_changes(&mut self, gap: i64, lifetime: i64, notice_every: i64) {
        let first = self.update.as_ref().unwrap().clone();
        let second_tick = first.tick + gap;
        self.updates.truncate(1);
        self.updates.push(base::Update {
            tick: second_tick,
            goal: Vec2::new(first.goal.x + 18.0 * gap as f64 / 3600.0, -first.goal.y),
            deadline_tick: second_tick + lifetime,
        });
        self.notice_ticks = if notice_every > 0 {
            (notice_every..self.template.episode_s)
                .step_by(notice_every as usize)
                .collect()
        } else {
            vec![]
        };
    }
    fn terminal(&self) -> bool {
        self.world.tick >= self.template.episode_s
            || (self.updates.iter().all(|u| u.tick <= self.world.tick)
                && (self.reached.is_some() || self.world.tick >= self.task.deadline_tick))
    }
    fn set_second_goal_dx(&mut self, dx: f64) -> Result<(), String> {
        if !dx.is_finite() || dx.abs() > 10.0 || self.updates.len() != 2 {
            return Err("invalid second-goal offset".into());
        }
        self.updates[1].goal.x = self.updates[0].goal.x + dx;
        Ok(())
    }
    fn step(&mut self, expected_tick: i64, proposed: Option<&Value>) -> Result<Value, String> {
        let now = self.world.tick + 1;
        if expected_tick != now || now > self.template.episode_s {
            return Err("invalid step order".into());
        }
        for (index, u) in self.updates.iter().enumerate() {
            if u.tick == now {
                self.task = base::Task {
                    revision: index as u32 + 1,
                    goal: u.goal,
                    visible_since_tick: now,
                    deadline_tick: u.deadline_tick,
                    radius_nm: self.template.goal_radius_nm,
                };
                self.reached = None;
            }
        }
        if self.notice_ticks.contains(&now) {
            self.notice_seq += 1;
        }
        let mut batch = vec![];
        let mut application = Value::Null;
        if let Some(p) = proposed {
            let id = p["request_id"].as_u64().ok_or("missing request id")? as usize;
            let status = if p["task_revision"].as_u64() != Some(self.task.revision as u64) {
                "stale_before_application"
            } else if now > self.task.deadline_tick {
                "expired_before_application"
            } else if p["unit"].as_u64() != Some(1) {
                "illegal_unit"
            } else if p["action"] == "hold" {
                "applied"
            } else if p["action"] == "goto" {
                let point: Vec2 =
                    serde_json::from_value(p["waypoint"].clone()).map_err(|e| e.to_string())?;
                if point.range_to(self.task.goal) > 1e-9 {
                    "illegal_waypoint"
                } else {
                    batch = vec![
                        Command::Goto {
                            unit: EntityId(1),
                            waypoints: vec![point],
                        },
                        Command::SetSpeed {
                            unit: EntityId(1),
                            speed_kts: 18.0,
                        },
                    ];
                    self.standing_revision = self.task.revision;
                    "applied"
                }
            } else {
                "illegal_action"
            };
            if status == "applied" {
                self.commands.push(Batch {
                    tick: now,
                    request_id: id,
                    commands: batch.clone(),
                });
            }
            application = json!({"request_id":id,"status":status,"commands":batch});
        }
        sim::tick(
            &mut self.world,
            &batch
                .into_iter()
                .map(|c| (Side::Blue, c))
                .collect::<Vec<_>>(),
        );
        if self.world.tick != now {
            return Err("production engine stopped unexpectedly".into());
        }
        let pos = self
            .world
            .entity(EntityId(1))
            .ok_or("missing own ship")?
            .pos;
        if now <= self.task.deadline_tick
            && pos.range_to(self.task.goal) <= self.task.radius_nm
            && self.reached.is_none()
        {
            self.reached = Some(now);
        }
        self.positions.push(Point {
            tick: now,
            pos,
            task_revision: self.task.revision,
            public_notice_seq: self.notice_seq,
        });
        Ok(json!({"input":self.snapshot(),"application":application,"terminal":self.terminal()}))
    }
    fn finish(&self) -> Result<Trace, String> {
        self.finish_with_terminal(false)
    }
    fn finish_with_terminal(&self, allow_terminal: bool) -> Result<Trace, String> {
        if self.world.tick != self.template.episode_s && !(allow_terminal && self.terminal()) {
            return Err("unfinished episode".into());
        }
        let mut trace = Trace {
            schema_version: "harpoon.realtime-live.v2".into(),
            initial_scenario: self.scenario.clone(),
            update: self.update.clone(),
            updates: self.updates.clone(),
            notice_ticks: self.notice_ticks.clone(),
            final_task: self.task.clone(),
            positions: self.positions.clone(),
            commands: self.commands.clone(),
            success: self.reached.is_some(),
            reached_tick: self.reached,
            world_hash: base::fingerprint(&self.world),
            database_hash: base::fingerprint(&Database::built_in()),
            replay_verified: false,
        };
        replay(&trace)?;
        trace.replay_verified = true;
        Ok(trace)
    }
}
fn replay(trace: &Trace) -> Result<(), String> {
    let mut world = load_scenario(trace.initial_scenario.clone(), Database::built_in())?;
    for point in &trace.positions {
        if point.tick > 0 {
            let commands: Vec<_> = trace
                .commands
                .iter()
                .filter(|b| b.tick == point.tick)
                .flat_map(|b| b.commands.iter().cloned().map(|c| (Side::Blue, c)))
                .collect();
            sim::tick(&mut world, &commands);
        }
        if world.entity(EntityId(1)).unwrap().pos != point.pos {
            return Err("replay position mismatch".into());
        }
    }
    if base::fingerprint(&world) != trace.world_hash {
        return Err("replay World mismatch".into());
    }
    Ok(())
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let value = |key: &str| args.windows(2).find(|p| p[0] == key).map(|p| p[1].clone());
    let number = |key: &str, default: i64| -> Result<i64, String> {
        value(key)
            .map(|s| s.parse::<i64>().map_err(|e| e.to_string()))
            .unwrap_or(Ok(default))
    };
    if let Some(path) = value("--replay") {
        let trace: Trace =
            serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        replay(&trace)?;
        println!(
            "{}",
            json!({"replay_verified":true,"world_hash":trace.world_hash})
        );
        return Ok(());
    }
    let family = value("--case").unwrap_or_else(|| "d2".into());
    if !["d0", "d1", "d2", "d5"].contains(&family.as_str()) {
        return Err("invalid family".into());
    }
    let template_family = if family == "d5" { "d1" } else { &family };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../research/realtime/scenarios/{template_family}.json"
    ));
    let mut template: base::Template =
        serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    template.episode_s = number("--episode-s", template.episode_s)?;
    template.scenario.duration_s = template.episode_s;
    if let Some(change) = template.change.as_mut() {
        if let Some(phase) = value("--first-event-tick") {
            let phase = phase.parse::<i64>().map_err(|e| e.to_string())?;
            if phase < 1 || phase >= template.episode_s {
                return Err("invalid first event phase".into());
            }
            change.earliest_s = phase;
            change.latest_s = phase;
        }
        change.lifetime_s = number("--event-window-s", change.lifetime_s)?;
    }
    if family == "d2" {
        let window = value("--tight-window-s")
            .unwrap_or_else(|| "300".into())
            .parse::<i64>()
            .map_err(|e| e.to_string())?;
        template.change.as_mut().unwrap().lifetime_s = window;
    }
    let seed = value("--seed")
        .unwrap_or_else(|| "7".into())
        .parse::<u64>()
        .map_err(|e| e.to_string())?;
    let mut judge = Judge::new(template, seed)?;
    if family == "d5" {
        let gap = number("--gap-s", 30)?;
        let lifetime = number("--event-window-s", 900)?;
        let notice = number("--notice-every-s", 30)?;
        if gap < 1
            || lifetime < 1
            || notice < 0
            || judge.update.as_ref().unwrap().tick + gap + lifetime > judge.template.episode_s
        {
            return Err("dense events/deadline outside horizon".into());
        }
        judge.configure_dense_changes(gap, lifetime, notice);
        if let Some(dx) = value("--second-goal-dx-nm") {
            judge.set_second_goal_dx(dx.parse::<f64>().map_err(|e| e.to_string())?)?;
        }
    }
    let mut output = io::stdout().lock();
    writeln!(output, "{}", json!({"input":judge.snapshot()})).map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    for line in io::stdin().lock().lines() {
        let message: Value =
            serde_json::from_str(&line.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let answer = match message["kind"].as_str() {
            Some("step") => judge.step(
                message["tick"].as_i64().ok_or("missing tick")?,
                message.get("plan").filter(|p| !p.is_null()),
            )?,
            Some("finish") => {
                let trace = judge
                    .finish_with_terminal(message["allow_terminal"].as_bool().unwrap_or(false))?;
                writeln!(output, "{}", serde_json::to_string(&trace).unwrap())
                    .map_err(|e| e.to_string())?;
                output.flush().map_err(|e| e.to_string())?;
                break;
            }
            _ => return Err("unknown bridge request".into()),
        };
        writeln!(output, "{answer}").map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn judge() -> Judge {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../research/realtime/scenarios/d2.json");
        Judge::new(
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap(),
            7,
        )
        .unwrap()
    }
    #[test]
    fn dense_updates_keep_second_change_private_until_publication() {
        let mut j = judge();
        j.add_dense_changes();
        let first = j.updates[0].tick;
        let second = j.updates[1].tick;
        assert_eq!(second - first, 30);
        assert!(j.snapshot().get("updates").is_none());
        for now in 1..=second {
            j.step(now, None).unwrap();
            assert_eq!(
                j.task.revision,
                if now < first {
                    0
                } else if now < second {
                    1
                } else {
                    2
                }
            );
        }
        assert!(j.notice_seq > 0);
    }
    #[test]
    fn routine_notices_do_not_change_task_revision() {
        let mut j = judge();
        j.add_dense_changes();
        for now in 1..=60 {
            j.step(now, None).unwrap();
        }
        assert_eq!(j.notice_seq, 2);
        assert_eq!(j.task.revision, 0);
    }
    #[test]
    fn dense_parameters_do_not_depend_on_control_policy() {
        let mut j = judge();
        j.configure_dense_changes(180, 420, 45);
        assert_eq!(j.updates[1].tick - j.updates[0].tick, 180);
        assert_eq!(j.updates[1].deadline_tick - j.updates[1].tick, 420);
        assert!((j.updates[1].goal.x - j.updates[0].goal.x - 0.9).abs() < 1e-9);
        assert_eq!(j.notice_ticks[0], 45);
        assert!(j.snapshot().get("updates").is_none());
    }
    #[test]
    fn fixed_goal_offset_is_independent_of_event_gap() {
        let mut short = judge();
        let mut long = judge();
        short.configure_dense_changes(30, 420, 30);
        long.configure_dense_changes(180, 420, 30);
        short.set_second_goal_dx(0.15).unwrap();
        long.set_second_goal_dx(0.15).unwrap();
        assert_eq!(short.updates[1].goal, long.updates[1].goal);
        assert_ne!(short.updates[1].tick, long.updates[1].tick);
        assert!(short.set_second_goal_dx(f64::NAN).is_err());
    }
    #[test]
    fn early_finish_cannot_reveal_unpublished_future_event() {
        let mut j = judge();
        j.configure_dense_changes(180, 420, 30);
        j.reached = Some(0);
        assert!(!j.terminal());
        assert!(j.finish_with_terminal(true).is_err());
        for now in 1..=j.updates[1].deadline_tick {
            j.step(now, None).unwrap();
        }
        assert!(j.terminal());
        assert!(j.finish_with_terminal(true).is_ok());
        assert!(j.finish().is_err());
    }
    #[test]
    fn snapshot_hides_future_update_and_enemy() {
        let j = judge();
        let input = serde_json::to_value(j.input()).unwrap();
        assert!(input.get("update").is_none());
        assert_eq!(input["public_task"]["revision"], 0);
        assert_eq!(input["observation"]["units"].as_array().unwrap().len(), 1);
        assert!(input["observation"]["contacts"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn stale_plan_is_rejected_at_publication_boundary() {
        let mut j = judge();
        let publication = j.update.as_ref().unwrap().tick;
        for now in 1..publication {
            j.step(now, None).unwrap();
        }
        let response = j
            .step(
                publication,
                Some(&json!({"request_id":0,"task_revision":0,"unit":1,"action":"hold"})),
            )
            .unwrap();
        assert_eq!(
            response["application"]["status"],
            "stale_before_application"
        );
        assert_eq!(j.standing_revision, 0);
    }
    #[test]
    fn hold_does_not_claim_new_revision_was_executed() {
        let mut j = judge();
        let publication = j.update.as_ref().unwrap().tick;
        for now in 1..publication {
            j.step(now, None).unwrap();
        }
        j.step(
            publication,
            Some(&json!({"request_id":0,"task_revision":1,"unit":1,"action":"hold"})),
        )
        .unwrap();
        assert_eq!(j.standing_revision, 0);
    }
    #[test]
    fn expired_plan_cannot_be_applied_even_with_current_revision() {
        let mut j = judge();
        let update = j.update.as_ref().unwrap().clone();
        for now in 1..=update.deadline_tick {
            j.step(now, None).unwrap();
        }
        let answer = j
            .step(
                update.deadline_tick + 1,
                Some(&json!({
                    "request_id":0,"task_revision":1,"unit":1,"action":"goto","waypoint":update.goal
                })),
            )
            .unwrap();
        assert_eq!(
            answer["application"]["status"],
            "expired_before_application"
        );
        assert!(j.commands.is_empty());
    }
    #[test]
    fn applied_command_log_replays_full_world() {
        let mut j = judge();
        let u = j.update.as_ref().unwrap().clone();
        for now in 1..=j.template.episode_s {
            let p = json!({"request_id":0,"task_revision":1,"unit":1,"action":"goto","waypoint":u.goal});
            j.step(now, if now == u.tick + 5 { Some(&p) } else { None })
                .unwrap();
        }
        let trace = j.finish().unwrap();
        assert!(trace.success && trace.replay_verified);
    }
}

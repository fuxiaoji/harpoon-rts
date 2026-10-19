//! Authoritative training objectives. This module observes the existing RTS
//! engine; it never replaces attack tables, dice, damage, or sensor results.
use harpoon_core::{Command, EntityId, EventKind, Side, World};
use harpoon_core::observe::observe;
use harpoon_core::types::PlatformKind;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Deserialize, Serialize)]
pub struct Step {
    pub id: String,
    pub objective: String,
    pub target: Option<f64>,
    pub point: Option<[f64; 2]>,
    #[serde(skip_serializing)]
    pub answer: Option<String>,
    #[serde(flatten)]
    pub content: BTreeMap<String, Value>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Course {
    pub id: String,
    pub scenario: String,
    pub steps: Vec<Step>,
    #[serde(flatten)]
    pub content: BTreeMap<String, Value>,
}
#[derive(Deserialize, Serialize)]
pub struct Catalog {
    pub version: u32,
    pub courses: Vec<Course>,
}
pub fn catalog() -> Catalog {
    serde_json::from_str(include_str!("../../../data/training/courses.json"))
        .expect("training curriculum must be valid JSON")
}

pub struct Session {
    pub attempt: u64,
    pub course: Course,
    pub unit: EntityId,
    pub step: usize,
    pub status: &'static str,
    pub feedback: &'static str,
    pub mistakes: u32,
    pub retries: u32,
    pub evidence: Vec<Value>,
    started_tick: i64,
    start_seq: u64,
    step_seq: u64,
    checkpoint: Box<World>,
    checkpoint_wave: bool,
    wave_sent: bool,
    manual_recall: bool,
    run_requested: bool,
    wave_started_tick: Option<i64>,
}

#[derive(Deserialize)]
pub struct Action {
    pub action: String,
    pub attempt: u64,
    pub step: usize,
    pub unit: Option<u32>,
    pub contact: Option<u32>,
    pub answer: Option<String>,
}

impl Session {
    pub fn new(course: Course, attempt: u64, world: &mut World) -> Result<Self, String> {
        let unit = world.entities.values().find(|e| e.side == Side::Blue && e.alive)
            .map(|e| e.id).ok_or("training unit missing")?;
        // These are explicit training posture choices, not adjudication overrides.
        for entity in world.entities.values_mut() {
            entity.auto_chaff = false;
            entity.fire_on = false;
            entity.assigned = None;
        }
        Ok(Self {
            attempt, course, unit, step: 0, status: "active", feedback: "ready",
            mistakes: 0, retries: 0, evidence: Vec::new(), started_tick: world.tick,
            start_seq: world.event_seq, step_seq: world.event_seq,
            checkpoint: Box::new(world.clone()), checkpoint_wave: false,
            wave_sent: false, manual_recall: false, run_requested: false, wave_started_tick: None,
        })
    }
    pub fn current(&self) -> Option<&Step> { self.course.steps.get(self.step) }
    fn advance(&mut self, world: &World, reason: &'static str) {
        if let Some(step) = self.current() {
            self.evidence.push(json!({"step": step.id, "tick": world.tick, "reason": reason}));
        }
        self.step += 1;
        self.feedback = "objective_met";
        self.step_seq = world.event_seq;
        self.checkpoint = Box::new(world.clone());
        self.checkpoint_wave = self.wave_sent;
        if self.step == self.course.steps.len() { self.status = "completed"; }
    }
    pub fn validate_action(&self, action: &Action) -> Result<(), &'static str> {
        if action.attempt != self.attempt || action.step != self.step {
            return Err("stale_attempt");
        }
        Ok(())
    }
    pub fn action(&mut self, world: &mut World, action: &Action) -> Result<(), &'static str> {
        self.validate_action(action)?;
        if action.action == "retry" {
            if self.step >= self.course.steps.len() { return Err("not_active"); }
            *world = (*self.checkpoint).clone();
            self.wave_sent = self.checkpoint_wave;
            if !self.wave_sent { self.wave_started_tick = None; }
            self.step_seq = world.event_seq;
            self.run_requested = false;
            self.status = "active";
            self.feedback = "checkpoint_restored";
            self.retries += 1;
            return Ok(());
        }
        if self.status != "active" { return Err("not_active"); }
        let step = self.current().ok_or("no_objective")?;
        let satisfied = match (action.action.as_str(), step.objective.as_str()) {
            ("ack", "ack") => true,
            ("ack", "report") => world.events_for(Side::Blue, self.start_seq).iter()
                .any(|e| !matches!(e.kind, EventKind::ScenarioStarted { .. })),
            ("select_unit", "select_unit") => action.unit == Some(self.unit.0)
                && world.entity(self.unit).is_some_and(|e| e.alive && e.side == Side::Blue),
            ("select_contact", "select_contact") => observe(world, Side::Blue, 0, 0).contacts.iter()
                .any(|c| Some(c.id.0) == action.contact && !c.stale && c.category == PlatformKind::Ship),
            ("answer", "question") => {
                if action.answer != step.answer {
                    self.mistakes += 1;
                    self.feedback = "incorrect_answer";
                    return Ok(());
                }
                true
            }
            _ => false,
        };
        if !satisfied { return Err("objective_requires_evidence"); }
        self.advance(world, "learner_action");
        Ok(())
    }
    pub fn time_action(&mut self, world: &World, speed: f64) -> bool {
        if self.status != "active" { return false; }
        let running = self.current().is_some_and(|s| s.objective == "running");
        let paused = self.current().is_some_and(|s| s.objective == "paused");
        if running && speed > 0.0 { self.run_requested = true; }
        if paused && speed == 0.0 {
            self.advance(world, "time_control");
            return true;
        }
        false
    }
    /// The training opponent uses only its own observation and normal commands.
    pub fn opponent_commands(&mut self, world: &World) -> Vec<(Side, Command)> {
        if self.status != "active" || self.course.id != "defence" || self.wave_sent
            || !self.current().is_some_and(|s| s.objective == "incoming") { return Vec::new(); }
        let obs = observe(world, Side::Red, 0, 0);
        let shooter = obs.units.iter().find(|u| u.class == "mgb");
        let target = obs.contacts.iter().find(|c| c.category == PlatformKind::Ship && !c.stale && c.quality >= 0.3);
        if let (Some(unit), Some(contact)) = (shooter, target) {
            self.wave_sent = true;
            self.wave_started_tick = Some(world.tick);
            return vec![(Side::Red, Command::Fire {
                unit: unit.id, contact: contact.id, weapon: "ascm_std".into(), rounds: 1,
            })];
        }
        Vec::new()
    }
    /// Returns true only when the simulation should stop at this exact tick.
    pub fn after_tick(&mut self, world: &World, commands: &[(Side, Command)]) -> bool {
        if self.status != "active" { return false; }
        let Some(unit) = world.entity(self.unit).filter(|e| e.alive) else {
            self.status = "failed"; self.feedback = "training_unit_lost"; return true;
        };
        if world.over { self.status = "failed"; self.feedback = "scenario_ended"; return true; }
        let Some(step) = self.current() else { return false; };
        let events = world.events_for(Side::Blue, self.step_seq);
        let mine: Vec<&Command> = commands.iter().filter(|(side, _)| *side == Side::Blue).map(|(_, cmd)| cmd).collect();
        let target = step.target.unwrap_or(0.0);
        let done = match step.objective.as_str() {
            "running" => self.run_requested && world.tick > self.checkpoint.tick,
            "speed" => mine.iter().any(|c| matches!(c, Command::SetSpeed { unit: id, speed_kts } if *id == self.unit && (*speed_kts-target).abs()<0.1))
                && (unit.ordered_speed_kts-target).abs()<0.1,
            "course" => mine.iter().any(|c| matches!(c, Command::SetCourse { unit: id, course_deg } if *id == self.unit && (*course_deg-target).abs()<0.1))
                && (unit.ordered_course_deg-target).abs()<0.1,
            "route" => step.point.is_some_and(|p| {
                let goal = harpoon_core::Vec2::new(p[0], p[1]);
                mine.iter().any(|c| matches!(c, Command::Goto { unit: id, waypoints } if *id == self.unit
                    && waypoints.last().is_some_and(|point| point.range_to(goal) < 0.05)))
                    && unit.waypoints.back().is_some_and(|point| point.range_to(goal) < 0.05)
                    && (unit.ordered_speed_kts-target).abs() < 0.1
            }),
            "arrive" => step.point.is_some_and(|p| unit.pos.range_to(harpoon_core::Vec2::new(p[0],p[1])) <= 0.25),
            "stop" => mine.iter().any(|c| matches!(c, Command::Stop { unit: id } if *id == self.unit))
                && unit.speed_kts == 0.0 && unit.waypoints.is_empty(),
            "sonar_on" | "sonar_off" => {
                let active = step.objective == "sonar_on";
                mine.iter().any(|c| matches!(c, Command::SetSonar { unit: id, active: a } if *id == self.unit && *a == active))
                    && unit.sensors.iter().any(|s| s.sensor.starts_with("sonar_a") && s.active == active)
            }
            "depth" => (unit.depth_m-target).abs() <= 1.0 && (unit.ordered_depth_m-target).abs() < 0.1,
            "contact" => observe(world, Side::Blue, 0, 0).contacts.iter().any(|c| c.category == PlatformKind::Ship && !c.stale),
            "identified_contact" => observe(world, Side::Blue, 0, 0).contacts.iter()
                .any(|c| c.category == PlatformKind::Ship && !c.stale && c.class_name.is_some()),
            "gun_fire" => !unit.fire_on && unit.assigned.is_none()
                && mine.iter().any(|c| matches!(c, Command::Fire { unit: id, weapon, rounds, .. } if *id == self.unit && weapon == "gun_5in" && *rounds > 0))
                && events.iter().any(|e| matches!(e.kind, EventKind::GunAction { shooter, .. } if shooter == self.unit)),
            "incoming" => events.iter().any(|e| matches!(e.kind, EventKind::Incoming { side: Side::Blue, .. })),
            "chaff" => !unit.auto_chaff && mine.iter().any(|c| matches!(c, Command::DeployChaff { unit: id } if *id == self.unit))
                && events.iter().any(|e| matches!(e.kind, EventKind::ChaffDeployed { unit, side: Side::Blue } if unit == self.unit)),
            "launch" => mine.iter().any(|c| matches!(c, Command::LaunchAircraft { unit: id, class, .. } if *id == self.unit && class == "helo_asw"))
                && events.iter().any(|e| matches!(e.kind, EventKind::AirLaunch { carrier, .. } if carrier == self.unit)),
            "recall" => mine.iter().any(|c| matches!(c, Command::RecoverAircraft { unit: id } if *id == self.unit))
                && (world.entities.values().any(|e| e.side == Side::Blue && matches!(e.mission, Some(harpoon_core::world::MissionState::Rtb)))
                    || events.iter().any(|e| matches!(e.kind, EventKind::AirRecovered { carrier, .. } if carrier == self.unit))),
            "recovered" => self.manual_recall && world.events_for(Side::Blue, self.start_seq).iter()
                .any(|e| matches!(e.kind, EventKind::AirRecovered { carrier, .. } if carrier == self.unit)),
            _ => false,
        };
        if done {
            let pause = step.objective != "running";
            if step.objective == "recall" { self.manual_recall = true; }
            self.advance(world, "engine_evidence");
            pause
        } else if step.objective == "incoming" && self.wave_started_tick.is_some_and(|t| world.tick - t > 180) {
            self.status = "failed"; self.feedback = "training_threat_missing"; true
        } else { false }
    }
    pub fn view(&self, world: &World) -> Value {
        json!({
            "attempt": self.attempt, "course_id": self.course.id, "step": self.step,
            "step_count": self.course.steps.len(), "status": self.status, "feedback": self.feedback,
            "unit": self.unit, "mistakes": self.mistakes, "retries": self.retries,
            "elapsed_s": world.tick-self.started_tick, "checkpoint_tick": self.checkpoint.tick,
            "evidence": self.evidence, "profile": "rts_adapted",
        })
    }
}

pub fn load_progress() -> BTreeSet<String> {
    std::fs::read_to_string(progress_path()).ok().and_then(|s| {
        let value: Value = serde_json::from_str(&s).ok()?;
        if value.is_array() { return serde_json::from_value(value).ok(); }
        if value.get("version")?.as_u64()? != 1 { return None; }
        serde_json::from_value(value.get("completed")?.clone()).ok()
    }).unwrap_or_default()
}
fn progress_path() -> std::path::PathBuf {
    std::env::var("HARPOON_TRAINING_PROGRESS").unwrap_or_else(|_| "tmp/training-progress.json".into()).into()
}
pub fn save_progress(completed: &BTreeSet<String>) -> std::io::Result<()> {
    let path = progress_path();
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(&json!({"version": 1, "completed": completed}))?)?;
    // Same-directory replacement leaves either the previous or complete new record.
    std::fs::rename(&temporary, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use harpoon_core::{Database, Vec2};
    fn setup(id: &str) -> (World, Session) {
        let course = catalog().courses.into_iter().find(|c| c.id == id).unwrap();
        let source = std::fs::read_to_string(format!("../../scenarios/{}.json", course.scenario)).unwrap();
        let mut world = harpoon_core::load_scenario(harpoon_core::scenario_from_json(&source).unwrap(), Database::built_in()).unwrap();
        let session = Session::new(course, 42, &mut world).unwrap();
        (world, session)
    }
    fn action(s: &Session, name: &str) -> Action {
        Action { action: name.into(), attempt: s.attempt, step: s.step, unit: Some(s.unit.0), contact: None, answer: None }
    }
    #[test]
    fn curriculum_is_bilingual_and_does_not_publish_answers() {
        let cat = catalog();
        assert_eq!(cat.courses.len(), 8);
        assert_eq!(cat.courses.iter().map(|c| c.steps.len()).sum::<usize>(), 55);
        let public = serde_json::to_value(&cat).unwrap();
        for course in &cat.courses {
            for step in &course.steps {
                for field in ["title", "body", "why"] {
                    for lang in ["zh", "en"] { assert!(!step.content[field][lang].as_str().unwrap().is_empty()); }
                }
            }
        }
        assert!(!public.to_string().contains("\"answer\":"));
    }
    #[test]
    fn next_cannot_skip_a_tactical_objective() {
        let (mut world, mut session) = setup("navigation");
        session.step = 2;
        let a = action(&session, "ack");
        assert_eq!(session.action(&mut world, &a), Err("objective_requires_evidence"));
        assert_eq!(session.step, 2);
    }
    #[test]
    fn stale_browser_actions_cannot_advance_new_steps() {
        let (mut world, mut session) = setup("bridge");
        let a = action(&session, "ack");
        session.action(&mut world, &a).unwrap();
        assert_eq!(session.action(&mut world, &a), Err("stale_attempt"));
    }
    #[test]
    fn wrong_unit_selection_does_not_pass() {
        let (mut world, mut session) = setup("sensors");
        session.step = 1;
        let mut a = action(&session, "select_unit"); a.unit = Some(99999);
        assert!(session.action(&mut world, &a).is_err());
    }
    #[test]
    fn queued_speed_requires_actual_tick_and_correct_unit() {
        let (mut world, mut session) = setup("navigation"); session.step = 2;
        assert!(!session.after_tick(&world, &[]));
        let commands = vec![(Side::Blue, Command::SetSpeed { unit: session.unit, speed_kts: 12.0 })];
        harpoon_core::sim::tick(&mut world, &commands);
        assert!(session.after_tick(&world, &commands)); assert_eq!(session.step, 3);
    }
    #[test]
    fn checkpoint_restores_world_rng_ammo_and_time() {
        let (mut world, mut session) = setup("navigation");
        session.action(&mut world, &action(&session, "ack")).unwrap();
        let saved = serde_json::to_string(&world).unwrap();
        for _ in 0..20 { harpoon_core::sim::tick(&mut world, &[]); }
        session.action(&mut world, &action(&session, "retry")).unwrap();
        assert_eq!(serde_json::to_string(&world).unwrap(), saved);
    }
    #[test]
    fn old_and_automatic_gun_events_do_not_pass_manual_task() {
        let (mut world, mut session) = setup("gunnery"); session.step = 4;
        world.emit(EventKind::GunAction { side: Side::Blue, shooter: session.unit, shooter_name: "Training".into(), target_contact: harpoon_core::ContactId(1), hits: 0 }, harpoon_core::Visibility::Side(Side::Blue));
        assert!(!session.after_tick(&world, &[]));
        let wrong = vec![(Side::Red, Command::Fire { unit: session.unit, contact: harpoon_core::ContactId(1), weapon: "gun_5in".into(), rounds: 1 })];
        assert!(!session.after_tick(&world, &wrong));
    }
    #[test]
    fn progress_view_does_not_contain_checkpoint_world_or_enemy_truth() {
        let (world, session) = setup("defence");
        let view = session.view(&world).to_string();
        assert!(!view.contains("TRAINING OPPONENT")); assert!(!view.contains("entities"));
        assert!(!view.contains("checkpoint_world"));
    }
    #[test]
    fn arrival_requires_actual_position() {
        let (mut world, mut session) = setup("navigation"); session.step = 5;
        world.entity_mut(session.unit).unwrap().waypoints.push_back(Vec2::new(1.0,0.0));
        assert!(!session.after_tick(&world, &[]));
        world.entity_mut(session.unit).unwrap().pos = Vec2::new(1.0,0.0);
        assert!(session.after_tick(&world, &[]));
    }
    #[test]
    fn first_low_quality_detection_does_not_finish_gunnery_preparation() {
        let (mut world, mut session) = setup("gunnery"); session.step = 2;
        for _ in 0..4 { harpoon_core::sim::tick(&mut world, &[]); }
        assert!(!session.after_tick(&world, &[]));
        assert_eq!(session.step, 2);
        for _ in 0..4 { harpoon_core::sim::tick(&mut world, &[]); }
        assert!(session.after_tick(&world, &[]));
    }
    #[test]
    fn running_requires_the_clock_to_actually_advance() {
        let (mut world, mut session) = setup("bridge"); session.step = 2;
        assert!(!session.time_action(&world, 15.0));
        assert!(!session.after_tick(&world, &[]));
        harpoon_core::sim::tick(&mut world, &[]);
        assert!(!session.after_tick(&world, &[])); // keep running for the pause exercise
        assert_eq!(session.step, 3);
        assert!(session.time_action(&world, 0.0));
        assert_eq!(session.step, 4);
    }
    #[test]
    fn automatic_fire_cannot_substitute_for_manual_gunnery() {
        let (mut world, mut session) = setup("gunnery"); session.step = 4;
        world.entity_mut(session.unit).unwrap().fire_on = true;
        world.emit(EventKind::GunAction { side: Side::Blue, shooter: session.unit, shooter_name: "Training".into(), target_contact: harpoon_core::ContactId(1), hits: 0 }, harpoon_core::Visibility::Side(Side::Blue));
        let commands = vec![(Side::Blue, Command::Fire { unit: session.unit, contact: harpoon_core::ContactId(1), weapon: "gun_5in".into(), rounds: 1 })];
        assert!(!session.after_tick(&world, &commands));
        assert_eq!(session.step, 4);
    }
    #[test]
    fn route_must_use_the_requested_training_point_and_speed() {
        let (mut world, mut session) = setup("navigation"); session.step = 4;
        world.entity_mut(session.unit).unwrap().ordered_speed_kts = 12.0;
        let wrong = vec![(Side::Blue, Command::Goto { unit: session.unit, waypoints: vec![Vec2::new(9.0,9.0)] })];
        harpoon_core::sim::tick(&mut world, &wrong);
        assert!(!session.after_tick(&world, &wrong));
        let right = vec![(Side::Blue, Command::Goto { unit: session.unit, waypoints: vec![Vec2::new(1.0,0.0)] })];
        harpoon_core::sim::tick(&mut world, &right);
        assert!(session.after_tick(&world, &right));
    }
}

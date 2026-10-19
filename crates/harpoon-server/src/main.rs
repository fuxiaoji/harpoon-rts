//! harpoon-server: authoritative referee service.
//!
//! - The simulation loop runs server-side at 10 Hz wall clock, advancing
//!   `speed` simulated seconds per real second (0 = pause, up to 512x).
//! - Every client (web UI, LLM staff, MCP tool, RL env) talks to the same
//!   REST/WS surface and only ever receives double-blind observations.
//! - POST /api/selfplay runs a fully deterministic headless rollout.

mod ai;
mod agent;
mod director;
mod tutorial;
mod doctrine;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path as AxPath, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use harpoon_core::commands::Command;
use harpoon_core::db::Database;
use harpoon_core::observe::{observe, Observation};
use harpoon_core::scenario::{load_scenario, scenario_from_json, Scenario};
use harpoon_core::types::Side;
use harpoon_core::world::World;
use serde::Deserialize;
use ai::CommandStaff;
use serde_json::json;
use std::collections::HashMap;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

#[derive(Clone)]
struct AppState {
    game: Arc<Mutex<Game>>,
    scenario_dir: Arc<PathBuf>,
    update_tx: Arc<tokio::sync::broadcast::Sender<()>>,
    /// Built-in database merged with database/volumes/*.json at startup.
    db: Arc<Database>,
    /// In-game AI staff state (player-view observations + memory).
    agents: Arc<agent::Runtime>,
    /// Two-sided adversarial director status and id allocator.
    director: Arc<Mutex<director::State>>,
    director_seq: Arc<AtomicU64>,
}

/// Merge every database/volumes/*.json (underscore-prefixed files skipped)
/// over the built-in database.
fn load_volume_database(volumes_dir: &std::path::Path) -> Database {
    let mut db = Database::built_in();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(volumes_dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().map(|x| x == "json").unwrap_or(false)
                        && !p
                            .file_name()
                            .map(|n| n.to_string_lossy().starts_with('_'))
                            .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    for f in files {
        match std::fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|t| Database::from_json(&t).map_err(|e| e.to_string())) {
            Ok(vol) => {
                let n = vol.platforms.len();
                db = db.merged(&vol);
                eprintln!("db volume: {} (+{} platforms)", f.display(), n);
            }
            Err(e) => eprintln!("db volume {f:?} FAILED to load: {e}"),
        }
    }
    db
}

struct Game {
    world: Option<World>,
    speed: f64,
    accumulator: f64,
    inbox: Vec<(Side, Command)>,
    training: Option<tutorial::Session>,
    training_completed: BTreeSet<String>,
    progress_saved: bool,
    /// A two-sided director owns the clock; the normal run loop must not tick.
    director_active: bool,
    director_id: u64,
    /// Fixed scheduled planning hold (public 5-minute window), not a private wake.
    agent_hold: bool,
    resume_speed: f64,
    planning_outstanding: u32,
}

impl Game {
    fn new() -> Self {
        Self { world: None, speed: 0.0, accumulator: 0.0, inbox: Vec::new(),
            training: None, training_completed: tutorial::load_progress(), progress_saved: true,
            director_active: false, director_id: 0,
            agent_hold: false, resume_speed: 0.0, planning_outstanding: 0 }
    }
    fn training_view(&self) -> serde_json::Value {
        json!({"session": self.training.as_ref().zip(self.world.as_ref()).map(|(s,w)| s.view(w)),
            "completed": self.training_completed, "progress_saved": self.progress_saved})
    }
    fn record_training(&mut self) {
        if let Some(session) = &self.training {
            if session.status == "completed" && self.training_completed.insert(session.course.id.clone()) {
                self.progress_saved = tutorial::save_progress(&self.training_completed).is_ok();
            }
        }
    }
}

const MAX_TICKS_PER_STEP: i64 = 512;

fn run_loop(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            interval.tick().await;
            // Scope 1: decide how many ticks to run (MutexGuard borrows are
            // not field-split through Deref, so keep sections minimal).
            let work: Option<(i64, Vec<(Side, Command)>)> = {
                let mut g = state.game.lock().unwrap();
                let over = g.world.as_ref().map(|w| w.over).unwrap_or(true);
                if !over && g.training.is_none() && g.speed <= 0.0 && !g.director_active && !g.agent_hold {
                    if let Some(speed) = state.agents.auto_run_speed() {
                        g.speed = speed;
                        g.accumulator = 0.0;
                    }
                }
                if g.speed > 0.0 && !over && !g.director_active && !g.agent_hold {
                    g.accumulator += g.speed * 0.1; // 100 ms wall step
                    let n = g.accumulator.floor() as i64;
                    g.accumulator -= n as f64;
                    let n = n.min(MAX_TICKS_PER_STEP);
                    if n > 0 {
                        Some((n, std::mem::take(&mut g.inbox)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            let mut poked = false;
            if let Some((n, cmds)) = work {
                let mut g = state.game.lock().unwrap();
                let mut pause_training = false;
                let Game { world, training, .. } = &mut *g;
                if let Some(world) = world.as_mut() {
                    // Commands apply once on the first tick (orders persist
                    // as entity state afterwards).
                    let empty: Vec<(Side, Command)> = Vec::new();
                    for i in 0..n {
                        let mut c = if i == 0 { cmds.clone() } else { empty.clone() };
                        if let Some(session) = training.as_mut() { c.extend(session.opponent_commands(world)); }
                        if training.is_none() {
                            if let Some(side) = world.context.automated_side {
                                let obs = observe(world, side, world.event_seq, 0);
                                c.extend(doctrine::orders(&obs));
                            }
                        }
                        harpoon_core::sim::tick(world, &c);
                        if training.as_mut().is_some_and(|session| session.after_tick(world, &c)) {
                            pause_training = true;
                            break;
                        }
                        if world.over {
                            break;
                        }
                    }
                    poked = true;
                }
                if pause_training { g.speed = 0.0; g.accumulator = 0.0; }
                g.record_training();
            }
            if poked {
                let _ = state.update_tx.send(());
                let due = {
                    let mut g = state.game.lock().unwrap();
                    if g.training.is_some() || g.director_active {
                        Vec::new()
                    } else if let Some(world) = g.world.as_ref() {
                        if world.over {
                            Vec::new()
                        } else {
                            let due = state.agents.due_sides(world.tick);
                            let hold_sides = due
                                .iter()
                                .filter(|side| state.agents.is_execute(**side))
                                .count();
                            if hold_sides > 0 {
                                g.resume_speed = g.speed;
                                g.speed = 0.0;
                                g.accumulator = 0.0;
                                g.agent_hold = true;
                                g.planning_outstanding = hold_sides as u32;
                            }
                            due
                        }
                    } else {
                        Vec::new()
                    }
                };
                for side in due {
                    let state2 = state.clone();
                    let state3 = state.clone();
                    let counts_for_hold = state.agents.is_execute(side);
                    tokio::spawn(async move {
                        let _ = run_agent_turn(state3, side, "scheduled", None).await;
                        if counts_for_hold {
                            scheduled_turn_done(&state2);
                        }
                    });
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct StateQuery {
    side: String,
    #[serde(default)]
    since: u64,
    #[serde(default = "default_max_events")]
    max_events: usize,
}

fn default_max_events() -> usize {
    200
}

async fn health() -> impl IntoResponse {
    Json(json!({"ok": true, "service": "harpoon-server"}))
}

async fn list_scenarios(State(state): State<AppState>) -> impl IntoResponse {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(state.scenario_dir.as_ref()) {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().map(|x| x == "json").unwrap_or(false) {
                if let Ok(txt) = std::fs::read_to_string(&path) {
                    if let Ok(sc) = scenario_from_json(&txt) {
                        out.push(json!({
                            "id": sc.id, "name": sc.name,
                            "description": sc.description,
                            "file": path.file_name().unwrap().to_string_lossy(),
                        }));
                    }
                }
            }
        }
    }
    Json(out)
}

#[derive(Deserialize)]
struct NewGame {
    scenario: String,
    seed: Option<u64>,
    #[serde(default = "default_speed")]
    speed: f64,
}

fn default_speed() -> f64 {
    60.0
}

async fn new_game(
    State(state): State<AppState>,
    Json(req): Json<NewGame>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = state
        .scenario_dir
        .join(format!("{}.json", req.scenario))
        .canonicalize()
        .map_err(|_| (StatusCode::NOT_FOUND, format!("scenario {} not found", req.scenario)))?;
    // Constrain to the scenarios directory.
    if !path.starts_with(state.scenario_dir.canonicalize().unwrap_or_default()) {
        return Err((StatusCode::FORBIDDEN, "path traversal".into()));
    }
    let txt = std::fs::read_to_string(&path).map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    let mut sc: Scenario =
        scenario_from_json(&txt).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if let Some(seed) = req.seed {
        sc.seed = seed;
    }
    let world = load_scenario(sc, (*state.db).clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let tick = world.tick;
    let name = world.scenario_name.clone();
    let scenario_id = world.scenario_id.clone();
    let seed = world.seed;
    let mut g = state.game.lock().unwrap();
    let completed = g.training_completed.clone();
    *g = Game { world: Some(world), speed: req.speed, accumulator: 0.0, inbox: Vec::new(),
        training: None, training_completed: completed, progress_saved: true,
        director_active: false, director_id: 0,
        agent_hold: false, resume_speed: 0.0, planning_outstanding: 0 };
    drop(g);
    state.agents.start_game(&scenario_id, seed, tick);
    Ok(Json(json!({"started": true, "scenario": name, "seeded_at_tick": tick})))
}

async fn state_handler(
    State(state): State<AppState>,
    Query(q): Query<StateQuery>,
) -> Result<Json<Observation>, (StatusCode, String)> {
    let side = Side::parse(&q.side).ok_or((StatusCode::BAD_REQUEST, "side must be Blue|Red".into()))?;
    let g = state.game.lock().unwrap();
    let Some(world) = g.world.as_ref() else {
        return Err((StatusCode::CONFLICT, "no game running; POST /api/game first".into()));
    };
    Ok(Json(observe(world, side, q.since, q.max_events)))
}

#[derive(Deserialize)]
struct CommandReq {
    side: String,
    command: Command,
}

async fn command_handler(
    State(state): State<AppState>,
    Json(req): Json<CommandReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let side = Side::parse(&req.side).ok_or((StatusCode::BAD_REQUEST, "side must be Blue|Red".into()))?;
    let mut g = state.game.lock().unwrap();
    if g.world.is_none() {
        return Err((StatusCode::CONFLICT, "no game running".into()));
    }
    g.inbox.push((side, req.command));
    Ok(Json(json!({"queued": true, "pending": g.inbox.len()})))
}

#[derive(Deserialize)]
struct TimeReq {
    speed: f64,
}

async fn time_handler(State(state): State<AppState>, Json(req): Json<TimeReq>) -> Json<serde_json::Value> {
    let mut g = state.game.lock().unwrap();
    g.speed = req.speed.clamp(0.0, 512.0);
    let Game { world, training, .. } = &mut *g;
    if let (Some(world), Some(session)) = (world.as_ref(), training.as_mut()) {
        session.time_action(world, req.speed);
    }
    g.record_training();
    g.accumulator = 0.0;
    Json(json!({"speed": g.speed}))
}

async fn time_get(State(state): State<AppState>) -> Json<serde_json::Value> {
    let g = state.game.lock().unwrap();
    let (over, tick) = g
        .world
        .as_ref()
        .map(|w| (w.over, w.tick))
        .unwrap_or((false, 0));
    Json(json!({"speed": g.speed, "over": over, "tick": tick}))
}

// ---------------------------------------------------------------------------
// In-game AI staff endpoints
// ---------------------------------------------------------------------------

fn agent_error(err: agent::TurnError) -> (StatusCode, String) {
    let status = match err {
        agent::TurnError::Busy => StatusCode::CONFLICT,
        agent::TurnError::NoGame => StatusCode::CONFLICT,
        agent::TurnError::NotConfigured => StatusCode::BAD_REQUEST,
    };
    (status, err.message().to_string())
}

fn scheduled_turn_done(state: &AppState) {
    let mut g = state.game.lock().unwrap();
    if g.planning_outstanding > 0 {
        g.planning_outstanding -= 1;
    }
    if g.planning_outstanding == 0 && g.agent_hold {
        g.agent_hold = false;
        let over = g.world.as_ref().map(|w| w.over).unwrap_or(true);
        g.speed = if over {
            0.0
        } else {
            state.agents.auto_run_speed().unwrap_or(g.resume_speed).max(0.0)
        };
        g.accumulator = 0.0;
    }
}

/// One player-view staff turn. The LLM sees only `observe(world, side)` plus
/// public scenario/database facts. Valid commands enter the ordinary inbox,
/// and only when the side is in `execute` autonomy.
async fn run_agent_turn(
    state: AppState,
    side: Side,
    trigger: &'static str,
    message: Option<String>,
) -> Result<agent::TurnReport, (StatusCode, String)> {
    let tick = {
        let g = state.game.lock().unwrap();
        g.world
            .as_ref()
            .map(|w| w.tick)
            .ok_or((StatusCode::CONFLICT, "no game running".to_string()))?
    };
    let ctx = state
        .agents
        .prepare_turn(side, trigger, tick)
        .map_err(agent_error)?;
    let obs = {
        let g = state.game.lock().unwrap();
        let Some(world) = g.world.as_ref() else {
            state
                .agents
                .abort_turn(&ctx, "no game running".to_string());
            return Err((StatusCode::CONFLICT, "no game running".to_string()));
        };
        observe(world, side, ctx.memory.last_event_seq, 120)
    };
    let last_event_seq = obs
        .events
        .iter()
        .map(|e| e.seq)
        .max()
        .unwrap_or(ctx.memory.last_event_seq);
    let db = state.db.clone();
    let ctx_for_llm = ctx.clone();
    let obs_for_llm = obs.clone();
    let request_message = message.clone();
    let decision = match tokio::task::spawn_blocking(move || {
        agent::execute_turn(&ctx_for_llm, &obs_for_llm, &db, request_message.as_deref())
    })
    .await
    {
        Ok(decision) => decision,
        Err(e) => {
            let report = state
                .agents
                .abort_turn(&ctx, format!("agent task failed: {e}"));
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                report.error.unwrap_or_default(),
            ));
        }
    };

    let mut applied = false;
    let same_generation = state.agents.generation() == ctx.generation;
    if decision.error.is_none()
        && same_generation
        && ctx.autonomy == "execute"
        && !decision.commands.is_empty()
    {
        let mut g = state.game.lock().unwrap();
        let same_world = g.world.as_ref().is_some_and(|world| {
            world.scenario_id == ctx.scenario_id && world.seed == ctx.seed && !world.over
        });
        if same_world && g.training.is_none() {
            for command in &decision.commands {
                g.inbox.push((side, command.clone()));
            }
            applied = true;
        }
    }
    let report = state
        .agents
        .finish_turn(&ctx, decision, applied, last_event_seq);
    if let Some(error) = &report.error {
        return Err((StatusCode::BAD_GATEWAY, error.clone()));
    }
    Ok(report)
}

async fn agent_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(state.agents.status())
}

async fn agent_side_status(
    State(state): State<AppState>,
    AxPath(side): AxPath<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let side = Side::parse(&side).ok_or((StatusCode::BAD_REQUEST, "side must be Blue|Red".into()))?;
    Ok(Json(state.agents.side_status(side)))
}

async fn agent_config(
    State(state): State<AppState>,
    Json(req): Json<agent::AgentConfigReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let tick = state
        .game
        .lock()
        .unwrap()
        .world
        .as_ref()
        .map(|w| w.tick)
        .unwrap_or(0);
    let status = state
        .agents
        .set_config(&req, tick)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(status))
}

async fn agent_reset(
    State(state): State<AppState>,
    Json(req): Json<agent::AgentResetReq>,
) -> Json<serde_json::Value> {
    let tick = state
        .game
        .lock()
        .unwrap()
        .world
        .as_ref()
        .map(|w| w.tick)
        .unwrap_or(0);
    Json(state.agents.reset(req.purge_memory, tick))
}

async fn agent_report(
    State(state): State<AppState>,
    AxPath(side): AxPath<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let side = Side::parse(&side).ok_or((StatusCode::BAD_REQUEST, "side must be Blue|Red".into()))?;
    let report = run_agent_turn(state.clone(), side, "manual", None).await?;
    Ok(Json(json!({"report": report, "status": state.agents.status()})))
}

async fn agent_chat(
    State(state): State<AppState>,
    AxPath(side): AxPath<String>,
    Json(req): Json<agent::ChatReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let side = Side::parse(&side).ok_or((StatusCode::BAD_REQUEST, "side must be Blue|Red".into()))?;
    let message = req.message.trim().to_string();
    if message.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "message must not be empty".into()));
    }
    if message.chars().count() > 4000 {
        return Err((StatusCode::BAD_REQUEST, "message is too long (max 4000 chars)".into()));
    }
    let report = run_agent_turn(state.clone(), side, "chat", Some(message)).await?;
    Ok(Json(json!({
        "reply": report.report,
        "intent": report.intent,
        "commands": report.commands,
        "rejected": report.rejected,
        "applied": report.applied,
        "tick": report.tick,
        "model": report.model,
        "latency_ms": report.latency_ms,
        "status": state.agents.status(),
    })))
}

async fn agent_execute(
    State(state): State<AppState>,
    AxPath(side): AxPath<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let side = Side::parse(&side).ok_or((StatusCode::BAD_REQUEST, "side must be Blue|Red".into()))?;
    let proposals = state.agents.last_proposals(side);
    if proposals.is_empty() {
        return Ok(Json(json!({"queued": 0, "rejected": []})));
    }
    let mut g = state.game.lock().unwrap();
    if g.training.is_some() {
        return Err((StatusCode::CONFLICT, "training session is active".into()));
    }
    let obs = {
        let world = g
            .world
            .as_ref()
            .ok_or((StatusCode::CONFLICT, "no game running".to_string()))?;
        if world.over {
            return Err((StatusCode::CONFLICT, "scenario is over".into()));
        }
        observe(world, side, 0, 0)
    };
    let (valid, rejected) = agent::validate_commands(&obs, side, &proposals);
    for command in &valid {
        g.inbox.push((side, command.clone()));
    }
    Ok(Json(json!({"queued": valid.len(), "rejected": rejected})))
}

async fn agent_duel_start(
    State(state): State<AppState>,
    Json(req): Json<agent::DuelReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    agent::start_duel(
        state.agents.clone(),
        state.db.clone(),
        (*state.scenario_dir).clone(),
        req,
    )
    .await
    .map(Json)
    .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn agent_duel_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(state.agents.duel_status())
}

async fn training_catalog() -> Json<tutorial::Catalog> { Json(tutorial::catalog()) }
async fn training_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(state.game.lock().unwrap().training_view())
}
#[derive(Deserialize)]
struct TrainingStart { course: String }
static TRAINING_ATTEMPT: AtomicU64 = AtomicU64::new(1);
async fn training_start(State(state): State<AppState>, Json(req): Json<TrainingStart>)
    -> Result<Json<serde_json::Value>, (StatusCode, String)>
{
    let course = tutorial::catalog().courses.into_iter().find(|c| c.id == req.course)
        .ok_or((StatusCode::NOT_FOUND, "unknown_course".into()))?;
    let source = std::fs::read_to_string(state.scenario_dir.join(format!("{}.json", course.scenario)))
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    let scenario = scenario_from_json(&source).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let mut world = load_scenario(scenario, Database::built_in()).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let tutorial_scenario = world.scenario_id.clone();
    let tutorial_seed = world.seed;
    let tutorial_tick = world.tick;
    let session = tutorial::Session::new(course, TRAINING_ATTEMPT.fetch_add(1, Ordering::Relaxed), &mut world)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let mut g = state.game.lock().unwrap();
    g.world = Some(world); g.training = Some(session); g.speed = 0.0; g.accumulator = 0.0; g.inbox.clear();
    g.director_active = false; g.director_id = 0;
    g.agent_hold = false; g.resume_speed = 0.0; g.planning_outstanding = 0;
    drop(g);
    state.agents.start_game(&tutorial_scenario, tutorial_seed, tutorial_tick);
    let _ = state.update_tx.send(());
    Ok(Json(state.game.lock().unwrap().training_view()))
}
async fn training_action(State(state): State<AppState>, Json(req): Json<tutorial::Action>)
    -> Result<Json<serde_json::Value>, (StatusCode, String)>
{
    let mut g = state.game.lock().unwrap();
    let mut session = g.training.take().ok_or((StatusCode::CONFLICT, "no_training_session".into()))?;
    if let Err(reason) = session.validate_action(&req) {
        g.training = Some(session); return Err((StatusCode::CONFLICT, reason.into()));
    }
    if req.action == "exit" {
        g.speed = 0.0; g.accumulator = 0.0; g.inbox.clear();
    } else if req.action == "step" {
        if session.status != "active" {
            g.training = Some(session);
            return Err((StatusCode::CONFLICT, "not_active".into()));
        }
        let mut commands = std::mem::take(&mut g.inbox);
        if let Some(world) = g.world.as_mut() {
            commands.extend(session.opponent_commands(world));
            harpoon_core::sim::tick(world, &commands);
            session.after_tick(world, &commands);
        }
        g.speed = 0.0; g.accumulator = 0.0; g.training = Some(session);
    } else {
        let result = session.action(g.world.as_mut().unwrap(), &req);
        g.training = Some(session);
        if let Err(reason) = result { return Err((StatusCode::CONFLICT, reason.into())); }
        g.speed = 0.0; g.accumulator = 0.0;
        if req.action == "retry" { g.inbox.clear(); }
    }
    g.record_training();
    let _ = state.update_tx.send(());
    Ok(Json(g.training_view()))
}


// ---------------------------------------------------------------------------
// Two-sided adversarial director
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct DirectorReq {
    scenario: String,
    seed: Option<u64>,
    max_ticks: Option<i64>,
}

fn director_is_active(state: &AppState, id: u64) -> bool {
    let g = state.game.lock().unwrap();
    g.director_active
        && g.director_id == id
        && g.world.as_ref().is_some_and(|world| !world.over)
}

fn update_director(state: &AppState, id: u64, f: impl FnOnce(&mut director::State)) {
    let mut st = state.director.lock().unwrap();
    if st.id == id {
        f(&mut st);
    }
}

async fn director_start(
    State(state): State<AppState>,
    Json(req): Json<DirectorReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = state
        .scenario_dir
        .join(format!("{}.json", req.scenario))
        .canonicalize()
        .map_err(|_| (StatusCode::NOT_FOUND, format!("scenario {} not found", req.scenario)))?;
    if !path.starts_with(state.scenario_dir.canonicalize().unwrap_or_default()) {
        return Err((StatusCode::FORBIDDEN, "path traversal".into()));
    }
    let txt = std::fs::read_to_string(&path).map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    let mut sc = scenario_from_json(&txt).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if let Some(seed) = req.seed {
        sc.seed = seed;
    }
    if !state.agents.status()["configured"].as_bool().unwrap_or(false) {
        return Err((StatusCode::BAD_REQUEST, "LLM API key is not configured".into()));
    }
    let world = load_scenario(sc, (*state.db).clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let scenario_id = world.scenario_id.clone();
    let seed = world.seed;
    let duration = world.duration_s;
    let tick = world.tick;
    let id = state.director_seq.fetch_add(1, Ordering::Relaxed) + 1;
    let max_ticks = req.max_ticks.unwrap_or(duration).clamp(1, duration.max(1));

    {
        let mut g = state.game.lock().unwrap();
        g.director_active = false;
        g.director_id = 0;
        let completed = g.training_completed.clone();
        *g = Game {
            world: Some(world),
            speed: 0.0,
            accumulator: 0.0,
            inbox: Vec::new(),
            training: None,
            training_completed: completed,
            progress_saved: true,
            director_active: true,
            director_id: id,
            agent_hold: false,
            resume_speed: 0.0,
            planning_outstanding: 0,
        };
    }
    state.agents.start_game(&scenario_id, seed, tick);
    for side in ["Blue", "Red"] {
        let cfg = agent::AgentConfigReq {
            side: Some(side.to_string()),
            enabled: Some(true),
            autonomy: Some("execute".into()),
            cadence_s: None,
            max_tokens: None,
            auto_run: Some(false),
            auto_speed: None,
            model: None,
            base_url: None,
            api_key: None,
            language: None,
        };
        if let Err(e) = state.agents.set_config(&cfg, tick) {
            let mut g = state.game.lock().unwrap();
            g.director_active = false;
            g.director_id = 0;
            return Err((StatusCode::BAD_REQUEST, e));
        }
    }

    let blue = director::WakeRuntime::new(Side::Blue);
    let red = director::WakeRuntime::new(Side::Red);
    {
        let mut st = state.director.lock().unwrap();
        *st = director::State {
            active: true,
            phase: "planning".into(),
            id,
            scenario_id: scenario_id.clone(),
            tick,
            duration_s: duration,
            trigger: Vec::new(),
            blue_speed: blue.desired_speed,
            red_speed: red.desired_speed,
            global_speed: 15.0,
            blue_rules: blue.rules.clone(),
            red_rules: red.rules.clone(),
            blue_report: String::new(),
            red_report: String::new(),
            last_plan_tick: tick,
            planning_started_unix: director::now_unix(),
            error: None,
            over: false,
            result: None,
            blue_kills: 0,
            red_kills: 0,
            blue_losses: 0,
            red_losses: 0,
        };
    }
    tokio::spawn(run_director(state.clone(), id, blue, red, max_ticks));
    Ok(Json(json!({
        "started": true,
        "id": id,
        "scenario": scenario_id,
        "seed": seed,
        "duration_s": duration,
        "max_ticks": max_ticks,
        "mode": "private_wake_simultaneous_planning_min_speed",
    })))
}

async fn director_stop(State(state): State<AppState>) -> Json<serde_json::Value> {
    {
        let mut g = state.game.lock().unwrap();
        g.director_active = false;
        g.director_id = 0;
        g.speed = 0.0;
        g.accumulator = 0.0;
    }
    let mut st = state.director.lock().unwrap();
    st.active = false;
    st.phase = "stopped".into();
    Json(serde_json::to_value(&*st).unwrap_or_else(|_| json!({"active": false})))
}

async fn director_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::to_value(&*state.director.lock().unwrap()).unwrap_or_else(|_| json!({"active": false})))
}

async fn run_director(
    state: AppState,
    id: u64,
    mut blue: director::WakeRuntime,
    mut red: director::WakeRuntime,
    max_ticks: i64,
) {
    let mut trigger_detail = "initial director planning".to_string();
    loop {
        if !director_is_active(&state, id) {
            return;
        }
        let (obs_blue, obs_red, tick_now, over) = {
            let g = state.game.lock().unwrap();
            let Some(world) = g.world.as_ref() else { return };
            let ob = blue.planning_observation(world);
            let or = red.planning_observation(world);
            (ob, or, world.tick, world.over)
        };
        if over || tick_now >= max_ticks {
            finish_director(&state, id);
            return;
        }
        update_director(&state, id, |st| {
            st.phase = "planning".into();
            st.tick = tick_now;
            st.blue_rules = blue.rules.clone();
            st.red_rules = red.rules.clone();
            st.planning_started_unix = director::now_unix();
            st.trigger = if trigger_detail == "initial director planning" {
                Vec::new()
            } else {
                st.trigger.clone()
            };
        });
        let _ = state.update_tx.send(());

        let ctx_blue = match state.agents.prepare_director_turn(
            Side::Blue,
            tick_now,
            blue.rules.clone(),
            trigger_detail.clone(),
        ) {
            Ok(ctx) => ctx,
            Err(e) => {
                update_director(&state, id, |st| {
                    st.phase = "error".into();
                    st.error = Some(format!("blue prepare: {}", e.message()));
                });
                return;
            }
        };
        let ctx_red = match state.agents.prepare_director_turn(
            Side::Red,
            tick_now,
            red.rules.clone(),
            trigger_detail.clone(),
        ) {
            Ok(ctx) => ctx,
            Err(e) => {
                state.agents.abort_turn(&ctx_blue, "director peer failed".into());
                update_director(&state, id, |st| {
                    st.phase = "error".into();
                    st.error = Some(format!("red prepare: {}", e.message()));
                });
                return;
            }
        };
        let last_seq_blue = obs_blue
            .events
            .iter()
            .map(|e| e.seq)
            .max()
            .unwrap_or(ctx_blue.memory.last_event_seq);
        let last_seq_red = obs_red
            .events
            .iter()
            .map(|e| e.seq)
            .max()
            .unwrap_or(ctx_red.memory.last_event_seq);

        let db_blue = state.db.clone();
        let db_red = state.db.clone();
        let ctx_blue_for_call = ctx_blue.clone();
        let ctx_red_for_call = ctx_red.clone();
        let call_blue = tokio::task::spawn_blocking(move || {
            agent::execute_turn(&ctx_blue_for_call, &obs_blue, &db_blue, None)
        });
        let call_red = tokio::task::spawn_blocking(move || {
            agent::execute_turn(&ctx_red_for_call, &obs_red, &db_red, None)
        });
        let joined = tokio::time::timeout(Duration::from_secs(90), async {
            tokio::join!(call_blue, call_red)
        })
        .await;
        let (decision_blue, decision_red) = match joined {
            Ok((Ok(b), Ok(r))) => (b, r),
            Ok((Err(e), _)) => {
                state.agents.abort_turn(&ctx_blue, format!("blue task failed: {e}"));
                state.agents.abort_turn(&ctx_red, "peer task failed".into());
                update_director(&state, id, |st| {
                    st.phase = "error".into();
                    st.error = Some(format!("blue task failed: {e}"));
                });
                return;
            }
            Ok((_, Err(e))) => {
                state.agents.abort_turn(&ctx_blue, "peer task failed".into());
                state.agents.abort_turn(&ctx_red, format!("red task failed: {e}"));
                update_director(&state, id, |st| {
                    st.phase = "error".into();
                    st.error = Some(format!("red task failed: {e}"));
                });
                return;
            }
            Err(_) => {
                state.agents.abort_turn(&ctx_blue, "director planning timeout (90s)".into());
                state.agents.abort_turn(&ctx_red, "director planning timeout (90s)".into());
                update_director(&state, id, |st| {
                    st.phase = "error".into();
                    st.error = Some("director planning timeout (90s)".into());
                });
                return;
            }
        };

        {
            let g = state.game.lock().unwrap();
            if !g.director_active || g.director_id != id {
                drop(g);
                state.agents.abort_turn(&ctx_blue, "director stopped".into());
                state.agents.abort_turn(&ctx_red, "director stopped".into());
                return;
            }
        }
        let blue_speed = if decision_blue.error.is_none() {
            decision_blue.desired_speed.unwrap_or(blue.desired_speed).clamp(5.0, 60.0)
        } else {
            blue.desired_speed.max(5.0)
        };
        let red_speed = if decision_red.error.is_none() {
            decision_red.desired_speed.unwrap_or(red.desired_speed).clamp(5.0, 60.0)
        } else {
            red.desired_speed.max(5.0)
        };
        let blue_rules = if decision_blue.wake_conditions.is_empty() {
            blue.rules.clone()
        } else {
            director::effective_rules(&decision_blue.wake_conditions)
        };
        let red_rules = if decision_red.wake_conditions.is_empty() {
            red.rules.clone()
        } else {
            director::effective_rules(&decision_red.wake_conditions)
        };
        let commands_blue = decision_blue.commands.clone();
        let commands_red = decision_red.commands.clone();
        let applied_blue = decision_blue.error.is_none() && !commands_blue.is_empty();
        let applied_red = decision_red.error.is_none() && !commands_red.is_empty();
        let error_blue = decision_blue.error.clone();
        let error_red = decision_red.error.clone();
        let report_blue = state
            .agents
            .finish_turn(&ctx_blue, decision_blue, applied_blue, last_seq_blue);
        let report_red = state
            .agents
            .finish_turn(&ctx_red, decision_red, applied_red, last_seq_red);
        blue.set_rules(blue_rules);
        red.set_rules(red_rules);
        blue.desired_speed = blue_speed;
        red.desired_speed = red_speed;
        let global = blue_speed.min(red_speed).clamp(5.0, 60.0);
        {
            let mut g = state.game.lock().unwrap();
            g.speed = global;
            g.accumulator = 0.0;
            g.director_active = true;
            g.director_id = id;
        }
        update_director(&state, id, |st| {
            st.phase = "running".into();
            st.tick = tick_now;
            st.blue_speed = blue_speed;
            st.red_speed = red_speed;
            st.global_speed = global;
            st.blue_rules = blue.rules.clone();
            st.red_rules = red.rules.clone();
            st.blue_report = if error_blue.is_some() {
                format!("ERROR: {}", error_blue.clone().unwrap_or_default())
            } else {
                report_blue.report.clone()
            };
            st.red_report = if error_red.is_some() {
                format!("ERROR: {}", error_red.clone().unwrap_or_default())
            } else {
                report_red.report.clone()
            };
            st.error = error_blue.clone().or(error_red.clone());
            st.trigger.clear();
            st.planning_started_unix = 0;
            st.refresh_outcome(
                state.game.lock().unwrap().world.as_ref().unwrap(),
            );
        });
        let _ = state.update_tx.send(());

        let mut pending: Vec<(Side, Command)> = Vec::new();
        pending.extend(commands_blue.into_iter().map(|c| (Side::Blue, c)));
        pending.extend(commands_red.into_iter().map(|c| (Side::Red, c)));
        let mut accumulator = 0.0f64;
        let mut hits: Vec<director::WakeHit> = Vec::new();
        let mut over_now = None;
        'advance: loop {
            if !director_is_active(&state, id) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            accumulator += global * 0.1;
            let n = accumulator.floor() as i64;
            accumulator -= n as f64;
            let n = n.min(MAX_TICKS_PER_STEP);
            if n <= 0 {
                continue;
            }
            for i in 0..n {
                let (over, hb, hr, tick) = {
                    let mut g = state.game.lock().unwrap();
                    let Some(world) = g.world.as_mut() else { return };
                    let applied: &[(Side, Command)] = if i == 0 { &pending } else { &[] };
                    harpoon_core::sim::tick(world, applied);
                    let hb = blue.evaluate(world);
                    let hr = red.evaluate(world);
                    (world.over, hb, hr, world.tick)
                };
                if i == 0 {
                    pending.clear();
                }
                if let Some(hit) = hb {
                    hits.push(hit);
                }
                if let Some(hit) = hr {
                    hits.push(hit);
                }
                over_now = Some(over);
                if tick % 15 == 0 || over || !hits.is_empty() {
                    update_director(&state, id, |st| {
                        st.tick = tick;
                        st.refresh_outcome(
                            state.game.lock().unwrap().world.as_ref().unwrap(),
                        );
                    });
                }
                if over || !hits.is_empty() {
                    break 'advance;
                }
            }
            let _ = state.update_tx.send(());
        }
        if over_now.unwrap_or(false) {
            finish_director(&state, id);
            return;
        }
        if hits.is_empty() {
            return;
        }
        blue.mark_wake(tick_now);
        red.mark_wake(tick_now);
        trigger_detail = hits
            .iter()
            .map(|hit| format!("{}:{} ({})", hit.side.name(), hit.rule, hit.detail))
            .collect::<Vec<_>>()
            .join("; ");
        update_director(&state, id, |st| {
            st.phase = "planning".into();
            st.trigger = hits.clone();
            st.blue_rules = blue.rules.clone();
            st.red_rules = red.rules.clone();
            st.planning_started_unix = director::now_unix();
        });
        let _ = state.update_tx.send(());
    }
}

fn finish_director(state: &AppState, id: u64) {
    let (over, result, kills, losses, tick) = {
        let g = state.game.lock().unwrap();
        let Some(world) = g.world.as_ref() else { return };
        (
            world.over,
            world.result.clone(),
            world.kill_counts(),
            world.losses,
            world.tick,
        )
    };
    update_director(state, id, |st| {
        st.active = false;
        st.phase = if over { "over".into() } else { "stopped".into() };
        st.tick = tick;
        st.over = over;
        st.result = result;
        st.blue_kills = kills.0;
        st.red_kills = kills.1;
        st.blue_losses = losses[Side::Blue.index()];
        st.red_losses = losses[Side::Red.index()];
        st.planning_started_unix = 0;
    });
    if let Ok(mut g) = state.game.lock() {
        g.director_active = false;
        g.director_id = 0;
        g.speed = 0.0;
        g.accumulator = 0.0;
    }
    let _ = state.update_tx.send(());
}

async fn db_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json((*state.db).clone())
}

/// AI interface: ask the configured command staff to decide orders for a side
/// and queue them. Uses the LLM when HARPOON_LLM_BASE_URL is set, else the
/// deterministic heuristic doctrine.
async fn ai_step(
    State(state): State<AppState>,
    AxPath(side_str): AxPath<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let side = Side::parse(&side_str).ok_or((StatusCode::BAD_REQUEST, "side must be Blue|Red".into()))?;
    let (obs, pending) = {
        let g = state.game.lock().unwrap();
        let world = g.world.as_ref().ok_or((StatusCode::CONFLICT, "no game running".into()))?;
        (observe(world, side, 0, 100), g.inbox.len())
    };
    let staff_result = tokio::task::spawn_blocking(move || {
        let mut staff = ai::make_staff(side);
        let cmds = staff.decide(&obs);
        (staff.name(), cmds)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (staff_name, cmds) = staff_result;
    let mut g = state.game.lock().unwrap();
    for c in &cmds {
        g.inbox.push((side, c.clone()));
    }
    Ok(Json(json!({"staff": staff_name, "commands": cmds, "pending_before": pending})))
}

#[derive(Deserialize)]
struct SelfplayReq {
    scenario: String,
    seed: Option<u64>,
    max_ticks: Option<i64>,
}

/// Deterministic headless rollout: both sides driven by the heuristic staff,
/// run until scenario end or max_ticks. Returns event counts, final VP, and
/// a state digest (same seed+scenario+staff => same digest).
async fn selfplay_handler(
    State(state): State<AppState>,
    Json(req): Json<SelfplayReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = state
        .scenario_dir
        .join(format!("{}.json", req.scenario))
        .canonicalize()
        .map_err(|_| (StatusCode::NOT_FOUND, "scenario not found".into()))?;
    let txt = std::fs::read_to_string(&path).map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    let mut sc = scenario_from_json(&txt).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if let Some(seed) = req.seed {
        sc.seed = seed;
    }
    let max_ticks = req.max_ticks.unwrap_or(sc.duration_s.max(1));
    let result = tokio::task::spawn_blocking(move || run_selfplay(&sc, max_ticks))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(result))
}

pub fn run_selfplay(sc: &Scenario, max_ticks: i64) -> serde_json::Value {
    SelfplayCtx::ensure().run(sc, max_ticks)
}

/// Lazily-initialised database for headless rollouts (also picks up volume
/// overrides so selfplay matches server games).
pub struct SelfplayCtx {
    db: Database,
}
static SELFPLAY_DB: std::sync::OnceLock<SelfplayCtx> = std::sync::OnceLock::new();
impl SelfplayCtx {
    fn ensure() -> &'static SelfplayCtx {
        SELFPLAY_DB.get_or_init(|| SelfplayCtx {
            db: load_volume_database(std::path::Path::new("database/volumes")),
        })
    }
    fn run(&self, sc: &Scenario, max_ticks: i64) -> serde_json::Value {
        let mut world = load_scenario(sc.clone(), self.db.clone()).expect("load");
    let mut blue = ai::HeuristicStaff::new(Side::Blue);
    let mut red = ai::HeuristicStaff::new(Side::Red);
    let digest = AtomicU64::new(0xcbf2_9ce4_8422_2325);
    let mut event_counts: HashMap<String, u64> = HashMap::new();
    let mut last_events_seq = 0u64;

    for t in 0..max_ticks {
        if world.over {
            break;
        }
        let mut cmds: Vec<(Side, Command)> = Vec::new();
        let mut applied: Vec<(Side, Command)> = Vec::new();
        if t % 30 == 0 {
            for (staff, side) in [(&mut blue, Side::Blue), (&mut red, Side::Red)] {
                let obs = observe(&world, side, 0, 50);
                applied.extend(staff.decide(&obs).into_iter().map(|c| (side, c)));
            }
        }
        harpoon_core::sim::tick(&mut world, &applied);
        cmds.clear();
        let _ = cmds;
        for ev in world.events_for(Side::Blue, last_events_seq) {
            *event_counts.entry(ev.kind.name().to_string()).or_default() += 1;
        }
        // Events visible to Blue ∪ Red covers everything non-referee; the
        // heuristic staff never produces referee-only events.
        if let Some(ev) = world.events.last() {
            last_events_seq = ev.seq;
        }
        let h = digest.load(Ordering::Relaxed);
        let mut x = h;
        for e in world.entities.values() {
            x ^= (e.pos.x.to_bits()) >> 1;
            x = x.wrapping_mul(0x100_0000_01b3);
            x ^= (e.pos.y.to_bits()) << 3;
            x = x.wrapping_mul(0x100_0000_01b3);
            x ^= (e.hp.to_bits()) >> 5;
            x = x.wrapping_mul(0x100_0000_01b3);
        }
        digest.store(x, Ordering::Relaxed);
    }

    let (vp_blue, vp_red) = world.victory_points();
    json!({
        "scenario": sc.id,
        "seed": sc.seed,
        "ticks_run": world.tick,
        "over": world.over,
        "result": world.result,
        "vp": {"blue": vp_blue, "red": vp_red},
        "state_digest": format!("{:016x}", digest.load(Ordering::Relaxed)),
        "event_counts": event_counts,
            "entities_alive": world.entities.values().filter(|e| e.alive).count(),
        })
    }
}

// ---------------------------------------------------------------------------
// WebSocket
// ---------------------------------------------------------------------------

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(q): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let side = q
        .get("side")
        .and_then(|s| Side::parse(s))
        .unwrap_or(Side::Blue);
    ws.on_upgrade(move |socket| ws_session(socket, state, side))
}

async fn ws_session(socket: WebSocket, state: AppState, side: Side) {
    let (mut tx, mut rx) = socket.split();
    let mut poke = state.update_tx.subscribe();

    // Push observations at ~4 Hz on pokes.
    let push_state = state.clone();
    let push_task = tokio::spawn(async move {
        loop {
            if poke.recv().await.is_err() {
                break;
            }
            let obs = {
                let g = push_state.game.lock().unwrap();
                g.world.as_ref().map(|w| observe(w, side, 0, 100))
            };
            if let Some(obs) = obs {
                let Ok(txt) = serde_json::to_string(&obs) else { break };
                if tx.send(Message::Text(txt)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Receive commands / time control.
    while let Some(Ok(msg)) = rx.next().await {
        if let Message::Text(txt) = msg {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) else { continue };
            if let Some(speed) = v.get("speed").and_then(|s| s.as_f64()) {
                let mut g = state.game.lock().unwrap();
                g.speed = speed.clamp(0.0, 512.0);
            }
            if let Some(cmd) = v.get("command").cloned() {
                if let Ok(command) = serde_json::from_value::<Command>(cmd) {
                    let mut g = state.game.lock().unwrap();
                    g.inbox.push((side, command));
                }
            }
        }
    }
    push_task.abort();
}

use futures_util::{SinkExt, StreamExt};

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str, default: &str| -> String {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| default.to_string())
    };
    let port: u16 = arg("--port", "8787").parse().unwrap_or(8787);
    let scenario_dir = Arc::new(PathBuf::from(arg("--scenarios", "scenarios")));
    let web_dir = PathBuf::from(arg("--web", "web/dist"));
    let start_scenario = args.iter().position(|a| a == "--scenario").and_then(|i| args.get(i + 1)).cloned();
    let start_speed: f64 = arg("--speed", "60").parse().unwrap_or(60.0);

    let update_tx = tokio::sync::broadcast::channel(64).0;
    let volumes_dir = std::path::PathBuf::from("database/volumes");
    let merged_db = load_volume_database(&volumes_dir);
    let state = AppState {
        game: Arc::new(Mutex::new(Game::new())),
        scenario_dir: scenario_dir.clone(),
        update_tx: Arc::new(update_tx),
        db: Arc::new(merged_db),
        agents: Arc::new(agent::Runtime::new()),
        director: Arc::new(Mutex::new(director::State::default())),
        director_seq: Arc::new(AtomicU64::new(0)),
    };

    run_loop(state.clone());

    let static_dir = if web_dir.exists() { web_dir.clone() } else { PathBuf::from("web") };
    let serve = ServeDir::new(&static_dir);

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/scenarios", get(list_scenarios))
        .route("/api/game", post(new_game))
        .route("/api/state", get(state_handler))
        .route("/api/command", post(command_handler))
        .route("/api/time", post(time_handler).get(time_get))
        .route("/api/tutorial/catalog", get(training_catalog))
        .route("/api/tutorial/status", get(training_status))
        .route("/api/tutorial/start", post(training_start))
        .route("/api/tutorial/action", post(training_action))
        .route("/api/db", get(db_handler))
        .route("/api/ai/step/:side", post(ai_step))
        .route("/api/agent/status", get(agent_status))
        .route("/api/agent/config", post(agent_config))
        .route("/api/agent/reset", post(agent_reset))
        .route("/api/agent/duel", post(agent_duel_start).get(agent_duel_status))
        .route("/api/director", get(director_status))
        .route("/api/director/start", post(director_start))
        .route("/api/director/stop", post(director_stop))
        .route("/api/agent/:side/status", get(agent_side_status))
        .route("/api/agent/:side/report", post(agent_report))
        .route("/api/agent/:side/chat", post(agent_chat))
        .route("/api/agent/:side/execute", post(agent_execute))
        .route("/api/selfplay", post(selfplay_handler))
        .route("/ws", get(ws_handler))
        .fallback_service(serve)
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    // Optional eager start.
    if let Some(sc) = start_scenario {
        let req = NewGame { scenario: sc, seed: None, speed: start_speed };
        let _ = new_game(State(state), Json(req)).await;
    }

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    eprintln!("harpoon-server listening on http://{addr}  (LLM staff: {})",
        if std::env::var("HARPOON_LLM_BASE_URL").is_ok() { "enabled" } else { "heuristic fallback" });
    axum::serve(listener, app).await.unwrap();
}

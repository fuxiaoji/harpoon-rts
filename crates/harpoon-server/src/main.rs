//! harpoon-server: authoritative referee service.
//!
//! - The simulation loop runs server-side at 10 Hz wall clock, advancing
//!   `speed` simulated seconds per real second (0 = pause, up to 512x).
//! - Every client (web UI, LLM staff, MCP tool, RL env) talks to the same
//!   REST/WS surface and only ever receives double-blind observations.
//! - POST /api/selfplay runs a fully deterministic headless rollout.

mod ai;

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
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

#[derive(Clone)]
struct AppState {
    game: Arc<Mutex<Game>>,
    scenario_dir: Arc<PathBuf>,
    update_tx: Arc<tokio::sync::broadcast::Sender<()>>,
    /// Built-in database merged with database/volumes/*.json at startup.
    db: Arc<Database>,
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
}

impl Game {
    fn new() -> Self {
        Self { world: None, speed: 0.0, accumulator: 0.0, inbox: Vec::new() }
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
                if g.speed > 0.0 && !over {
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
                if let Some(world) = g.world.as_mut() {
                    // Commands apply once on the first tick (orders persist
                    // as entity state afterwards).
                    let empty: Vec<(Side, Command)> = Vec::new();
                    for i in 0..n {
                        let c: &[(Side, Command)] = if i == 0 { &cmds } else { &empty };
                        harpoon_core::sim::tick(world, c);
                        if world.over {
                            break;
                        }
                    }
                    poked = true;
                }
            }
            if poked {
                let _ = state.update_tx.send(());
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
    let mut g = state.game.lock().unwrap();
    *g = Game { world: Some(world), speed: req.speed, accumulator: 0.0, inbox: Vec::new() };
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
        .route("/api/db", get(db_handler))
        .route("/api/ai/step/:side", post(ai_step))
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

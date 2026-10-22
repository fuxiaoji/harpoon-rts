//! Local referee persistence. Save payloads never travel through player APIs.
//! The archive keeps every event beyond World's ring buffer. Command replay
//! reinserts controller audit events at the original boundary, then ticks core.
use super::*;
use harpoon_core::events::Event;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;

const VERSION: u32 = 1;
const COMPAT: &str = "harpoon-rts-save-20261022-3";
const LEGACY_COMPAT: &str = "harpoon-rts-save-20261020-2";

pub fn digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serializable state");
    let h = bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x100000001b3)
    });
    format!("{h:016x}")
}

#[derive(Clone, Serialize, Deserialize)]
struct Input {
    tick: i64,
    commands: Vec<(Side, Command)>,
    injected: Vec<Event>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Recording {
    initial: World,
    inputs: Vec<Input>,
    events: Vec<Event>,
    frames: BTreeMap<String, Vec<Observation>>,
    last_seq: u64,
    #[serde(default, skip_serializing_if = "is_false")]
    completed_saved: bool,
}
fn is_false(value: &bool) -> bool {
    !*value
}

impl Recording {
    pub fn new(world: &World) -> Self {
        let mut r = Self {
            initial: world.clone(),
            inputs: vec![],
            events: world.events.clone(),
            frames: BTreeMap::new(),
            last_seq: world.event_seq,
            completed_saved: false,
        };
        r.sample(world);
        r
    }
    fn sample(&mut self, world: &World) {
        for side in [Side::Blue, Side::Red] {
            let list = self.frames.entry(side.name().into()).or_default();
            if list.last().is_some_and(|f| f.tick == world.tick) {
                list.pop();
            }
            list.push(observe(world, side, world.event_seq, 0));
        }
    }
    pub fn before(&mut self, world: &World, commands: &[(Side, Command)]) {
        let injected: Vec<_> = world
            .events
            .iter()
            .filter(|e| e.seq > self.last_seq)
            .cloned()
            .collect();
        if !commands.is_empty() || !injected.is_empty() {
            self.inputs.push(Input {
                tick: world.tick,
                commands: commands.to_vec(),
                injected,
            });
        }
    }
    pub fn after(&mut self, world: &World) {
        self.events.extend(
            world
                .events
                .iter()
                .filter(|e| e.seq > self.last_seq)
                .cloned(),
        );
        self.last_seq = world.event_seq;
        if world.tick % 30 == 0 || world.over {
            self.sample(world);
        }
    }
    pub fn archive(&self, world: &World) -> Value {
        let mut r = self.clone();
        r.sample(world);
        let orders:Vec<_>=r.inputs.iter().flat_map(|input|input.commands.iter().map(move |(side,command)|json!({"tick":input.tick+1,"side":side,"command":command}))).collect();
        let max_gap = r
            .frames
            .values()
            .flat_map(|frames| frames.windows(2).map(|w| w[1].tick - w[0].tick))
            .max()
            .unwrap_or(0);
        json!({"format":"harpoon-observation-replay", "version":1,
            "id":format!("{}-{}-{}",world.scenario_id,world.seed,world.tick),
            "title":world.scenario_name, "seed":world.seed,"theater":world.context.theater,"terrain":world.context.terrain,
            "frames":r.frames,"events":r.events,"orders":orders,"bookmarks":[],
            "source":{"kind":"server-synchronous", "sample_interval_s":30,
                "start_tick":r.initial.tick,"max_sample_gap_s":max_gap,"event_count":r.events.len(),
                "command_batches":r.inputs.len(),"engine_compat":COMPAT,
                "final_world_fnv64":digest(world),
                "precision":"Synchronized 30-second observations and all per-second events; commands can be replayed by the server. No interpolation."}})
    }
    pub fn verify(&self, expected: &World) -> Result<Value, String> {
        let mut world = self.initial.clone();
        let mut events = world.events.clone();
        let mut i = 0;
        while world.tick < expected.tick && !world.over {
            let last_seq = world.event_seq;
            let cap = world.event_cap;
            world.event_cap = usize::MAX;
            let input = self.inputs.get(i).filter(|x| x.tick == world.tick);
            if let Some(input) = input {
                for event in &input.injected {
                    if event.seq != world.event_seq + 1 {
                        return Err("injected event sequence mismatch".into());
                    }
                    world.emit(event.kind.clone(), event.vis);
                }
                harpoon_core::sim::tick(&mut world, &input.commands);
                i += 1;
            } else {
                harpoon_core::sim::tick(&mut world, &[]);
            }
            events.extend(world.events.iter().filter(|e| e.seq > last_seq).cloned());
            world.event_cap = cap;
            if world.events.len() > cap {
                world.events.drain(0..world.events.len() - cap);
            }
        }
        let actual = digest(&world);
        let wanted = digest(expected);
        if actual != wanted {
            return Err(format!("replay diverged: {actual} != {wanted}"));
        }
        if digest(&events) != digest(&self.events) {
            return Err("replay event history diverged".into());
        }
        Ok(
            json!({"verified":true,"tick":world.tick,"world_fnv64":actual,"command_batches":i,"events_verified":events.len()}),
        )
    }
}

#[derive(Serialize, Deserialize)]
struct Payload {
    world: World,
    inbox: Vec<(Side, Command)>,
    tactical: [TacticalSlot; 2],
    recording: Recording,
    agents: Value,
    #[serde(default,skip_serializing_if="Option::is_none")]
    ai_supervision:Option<[tactical::supervision::Supervisor;2]>,
}
#[derive(Serialize, Deserialize)]
struct Save {
    format: String,
    version: u32,
    engine_compat: String,
    label: String,
    created_unix_ms: u128,
    checksum_fnv64: String,
    payload: Payload,
}

fn directory() -> PathBuf {
    std::env::var_os("HARPOON_SAVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tmp/saves"))
}
fn path(id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("invalid save id".into());
    }
    Ok(directory().join(format!("{id}.json")))
}
fn read(id: &str) -> Result<Save, String> {
    let save: Save = serde_json::from_slice(&std::fs::read(path(id)?).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if save.format != "harpoon-server-save"
        || save.version != VERSION
        || ![COMPAT,LEGACY_COMPAT,"harpoon-rts-save-20261022-2","harpoon-rts-save-20261022-1","harpoon-rts-save-20261021-1","harpoon-rts-save-20261021-2","harpoon-rts-save-20261021-3","harpoon-rts-save-20261021-4"].contains(&save.engine_compat.as_str())
    {
        return Err("unsupported save version or engine compatibility".into());
    }
    if digest(&save.payload) != save.checksum_fnv64 {
        return Err("save checksum mismatch; current game retained".into());
    }
    save.payload.world.rules.validate_tables()?;
    save.payload.recording.initial.rules.validate_tables()?;
    Ok(save)
}
fn info(id: &str, save: &Save) -> Value {
    json!({"id":id,"label":save.label,"created_unix_ms":save.created_unix_ms,
        "scenario":save.payload.world.scenario_name,"scenario_id":save.payload.world.scenario_id,
        "tick":save.payload.world.tick,"seed":save.payload.world.seed,"over":save.payload.world.over,
        "version":save.version,"checksum_fnv64":save.checksum_fnv64})
}
fn err(message: impl ToString) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, message.to_string())
}

#[derive(Deserialize)]
pub struct SaveReq {
    #[serde(default)]
    label: String,
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<SaveReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Lock order matches the simulation loop: game, then agent runtime.
    let mut g = state.game.lock().unwrap();
    if g.training.is_some() || g.director_active || g.agent_hold || state.agents.any_busy() {
        return Err(err("save an operation outside an active planning window; training checkpoints use the course controls"));
    }
    let result = write_game(&g, &state.agents, &req.label)?;
    // A successful manual save is also a clear pause point.
    g.speed = 0.0;
    g.accumulator = 0.0;
    state.agents.suspend_auto_run();
    Ok(Json(result))
}
fn write_game(
    g: &Game,
    agents: &agent::Runtime,
    label: &str,
) -> Result<Value, (StatusCode, String)> {
    let world = g.world.as_ref().ok_or_else(|| err("no game"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let id = format!("{now}-{}-{}", std::process::id(), world.tick);
    let payload = Payload {
        world: world.clone(),
        inbox: g.inbox.clone(),
        tactical: g.tactical.clone(),
        recording: g
            .recording
            .clone()
            .ok_or_else(|| err("recording not initialized"))?,
        agents: agents.snapshot(),
        ai_supervision:g.ai_supervision.clone(),
    };
    let save = Save {
        format: "harpoon-server-save".into(),
        version: VERSION,
        engine_compat: COMPAT.into(),
        label: label.chars().take(100).collect(),
        created_unix_ms: now,
        checksum_fnv64: digest(&payload),
        payload,
    };
    std::fs::create_dir_all(directory()).map_err(err)?;
    let final_path = path(&id).map_err(err)?;
    let pending = final_path.with_extension("pending");
    let bytes = serde_json::to_vec(&save).map_err(err)?;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .map_err(err)?;
    f.write_all(&bytes)
        .and_then(|_| f.sync_all())
        .map_err(err)?;
    drop(f);
    std::fs::rename(&pending, final_path).map_err(err)?;
    Ok(info(&id, &save))
}
pub fn complete(g: &mut Game, agents: &agent::Runtime) {
    if g.training.is_none()
        && g.world.as_ref().is_some_and(|w| w.over)
        && g.recording.as_ref().is_some_and(|r| !r.completed_saved)
    {
        match write_game(g, agents, "自动归档 · Completed battle") {
            Ok(_) => {
                g.recording.as_mut().unwrap().completed_saved = true;
            }
            Err((_, e)) => eprintln!("completed battle save failed: {e}"),
        }
    }
}
pub async fn list() -> Json<Value> {
    let mut saves = vec![];
    if let Ok(entries) = std::fs::read_dir(directory()) {
        for e in entries.flatten() {
            if e.path().extension().is_some_and(|x| x == "json") {
                let id = e
                    .path()
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                match read(&id) {
                    Ok(save) => saves.push(info(&id, &save)),
                    Err(error) => saves.push(json!({"id":id,"invalid":true,"error":error})),
                }
            }
        }
    }
    saves.sort_by(|a, b| b["id"].as_str().cmp(&a["id"].as_str()));
    Json(json!({"saves":saves}))
}
pub async fn load(
    State(state): State<AppState>,
    AxPath(id): AxPath<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let save = read(&id).map_err(err)?;
    let mut g = state.game.lock().unwrap();
    if g.director_active || g.agent_hold || state.agents.any_busy() {
        return Err(err("stop planning before loading"));
    }
    let p = save.payload;
    // Validate agent data before replacing any authoritative state.
    state
        .agents
        .restore_snapshot(p.agents, &p.world.scenario_id, p.world.seed, p.world.tick)
        .map_err(err)?;
    let mut tactical = p.tactical;
    for slot in &mut tactical {
        slot.staff.restore_database(p.world.db.clone());
    }
    let completed = g.training_completed.clone();
    *g = Game {
        ai_supervision:p.ai_supervision,
        recording: Some(p.recording),
        world: Some(p.world),
        speed: 0.0,
        accumulator: 0.0,
        inbox: p.inbox,
        training: None,
        training_completed: completed,
        progress_saved: true,
        director_active: false,
        director_id: 0,
        agent_hold: false,
        resume_speed: 0.0,
        planning_outstanding: 0,
        tactical,
    };
    state.bump_session();
    let _ = state.update_tx.send(());
    Ok(Json(
        json!({"loaded":true,"paused":true,"tick":g.world.as_ref().unwrap().tick,"id":id}),
    ))
}
pub async fn archive(State(state): State<AppState>) -> Result<Json<Value>, (StatusCode, String)> {
    let g = state.game.lock().unwrap();
    let w = g.world.as_ref().ok_or_else(|| err("no game"))?;
    if !w.over {
        return Err((
            StatusCode::CONFLICT,
            "full replay is available after the battle ends".into(),
        ));
    }
    Ok(Json(
        g.recording
            .as_ref()
            .ok_or_else(|| err("no recording"))?
            .archive(w),
    ))
}
pub async fn saved_archive(
    AxPath(id): AxPath<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let save = read(&id).map_err(err)?;
    if !save.payload.world.over {
        return Err((
            StatusCode::CONFLICT,
            "full replay is available after the battle ends".into(),
        ));
    }
    Ok(Json(save.payload.recording.archive(&save.payload.world)))
}
pub async fn verify(State(state): State<AppState>) -> Result<Json<Value>, (StatusCode, String)> {
    let (r, w) = {
        let g = state.game.lock().unwrap();
        (
            g.recording.clone().ok_or_else(|| err("no recording"))?,
            g.world.clone().ok_or_else(|| err("no game"))?,
        )
    };
    // CPU-heavy replay never holds the authoritative game lock.
    tokio::task::spawn_blocking(move || r.verify(&w).map(Json).map_err(err))
        .await
        .map_err(err)?
}
pub async fn verify_saved(AxPath(id): AxPath<String>) -> Result<Json<Value>, (StatusCode, String)> {
    tokio::task::spawn_blocking(move || {
        let save = read(&id).map_err(err)?;
        save.payload
            .recording
            .verify(&save.payload.world)
            .map(Json)
            .map_err(err)
    })
    .await
    .map_err(err)?
}

#[derive(Deserialize)]
pub struct AdvanceReq {
    side: String,
    #[serde(default)]
    commands: Vec<Command>,
    #[serde(default)]
    intent: String,
    ticks: i64,
    #[serde(default)]
    plan_id: Option<String>,
    #[serde(default)]
    expected_tick: Option<i64>,
    #[serde(default)]
    session_revision: Option<u64>,
}

/// Transport replay protection, deliberately outside World/RNG/save hashes.
/// Loading a save or starting a game changes the session, invalidating old plans.
#[derive(Default)]
pub struct AdvancePlans {
    revision: u64,
    used: [std::collections::VecDeque<String>; 2],
}
impl AdvancePlans {
    fn contains(&self, revision: u64, side: Side, id: &str) -> bool {
        self.revision == revision && self.used[side.index()].iter().any(|p| p == id)
    }
    fn consume(&mut self, revision: u64, side: Side, id: String) {
        if self.revision != revision { *self = Self { revision, ..Self::default() }; }
        let list = &mut self.used[side.index()];
        list.push_back(id);
        // Older requests still fail expected_tick even after leaving this tail.
        if list.len() > 256 { list.pop_front(); }
    }
}
/// Local, paused referee driver for externally orchestrated side-only agents.
/// A fixed 300-second maximum preserves the same public planning window.
pub async fn advance(
    State(state): State<AppState>,
    Json(req): Json<AdvanceReq>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let side = Side::parse(&req.side).ok_or_else(|| err("side must be Blue|Red"))?;
    if !(1..=300).contains(&req.ticks) || req.commands.len() > agent::MAX_COMMANDS {
        return Err(err(
            "ticks 1..300, commands <=16; oversized batches are rejected without advancing",
        ));
    }
    let mut g = state.game.lock().unwrap();
    if g.speed != 0.0
        || g.director_active
        || g.agent_hold
        || g.training.is_some()
        || state.agents.any_busy()
    {
        return Err(err("paused operation required"));
    }
    let w = g.world.as_ref().ok_or_else(|| err("no game"))?;
    if w.over || g.tactical[side.index()].enabled {
        return Err(err("battle over or this side is under FSM control"));
    }
    let plan_id = req.plan_id.as_deref().filter(|s| !s.is_empty() && s.len() <= 96
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
        .ok_or_else(|| err("plan_id is required (1..96 ASCII letters, digits, '-' or '_')"))?;
    let expected_tick = req.expected_tick.ok_or_else(|| err("expected_tick from the plan's observation is required"))?;
    let expected_revision = req.session_revision.ok_or_else(|| err("session_revision from the plan's observation is required"))?;
    let revision = state.session_revision.load(Ordering::Relaxed);
    if expected_revision != revision {
        return Err((StatusCode::CONFLICT, "stale_session: observe again after new game or load".into()));
    }
    if state.advance_plans.lock().unwrap().contains(revision, side, plan_id) {
        return Err((StatusCode::CONFLICT, "duplicate_plan: already consumed; no commands queued or time advanced".into()));
    }
    if expected_tick != w.tick {
        return Err((StatusCode::CONFLICT, format!("stale_plan: expected tick {expected_tick}, current {}", w.tick)));
    }
    let obs = observe(w, side, 0, 0);
    let start = w.tick;
    let (valid, mut rejected) = agent::validate_commands(&obs, side, &req.commands);
    let (valid,policy_rejected)=g.supervise_ai(side,valid);rejected.extend(policy_rejected);
    if !req.intent.trim().is_empty() {
        let w = g.world.as_mut().unwrap();
        w.emit(
            harpoon_core::events::EventKind::RulesEvent {
                domain: "llm_ai".into(),
                rule: "intent".into(),
                side: Some(side),
                unit: None,
                contact: None,
                detail: req.intent.chars().take(4000).collect(),
            },
            harpoon_core::events::Visibility::Side(side),
        );
    }
    let mut commands = std::mem::take(&mut g.inbox);
    commands.extend(valid.iter().cloned().map(|c| (side, c)));
    for i in 0..req.ticks {
        g.step(if i == 0 { &commands } else { &[] });
        if g.world.as_ref().unwrap().over {
            break;
        }
    }
    complete(&mut g, &state.agents);
    let w = g.world.as_ref().unwrap();
    state.advance_plans.lock().unwrap().consume(revision, side, plan_id.to_string());
    let out = json!({"plan_id":plan_id,"start_tick":start,"end_tick":w.tick,"accepted":valid,
        "applied":valid,"rejected":rejected,"observation":state.player_view(w,side,0,200)});
    let _ = state.update_tx.send(());
    Ok(Json(out))
}

/// Full-state storage operations are local referee controls, never LAN player controls.
pub async fn local_only(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let local = req
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .is_some_and(|peer| peer.0.ip().is_loopback());
    if !local {
        return (StatusCode::FORBIDDEN, "local referee controls only").into_response();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> Game {
        let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
        let sc=scenario_from_json(r#"{"id":"persistence-test","name":"Save test","duration_s":1500,"seed":19,
          "weather":{"visibility_nm":40,"jam":[0,0]},"victory":{"kill_count_at_end":true},
          "context":{"air_wings":{"CVN":[{"class":"us_f18e","count":8},{"class":"us_f18e_asm","count":8}]}},
          "units":[{"side":"Blue","class":"us_ford","name":"CVN","pos":[0,0]},
          {"side":"Red","class":"cn_055","name":"RED","pos":[100,0]}]}"#).unwrap();
        let mut w = load_scenario(sc, db.clone()).unwrap();
        w.event_cap = 8;
        let mut g = Game::new(w.db.clone());
        g.recording = Some(Recording::new(&w));
        g.world = Some(w);
        for slot in &mut g.tactical {
            slot.enabled = true;
            slot.last_tick = -30;
        }
        g
    }
    #[test]
    fn save_roundtrip_continues_fsm_rng_queue_and_archive_exactly() {
        let mut a = game();
        for _ in 0..500 {
            a.step(&[]);
        }
        let id = a
            .world
            .as_ref()
            .unwrap()
            .entities
            .values()
            .find(|e| e.side == Side::Blue && e.kind == harpoon_core::types::PlatformKind::Ship)
            .unwrap()
            .id;
        a.inbox.push((
            Side::Blue,
            Command::SetSpeed {
                unit: id,
                speed_kts: 12.345678901234567,
            },
        ));
        let runtime = agent::Runtime::new();
        let p = Payload {
            world: a.world.clone().unwrap(),
            inbox: a.inbox.clone(),
            tactical: a.tactical.clone(),
            recording: a.recording.clone().unwrap(),
            agents: runtime.snapshot(),
            ai_supervision:a.ai_supervision.clone(),
        };
        let hash = digest(&p);
        let bytes = serde_json::to_vec(&p).unwrap();
        let mut p: Payload = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(hash, digest(&p));
        let mut b = Game::new(p.world.db.clone());
        for s in &mut p.tactical {
            s.staff.restore_database(p.world.db.clone());
        }
        b.world = Some(p.world);
        b.inbox = p.inbox;
        b.tactical = p.tactical;
        b.recording = Some(p.recording);
        for _ in 0..500 {
            let ca = std::mem::take(&mut a.inbox);
            let cb = std::mem::take(&mut b.inbox);
            a.step(&ca);
            b.step(&cb);
        }
        assert_eq!(
            digest(a.world.as_ref().unwrap()),
            digest(b.world.as_ref().unwrap())
        );
        assert_eq!(digest(&a.tactical), digest(&b.tactical));
        assert_eq!(digest(&a.recording), digest(&b.recording));
        let r = b.recording.as_ref().unwrap();
        let w = b.world.as_ref().unwrap();
        assert!(r.events.len() > w.events.len());
        assert!(r.events.windows(2).all(|e| e[1].seq == e[0].seq + 1));
        assert!(r.verify(w).unwrap()["verified"].as_bool().unwrap());
        let archive = r.archive(w);
        assert_eq!(
            archive["frames"]["Blue"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["tick"],
            w.tick
        );
        assert!(archive["frames"]["Blue"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["units"]
                .as_array()
                .unwrap()
                .iter()
                .all(|u| u["side"] == "Blue")));
    }
    #[test]
    fn historical_save_checksum_and_recorded_core_replay_remain_valid(){
        let Some(path)=std::env::var_os("HARPOON_HISTORICAL_SAVE")else{return};
        let bytes=std::fs::read(&path).unwrap();let save:Save=serde_json::from_slice(&bytes).unwrap();
        assert_eq!(digest(&save.payload),save.checksum_fnv64,"new defaults may not rewrite the historical payload checksum");
        assert_eq!(save.payload.world.rules.profile,harpoon_core::rules::damage_runtime::RuleProfile::Legacy);
        let result=save.payload.recording.verify(&save.payload.world);
        assert!(result.is_ok(),"historical recorded core replay: {result:?}");
        assert_eq!(std::fs::read(path).unwrap(),bytes,"historical evidence is read-only");
    }
    #[test]
    fn damaged_carrier_save_preserves_casualty_clocks_and_command_replay() {
        let mut g=game();
        for slot in &mut g.tactical {slot.enabled=false;}
        let w=g.world.as_mut().unwrap();
        let id=w.entities.values().find(|e|e.side==Side::Blue).unwrap().id;
        for e in w.entities.values_mut() {e.fire_on=false;}
        let weapon=w.db.weapons["cn_yj18"].clone();
        harpoon_core::damage::apply_hit_at(w,id,&weapon,false,Some(90.0));
        assert!(w.entity(id).unwrap().ext["damage.fire_pct"]>0.0);
        g.recording=Some(Recording::new(w));
        let runtime=agent::Runtime::new();
        let payload=Payload{world:w.clone(),inbox:vec![],tactical:g.tactical.clone(),recording:g.recording.clone().unwrap(),agents:runtime.snapshot(),ai_supervision:g.ai_supervision.clone()};
        let save=Save{format:"harpoon-server-save".into(),version:VERSION,engine_compat:COMPAT.into(),label:"规则验收：真实命中后的航母损伤".into(),created_unix_ms:0,checksum_fnv64:digest(&payload),payload};
        let bytes=serde_json::to_vec(&save).unwrap();
        let restored:Save=serde_json::from_slice(&bytes).unwrap();
        assert_eq!(digest(&restored.payload),save.checksum_fnv64);
        assert!(restored.payload.world.rules.validate_tables().is_ok());
        let mut mismatched=restored.payload.world.rules.clone();
        mismatched.table_fingerprint=Some("different-rules".into());
        assert!(mismatched.validate_tables().is_err());
        if let Some(path)=std::env::var_os("HARPOON_DAMAGE_QA_SAVE") {std::fs::write(path,&bytes).unwrap();}
        let mut b=Game::new(restored.payload.world.db.clone());
        b.world=Some(restored.payload.world);
        b.tactical=restored.payload.tactical;
        b.recording=Some(restored.payload.recording);
        b.ai_supervision=restored.payload.ai_supervision;
        for _ in 0..600 {g.step(&[]);b.step(&[]);}
        assert_eq!(digest(g.world.as_ref().unwrap()),digest(b.world.as_ref().unwrap()));
        assert!(b.recording.as_ref().unwrap().verify(b.world.as_ref().unwrap()).unwrap()["verified"]==true);
    }
    #[test]
    fn manual_unit_override_survives_fsm_review_and_requires_explicit_resume() {
        let mut g=game();
        let id=g.world.as_ref().unwrap().entities.values().find(|e|e.side==Side::Blue).unwrap().id;
        let take:Command=serde_json::from_value(json!({"cmd":"set_unit_automation","unit":id,"enabled":false})).unwrap();
        g.step(&[(Side::Blue,take),(Side::Blue,Command::SetCourse{unit:id,course_deg:123.0})]);
        for _ in 0..60 {g.step(&[]);}
        let w=g.world.as_ref().unwrap();
        assert_eq!(w.entity(id).unwrap().ordered_course_deg,123.0,"FSM cannot overwrite a manual unit next window");
        assert_eq!(w.entity(id).unwrap().ext.get("control.manual"),Some(&1.0));
        assert!(!w.entities.values().any(|e|e.side==Side::Blue&&e.kind==harpoon_core::types::PlatformKind::Aircraft),"locked carrier must not silently launch new groups");
        let restored:World=serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap();
        assert_eq!(restored.entity(id).unwrap().ext.get("control.manual"),Some(&1.0));
        let (orders,notes)=g.supervise_ai(Side::Blue,vec![Command::SetCourse{unit:id,course_deg:0.0}]);
        assert!(orders.is_empty()&&!notes.is_empty(),"LLM task supervision also respects manual override");
        let resume:Command=serde_json::from_value(json!({"cmd":"set_unit_automation","unit":id,"enabled":true})).unwrap();
        g.step(&[(Side::Blue,resume)]);
        assert!(g.world.as_ref().unwrap().entity(id).unwrap().ext.get("control.manual").is_none());
        assert!(g.recording.as_ref().unwrap().verify(g.world.as_ref().unwrap()).unwrap()["verified"]==true);
    }
    #[test]
    fn takeover_cancels_previous_owners_delayed_navigation_but_keeps_new_human_batch() {
        let mut g=game();for t in &mut g.tactical{t.enabled=false;}
        let id=harpoon_core::EntityId(1);
        g.world.as_mut().unwrap().entity_mut(id).unwrap().ext.insert("damage.bridge_order_delay".into(),360.0);
        g.recording=Some(Recording::new(g.world.as_ref().unwrap()));
        g.step(&[(Side::Blue,Command::SetCourse{unit:id,course_deg:25.0})]);
        assert_eq!(g.world.as_ref().unwrap().rules.delayed_orders.len(),1);
        g.queue_player_command(Side::Blue,Command::ClearStation{unit:id}).unwrap();
        g.queue_player_command(Side::Blue,Command::SetCourse{unit:id,course_deg:123.0}).unwrap();
        assert_eq!(g.inbox.iter().filter(|(_,c)|matches!(c,Command::SetUnitAutomation{..})).count(),1,"one control transition per group, not per low-level order");
        let queued=std::mem::take(&mut g.inbox);g.step(&queued);
        let delayed=&g.world.as_ref().unwrap().rules.delayed_orders;
        assert_eq!(delayed.len(),2,"new human clear and course both retain real bridge delay");
        assert!(!delayed.iter().any(|o|matches!(o.command,Command::SetCourse{course_deg:25.0,..})));
        for _ in 0..361{g.step(&[]);}
        assert_eq!(g.world.as_ref().unwrap().entity(id).unwrap().ordered_course_deg,123.0);
        assert!(g.recording.as_ref().unwrap().verify(g.world.as_ref().unwrap()).unwrap()["verified"]==true);
    }
    #[tokio::test]
    async fn human_queue_validates_before_control_transfer_and_rejects_old_session() {
        let app=test_app();
        {
            let mut g=app.game.lock().unwrap();
            assert!(g.queue_player_command(Side::Red,Command::Stop{unit:harpoon_core::EntityId(1)}).is_err());
            assert!(g.inbox.is_empty(),"invalid foreign order must not transfer control or queue work");
            assert!(g.queue_player_command(Side::Blue,Command::SetSpeed{unit:harpoon_core::EntityId(1),speed_kts:-1.0}).is_err());
            assert!(g.inbox.is_empty());
        }
        let old=serde_json::from_value(json!({"side":"Blue","session_revision":40,"command":{"cmd":"stop","unit":1}})).unwrap();
        assert!(super::super::command_handler(State(app.clone()),Json(old)).await.is_err());
        assert!(app.game.lock().unwrap().inbox.is_empty());
        let valid=serde_json::from_value(json!({"side":"Blue","session_revision":41,"command":{"cmd":"stop","unit":1}})).unwrap();
        let _=super::super::command_handler(State(app.clone()),Json(valid)).await.unwrap();
        let mut g=app.game.lock().unwrap();
        assert!(g.world.as_ref().unwrap().entity(harpoon_core::EntityId(1)).unwrap().ext.get("control.manual").is_none(),"queued intent is not yet executed");
        let queued=std::mem::take(&mut g.inbox);g.step(&queued);
        assert_eq!(g.world.as_ref().unwrap().entity(harpoon_core::EntityId(1)).unwrap().ext.get("control.manual"),Some(&1.0));
    }
    #[test]
    fn navigation_preview_uses_public_land_without_mutating_game_or_taking_control(){
        let mut g=game();let id=harpoon_core::EntityId(1);
        g.world.as_mut().unwrap().context.terrain=Some(serde_json::from_value(json!({"source":"controlled public square","polygons":[{"outer":[{"x":4,"y":-2},{"x":6,"y":-2},{"x":6,"y":2},{"x":4,"y":2}]}]})).unwrap());
        let before=digest(g.world.as_ref().unwrap());
        let land=harpoon_core::Vec2::new(5.0,0.0);let goal=harpoon_core::Vec2::new(10.0,0.0);
        assert!(g.preview_navigation(Side::Blue,id,land).is_err());
        assert!(g.preview_navigation(Side::Red,id,goal).is_err());
        assert!(g.preview_navigation(Side::Blue,id,harpoon_core::Vec2::new(f64::NAN,0.0)).is_err());
        let route=g.preview_navigation(Side::Blue,id,goal).unwrap();assert!(route.len()>1,"preview must use actual coast detour, not a straight line");
        assert_eq!(route.last(),Some(&goal));assert_eq!(digest(g.world.as_ref().unwrap()),before);assert!(g.inbox.is_empty());
        assert!(g.queue_player_command(Side::Blue,Command::Goto{unit:id,waypoints:vec![land]}).is_err());
        assert_eq!(digest(g.world.as_ref().unwrap()),before);assert!(g.inbox.is_empty(),"invalid land destination must not cancel tasks or transfer control");
    }
    #[test]
    fn replay_detects_changed_orders_and_save_ids_cannot_traverse() {
        let mut g = game();
        for _ in 0..35 {
            g.step(&[]);
        }
        let mut r = g.recording.clone().unwrap();
        r.inputs.clear();
        assert!(r.verify(g.world.as_ref().unwrap()).is_err());
        for id in ["../world", "a/b", "a\\b", "", "C:world"] {
            assert!(path(id).is_err());
        }
    }
    #[test]
    fn restored_llm_memory_is_preserved_without_auto_run_or_pending_calls() {
        let r = agent::Runtime::new();
        let snapshot = r.snapshot();
        r.restore_snapshot(snapshot.clone(), "save-test", 19, 500)
            .unwrap();
        assert!(!r.any_busy());
        assert!(r.auto_run_speed().is_none());
        let after = r.snapshot();
        assert_eq!(snapshot[0]["memory"], after[0]["memory"]);
        assert_eq!(after[0]["enabled"], false);
    }

    fn test_app() -> AppState {
        let mut g = game();
        for slot in &mut g.tactical { slot.enabled = false; }
        let db = g.world.as_ref().unwrap().db.clone();
        let (tx, _) = tokio::sync::broadcast::channel(8);
        AppState { session_revision:Arc::new(AtomicU64::new(41)), advance_plans:Arc::new(Mutex::new(AdvancePlans::default())), game:Arc::new(Mutex::new(g)),
            scenario_dir:Arc::new(PathBuf::from("scenarios")), update_tx:Arc::new(tx), db:Arc::new(db),
            agents:Arc::new(agent::Runtime::new()), director:Arc::new(Mutex::new(director::State::default())),
            director_seq:Arc::new(AtomicU64::new(0)) }
    }

    #[tokio::test]
    async fn missing_or_stale_plan_identity_never_changes_world_or_queue() {
        for fields in [json!({}), json!({"plan_id":"old","expected_tick":1,"session_revision":41}),
            json!({"plan_id":"old-session","expected_tick":0,"session_revision":40})] {
            let app = test_app();
            let before = digest(app.game.lock().unwrap().world.as_ref().unwrap());
            let mut request = json!({"side":"Blue","ticks":1,"commands":[{"cmd":"stop","unit":1}]});
            request.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
            let result = advance(State(app.clone()), Json(serde_json::from_value(request).unwrap())).await;
            assert!(result.is_err(), "stale or missing plan identity accepted");
            let g = app.game.lock().unwrap();
            assert_eq!(digest(g.world.as_ref().unwrap()), before);
            assert!(g.inbox.is_empty());
        }
    }

    #[tokio::test]
    async fn duplicate_plan_id_cannot_advance_or_queue_again() {
        let app = test_app();
        let request = json!({"side":"Blue","ticks":1,"commands":[{"cmd":"stop","unit":1}],
            "plan_id":"one-shot","expected_tick":0,"session_revision":41});
        let first = advance(State(app.clone()), Json(serde_json::from_value(request.clone()).unwrap())).await;
        assert!(first.is_ok());
        let before = digest(app.game.lock().unwrap().world.as_ref().unwrap());
        let mut repeated = request;
        repeated["expected_tick"] = json!(1);
        let result = advance(State(app.clone()), Json(serde_json::from_value(repeated).unwrap())).await;
        assert!(result.is_err(), "a consumed plan was executed again");
        assert_eq!(digest(app.game.lock().unwrap().world.as_ref().unwrap()), before);
    }
}

//! In-game AI staff officer.
//!
//! The assistant is deliberately *not* a referee client. Every turn receives
//! the same double-blind `Observation` a human player gets, plus the public
//! scenario/database facts. Commands are validated against that observation
//! and then enter the ordinary command inbox, so the engine remains the only
//! authority that can change the world.
//!
//! Features:
//! - per-side long-term memory (summary + recent reports), persisted under
//!   `tmp/agents/` and never containing hidden enemy truth;
//! - scheduled reports every `cadence_s` simulated seconds (default 300);
//! - chat turns and manual report requests;
//! - advise / execute autonomy;
//! - an independent headless two-agent duel used for verification.

use harpoon_core::commands::{AirMission, Command};
use harpoon_core::db::Database;
use harpoon_core::events::Event;
use harpoon_core::observe::{Observation, UnitObs};
use harpoon_core::scenario::{load_scenario, scenario_from_json, Scenario};
use harpoon_core::types::{EntityId, Side};
use harpoon_core::world::{VictoryResult, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
const DEFAULT_MODEL: &str = "deepseek-flash";
const DEFAULT_CADENCE_S: i64 = 300;
const DEFAULT_MAX_TOKENS: u32 = 65_536;
const DEFAULT_TIMEOUT_S: u64 = 60;
const MAX_REPORTS: usize = 24;
const MAX_RECENT_FOR_PROMPT: usize = 3;
const MAX_MEMORY_TURNS: usize = 16;
const MAX_MEMORY_CHARS: usize = 2400;
const MAX_COMMANDS: usize = 16;
const MAX_EVENTS_FOR_PROMPT: usize = 40;
const MAX_CONTACTS_FOR_PROMPT: usize = 60;
const MEMORY_VERSION: u32 = 1;
const DUEL_ARTIFACT_DIR: &str = "tmp/agent-duels";

// ---------------------------------------------------------------------------
// Settings and per-side state
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Settings {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub cadence_s: i64,
    pub language: String,
    pub max_tokens: u32,
    pub timeout_s: u64,
}

fn env_first(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()))
}

impl Settings {
    fn from_env() -> Self {
        let cadence_s = env_first(&["HARPOON_AGENT_CADENCE_S"])
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(DEFAULT_CADENCE_S)
            .clamp(60, 3600);
        Self {
            base_url: env_first(&["HARPOON_AGENT_BASE_URL", "HARPOON_LLM_BASE_URL"])
                .unwrap_or_else(|| DEFAULT_BASE_URL.into())
                .trim_end_matches('/')
                .to_string(),
            model: env_first(&["HARPOON_AGENT_MODEL", "HARPOON_LLM_MODEL"])
                .unwrap_or_else(|| DEFAULT_MODEL.into()),
            api_key: env_first(&["HARPOON_AGENT_API_KEY", "HARPOON_LLM_API_KEY"]).unwrap_or_default(),
            cadence_s,
            language: env_first(&["HARPOON_AGENT_LANGUAGE"]).unwrap_or_else(|| "zh-CN".into()),
            max_tokens: env_first(&["HARPOON_AGENT_MAX_TOKENS"])
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(DEFAULT_MAX_TOKENS)
                .clamp(512, 65_536),
            timeout_s: env_first(&["HARPOON_AGENT_TIMEOUT_S"])
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(DEFAULT_TIMEOUT_S)
                .clamp(15, 120),
        }
    }

    fn configured(&self) -> bool {
        !self.api_key.trim().is_empty()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct MemoryTurn {
    tick: i64,
    report: String,
    intent: String,
    commands: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Memory {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    turns: Vec<MemoryTurn>,
    #[serde(default)]
    pub(crate) last_event_seq: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Usage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
    /// DeepSeek/OpenAI prompt-cache accounting, when provided.
    #[serde(default)]
    cache_hit_tokens: Option<u64>,
    #[serde(default)]
    cache_miss_tokens: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UsageStats {
    pub turns: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cache_hit_tokens: u64,
    pub cache_miss_tokens: u64,
}

/// One server-evaluated wake condition. The model may only use these
/// whitelisted predicates; it never supplies executable code.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WakeCondition {
    /// Fire when this many simulated seconds have elapsed since the last plan.
    Time { every_s: i64 },
    /// First observation of a contact (optionally filtered by category/quality).
    NewContact {
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        min_quality: Option<f64>,
    },
    /// A visible incoming-weapon event.
    Incoming {
        #[serde(default)]
        max_eta_s: Option<f64>,
    },
    /// Any own platform loss event.
    OwnLoss,
    /// Own damage/critical event above the selected materiality.
    OwnDamage {
        #[serde(default)]
        min_damage: Option<f64>,
        #[serde(default)]
        hp_fraction_below: Option<f64>,
    },
    /// A contact of the selected category/quality inside `max_nm` of own units.
    ContactRange {
        max_nm: f64,
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        min_quality: Option<f64>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WakeRule {
    pub id: String,
    #[serde(flatten)]
    pub condition: WakeCondition,
    #[serde(default)]
    pub cooldown_s: i64,
    #[serde(default)]
    pub one_shot: bool,
}

fn validate_wake_rule(rule: &WakeRule) -> Result<(), String> {
    if rule.id.trim().is_empty() {
        return Err("wake rule id must not be empty".into());
    }
    if !(0..=3600).contains(&rule.cooldown_s) {
        return Err("wake rule cooldown_s must be 0..3600".into());
    }
    match rule.condition {
        WakeCondition::Time { every_s } => {
            if !(120..=3600).contains(&every_s) {
                return Err("time wake every_s must be 120..3600".into());
            }
        }
        WakeCondition::NewContact { ref category, min_quality } => {
            validate_wake_filters(category, min_quality)?;
        }
        WakeCondition::Incoming { max_eta_s } => {
            if let Some(v) = max_eta_s {
                if !v.is_finite() || v < 0.0 {
                    return Err("incoming max_eta_s must be finite and >= 0".into());
                }
            }
        }
        WakeCondition::OwnLoss => {}
        WakeCondition::OwnDamage { min_damage, hp_fraction_below } => {
            if let Some(v) = min_damage {
                if !v.is_finite() || !(0.0..=10_000.0).contains(&v) {
                    return Err("own_damage min_damage must be finite 0..10000".into());
                }
            }
            if let Some(v) = hp_fraction_below {
                if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                    return Err("own_damage hp_fraction_below must be finite 0..1".into());
                }
            }
        }
        WakeCondition::ContactRange { max_nm, ref category, min_quality } => {
            if !max_nm.is_finite() || max_nm <= 0.0 || max_nm > 1000.0 {
                return Err("contact_range max_nm must be finite 0..1000".into());
            }
            validate_wake_filters(category, min_quality)?;
        }
    }
    Ok(())
}

fn validate_wake_filters(category: &Option<String>, min_quality: Option<f64>) -> Result<(), String> {
    if let Some(v) = min_quality {
        if !v.is_finite() || !(0.0..=1.0).contains(&v) {
            return Err("wake min_quality must be finite 0..1".into());
        }
    }
    if let Some(category) = category {
        let c = category.to_ascii_lowercase();
        if !["surface", "air", "subsurface", "sub", "missile", "torpedo", "land", "unknown"]
            .contains(&c.as_str())
        {
            return Err(format!("unsupported wake category `{category}`"));
        }
    }
    Ok(())
}

impl UsageStats {
    fn add(&mut self, usage: &Usage) {
        self.turns += 1;
        self.prompt_tokens += usage.prompt_tokens.unwrap_or(0);
        self.completion_tokens += usage.completion_tokens.unwrap_or(0);
        self.cache_hit_tokens += usage.cache_hit_tokens.unwrap_or(0);
        self.cache_miss_tokens += usage.cache_miss_tokens.unwrap_or(0);
    }

    fn value(&self) -> Value {
        let turns = self.turns.max(1);
        json!({
            "turns": self.turns,
            "prompt_tokens": self.prompt_tokens,
            "completion_tokens": self.completion_tokens,
            "cache_hit_tokens": self.cache_hit_tokens,
            "cache_miss_tokens": self.cache_miss_tokens,
            "avg_prompt_tokens": self.prompt_tokens as f64 / turns as f64,
            "avg_completion_tokens": self.completion_tokens as f64 / turns as f64,
            "avg_total_tokens": (self.prompt_tokens + self.completion_tokens) as f64 / turns as f64,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TurnReport {
    pub tick: i64,
    pub time_s: f64,
    pub trigger: String,
    pub report: String,
    pub intent: String,
    pub commands: Vec<Command>,
    pub rejected: Vec<String>,
    pub applied: bool,
    pub error: Option<String>,
    pub model: String,
    pub latency_ms: u64,
    #[serde(default)]
    pub usage: Usage,
    #[serde(default)]
    pub desired_speed: Option<f64>,
    #[serde(default)]
    pub wake_conditions: Vec<WakeRule>,
}

struct SideAgent {
    enabled: bool,
    autonomy: String,
    busy: bool,
    next_report_tick: i64,
    memory: Memory,
    reports: Vec<TurnReport>,
    last_proposals: Vec<Command>,
    last_error: Option<String>,
    last_turn_tick: Option<i64>,
    usage: UsageStats,
    auto_run: bool,
    auto_speed: f64,
    /// Last model-requested pace. Auto-run uses the minimum of both sides.
    desired_speed: Option<f64>,
}

impl SideAgent {
    fn new() -> Self {
        Self {
            enabled: false,
            autonomy: "advise".into(),
            busy: false,
            next_report_tick: i64::MAX,
            memory: Memory::default(),
            reports: Vec::new(),
            last_proposals: Vec::new(),
            last_error: None,
            last_turn_tick: None,
            usage: UsageStats::default(),
            auto_run: false,
            auto_speed: 15.0,
            desired_speed: None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct TurnContext {
    pub side: Side,
    pub trigger: String,
    pub settings: Settings,
    pub autonomy: String,
    pub memory: Memory,
    pub recent: Vec<TurnReport>,
    pub generation: u64,
    pub scenario_id: String,
    pub seed: u64,
    /// Simulated tick at which this turn was triggered (not when the LLM
    /// happened to finish; a slow call must not relabel a 5-minute report).
    pub tick: i64,
    /// True when this turn was requested by the two-sided director.
    pub director: bool,
    /// Wake rules active before this turn; the model may replace them.
    pub wake_rules: Vec<WakeRule>,
    /// Human-readable trigger reason for the director turn.
    pub trigger_detail: String,
}

#[derive(Clone, Debug)]
pub(crate) struct TurnDecision {
    pub report: String,
    pub memory: String,
    pub intent: String,
    pub commands: Vec<Command>,
    pub rejected: Vec<String>,
    pub error: Option<String>,
    pub usage: Usage,
    pub latency_ms: u64,
    pub desired_speed: Option<f64>,
    pub wake_conditions: Vec<WakeRule>,
}

#[derive(Debug)]
pub(crate) enum TurnError {
    Busy,
    NoGame,
    NotConfigured,
}

impl TurnError {
    pub(crate) fn message(&self) -> &'static str {
        match self {
            TurnError::Busy => "agent_busy",
            TurnError::NoGame => "no_game",
            TurnError::NotConfigured => "agent_not_configured",
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

struct Inner {
    settings: Settings,
    sides: [SideAgent; 2],
    generation: u64,
    scenario_id: Option<String>,
    seed: u64,
    current_tick: i64,
    duel: Option<DuelJob>,
    duel_seq: u64,
}

pub struct Runtime {
    inner: Mutex<Inner>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                settings: Settings::from_env(),
                sides: [SideAgent::new(), SideAgent::new()],
                generation: 0,
                scenario_id: None,
                seed: 0,
                current_tick: 0,
                duel: None,
                duel_seq: 0,
            }),
        }
    }

    /// Reset per-game memory when a new authoritative world is installed.
    /// Model/API settings are deliberately kept.
    pub fn start_game(&self, scenario_id: &str, seed: u64, tick: i64) {
        let mut inner = self.inner.lock().unwrap();
        inner.generation = inner.generation.wrapping_add(1);
        inner.scenario_id = Some(scenario_id.to_string());
        inner.seed = seed;
        inner.current_tick = tick;
        for side in [Side::Blue, Side::Red] {
            let slot = &mut inner.sides[side.index()];
            slot.busy = false;
            slot.last_proposals.clear();
            slot.last_error = None;
            slot.last_turn_tick = None;
            slot.memory = load_memory(scenario_id, seed, side).unwrap_or_default();
            slot.reports = load_reports(scenario_id, seed, side)
                .unwrap_or_default()
                .into_iter()
                .filter(|r| r.error.is_none())
                .rev()
                .take(MAX_REPORTS)
                .collect();
            slot.usage = load_usage(scenario_id, seed, side).unwrap_or_default();
            slot.desired_speed = None;
            slot.next_report_tick = if slot.enabled { tick + 1 } else { i64::MAX };
        }
    }

    pub(crate) fn generation(&self) -> u64 {
        self.inner.lock().unwrap().generation
    }

    /// The server-side autopilot speed requested by an enabled side. The
    /// browser does not need to stay open for time to advance.
    pub(crate) fn is_execute(&self, side: Side) -> bool {
        let inner = self.inner.lock().unwrap();
        let slot = &inner.sides[side.index()];
        slot.enabled && slot.autonomy == "execute"
    }

    pub(crate) fn auto_run_speed(&self) -> Option<f64> {
        let inner = self.inner.lock().unwrap();
        if !inner.settings.configured() {
            return None;
        }
        let mut speeds: Vec<f64> = Vec::new();
        for side in [Side::Blue, Side::Red] {
            let slot = &inner.sides[side.index()];
            if slot.enabled && slot.auto_run {
                let speed = slot.desired_speed.unwrap_or(slot.auto_speed).clamp(5.0, 60.0);
                speeds.push(speed);
            }
        }
        speeds.into_iter().reduce(f64::min)
    }

    pub fn due_sides(&self, tick: i64) -> Vec<Side> {
        let mut inner = self.inner.lock().unwrap();
        inner.current_tick = tick;
        if !inner.settings.configured() {
            return Vec::new();
        }
        [Side::Blue, Side::Red]
            .into_iter()
            .filter(|side| {
                let slot = &inner.sides[side.index()];
                slot.enabled && !slot.busy && slot.next_report_tick <= tick
            })
            .collect()
    }

    pub(crate) fn prepare_turn(
        &self,
        side: Side,
        trigger: &str,
        current_tick: i64,
    ) -> Result<TurnContext, TurnError> {
        let mut inner = self.inner.lock().unwrap();
        if !inner.settings.configured() {
            return Err(TurnError::NotConfigured);
        }
        let settings = inner.settings.clone();
        let scenario_id = inner.scenario_id.clone().ok_or(TurnError::NoGame)?;
        let seed = inner.seed;
        let generation = inner.generation;
        let slot = &mut inner.sides[side.index()];
        if slot.busy {
            return Err(TurnError::Busy);
        }
        slot.busy = true;
        slot.last_error = None;
        slot.last_turn_tick = Some(current_tick);
        if trigger == "scheduled" {
            slot.next_report_tick = current_tick.saturating_add(settings.cadence_s);
        }
        Ok(TurnContext {
            side,
            trigger: trigger.to_string(),
            settings,
            autonomy: slot.autonomy.clone(),
            memory: slot.memory.clone(),
            recent: slot.reports.iter().take(MAX_RECENT_FOR_PROMPT).cloned().collect(),
            generation,
            scenario_id,
            seed,
            tick: current_tick,
            director: false,
            wake_rules: Vec::new(),
            trigger_detail: String::new(),
        })
    }

    /// Prepare a director turn. The director does not use the per-side
    /// scheduled cadence; it pauses both sides and asks them to plan.
    pub(crate) fn prepare_director_turn(
        &self,
        side: Side,
        tick: i64,
        wake_rules: Vec<WakeRule>,
        trigger_detail: String,
    ) -> Result<TurnContext, TurnError> {
        let mut ctx = self.prepare_turn(side, "wake", tick)?;
        ctx.director = true;
        ctx.wake_rules = wake_rules;
        ctx.trigger_detail = trigger_detail;
        Ok(ctx)
    }

    pub(crate) fn finish_turn(
        &self,
        ctx: &TurnContext,
        decision: TurnDecision,
        applied: bool,
        last_event_seq: u64,
    ) -> TurnReport {
        let mut inner = self.inner.lock().unwrap();
        let current_tick = ctx.tick;
        let game_tick = inner.current_tick;
        let cadence = inner.settings.cadence_s;
        let generation = inner.generation;
        let scenario_id = inner.scenario_id.clone().unwrap_or_default();
        let seed = inner.seed;
        let report = TurnReport {
            tick: current_tick,
            time_s: current_tick as f64,
            trigger: ctx.trigger.clone(),
            report: decision.report.clone(),
            intent: decision.intent.clone(),
            commands: decision.commands.clone(),
            rejected: decision.rejected.clone(),
            applied,
            error: decision.error.clone(),
            model: ctx.settings.model.clone(),
            latency_ms: decision.latency_ms,
            usage: decision.usage.clone(),
            desired_speed: decision.desired_speed,
            wake_conditions: decision.wake_conditions.clone(),
        };
        if generation != ctx.generation {
            return report;
        }
        let slot = &mut inner.sides[ctx.side.index()];
        slot.usage.add(&decision.usage);
        if decision.error.is_none() {
            if let Some(speed) = decision.desired_speed {
                slot.desired_speed = Some(speed.clamp(5.0, 60.0));
            }
        }
        slot.busy = false;
        if ctx.trigger == "scheduled" && game_tick > ctx.tick.saturating_add(cadence) {
            // Do not replay every missed 5-minute boundary back-to-back when a
            // slow model falls behind a fast game clock.
            slot.next_report_tick = game_tick.saturating_add(cadence);
        }
        slot.last_error = decision.error.clone();
        slot.last_proposals = if decision.commands.is_empty() {
            slot.last_proposals.clone()
        } else {
            decision.commands.clone()
        };
        if decision.error.is_none() {
            let summary = if decision.memory.trim().is_empty() {
                slot.memory.summary.clone()
            } else {
                truncate_chars(decision.memory.trim(), MAX_MEMORY_CHARS)
            };
            slot.memory.summary = summary;
            slot.memory.last_event_seq = slot.memory.last_event_seq.max(last_event_seq);
            slot.memory.turns.push(MemoryTurn {
                tick: current_tick,
                report: truncate_chars(&decision.report, 500),
                intent: decision.intent.clone(),
                commands: decision.commands.iter().map(command_short).collect(),
            });
            if slot.memory.turns.len() > MAX_MEMORY_TURNS {
                let excess = slot.memory.turns.len() - MAX_MEMORY_TURNS;
                slot.memory.turns.drain(0..excess);
            }
        }
        slot.reports.insert(0, report.clone());
        if slot.reports.len() > MAX_REPORTS {
            slot.reports.truncate(MAX_REPORTS);
        }
        let _ = save_memory(&scenario_id, seed, ctx.side, &slot.memory, &slot.reports, &slot.usage);
        report
    }

    pub(crate) fn abort_turn(&self, ctx: &TurnContext, error: String) -> TurnReport {
        let mut inner = self.inner.lock().unwrap();
        let current_tick = ctx.tick;
        let report = TurnReport {
            tick: current_tick,
            time_s: current_tick as f64,
            trigger: ctx.trigger.clone(),
            report: String::new(),
            intent: String::new(),
            commands: Vec::new(),
            rejected: Vec::new(),
            applied: false,
            error: Some(error.clone()),
            model: ctx.settings.model.clone(),
            latency_ms: 0,
            usage: Usage::default(),
            desired_speed: None,
            wake_conditions: Vec::new(),
        };
        if inner.generation == ctx.generation {
            let slot = &mut inner.sides[ctx.side.index()];
            slot.busy = false;
            slot.last_error = Some(error);
        }
        report
    }

    pub(crate) fn last_proposals(&self, side: Side) -> Vec<Command> {
        self.inner.lock().unwrap().sides[side.index()].last_proposals.clone()
    }

    pub fn set_config(
        &self,
        req: &AgentConfigReq,
        current_tick: i64,
    ) -> Result<Value, String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(side) = &req.side {
            if Side::parse(side).is_none() {
                return Err("side must be Blue or Red".into());
            }
        }
        let target_sides: Vec<Side> = match &req.side {
            Some(side) => vec![Side::parse(side).unwrap()],
            None => vec![Side::Blue, Side::Red],
        };
        if let Some(v) = &req.model {
            if v.trim().is_empty() {
                return Err("model must not be empty".into());
            }
            inner.settings.model = v.trim().to_string();
        }
        if let Some(v) = &req.base_url {
            let v = v.trim().trim_end_matches('/');
            if !(v.starts_with("http://") || v.starts_with("https://")) {
                return Err("base_url must start with http:// or https://".into());
            }
            inner.settings.base_url = v.to_string();
        }
        if let Some(v) = &req.api_key {
            inner.settings.api_key = v.trim().to_string();
        }
        if let Some(v) = req.cadence_s {
            if !(60..=3600).contains(&v) {
                return Err("cadence_s must be between 60 and 3600".into());
            }
            inner.settings.cadence_s = v;
        }
        if let Some(v) = req.max_tokens {
            if !(512..=65_536).contains(&v) {
                return Err("max_tokens must be between 512 and 65536".into());
            }
            inner.settings.max_tokens = v;
        }
        if let Some(v) = &req.language {
            if v != "zh-CN" && v != "en" {
                return Err("language must be zh-CN or en".into());
            }
            inner.settings.language = v.clone();
        }
        if let Some(v) = &req.autonomy {
            if v != "advise" && v != "execute" {
                return Err("autonomy must be advise or execute".into());
            }
            for side in &target_sides {
                inner.sides[side.index()].autonomy = v.clone();
            }
        }
        if let Some(enabled) = req.enabled {
            if enabled && !inner.settings.configured() {
                return Err("LLM API key is not configured; set HARPOON_AGENT_API_KEY or POST api_key".into());
            }
            for side in &target_sides {
                let slot = &mut inner.sides[side.index()];
                let was_enabled = slot.enabled;
                slot.enabled = enabled;
                if enabled && !was_enabled && !slot.busy {
                    slot.next_report_tick = current_tick + 1;
                }
                if !enabled {
                    slot.next_report_tick = i64::MAX;
                    slot.auto_run = false;
                }
            }
        }
        if let Some(v) = req.auto_speed {
            if !(1.0..=60.0).contains(&v) {
                return Err("auto_speed must be between 1 and 60".into());
            }
            for side in &target_sides {
                inner.sides[side.index()].auto_speed = v;
            }
        }
        if let Some(auto) = req.auto_run {
            for side in &target_sides {
                let slot = &mut inner.sides[side.index()];
                if auto && !slot.enabled {
                    return Err("enable the agent before enabling auto-run".into());
                }
                slot.auto_run = auto;
            }
        }
        inner.current_tick = current_tick;
        Ok(inner.status_value())
    }

    pub fn reset(&self, purge_memory: bool, current_tick: i64) -> Value {
        let mut inner = self.inner.lock().unwrap();
        let scenario_id = inner.scenario_id.clone().unwrap_or_default();
        let seed = inner.seed;
        inner.generation = inner.generation.wrapping_add(1);
        for side in [Side::Blue, Side::Red] {
            let slot = &mut inner.sides[side.index()];
            slot.busy = false;
            slot.memory = Memory::default();
            slot.reports.clear();
            slot.last_proposals.clear();
            slot.last_error = None;
            slot.last_turn_tick = None;
            slot.usage = UsageStats::default();
            slot.desired_speed = None;
            slot.next_report_tick = if slot.enabled { current_tick + 1 } else { i64::MAX };
            if purge_memory && !scenario_id.is_empty() {
                let _ = std::fs::remove_file(memory_path(&scenario_id, seed, side));
            }
        }
        inner.status_value()
    }

    pub fn status(&self) -> Value {
        self.inner.lock().unwrap().status_value()
    }

    pub fn side_status(&self, side: Side) -> Value {
        let inner = self.inner.lock().unwrap();
        inner.side_value(side)
    }

    // --- duel job ---------------------------------------------------------

    pub fn duel_status(&self) -> Value {
        let inner = self.inner.lock().unwrap();
        match &inner.duel {
            Some(job) => job.public_value(),
            None => json!({"status": "idle"}),
        }
    }

    fn begin_duel(&self, job: DuelJob) -> Result<u64, String> {
        let mut inner = self.inner.lock().unwrap();
        if inner.duel.as_ref().is_some_and(|d| d.status == "running") {
            return Err("a duel is already running".into());
        }
        if !inner.settings.configured() {
            return Err("LLM API key is not configured".into());
        }
        inner.duel_seq = inner.duel_seq.wrapping_add(1);
        let id = inner.duel_seq;
        let mut job = job;
        job.id = id;
        inner.duel = Some(job);
        Ok(id)
    }

    fn update_duel(
        &self,
        id: u64,
        tick: i64,
        reports: &BTreeMap<String, Vec<TurnReport>>,
        commands: &BTreeMap<String, u64>,
        usage: &BTreeMap<String, UsageStats>,
        errors: &[String],
    ) {
        let mut inner = self.inner.lock().unwrap();
        let Some(job) = inner.duel.as_mut().filter(|j| j.id == id) else { return };
        job.tick = tick;
        job.reports = reports.clone();
        job.commands = commands.clone();
        job.usage = usage.clone();
        job.errors = errors.to_vec();
    }

    fn finish_duel(
        &self,
        id: u64,
        world: &World,
        reports: BTreeMap<String, Vec<TurnReport>>,
        commands: BTreeMap<String, u64>,
        usage: BTreeMap<String, UsageStats>,
        errors: Vec<String>,
        artifact: String,
    ) {
        let mut inner = self.inner.lock().unwrap();
        let Some(job) = inner.duel.as_mut().filter(|j| j.id == id) else { return };
        job.tick = world.tick;
        job.status = "completed".into();
        job.finished_at_unix = now_unix();
        job.result = world.result.clone();
        job.vp = Some(json!({"blue": world.victory_points().0, "red": world.victory_points().1}));
        job.kills = Some(json!({"blue": world.kill_counts().0, "red": world.kill_counts().1}));
        job.reports = reports;
        job.commands = commands;
        job.usage = usage;
        job.errors = errors;
        job.artifact = Some(artifact);
    }

    fn fail_duel(&self, id: u64, error: String) {
        let mut inner = self.inner.lock().unwrap();
        let Some(job) = inner.duel.as_mut().filter(|j| j.id == id) else { return };
        job.status = "failed".into();
        job.finished_at_unix = now_unix();
        job.errors.push(error);
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Status serialization
// ---------------------------------------------------------------------------

impl Inner {
    fn status_value(&self) -> Value {
        json!({
            "configured": self.settings.configured(),
            "model": self.settings.model,
            "base_url": self.settings.base_url,
            "cadence_s": self.settings.cadence_s,
            "max_tokens": self.settings.max_tokens,
            "language": self.settings.language,
            "current_tick": self.current_tick,
            "scenario_id": self.scenario_id,
            "seed": self.seed,
            "generation": self.generation,
            "sides": {
                "Blue": self.side_value(Side::Blue),
                "Red": self.side_value(Side::Red),
            },
            "duel": self.duel.as_ref().map(DuelJob::public_value).unwrap_or_else(|| json!({"status": "idle"})),
        })
    }

    fn side_value(&self, side: Side) -> Value {
        let slot = &self.sides[side.index()];
        let reports: Vec<Value> = slot.reports.iter().take(8).cloned().map(|r| serde_json::to_value(r).unwrap_or(Value::Null)).collect();
        json!({
            "side": side,
            "enabled": slot.enabled,
            "autonomy": slot.autonomy,
            "busy": slot.busy,
            "next_report_tick": slot.next_report_tick,
            "next_report_in_s": if slot.next_report_tick == i64::MAX { Value::Null } else { json!((slot.next_report_tick - self.current_tick).max(0)) },
            "last_turn_tick": slot.last_turn_tick,
            "last_error": slot.last_error,
            "memory": slot.memory.summary,
            "memory_turns": slot.memory.turns.len(),
            "auto_run": slot.auto_run,
            "auto_speed": slot.auto_speed,
            "desired_speed": slot.desired_speed,
            "usage": slot.usage.value(),
            "reports": reports,
            "last_proposals": slot.last_proposals,
            "cadence_s": self.settings.cadence_s,
            "max_tokens": self.settings.max_tokens,
        })
    }
}

// ---------------------------------------------------------------------------
// Public request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AgentConfigReq {
    pub side: Option<String>,
    pub enabled: Option<bool>,
    pub autonomy: Option<String>,
    pub cadence_s: Option<i64>,
    pub max_tokens: Option<u32>,
    pub auto_run: Option<bool>,
    pub auto_speed: Option<f64>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AgentResetReq {
    #[serde(default)]
    pub purge_memory: bool,
}

#[derive(Debug, Deserialize)]
pub struct ChatReq {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct DuelReq {
    pub scenario: String,
    pub seed: Option<u64>,
    pub max_ticks: Option<i64>,
    pub cadence_s: Option<i64>,
}

#[derive(Clone)]
struct DuelJob {
    id: u64,
    scenario: String,
    seed: u64,
    status: String,
    tick: i64,
    duration_s: i64,
    cadence_s: i64,
    model: String,
    base_url: String,
    started_at_unix: u64,
    finished_at_unix: u64,
    result: Option<VictoryResult>,
    vp: Option<Value>,
    kills: Option<Value>,
    reports: BTreeMap<String, Vec<TurnReport>>,
    commands: BTreeMap<String, u64>,
    usage: BTreeMap<String, UsageStats>,
    errors: Vec<String>,
    artifact: Option<String>,
    message: Option<String>,
}

impl DuelJob {
    fn public_value(&self) -> Value {
        let reports: BTreeMap<String, Vec<Value>> = self
            .reports
            .iter()
            .map(|(k, v)| (k.clone(), v.iter().take(6).cloned().map(|r| serde_json::to_value(r).unwrap_or(Value::Null)).collect()))
            .collect();
        json!({
            "id": self.id,
            "scenario": self.scenario,
            "seed": self.seed,
            "status": self.status,
            "tick": self.tick,
            "time_s": self.tick,
            "duration_s": self.duration_s,
            "progress": if self.duration_s > 0 { (self.tick as f64 / self.duration_s as f64).clamp(0.0, 1.0) } else { 0.0 },
            "cadence_s": self.cadence_s,
            "model": self.model,
            "base_url": self.base_url,
            "started_at_unix": self.started_at_unix,
            "finished_at_unix": self.finished_at_unix,
            "result": self.result,
            "vp": self.vp,
            "kills": self.kills,
            "commands": self.commands,
            "usage": self.usage,
            "reports": reports,
            "errors": self.errors,
            "artifact": self.artifact,
            "message": self.message,
        })
    }
}

// ---------------------------------------------------------------------------
// Turn execution (blocking LLM call, called from spawn_blocking)
// ---------------------------------------------------------------------------

pub(crate) fn execute_turn(
    ctx: &TurnContext,
    obs: &Observation,
    db: &Database,
    player_message: Option<&str>,
) -> TurnDecision {
    let started = Instant::now();
    let system = build_system_prompt(ctx);
    let user = build_user_prompt(ctx, obs, db, player_message);
    let reply = match call_llm(&ctx.settings, &system, &user) {
        Ok(r) => r,
        Err(e) => {
            return TurnDecision {
                report: String::new(),
                memory: ctx.memory.summary.clone(),
                intent: String::new(),
                commands: Vec::new(),
                rejected: Vec::new(),
                error: Some(e),
                usage: Usage::default(),
                latency_ms: started.elapsed().as_millis() as u64,
                desired_speed: None,
                wake_conditions: Vec::new(),
            };
        }
    };
    let mut usage = reply.usage;
    let mut decision = parse_decision(&reply.content, obs, ctx.side);
    if decision.error.as_deref() == Some("model output was not valid JSON") {
        // One retry is cheap compared with losing a five-minute turn. The
        // retry prompt is deliberately short and asks only for valid JSON.
        let retry_user = format!(
            "{user}

Previous response was not valid JSON. Return only one complete JSON object; no markdown or prose."
        );
        if let Ok(retry) = call_llm(&ctx.settings, &system, &retry_user) {
            usage.prompt_tokens = Some(usage.prompt_tokens.unwrap_or(0) + retry.usage.prompt_tokens.unwrap_or(0));
            usage.completion_tokens = Some(usage.completion_tokens.unwrap_or(0) + retry.usage.completion_tokens.unwrap_or(0));
            usage.cache_hit_tokens = Some(usage.cache_hit_tokens.unwrap_or(0) + retry.usage.cache_hit_tokens.unwrap_or(0));
            usage.cache_miss_tokens = Some(usage.cache_miss_tokens.unwrap_or(0) + retry.usage.cache_miss_tokens.unwrap_or(0));
            decision = parse_decision(&retry.content, obs, ctx.side);
        }
    }
    decision.usage = usage;
    decision.latency_ms = started.elapsed().as_millis() as u64;
    if decision.report.trim().is_empty() {
        decision.report = if ctx.settings.language == "en" {
            "The staff returned no readable report.".into()
        } else {
            "参谋本次没有返回可读简报。".into()
        };
    }
    if decision.memory.trim().is_empty() {
        decision.memory = ctx.memory.summary.clone();
    }
    decision.report = truncate_chars(&decision.report, 6000);
    decision.memory = truncate_chars(&decision.memory, MAX_MEMORY_CHARS);
    decision
}

struct LlmReply {
    content: String,
    usage: Usage,
}

fn call_llm(settings: &Settings, system: &str, user: &str) -> Result<LlmReply, String> {
    if !settings.configured() {
        return Err("LLM API key is not configured".into());
    }
    let url = format!("{}/chat/completions", settings.base_url.trim_end_matches('/'));
    let body = json!({
        "model": settings.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ],
        "temperature": 0.25,
        "max_tokens": settings.max_tokens,
        "stream": false
    });
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(settings.timeout_s))
        .build();
    let response = agent
        .post(&url)
        .set("Authorization", &format!("Bearer {}", settings.api_key))
        .set("Content-Type", "application/json")
        .send_json(body);
    match response {
        Ok(resp) => {
            let value: Value = resp
                .into_json()
                .map_err(|e| format!("LLM response was not JSON: {e}"))?;
            let content = value
                .pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .ok_or_else(|| "LLM response did not contain message.content".to_string())?
                .to_string();
            if value.pointer("/choices/0/finish_reason").and_then(Value::as_str) == Some("length") {
                return Err("LLM output truncated (finish_reason=length); retry with a shorter report".into());
            }
            let usage = Usage {
                prompt_tokens: value.pointer("/usage/prompt_tokens").and_then(Value::as_u64),
                completion_tokens: value.pointer("/usage/completion_tokens").and_then(Value::as_u64),
                cache_hit_tokens: value
                    .pointer("/usage/prompt_cache_hit_tokens")
                    .and_then(Value::as_u64)
                    .or_else(|| value.pointer("/usage/prompt_tokens_details/cached_tokens").and_then(Value::as_u64)),
                cache_miss_tokens: value
                    .pointer("/usage/prompt_cache_miss_tokens")
                    .and_then(Value::as_u64),
            };
            Ok(LlmReply { content, usage })
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            Err(format!(
                "LLM HTTP {code}: {}",
                truncate_chars(body.trim(), 400)
            ))
        }
        Err(e) => Err(format!("LLM request failed: {e}")),
    }
}

fn parse_decision(content: &str, obs: &Observation, side: Side) -> TurnDecision {
    let Some(value) = extract_json(content) else {
        let _ = std::fs::create_dir_all("tmp/agents");
        let _ = std::fs::write(format!("tmp/agents/raw-{}.txt", side.name()), content);
        return TurnDecision {
            report: truncate_chars(content, 4000),
            memory: String::new(),
            intent: String::new(),
            commands: Vec::new(),
            rejected: vec!["model output was not valid JSON".into()],
            error: Some("model output was not valid JSON".into()),
            usage: Usage::default(),
            latency_ms: 0,
            desired_speed: None,
            wake_conditions: Vec::new(),
        };
    };
    let report = value.get("report").and_then(Value::as_str).unwrap_or("").to_string();
    let memory = value.get("memory").and_then(Value::as_str).unwrap_or("").to_string();
    let intent = value.get("intent").and_then(Value::as_str).unwrap_or("").to_string();
    let raw_commands: Vec<Value> = value
        .get("commands")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let (commands, mut rejected) = parse_and_validate_commands(&raw_commands, obs, side);
    let desired_speed = value
        .get("desired_speed")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(5.0, 60.0));
    let mut wake_conditions = Vec::new();
    let mut wake_ids = std::collections::BTreeSet::new();
    if let Some(list) = value.get("wake_conditions").and_then(Value::as_array) {
        for item in list {
            match serde_json::from_value::<WakeRule>(item.clone()) {
                Ok(rule) => {
                    if !wake_ids.insert(rule.id.clone()) {
                        rejected.push(format!("duplicate wake rule id `{}`", rule.id));
                        continue;
                    }
                    match validate_wake_rule(&rule) {
                        Ok(()) => wake_conditions.push(rule),
                        Err(e) => rejected.push(format!("wake rule `{}`: {e}", rule.id)),
                    }
                }
                Err(e) => rejected.push(format!("invalid wake rule {item}: {e}")),
            }
        }
    }
    let error = if value.get("error").and_then(Value::as_str).is_some_and(|s| !s.is_empty()) {
        value.get("error").and_then(Value::as_str).map(str::to_string)
    } else {
        None
    };
    TurnDecision {
        report,
        memory,
        intent,
        commands,
        rejected,
        error,
        usage: Usage::default(),
        latency_ms: 0,
        desired_speed,
        wake_conditions,
    }
}

fn extract_json(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    let unfenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim()
        .strip_suffix("```")
        .unwrap_or(trimmed)
        .trim();
    if let Ok(v) = serde_json::from_str::<Value>(unfenced) {
        return Some(v);
    }
    let repaired = remove_trailing_commas(unfenced);
    if let Ok(v) = serde_json::from_str::<Value>(&repaired) {
        return Some(v);
    }
    let start = unfenced.find('{')?;
    let end = unfenced.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<Value>(&remove_trailing_commas(&unfenced[start..=end])).ok()
}

/// Models occasionally leave a trailing comma before a closing brace.
/// Removing it is safe for JSON and saves a whole 5-minute turn.
fn remove_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, ch) in chars.iter().enumerate() {
        if *ch == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && matches!(chars[j], '}' | ']') {
                continue;
            }
        }
        out.push(*ch);
    }
    out
}

/// Public (crate) validation for proposals that did not just come from the
/// model, e.g. an advise-mode proposal the player chooses to execute.
pub(crate) fn validate_commands(
    obs: &Observation,
    side: Side,
    commands: &[Command],
) -> (Vec<Command>, Vec<String>) {
    let mut valid = Vec::new();
    let mut rejected = Vec::new();
    for command in commands.iter().take(MAX_COMMANDS) {
        match validate_command(obs, side, command) {
            Ok(()) => valid.push(command.clone()),
            Err(e) => rejected.push(format!("{}: {e}", command_short(command))),
        }
    }
    (valid, rejected)
}

fn parse_and_validate_commands(
    raw: &[Value],
    obs: &Observation,
    side: Side,
) -> (Vec<Command>, Vec<String>) {
    let mut commands = Vec::new();
    let mut rejected = Vec::new();
    for value in raw.iter().take(MAX_COMMANDS * 2) {
        if commands.len() >= MAX_COMMANDS {
            rejected.push(format!("超过每回合 {MAX_COMMANDS} 条命令上限"));
            break;
        }
        let normalized = normalize_command_value(value);
        let parsed: Command = match serde_json::from_value(normalized.clone()) {
            Ok(c) => c,
            Err(e) => {
                rejected.push(format!("无法解析命令 {}: {e}", truncate_chars(&normalized.to_string(), 180)));
                continue;
            }
        };
        match validate_command(obs, side, &parsed) {
            Ok(()) => commands.push(parsed),
            Err(e) => rejected.push(format!("{}: {e}", command_short(&parsed))),
        }
    }
    (commands, rejected)
}

fn validate_command(obs: &Observation, side: Side, command: &Command) -> Result<(), String> {
    let own = |id: EntityId| -> Result<&UnitObs, String> {
        let u = obs
            .units
            .iter()
            .find(|u| u.id == id && u.side == side)
            .ok_or_else(|| format!("单位 #{} 不属于本方或已不在态势中", id.0))?;
        Ok(u)
    };
    let contact = |id: harpoon_core::types::ContactId| -> Result<(), String> {
        if obs.contacts.iter().any(|c| c.id == id) {
            Ok(())
        } else {
            Err(format!("接触 #{} 不在本方态势中", id.0))
        }
    };
    let finite = |v: f64, name: &str| -> Result<(), String> {
        if v.is_finite() {
            Ok(())
        } else {
            Err(format!("{name} 必须是有限数值"))
        }
    };
    match command {
        Command::SetCourse { unit, course_deg } => {
            own(*unit)?;
            finite(*course_deg, "course_deg")?;
        }
        Command::SetSpeed { unit, speed_kts } => {
            own(*unit)?;
            finite(*speed_kts, "speed_kts")?;
            if !(0.0..=2000.0).contains(speed_kts) {
                return Err("speed_kts 必须在 0..2000".into());
            }
        }
        Command::SetDepth { unit, depth_m } => {
            own(*unit)?;
            finite(*depth_m, "depth_m")?;
            if !(10.0..=1000.0).contains(depth_m) {
                return Err("depth_m 必须在 10..1000".into());
            }
        }
        Command::SetAltitude { unit, altitude_m } => {
            own(*unit)?;
            finite(*altitude_m, "altitude_m")?;
            if !(15.0..=30000.0).contains(altitude_m) {
                return Err("altitude_m 必须在 15..30000".into());
            }
        }
        Command::Goto { unit, waypoints } => {
            own(*unit)?;
            if waypoints.is_empty() || waypoints.len() > 24 {
                return Err("waypoints 数量必须在 1..24".into());
            }
            for p in waypoints {
                finite(p.x, "waypoint.x")?;
                finite(p.y, "waypoint.y")?;
            }
        }
        Command::Stop { unit } => {
            own(*unit)?;
        }
        Command::Follow { unit, leader, bearing_rel_deg, distance_nm } => {
            own(*unit)?;
            own(*leader)?;
            if unit == leader {
                return Err("不能跟随自己".into());
            }
            finite(*bearing_rel_deg, "bearing_rel_deg")?;
            finite(*distance_nm, "distance_nm")?;
            if !(0.0..=100.0).contains(distance_nm) {
                return Err("distance_nm 必须在 0..100".into());
            }
        }
        Command::ClearStation { unit } => {
            own(*unit)?;
        }
        Command::SetSonar { unit, .. } => {
            own(*unit)?;
        }
        Command::SetWeaponsFree { unit, .. } => {
            own(*unit)?;
        }
        Command::SetAutoChaff { unit, .. } => {
            own(*unit)?;
        }
        Command::AssignTarget { unit, contact: c } => {
            own(*unit)?;
            contact(*c)?;
        }
        Command::ClearTarget { unit } => {
            own(*unit)?;
        }
        Command::Fire { unit, contact: c, weapon, rounds } => {
            let u = own(*unit)?;
            contact(*c)?;
            let mount = u
                .weapons
                .iter()
                .find(|w| &w.weapon == weapon)
                .ok_or_else(|| format!("单位没有武器 {weapon}"))?;
            if mount.ammo == 0 {
                return Err(format!("{weapon} 弹药耗尽"));
            }
            if *rounds == 0 || u16::from(*rounds) > mount.ammo {
                return Err(format!("rounds 必须在 1..{}", mount.ammo));
            }
        }
        Command::DeployChaff { unit } => {
            let u = own(*unit)?;
            if u.chaff == 0 {
                return Err("箔条已耗尽".into());
            }
        }
        Command::LaunchAircraft { unit, class, count, mission } => {
            let u = own(*unit)?;
            let deck = u.deck.as_ref().ok_or_else(|| "该单位没有飞行甲板".to_string())?;
            if *count == 0 || *count > 24 {
                return Err("count 必须在 1..24".into());
            }
            let ready = deck.ready.iter().find(|(c, n)| c == class && *n > 0);
            let cycling = deck.cycling.iter().any(|(c, _, _)| c == class);
            if ready.is_none() && !cycling {
                return Err(format!("甲板上没有可用机型 {class}"));
            }
            match mission {
                AirMission::Cap { station, radius_nm } => {
                    finite(station.x, "station.x")?;
                    finite(station.y, "station.y")?;
                    finite(*radius_nm, "radius_nm")?;
                }
                AirMission::Strike { contact: c } => contact(*c)?,
                AirMission::Asw { area, radius_nm } => {
                    finite(area.x, "area.x")?;
                    finite(area.y, "area.y")?;
                    finite(*radius_nm, "radius_nm")?;
                }
                AirMission::Recon { point } => {
                    finite(point.x, "point.x")?;
                    finite(point.y, "point.y")?;
                }
                AirMission::Rtb => {}
            }
        }
        Command::RecoverAircraft { unit } => {
            let u = own(*unit)?;
            if u.deck.is_none() {
                return Err("该单位没有飞行甲板".into());
            }
        }
    }
    Ok(())
}

/// LLMs frequently wrap a point as `[x, y]`, use `station` for a recon
/// point, or omit a default `rounds`. Accept those harmless shape variants
/// without weakening engine validation: ids, ownership, ranges and mission
/// types are still checked afterwards.
fn normalize_command_value(value: &Value) -> Value {
    let mut v = value.clone();
    let Some(obj) = v.as_object_mut() else { return v };
    if !obj.contains_key("cmd") {
        if let Some(tag) = obj.get("type").and_then(Value::as_str) {
            obj.insert("cmd".into(), Value::String(tag.to_string()));
        }
    }
    let cmd = obj.get("cmd").and_then(Value::as_str).unwrap_or("").to_string();
    match cmd.as_str() {
        "goto" => {
            if let Some(points) = obj.get_mut("waypoints").and_then(Value::as_array_mut) {
                for point in points {
                    normalize_point(point);
                }
            }
        }
        "launch_aircraft" => {
            if !obj.contains_key("class") {
                for key in ["aircraft", "aircraft_class", "class_id"] {
                    if let Some(value) = obj.get(key).cloned() {
                        obj.insert("class".into(), value);
                        break;
                    }
                }
            }
            if let Some(mission) = obj.get_mut("mission") {
                normalize_mission(mission);
            }
        }
        "fire" => {
            if !obj.contains_key("weapon") {
                for key in ["weapon_id", "weaponId", "mount"] {
                    if let Some(value) = obj.get(key).cloned() {
                        obj.insert("weapon".into(), value);
                        break;
                    }
                }
            }
            if !obj.contains_key("rounds") {
                obj.insert("rounds".into(), json!(1));
            }
        }
        _ => {}
    }
    v
}

fn normalize_point(value: &mut Value) {
    if let Some(pair) = value.as_array() {
        if pair.len() >= 2 {
            let x = pair[0].clone();
            let y = pair[1].clone();
            *value = json!({"x": x, "y": y});
        }
    }
}

fn normalize_mission(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else { return };
    for key in ["station", "area", "point"] {
        if let Some(point) = obj.get_mut(key) {
            normalize_point(point);
        }
    }
    let tag = obj.get("mission").and_then(Value::as_str).unwrap_or("").to_string();
    match tag.as_str() {
        "recon" => {
            if !obj.contains_key("point") {
                if let Some(point) = obj.get("station").or_else(|| obj.get("area")).cloned() {
                    obj.insert("point".into(), point);
                }
            }
            obj.remove("station");
            obj.remove("area");
            obj.remove("radius_nm");
        }
        "cap" => {
            if !obj.contains_key("station") {
                if let Some(point) = obj.get("point").or_else(|| obj.get("area")).cloned() {
                    obj.insert("station".into(), point);
                }
            }
            if !obj.contains_key("radius_nm") {
                obj.insert("radius_nm".into(), json!(25.0));
            }
            obj.remove("point");
            obj.remove("area");
        }
        "asw" => {
            if !obj.contains_key("area") {
                if let Some(point) = obj.get("station").or_else(|| obj.get("point")).cloned() {
                    obj.insert("area".into(), point);
                }
            }
            if !obj.contains_key("radius_nm") {
                obj.insert("radius_nm".into(), json!(20.0));
            }
            obj.remove("station");
            obj.remove("point");
        }
        _ => {}
    }
}

fn command_short(c: &Command) -> String {
    serde_json::to_string(c).unwrap_or_else(|_| "<command>".into())
}

// ---------------------------------------------------------------------------
// Prompt construction
// ---------------------------------------------------------------------------

fn build_system_prompt(ctx: &TurnContext) -> String {
    let zh = ctx.settings.language == "zh-CN";
    let rules = if zh {
        r#"## 游戏模型
- 1 tick = 1 模拟秒；每 300 模拟秒（5 分钟）你主动汇报一次。当前是 RTS 改编，不是桌游回合制。
- 你与玩家看到完全相同的双盲态势：只包含本方单位、本方接触估计、本方可见事件。绝不能声称知道未探测到的敌军真态、隐藏舰位、隐藏损伤或未确认战果。
- 接触有质量、最后更新时间、陈旧标记和估计误差。陈旧接触不等于当前真态。identified 只代表型号识别，不代表精确位置。
- 命令在下一 tick 生效；移动/转向/起飞/回收/冷却需要模拟时间。排队回执不是战果。
- 观察中的 speed_kts/course_deg 是当前实际值，ordered_* 是已排队命令值；暂停或加速过程中二者可以不同。不要仅因实际值未变就重复下令。
- 已在空中的飞机也会出现在 own_units 中。先核对甲板 ready 与在空兵力，不要重复放飞已经足够的机群；AEW 用于搜索/预警，攻击机用于反舰突击。
- 自动选敌（weapons free）允许火控自行交战；assign_target 是持续指派；fire 只打一次。反舰导弹、鱼雷、防空导弹有飞行时间、末段拦截、箔条/诱饵、命中概率和损伤。
- 航空：飞行甲板有 ready/cycling；launch_aircraft 需要甲板上有机型；CAP 守空、ASW 反潜、Recon 侦察、Strike 对接触突击、Rtb 返航。飞机有续航，放飞后要记得召回/回收。
- 潜艇：set_depth 改深，主动声纳会暴露自己；鱼雷适合近距伏击。导弹射程远，但需要可靠接触与火控。
- 击沉/击落敌平台计入杀伤数；导弹和鱼雷不计；飞机燃油迫降不计为对方击杀。终局以权威结果为准，平时只能看到本方确认击杀与本方损失。
"#
    } else {
        r#"## Game model
- One tick is one simulated second; you report proactively every 300 simulated seconds (5 minutes). This is an RTS adaptation, not the tabletop turn sequence.
- You see exactly the same double-blind picture as the human player: own units, own contact estimates and own visible events. Never claim knowledge of undetected enemy truth, hidden positions, hidden damage or unconfirmed kills.
- Contacts carry quality, last-seen age, stale flags and estimate error. A stale contact is not current truth. Identification is not a precise position.
- Orders apply on the next tick; movement, turns, launch/recovery and reloads take simulated time. A queued receipt is not a result.
- In the observation, speed_kts/course_deg are current actual values and ordered_* are queued command values. They can differ while paused or accelerating; do not repeat an order merely because the actual value has not changed yet.
- Aircraft already airborne appear in own_units. Check deck ready counts and airborne strength before launching more; use AEW for search/picket and strike aircraft for anti-ship strikes.
- Weapons free lets fire control engage; assign_target is a standing assignment; fire is one volley. Missiles, torpedoes and SAMs have flight time, terminal interception, chaff/decoys, hit probability and damage.
- Air: the deck tracks ready/cycling aircraft. launch_aircraft needs an available class; CAP, ASW, Recon, Strike and Rtb are supported. Aircraft have endurance; recover them.
- Submarines: set_depth changes depth; active sonar reveals you. Torpedoes are close-range ambush weapons. Missiles have long range but need a usable contact and fire control.
- Destroyed enemy platforms count as kills; missiles and torpedoes do not; aircraft lost to fuel do not count as enemy kills. The authoritative result is revealed at the end; during play you see only your own confirmed kills and losses.
"#
    };
    let mcp = if zh {
        r#"## MCP 与引擎接口
MCP 工具（与游戏内命令同一套权威接口）：list_scenarios、new_game{scenario,seed}、get_state{side,since,max_events}、send_command{side,command}、set_time{speed}、step{ticks}、get_events{side,since}、run_selfplay{scenario,seed,max_ticks}。
你不是 MCP 客户端；你已经是游戏内参谋。请直接返回 commands 数组，服务端会用同一 Command 枚举校验并送进权威引擎。不要尝试调用外部 MCP 工具、不要编造工具结果。"#
    } else {
        r#"## MCP and engine interface
MCP tools (the same authoritative surface): list_scenarios, new_game{scenario,seed}, get_state{side,since,max_events}, send_command{side,command}, set_time{speed}, step{ticks}, get_events{side,since}, run_selfplay{scenario,seed,max_ticks}.
You are not an external MCP client; you are the in-game staff. Return commands directly; the server validates them with the same Command enum and submits them to the authoritative engine. Do not claim to have called external tools."#
    };
    let schema = if zh {
        r#"## 命令 JSON（cmd 标签；坐标必须写成 {"x":数值,"y":数值}，单位/接触 ID 用观察中的数字）
{"cmd":"set_course","unit":1,"course_deg":90}
{"cmd":"set_speed","unit":1,"speed_kts":24}
{"cmd":"set_depth","unit":2,"depth_m":120}
{"cmd":"set_altitude","unit":3,"altitude_m":6000}
{"cmd":"goto","unit":1,"waypoints":[{"x":330,"y":350}]}
{"cmd":"stop","unit":1}
{"cmd":"follow","unit":1,"leader":2,"bearing_rel_deg":0,"distance_nm":12}
{"cmd":"clear_station","unit":1}
{"cmd":"set_sonar","unit":2,"active":false}
{"cmd":"set_weapons_free","unit":1,"free":true}
{"cmd":"set_auto_chaff","unit":1,"on":true}
{"cmd":"assign_target","unit":1,"contact":10}
{"cmd":"clear_target","unit":1}
{"cmd":"fire","unit":1,"contact":10,"weapon":"cn_yj18","rounds":2}
{"cmd":"deploy_chaff","unit":1}
{"cmd":"launch_aircraft","unit":5,"class":"cn_j15","count":2,"mission":{"mission":"cap","station":{"x":350,"y":355},"radius_nm":15}}
{"cmd":"launch_aircraft","unit":5,"class":"cn_j15","count":2,"mission":{"mission":"strike","contact":10}}
{"cmd":"launch_aircraft","unit":5,"class":"cn_z18j","count":1,"mission":{"mission":"recon","point":{"x":430,"y":350}}}
{"cmd":"launch_aircraft","unit":5,"class":"cn_z18f","count":2,"mission":{"mission":"asw","area":{"x":430,"y":350},"radius_nm":20}}
{"cmd":"recover_aircraft","unit":5}
"#
    } else {
        r#"## Command JSON (cmd tag; points MUST be {"x":number,"y":number}; use numeric own-unit/contact ids from the observation)
{"cmd":"set_course","unit":1,"course_deg":90}
{"cmd":"set_speed","unit":1,"speed_kts":24}
{"cmd":"set_depth","unit":2,"depth_m":120}
{"cmd":"set_altitude","unit":3,"altitude_m":6000}
{"cmd":"goto","unit":1,"waypoints":[{"x":330,"y":350}]}
{"cmd":"stop","unit":1}
{"cmd":"follow","unit":1,"leader":2,"bearing_rel_deg":0,"distance_nm":12}
{"cmd":"clear_station","unit":1}
{"cmd":"set_sonar","unit":2,"active":false}
{"cmd":"set_weapons_free","unit":1,"free":true}
{"cmd":"set_auto_chaff","unit":1,"on":true}
{"cmd":"assign_target","unit":1,"contact":10}
{"cmd":"clear_target","unit":1}
{"cmd":"fire","unit":1,"contact":10,"weapon":"cn_yj18","rounds":2}
{"cmd":"deploy_chaff","unit":1}
{"cmd":"launch_aircraft","unit":5,"class":"fighter","count":2,"mission":{"mission":"cap","station":{"x":350,"y":355},"radius_nm":15}}
{"cmd":"launch_aircraft","unit":5,"class":"fighter","count":2,"mission":{"mission":"strike","contact":10}}
{"cmd":"launch_aircraft","unit":5,"class":"aew","count":1,"mission":{"mission":"recon","point":{"x":430,"y":350}}}
{"cmd":"launch_aircraft","unit":5,"class":"helo_asw","count":2,"mission":{"mission":"asw","area":{"x":430,"y":350},"radius_nm":20}}
{"cmd":"recover_aircraft","unit":5}
"#
    };
    let report_language = if zh {
        "用简体中文写 report；命令 JSON 保持英文字段。报告先给结论，再给依据，最后给下一步；不要输出思维链。"
    } else {
        "Write the report in English; keep command JSON field names in English. Lead with the conclusion, then evidence, then next actions; do not reveal private chain-of-thought."
    };
        let director_block = if ctx.director {
        r#"## Director / wake mode
When the turn trigger is `wake`, both sides are paused at the same tick and plan simultaneously. Return:
- `desired_speed`: 5–60 simulated seconds per real second. The director advances at the **minimum** of both sides' requested speeds.
- `wake_conditions`: the conditions that should pause both sides again. They are evaluated only against your own observation. A time wake is a legitimate adversarial tool but keep a time fallback.
Allowed wake rule types (JSON):
  {"id":"time","type":"time","every_s":300,"cooldown_s":0,"one_shot":false}
  {"id":"contact","type":"new_contact","category":"surface","min_quality":0.5,"cooldown_s":60,"one_shot":false}
  {"id":"incoming","type":"incoming","max_eta_s":120,"cooldown_s":0,"one_shot":false}
  {"id":"loss","type":"own_loss","cooldown_s":0,"one_shot":false}
  {"id":"damage","type":"own_damage","min_damage":20,"hp_fraction_below":0.5,"cooldown_s":0,"one_shot":false}
  {"id":"range","type":"contact_range","max_nm":80,"category":"surface","min_quality":0.5,"cooldown_s":30,"one_shot":false}
Do not invent other condition types. Always leave a time fallback. A private trigger pauses both sides, so consider the information/tempo trade-off.
"#
    } else {
        r#"## Strict scheduled mode
This is a fixed five-minute planning window. Both sides plan from their own double-blind observation at the same simulated tick; private events never pause the clock. Return only a `desired_speed` (5–60) for the next segment. Do not return `wake_conditions` in this mode.
"#
    };

format!(
        r#"You are the in-game AI staff officer (参谋) for the {side} side in HARPOON RTS. You are competent, concise and intellectually honest about uncertainty. You serve the human commander, but when the game is running in autonomous execute mode you may issue valid orders directly.

{rules}

{mcp}

{schema}

## Mission-specific guidance
- The kill-count exercise rewards destroying more enemy platforms than the opponent. Detect, close or use stand-off weapons, and keep your carriers alive. Do not wait for a perfect picture; use what you have and preserve a reserve.
- At every scheduled turn, if you have a usable contact and a valid weapon, consider weapons free / assign_target / fire; if the enemy is undetected, move sensors and aircraft to find them.
- Never invent unit ids, contact ids, weapon ids or coordinates. If a command is uncertain, omit it rather than hallucinate.
- In advise mode your commands are proposals; in execute mode the server will apply every valid command. Keep each turn's list small and mutually consistent.

## Decisive fleet battle doctrine
- In a kill-count or fleet-decisive scenario, no patch of water is worth holding: find the enemy, fix the contact, and destroy platforms.
- Prefer enemy carriers, then the escorts and aircraft that enable their strikes; mass missiles in coordinated salvos instead of trickling them into Aegis.
- Keep your own carriers functional and use AEW/CAP to preserve the picture. Submarines are for close ambush; aircraft are for search, CAP, ASW and stand-off strike.
- A submarine inside torpedo range (about 22 nm for Yu-10 / Mk 48) is your highest-value attack: fire a 2-4 torpedo salvo in the same turn. Single torpedoes are usually seduced by carrier decoys; do not leave an in-range submarine idle in favour of another CAP rotation.
- Do not fire without a usable track and fire-control solution, and do not sacrifice the last carrier air group for a marginal damage roll.

## Ammunition discipline
- Each contact shows `own_in_flight`: your own rounds already targeting it. Before firing, check it. As a rule do not push a contact past ~4 ASMs in flight per window; every extra round into an already-saturated target is a wasted magazine.
- Carriers and Aegis escorts bleed salvos dry with SAM, RAM, CIWS and decoys. Saturate deliberately (one coordinated window, several shooters), then hold fire and reassess with BDA before re-attacking.
- Keep a reserve: do not empty a submarine's torpedoes or a shooter's missiles in the first hour unless it is sinking the target. A platform with empty magazines is a target.
- Aircraft only carry a few ASMs; bring them home and re-arm instead of dribbling single rounds.

## Protect your AEW and fight the submarine war
- The AEW aircraft is your radar picture. Keep it 25-40 nm behind your CAP line, away from known enemy fighters; if enemy fighters close within ~30 nm of the AEW, recall it or vector CAP onto them. An AEW sniped over the fleet is a self-inflicted blindness.
- When you hold a submarine contact (category Sub), react: launch ASW helos (mission asw) over the contact, vector ASW-capable ships, and weapons-free ASW rockets inside range. Enemy submarines sink your carriers for heavy points.
- Your own submarines hunt: transit toward the enemy's last known position, and once inside torpedo range of a tracked ship, fire a 2-4 torpedo salvo. Do not trail the fleet passively.
- Surface ships and submarines usually score far more than aircraft — see `victory.kill_weights` in the observation. Shooting down aircraft alone will lose a weighted battle; kill hulls.

{director_block}

## Output contract
Return exactly one JSON object, no markdown fences:
{{
  "intent": "one-sentence commander intent",
  "report": "concise staff briefing for the human commander at the current simulated time (<= 260 Chinese chars / 180 English words)",
  "memory": "updated long-term memory summary (mission progress, own losses, confirmed kills, standing plans, lessons; no hidden enemy truth; <= 900 chars)",
  "commands": [ ... zero or more command objects; at most 16 ... ],
  "desired_speed": 15,
  "wake_conditions": [ ... zero or more wake rule objects ... ],
  "error": null
}}
{report_language}
"#,
        side = ctx.side.name(),
        director_block = director_block,
        rules = rules,
        mcp = mcp,
        schema = schema,
        report_language = report_language,
    )
}

fn build_user_prompt(
    ctx: &TurnContext,
    obs: &Observation,
    db: &Database,
    player_message: Option<&str>,
) -> String {
    let zh = ctx.settings.language == "zh-CN";
    let mission = mission_text(obs, zh);
    let force = serde_json::to_string(&force_data(obs, db)).unwrap_or_else(|_| "{}".into());
    let contact_classes = serde_json::to_string(&contact_data(obs, db)).unwrap_or_else(|_| "[]".into());
    let mut events: Vec<&Event> = obs.events.iter().collect();
    if events.len() > MAX_EVENTS_FOR_PROMPT {
        events = events.split_off(events.len() - MAX_EVENTS_FOR_PROMPT);
    }
    let event_values: Vec<Value> = events
        .iter()
        .map(|e| serde_json::to_value(e).unwrap_or(Value::Null))
        .collect();
    let units: Vec<Value> = obs
        .units
        .iter()
        .map(|u| {
            json!({
                "id": u.id.0, "name": u.name, "class": u.class, "kind": u.kind,
                "pos": {"x": round1(u.pos.x), "y": round1(u.pos.y)},
                "course_deg": round1(u.course_deg), "speed_kts": round1(u.speed_kts),
                "ordered_course_deg": round1(u.ordered_course_deg), "ordered_speed_kts": round1(u.ordered_speed_kts),
                "depth_m": round1(u.depth_m), "altitude_m": round1(u.altitude_m),
                "hp": round1(u.hp), "hp_max": round1(u.hp_max),
                "fire_on": u.fire_on, "assigned": u.assigned.map(|c| c.0),
                "weapons": u.weapons.iter().map(|w| json!({"weapon":w.weapon,"name":w.name,"ammo":w.ammo,"cooldown_s":round1(w.cooldown_s)})).collect::<Vec<_>>(),
                "sensors": u.sensors.iter().map(|s| json!({"sensor":s.sensor,"name":s.name,"active":s.active,"vs_air":s.vs_air,"vs_surface":s.vs_surface,"vs_sub":s.vs_sub})).collect::<Vec<_>>(),
                "chaff": u.chaff, "decoys": u.decoys,
                "deck": u.deck.as_ref().map(|d| json!({"ready": d.ready, "cycling": d.cycling})),
                "mission": u.mission,
                "systems": u.systems,
                "below_layer": u.below_layer, "battery": u.battery, "towed_deployed": u.towed_deployed,
            })
        })
        .collect();
    let mut contact_refs: Vec<&harpoon_core::observe::ContactObs> = obs.contacts.iter().collect();
    contact_refs.sort_by(|a, b| {
        a.stale
            .cmp(&b.stale)
            .then_with(|| b.quality.partial_cmp(&a.quality).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.last_seen_s.partial_cmp(&b.last_seen_s).unwrap_or(std::cmp::Ordering::Equal))
    });
    contact_refs.truncate(MAX_CONTACTS_FOR_PROMPT);
    let contacts: Vec<Value> = contact_refs
        .into_iter()
        .map(|c| {
            json!({
                "id": c.id.0, "category": c.category, "class_name": c.class_name,
                "estimate": {"x": round1(c.estimate.x), "y": round1(c.estimate.y)},
                "est_course_deg": round1(c.est_course_deg), "est_speed_kts": round1(c.est_speed_kts),
                "quality": round2(c.quality), "bearing_only": c.bearing_only,
                "last_seen_s": round1(c.last_seen_s), "stale": c.stale, "sensors": c.sensors,
            })
        })
        .collect();
    let situation = json!({
        "tick": obs.tick,
        "time_s": obs.time_s,
        "time_hhmmss": fmt_hms(obs.time_s as i64),
        "scenario_id": obs.scenario_id,
        "scenario_name": obs.scenario_name,
        "scenario_description": obs.scenario_description,
        "your_side": obs.your_side,
        "over": obs.over,
        "result": obs.result,
        "victory": obs.victory,
        "mission": obs.mission,
        "weather": obs.weather,
        "day_night": obs.day_night,
        "director": ctx.director,
        "director_trigger": ctx.trigger_detail,
        "current_wake_rules": ctx.wake_rules,
        "own_units": units,
        "own_contacts": contacts,
        "visible_events": event_values,
    });
    let history: Vec<Value> = ctx
        .recent
        .iter()
        .rev()
        .map(|r| {
            json!({
                "tick": r.tick,
                "time": fmt_hms(r.time_s as i64),
                "trigger": r.trigger,
                "report": truncate_chars(&r.report, 450),
                "intent": r.intent,
                "commands": r.commands,
                "applied": r.applied,
                "error": r.error,
            })
        })
        .collect();
    let memory = if ctx.memory.summary.trim().is_empty() {
        if zh { "（尚无长期记忆，本回合建立）".to_string() } else { "(no long-term memory yet; establish it this turn)".to_string() }
    } else {
        ctx.memory.summary.clone()
    };
    let turn_request = match (&ctx.trigger[..], player_message) {
        ("chat", Some(msg)) => {
            if zh {
                format!("玩家发来消息：\n{}\n\n请回答玩家，并在需要时给出命令。", truncate_chars(msg, 4000))
            } else {
                format!("The player says:\n{}\n\nAnswer the player and issue commands if needed.", truncate_chars(msg, 4000))
            }
        }
        ("wake", _) => {
            if zh { "导演器唤醒：双方已同时暂停。请给出简报、命令、期望推进速度（5–60）和下一组唤醒条件；时间唤醒是合法的对抗手段，但请留下兜底时间条件。".to_string() } else { "Director wake: both sides are paused. Give a briefing, commands, a desired speed (5–60), and the next wake rules. A time wake is a legitimate adversarial tool, but keep a time fallback.".to_string() }
        }
        ("manual", _) => {
            if zh { "玩家要求立即汇报。请给出当前态势简报和命令建议。".to_string() } else { "The player requested an immediate report. Give a current staff briefing and recommended orders.".to_string() }
        }
        _ => {
            if zh { "这是每 5 分钟一次的定时参谋汇报。请更新长期记忆，报告态势、战果/损失、下一步计划，并下达你认为必要的命令。".to_string() } else { "This is your scheduled five-minute staff report. Update long-term memory, report the situation, kills/losses, next plan, and issue any necessary orders.".to_string() }
        }
    };
    format!(
        r#"MISSION AND PUBLIC DATA:
{mission}

OWN FORCE CLASS DATA (public database; current unit state is in the observation):
{force}

CONTACT CLASS DATA (only identified classes):
{contacts}

CURRENT PLAYER-VIEW OBSERVATION (the only tactical picture you may use):
{situation}

LONG-TERM MEMORY (your own notes from earlier turns):
{memory}

RECENT STAFF REPORTS (oldest to newest):
{history}

TURN REQUEST:
{turn_request}

Remember: never use hidden enemy truth. Return one JSON object with intent, report, memory, commands, error.
"#,
        mission = mission,
        force = force,
        contacts = contact_classes,
        situation = serde_json::to_string(&situation).unwrap_or_default(),
        memory = memory,
        history = serde_json::to_string(&history).unwrap_or_default(),
        turn_request = turn_request,
    )
}

fn human_duration(seconds: i64, zh: bool) -> String {
    let s = seconds.max(1);
    if s >= 3600 {
        let h = s as f64 / 3600.0;
        if zh {
            if (h - h.round()).abs() < 0.01 { format!("{:.0} 小时", h) } else { format!("{:.1} 小时", h) }
        } else if (h - h.round()).abs() < 0.01 {
            format!("{:.0} hours", h)
        } else {
            format!("{:.1} hours", h)
        }
    } else {
        let m = ((s + 30) / 60).max(1);
        if zh { format!("{m} 分钟") } else { format!("{m} minutes") }
    }
}

fn mission_text(obs: &Observation, zh: bool) -> String {
    let remaining = obs.victory.as_ref().map(|v| v.remaining_s).unwrap_or(0);
    let clock = fmt_hms(remaining);
    if let Some(m) = &obs.mission {
        let defender = m.config.defender;
        return if zh {
            format!(
                "## 剧本胜利条件\n剧本「{}」。区域拒止演习：剩余 {}。防守方 {}，任务区圆心 ({:.0},{:.0})，半径 {:.0} 海里，连续占区 {} 秒。本方可用航母 {}/{}。观察中 own_hold_s={:?}。\n{}",
                obs.scenario_name, clock, defender, m.config.center.x, m.config.center.y,
                m.config.radius_nm, m.config.hold_s, m.own_functional, m.own_total,
                m.own_hold_s, obs.scenario_description
            )
        } else {
            format!(
                "## Scenario victory conditions\nScenario \"{}\". Area-denial exercise, remaining {}. Defender {}. Circle center ({:.0},{:.0}), radius {:.0} nm, continuous hold {} s. Own functional carriers {}/{}. Observation own_hold_s={:?}.\n{}",
                obs.scenario_name, clock, defender, m.config.center.x, m.config.center.y,
                m.config.radius_nm, m.config.hold_s, m.own_functional, m.own_total,
                m.own_hold_s, obs.scenario_description
            )
        };
    }
    let Some(v) = &obs.victory else {
        return if zh { "## 剧本胜利条件\n按想定终局裁决。".into() } else { "## Scenario victory conditions\nResolved by the scenario's end condition.".into() };
    };
    if v.kill_count_at_end {
        return if zh {
            format!(
                "## 剧本胜利条件
剧本「{}」，{}击杀数决胜。剩余 {}（模拟时间）。到 {} 时，击毁敌方平台更多的一方获胜；相等为平局。分数按平台计数：舰艇、潜艇、飞机、基地各计 1；导弹/鱼雷不计；燃油迫降不计为敌方击杀。当前本方确认击杀 {}，本方损失 {}。敌方真实损失在终局前不公开。
{}",
                obs.scenario_name, human_duration(v.duration_s, true), clock, human_duration(v.duration_s, true), v.own_confirmed_kills, v.own_losses, obs.scenario_description
            )
        } else {
            format!(
                "## Scenario victory conditions
Scenario \"{}\" is decided by kill count. Remaining {}. At {} the side with more destroyed enemy platforms wins; equal counts are a draw. Score counts platforms: ships, submarines, aircraft and bases each count 1; missiles/torpedoes do not; aircraft lost to fuel do not count as enemy kills. Own confirmed kills {}, own losses {}. The enemy's true losses stay hidden until the end.
{}",
                obs.scenario_name, clock, human_duration(v.duration_s, false), v.own_confirmed_kills, v.own_losses, obs.scenario_description
            )
        };
    }
    if zh {
        format!(
            "## 剧本胜利条件\n剧本「{}」，剩余 {}。提前胜利 VP 差额 {}；到期按 VP 比较。当前本方确认击杀 {}，本方损失 {}。\n{}",
            obs.scenario_name, clock, v.target_vp, v.own_confirmed_kills, v.own_losses, obs.scenario_description
        )
    } else {
        format!(
            "## Scenario victory conditions\nScenario \"{}\", remaining {}. Early-win VP margin {}; at the deadline compare VP. Own confirmed kills {}, own losses {}.\n{}",
            obs.scenario_name, clock, v.target_vp, v.own_confirmed_kills, v.own_losses, obs.scenario_description
        )
    }
}

fn force_data(obs: &Observation, db: &Database) -> Value {
    let mut classes: BTreeMap<String, Value> = BTreeMap::new();
    for u in &obs.units {
        classes.entry(u.class.clone()).or_insert_with(|| platform_card(db, &u.class));
    }
    json!({"classes": classes})
}

fn contact_data(obs: &Observation, db: &Database) -> Value {
    let mut identified: BTreeMap<String, Value> = BTreeMap::new();
    for c in &obs.contacts {
        if let Some(class) = &c.class_name {
            if !identified.contains_key(class) {
                identified.insert(class.clone(), platform_card(db, class));
            }
        }
    }
    json!({
        "identified_class_public_data": identified,
        "note": "Contact positions are estimates. Unknown class_name means not identified; do not substitute a guessed class from the database.",
    })
}

fn platform_card(db: &Database, class_id: &str) -> Value {
    let Some(p) = db.platforms.get(class_id) else {
        return json!({"id": class_id, "note": "class not present in current database"});
    };
    let weapons: Vec<Value> = p
        .weapons
        .iter()
        .map(|fit| {
            let Some(w) = db.weapons.get(&fit.weapon) else {
                return json!({"id": fit.weapon, "mounts": fit.mounts, "ammo_per_mount": fit.ammo, "note":"weapon data missing"});
            };
            json!({
                "id": w.id, "name": w.name, "kind": w.kind, "guidance": w.guidance,
                "targets": w.targets, "range_nm": w.range_nm, "min_range_nm": w.min_range_nm,
                "speed_kts": w.speed_kts, "pk": w.pk, "salvo": w.salvo, "reload_s": w.reload_s,
                "damage": w.damage, "ir": w.ir, "directors": w.directors,
                "mounts": fit.mounts, "ammo_per_mount": fit.ammo,
            })
        })
        .collect();
    let sensors: Vec<Value> = p
        .sensors
        .iter()
        .map(|id| {
            let Some(s) = db.sensors.get(id) else {
                return json!({"id": id, "note":"sensor data missing"});
            };
            json!({
                "id": s.id, "name": s.name, "kind": s.kind,
                "air_range_nm": s.air_range_nm, "surface_range_nm": s.surface_range_nm,
                "sub_range_nm": s.sub_range_nm, "missile_range_nm": s.missile_range_nm,
                "alt_m": s.alt_m, "radar_class": s.radar_class,
            })
        })
        .collect();
    json!({
        "id": p.id, "name": p.name, "kind": p.kind, "hp": p.hp,
        "max_speed_kts": p.max_speed_kts, "cruise_speed_kts": p.cruise_speed_kts,
        "turn_rate_deg_s": p.turn_rate_deg_s, "accel_kts_per_s": p.accel_kts_per_s,
        "rcs": p.rcs, "vp_value": p.vp_value, "noise_db": p.noise_db,
        "max_depth_m": p.max_depth_m, "endurance_s": p.endurance_s,
        "dipping_sonar": p.dipping_sonar, "chaff": p.chaff, "decoys": p.decoys,
        "weapons": weapons, "sensors": sensors,
        "deck": p.deck.as_ref().map(|d| json!({"capacity":d.capacity,"cycle_s":d.cycle_s,"wing":d.wing})),
    })
}

// ---------------------------------------------------------------------------
// Memory persistence (no secrets, no hidden truth)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct MemoryFile {
    version: u32,
    scenario_id: String,
    seed: u64,
    side: Side,
    memory: Memory,
    #[serde(default)]
    reports: Vec<TurnReport>,
    #[serde(default)]
    usage: UsageStats,
}

fn memory_path(scenario_id: &str, seed: u64, side: Side) -> PathBuf {
    let safe: String = scenario_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    PathBuf::from("tmp/agents").join(format!("{}_{}_{}.json", safe, seed, side.name().to_ascii_lowercase()))
}

fn load_memory(scenario_id: &str, seed: u64, side: Side) -> Option<Memory> {
    let text = std::fs::read_to_string(memory_path(scenario_id, seed, side)).ok()?;
    let file: MemoryFile = serde_json::from_str(&text).ok()?;
    if file.version != MEMORY_VERSION || file.scenario_id != scenario_id || file.seed != seed || file.side != side {
        return None;
    }
    Some(file.memory)
}

fn load_reports(scenario_id: &str, seed: u64, side: Side) -> Option<Vec<TurnReport>> {
    let text = std::fs::read_to_string(memory_path(scenario_id, seed, side)).ok()?;
    let file: MemoryFile = serde_json::from_str(&text).ok()?;
    if file.version != MEMORY_VERSION || file.scenario_id != scenario_id || file.seed != seed || file.side != side {
        return None;
    }
    Some(file.reports)
}

fn load_usage(scenario_id: &str, seed: u64, side: Side) -> Option<UsageStats> {
    let text = std::fs::read_to_string(memory_path(scenario_id, seed, side)).ok()?;
    let file: MemoryFile = serde_json::from_str(&text).ok()?;
    if file.version != MEMORY_VERSION || file.scenario_id != scenario_id || file.seed != seed || file.side != side {
        return None;
    }
    Some(file.usage)
}

fn save_memory(
    scenario_id: &str,
    seed: u64,
    side: Side,
    memory: &Memory,
    reports: &[TurnReport],
    usage: &UsageStats,
) -> Result<(), String> {
    if scenario_id.is_empty() {
        return Ok(());
    }
    let path = memory_path(scenario_id, seed, side);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let file = MemoryFile {
        version: MEMORY_VERSION,
        scenario_id: scenario_id.to_string(),
        seed,
        side,
        memory: memory.clone(),
        reports: reports.iter().take(MAX_REPORTS).cloned().collect(),
        usage: usage.clone(),
    };
    std::fs::write(path, serde_json::to_vec_pretty(&file).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Two-agent duel
// ---------------------------------------------------------------------------

pub async fn start_duel(
    runtime: Arc<Runtime>,
    db: Arc<Database>,
    scenario_dir: PathBuf,
    req: DuelReq,
) -> Result<Value, String> {
    if req.scenario.trim().is_empty() {
        return Err("scenario must not be empty".into());
    }
    let (sc, _path) = find_scenario(&scenario_dir, &req.scenario)?;
    let mut sc = sc;
    if let Some(seed) = req.seed {
        sc.seed = seed;
    }
    let settings = runtime.inner.lock().unwrap().settings.clone();
    if !settings.configured() {
        return Err("LLM API key is not configured".into());
    }
    let cadence = req
        .cadence_s
        .unwrap_or(settings.cadence_s)
        .clamp(60, 3600);
    let max_ticks = req
        .max_ticks
        .unwrap_or(sc.duration_s)
        .clamp(1, 1_000_000)
        .min(sc.duration_s.max(1));
    let job = DuelJob {
        id: 0,
        scenario: sc.id.clone(),
        seed: sc.seed,
        status: "running".into(),
        tick: 0,
        duration_s: sc.duration_s,
        cadence_s: cadence,
        model: settings.model.clone(),
        base_url: settings.base_url.clone(),
        started_at_unix: now_unix(),
        finished_at_unix: 0,
        result: None,
        vp: None,
        kills: None,
        reports: BTreeMap::new(),
        commands: BTreeMap::new(),
        usage: BTreeMap::new(),
        errors: Vec::new(),
        artifact: None,
        message: Some("Two in-game staff officers are playing the same authoritative engine with separate player-view observations.".into()),
    };
    let id = runtime.begin_duel(job)?;
    let runtime2 = runtime.clone();
    let db2 = db.clone();
    let sc2 = sc.clone();
    tokio::spawn(async move {
        match run_duel_job(runtime2.clone(), db2, id, sc2, settings, cadence, max_ticks).await {
            Ok(()) => {}
            Err(e) => runtime2.fail_duel(id, e),
        }
    });
    Ok(json!({"started": true, "id": id, "status": "running", "scenario": sc.id, "seed": sc.seed, "cadence_s": cadence, "max_ticks": max_ticks}))
}

async fn run_duel_job(
    runtime: Arc<Runtime>,
    db: Arc<Database>,
    id: u64,
    sc: Scenario,
    settings: Settings,
    cadence: i64,
    max_ticks: i64,
) -> Result<(), String> {
    let scenario_id = sc.id.clone();
    let scenario_name = sc.name.clone();
    let seed = sc.seed;
    let duration_s = sc.duration_s;
    let mut world = load_scenario(sc, (*db).clone())?;
    let mut memories: [Memory; 2] = Default::default();
    let mut last_seq = [0u64; 2];
    let mut all_reports: [Vec<TurnReport>; 2] = [Vec::new(), Vec::new()];
    let mut current_reports: [Vec<TurnReport>; 2] = [Vec::new(), Vec::new()];
    let mut command_counts = [0u64; 2];
    let mut usage_stats: [UsageStats; 2] = Default::default();
    let mut errors: Vec<String> = Vec::new();

    while !world.over && world.tick < max_ticks {
        let tick = world.tick;
        let mut observations: [Option<Observation>; 2] = [None, None];
        for side in [Side::Blue, Side::Red] {
            let obs = harpoon_core::observe::observe(&world, side, last_seq[side.index()], 120);
            last_seq[side.index()] = obs.events.iter().map(|e| e.seq).max().unwrap_or(last_seq[side.index()]);
            observations[side.index()] = Some(obs);
        }
        let mut contexts: [Option<TurnContext>; 2] = [None, None];
        for side in [Side::Blue, Side::Red] {
            contexts[side.index()] = Some(TurnContext {
                side,
                trigger: "scheduled".into(),
                settings: settings.clone(),
                autonomy: "execute".into(),
                memory: memories[side.index()].clone(),
                recent: current_reports[side.index()].iter().take(MAX_RECENT_FOR_PROMPT).cloned().collect(),
                generation: 0,
                scenario_id: scenario_id.clone(),
                seed,
                tick,
                director: false,
                wake_rules: Vec::new(),
                trigger_detail: String::new(),
            });
        }
        let blue_obs = observations[0].take().unwrap();
        let red_obs = observations[1].take().unwrap();
        let blue_ctx = contexts[0].take().unwrap();
        let red_ctx = contexts[1].take().unwrap();
        let blue_db = db.clone();
        let red_db = db.clone();
        let blue = tokio::task::spawn_blocking(move || execute_turn(&blue_ctx, &blue_obs, &blue_db, None));
        let red = tokio::task::spawn_blocking(move || execute_turn(&red_ctx, &red_obs, &red_db, None));
        let (blue, red) = tokio::join!(blue, red);
        let decisions = [
            blue.map_err(|e| format!("Blue agent task failed: {e}"))?,
            red.map_err(|e| format!("Red agent task failed: {e}"))?,
        ];
        let mut commands: Vec<(Side, Command)> = Vec::new();
        for side in [Side::Blue, Side::Red] {
            let decision = decisions[side.index()].clone();
            if let Some(err) = &decision.error {
                errors.push(format!("{} T{}: {}", side.name(), tick, err));
                if errors.len() > 40 {
                    errors.drain(0..errors.len() - 40);
                }
            }
            let report = TurnReport {
                tick,
                time_s: world.time_s(),
                trigger: "scheduled".into(),
                report: decision.report.clone(),
                intent: decision.intent.clone(),
                commands: decision.commands.clone(),
                rejected: decision.rejected.clone(),
                applied: decision.error.is_none() && !decision.commands.is_empty(),
                error: decision.error.clone(),
                model: settings.model.clone(),
                latency_ms: decision.latency_ms,
                usage: decision.usage.clone(),
                desired_speed: decision.desired_speed,
                wake_conditions: decision.wake_conditions.clone(),
            };
            if decision.error.is_none() {
                let memory = &mut memories[side.index()];
                if !decision.memory.trim().is_empty() {
                    memory.summary = truncate_chars(decision.memory.trim(), MAX_MEMORY_CHARS);
                }
                memory.last_event_seq = last_seq[side.index()];
                memory.turns.push(MemoryTurn {
                    tick,
                    report: truncate_chars(&decision.report, 500),
                    intent: decision.intent.clone(),
                    commands: decision.commands.iter().map(command_short).collect(),
                });
                if memory.turns.len() > MAX_MEMORY_TURNS {
                    let excess = memory.turns.len() - MAX_MEMORY_TURNS;
                    memory.turns.drain(0..excess);
                }
            }
            usage_stats[side.index()].add(&decision.usage);
            command_counts[side.index()] += decision.commands.len() as u64;
            commands.extend(decision.commands.into_iter().map(|c| (side, c)));
            all_reports[side.index()].push(report.clone());
            current_reports[side.index()].insert(0, report);
            if current_reports[side.index()].len() > MAX_RECENT_FOR_PROMPT {
                current_reports[side.index()].truncate(MAX_RECENT_FOR_PROMPT);
            }
        }
        let step = cadence.min(max_ticks - world.tick).max(1);
        for i in 0..step {
            let applied: &[(Side, Command)] = if i == 0 { commands.as_slice() } else { &[] };
            harpoon_core::sim::tick(&mut world, applied);
            if world.over {
                break;
            }
        }
        let mut report_map = BTreeMap::new();
        let mut command_map = BTreeMap::new();
        let mut usage_map = BTreeMap::new();
        for side in [Side::Blue, Side::Red] {
            report_map.insert(side.name().to_string(), current_reports[side.index()].clone());
            command_map.insert(side.name().to_string(), command_counts[side.index()]);
            usage_map.insert(side.name().to_string(), usage_stats[side.index()].clone());
        }
        runtime.update_duel(id, world.tick, &report_map, &command_map, &usage_map, &errors);
    }

    let artifact_path = write_duel_artifact(
        id,
        &scenario_id,
        &scenario_name,
        seed,
        duration_s,
        cadence,
        &settings,
        &world,
        &all_reports,
        &command_counts,
        &usage_stats,
        &errors,
    )?;
    let mut report_map = BTreeMap::new();
    let mut command_map = BTreeMap::new();
    let mut usage_map = BTreeMap::new();
    for side in [Side::Blue, Side::Red] {
        report_map.insert(side.name().to_string(), all_reports[side.index()].clone());
        command_map.insert(side.name().to_string(), command_counts[side.index()]);
        usage_map.insert(side.name().to_string(), usage_stats[side.index()].clone());
    }
    runtime.finish_duel(id, &world, report_map, command_map, usage_map, errors, artifact_path);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_duel_artifact(
    id: u64,
    scenario_id: &str,
    scenario_name: &str,
    seed: u64,
    duration_s: i64,
    cadence: i64,
    settings: &Settings,
    world: &World,
    reports: &[Vec<TurnReport>; 2],
    command_counts: &[u64; 2],
    usage_stats: &[UsageStats; 2],
    errors: &[String],
) -> Result<String, String> {
    std::fs::create_dir_all(DUEL_ARTIFACT_DIR).map_err(|e| e.to_string())?;
    let path = PathBuf::from(DUEL_ARTIFACT_DIR).join(format!("duel-{}-{}.json", id, safe_name(scenario_id)));
    let event_counts: BTreeMap<String, u64> = world
        .events
        .iter()
        .fold(BTreeMap::new(), |mut acc, e| {
            *acc.entry(e.kind.name().to_string()).or_default() += 1;
            acc
        });
    let mut report_error_counts: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut report_errors: BTreeMap<String, u64> = BTreeMap::new();
    for side in [Side::Blue, Side::Red] {
        let mut counts: BTreeMap<String, u64> = BTreeMap::new();
        let mut total = 0u64;
        for report in &reports[side.index()] {
            if let Some(error) = &report.error {
                total += 1;
                let kind = error.split(':').next().unwrap_or(error).trim().to_string();
                *counts.entry(kind).or_default() += 1;
            }
        }
        report_errors.insert(side.name().to_string(), total);
        report_error_counts.insert(side.name().to_string(), counts);
    }
    let (vp_blue, vp_red) = world.victory_points();
    let (kills_blue, kills_red) = world.kill_counts();
    let artifact = json!({
        "id": id,
        "scenario": scenario_id,
        "scenario_name": scenario_name,
        "seed": seed,
        "finished_at_unix": now_unix(),
        "duration_s": duration_s,
        "cadence_s": cadence,
        "model": settings.model,
        "base_url": settings.base_url,
        "world_tick": world.tick,
        "over": world.over,
        "result": world.result,
        "vp": {"blue": vp_blue, "red": vp_red},
        "kills": {"blue": kills_blue, "red": kills_red},
        "entities_alive": world.entities.values().filter(|e| e.alive).count(),
        "event_counts": event_counts,
        "event_counts_note": "world.events is capped at 4096; these counts are a tail, not a full-match tally. kills/result are authoritative.",
        "report_errors": report_errors,
        "report_error_counts": report_error_counts,
        "commands": {"Blue": command_counts[0], "Red": command_counts[1]},
        "usage": {"Blue": usage_stats[0], "Red": usage_stats[1]},
        "reports": {
            "Blue": reports[0],
            "Red": reports[1],
        },
        "errors": errors,
        "state_digest": format!("{:016x}", world_digest(world)),
        "scope": "Both agents received only their own double-blind observations. The referee result is recorded after the match.",
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&artifact).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn world_digest(world: &World) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    h ^= world.tick as u64;
    h = h.wrapping_mul(0x100_0000_01b3);
    for e in world.entities.values() {
        for bits in [e.pos.x.to_bits(), e.pos.y.to_bits(), e.hp.to_bits(), e.alive as u64] {
            h ^= bits;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

fn find_scenario(dir: &Path, wanted: &str) -> Result<(Scenario, PathBuf), String> {
    if wanted.is_empty() || wanted.contains('/') || wanted.contains('\\') {
        return Err(format!("invalid scenario id `{wanted}`"));
    }
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read scenarios directory `{}`: {e}", dir.display()))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    paths.sort();
    let stem = Path::new(wanted).file_stem().and_then(|s| s.to_str()).unwrap_or(wanted);
    for path in paths {
        let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let Ok(sc) = scenario_from_json(&text) else { continue };
        if file_stem == stem || sc.id == wanted {
            return Ok((sc, path));
        }
    }
    Err(format!("scenario `{wanted}` not found in `{}`", dir.display()))
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn fmt_hms(seconds: i64) -> String {
    let s = seconds.max(0);
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn truncate_chars(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn safe_name(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(value: Value) -> Command {
        serde_json::from_value(normalize_command_value(&value)).expect("normalized command")
    }

    #[test]
    fn recon_accepts_station_alias_and_array_points() {
        let cmd = parse(json!({
            "cmd":"launch_aircraft","unit":5,"class":"aew","count":1,
            "mission":{"mission":"recon","station":[430,350],"radius_nm":40}
        }));
        match cmd {
            Command::LaunchAircraft { mission: AirMission::Recon { point }, count, .. } => {
                assert_eq!((point.x, point.y), (430.0, 350.0));
                assert_eq!(count, 1);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn asw_and_cap_aliases_get_valid_defaults() {
        let asw = parse(json!({
            "cmd":"launch_aircraft","unit":5,"class":"helo","count":1,
            "mission":{"mission":"asw","station":{"x":1,"y":2}}
        }));
        match asw {
            Command::LaunchAircraft { mission: AirMission::Asw { area, radius_nm }, .. } => {
                assert_eq!((area.x, area.y, radius_nm), (1.0, 2.0, 20.0));
            }
            other => panic!("unexpected command: {other:?}"),
        }
        let cap = parse(json!({
            "cmd":"launch_aircraft","unit":5,"class":"fighter","count":2,
            "mission":{"mission":"cap","point":{"x":3,"y":4}}
        }));
        match cap {
            Command::LaunchAircraft { mission: AirMission::Cap { station, radius_nm }, .. } => {
                assert_eq!((station.x, station.y, radius_nm), (3.0, 4.0, 25.0));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn fire_defaults_rounds_and_goto_accepts_pairs() {
        let fire = parse(json!({"cmd":"fire","unit":1,"contact":2,"weapon":"gun_5in"}));
        match fire {
            Command::Fire { rounds, .. } => assert_eq!(rounds, 1),
            other => panic!("unexpected command: {other:?}"),
        }
        let goto = parse(json!({"cmd":"goto","unit":1,"waypoints":[[10,20]]}));
        match goto {
            Command::Goto { waypoints, .. } => assert_eq!((waypoints[0].x, waypoints[0].y), (10.0, 20.0)),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn scheduled_turns_keep_their_trigger_tick_and_five_minute_cadence() {
        let scenario = format!("agent-test-{}", std::process::id());
        let runtime = Runtime::new();
        runtime.inner.lock().unwrap().settings.api_key = "test-key".into();
        runtime.start_game(&scenario, 7, 0);
        runtime
            .set_config(
                &AgentConfigReq {
                    side: Some("Blue".into()),
                    enabled: Some(true),
                    autonomy: Some("execute".into()),
                    cadence_s: Some(300),
                    max_tokens: Some(65_536),
                    auto_run: Some(true),
                    auto_speed: Some(15.0),
                    model: None,
                    base_url: None,
                    api_key: None,
                    language: None,
                },
                0,
            )
            .unwrap();
        assert_eq!(runtime.auto_run_speed(), Some(15.0));
        runtime
            .set_config(
                &AgentConfigReq {
                    side: Some("Red".into()),
                    enabled: Some(true),
                    autonomy: Some("execute".into()),
                    cadence_s: None,
                    max_tokens: None,
                    auto_run: Some(true),
                    auto_speed: Some(30.0),
                    model: None,
                    base_url: None,
                    api_key: None,
                    language: None,
                },
                0,
            )
            .unwrap();
        assert_eq!(runtime.auto_run_speed(), Some(15.0));
        runtime
            .set_config(
                &AgentConfigReq {
                    side: Some("Red".into()),
                    enabled: Some(false),
                    autonomy: None,
                    cadence_s: None,
                    max_tokens: None,
                    auto_run: None,
                    auto_speed: None,
                    model: None,
                    base_url: None,
                    api_key: None,
                    language: None,
                },
                0,
            )
            .unwrap();
        assert_eq!(runtime.due_sides(0), Vec::<Side>::new());
        assert_eq!(runtime.due_sides(1), vec![Side::Blue]);
        let ctx = runtime.prepare_turn(Side::Blue, "scheduled", 1).unwrap();
        assert_eq!(ctx.tick, 1);
        // The game clock can run while the model is thinking; the report must
        // still be stamped with the trigger tick, not the completion tick.
        runtime.due_sides(900);
        let report = runtime.finish_turn(
            &ctx,
            TurnDecision {
                report: "brief".into(),
                memory: "memory".into(),
                intent: "intent".into(),
                commands: Vec::new(),
                rejected: Vec::new(),
                error: None,
                usage: Usage {
                    prompt_tokens: Some(10),
                    completion_tokens: Some(2),
                    cache_hit_tokens: Some(8),
                    cache_miss_tokens: Some(2),
                },
                latency_ms: 5,
                desired_speed: None,
                wake_conditions: Vec::new(),
            },
            false,
            0,
        );
        assert_eq!(report.tick, 1);
        assert_eq!(report.time_s, 1.0);
        let status = runtime.side_status(Side::Blue);
        assert_eq!(status["usage"]["turns"], 1);
        assert_eq!(status["usage"]["avg_prompt_tokens"], 10.0);
        assert_eq!(status["usage"]["avg_completion_tokens"], 2.0);
        // The model finished at game tick 900; the missed 301 boundary is not
        // replayed immediately. The next report is one cadence later.
        assert_eq!(runtime.due_sides(1199), Vec::<Side>::new());
        assert_eq!(runtime.due_sides(1200), vec![Side::Blue]);
        runtime
            .set_config(
                &AgentConfigReq {
                    side: Some("Blue".into()),
                    enabled: Some(false),
                    autonomy: None,
                    cadence_s: None,
                    max_tokens: None,
                    auto_run: None,
                    auto_speed: None,
                    model: None,
                    base_url: None,
                    api_key: None,
                    language: None,
                },
                1200,
            )
            .unwrap();
        assert_eq!(runtime.auto_run_speed(), None);
        let _ = std::fs::remove_file(memory_path(&scenario, 7, Side::Blue));
    }

    #[test]
    fn aircraft_speed_above_100_is_not_rejected_by_the_validator() {
        let sc = scenario_from_json(r#"{
          "id":"agent-speed-test","name":"Agent speed test","duration_s":100,"seed":1,
          "weather":{"visibility_nm":10,"jam":[0,0]},
          "units":[{"side":"Blue","class":"fighter","name":"BLUE AIR","pos":[0,0]}]
        }"#).unwrap();
        let world = load_scenario(sc, Database::built_in()).unwrap();
        let obs = harpoon_core::observe::observe(&world, Side::Blue, 0, 0);
        let unit = obs.units[0].id;
        assert!(validate_command(&obs, Side::Blue, &Command::SetSpeed { unit, speed_kts: 700.0 }).is_ok());
        assert!(validate_command(&obs, Side::Blue, &Command::SetSpeed { unit, speed_kts: 2500.0 }).is_err());
    }

    #[test]
    fn trailing_commas_are_repaired() {
        let value = extract_json("{\"report\":\"x\",\"commands\":[{\"cmd\":\"stop\",\"unit\":1,},],}").unwrap();
        assert_eq!(value["report"], "x");
        assert_eq!(value["commands"].as_array().unwrap().len(), 1);
    }
}

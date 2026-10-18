//! MCP tools and the in-process engine session.
//!
//! The MCP layer is deliberately thin: every read goes through
//! [`harpoon_core::observe::observe`] (the double-blind projection) and every
//! order is a [`harpoon_core::Command`] queued for the first tick of the next
//! `step` call. Nothing here reaches into enemy truth.

use harpoon_core::commands::Command;
use harpoon_core::db::Database;
use harpoon_core::observe::{observe, Observation};
use harpoon_core::scenario::{load_scenario, scenario_from_json, Scenario};
use harpoon_core::types::{PlatformKind, Side};
use harpoon_core::world::World;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DEFAULT_MAX_EVENTS: usize = 200;
/// Upper bound for one `step` call, to keep a single MCP request bounded.
const MAX_STEP_TICKS: i64 = 1_000_000;
const MAX_SELFPLAY_TICKS: i64 = 1_000_000;
/// Selfplay staff re-plans at the same cadence as the server's heuristic staff.
const SELFPLAY_DECIDE_EVERY: i64 = 30;

/// One MCP server process owns exactly one game session at a time.
pub struct Session {
    world: Option<World>,
    /// Orders buffered by `send_command`, consumed by the next `step`.
    inbox: Vec<(Side, Command)>,
    /// Advisory time multiplier (simulated seconds per wall-clock second).
    /// The MCP server is step-driven and never advances time on its own.
    speed: f64,
    scenario_dir: PathBuf,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            world: None,
            inbox: Vec::new(),
            speed: 60.0,
            scenario_dir: PathBuf::from("scenarios"),
        }
    }

    /// Dispatch a tool call. `Ok` is the tool's JSON result; `Err` becomes an
    /// `isError: true` tool result.
    pub fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        if !args.is_null() && !args.is_object() {
            return Err("arguments must be a JSON object".into());
        }
        match name {
            "list_scenarios" => self.list_scenarios(),
            "new_game" => self.new_game(args),
            "get_state" => self.get_state(args),
            "send_command" => self.send_command(args),
            "set_time" => self.set_time(args),
            "step" => self.step(args),
            "get_events" => self.get_events(args),
            "run_selfplay" => self.run_selfplay(args),
            other => Err(format!("unknown tool `{other}`")),
        }
    }

    /// Scan `./scenarios/*.json`, skipping files that do not parse.
    fn list_scenarios(&self) -> Result<Value, String> {
        let mut rows = Vec::new();
        for path in json_paths(&self.scenario_dir)? {
            let txt = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!(
                        "harpoon-mcp: skipping unreadable scenario {}: {e}",
                        path.display()
                    );
                    continue;
                }
            };
            match scenario_from_json(&txt) {
                Ok(sc) => rows.push(json!({
                    "id": sc.id,
                    "name": sc.name,
                    "file": path.file_name().and_then(|s| s.to_str()).unwrap_or_default(),
                })),
                Err(e) => eprintln!(
                    "harpoon-mcp: skipping invalid scenario {}: {e}",
                    path.display()
                ),
            }
        }
        Ok(Value::Array(rows))
    }

    fn new_game(&mut self, args: &Value) -> Result<Value, String> {
        let wanted = require_str(args, "scenario")?;
        let seed = opt_u64(args, "seed")?;
        let (mut sc, path) = find_scenario(&self.scenario_dir, &wanted)?;
        if let Some(seed) = seed {
            sc.seed = seed;
        }
        // Build the world before touching the live session: a failed load must
        // not destroy the game already in progress.
        let world = load_scenario(sc, Database::built_in())?;
        let summary = json!({
            "started": true,
            "scenario": world.scenario_id,
            "name": world.scenario_name,
            "seed": world.seed,
            "file": path.file_name().and_then(|s| s.to_str()).unwrap_or_default(),
            "tick": world.tick,
            "units": world.entities.len(),
            "unit_counts": {
                "Blue": world.entities.values().filter(|e| e.side == Side::Blue).count(),
                "Red": world.entities.values().filter(|e| e.side == Side::Red).count(),
            },
        });
        self.world = Some(world);
        self.inbox.clear();
        Ok(summary)
    }

    fn get_state(&self, args: &Value) -> Result<Value, String> {
        let side = parse_side(args)?;
        let since = opt_u64(args, "since")?.unwrap_or(0);
        let max_events = opt_u64(args, "max_events")?
            .map(|v| usize::try_from(v).unwrap_or(usize::MAX))
            .unwrap_or(DEFAULT_MAX_EVENTS);
        let world = self.world.as_ref().ok_or_else(no_game)?;
        let obs = observe(world, side, since, max_events);
        serde_json::to_value(&obs)
            .map_err(|e| format!("failed to serialize observation: {e}"))
    }

    fn send_command(&mut self, args: &Value) -> Result<Value, String> {
        let side = parse_side(args)?;
        let raw = args
            .get("command")
            .ok_or_else(|| "missing argument `command`".to_string())?;
        let command: Command = serde_json::from_value(raw.clone())
            .map_err(|e| format!("invalid command: {e}"))?;
        if self.world.is_none() {
            return Err(no_game().into());
        }
        self.inbox.push((side, command));
        Ok(json!({"queued": true, "pending": self.inbox.len()}))
    }

    fn set_time(&mut self, args: &Value) -> Result<Value, String> {
        let requested = require_f64(args, "speed")?;
        let speed = requested.clamp(0.0, 512.0);
        self.speed = speed;
        Ok(json!({
            "speed": speed,
            "requested": requested,
            "clamped": speed != requested,
            "paused": speed == 0.0,
            "ticks_per_wall_second": speed,
            "note": "advisory only: call step to advance simulated time",
        }))
    }

    fn step(&mut self, args: &Value) -> Result<Value, String> {
        let ticks = require_i64(args, "ticks", 1, MAX_STEP_TICKS)?;
        // Split the borrow so the world and the inbox are borrowed separately.
        let Session { world, inbox, .. } = self;
        let world = world.as_mut().ok_or_else(no_game)?;
        if world.over {
            return Ok(json!({
                "tick": world.tick,
                "time_s": world.time_s(),
                "over": true,
                "result": world.result,
                "ticks_advanced": 0,
                "commands_applied": 0,
                "inbox_pending": inbox.len(),
            }));
        }
        // Queued orders apply once, on the first tick, then stand as orders
        // inside the engine (course/speed/etc. persist on the entities).
        let commands = std::mem::take(inbox);
        let commands_applied = commands.len();
        let mut ticks_advanced = 0i64;
        for i in 0..ticks {
            let applied: &[(Side, Command)] = if i == 0 { commands.as_slice() } else { &[] };
            harpoon_core::sim::tick(world, applied);
            ticks_advanced += 1;
            if world.over {
                break;
            }
        }
        Ok(json!({
            "tick": world.tick,
            "time_s": world.time_s(),
            "over": world.over,
            "result": world.result,
            "ticks_advanced": ticks_advanced,
            "commands_applied": commands_applied,
            "inbox_pending": inbox.len(),
        }))
    }

    fn get_events(&self, args: &Value) -> Result<Value, String> {
        let side = parse_side(args)?;
        let since = opt_u64(args, "since")?.unwrap_or(0);
        let world = self.world.as_ref().ok_or_else(no_game)?;
        let events = world.events_for(side, since);
        serde_json::to_value(events).map_err(|e| format!("failed to serialize events: {e}"))
    }

    fn run_selfplay(&self, args: &Value) -> Result<Value, String> {
        let wanted = require_str(args, "scenario")?;
        let seed = opt_u64(args, "seed")?;
        let (mut sc, _) = find_scenario(&self.scenario_dir, &wanted)?;
        if let Some(seed) = seed {
            sc.seed = seed;
        }
        let max_ticks = match opt_u64(args, "max_ticks")? {
            Some(v) => {
                if v == 0 || v > MAX_SELFPLAY_TICKS as u64 {
                    return Err(format!(
                        "argument `max_ticks` must be between 1 and {MAX_SELFPLAY_TICKS}"
                    ));
                }
                v as i64
            }
            None => sc.duration_s.max(1).min(MAX_SELFPLAY_TICKS),
        };
        // Fully independent: the live session world is untouched.
        selfplay(&sc, max_ticks)
    }
}

// ---------------------------------------------------------------------------
// Tool definitions (MCP tools/list)
// ---------------------------------------------------------------------------

pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "list_scenarios",
            "description": "List scenario JSON files in ./scenarios. Returns [{id,name,file}]. Files that fail to parse are skipped.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        },
        {
            "name": "new_game",
            "description": "Load a scenario into the local engine session and start a new game. Replaces any game in progress and clears queued commands.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scenario": {
                        "type": "string",
                        "description": "Scenario id, file stem, or <id>.json; resolved inside ./scenarios"
                    },
                    "seed": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Optional seed override for a deterministic run"
                    }
                },
                "required": ["scenario"],
                "additionalProperties": false
            }
        },
        {
            "name": "get_state",
            "description": "Get the double-blind observation for one side (units, contacts with estimates, events tail). Enemy truth is never exposed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "side": {"type": "string", "enum": ["Blue", "Red"]},
                    "since": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 0,
                        "description": "Only include events with seq greater than this"
                    },
                    "max_events": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 200,
                        "description": "Maximum number of events to include"
                    }
                },
                "required": ["side"],
                "additionalProperties": false
            }
        },
        {
            "name": "send_command",
            "description": "Queue one engine command for a side. It is applied once, on the first tick of the next step call.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "side": {"type": "string", "enum": ["Blue", "Red"]},
                    "command": command_schema()
                },
                "required": ["side", "command"],
                "additionalProperties": false
            }
        },
        {
            "name": "set_time",
            "description": "Set the advisory simulation time multiplier (0..512, 0 = paused). The MCP server never advances time itself; the host calls step on a timer and passes the desired tick count.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "speed": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 512,
                        "description": "Simulated seconds per wall-clock second (0 pauses)"
                    }
                },
                "required": ["speed"],
                "additionalProperties": false
            }
        },
        {
            "name": "step",
            "description": "Advance the simulation by N one-second ticks. Queued commands apply on the first tick; the rest advance with no new orders. Stops early when the scenario ends.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticks": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_STEP_TICKS,
                        "description": "Number of one-second ticks to run"
                    }
                },
                "required": ["ticks"],
                "additionalProperties": false
            }
        },
        {
            "name": "get_events",
            "description": "Return the event log visible to one side, filtered by sequence number (world.events_for).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "side": {"type": "string", "enum": ["Blue", "Red"]},
                    "since": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 0,
                        "description": "Only events with seq greater than this"
                    }
                },
                "required": ["side"],
                "additionalProperties": false
            }
        },
        {
            "name": "run_selfplay",
            "description": "Run an independent deterministic headless match with the built-in heuristic on both sides (weapons free plus automatic fire control, submarine torpedo attacks). Does not affect the current session. Returns a summary with result, VP, event counts and a state digest.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scenario": {
                        "type": "string",
                        "description": "Scenario id, file stem, or <id>.json; resolved inside ./scenarios"
                    },
                    "seed": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Optional seed override"
                    },
                    "max_ticks": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_SELFPLAY_TICKS,
                        "description": "Tick cap; defaults to the scenario duration"
                    }
                },
                "required": ["scenario"],
                "additionalProperties": false
            }
        }
    ])
}

fn command_schema() -> Value {
    let unit = || json!({"type": "integer", "minimum": 1, "description": "Own unit id"});
    let contact = || json!({"type": "integer", "minimum": 1, "description": "Contact id from get_state"});
    let xy = |desc: &str| {
        json!({
            "type": "array",
            "items": {"type": "number"},
            "minItems": 2,
            "maxItems": 2,
            "description": desc
        })
    };
    json!({
        "type": "object",
        "description": "One engine command; `cmd` selects the variant. Coordinates are nautical miles on a flat plane (+x east, +y north). Unit and contact ids are numbers.",
        "properties": {
            "cmd": {
                "type": "string",
                "enum": [
                    "set_course", "set_speed", "set_depth", "set_altitude", "goto",
                    "stop", "follow", "clear_station", "set_sonar", "set_weapons_free",
                    "set_auto_chaff", "assign_target", "clear_target", "fire",
                    "deploy_chaff", "launch_aircraft", "recover_aircraft"
                ]
            }
        },
        "required": ["cmd"],
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "set_course"},
                    "unit": unit(),
                    "course_deg": {"type": "number", "description": "Desired course, 0 = north"}
                },
                "required": ["cmd", "unit", "course_deg"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "set_speed"},
                    "unit": unit(),
                    "speed_kts": {"type": "number", "minimum": 0}
                },
                "required": ["cmd", "unit", "speed_kts"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "set_depth"},
                    "unit": unit(),
                    "depth_m": {"type": "number", "minimum": 10}
                },
                "required": ["cmd", "unit", "depth_m"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "set_altitude"},
                    "unit": unit(),
                    "altitude_m": {"type": "number", "minimum": 15}
                },
                "required": ["cmd", "unit", "altitude_m"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "goto"},
                    "unit": unit(),
                    "waypoints": {
                        "type": "array",
                        "items": xy("Waypoint [x, y] in nautical miles"),
                        "description": "Ordered waypoints"
                    }
                },
                "required": ["cmd", "unit", "waypoints"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "stop"},
                    "unit": unit()
                },
                "required": ["cmd", "unit"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "follow"},
                    "unit": unit(),
                    "leader": {"type": "integer", "minimum": 1, "description": "Own unit id of the leader"},
                    "bearing_rel_deg": {"type": "number"},
                    "distance_nm": {"type": "number", "minimum": 0}
                },
                "required": ["cmd", "unit", "leader", "bearing_rel_deg", "distance_nm"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "clear_station"},
                    "unit": unit()
                },
                "required": ["cmd", "unit"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "set_sonar"},
                    "unit": unit(),
                    "active": {"type": "boolean"}
                },
                "required": ["cmd", "unit", "active"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "set_weapons_free"},
                    "unit": unit(),
                    "free": {"type": "boolean"}
                },
                "required": ["cmd", "unit", "free"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "set_auto_chaff"},
                    "unit": unit(),
                    "on": {"type": "boolean"}
                },
                "required": ["cmd", "unit", "on"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "assign_target"},
                    "unit": unit(),
                    "contact": contact()
                },
                "required": ["cmd", "unit", "contact"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "clear_target"},
                    "unit": unit()
                },
                "required": ["cmd", "unit"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "fire"},
                    "unit": unit(),
                    "contact": contact(),
                    "weapon": {"type": "string", "description": "Weapon id, e.g. gun_5in or ascm_std"},
                    "rounds": {"type": "integer", "minimum": 1, "maximum": 255}
                },
                "required": ["cmd", "unit", "contact", "weapon", "rounds"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "deploy_chaff"},
                    "unit": unit()
                },
                "required": ["cmd", "unit"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "launch_aircraft"},
                    "unit": unit(),
                    "class": {"type": "string", "description": "Aircraft class id, e.g. fighter"},
                    "count": {"type": "integer", "minimum": 1, "maximum": 255},
                    "mission": mission_schema()
                },
                "required": ["cmd", "unit", "class", "count", "mission"]
            },
            {
                "type": "object",
                "properties": {
                    "cmd": {"const": "recover_aircraft"},
                    "unit": unit()
                },
                "required": ["cmd", "unit"]
            }
        ]
    })
}

fn mission_schema() -> Value {
    let point = |desc: &str| {
        json!({
            "type": "array",
            "items": {"type": "number"},
            "minItems": 2,
            "maxItems": 2,
            "description": desc
        })
    };
    json!({
        "type": "object",
        "description": "Aircraft mission (tagged by `mission`)",
        "oneOf": [
            {
                "properties": {
                    "mission": {"const": "cap"},
                    "station": point("CAP station [x, y] in nautical miles"),
                    "radius_nm": {"type": "number", "minimum": 0}
                },
                "required": ["mission", "station", "radius_nm"]
            },
            {
                "properties": {
                    "mission": {"const": "strike"},
                    "contact": {"type": "integer", "minimum": 1}
                },
                "required": ["mission", "contact"]
            },
            {
                "properties": {
                    "mission": {"const": "asw"},
                    "area": point("ASW search area center [x, y] in nautical miles"),
                    "radius_nm": {"type": "number", "minimum": 0}
                },
                "required": ["mission", "area", "radius_nm"]
            },
            {
                "properties": {
                    "mission": {"const": "recon"},
                    "point": point("Recon point [x, y] in nautical miles")
                },
                "required": ["mission", "point"]
            },
            {
                "properties": {
                    "mission": {"const": "rtb"}
                },
                "required": ["mission"]
            }
        ]
    })
}

// ---------------------------------------------------------------------------
// Selfplay: deterministic heuristic staff for both sides
// ---------------------------------------------------------------------------

fn selfplay(sc: &Scenario, max_ticks: i64) -> Result<Value, String> {
    let scenario_id = sc.id.clone();
    let scenario_name = sc.name.clone();
    let seed = sc.seed;
    let mut world = load_scenario(sc.clone(), Database::built_in())?;
    let mut commands_issued = 0u64;

    for tick in 0..max_ticks {
        if world.over {
            break;
        }
        let mut commands: Vec<(Side, Command)> = Vec::new();
        if tick % SELFPLAY_DECIDE_EVERY == 0 {
            for side in [Side::Blue, Side::Red] {
                let obs = observe(&world, side, 0, 64);
                let decided = heuristic(&obs);
                commands_issued += decided.len() as u64;
                commands.extend(decided.into_iter().map(|c| (side, c)));
            }
        }
        harpoon_core::sim::tick(&mut world, &commands);
    }

    // Count events that either side can see (All + per-side); Referee events
    // stay secret.
    let mut event_counts: BTreeMap<String, u64> = BTreeMap::new();
    for ev in &world.events {
        if ev.vis.visible_to(Side::Blue) || ev.vis.visible_to(Side::Red) {
            *event_counts
                .entry(ev.kind.name().to_string())
                .or_default() += 1;
        }
    }
    let (vp_blue, vp_red) = world.victory_points();
    let entities_alive = world.entities.values().filter(|e| e.alive).count();

    Ok(json!({
        "scenario": scenario_id,
        "name": scenario_name,
        "seed": seed,
        "max_ticks": max_ticks,
        "ticks_run": world.tick,
        "over": world.over,
        "result": world.result,
        "vp": {"blue": vp_blue, "red": vp_red},
        "event_counts": event_counts,
        "entities_alive": entities_alive,
        "commands_issued": commands_issued,
        "state_digest": format!("{:016x}", digest(&world)),
    }))
}

/// Minimal deterministic doctrine: everyone weapons-free (the engine's fire
/// control picks targets), plus deliberate submarine torpedo attacks.
fn heuristic(obs: &Observation) -> Vec<Command> {
    let mut cmds = Vec::new();
    for u in &obs.units {
        if !u.fire_on {
            cmds.push(Command::SetWeaponsFree {
                unit: u.id,
                free: true,
            });
        }
    }
    for u in obs.units.iter().filter(|u| u.kind == PlatformKind::Sub) {
        let target = obs
            .contacts
            .iter()
            .filter(|c| !c.stale && c.quality > 0.3 && c.category == PlatformKind::Ship)
            .map(|c| (c.id, u.pos.range_to(c.estimate)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((contact, dist)) = target {
            if u.assigned != Some(contact) {
                cmds.push(Command::AssignTarget {
                    unit: u.id,
                    contact,
                });
            }
            let has_torpedoes = u
                .weapons
                .iter()
                .any(|w| w.weapon == "torp_homing" && w.ammo > 0);
            if has_torpedoes && dist <= 10.0 {
                cmds.push(Command::Fire {
                    unit: u.id,
                    contact,
                    weapon: "torp_homing".into(),
                    rounds: 2,
                });
            }
        }
    }
    cmds
}

/// Stable digest over entity truth, so two selfplay runs can be compared.
fn digest(world: &World) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    h ^= world.tick as u64;
    h = h.wrapping_mul(0x100_0000_01b3);
    for e in world.entities.values() {
        for bits in [
            e.pos.x.to_bits(),
            e.pos.y.to_bits(),
            e.hp.to_bits(),
            e.alive as u64,
        ] {
            h ^= bits;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

// ---------------------------------------------------------------------------
// Argument + scenario helpers
// ---------------------------------------------------------------------------

fn no_game() -> &'static str {
    "no game running; call new_game first"
}

fn parse_side(args: &Value) -> Result<Side, String> {
    let raw = require_str(args, "side")?;
    Side::parse(&raw).ok_or_else(|| format!("argument `side` must be Blue or Red (got `{raw}`)"))
}

fn require_str(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing or invalid string argument `{key}`"))
}

fn opt_u64(args: &Value, key: &str) -> Result<Option<u64>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("argument `{key}` must be a non-negative integer")),
    }
}

fn require_f64(args: &Value, key: &str) -> Result<f64, String> {
    let v = args
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("missing or invalid number argument `{key}`"))?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(format!("argument `{key}` must be finite"))
    }
}

fn require_i64(args: &Value, key: &str, min: i64, max: i64) -> Result<i64, String> {
    let v = args
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing or invalid integer argument `{key}`"))?;
    if !(min..=max).contains(&v) {
        return Err(format!("argument `{key}` must be between {min} and {max}"));
    }
    Ok(v)
}

fn json_paths(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| {
        format!(
            "cannot read scenarios directory `{}`: {e}",
            dir.display()
        )
    })?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    paths.sort();
    Ok(paths)
}

/// Resolve a scenario by id, file stem, or `<id>.json` inside `dir`. Path
/// separators are rejected, so the lookup can never escape the directory.
fn find_scenario(dir: &Path, wanted: &str) -> Result<(Scenario, PathBuf), String> {
    if wanted.is_empty() || wanted.contains('/') || wanted.contains('\\') {
        return Err(format!("invalid scenario id `{wanted}`"));
    }
    let mut available: Vec<String> = Vec::new();
    for path in json_paths(dir)? {
        let file = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let direct = file == wanted || stem == wanted;
        let txt = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                if direct {
                    return Err(format!("cannot read `{}`: {e}", path.display()));
                }
                continue;
            }
        };
        match scenario_from_json(&txt) {
            Ok(sc) => {
                available.push(sc.id.clone());
                if direct || sc.id == wanted {
                    return Ok((sc, path));
                }
            }
            Err(e) => {
                if direct {
                    return Err(format!("failed to parse `{}`: {e}", path.display()));
                }
                eprintln!(
                    "harpoon-mcp: skipping invalid scenario {}: {e}",
                    path.display()
                );
            }
        }
    }
    available.sort();
    available.dedup();
    let suffix = if available.is_empty() {
        String::new()
    } else {
        format!(" (available: {})", available.join(", "))
    };
    Err(format!(
        "scenario `{wanted}` not found in `{}`{suffix}",
        dir.display()
    ))
}

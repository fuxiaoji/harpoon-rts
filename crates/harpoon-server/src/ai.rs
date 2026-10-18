//! AI command staff: the pluggable "大模型接口".
//!
//! A `CommandStaff` turns an `Observation` into player commands. Two built-in
//! implementations:
//! - `HeuristicStaff` — deterministic rule-based doctrine (always available,
//!   used by selfplay/tests and as the LLM fallback).
//! - `OpenAiCompatStaff` — any OpenAI-compatible chat endpoint (DeepSeek,
//!   GLM, vLLM, Ollama...). Config via env:
//!   HARPOON_LLM_BASE_URL / HARPOON_LLM_API_KEY / HARPOON_LLM_MODEL.
//!
//! The observation the staff receives is already double-blind filtered by
//! `harpoon_core::observe`, so no side ever sees enemy truth.

use harpoon_core::commands::{AirMission, Command};
use harpoon_core::observe::{Observation, UnitObs};
use harpoon_core::types::{ContactId, EntityId, PlatformKind, Side};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub trait CommandStaff: Send {
    fn name(&self) -> &'static str;
    /// Decide this tick's commands for `side`.
    fn decide(&mut self, obs: &Observation) -> Vec<Command>;
}

// ---------------------------------------------------------------------------
// Heuristic doctrine staff
// ---------------------------------------------------------------------------

pub struct HeuristicStaff {
    pub side: Side,
    /// Engagement ranges already handled by weapons-free doctrine; the staff
    /// handles posture: target assignment, strikes, recalls.
    launched_strike: bool,
}

impl HeuristicStaff {
    pub fn new(side: Side) -> Self {
        Self { side, launched_strike: false }
    }
}

impl CommandStaff for HeuristicStaff {
    fn name(&self) -> &'static str {
        "heuristic"
    }

    fn decide(&mut self, obs: &Observation) -> Vec<Command> {
        let mut cmds = Vec::new();
        let enemy = self.side.other();

        // 1. Weapons free everywhere (ships auto-engage per doctrine).
        for u in &obs.units {
            if u.kind == PlatformKind::Ship && !u.fire_on {
                cmds.push(Command::SetWeaponsFree { unit: u.id, free: true });
            }
        }

        // 2. Submarines: assign the nearest tracked surface contact and fire
        //    deliberately; otherwise stay quiet.
        for u in obs.units.iter().filter(|u| u.kind == PlatformKind::Sub) {
            if let Some((cid, dist)) = nearest_contact(obs, u, &[PlatformKind::Ship]) {
                if u.assigned != Some(cid) {
                    cmds.push(Command::AssignTarget { unit: u.id, contact: cid });
                }
                if dist < 10.0 && u.assigned == Some(cid) {
                    cmds.push(Command::Fire {
                        unit: u.id,
                        contact: cid,
                        weapon: "torp_homing".into(),
                        rounds: 2,
                    });
                }
            }
        }

        // 3. Missile boats: close to SSM envelope of the nearest surface
        //    contact, then launch.
        for u in obs.units.iter().filter(|u| u.class == "mgb") {
            if let Some((cid, dist)) = nearest_contact(obs, u, &[PlatformKind::Ship]) {
                if u.assigned != Some(cid) {
                    cmds.push(Command::AssignTarget { unit: u.id, contact: cid });
                }
                if dist < 55.0 && u.assigned == Some(cid) {
                    cmds.push(Command::Fire {
                        unit: u.id,
                        contact: cid,
                        weapon: "ascm_std".into(),
                        rounds: 1,
                    });
                }
                if dist > 40.0 {
                    cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 30.0 });
                } else {
                    cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 12.0 });
                }
            }
        }

        // 4. Carriers: launch one CAP + one ASW helo early; launch a strike
        //    against the first identified surface contact.
        for u in obs.units.iter().filter(|u| u.deck.is_some() && u.kind == PlatformKind::Ship) {
            let Some(deck) = &u.deck else { continue };
            let cap_ready = deck
                .ready
                .iter()
                .find(|(c, n)| c == "fighter" && *n > 0)
                .is_some();
            let helo_ready = deck
                .ready
                .iter()
                .find(|(c, n)| c == "helo_asw" && *n > 0)
                .is_some();
            if cap_ready {
                cmds.push(Command::LaunchAircraft {
                    unit: u.id,
                    class: "fighter".into(),
                    count: 2,
                    mission: AirMission::Cap { station: u.pos, radius_nm: 25.0 },
                });
            }
            if helo_ready {
                cmds.push(Command::LaunchAircraft {
                    unit: u.id,
                    class: "helo_asw".into(),
                    count: 2,
                    mission: AirMission::Asw { area: u.pos, radius_nm: 15.0 },
                });
            }
            if !self.launched_strike {
                if let Some((cid, c)) = obs
                    .contacts
                    .iter()
                    .filter(|c| c.category == PlatformKind::Ship && c.quality > 0.4)
                    .map(|c| (c.id, c))
                    .min_by(|a, b| a.1.quality.partial_cmp(&b.1.quality).unwrap())
                {
                    let _ = c;
                    cmds.push(Command::LaunchAircraft {
                        unit: u.id,
                        class: "striker".into(),
                        count: 2,
                        mission: AirMission::Strike { contact: cid },
                    });
                    self.launched_strike = true;
                }
            }
        }

        // 5. Evasive posture when torpedoes are inbound: turn away + flank.
        for u in &obs.units {
            if u.kind != PlatformKind::Ship {
                continue;
            }
            let torpedo_inbound = obs.contacts.iter().any(|c| {
                c.category == PlatformKind::Torpedo
                    && c.estimate.range_to(u.pos) < 4.0
            });
            if torpedo_inbound {
                cmds.push(Command::DeployChaff { unit: u.id });
                cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 30.0 });
            }
        }

        let _ = enemy;
        cmds
    }
}

fn nearest_contact(
    obs: &Observation,
    from: &UnitObs,
    kinds: &[PlatformKind],
) -> Option<(ContactId, f64)> {
    obs.contacts
        .iter()
        .filter(|c| !c.stale && c.quality > 0.3 && kinds.contains(&c.category))
        .map(|c| (c.id, from.pos.range_to(c.estimate)))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
}

// ---------------------------------------------------------------------------
// OpenAI-compatible LLM staff
// ---------------------------------------------------------------------------

pub struct OpenAiCompatStaff {
    #[allow(dead_code)]
    pub side: Side,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

impl OpenAiCompatStaff {
    pub fn from_env(side: Side) -> Option<Self> {
        let base_url = std::env::var("HARPOON_LLM_BASE_URL").ok()?;
        Some(Self {
            side,
            base_url,
            api_key: std::env::var("HARPOON_LLM_API_KEY").unwrap_or_default(),
            model: std::env::var("HARPOON_LLM_MODEL").unwrap_or_else(|_| "deepseek-chat".into()),
        })
    }

    fn summarize(obs: &Observation) -> String {
        let units: Vec<_> = obs
            .units
            .iter()
            .map(|u| {
                json!({
                    "id": u.id.0, "class": u.class, "pos": [u.pos.x, u.pos.y],
                    "hp": u.hp, "fire_on": u.fire_on,
                    "weapons": u.weapons.iter().map(|w| (&w.weapon, w.ammo)).collect::<Vec<_>>(),
                })
            })
            .collect();
        let contacts: Vec<_> = obs
            .contacts
            .iter()
            .filter(|c| !c.stale)
            .map(|c| {
                json!({
                    "id": c.id.0, "category": c.category, "class": c.class_name,
                    "pos": [c.estimate.x, c.estimate.y], "quality": c.quality,
                })
            })
            .collect();
        serde_json::to_string(&json!({
            "tick": obs.tick, "side": obs.your_side.name(),
            "units": units, "contacts": contacts,
        }))
        .unwrap_or_default()
    }
}

impl CommandStaff for OpenAiCompatStaff {
    fn name(&self) -> &'static str {
        "openai_compat"
    }

    fn decide(&mut self, obs: &Observation) -> Vec<Command> {
        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": "You are a naval task-force commander AI for a real-time wargame. Reply ONLY with JSON: {\"commands\": [{\"cmd\": \"set_weapons_free\", ...}]} using the harpoon command schema. Available cmds: set_course{unit,course_deg}, set_speed{unit,speed_kts}, goto{unit,waypoints}, assign_target{unit,contact}, fire{unit,contact,weapon,rounds}, deploy_chaff{unit}, launch_aircraft{unit,class,count,mission{mission:cap|strike|asw,...}}, set_weapons_free{unit,free}."},
                {"role": "user", "content": Self::summarize(obs)}
            ],
            "temperature": 0.2,
            "max_tokens": 1500,
        });
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let resp = ureq::post(&url)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(body);
        match resp {
            Ok(r) => match r.into_json::<ChatResponse>() {
                Ok(chat) => {
                    let content = chat
                        .choices
                        .first()
                        .map(|c| c.message.content.clone())
                        .unwrap_or_default();
                    parse_llm_commands(&content, obs)
                }
                Err(e) => {
                    eprintln!("[llm-staff] bad response: {e}");
                    Vec::new()
                }
            },
            Err(e) => {
                eprintln!("[llm-staff] request failed: {e}; falling back to no-op");
                Vec::new()
            }
        }
    }
}

/// Best-effort parse of the LLM's command JSON. Numbers arrive as f64 via
/// serde_json, ids are mapped back to engine ids.
pub fn parse_llm_commands(content: &str, obs: &Observation) -> Vec<Command> {
    let json_str = content.trim().trim_start_matches("```json").trim_end_matches("```");
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return Vec::new();
    };
    let Some(list) = v.get("commands").and_then(|c| c.as_array()) else {
        return Vec::new();
    };
    let unit_ids: Vec<EntityId> = obs.units.iter().map(|u| u.id).collect();
    let contact_ids: Vec<ContactId> = obs.contacts.iter().map(|c| c.id).collect();
    let mut out = Vec::new();
    for c in list {
        let get_u32 = |k: &str| c.get(k).and_then(|x| x.as_u64()).map(|x| x as u32);
        let get_f64 = |k: &str| c.get(k).and_then(|x| x.as_f64());
        let kind = c.get("cmd").and_then(|x| x.as_str()).unwrap_or("");
        let cmd = match kind {
            "set_course" => get_u32("unit").filter(|id| unit_ids.contains(&EntityId(*id))).map(|id| {
                Command::SetCourse { unit: EntityId(id), course_deg: get_f64("course_deg").unwrap_or(0.0) }
            }),
            "set_speed" => get_u32("unit").filter(|id| unit_ids.contains(&EntityId(*id))).map(|id| {
                Command::SetSpeed { unit: EntityId(id), speed_kts: get_f64("speed_kts").unwrap_or(0.0) }
            }),
            "assign_target" => match (
                get_u32("unit").filter(|id| unit_ids.contains(&EntityId(*id))),
                get_u32("contact").filter(|id| contact_ids.contains(&ContactId(*id))),
            ) {
                (Some(u), Some(c)) => Some(Command::AssignTarget { unit: EntityId(u), contact: ContactId(c) }),
                _ => None,
            },
            "fire" => match (
                get_u32("unit").filter(|id| unit_ids.contains(&EntityId(*id))),
                get_u32("contact").filter(|id| contact_ids.contains(&ContactId(*id))),
            ) {
                (Some(u), Some(ct)) => Some(Command::Fire {
                    unit: EntityId(u),
                    contact: ContactId(ct),
                    weapon: c.get("weapon").and_then(|x| x.as_str()).unwrap_or("gun_5in").into(),
                    rounds: get_f64("rounds").unwrap_or(1.0) as u8,
                }),
                _ => None,
            },
            "deploy_chaff" => get_u32("unit").filter(|id| unit_ids.contains(&EntityId(*id))).map(|id| {
                Command::DeployChaff { unit: EntityId(id) }
            }),
            "set_weapons_free" => get_u32("unit").filter(|id| unit_ids.contains(&EntityId(*id))).map(|id| {
                Command::SetWeaponsFree { unit: EntityId(id), free: c.get("free").and_then(|x| x.as_bool()).unwrap_or(true) }
            }),
            _ => None,
        };
        if let Some(cmd) = cmd {
            out.push(cmd);
        }
    }
    out
}

/// Build the staff stack for a side: LLM if configured, heuristic fallback.
pub fn make_staff(side: Side) -> Box<dyn CommandStaff> {
    match OpenAiCompatStaff::from_env(side) {
        Some(s) => Box::new(s),
        None => Box::new(HeuristicStaff::new(side)),
    }
}

#[derive(Serialize)]
#[allow(dead_code)]
struct StaffInfo {
    name: &'static str,
}

#[allow(dead_code)]
fn describe(s: &dyn CommandStaff) -> StaffInfo {
    StaffInfo { name: s.name() }
}

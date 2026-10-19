//! Two-sided adversarial director.
//!
//! Mode 2 (chosen by the operator): a **private** wake condition immediately
//! pauses both sides. Both agents plan from their own double-blind snapshot;
//! commands apply on the next tick and the clock resumes at the **minimum** of
//! the two requested speeds. Wake conditions are a whitelisted, server-
//! evaluated DSL — the model never runs code.
//!
//! This module contains the pure evaluation/state logic. The server task that
//! owns the world and invokes the agents lives in `main.rs`.

use crate::agent::{WakeCondition, WakeRule};
use harpoon_core::events::EventKind;
use harpoon_core::observe::{observe, Observation};
use harpoon_core::types::{ContactId, PlatformKind, Side};
use harpoon_core::world::{VictoryResult, World};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

/// Minimum simulated time between any two private wakes. This keeps a
/// competitive match from spending all wall-clock time in 20–30s LLM plans
/// while still allowing 120s adversarial time wakes.
pub const MIN_WAKE_GAP_S: i64 = 60;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WakeHit {
    pub side: Side,
    pub rule: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct State {
    pub active: bool,
    pub phase: String,
    pub id: u64,
    pub scenario_id: String,
    pub tick: i64,
    pub duration_s: i64,
    pub trigger: Vec<WakeHit>,
    pub blue_speed: f64,
    pub red_speed: f64,
    pub global_speed: f64,
    pub blue_rules: Vec<WakeRule>,
    pub red_rules: Vec<WakeRule>,
    pub blue_report: String,
    pub red_report: String,
    pub last_plan_tick: i64,
    pub planning_started_unix: u64,
    pub error: Option<String>,
    pub over: bool,
    pub result: Option<VictoryResult>,
    pub blue_kills: u32,
    pub red_kills: u32,
    pub blue_losses: u32,
    pub red_losses: u32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            active: false,
            phase: "idle".into(),
            id: 0,
            scenario_id: String::new(),
            tick: 0,
            duration_s: 0,
            trigger: Vec::new(),
            blue_speed: 15.0,
            red_speed: 15.0,
            global_speed: 15.0,
            blue_rules: Vec::new(),
            red_rules: Vec::new(),
            blue_report: String::new(),
            red_report: String::new(),
            last_plan_tick: 0,
            planning_started_unix: 0,
            error: None,
            over: false,
            result: None,
            blue_kills: 0,
            red_kills: 0,
            blue_losses: 0,
            red_losses: 0,
        }
    }
}

impl State {
    pub fn refresh_outcome(&mut self, world: &World) {
        self.tick = world.tick;
        self.over = world.over;
        self.result = world.result.clone();
        let (blue_kills, red_kills) = world.kill_counts();
        self.blue_kills = blue_kills;
        self.red_kills = red_kills;
        self.blue_losses = world.losses[Side::Blue.index()];
        self.red_losses = world.losses[Side::Red.index()];
    }
}

#[derive(Clone, Debug)]
pub struct WakeRuntime {
    pub rules: Vec<WakeRule>,
    pub last_fired: BTreeMap<String, i64>,
    pub fired_once: BTreeSet<String>,
    /// Whether a persistent range condition was true on the previous check.
    pub rule_active: BTreeMap<String, bool>,
    /// A newly installed persistent range rule is armed on its first
    /// evaluation without firing; it must go false then true to wake.
    pub armed: BTreeSet<String>,
    /// Highest quality seen for each contact, for threshold-crossing wakes.
    pub known_quality: BTreeMap<ContactId, f64>,
    /// Event cursor used by the per-tick evaluator.
    pub eval_event_seq: u64,
    /// Event cursor used for the next agent observation.
    pub plan_event_seq: u64,
    pub last_plan_tick: i64,
    pub last_wake_tick: i64,
    pub desired_speed: f64,
    pub side: Side,
}

impl WakeRuntime {
    pub fn new(side: Side) -> Self {
        Self {
            rules: default_rules(),
            last_fired: BTreeMap::new(),
            fired_once: BTreeSet::new(),
            rule_active: BTreeMap::new(),
            armed: BTreeSet::new(),
            known_quality: BTreeMap::new(),
            eval_event_seq: 0,
            plan_event_seq: 0,
            last_plan_tick: 0,
            last_wake_tick: -3600,
            desired_speed: 15.0,
            side,
        }
    }

    pub fn set_rules(&mut self, rules: Vec<WakeRule>) {
        self.rules = if rules.iter().any(|r| matches!(r.condition, WakeCondition::Time { .. })) {
            rules
        } else {
            let mut rules = rules;
            rules.extend(default_rules());
            rules
        };
    }

    pub fn mark_wake(&mut self, tick: i64) {
        self.last_wake_tick = tick;
    }

    /// Observation for a planning turn; advances the plan event cursor.
    pub fn planning_observation(&mut self, world: &World) -> Observation {
        let obs = observe(world, self.side, self.plan_event_seq, 160);
        self.plan_event_seq = obs.events.iter().map(|e| e.seq).max().unwrap_or(self.plan_event_seq);
        self.last_plan_tick = obs.tick;
        obs
    }

    /// Evaluate all rules against this side's current observation. Advances
    /// the evaluator event cursor and contact-quality cache.
    pub fn evaluate(&mut self, world: &World) -> Option<WakeHit> {
        let obs = observe(world, self.side, self.eval_event_seq, 160);
        self.eval_event_seq = obs.events.iter().map(|e| e.seq).max().unwrap_or(self.eval_event_seq);
        let now = obs.tick;
        let gated = now - self.last_wake_tick < MIN_WAKE_GAP_S;
        let mut hit: Option<WakeHit> = None;

        for rule in self.rules.clone() {
            if self.fired_once.contains(&rule.id) {
                continue;
            }
            if let Some(last) = self.last_fired.get(&rule.id) {
                if rule.cooldown_s > 0 && now - *last < rule.cooldown_s {
                    continue;
                }
            }
            let matched: Option<String> = match &rule.condition {
                WakeCondition::Time { every_s } => {
                    if now - self.last_plan_tick >= *every_s {
                        Some(format!("time {}s since last plan", every_s))
                    } else {
                        None
                    }
                }
                WakeCondition::NewContact { category, min_quality } => {
                    let current = self.new_or_improved_contact(&obs, category.as_deref(), *min_quality);
                    current.map(|c| format!("new contact C{} ({:?})", c, obs.contacts.iter().find(|x| x.id == c).and_then(|x| x.class_name.clone()).unwrap_or_else(|| "unidentified".into())))
                }
                WakeCondition::Incoming { max_eta_s } => obs
                    .events
                    .iter()
                    .filter_map(|event| match &event.kind {
                        EventKind::Incoming { side, contact, eta_s } if *side == self.side => {
                            if max_eta_s.map(|m| *eta_s <= m).unwrap_or(true) {
                                Some(format!("incoming C{} eta {:.0}s", contact.0, eta_s))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .next(),
                WakeCondition::OwnLoss => obs
                    .events
                    .iter()
                    .find_map(|event| match &event.kind {
                        EventKind::Sunk { side, .. }
                        | EventKind::AirShotDown { side, .. }
                        | EventKind::Ditched { side, .. }
                        | EventKind::BaseDestroyed { side, .. }
                            if *side == self.side =>
                        {
                            Some(format!("own loss: {}", event.kind.name()))
                        }
                        _ => None,
                    }),
                WakeCondition::OwnDamage { min_damage, hp_fraction_below } => obs
                    .events
                    .iter()
                    .find_map(|event| match &event.kind {
                        EventKind::Impact { side, unit, damage, hp_left, .. } if *side == self.side => {
                            let hp_low = obs
                                .units
                                .iter()
                                .find(|u| u.id == *unit)
                                .map(|u| u.hp_max > 0.0 && *hp_left / u.hp_max < hp_fraction_below.unwrap_or(1.0))
                                .unwrap_or(true);
                            if damage >= &min_damage.unwrap_or(20.0) || hp_low {
                                Some(format!("own damage on #{}: {:.1}", unit.0, damage))
                            } else {
                                None
                            }
                        }
                        EventKind::CriticalHit { side, unit, system } if *side == self.side => {
                            Some(format!("critical hit on #{}: {}", unit.0, system))
                        }
                        _ => None,
                    }),
                WakeCondition::ContactRange { max_nm, category, min_quality } => {
                    let mut found = None;
                    'contacts: for contact in &obs.contacts {
                        if contact.stale {
                            continue;
                        }
                        if !category_matches(category.as_deref(), contact.category) {
                            continue;
                        }
                        if contact.quality < min_quality.unwrap_or(0.0) {
                            continue;
                        }
                        for unit in &obs.units {
                            if unit.pos.range_to(contact.estimate) <= *max_nm {
                                found = Some(format!("contact C{} within {:.0}nm", contact.id.0, max_nm));
                                break 'contacts;
                            }
                        }
                    }
                    let active = found.is_some();
                    if !self.armed.contains(&rule.id) {
                        self.armed.insert(rule.id.clone());
                        self.rule_active.insert(rule.id.clone(), active);
                        None
                    } else {
                        let previous = self.rule_active.get(&rule.id).copied().unwrap_or(false);
                        self.rule_active.insert(rule.id.clone(), active);
                        if active && !previous { found } else { None }
                    }
                }
            };

            if let Some(detail) = matched {
                if gated {
                    continue;
                }
                self.last_fired.insert(rule.id.clone(), now);
                if rule.one_shot {
                    self.fired_once.insert(rule.id.clone());
                }
                if hit.is_none() {
                    hit = Some(WakeHit { side: self.side, rule: rule.id, detail });
                }
            }
        }
        hit
    }

    fn new_or_improved_contact(
        &mut self,
        obs: &Observation,
        category: Option<&str>,
        min_quality: Option<f64>,
    ) -> Option<ContactId> {
        let threshold = min_quality.unwrap_or(0.0);
        let mut found = None;
        for contact in &obs.contacts {
            if contact.stale || !category_matches(category, contact.category) {
                continue;
            }
            let previous = self.known_quality.get(&contact.id).copied();
            if contact.quality >= threshold
                && previous.map(|q| q < threshold).unwrap_or(true)
            {
                found.get_or_insert(contact.id);
            }
            let entry = self.known_quality.entry(contact.id).or_insert(0.0);
            if contact.quality > *entry {
                *entry = contact.quality;
            }
        }
        found
    }
}

pub fn default_rules() -> Vec<WakeRule> {
    vec![WakeRule {
        id: "cadence".into(),
        condition: WakeCondition::Time { every_s: 300 },
        cooldown_s: 0,
        one_shot: false,
    }]
}

pub fn effective_rules(rules: &[WakeRule]) -> Vec<WakeRule> {
    if rules.iter().any(|r| matches!(r.condition, WakeCondition::Time { .. })) {
        rules.to_vec()
    } else {
        let mut out = rules.to_vec();
        out.extend(default_rules());
        out
    }
}

pub fn category_matches(filter: Option<&str>, kind: PlatformKind) -> bool {
    let Some(filter) = filter else { return true };
    let f = filter.to_ascii_lowercase();
    let actual = match kind {
        PlatformKind::Ship => "surface",
        PlatformKind::Sub => "subsurface",
        PlatformKind::Aircraft => "air",
        PlatformKind::Missile => "missile",
        PlatformKind::Torpedo => "torpedo",
        PlatformKind::Base => "land",
    };
    f == actual || (f == "sub" && actual == "subsurface") || f == "unknown"
}

pub fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wake_rule_json_shapes_parse() {
        let time: WakeRule = serde_json::from_value(json!({
            "id":"t","type":"time","every_s":300,"cooldown_s":0,"one_shot":false
        })).unwrap();
        assert!(matches!(time.condition, WakeCondition::Time { every_s: 300 }));
        let contact: WakeRule = serde_json::from_value(json!({
            "id":"c","type":"new_contact","category":"surface","min_quality":0.5,
            "cooldown_s":60,"one_shot":false
        })).unwrap();
        match contact.condition {
            WakeCondition::NewContact { category, min_quality } => {
                assert_eq!(category.as_deref(), Some("surface"));
                assert_eq!(min_quality, Some(0.5));
            }
            _ => panic!("wrong condition"),
        }
    }

    #[test]
    fn category_aliases_and_time_fallback() {
        assert!(category_matches(Some("surface"), PlatformKind::Ship));
        assert!(category_matches(Some("sub"), PlatformKind::Sub));
        assert!(category_matches(Some("air"), PlatformKind::Aircraft));
        assert!(!category_matches(Some("air"), PlatformKind::Ship));
        let rules = effective_rules(&[]);
        assert!(rules.iter().any(|r| matches!(r.condition, WakeCondition::Time { .. })));
    }
}

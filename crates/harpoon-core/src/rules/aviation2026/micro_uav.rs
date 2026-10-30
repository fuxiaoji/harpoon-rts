//! §4.5.2/§4.5.3 very-small HITL UAV low-flight exemption (rules 1 Jan 26,
//! PDF34, printed 4-2; page visually reviewed 2026-10-30). The exemption is
//! never inferred from a model name, an "unmanned" label or automation state:
//! it requires an explicit scenario-author micro qualification AND the actual
//! unmanned §2.5 hardware fit in established HITL with a live control link,
//! during daylight in printed clear weather (visibility 40% or more).
//! Larger UAVs keep rolling on the printed SSkim/NOE table every Tactical
//! Turn; a `None` configuration keeps the unchanged legacy low-flight roll
//! for every aircraft. No probability, threshold or endurance is invented.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClassQualification {
    /// Must be scenario_assumption-authored. The source pages print no real
    /// hardware micro list; no real DB class is claimed as micro here.
    pub source: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Configuration {
    pub source: String,
    /// Explicit author micro qualification keyed by exact DB class id.
    pub micro_classes: BTreeMap<String, ClassQualification>,
}

impl Configuration {
    fn authored_qualification(&self, class: &str) -> bool {
        if !(self.source == "scenario_assumption" || self.source.starts_with("scenario_assumption:")) {
            return false;
        }
        self.micro_classes.get(class).is_some_and(|q| {
            q.source == "scenario_assumption" || q.source.starts_with("scenario_assumption:")
        })
    }
}

/// Every printed condition of the §4.5.3 exemption, individually visible so
/// public cases can name the exact failing premise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Eligibility {
    pub author_micro: bool,
    pub actual_unmanned: bool,
    pub hitl_live_link: bool,
    pub daylight: bool,
    pub clear_weather: bool,
}

impl Eligibility {
    pub fn complete(self) -> bool {
        self.author_micro
            && self.actual_unmanned
            && self.hitl_live_link
            && self.daylight
            && self.clear_weather
    }
}

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/micro-uav-low-flight-2026.json"
        ))
        .expect("verified very-small HITL UAV exemption source")
    })
}

fn clear_min_visibility() -> f64 {
    tables()["source_values"]["clear_weather_min_visibility_percent"]
        ["value"]
        .as_f64()
        .expect("printed clear-weather visibility threshold")
}

/// The five printed premises of the §4.5.3 exemption for one aircraft.
pub fn eligibility(
    w: &World,
    id: EntityId,
    config: Option<&Configuration>,
) -> Result<Eligibility, String> {
    let e = w
        .entity(id)
        .filter(|e| e.alive && e.kind == crate::types::PlatformKind::Aircraft)
        .ok_or("4.5.2/4.5.3 aircraft absent")?;
    let author_micro = config.is_some_and(|c| c.authored_qualification(&e.class));
    let actual_unmanned = crate::rules::unmanned2026::is_configured(w, id);
    // Established HITL with a live link: rejects autonomous mode, one-turn
    // transitions, loss states and every failed link_with gate (range,
    // bandwidth availability, disabled, interference).
    let hitl_live_link = actual_unmanned
        && crate::rules::unmanned2026::can_control(w, e.side, id).is_ok();
    let daylight = w.day_night == crate::types::DayNight::Day;
    let clear_weather = w
        .context
        .aviation_2026
        .as_ref()
        .and_then(|c| c.visibility_percent)
        .is_some_and(|v| v.is_finite() && (clear_min_visibility()..=100.).contains(&v));
    Ok(Eligibility {
        author_micro,
        actual_unmanned,
        hitl_live_link,
        daylight,
        clear_weather,
    })
}

/// The safety_step hook calls this inside the SSkim/NOE (<=20 m) branch after
/// the engine-TS and high-altitude skips. `None` — or any incomplete printed
/// condition — keeps the unchanged printed crash roll.
pub fn exempt_with(w: &World, id: EntityId, config: Option<&Configuration>) -> bool {
    eligibility(w, id, config)
        .map(|e| e.complete())
        .unwrap_or(false)
}

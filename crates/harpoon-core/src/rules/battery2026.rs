//! Source-bound submarine battery/AIP accounting for HV2026 §3.2.4–3.2.5.
//!
//! This module is deliberately opt-in. Annex ratings establish capacity, not
//! starting charge. Missing charge, chemistry, snorkel, and AIP fuel remain
//! unknown and cannot be filled from platform class defaults.
pub mod replenishment;
#[path="battery2026/annex_extra.rs"]
pub mod annex_extra;

use crate::{EntityId, EventKind, Side, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/battery-2026.json")).unwrap()
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    Intermediate,
    Tactical,
}
impl Resolution {
    fn seconds(self) -> i64 {
        match self {
            Self::Intermediate => 1800,
            Self::Tactical => 180,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Old10KtPolicy {
    TableLiteral,
    ProseDouble,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeCombinationPolicy {
    MultiplyHalfRate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerMode {
    Diesel,
    Battery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeRateEra {
    Pre1970,
    Post1970,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub resolution: Resolution,
    #[serde(default)]
    pub old_10kt_policy: Option<Old10KtPolicy>,
    /// Keyed by the exact scenario entity name. The configured platform_class
    /// must exactly match the entity's stable database class ID.
    pub units: BTreeMap<String, UnitConfiguration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub replenishment: Option<replenishment::Configuration>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitConfiguration {
    pub source: String,
    pub platform_class: String,
    /// Explicit scenario state; null means unknown, never full by default.
    pub initial_charge_units: Option<f64>,
    pub initial_aip_remaining_units: Option<f64>,
    pub initial_aip_operating: bool,
    pub initial_charging: bool,
    pub initial_power_mode: PowerMode,
    pub initial_mast_raised: bool,
    /// Explicit fit declaration; None is unknown and cannot authorize use.
    pub snorkel_fitted: Option<bool>,
    /// true/false only when Annex/scenario explicitly establishes chemistry.
    pub lithium_ion: Option<bool>,
    /// Charge table is indexed by build era, not Annex battery technology.
    pub charge_rate_era: Option<ChargeRateEra>,
    /// The source does not identify this denominator; it must be authored.
    pub full_speed_reference_kts: Option<f64>,
    /// Required only when speed is above half the full-speed reference.
    pub charge_combination_policy: Option<ChargeCombinationPolicy>,
    /// For speeds where no source curve is printed. At exactly 4/5/9 kt,
    /// source demand/examples select 2/3/3 units/IT and ignore this override.
    pub aip_output_units_per_intermediate_turn: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub units: BTreeMap<EntityId, UnitState>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub replenishment: Option<replenishment::State>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitState {
    pub source: String,
    pub platform_class: String,
    pub capacity_units: f64,
    pub original_capacity_units: f64,
    pub new_battery_technology: bool,
    pub charge_units: Option<f64>,
    pub aip_fitted: bool,
    pub aip_remaining_units: Option<f64>,
    pub aip_operational: bool,
    pub snorkel_fitted: Option<bool>,
    pub mast_raised: bool,
    /// Unknown on an older raised-mast save; never invent an exposure age.
    #[serde(default)]
    pub mast_raised_since: Option<i64>,
    pub lithium_ion: Option<bool>,
    pub charge_rate_era: Option<ChargeRateEra>,
    pub full_speed_reference_kts: Option<f64>,
    pub charge_combination_policy: Option<ChargeCombinationPolicy>,
    pub aip_output_units_per_intermediate_turn: Option<f64>,
    pub charging: bool,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub stowed_at: Option<i64>,
    pub power_mode: PowerMode,
    pub turn_started_at: i64,
    pub turn_speed_knot_seconds: f64,
    pub turn_sample_seconds: i64,
    pub turn_battery_powered_seconds: i64,
    #[serde(default)]
    pub turn_aip_seconds: i64,
    #[serde(default)]
    pub turn_aip_speed_knot_seconds: f64,
    /// Output already allocated this Intermediate Turn is reserved even if
    /// the operator switches AIP off before its potential settles.
    #[serde(default)]
    pub intermediate_aip_spent_units: f64,
    pub intermediate_unmet_battery_units: f64,
    pub intermediate_started_at: i64,
    pub intermediate_speed_knot_seconds: f64,
    pub intermediate_sample_seconds: i64,
    pub intermediate_eligible_charge_seconds: i64,
    /// Charge speed is sampled only while charging is actually eligible.
    /// None preserves an older save's missing interval history as unknown.
    #[serde(default)]
    pub intermediate_eligible_speed_knot_seconds: Option<f64>,
    pub aip_critical_identities: BTreeSet<String>,
    pub capacity_critical_identities: BTreeSet<String>,
    /// Cumulative percentage against the original maximum is explicit RTS
    /// policy; repeated criticals are capped at 100%.
    pub cumulative_capacity_loss_fraction: f64,
    pub pending_adjudications: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    StartCharging {},
    StopCharging {},
    RaiseSnorkel {},
    RetractMasts {},
    SetAipOperating { enabled: bool },
    SetPowerMode { mode: PowerMode },
    StartReplenishment { provider:String,units:f64 },
    CancelReplenishment {},
}

pub fn initialize(w: &mut World) -> Result<(), String> {
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return Ok(());
    }
    let Some(config) = &w.context.battery_2026 else {
        w.rules.battery = None;
        return Ok(());
    };
    if config.source.trim().is_empty() {
        return Err("3.2 battery configuration needs source label".into());
    }
    let mut state = State::default();
    for (name, cfg) in &config.units {
        if cfg.source.trim().is_empty() {
            return Err("3.2 battery unit needs source/assumption label".into());
        }
        let matching: Vec<_> = w
            .entities
            .values()
            .filter(|e| &e.name == name)
            .collect();
        if matching.len() != 1 {
            return Err("3.2 configured battery unit must resolve to one exact unique entity".into());
        }
        let e = matching[0];
        if e.kind != crate::types::PlatformKind::Sub {
            return Err("3.2 battery fit requires submarine entity".into());
        }
        if cfg.platform_class != e.class {
            return Err(
                "3.2 battery platform_class must exactly match stable entity class ID".into(),
            );
        }
        let battery_fit = super::submarine_annex::battery_fit(w, &e.class)
            .ok_or("3.2 exact Annex row has no verified battery fit")?;
        let capacity = battery_fit["rating_charge_units"]
            .as_f64()
            .ok_or("3.2 Annex Btry Rtng missing")?;
        let technology = battery_fit["technology_as_printed"]
            .as_str()
            .ok_or("3.2 Annex battery technology missing")?;
        let new_technology = match technology {
            "new" => true,
            "old" => false,
            _ => {
                return Err(
                    "3.2 Annex battery technology does not select a source rate column".into(),
                )
            }
        };
        let aip = battery_fit["aip_fit"]
            .as_bool()
            .ok_or("3.2 Annex AIP marker unresolved")?;
        let check_value = |value: Option<f64>, max: f64, label: &str| -> Result<(), String> {
            if value.is_some_and(|v| !v.is_finite() || v < 0. || v > max) {
                return Err(format!("3.2 {label} must be within 0..={max}"));
            }
            Ok(())
        };
        check_value(cfg.initial_charge_units, capacity, "initial charge")?;
        if battery_fit["aip_potential_runtime_policy"].is_string()
            && cfg.initial_aip_remaining_units.is_some_and(|v| v > 2000.) {
            return Err("3.2.5 source_pending: Annex AIP potential exceeds English2026 2000-unit limit".into());
        }
        check_value(
            cfg.initial_aip_remaining_units,
            2000., // English2026 §3.2.5; higher Annex potential remains source-pending
            "initial AIP remaining",
        )?;
        if !aip && (cfg.initial_aip_remaining_units.is_some() || cfg.initial_aip_operating) {
            return Err("3.2 AIP state supplied for Annex row without AIP".into());
        }
        if aip
            && cfg.initial_aip_operating
            && !cfg.initial_aip_remaining_units.is_some_and(|v| v > 0.0)
        {
            return Err(
                "3.2 AIP operating requires explicit positive current remaining potential".into(),
            );
        }
        if cfg.initial_mast_raised && cfg.snorkel_fitted != Some(true) {
            return Err("3.2 raised snorkel requires explicit fitted snorkel".into());
        }
        if cfg.initial_mast_raised && e.speed_kts > 15. {
            return Err("3.2.1 no mast may be raised above 15 kt".into());
        }
        if cfg.initial_mast_raised && e.depth_m > 25.0 {
            return Err(
                "3.2.1 snorkel mast requires surfaced or Periscope/Snorkeling depth".into(),
            );
        }
        if cfg
            .full_speed_reference_kts
            .is_some_and(|v| !v.is_finite() || v <= 0.)
        {
            return Err("3.2 full-speed reference must be positive and finite".into());
        }
        if let Some(v) = cfg.aip_output_units_per_intermediate_turn {
            if !v.is_finite() || !(0.0..=3.0).contains(&v) {
                return Err("3.2 AIP output assumption must be 0..=3 per Intermediate Turn".into());
            }
        }
        if cfg.initial_charging && cfg.initial_charge_units.is_none() {
            return Err("3.2 charging cannot resolve unknown initial charge".into());
        }
        if cfg.initial_charging && cfg.initial_aip_operating {
            return Err(
                "3.2.4 charging and §3.2.5 AIP power combination has no resolved source procedure"
                    .into(),
            );
        }
        if cfg.initial_charging && cfg.initial_power_mode != PowerMode::Diesel {
            return Err("3.2 charging and battery-powered propulsion cannot both be active".into());
        }
        if cfg.initial_charging
            && e.depth_m != 0.
            && !(at_snorkeling_depth(e.depth_m)
                && cfg.initial_mast_raised
                && cfg.snorkel_fitted == Some(true))
        {
            return Err("3.2 initial charging requires surfaced or a fitted, raised snorkel at source Periscope/Snorkeling depth (0–25 m)".into());
        }
        if cfg.initial_power_mode == PowerMode::Battery
            && (e.depth_m == 0. || (at_snorkeling_depth(e.depth_m) && cfg.initial_mast_raised))
        {
            return Err("3.2 battery-powered mode requires submerged state without the diesel-powered raised-snorkel condition".into());
        }
        if cfg.initial_power_mode == PowerMode::Diesel
            && e.depth_m > 0.
            && !(at_snorkeling_depth(e.depth_m)
                && cfg.initial_mast_raised
                && cfg.snorkel_fitted == Some(true))
            && !(cfg.initial_aip_operating && e.speed_kts <= 5.0)
        {
            return Err("3.2 submerged Diesel mode requires source P/S depth (0–25 m) with fitted raised snorkel or source-supported ≤5 kt AIP operation".into());
        }
        state.units.insert(
            e.id,
            UnitState {
                source: cfg.source.clone(),
                platform_class: e.class.clone(),
                capacity_units: capacity,
                original_capacity_units: capacity,
                new_battery_technology: new_technology,
                charge_units: cfg.initial_charge_units,
                aip_fitted: aip,
                aip_remaining_units: cfg.initial_aip_remaining_units,
                aip_operational: cfg.initial_aip_operating,
                snorkel_fitted: cfg.snorkel_fitted,
                mast_raised: cfg.initial_mast_raised,
                mast_raised_since: cfg.initial_mast_raised.then_some(w.tick),
                lithium_ion: cfg.lithium_ion,
                full_speed_reference_kts: cfg.full_speed_reference_kts,
                charge_rate_era: cfg.charge_rate_era,
                charge_combination_policy: cfg.charge_combination_policy,
                aip_output_units_per_intermediate_turn: cfg.aip_output_units_per_intermediate_turn,
                charging: cfg.initial_charging,
                stowed_at: None,
                power_mode: cfg.initial_power_mode,
                turn_started_at: w.tick,
                turn_speed_knot_seconds: 0.,
                turn_sample_seconds: 0,
                turn_battery_powered_seconds: 0,
                turn_aip_seconds: 0,
                turn_aip_speed_knot_seconds: 0.0,
                intermediate_aip_spent_units: 0.0,
                intermediate_unmet_battery_units: 0.0,
                intermediate_started_at: w.tick,
                intermediate_speed_knot_seconds: 0.,
                intermediate_sample_seconds: 0,
                intermediate_eligible_charge_seconds: 0,
                intermediate_eligible_speed_knot_seconds: Some(0.0),
                aip_critical_identities: BTreeSet::new(),
                capacity_critical_identities: BTreeSet::new(),
                cumulative_capacity_loss_fraction: 0.,
                pending_adjudications: Vec::new(),
            },
        );
    }
    w.rules.battery = Some(state);
    replenishment::initialize(w)?;
    Ok(())
}

fn table_discharge(speed: u8, is_new: bool, resolution: Resolution) -> Option<f64> {
    let rows = tables()["discharge"]["rows"].as_array()?;
    let index = usize::from(speed.checked_sub(4)?);
    let col = match (is_new, resolution) {
        (false, Resolution::Intermediate) => 0,
        (true, Resolution::Intermediate) => 1,
        (false, Resolution::Tactical) => 2,
        (true, Resolution::Tactical) => 3,
    };
    rows.get(index)?.get(col)?.as_f64()
}

fn charge_band(percent: f64) -> Option<usize> {
    if percent.is_finite() && percent <= 70.0 {
        Some(0)
    } else if percent.is_finite() && percent >= 71.0 {
        Some(1)
    } else {
        None
    }
}

fn resolved_discharge(
    speed: f64,
    new: bool,
    resolution: Resolution,
    policy: Option<Old10KtPolicy>,
) -> Result<f64, String> {
    let rounded = speed.round();
    if !speed.is_finite() || (speed - rounded).abs() > 1e-9 || !(4.0..=25.0).contains(&rounded) {
        return Err(format!(
            "§3.2.4 no source discharge row for turn-average speed {speed:.4} kt"
        ));
    }
    if !new && rounded == 10.0 {
        return match policy {
            Some(Old10KtPolicy::TableLiteral) => {
                table_discharge(10, false, resolution).ok_or("missing source cell".into())
            }
            Some(Old10KtPolicy::ProseDouble) => {
                Ok(2.0 * table_discharge(10, true, resolution).ok_or("missing source cell")?)
            }
            None => Err(
                "§3.2.4 R-062 old-technology 10 kt prose/table conflict needs explicit policy"
                    .into(),
            ),
        };
    }
    table_discharge(rounded as u8, new, resolution).ok_or("§3.2.4 no source discharge cell".into())
}

fn aip_output_rate(speed: f64, explicit: Option<f64>) -> Result<f64, String> {
    if !speed.is_finite() || speed < 0.0 {
        return Err("§3.2.5 invalid AIP interval speed".into());
    }
    if (speed - 4.0).abs() <= 1e-9 {
        return Ok(2.0);
    }
    if (speed - 5.0).abs() <= 1e-9 || (speed - 9.0).abs() <= 1e-9 {
        return Ok(3.0);
    }
    if speed >= 20.0 {
        return Err(
            "§3.2.5 AIP contributes <10% at ≥20 kt but no charge-unit conversion is printed".into(),
        );
    }
    explicit.ok_or_else(|| format!("§3.2.5 no quantified output for AIP interval average {speed:.4} kt; explicit scenario output required"))
}

fn add_pending(u: &mut UnitState, message: String) {
    u.pending_adjudications.push(message);
    if u.pending_adjudications.len() > 64 {
        u.pending_adjudications.remove(0);
    }
}

fn at_snorkeling_depth(depth_m: f64) -> bool {
    depth_m > 0.0 && depth_m <= 25.0
}

fn trace(w: &mut World, id: EntityId, rule: &str, mut value: Value) {
    if let Some(source) = w
        .rules
        .battery
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .map(|u| u.source.clone())
    {
        if let Some(fields) = value.as_object_mut() {
            fields.insert("configuration_source".into(), json!(source));
        }
    }
    let side = w.entity(id).map(|e| e.side);
    w.emit(
        EventKind::RulesEvent {
            domain: "battery2026".into(),
            rule: rule.into(),
            side,
            unit: Some(id),
            contact: None,
            detail: value.to_string(),
        },
        side.map(Visibility::Side).unwrap_or(Visibility::Referee),
    );
}

pub fn step(w: &mut World) {
    replenishment::step(w);
    let ids:Vec<_>=w.rules.battery.as_ref().map(|s|s.units.keys().copied().collect()).unwrap_or_default();
    step_selected(w,ids);
}
fn step_selected(w:&mut World,ids:Vec<EntityId>){
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return;
    }
    let Some(resolution) = w.context.battery_2026.as_ref().map(|c| c.resolution) else {
        return;
    };
    let duration = resolution.seconds();
    for id in ids {
        let Some(entity) = w.entity(id)
            .filter(|e| e.alive && e.kind == crate::types::PlatformKind::Sub)
            .cloned() else {
            continue;
        };
        if !w.rules.battery.as_ref().and_then(|s| s.units.get(&id))
            .is_some_and(|u| u.platform_class == entity.class && u.stowed_at.is_none()) {
            continue;
        }
        let (
            discharge_boundary,
            discharge_avg,
            powered_seconds,
            aip_seconds,
            aip_avg,
            intermediate_boundary,
            intermediate_avg,
            eligible_seconds,
        ) = {
            let Some(state) = w.rules.battery.as_mut().and_then(|s| s.units.get_mut(&id)) else {
                continue;
            };
            let turn_elapsed = (w.tick - state.turn_started_at).clamp(0, duration);
            if turn_elapsed > state.turn_sample_seconds {
                let sample = turn_elapsed - state.turn_sample_seconds;
                let aip_on = state.aip_operational
                    && state.aip_fitted
                    && state.aip_critical_identities.is_empty()
                    && state.aip_remaining_units.is_some_and(|v| v > 0.0);
                let submerged_electric = entity.depth_m > 0.0
                    && (state.power_mode == PowerMode::Battery
                        || (aip_on && !(at_snorkeling_depth(entity.depth_m) && state.mast_raised)));
                if submerged_electric {
                    state.turn_speed_knot_seconds += entity.speed_kts * sample as f64;
                    state.turn_battery_powered_seconds += sample;
                    if aip_on {
                        state.turn_aip_seconds += sample;
                        state.turn_aip_speed_knot_seconds += entity.speed_kts * sample as f64;
                    }
                }
                state.turn_sample_seconds = turn_elapsed;
            }
            let it_elapsed = (w.tick - state.intermediate_started_at).clamp(0, 1800);
            if it_elapsed > state.intermediate_sample_seconds {
                let sample = it_elapsed - state.intermediate_sample_seconds;
                state.intermediate_speed_knot_seconds += entity.speed_kts * sample as f64;
                if state.charging
                    && (entity.depth_m == 0.
                        || (at_snorkeling_depth(entity.depth_m)
                            && state.mast_raised
                            && state.snorkel_fitted == Some(true)))
                {
                    state.intermediate_eligible_charge_seconds += sample;
                    if let Some(speed_sample) = &mut state.intermediate_eligible_speed_knot_seconds {
                        *speed_sample += entity.speed_kts * sample as f64;
                    }
                }
                state.intermediate_sample_seconds = it_elapsed;
            }
            let d = w.tick - state.turn_started_at >= duration;
            let i = w.tick - state.intermediate_started_at >= 1800;
            let davg = if d && state.turn_battery_powered_seconds > 0 {
                state.turn_speed_knot_seconds / state.turn_battery_powered_seconds as f64
            } else {
                0.
            };
            let powered = if d {
                state.turn_battery_powered_seconds
            } else {
                0
            };
            let aip_seconds = if d { state.turn_aip_seconds } else { 0 };
            let aip_avg = if aip_seconds > 0 {
                state.turn_aip_speed_knot_seconds / aip_seconds as f64
            } else {
                0.0
            };
            let iavg = if i && state.intermediate_eligible_charge_seconds > 0 {
                state.intermediate_eligible_speed_knot_seconds
                    .map(|v| v / state.intermediate_eligible_charge_seconds as f64)
            } else {
                Some(0.0)
            };
            let eligible = if i {
                state.intermediate_eligible_charge_seconds
            } else {
                0
            };
            if d {
                state.turn_started_at = w.tick;
                state.turn_speed_knot_seconds = 0.;
                state.turn_sample_seconds = 0;
                state.turn_battery_powered_seconds = 0;
                state.turn_aip_seconds = 0;
                state.turn_aip_speed_knot_seconds = 0.0;
            }
            if i {
                state.intermediate_started_at = w.tick;
                state.intermediate_speed_knot_seconds = 0.;
                state.intermediate_sample_seconds = 0;
                state.intermediate_eligible_charge_seconds = 0;
                state.intermediate_eligible_speed_knot_seconds = Some(0.0);
            }
            (d, davg, powered, aip_seconds, aip_avg, i, iavg, eligible)
        };
        let state = w
            .rules
            .battery
            .as_mut()
            .unwrap()
            .units
            .get_mut(&id)
            .unwrap();
        let mut trace_data = Vec::new();
        if discharge_boundary && powered_seconds > 0 {
            let policy = w
                .context
                .battery_2026
                .as_ref()
                .and_then(|c| c.old_10kt_policy);
            let mut issue: Option<String> = None;
            let mut supplied = 0.0;
            if aip_seconds > 0 {
                match aip_output_rate(aip_avg, state.aip_output_units_per_intermediate_turn) {
                    Ok(rate) => {
                        let available = state
                            .aip_remaining_units
                            .map(|v| (v - state.intermediate_aip_spent_units).max(0.0))
                            .unwrap_or(0.0);
                        let required = rate * aip_seconds as f64 / 1800.0;
                        supplied = required.min(available);
                        if aip_avg < 4.0 && (required <= 0.0 || supplied + 1e-9 < required) {
                            issue = Some("§3.2.5 low-speed AIP potential is insufficient; no below-4 kt battery fallback rate is printed".into());
                        }
                    }
                    Err(message) => issue = Some(message),
                }
            }
            let gross = if aip_seconds == powered_seconds
                && discharge_avg < 4.0
                && discharge_avg <= 5.0
                && issue.is_none()
            {
                // The low-speed no-battery rule is printed, but its reserve
                // draw outside 4/5 kt requires an explicit output assumption.
                Some(supplied)
            } else {
                match resolved_discharge(
                    discharge_avg,
                    state.new_battery_technology,
                    resolution,
                    policy,
                ) {
                    Ok(rate) => Some(rate * powered_seconds as f64 / duration as f64),
                    Err(message) => {
                        issue = Some(message);
                        None
                    }
                }
            };
            if issue.is_none() {
                if let Some(gross) = gross {
                    let requested = (gross - supplied).max(0.0);
                    match state.charge_units {
                        Some(charge) => {
                            let applied = requested.min(charge);
                            state.charge_units = Some((charge - applied).max(0.0));
                            state.intermediate_unmet_battery_units += requested - applied;
                        }
                        None if requested == 0.0 => {}
                        None => {
                            issue = Some(
                                "Battery current charge unknown; no debit can be applied safely"
                                    .into(),
                            )
                        }
                    }
                    if issue.is_none() {
                        state.intermediate_aip_spent_units += supplied;
                        trace_data.push(("3.2.discharge", json!({"average_speed_kts":discharge_avg,"battery_powered_seconds":powered_seconds,"aip_active_seconds":aip_seconds,"aip_output_reserved_units":supplied,"gross_units":gross,"battery_units":requested,"charge_units":state.charge_units,"resolution":resolution})));
                    }
                }
            }
            if let Some(message) = issue {
                add_pending(state, message.clone());
                trace_data.push(("3.2.discharge_pending",json!({"message":message,"average_speed_kts":discharge_avg,"aip_active_seconds":aip_seconds})));
            }
        }
        if discharge_boundary
            && powered_seconds == 0
            && entity.depth_m > 0.
            && state.power_mode == PowerMode::Diesel
            && !(at_snorkeling_depth(entity.depth_m)
                && state.mast_raised
                && state.snorkel_fitted == Some(true))
            && !(state.aip_operational
                && state.aip_fitted
                && state.aip_remaining_units.is_some()
                && entity.speed_kts <= 5.0)
        {
            let message="§3.2.4 submerged Diesel mode lacks a sourced snorkeling-depth/raised-snorkel condition and has no ≤5 kt AIP source; no free propulsion accounting assumed".to_string();
            add_pending(state, message.clone());
            trace_data.push((
                "3.2.power_source_pending",
                json!({"message":message,"depth_m":entity.depth_m,"speed_kts":entity.speed_kts}),
            ));
        }
        if discharge_boundary && state.power_mode == PowerMode::Battery && entity.depth_m == 0. {
            let message="§3.2.4 source says surfaced diesel submarines use diesel engines; battery propulsion mode at the surface is not inferred".to_string();
            add_pending(state, message.clone());
            trace_data.push((
                "3.2.power_source_pending",
                json!({"message":message,"depth_m":entity.depth_m}),
            ));
        }
        if intermediate_boundary {
            let mut issue: Option<String> = None;
            let aip_output = state.intermediate_aip_spent_units;
            state.intermediate_aip_spent_units = 0.0;
            state.intermediate_unmet_battery_units = 0.0;
            if aip_output > 0.0 {
                if let Some(remaining) = state.aip_remaining_units {
                    state.aip_remaining_units = Some((remaining - aip_output).max(0.0));
                    if state.aip_remaining_units == Some(0.0) {
                        state.aip_operational = false;
                        trace_data.push(("3.2.aip_exhausted",json!({"instruction":"return to port to replenish fuel and oxygen; no automatic movement or replenishment","aip_remaining_units":0.0})));
                    }
                }
            }
            // AIP's supplied units are applied at this 1800-second boundary.
            // Charging is also settled only here, regardless of discharge resolution.
            if issue.is_none() && eligible_seconds > 0 {
                match state.charge_units {
                    None => {
                        issue = Some("Battery current charge unknown; cannot apply charging".into())
                    }
                    Some(charge) => {
                        if state.capacity_units <= 0.0 {
                            issue=Some("14.5 battery capacity exhausted; charging rate percentage undefined".into());
                        }
                        let pct = if state.capacity_units > 0.0 {
                            100.0 * charge / state.capacity_units
                        } else {
                            0.0
                        };
                        let band = charge_band(pct);
                        if band.is_none() {
                            issue = Some("§3.2.4 R-064 70–71% charging band gap".into());
                        }
                        let col = match state.charge_rate_era {
                            Some(ChargeRateEra::Pre1970) => "pre_1970",
                            Some(ChargeRateEra::Post1970) => "1970_plus",
                            None => {
                                issue=Some("§3.2.4 charge table requires Pre-1970/1970+ build era; Annex old/new battery technology is not a build-date field".into());
                                "unknown"
                            }
                        };
                        let rates = &tables()["charging"]["table"]["rows"];
                        let rate = band.and_then(|b| {
                            rates
                                .get(col)
                                .and_then(|v| v.get(b))
                                .and_then(Value::as_f64)
                        });
                        if state.lithium_ion.is_none() {
                            issue=Some("§3.2.4 lithium-ion status unknown; source says x2 only for lithium-ion".into());
                        }
                        let speed_gate = match (state.full_speed_reference_kts, intermediate_avg) {
                            (Some(max), Some(avg)) => avg > max / 2.0,
                            (None, _) => {
                                issue = Some(
                                    "§3.2.4 charge full-speed denominator is not source-provided"
                                        .into(),
                                );
                                false
                            }
                            (_, None) => {
                                issue = Some("§3.2.4 charging-eligible speed history absent in older save; no rate assumed for this interval".into());
                                false
                            }
                        };
                        if speed_gate && state.charge_combination_policy.is_none() {
                            issue=Some("§3.2.4 R-064 half-speed/table-band composition needs explicit policy".into());
                        }
                        if let Some(mut r) = rate {
                            if speed_gate {
                                r *= 0.5;
                            }
                            if state.lithium_ion == Some(true) {
                                r *= 2.0;
                            }
                            if issue.is_none() {
                                let gain = r * (eligible_seconds as f64 / 1800.0);
                                state.charge_units =
                                    Some((charge + gain).min(state.capacity_units));
                            }
                        } else if issue.is_none() {
                            issue = Some("§3.2.4 no charge-rate cell".into());
                        }
                    }
                }
            }
            if let Some(message) = issue {
                add_pending(state, message.clone());
                trace_data.push((
                    "3.2.intermediate_pending",
                    json!({"message":message,"average_speed_kts":intermediate_avg}),
                ));
            } else {
                trace_data.push(("3.2.intermediate_settled",json!({"average_speed_kts":intermediate_avg,"aip_output_units":aip_output,"eligible_charge_seconds":eligible_seconds,"charge_units":state.charge_units,"aip_remaining_units":state.aip_remaining_units})));
            }
        }
        let _ = state;
        for (rule, data) in trace_data {
            trace(w, id, rule, data);
        }
    }
}

pub fn validate_action(w: &World, side: Side, id: EntityId, action: &Action) -> Result<(), String> {
    match action {
        Action::StartReplenishment{provider,units}=>return replenishment::validate(w,side,id,&replenishment::Request{provider:provider.clone(),units:*units}),
        Action::CancelReplenishment{}=>return replenishment::validate_cancel(w,side,id),
        _=>{},
    }
    let state = w
        .rules
        .battery
        .as_ref()
        .and_then(|s| s.units.get(&id))
        .ok_or("3.2 explicit battery configuration required")?;
    let entity = w
        .entity(id)
        .filter(|e| e.alive && e.kind == crate::types::PlatformKind::Sub && e.class == state.platform_class)
        .ok_or("3.2 submarine unavailable")?;
    if entity.side != side {
        return Err("3.2 battery command is own-side only".into());
    }
    match action {
        Action::StartReplenishment{..}|Action::CancelReplenishment{}=>unreachable!("port action handled before ordinary battery action"),
        Action::StartCharging {} => {
            if state.charge_units.is_none() {
                return Err("3.2 current charge is unknown".into());
            }
            if state.aip_operational {
                return Err(
                    "3.2.4 charging with active AIP has no resolved source combination".into(),
                );
            }
            if state.power_mode != PowerMode::Diesel {
                return Err("3.2 stop battery-powered propulsion before charging".into());
            }
            if entity.depth_m != 0.
                && !(at_snorkeling_depth(entity.depth_m)
                    && state.mast_raised
                    && state.snorkel_fitted == Some(true))
            {
                return Err("3.2 charging requires surfaced or source P/S depth (0–25 m) with fitted raised snorkel".into());
            }
        }
        Action::StopCharging {} => {}
        Action::RaiseSnorkel {} => {
            if state.snorkel_fitted != Some(true) {
                return Err("3.2 snorkel fit unknown or absent".into());
            }
            if entity.speed_kts > 15.0 {
                return Err("3.2.1 no mast may be raised above 15 kt".into());
            }
            if entity.depth_m > 25.0 {
                return Err(
                    "3.2.1 snorkel mast requires surfaced or Periscope/Snorkeling depth".into(),
                );
            }
        }
        Action::RetractMasts {} => {
            if entity.depth_m > 0.
                && at_snorkeling_depth(entity.depth_m)
                && state.mast_raised
                && state.power_mode == PowerMode::Diesel
                && !(state.aip_operational
                    && state.aip_fitted
                    && state.aip_remaining_units.is_some()
                    && entity.speed_kts <= 5.0)
            {
                if state.charge_units.is_none() {
                    return Err(
                        "3.2 retracting the snorkel into battery propulsion requires known charge"
                            .into(),
                    );
                }
            }
        }
        Action::SetAipOperating { enabled } => {
            if *enabled && !state.aip_critical_identities.is_empty() {
                return Err(
                    "14.5 critical-disabled AIP is not repaired by an operating command".into(),
                );
            }
            if *enabled && state.charging {
                return Err(
                    "3.2.5 active AIP with charging has no resolved source combination".into(),
                );
            }
            if *enabled && !state.aip_fitted {
                return Err("3.2.5 no exact Annex AIP fit".into());
            }
            if *enabled
                && (state.aip_remaining_units.is_none() || state.aip_remaining_units == Some(0.))
            {
                return Err("3.2.5 current remaining AIP potential unknown or exhausted".into());
            }
        }
        Action::SetPowerMode { mode } => {
            if *mode == PowerMode::Battery
                && (entity.depth_m == 0.
                    || (at_snorkeling_depth(entity.depth_m) && state.mast_raised)
                    || state.charge_units.is_none())
            {
                return Err("3.2 battery power requires submerged state without raised-snorkel diesel operation and known charge".into());
            }
            if *mode == PowerMode::Battery && state.charging {
                return Err("3.2 stop charging before battery-powered propulsion".into());
            }
            if *mode == PowerMode::Diesel
                && entity.depth_m > 0.
                && !(at_snorkeling_depth(entity.depth_m)
                    && state.mast_raised
                    && state.snorkel_fitted == Some(true))
                && !(state.aip_operational
                    && state.aip_fitted
                    && state.aip_remaining_units.is_some()
                    && entity.speed_kts <= 5.0)
            {
                return Err("3.2 submerged Diesel mode requires fitted raised snorkel at P/S depth or ≤5 kt AIP operation".into());
            }
        }
    }
    Ok(())
}
/// Explicit scenario stowage is neither charging nor active propulsion.
/// Settle this unit alone before removing it; never resample other boats as a
/// side effect of client command ordering. Partial work and AIP reservations stay.
pub fn pause_stowed(w:&mut World,id:EntityId)->Result<(),String>{
    let Some(u)=w.rules.battery.as_ref().and_then(|s|s.units.get(&id)) else{return Ok(())};
    if u.stowed_at.is_some(){return Ok(());}
    if !w.entity(id).is_some_and(|e|e.alive&&e.class==u.platform_class){return Err("battery stowage needs its actual live bound body before removal".into());}
    step_selected(w,vec![id]);
    let u=w.rules.battery.as_mut().unwrap().units.get_mut(&id).unwrap();
    u.stowed_at=Some(w.tick);u.charging=false;u.aip_operational=false;u.mast_raised=false;u.mast_raised_since=None;
    Ok(())
}
/// The same physical vehicle returns with its old energy and partial work.
/// Shift only clocks by the inactive interval, not charge or spent reserves.
pub fn resume_stowed(w:&mut World,id:EntityId,stored_at:i64)->Result<(),String>{
    let Some(u)=w.rules.battery.as_ref().and_then(|s|s.units.get(&id)) else{return Ok(())};
    if u.stowed_at!=Some(stored_at)||stored_at>w.tick{return Err("battery launch clock must match the original physical stowage".into());}
    if !w.entity(id).is_some_and(|e|e.alive&&e.class==u.platform_class){return Err("battery launch needs the same actual bound body after insertion".into());}
    let elapsed=w.tick-stored_at;
    let turn=u.turn_started_at.checked_add(elapsed).ok_or("battery stowage clock overflow")?;
    let intermediate=u.intermediate_started_at.checked_add(elapsed).ok_or("battery stowage clock overflow")?;
    let u=w.rules.battery.as_mut().unwrap().units.get_mut(&id).unwrap();
    u.turn_started_at=turn;u.intermediate_started_at=intermediate;u.stowed_at=None;
    Ok(())
}

pub fn apply(w: &mut World, side: Side, id: EntityId, action: &Action) -> Result<(), String> {
    validate_action(w, side, id, action)?;
    match action {
        Action::StartReplenishment{provider,units}=>return replenishment::prepare(w,side,id,replenishment::Request{provider:provider.clone(),units:*units}),
        Action::CancelReplenishment{}=>return replenishment::cancel(w,side,id),
        _=>{},
    }
    let submerged = w.entity(id).is_some_and(|e| e.depth_m > 0.0);
    let u = w
        .rules
        .battery
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap();
    match action {
        Action::StartReplenishment{..}|Action::CancelReplenishment{}=>unreachable!("port action handled before ordinary battery action"),
        Action::StartCharging {} => u.charging = true,
        Action::StopCharging {} => u.charging = false,
        Action::RaiseSnorkel {} => {
            if !u.mast_raised {
                u.mast_raised_since = Some(w.tick);
            }
            u.mast_raised = true;
            // Source §3.2.4: using the snorkel runs the diesel engines. This
            // atomic RTS transition avoids an impossible battery↔diesel
            // intermediate state between two separately validated commands.
            u.power_mode = PowerMode::Diesel;
        }
        Action::RetractMasts {} => {
            u.mast_raised = false;
            u.mast_raised_since = None;
            if submerged {
                u.power_mode = PowerMode::Battery;
                u.charging = false;
            }
        }
        Action::SetAipOperating { enabled } => u.aip_operational = *enabled,
        Action::SetPowerMode { mode } => u.power_mode = *mode,
    }
    trace(w, id, "3.2.order", json!({"action":action,"tick":w.tick}));
    Ok(())
}

/// Source critical resolution is owned by the damage subsystem; this API only
/// applies its already-resolved percentage and stable identity once.
pub fn critical_capacity_loss(
    w: &mut World,
    id: EntityId,
    identity: &str,
    loss_fraction: f64,
) -> Result<(), String> {
    if identity.trim().is_empty() {
        return Err("14.5 battery critical requires stable original-hit identity".into());
    }
    if !loss_fraction.is_finite() || loss_fraction < 0. || loss_fraction > 0.30 {
        return Err("14.5 battery loss fraction must be 0..=0.30 per critical".into());
    }
    let u = w
        .rules
        .battery
        .as_mut()
        .and_then(|s| s.units.get_mut(&id))
        .ok_or("14.5 no configured battery state")?;
    if !u.capacity_critical_identities.insert(identity.to_string()) {
        return Ok(());
    }
    let old = u.capacity_units;
    u.cumulative_capacity_loss_fraction =
        (u.cumulative_capacity_loss_fraction + loss_fraction).min(1.0);
    let new = u.original_capacity_units * (1.0 - u.cumulative_capacity_loss_fraction).max(0.);
    let ratio = if old > 0. { new / old } else { 0. };
    if let Some(charge) = u.charge_units {
        u.charge_units = Some((charge * ratio).min(new));
    }
    u.capacity_units = new;
    let charge = u.charge_units;
    let _ = u;
    trace(
        w,
        id,
        "14.5.battery_capacity_loss",
        json!({"identity":identity,"loss_fraction_original_max":loss_fraction,"new_capacity_units":new,"charge_units":charge}),
    );
    Ok(())
}

pub fn critical_aip_disable(w: &mut World, id: EntityId, identity: &str) -> Result<(), String> {
    if identity.trim().is_empty() {
        return Err("14.5 AIP critical requires stable original-hit identity".into());
    }
    let u = w
        .rules
        .battery
        .as_mut()
        .and_then(|s| s.units.get_mut(&id))
        .ok_or("14.5 no configured battery state")?;
    if !u.aip_fitted {
        return Err("14.5 AIP critical applied to platform without exact AIP fit".into());
    }
    let disabled = u.aip_critical_identities.insert(identity.to_string());
    if disabled {
        u.aip_operational = false;
    }
    let _ = u;
    if disabled {
        trace(w, id, "14.5.aip_disabled", json!({"identity":identity}));
    }
    Ok(())
}
pub fn aip_fitted(w: &World, id: EntityId) -> Option<bool> {
    w.rules
        .battery
        .as_ref()?
        .units
        .get(&id)
        .map(|u| u.aip_fitted)
}

/// A read-only physical power gate, before either actual movement adapter.
/// None means this opt-in module supplies no ceiling, not verified endurance.
/// Zero-cap and slow AIP ceilings are explicit RTS guards; no navigation order,
/// diesel fuel, mast or replenishment is created by this function.
pub fn energy_movement_gate(w: &World, id: EntityId) -> Result<Option<f64>, String> {
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return Ok(None);
    }
    let Some(u) = w.rules.battery.as_ref().and_then(|s| s.units.get(&id)) else {
        return Ok(None);
    };
    let e = w.entity(id)
        .filter(|e| e.alive && e.kind == crate::types::PlatformKind::Sub && e.class == u.platform_class)
        .ok_or("3.2 original exact battery platform is unavailable")?;
    let air = e.depth_m == 0.0
        || (at_snorkeling_depth(e.depth_m) && u.mast_raised && u.snorkel_fitted == Some(true));
    if air && u.power_mode == PowerMode::Diesel {
        return Ok(None);
    }
    if e.depth_m == 0.0 && u.power_mode == PowerMode::Battery {
        return Err("§3.2.4 surfaced battery propulsion has no sourced power-mode transition; declare diesel operation".into());
    }
    let aip = u.aip_fitted && u.aip_operational && u.aip_critical_identities.is_empty()
        && u.aip_remaining_units.is_some_and(|v| v - u.intermediate_aip_spent_units > 1e-9);
    if u.power_mode == PowerMode::Diesel && !air {
        return if aip {
            Ok(Some(5.0))
        } else {
            Err("§3.2.4 submerged Diesel lacks source air or active remaining AIP; no free propulsion assumed".into())
        };
    }
    match u.charge_units {
        Some(charge) if charge > 0.0 => Ok(None),
        Some(_) if aip => Ok(Some(5.0)),
        Some(_) => Ok(Some(0.0)),
        None if aip => Ok(Some(5.0)),
        None => Err("§3.2.4 current battery charge unknown and no independently known active AIP; endurance remains pending, not zero or unlimited".into()),
    }
}

/// The one actual battery snorkel, without inventing a timestamp in old saves.
/// Shared detection combines it with its untyped mast count as a minimum count,
/// not an additional independent mast when the old count may already include it.
pub fn raised_snorkel(w: &World, id: EntityId) -> Option<(bool, Option<i64>)> {
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return None;
    }
    let u = w.rules.battery.as_ref()?.units.get(&id)?;
    let e = w.entity(id)?;
    if !e.alive || e.kind != crate::types::PlatformKind::Sub || e.class != u.platform_class
        || u.snorkel_fitted != Some(true) {
        return None;
    }
    Some((u.mast_raised, u.mast_raised_since.filter(|_| u.mast_raised)))
}

/// Admission and apply must share this gate; retraction remains a real command.
pub fn validate_depth_change(w: &World, id: EntityId, target_depth_m: f64) -> Result<(), String> {
    if raised_snorkel(w, id).is_some_and(|(raised, _)| raised) && target_depth_m > 25.0 {
        return Err("§3.2.1 retract the actual snorkel before ordering depth below P/S25m".into());
    }
    Ok(())
}

pub fn own_view(w: &World, id: EntityId) -> Option<Value> {
    let u = w.rules.battery.as_ref()?.units.get(&id)?;
    let movement_gate = energy_movement_gate(w, id);
    Some(
        json!({"source":u.source,"capacity_units":u.capacity_units,"charge_units":u.charge_units,"aip_fitted":u.aip_fitted,"aip_remaining_units":u.aip_remaining_units,"aip_operational":u.aip_operational,"aip_replenishment_due":u.aip_fitted&&u.aip_remaining_units==Some(0.0),"aip_replenishment_job":replenishment::own_job(w,id),"snorkel_fitted":u.snorkel_fitted,"mast_raised":u.mast_raised,"charging":u.charging,"power_mode":u.power_mode,"recharge_required":u.charge_units==Some(0.0),"movement_speed_limit_kts":movement_gate.as_ref().ok().copied().flatten(),"movement_power_pending":movement_gate.err(),"pending_adjudications":u.pending_adjudications}),
    )
}

pub fn snorkel_speed_limit(w: &World, id: EntityId) -> Option<f64> {
    raised_snorkel(w, id)?.0.then_some(15.0)
}

#[cfg(test)]
mod source_cases {
    use super::*;

    #[test]
    fn printed_discharge_cells_include_old_new_and_resolution_columns() {
        assert_eq!(
            table_discharge(9, true, Resolution::Intermediate),
            Some(8.0)
        );
        assert_eq!(table_discharge(9, true, Resolution::Tactical), Some(0.8));
        assert_eq!(
            table_discharge(10, false, Resolution::Intermediate),
            Some(15.0)
        );
        assert_eq!(
            table_discharge(10, true, Resolution::Intermediate),
            Some(10.0)
        );
        assert_eq!(table_discharge(3, true, Resolution::Intermediate), None);
        assert_eq!(table_discharge(26, true, Resolution::Tactical), None);
    }

    #[test]
    fn charge_threshold_gap_is_not_interpolated() {
        assert_eq!(charge_band(70.0), Some(0));
        assert_eq!(charge_band(70.5), None);
        assert_eq!(charge_band(71.0), Some(1));
    }

    #[test]
    fn old_ten_knot_prose_policy_doubles_the_new_column_not_the_old_column() {
        assert_eq!(
            resolved_discharge(
                10.0,
                false,
                Resolution::Intermediate,
                Some(Old10KtPolicy::ProseDouble)
            )
            .unwrap(),
            20.0
        );
        assert_eq!(
            resolved_discharge(
                10.0,
                false,
                Resolution::Tactical,
                Some(Old10KtPolicy::ProseDouble)
            )
            .unwrap(),
            2.0
        );
        assert_eq!(
            resolved_discharge(
                10.0,
                false,
                Resolution::Intermediate,
                Some(Old10KtPolicy::TableLiteral)
            )
            .unwrap(),
            15.0
        );
        assert!(resolved_discharge(10.0, false, Resolution::Intermediate, None).is_err());
    }

    #[test]
    fn aip_source_examples_select_known_outputs_without_a_fictional_general_curve() {
        assert_eq!(aip_output_rate(4.0, None).unwrap(), 2.0);
        assert_eq!(aip_output_rate(5.0, None).unwrap(), 3.0);
        assert_eq!(aip_output_rate(9.0, None).unwrap(), 3.0);
        assert!(aip_output_rate(8.0, None).is_err());
        assert_eq!(aip_output_rate(8.0, Some(2.5)).unwrap(), 2.5);
        assert!(aip_output_rate(20.0, Some(3.0)).is_err());
    }
}

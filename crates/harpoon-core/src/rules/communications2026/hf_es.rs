//! Owned leaf: English2026 PDF74/printed 6-6 §6.3.9 — committed TDL HF radio
//! transmissions can be detected by ES systems. The rule prints no chance,
//! range or procedure, so every receiver here is an explicit
//! scenario_assumption fit with its own declared range and bearing error;
//! nothing defaults and no ES hardware is claimed to support HF.
//!
//! Strict currency: an emission is an ES source only at the actual current
//! sending tick (`queued_at == w.tick`). A committed message is not
//! continuous radiation; a late receiver never detects a historical pulse.
//!
//! Pure over `World` + `Configuration`: the parent module declaration, the
//! Context field, the fingerprint and the detection2026 ES integration are
//! root-owned. Detection outcomes here are bearing-only; this leaf never
//! produces a measured position, fix, category, class, FCS or quality, and
//! its public view never carries the enemy emitter identity or the private
//! message reference (referee trace only).
use super::{hf_transmissions, MessageRef};
use crate::{EntityId, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/hf-es-2026.json"
        ))
        .expect("verified HF-ES source/policy")
    })
}
fn authored(s: &str) -> bool {
    s == "scenario_assumption" || s.starts_with("scenario_assumption:")
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    /// Key: exact own unit.name. One declared HF-ES receiver fit per unit;
    /// an undeclared unit has no HF-ES capability at all.
    pub receivers: BTreeMap<String, ReceiverFit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiverFit {
    pub source: String,
    /// Must be a sensor actually fitted on the unit; damage is rechecked per
    /// sample against the real hardware state.
    pub sensor: String,
    pub range_nm: f64,
    pub bearing_error_deg: f64,
}
impl Configuration {
    pub fn validate(&self) -> Result<(), String> {
        if !self.receivers.is_empty() && !authored(&self.source) {
            return Err("hf-es configuration requires an explicit scenario_assumption source".into());
        }
        for (name, fit) in &self.receivers {
            if name.trim().is_empty()
                || !authored(&fit.source)
                || fit.sensor.trim().is_empty()
                || !fit.range_nm.is_finite()
                || fit.range_nm <= 0.
                || !fit.bearing_error_deg.is_finite()
                || fit.bearing_error_deg < 0.
            {
                return Err("hf-es receiver fit requires authored source and finite declared values".into());
            }
        }
        Ok(())
    }
}

/// One committed TDL HF transmission at the actual current sending tick.
#[derive(Clone, Debug, Serialize)]
pub struct HfEsSource {
    pub emitter: EntityId,
    pub position: crate::Vec2,
    pub at: i64,
    pub reference: MessageRef,
    pub protocol: String,
}
pub fn es_sources(w: &World) -> Vec<HfEsSource> {
    hf_transmissions(w)
        .into_iter()
        .filter(|t| t.at == w.tick)
        .map(|t| HfEsSource {
            emitter: t.emitter,
            position: t.position,
            at: t.at,
            reference: t.reference,
            protocol: t.protocol,
        })
        .collect()
}

/// Deterministic bearing-only sample for one declared receiver fit. Inside
/// the declared range the detection always happens (no printed chance);
/// outside it is silence. Side filtering is the caller's pair-enum decision.
/// `emitter`/`reference` exist for the root-owned detector attribution and
/// referee trace; `own_view` never publishes them.
#[derive(Clone, Debug, Serialize)]
pub struct HfEsBearing {
    pub emitter: EntityId,
    pub sensor: String,
    pub bearing_deg: f64,
    pub bearing_error_deg: f64,
    pub emitted_at: i64,
    pub observed_at: i64,
    pub reference: MessageRef,
    pub source: String,
}
pub fn sample(
    w: &World,
    config: &Configuration,
    unit: EntityId,
) -> Result<Vec<HfEsBearing>, String> {
    config.validate()?;
    let Some(e) = w.entity(unit).filter(|e| e.alive) else {
        return Ok(Vec::new());
    };
    let Some(fit) = config.receivers.get(&e.name) else {
        return Ok(Vec::new());
    };
    if !e.sensors.iter().any(|i| i.sensor == fit.sensor)
        || crate::sensors::sensor_damaged(e, &fit.sensor)
    {
        return Ok(Vec::new());
    }
    let source = format!(
        "English2026 PDF74/6-6 6.3.9 HF TDL ES detectability; {}",
        fit.source
    );
    Ok(es_sources(w)
        .into_iter()
        .filter(|s| s.emitter != unit)
        .filter(|s| e.pos.range_to(s.position) <= fit.range_nm)
        .map(|s| HfEsBearing {
            emitter: s.emitter,
            sensor: fit.sensor.clone(),
            bearing_deg: crate::geom::normalize_deg(e.pos.bearing_to(s.position)),
            bearing_error_deg: fit.bearing_error_deg,
            emitted_at: s.at,
            observed_at: w.tick,
            reference: s.reference,
            source: source.clone(),
        })
        .collect())
}

/// Public JSON summary for observation wiring. Bearing, declared error,
/// observation time and the receiver's own sensor only: no enemy emitter
/// identity, no message reference, no position/fix/quality/category.
pub fn own_view(w: &World, config: &Configuration, unit: EntityId) -> Option<Value> {
    let bearings = sample(w, config, unit).ok()?;
    if bearings.is_empty() {
        return None;
    }
    let public: Vec<Value> = bearings
        .iter()
        .map(|b| {
            json!({
                "sensor": b.sensor,
                "bearing_deg": b.bearing_deg,
                "bearing_error_deg": b.bearing_error_deg,
                "observed_at": b.observed_at,
            })
        })
        .collect();
    Some(json!({
        "scope":"hf_es_declared_receiver",
        "bearings":public,
        "bearing_only":true,
        "source":tables()["rts_policy"]["report"]
    }))
}

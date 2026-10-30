//! §12.4 floating radar reflector raft towing. The source supplies a ten-knot
//! cable-break ceiling, not cable length/dynamics or a wreck attachment rule.
//! Geometry and owner-loss behavior are explicit scenario-author inputs.
use super::{active, emit, Object, ObjectKind, Status};
use crate::types::PlatformKind;
use crate::{EntityId, Side, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;

const OWN_RAFT: &str = "12.4: own effective floating radar raft and live ship launcher required";

pub fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/floating-tow-2026.json"
        ))
        .expect("verified FloatingAsm towing source and explicit author-model boundary")
    })
}

fn maximum_speed() -> f64 {
    tables()["source_rule"]["max_towing_speed_kts"]
        .as_f64()
        .expect("verified section12.4 towing ceiling")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FloatingTowGeometry {
    pub source: String,
    pub fit: String,
    pub model: String,
    pub length_nm: f64,
    pub capture_tolerance_nm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_loss: Option<String>,
}

impl FloatingTowGeometry {
    fn validate(&self) -> Result<(), String> {
        if self.source.trim().is_empty()
            || self.fit.trim().is_empty()
            || self.model != "rigid_stern_offset"
            || !self.length_nm.is_finite()
            || self.length_nm <= 0.0
            || !self.capture_tolerance_nm.is_finite()
            || self.capture_tolerance_nm < 0.0
        {
            return Err("12.4: specific pending; explicit finite source/author stern geometry and numerical capture tolerance required".into());
        }
        if self
            .owner_loss
            .as_deref()
            .is_some_and(|policy| policy != "detach_and_drift")
        {
            return Err("12.4: specific pending; unsupported owner-loss cable policy".into());
        }
        Ok(())
    }
}

/// Persistent own-object receipt. Object.status remains a separate real raft
/// lifecycle; broken/released cables leave that raft deployed and expendable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub status: String,
    pub geometry: String,
    pub source: String,
    pub owner: EntityId,
    pub length_nm: f64,
    pub model: String,
    pub capture_tolerance_nm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_loss: Option<String>,
    pub attached_at: i64,
    pub ended_at: Option<i64>,
    pub reason: Option<String>,
}

pub enum TowStep {
    Free,
    Followed,
    Pending,
    /// Parent emits this once and then executes its existing free-drift block
    /// once for the interval. The child never applies wind or reads taken State.
    Ended {
        rule: &'static str,
        detail: Value,
    },
}

struct AttachPlan {
    side: Side,
    record: Record,
}

/// None is an identical active attachment. It does not re-check a pickup
/// position that the real ship has since moved beyond, reset clocks or retow.
fn attach_preflight(
    w: &World,
    unit: EntityId,
    object: EntityId,
    geometry: &str,
) -> Result<Option<AttachPlan>, String> {
    if !active(w) {
        return Err("12.4: floating tow orders require explicit hv2026-rts-v4".into());
    }
    let owner = w
        .entity(unit)
        .filter(|e| e.alive && e.kind == PlatformKind::Ship)
        .ok_or(OWN_RAFT)?;
    let state = w.rules.ew.as_ref().ok_or(OWN_RAFT)?;
    let obj = state
        .objects
        .get(&object)
        .filter(|o| {
            o.owner == unit
                && o.side == owner.side
                && o.kind == ObjectKind::FloatingAsm
                && o.effective(w.tick)
        })
        .ok_or(OWN_RAFT)?;
    if let Some(previous) = &obj.floating_tow {
        if previous.status == "attached" && previous.geometry == geometry && previous.owner == unit
        {
            return Ok(None);
        }
        return Err("12.4: specific pending; reconnect/retow or changing an existing cable requires a separately resolved procedure".into());
    }
    let model = state
        .config
        .floating_tow_geometries
        .get(geometry)
        .ok_or("12.4: specific pending; explicit source/author cable geometry absent")?;
    model.validate()?;
    if model.fit != obj.fit {
        return Err("12.4: declared cable geometry does not match this own fitted raft".into());
    }
    if !owner.speed_kts.is_finite() || owner.speed_kts < 0.0 || owner.speed_kts > maximum_speed() {
        return Err(
            "12.4: cannot attach a floating radar raft at a real tow speed above10 knots".into(),
        );
    }
    if !owner.course_deg.is_finite()
        || !owner.pos.x.is_finite()
        || !owner.pos.y.is_finite()
        || !obj.pos.x.is_finite()
        || !obj.pos.y.is_finite()
    {
        return Err("12.4: specific pending; finite real ship/raft geometry required".into());
    }
    let stern = owner
        .pos
        .advanced(owner.course_deg + 180.0, model.length_nm);
    if obj.pos.range_to(stern) > model.capture_tolerance_nm {
        return Err("12.4: own raft is not at the explicitly declared author attachment point; no teleport or invented pickup range".into());
    }
    Ok(Some(AttachPlan {
        side: owner.side,
        record: Record {
            status: "attached".into(),
            geometry: geometry.into(),
            source: model.source.clone(),
            owner: unit,
            length_nm: model.length_nm,
            model: model.model.clone(),
            capture_tolerance_nm: model.capture_tolerance_nm,
            owner_loss: model.owner_loss.clone(),
            attached_at: w.tick,
            ended_at: None,
            reason: None,
        },
    }))
}

pub(super) fn validate_attach(
    w: &World,
    unit: EntityId,
    object: EntityId,
    geometry: &str,
) -> Result<(), String> {
    attach_preflight(w, unit, object, geometry).map(|_| ())
}

pub(super) fn attach(
    w: &mut World,
    unit: EntityId,
    object: EntityId,
    geometry: &str,
) -> Result<(), String> {
    let Some(plan) = attach_preflight(w, unit, object, geometry)? else {
        return Ok(());
    };
    let record = plan.record;
    w.rules
        .ew
        .as_mut()
        .and_then(|s| s.objects.get_mut(&object))
        .ok_or(OWN_RAFT)?
        .floating_tow = Some(record.clone());
    emit(
        w,
        plan.side,
        unit,
        "12.4.tow_attached",
        json!({"object":object,"tow":record,"source_max_tow_speed_kts":maximum_speed(),"geometry_notice":"Explicit author rigid offset; no source cable length/dynamics inferred; no object relocation at attachment"}),
    );
    Ok(())
}

fn detach_preflight(w: &World, unit: EntityId, object: EntityId) -> Result<Option<Side>, String> {
    if !active(w) {
        return Err("12.4: floating tow orders require explicit hv2026-rts-v4".into());
    }
    let owner = w
        .entity(unit)
        .filter(|e| e.alive && e.kind == PlatformKind::Ship)
        .ok_or(OWN_RAFT)?;
    let obj = w
        .rules
        .ew
        .as_ref()
        .and_then(|s| s.objects.get(&object))
        .filter(|o| {
            o.owner == unit
                && o.side == owner.side
                && o.kind == ObjectKind::FloatingAsm
                && o.effective(w.tick)
        })
        .ok_or(OWN_RAFT)?;
    let record = obj
        .floating_tow
        .as_ref()
        .filter(|r| r.owner == unit)
        .ok_or("12.4: no own attached floating radar raft cable")?;
    if record.status == "released" && record.reason.as_deref() == Some("manual") {
        return Ok(None);
    }
    if record.status != "attached" {
        return Err("12.4: no own attached floating radar raft cable".into());
    }
    if record.source.trim().is_empty() || record.model != "rigid_stern_offset" {
        return Err("12.4: specific pending; explicit author release model absent".into());
    }
    Ok(Some(owner.side))
}

pub(super) fn validate_detach(w: &World, unit: EntityId, object: EntityId) -> Result<(), String> {
    detach_preflight(w, unit, object).map(|_| ())
}

pub(super) fn detach(w: &mut World, unit: EntityId, object: EntityId) -> Result<(), String> {
    let Some(side) = detach_preflight(w, unit, object)? else {
        return Ok(());
    };
    let obj = w
        .rules
        .ew
        .as_mut()
        .and_then(|s| s.objects.get_mut(&object))
        .ok_or(OWN_RAFT)?;
    let record = obj.floating_tow.as_mut().ok_or(OWN_RAFT)?;
    record.status = "released".into();
    record.reason = Some("manual".into());
    record.ended_at = Some(w.tick);
    obj.speed_kts = 0.0;
    let detail = json!({"object":object,"tow":record,"policy":"Explicit author manual release; source free half-wind drift resumes in normal EW step; no stock/life refill"});
    emit(w, side, unit, "12.4.tow_released", detail);
    Ok(())
}

/// Called inside the existing FloatingAsm branch while parent owns its taken
/// EW State. It must not read w.rules.ew, scan objects or apply wind itself.
pub(super) fn step_one(w: &World, obj: &mut Object, now: i64) -> TowStep {
    let Some(record) = obj.floating_tow.as_mut() else {
        return TowStep::Free;
    };
    if record.status != "attached" || obj.kind != ObjectKind::FloatingAsm {
        return TowStep::Free;
    }
    if obj.status != Status::Deployed || obj.expires_at.is_some_and(|at| now >= at) {
        return TowStep::Free;
    }
    if now < obj.ready_at {
        return TowStep::Pending;
    }
    if record.owner != obj.owner
        || record.source.trim().is_empty()
        || record.model != "rigid_stern_offset"
        || !record.length_nm.is_finite()
        || record.length_nm <= 0.0
    {
        record.reason = Some("source_pending: saved cable provenance/geometry unresolved".into());
        return TowStep::Pending;
    }
    match w.entity(obj.owner).filter(|e| e.alive) {
        Some(owner) => {
            if owner.kind != PlatformKind::Ship
                || owner.side != obj.side
                || !owner.speed_kts.is_finite()
                || owner.speed_kts < 0.0
                || !owner.course_deg.is_finite()
                || !owner.pos.x.is_finite()
                || !owner.pos.y.is_finite()
            {
                record.reason =
                    Some("source_pending: real tow owner pose/provenance unresolved".into());
                return TowStep::Pending;
            }
            if owner.speed_kts > maximum_speed() {
                record.status = "broken".into();
                record.ended_at = Some(now);
                record.reason = Some("overspeed".into());
                obj.speed_kts = 0.0;
                return TowStep::Ended {
                    rule: "12.4.tow_broken",
                    detail: json!({"object":obj.id,"tow":record,"actual_ship_speed_kts":owner.speed_kts,"source_max_tow_speed_kts":maximum_speed(),"consequence":"cable breaks; raft remains deployed with its original lifetime and stock; no ship order clamp","position_policy":"Explicit author one-second mapping: keep previous authoritative raft point then normal parent free drift once"}),
                };
            }
            obj.pos = owner
                .pos
                .advanced(owner.course_deg + 180.0, record.length_nm);
            obj.course_deg = owner.course_deg;
            obj.speed_kts = owner.speed_kts;
            obj.depth_m = 0.0;
            record.reason = None;
            TowStep::Followed
        }
        None => {
            if record.owner_loss.as_deref() != Some("detach_and_drift") {
                record.reason = Some("source_pending: Book does not specify owner-loss cable fate; explicit author owner_loss policy required".into());
                return TowStep::Pending;
            }
            record.status = "released".into();
            record.ended_at = Some(now);
            record.reason = Some("owner_lost".into());
            obj.speed_kts = 0.0;
            TowStep::Ended {
                rule: "12.4.tow_owner_lost",
                detail: json!({"object":obj.id,"tow":record,"policy":"Explicit author detach_and_drift; not the acoustic towed-decoy destroyed-on-owner-loss rule; parent applies free drift once"}),
            }
        }
    }
}

/// Existing source Sink/Swamped/Expired gates use this to stop an attached
/// record. Real raft status, position, inventory and lifetime belong to parent.
pub(super) fn terminate(obj: &mut Object, now: i64, reason: &str) {
    let Some(record) = obj.floating_tow.as_mut() else {
        return;
    };
    if record.status == "attached" {
        record.status = "released".into();
        record.ended_at = Some(now);
        record.reason = Some(reason.into());
        obj.speed_kts = 0.0;
    }
}

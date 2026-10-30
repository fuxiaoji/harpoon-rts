//! §12.6.2 independent speed allowance for one genuinely deployed simulator.
//! The ordinary world step consumes its new speed; no trajectory or lifetime
//! is fabricated here, and a generic combat entity never acquires this quota.
use super::{active, emit, finite, mobile_row, ObjectKind};
use crate::types::PlatformKind;
use crate::{EntityId, Side, World};
use serde_json::json;

const OWN_EFFECTIVE_REQUIRED: &str = "12.6.2: own effective mobile simulator required";

struct SpeedPlan {
    side: Side,
    old_speed: f64,
    changes: u8,
    course_changes: u8,
    expires_at: Option<i64>,
    generation: u8,
    maximum: f64,
    allowance: u8,
}

/// One read-only preflight serves both public admission and commit. None is
/// a valid same-value no-op; a queued command must pass this again at commit.
fn preflight(
    w: &World,
    unit: EntityId,
    object: EntityId,
    speed_kts: f64,
) -> Result<Option<SpeedPlan>, String> {
    if !active(w) {
        return Err("12.6.2: mobile speed orders require explicit hv2026-rts-v4".into());
    }
    finite(speed_kts)?;
    if speed_kts < 0. {
        return Err("12.6.2: nonnegative simulator speed required".into());
    }
    let owner = w
        .entity(unit)
        .filter(|e| e.alive && e.kind == PlatformKind::Sub)
        .ok_or("12.6.2: actual live submarine launcher required")?;
    let side = owner.side;
    let obj = w
        .rules
        .ew
        .as_ref()
        .and_then(|s| s.objects.get(&object))
        .filter(|obj| {
            obj.owner == unit
                && obj.side == side
                && obj.kind == ObjectKind::MobileDecoy
                && obj.effective(w.tick)
        })
        .ok_or(OWN_EFFECTIVE_REQUIRED)?;
    let generation = obj
        .generation
        .ok_or("12.6.2: source simulator generation required")?;
    let row = mobile_row(generation)?;
    let maximum = row["max_speed_kts"]
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0.)
        .ok_or("12.6.2: source maximum speed unavailable")?;
    if speed_kts > maximum {
        return Err("12.6.2: ordered simulator speed exceeds source generation maximum".into());
    }
    if !obj.speed_kts.is_finite() || obj.speed_kts < 0. {
        return Err("12.6.2: recorded simulator speed unresolved".into());
    }
    // Repeating the present speed is not a physical change, including on
    // the early single-speed generations or after two real changes.
    if obj.speed_kts == speed_kts {
        return Ok(None);
    }
    let allowance = row["speed_changes_max"]
        .as_u64()
        .and_then(|v| u8::try_from(v).ok())
        .ok_or("12.6.2: source speed-change allowance unavailable")?;
    if obj.speed_changes >= allowance {
        return Err("12.6.2: source lifetime speed-change allowance exhausted".into());
    }
    Ok(Some(SpeedPlan {
        side,
        old_speed: obj.speed_kts,
        changes: obj.speed_changes + 1,
        course_changes: obj.course_changes,
        expires_at: obj.expires_at,
        generation,
        maximum,
        allowance,
    }))
}

pub(super) fn validate(
    w: &World,
    unit: EntityId,
    object: EntityId,
    speed_kts: f64,
) -> Result<(), String> {
    preflight(w, unit, object, speed_kts).map(|_| ())
}

pub(super) fn apply(
    w: &mut World,
    unit: EntityId,
    object: EntityId,
    speed_kts: f64,
) -> Result<(), String> {
    let Some(plan) = preflight(w, unit, object, speed_kts)? else {
        return Ok(());
    };
    let obj = w
        .rules
        .ew
        .as_mut()
        .and_then(|s| s.objects.get_mut(&object))
        .ok_or(OWN_EFFECTIVE_REQUIRED)?;
    obj.speed_kts = speed_kts;
    obj.speed_changes = plan.changes;
    emit(
        w,
        plan.side,
        unit,
        "12.6.2.speed",
        json!({
            "object":object,
            "generation":plan.generation,
            "previous_speed_kts":plan.old_speed,
            "speed_kts":speed_kts,
            "source_max_speed_kts":plan.maximum,
            "source_speed_changes_max":plan.allowance,
            "speed_changes":plan.changes,
            "course_changes":plan.course_changes,
            "expires_at":plan.expires_at,
            "source":"English2026 PDF121/12-3 section12.6.2: independent individual simulator speed allowance; no per-turn reset",
            "energy_policy":"Original maximum-speed expiry retained; no lower-speed energy curve or lifetime extension inferred"
        }),
    );
    Ok(())
}

//! Source3.1.4 standard rudder advance and45-degree pivot loss.
use super::{tables, trace, Fit};
use crate::{
    geom::{angle_diff, normalize_deg},
    world::Entity,
    EntityId, World,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub window: i64,
    pub charged_degrees: f64,
    pub advanced_nm: f64,
    pub direction: i8,
}
fn row(f: &Fit, submerged: bool) -> Result<(f64, f64, f64), String> {
    let data = &tables()["turning"];
    let pivot = if f.waterjet {
        &data["waterjet_pivot_deg"]
    } else {
        &data["pivot_deg"]
    }
    .as_f64()
    .unwrap();
    if submerged {
        if f.waterjet {
            return Err(
                "3.1.4 submerged waterjet smaller-row relation is not source-verified".into(),
            );
        }
        let r = &data["warship"]["submerged_sub"];
        return Ok((r[0].as_f64().unwrap(), r[1].as_f64().unwrap(), pivot));
    }
    let size = if f.waterjet {
        let mut chars = "ABCDEFG".chars();
        while chars.next().is_some_and(|c| c != f.size) {}
        chars
            .next()
            .ok_or("3.1.4 no printed smaller size thanG for waterjet; adjudication required")?
    } else {
        f.size
    };
    let (category, key) = if f.merchant {
        (
            "merchant",
            match size {
                'A' => "a",
                'B' | 'C' => "b_c",
                'D' | 'E' => "d_e",
                'F' | 'G' => "fg",
                _ => return Err("3.1.4 invalid source size".into()),
            },
        )
    } else {
        (
            "warship",
            match size {
                'A' => "a",
                'B' => "b",
                'C' => "c",
                'D' | 'E' => "d_e",
                'F' | 'G' => {
                    if f.undamaged_max_kts
                        >= tables()["acceleration"]["fast_threshold_kts"]
                            .as_f64()
                            .unwrap()
                    {
                        "fast_fg"
                    } else {
                        "slow_fg"
                    }
                }
                _ => return Err("3.1.4 invalid source size".into()),
            },
        )
    };
    let r = &data[category][key];
    Ok((r[0].as_f64().unwrap(), r[1].as_f64().unwrap(), pivot))
}
pub(super) fn segment(w: &World, e: &mut Entity, distance: f64) -> bool {
    if distance <= 1e-12 {
        return true;
    }
    let next = e.pos.advanced(e.course_deg, distance);
    if w.context
        .terrain
        .as_ref()
        .is_some_and(|t| t.blocked(e.pos, next))
    {
        if !e.waypoints.is_empty() {
            e.ext
                .entry("terrain.resume_speed".into())
                .or_insert(e.ordered_speed_kts);
        }
        e.speed_kts = 0.;
        e.ordered_speed_kts = 0.;
        e.station = None;
        return false;
    }
    e.pos = next;
    true
}
pub fn advance(w: &mut World, id: EntityId) -> Result<(), String> {
    let mut runtime = w
        .rules
        .navigation
        .take()
        .ok_or("3.1.4 navigation runtime missing")?;
    let result = match runtime.units.get_mut(&id) {
        Some(unit) => {
            let result = advance_inner(w, id, &unit.fit, &mut unit.turning);
            if let Ok(events) = &result {
                if !events.is_empty() {
                    // Record the real pivot in the same Tactical Turn. A
                    // next-tick sample would misattribute tick179 to180.
                    unit.turn_degrees += events
                        .iter()
                        .filter_map(|event| event["angle_deg"].as_f64())
                        .sum::<f64>();
                    if let Some(e) = w.entity(id) {
                        unit.prior_course = e.course_deg;
                    }
                }
            }
            result
        }
        None => Err("3.1.4 source navigation fit missing".into()),
    };
    w.rules.navigation = Some(runtime);
    if let Ok(events) = &result {
        for event in events {
            trace(w, id, "3.1.4.pivot", event.clone());
        }
    }
    result.map(|_| ())
}
fn advance_inner(
    w: &mut World,
    id: EntityId,
    fit: &Fit,
    state: &mut Option<State>,
) -> Result<Vec<Value>, String> {
    let mut e = w.entity(id).ok_or("3.1.4 source ship missing")?.clone();
    let initial_diff = angle_diff(e.course_deg, e.ordered_course_deg);
    let casualty = super::super::damage_runtime::bridge_holds(&e, w.tick)
        || e.ext.get("damage.rudder_jammed").is_some_and(|v| *v > 0.)
        || e.ext.get("damage.rudder_disabled").is_some_and(|v| *v > 0.)
        || e.ext.get("damage.hard_rudder_jammed").is_some_and(|v| *v > 0.);
    // Damaged steering retains its already-versioned casualty procedure.
    // The source normal-rudder table is not promoted as its replacement.
    let values = if initial_diff.abs() > 1e-9 && !casualty {
        Some(row(
            fit,
            e.kind == crate::types::PlatformKind::Sub && e.depth_m > 0.,
        )?)
    } else {
        None
    };
    let s = state.get_or_insert_with(State::default);
    let window = w.tick / tables()["turn_seconds"].as_i64().unwrap();
    if s.window != window {
        s.window = window;
        s.charged_degrees = 0.;
    }
    let mut remaining = crate::world::TICK_SECONDS;
    let mut events = Vec::new();
    let mut land_stop = false;
    if let Some((advance_yd, loss_per45, max_pivot)) = values {
        let distance = advance_yd * tables()["turning"]["yards_to_nm"].as_f64().unwrap();
        let direction = initial_diff.signum() as i8;
        if direction != s.direction {
            s.direction = direction;
            s.advanced_nm = 0.;
        }
        while remaining > 1e-12 {
            let diff = angle_diff(e.course_deg, e.ordered_course_deg);
            if diff.abs() <= 1e-9 {
                s.direction = 0;
                s.advanced_nm = 0.;
                let left_distance = e.speed_kts * remaining / 3600.;
                land_stop = !segment(w, &mut e, left_distance);
                break;
            }
            if e.speed_kts <= 1e-12 {
                break;
            }
            let needed = (distance - s.advanced_nm).max(0.);
            let available = e.speed_kts * remaining / 3600.;
            let travel = available.min(needed);
            if !segment(w, &mut e, travel) {
                land_stop = true;
                break;
            }
            s.advanced_nm += travel;
            remaining = (remaining - travel / e.speed_kts * 3600.).max(0.);
            if s.advanced_nm + 1e-12 < distance {
                break;
            }
            let angle = diff.abs().min(max_pivot);
            let source45 = tables()["turning"]["pivot_deg"].as_f64().unwrap();
            let maximum = tables()["turning"]["maximum_loss_pivots"].as_f64().unwrap() * source45;
            let charged = angle.min((maximum - s.charged_degrees).max(0.));
            let loss = loss_per45 * charged / source45;
            let from = e.course_deg;
            e.course_deg = normalize_deg(from + diff.signum() * angle);
            e.speed_kts = (e.speed_kts - loss).max(0.);
            s.charged_degrees += charged;
            s.advanced_nm = 0.;
            events.push(json!({"source":fit.source,"advance_yd":advance_yd,"advance_nm":distance,"from_course_deg":from,"to_course_deg":e.course_deg,
                "angle_deg":angle,"speed_loss_per45_kts":loss_per45,"speed_loss_kts":loss,"charged_degrees":s.charged_degrees,"turn_window":window,
                "pivot_position":e.pos,"waterjet":fit.waterjet,"partial_loss_policy":"rts_proportional_to_printed45deg"}));
        }
    } else {
        s.advanced_nm = 0.;
        s.direction = 0;
        let distance = e.speed_kts * remaining / 3600.;
        land_stop = !segment(w, &mut e, distance);
    }
    if land_stop {
        s.advanced_nm = 0.;
        s.direction = 0;
        let last = e.ext.get("terrain.last_block").copied().unwrap_or(-100000.);
        if w.tick as f64 - last >= 30. {
            e.ext.insert("terrain.last_block".into(), w.tick as f64);
            w.emit(crate::EventKind::RulesEvent{domain:"navigation".into(),rule:"land_stop".into(),side:Some(e.side),unit:Some(id),contact:None,
                detail:"Charted coast blocks a source advance/pivot segment; stopped before land. Grounding damage not inferred.".into()},crate::Visibility::Side(e.side));
        }
    }
    w.entities.insert(id, e);
    Ok(events)
}

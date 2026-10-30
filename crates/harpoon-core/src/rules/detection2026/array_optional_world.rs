//! World adapter for optional Harpoon V §5.4.2.1.
//!
//! The rule state is referee-side and serializable. It records source-qualified
//! tracks and the observer's initial heading, but never stores target truth
//! geometry. A SensorReport is converted to two bearings mirrored about the
//! array's realized fore/aft axis while the
//! left/right solution is unresolved; the lower numeric value is only a
//! display anchor, not a selected side.
use super::{array_optional as rule, SensorReport, State as DetectionState};
use crate::geom::{angle_diff, normalize_deg};
use crate::{EntityId, World};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub rule_state: rule::State,
    /// Private association and heading at the first successful detection.
    /// No target position/bearing is persisted here.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    bindings: BTreeMap<String, TrackBinding>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TrackBinding {
    observer: EntityId,
    sensor: String,
    array_key: String,
    initial_course_deg: f64,
}

fn track_key(observer: EntityId, sensor: &str, target: EntityId) -> String {
    format!("{}:{}:{}", observer.0, sensor, target.0)
}

fn source_config<'a>(state: &'a DetectionState) -> Option<&'a rule::Configuration> {
    state
        .config
        .optional_towed_array
        .as_ref()
        .filter(|c| c.enabled)
}

fn configured_array(
    w: &World,
    state: &DetectionState,
    observer: EntityId,
    sensor: &str,
) -> Option<(String, rule::ArrayCapability)> {
    let own = w.entity(observer)?;
    let sensor_def = w.db.sensors.get(sensor)?;
    let instance = own.sensors.iter().find(|s| s.sensor == sensor)?;
    let config = source_config(state)?;
    let key = format!("{}:{}", own.class, sensor);
    let cap = config.arrays.get(&key)?.clone();
    let kind_matches = matches!(
        (cap.kind, sensor_def.kind),
        (
            rule::ArrayKind::PassiveTowed,
            crate::db::SensorKind::SonarPassive
        ) | (
            rule::ArrayKind::ActiveTowed,
            crate::db::SensorKind::SonarActive
        )
    );
    if !kind_matches || !instance.is_active_kind(sensor_def.kind) || cap.generation == 0 {
        return None;
    }
    Some((key, cap))
}

fn array_state_mut(state: &mut DetectionState) -> &mut State {
    state.optional_arrays.get_or_insert_with(State::default)
}
pub(super) fn stow_platform(state:&mut DetectionState,observer:EntityId){
    let Some(s)=state.optional_arrays.as_mut() else{return};
    let keys:Vec<_>=s.bindings.iter().filter(|(_,b)|b.observer==observer).map(|(key,_)|key.clone()).collect();
    for key in keys{s.bindings.remove(&key);s.rule_state.tracks.remove(&key);}
}

/// Called before an actual sensor scan. It advances stabilization clocks and
/// observes the owner's current, realized heading; ordered course alone never
/// counts as completed maneuver.
pub fn before_scan(w: &World, state: &mut DetectionState, observer: EntityId, sensor: &str) {
    let Some((array_key, _)) = configured_array(w, state, observer, sensor) else {
        return;
    };
    let Some(config) = source_config(state).cloned() else {
        return;
    };
    let Some(own) = w.entity(observer) else {
        return;
    };
    let now = w.tick;
    let course = own.course_deg;
    let speed = own.speed_kts;
    let ordered_course = own.ordered_course_deg;
    let Some(world_state) = state.optional_arrays.as_mut() else {
        return;
    };
    let keys: Vec<String> = world_state
        .bindings
        .iter()
        .filter(|(_, b)| b.observer == observer && b.sensor == sensor && b.array_key == array_key)
        .map(|(key, _)| key.clone())
        .collect();
    for key in keys {
        let _ = rule::advance(&mut world_state.rule_state, &key, now);
        let Some(binding) = world_state.bindings.get(&key) else {
            continue;
        };
        let signed_change = angle_diff(binding.initial_course_deg, course);
        // This is a floating-point equality tolerance only; the source does
        // not define an additional course-completion angle band.
        let course_complete = angle_diff(course, ordered_course).abs() <= 1e-9;
        let _ = rule::observe_course_change(
            &mut world_state.rule_state,
            &key,
            signed_change,
            course_complete,
            now,
        );
        let established = world_state
            .rule_state
            .tracks
            .get(&key)
            .is_some_and(|t| t.track_established);
        let initial_course_completed = !established
            && course_complete
            && world_state.rule_state.tracks.get(&key).is_some_and(|t| {
                matches!(
                    &t.phase,
                    rule::BearingPhase::BearingAmbiguous
                        | rule::BearingPhase::AwaitingReacquisition { .. }
                )
            });
        let completed_resolution =
            world_state.rule_state.tracks.get(&key).is_some_and(|t| {
                matches!(&t.phase, rule::BearingPhase::AwaitingReacquisition { .. })
            });
        let completed_established_turn = established
            && course_complete
            && world_state
                .rule_state
                .tracks
                .get(&key)
                .is_some_and(|t| matches!(&t.phase, rule::BearingPhase::Maneuvering { .. }));
        if completed_established_turn {
            let _ = rule::stabilize_after_course_change(
                &config,
                &mut world_state.rule_state,
                &key,
                speed,
                now,
            );
        }
        if initial_course_completed || completed_resolution || completed_established_turn {
            if let Some(binding) = world_state.bindings.get_mut(&key) {
                binding.initial_course_deg = course;
            }
        }
    }
}

/// Called after `evaluate_world` has produced the actual sonar decision. Only
/// a real successful scan creates a track or reacquires one after a completed
/// course change; failed/blocked searches do not progress the contact.
pub fn after_search(
    w: &World,
    state: &mut DetectionState,
    observer: EntityId,
    sensor: &str,
    target: EntityId,
    scan: &super::ScanDecision,
) {
    if !scan.detected {
        return;
    }
    let Some((array_key, _capability)) = configured_array(w, state, observer, sensor) else {
        return;
    };
    let Some(config) = source_config(state).cloned() else {
        return;
    };
    let Some(own) = w.entity(observer) else {
        return;
    };
    let key = track_key(observer, sensor, target);
    let now = w.tick;
    let speed = own.speed_kts;
    let course = own.course_deg;
    let world_state = array_state_mut(state);
    let existing = world_state.rule_state.tracks.get(&key).cloned();
    match existing {
        None => {
            if rule::on_detection(&config, &mut world_state.rule_state, &key, &array_key, now)
                .is_ok()
            {
                world_state.bindings.insert(
                    key.clone(),
                    TrackBinding {
                        observer,
                        sensor: sensor.to_owned(),
                        array_key,
                        initial_course_deg: course,
                    },
                );
            }
        }
        Some(track)
            if matches!(
                &track.phase,
                rule::BearingPhase::AwaitingReacquisition { .. }
            ) =>
        {
            let _ = rule::reacquire(&config, &mut world_state.rule_state, &key, speed, now);
        }
        Some(_) => {
            let _ = rule::advance(&mut world_state.rule_state, &key, now);
        }
    }
}

fn current_track<'a>(
    state: &'a DetectionState,
    observer: EntityId,
    sensor: &str,
    target: EntityId,
) -> Option<&'a rule::TrackState> {
    let key = track_key(observer, sensor, target);
    state.optional_arrays.as_ref()?.rule_state.tracks.get(&key)
}

/// Classification remains permitted during source stabilization intervals.
/// An absent/unconfigured optional rule leaves the normal detector unchanged.
pub fn classification_allowed(
    w: &World,
    state: &DetectionState,
    observer: EntityId,
    sensor: &str,
    target: EntityId,
) -> bool {
    let Some(track) = current_track(state, observer, sensor, target) else {
        return true;
    };
    if matches!(&track.phase, rule::BearingPhase::Stabilizing { ready_at_tick, .. } if *ready_at_tick <= w.tick)
    {
        return true;
    }
    rule::classification_allowed(
        &state.optional_arrays.as_ref().unwrap().rule_state,
        &track_key(observer, sensor, target),
    )
}

/// Fire control is barred until the correct bearing is reacquired and the
/// source stabilization interval has elapsed. Ordinary/non-towed detections
/// are unchanged when no optional-rule track exists.
pub fn fire_control_allowed(w: &World, observer: EntityId, sensor: &str, target: EntityId) -> bool {
    let Some(state) = w.rules.detection.as_ref() else {
        return true;
    };
    let Some(track) = current_track(state, observer, sensor, target) else {
        return true;
    };
    let key = track_key(observer, sensor, target);
    let optional = &state.optional_arrays.as_ref().unwrap().rule_state;
    let usable_now = matches!(&track.phase, rule::BearingPhase::Stable)
        || matches!(&track.phase, rule::BearingPhase::Stabilizing { ready_at_tick, .. } if *ready_at_tick <= w.tick);
    rule::fire_control_track_prerequisite(optional, &key) && usable_now
}

/// Publish two antipodal solutions without exposing which one is true until
/// successful reacquisition resolves the side. During stabilization the side
/// is known, but the bearing is unreliable, so retain only that reported
/// side-bearing and suppress coordinates. The FCS guard remains closed.
pub fn apply_report(
    w: &World,
    state: &DetectionState,
    observer: EntityId,
    sensor: &str,
    target: EntityId,
    report: &mut SensorReport,
) {
    let Some(track) = current_track(state, observer, sensor, target) else {
        return;
    };
    match &track.phase {
        rule::BearingPhase::BearingAmbiguous
        | rule::BearingPhase::Maneuvering { .. }
        | rule::BearingPhase::AwaitingReacquisition { .. } => {
            if !track.track_established {
                let Some(own) = w.entity(observer) else {
                    return;
                };
                let alternatives = mirrored_bearings(report.bearing_deg, own.course_deg);
                report.bearing_alternatives = alternatives.to_vec();
                report.bearing_deg = alternatives[0];
            } else {
                // A later kink makes bearings unusable; it does not undo the
                // side already selected by a successful reacquisition.
                report.bearing_alternatives.clear();
            }
            report.measured_position = None;
        }
        rule::BearingPhase::Stabilizing { .. }
        | rule::BearingPhase::StabilizationPending { .. } => {
            report.bearing_alternatives.clear();
            report.measured_position = None;
        }
        rule::BearingPhase::Stable => {}
    }
}

fn mirrored_bearings(bearing_deg: f64, array_axis_deg: f64) -> [f64; 2] {
    let mut alternatives = [
        normalize_deg(bearing_deg),
        normalize_deg(2.0 * array_axis_deg - bearing_deg),
    ];
    alternatives.sort_by(f64::total_cmp);
    alternatives
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_right_solutions_mirror_the_realized_array_axis_without_choosing_a_side() {
        let a = mirrored_bearings(350.0, 0.0);
        assert_eq!(a, [10.0, 350.0]);
        assert_eq!(a, mirrored_bearings(10.0, 0.0));
        assert_eq!(mirrored_bearings(60.0, 90.0), [60.0, 120.0]);
        assert_eq!(mirrored_bearings(120.0, 90.0), [60.0, 120.0]);
    }
}
